//! Non-interactive `--verbose 2` rendering of `SYNC_EVENT_SPAN_NAME`.
//! Concurrent events would otherwise interleave their start/debug/success
//! lines on screen, so everything belonging to one event (start line, every
//! `http_debug` block nested inside it, then success/error) is buffered in
//! the span's own extensions and flushed as one write on close.
//! `http_debug` pushes into the buffer via `append`; outside a sync
//! (`dump`/`diff`/`ping`), there's no such ancestor and it prints directly.

use adc_sdk::SYNC_EVENT_SPAN_NAME;
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::progress::format_scoped_line;

use super::sync_span_fields::SpanFields;

/// Lines waiting to be flushed together when the owning sync_event span
/// closes.
pub struct Buffer(Vec<String>);

/// Appends `line` to the nearest ancestor sync_event span's buffer, or
/// hands it back so the caller can print it itself.
pub fn append<S>(ctx: &Context<'_, S>, id: &Id, line: String) -> Result<(), String>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let Some(span) = ctx.span(id) else {
        return Err(line);
    };
    for ancestor in span.scope() {
        if ancestor.metadata().name() != SYNC_EVENT_SPAN_NAME {
            continue;
        }
        let mut extensions = ancestor.extensions_mut();
        return match extensions.get_mut::<Buffer>() {
            Some(buffer) => {
                buffer.0.push(line);
                Ok(())
            }
            None => Err(line),
        };
    }
    Err(line)
}

pub struct SyncDebugLayer;

impl<S> Layer<S> for SyncDebugLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().name() != SYNC_EVENT_SPAN_NAME {
            return;
        }
        let mut fields = SpanFields::default();
        attrs.record(&mut fields);

        let start_line = format_scoped_line("ADC", '\u{25b6}', "start", &fields.display());
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Buffer(vec![start_line]));
            span.extensions_mut().insert(fields);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        if span.metadata().name() != SYNC_EVENT_SPAN_NAME {
            return;
        }
        let mut extensions = span.extensions_mut();
        if let Some(fields) = extensions.get_mut::<SpanFields>() {
            values.record(fields);
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        if span.metadata().name() != SYNC_EVENT_SPAN_NAME {
            return;
        }
        let mut extensions = span.extensions_mut();
        let Some(Buffer(mut lines)) = extensions.remove::<Buffer>() else {
            return;
        };
        let fields = extensions.remove::<SpanFields>().unwrap_or_default();
        drop(extensions);

        lines.push(match fields.error.as_deref() {
            Some(error) => format_scoped_line(
                "ADC",
                '\u{2716}',
                "error",
                &format!("{}: {error}", fields.display()),
            ),
            None => format_scoped_line("ADC", '\u{2714}', "success", &fields.display()),
        });
        eprintln!("{}", lines.join("\n"));
    }
}
