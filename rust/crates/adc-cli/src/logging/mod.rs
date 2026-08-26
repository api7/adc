//! Wires the library crates' `log::` calls into a `tracing-subscriber`
//! output. Also installs `IndicatifLayer` (`progress`'s spinners),
//! `sync_slots`/`sync_report`/`sync_debug` (interactive slots,
//! non-interactive `--verbose 1`/`2` sync rendering) and `http_debug`
//! (per-request detail), all sharing `IndicatifLayer::get_stderr_writer()`
//! so a log line never fights an active spinner for the same terminal line.
//!
//! `--verbose`: `0` silences everything, `1` shows warnings + spinners, `2`
//! adds `http_debug`'s per-request detail and forces non-interactive
//! rendering even on a real tty (see `progress::interactive`).

use std::io::{IsTerminal, Write};

use tracing_indicatif::IndicatifLayer;
use tracing_indicatif::writer::{IndicatifWriter, Stderr};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod http_debug;
mod sync_debug;
pub mod sync_report;
pub mod sync_slots;
mod sync_span_fields;

/// Any `log::`-bridged line (a plain `log::warn!` in a library crate, say —
/// not one of our own spans) needs to suspend whichever `MultiProgress` is
/// actually on screen before printing, or it tears the output. `IndicatifLayer`
/// only knows how to suspend its own — `sync_slots` hand-rolls a separate
/// one while it's armed, so this checks that first and falls back to
/// `IndicatifLayer`'s otherwise.
struct RoutingWriter {
    fallback: IndicatifWriter<Stderr>,
}

impl<'a> MakeWriter<'a> for RoutingWriter {
    type Writer = Box<dyn Write + Send>;

    fn make_writer(&'a self) -> Self::Writer {
        match sync_slots::active_multi_progress() {
            Some(mp) => Box::new(IndicatifWriter::<Stderr>::new(mp)),
            None => Box::new(self.fallback.clone()),
        }
    }
}

pub fn init(verbose: u8) {
    let log_filter = match verbose {
        0 => "off",
        1 => "warn",
        _ => "warn,adc_sdk=debug,adc_differ=debug,adc_backend_apisix=debug,adc_cli=debug",
    };

    let indicatif_layer = IndicatifLayer::new();
    let show_progress = verbose > 0;

    // Mirrors `progress::interactive()` — can't call it directly, this runs
    // before `progress::set_verbose` does.
    let interactive = std::io::stderr().is_terminal() && verbose < 2;

    let sync_debug_layer = sync_debug::SyncDebugLayer.with_filter(filter_fn(move |metadata| {
        metadata.name() == adc_sdk::SYNC_EVENT_SPAN_NAME && !interactive && verbose == 2
    }));

    let sync_slots_layer = sync_slots::SyncSlotsLayer.with_filter(filter_fn(move |metadata| {
        metadata.name() == adc_sdk::SYNC_EVENT_SPAN_NAME && interactive && verbose > 0
    }));

    let sync_report_layer = sync_report::SyncReportLayer.with_filter(filter_fn(move |metadata| {
        metadata.name() == adc_sdk::SYNC_EVENT_SPAN_NAME && !interactive && verbose == 1
    }));

    // Also admits `sync_event`, not just `http_request`: tracing-subscriber's
    // per-layer filtering makes `Context::span(id).scope()` only walk
    // ancestors *this layer's own filter* accepts — `sync_debug::append`
    // needs to see the enclosing sync_event span, or it can never find it.
    let http_debug_layer = http_debug::HttpDebugLayer.with_filter(filter_fn(move |metadata| {
        verbose == 2
            && (metadata.name() == adc_backend_core::HTTP_REQUEST_SPAN_NAME
                || metadata.name() == adc_sdk::SYNC_EVENT_SPAN_NAME)
    }));

    let routing_writer = RoutingWriter {
        fallback: indicatif_layer.get_stderr_writer(),
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(routing_writer)
                .with_target(false)
                .without_time()
                .with_ansi(interactive)
                .with_filter(
                    // `--verbose 0` is an absolute "silence everything"
                    // promise (see this module's own doc comment), not a
                    // default `RUST_LOG` can override — an unrelated
                    // `RUST_LOG` left in the environment (or loaded from a
                    // stray `.env`) must not un-silence it.
                    if verbose == 0 {
                        EnvFilter::new(log_filter)
                    } else {
                        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_filter))
                    },
                ),
        )
        .with(sync_debug_layer)
        .with(sync_slots_layer)
        .with(sync_report_layer)
        .with(http_debug_layer)
        // Excludes sync_event/http_request: those render through the
        // layers above, not IndicatifLayer's default one-row-per-span
        // (which also panics under concurrent http_request spans).
        .with(indicatif_layer.with_filter(filter_fn(move |metadata| {
            show_progress
                && metadata.name() != adc_sdk::SYNC_EVENT_SPAN_NAME
                && metadata.name() != adc_backend_core::HTTP_REQUEST_SPAN_NAME
        })))
        .init();
}
