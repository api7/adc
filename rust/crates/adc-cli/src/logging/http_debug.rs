//! Renders `HTTP_REQUEST_SPAN_NAME` spans (one per `HttpClient::execute`
//! call) into the multi-line request/response block `--verbose 2` shows.
//! Field names follow OTel HTTP-client semconv where one applies; see
//! `HTTP_REQUEST_SPAN_NAME`'s doc for the full list.
//!
//! Tries `sync_debug::append` first, so a request inside a `sync_event`
//! span folds into that event's buffered block instead of interleaving
//! with concurrent events; outside a sync, prints immediately.

use adc_backend_core::HTTP_REQUEST_SPAN_NAME;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use super::sync_debug;

pub struct HttpDebugLayer;

impl<S> Layer<S> for HttpDebugLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().name() != HTTP_REQUEST_SPAN_NAME {
            return;
        }
        let mut fields = SpanFields::default();
        attrs.record(&mut fields);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(fields);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        if span.metadata().name() != HTTP_REQUEST_SPAN_NAME {
            return;
        }
        let mut extensions = span.extensions_mut();
        if let Some(fields) = extensions.get_mut::<SpanFields>() {
            values.record(fields);
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        if span.metadata().name() != HTTP_REQUEST_SPAN_NAME {
            return;
        }
        let Some(fields) = span.extensions_mut().remove::<SpanFields>() else {
            return;
        };
        // Leading `\n`: the block starts on its own line, not tacked onto
        // the label line, matching the TS CLI's signale-rendered debug block.
        let line = crate::progress::format_scoped_line(
            &fields.scope,
            '\u{2b24}',
            "debug",
            &format!("\n{}", fields.render()),
        );
        if let Err(line) = sync_debug::append(&ctx, &id, line) {
            eprintln!("{line}");
        }
    }
}

#[derive(Default)]
struct SpanFields {
    scope: String,
    description: String,
    method: String,
    url: String,
    request_headers: String,
    request_body: String,
    status_code: Option<u16>,
    response_headers: String,
    response_body: String,
    error_message: String,
}

impl SpanFields {
    /// Same shape as the TS CLI's `buildReqAndRespDebugOutput`: optional
    /// description, `method url`, headers, body, blank line, then status
    /// (reason phrase looked up here from the numeric code) or the error.
    fn render(&self) -> String {
        let mut block = String::new();
        if !self.description.is_empty() {
            block.push_str(&self.description);
            block.push('\n');
        }
        block.push_str(&format!(
            "{} {}\n{}\n",
            self.method, self.url, self.request_headers
        ));
        if !self.request_body.is_empty() {
            block.push_str(&format!("\n{}\n", self.request_body));
        }
        block.push('\n');
        match self.status_code {
            Some(code) => {
                let reason = http::StatusCode::from_u16(code)
                    .ok()
                    .and_then(|s| s.canonical_reason())
                    .unwrap_or("");
                block.push_str(&format!("{code} {reason}\n{}\n", self.response_headers));
                if !self.response_body.is_empty() {
                    block.push_str(&format!("\n{}", self.response_body));
                }
            }
            None => block.push_str(&self.error_message),
        }
        block
    }
}

impl Visit for SpanFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.set(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.set(field.name(), format!("{value:?}"));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() == "http.response.status_code" {
            self.status_code = u16::try_from(value).ok();
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "http.response.status_code" {
            self.status_code = u16::try_from(value).ok();
        }
    }
}

impl SpanFields {
    fn set(&mut self, name: &str, value: String) {
        match name {
            "scope" => self.scope = value,
            "description" => self.description = value,
            "http.request.method" => self.method = value,
            "url.full" => self.url = value,
            "request_headers" => self.request_headers = value,
            "request_body" => self.request_body = value,
            "response_headers" => self.response_headers = value,
            "response_body" => self.response_body = value,
            "error_message" => self.error_message = value,
            _ => {}
        }
    }
}
