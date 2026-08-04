//! Non-interactive (`--verbose 1`) rendering of `SYNC_EVENT_SPAN_NAME`:
//! periodic running-total snapshot lines (pnpm's `--reporter=append-only`
//! style) instead of one start+success line per event — the old approach
//! floods a large sync's log (12000 lines for 6000 events). Failures still
//! print immediately.
//!
//! `--verbose 2` goes through `http_debug`/`sync_debug` instead, not this.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use adc_sdk::SYNC_EVENT_SPAN_NAME;
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::progress::{compact_duration, print_line};

use super::sync_span_fields::SpanFields;

/// Not tied to event count — a fixed "every N events" would spam at high
/// concurrency and stall at low.
const REPORT_INTERVAL: Duration = Duration::from_secs(2);

static ACTIVE: Mutex<Option<Report>> = Mutex::new(None);

struct Report {
    total: u64,
    completed: u64,
    failed: u64,
    started_at: Instant,
    last_report: Instant,
}

/// Call once, right before `Backend::sync`, only when
/// `!progress::interactive() && progress::verbose() == 1`.
pub fn start(total: u64) {
    print_line('\u{25b6}', "start", &format!("Syncing {total} event(s)..."));
    let now = Instant::now();
    *ACTIVE.lock().expect("not poisoned") = Some(Report {
        total,
        completed: 0,
        failed: 0,
        started_at: now,
        last_report: now,
    });
}

pub fn finish() {
    *ACTIVE.lock().expect("not poisoned") = None;
}

pub struct SyncReportLayer;

impl<S> Layer<S> for SyncReportLayer
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
        let Some(report) = guard.as_mut() else { return };
        report.completed += 1;

        if let Some(error) = fields.as_ref().and_then(|f| f.error.as_deref()) {
            report.failed += 1;
            let message = fields.as_ref().map(SpanFields::display).unwrap_or_default();
            print_line('\u{2716}', "error", &format!("{message}: {error}"));
        }

        // Force a final snapshot so it doesn't sit at "5998/6000" until the
        // separate "Sync completed" line.
        let done = report.completed >= report.total;
        if done || report.last_report.elapsed() >= REPORT_INTERVAL {
            report.last_report = Instant::now();
            print_progress(report);
        }
    }
}

fn print_progress(report: &Report) {
    let elapsed = report.started_at.elapsed();
    let (percent, eta) = crate::progress::percent_and_eta(report.completed, report.total, elapsed);

    print_line(
        '\u{2026}',
        "progress",
        &format!(
            "{}/{} ({percent}%) applied, {} failed, elapsed {} eta {}",
            report.completed,
            report.total,
            report.failed,
            compact_duration(elapsed),
            compact_duration(eta),
        ),
    );
}
