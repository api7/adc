//! Interactive rendering of `SYNC_EVENT_SPAN_NAME`: a "Syncing N
//! event(s)..." header and a by-event-type counter line — two fixed lines,
//! redrawn in place, pnpm-style (`Progress: resolved N, reused N,
//! downloaded N, added N, done`) — plain text, no progress-bar graphic.
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
use std::time::{Duration, Instant};

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
/// `progress::interactive()`; call `finish` unconditionally afterward.
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

fn render(state: &mut State) {
    // Header has no steady-tick thread anymore; this is its only tick.
    state.header.tick();

    let elapsed = state.started_at.elapsed();
    let percent = state
        .completed
        .checked_mul(100)
        .and_then(|n| n.checked_div(state.total))
        .unwrap_or(100);
    let eta = if state.completed > 0 {
        let secs_per_event = elapsed.as_secs_f64() / state.completed as f64;
        Duration::from_secs_f64(secs_per_event * state.total.saturating_sub(state.completed) as f64)
    } else {
        Duration::ZERO
    };

    state.counts.set_message(format!(
        "created {}, updated {}, deleted {}, failed {}, {}/{} ({percent}%) eta {}",
        state.created,
        state.updated,
        state.deleted,
        state.failed,
        state.completed,
        state.total,
        crate::progress::compact_duration(eta),
    ));
    state.counts.tick();
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

        let mut guard = ACTIVE.lock().expect("not poisoned");
        let Some(state) = guard.as_mut() else { return };
        state.completed += 1;
        let error = fields.as_ref().and_then(|f| f.error.as_deref());
        if error.is_none() {
            match fields.as_ref().map(|f| f.event_type.as_str()) {
                Some("Create") => state.created += 1,
                Some("Update") => state.updated += 1,
                Some("Delete") => state.deleted += 1,
                _ => {}
            }
        }
        if let Some(error) = error {
            state.failed += 1;
            let message = fields.as_ref().map(SpanFields::display).unwrap_or_default();
            let _ = state
                .multi
                .println(format!("\u{1b}[31m\u{2716}  {message}: {error}\u{1b}[0m"));
        }
        render(state);
    }
}
