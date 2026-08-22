//! Interactive rendering of `SYNC_EVENT_SPAN_NAME`: a "Syncing N
//! event(s)..." header and a by-event-type counter line — two fixed lines,
//! redrawn in place, like a package manager's `Progress: resolved N,
//! reused N, downloaded N, added N, done` line — plain text, no
//! progress-bar graphic.
//!
//! Replaces an earlier per-event-slot design (`N` reusable spinner rows,
//! `N` = `--request-concurrent`), retired because enough concurrently
//! ticking, variable-length lines made indicatif's `MultiProgress` redraw
//! bookkeeping drift and leave stale frames on screen. Fixed line count and
//! width here removes that surface instead of narrowing it.
//!
//! Driven by real span lifecycle, not a synthetic start/finish pair — the
//! same span a future OTel export layer would use unmodified.

use std::sync::Mutex;
use std::time::Instant;

use adc_sdk::SYNC_EVENT_SPAN_NAME;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use super::sync_span_fields::SpanFields;

static ACTIVE: Mutex<Option<State>> = Mutex::new(None);

/// The armed display's own draw surface, if any — `logging`'s writer
/// routes every log line through this (instead of `IndicatifLayer`'s own
/// `MultiProgress`) while it's active, so nothing prints without first
/// suspending it.
pub fn active_multi_progress() -> Option<MultiProgress> {
    ACTIVE
        .lock()
        .expect("not poisoned")
        .as_ref()
        .map(|s| s.multi.clone())
}

struct State {
    multi: MultiProgress,
    header: ProgressBar,
    counts: ProgressBar,
    started_at: Instant,
    total: u64,
    completed: u64,
    created: u64,
    updated: u64,
    deleted: u64,
    failed: u64,
}

/// Call once, right before `Backend::sync`, only when
/// `progress::interactive() && progress::verbose() > 0` — `interactive()`
/// alone doesn't rule out `verbose == 0`, which the actual redraw layer
/// (`SyncSlotsLayer`'s own filter) does require, so calling this without
/// also checking `verbose` draws a header and a stuck-at-zero counts line
/// that nothing ever updates. Call `finish` unconditionally afterward.
pub fn start(total: u64) {
    let multi = MultiProgress::new();

    // No `enable_steady_tick`: it spawns a background thread that redraws
    // outside the `ACTIVE` lock `render()` serializes through — a second,
    // unsynchronized path into the same draw target, which tore output in
    // practice. Every redraw now goes through `render()` only.
    let header_style =
        ProgressStyle::with_template("{spinner:.cyan} {msg}").expect("static template is valid");
    let header = multi.add(ProgressBar::new_spinner());
    header.set_style(header_style);
    header.set_message(format!("Syncing {total} event(s)..."));

    let plain_style = ProgressStyle::with_template("  {msg}").expect("static template is valid");
    let counts = multi.add(ProgressBar::new_spinner());
    counts.set_style(plain_style);
    counts.set_message(format!(
        "created 0, updated 0, deleted 0, failed 0, 0/{total} (0%) eta -"
    ));

    *ACTIVE.lock().expect("not poisoned") = Some(State {
        multi,
        header,
        counts,
        started_at: Instant::now(),
        total,
        completed: 0,
        created: 0,
        updated: 0,
        deleted: 0,
        failed: 0,
    });
}

pub fn finish() {
    let Some(state) = ACTIVE.lock().expect("not poisoned").take() else {
        return;
    };
    // Header ("Syncing N event(s)...") is just a transient status, cleared
    // once done. Counts is the final tally — sync may have exited early on
    // a failure with some events already applied, so it stays on screen
    // instead of vanishing along with the header.
    state.header.finish_and_clear();
    state.counts.finish();
}

/// A snapshot of what `render_frame` needs, copied out of `State` while
/// `ACTIVE`'s lock is held — `multi`/`header`/`counts` are cheap,
/// `Arc`-backed clones, so the actual redraw (real terminal I/O) then
/// happens without holding the lock, which would otherwise stall any
/// concurrent `active_multi_progress()` caller for the duration of a paint.
struct Frame {
    multi: MultiProgress,
    header: ProgressBar,
    counts: ProgressBar,
    started_at: Instant,
    total: u64,
    completed: u64,
    created: u64,
    updated: u64,
    deleted: u64,
    failed: u64,
}

fn render_frame(frame: &Frame) {
    // Header has no steady-tick thread anymore; this is its only tick.
    frame.header.tick();

    let elapsed = frame.started_at.elapsed();
    let (percent, eta) = crate::progress::percent_and_eta(frame.completed, frame.total, elapsed);

    frame.counts.set_message(format!(
        "created {}, updated {}, deleted {}, failed {}, {}/{} ({percent}%) eta {}",
        frame.created,
        frame.updated,
        frame.deleted,
        frame.failed,
        frame.completed,
        frame.total,
        crate::progress::compact_duration(eta),
    ));
    frame.counts.tick();
}

pub struct SyncSlotsLayer;

impl<S> Layer<S> for SyncSlotsLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().name() != SYNC_EVENT_SPAN_NAME {
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
        let fields = span.extensions_mut().remove::<SpanFields>();
        let error = fields.as_ref().and_then(|f| f.error.as_deref());

        let frame = {
            let mut guard = ACTIVE.lock().expect("not poisoned");
            let Some(state) = guard.as_mut() else { return };
            state.completed += 1;
            if error.is_none() {
                match fields.as_ref().map(|f| f.event_type.as_str()) {
                    Some("Create") => state.created += 1,
                    Some("Update") => state.updated += 1,
                    Some("Delete") => state.deleted += 1,
                    _ => {}
                }
            } else {
                state.failed += 1;
            }
            Frame {
                multi: state.multi.clone(),
                header: state.header.clone(),
                counts: state.counts.clone(),
                started_at: state.started_at,
                total: state.total,
                completed: state.completed,
                created: state.created,
                updated: state.updated,
                deleted: state.deleted,
                failed: state.failed,
            }
        };

        if let Some(error) = error {
            let message = fields.as_ref().map(SpanFields::display).unwrap_or_default();
            let _ = frame
                .multi
                .println(format!("\u{1b}[31m\u{2716}  {message}: {error}\u{1b}[0m"));
        }
        render_frame(&frame);
    }
}
