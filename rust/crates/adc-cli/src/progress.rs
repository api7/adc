//! Per-stage progress display, replacing `listr2`'s task-list rendering.
//! Interactive: `stage` runs the future inside a span that `IndicatifLayer`
//! turns into a spinner. Non-interactive: prints plain
//! `[time] [ADC] › start/success  message` lines (matches the TS CLI's
//! `signale` renderer, which never redraws regardless of tty).
//!
//! `--verbose 0` silences both modes.

use std::future::Future;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicU8, Ordering};

use indicatif::ProgressStyle;
use tracing::Instrument;
use tracing_indicatif::span_ext::IndicatifSpanExt;

static VERBOSE: AtomicU8 = AtomicU8::new(1);

/// `{spinner} {msg}` with the finished frame recolored to `finish_glyph`
/// instead of indicatif's default blank space (which would leave the
/// finish message one column off from where the spinner had been ticking).
/// Colored per-frame via `tick_strings` since the template's `:.cyan`
/// modifier can't color the spinning and finished frames differently.
fn colored_spinner_style(finish_color: &str, finish_glyph: char) -> ProgressStyle {
    const SPIN_FRAMES: &str = "⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈";
    let mut frames: Vec<String> = SPIN_FRAMES
        .chars()
        .map(|c| format!("\u{1b}[36m{c}\u{1b}[0m"))
        .collect();
    frames.push(format!("{finish_color}{finish_glyph}\u{1b}[0m"));
    let frame_refs: Vec<&str> = frames.iter().map(String::as_str).collect();

    ProgressStyle::with_template("{spinner} {msg}")
        .expect("static template is valid")
        .tick_strings(&frame_refs)
}

/// `stage`'s style: green `✓` on finish.
pub fn spinner_style() -> ProgressStyle {
    colored_spinner_style("\u{1b}[32m", '\u{2713}')
}

/// `info`'s style: blue `ℹ` on finish.
fn info_style() -> ProgressStyle {
    colored_spinner_style("\u{1b}[34m", '\u{2139}')
}

pub fn set_verbose(verbose: u8) {
    VERBOSE.store(verbose, Ordering::Relaxed);
}

pub fn verbose() -> u8 {
    VERBOSE.load(Ordering::Relaxed)
}

/// `--verbose 2` forces non-interactive even on a real tty: its per-request
/// debug spans render through `sync_slots`'s own `MultiProgress`, a
/// separate draw surface from `IndicatifLayer`'s — the two can't coordinate
/// redraws, so mixing them corrupts the screen.
pub fn interactive() -> bool {
    std::io::stderr().is_terminal() && VERBOSE.load(Ordering::Relaxed) < 2
}

pub async fn stage<F, T>(message: &str, fut: F) -> T
where
    F: Future<Output = T>,
{
    if VERBOSE.load(Ordering::Relaxed) == 0 {
        return fut.await;
    }

    if interactive() {
        let span = tracing::info_span!("adc_stage");
        span.pb_set_style(&spinner_style());
        span.pb_set_message(message);
        span.pb_set_finish_message(message);
        fut.instrument(span).await
    } else {
        print_line('\u{25b6}', "start", message);
        let result = fut.await;
        print_line('\u{2714}', "success", message);
        result
    }
}

pub fn finish_ok() {
    if VERBOSE.load(Ordering::Relaxed) > 0 && !interactive() {
        print_line('\u{2605}', "star", "All is well, see you next time!");
    }
}

/// A `signale` `info`-styled one-shot line — a fact being reported, not a
/// `stage`-style task with a start/finish lifecycle. Interactive mode still
/// goes through `IndicatifLayer` (span entered and immediately dropped, no
/// future to await) rather than a bare `eprintln!`, so it lines up visually
/// with the `stage` lines above it.
pub fn info(message: &str) {
    if VERBOSE.load(Ordering::Relaxed) == 0 {
        return;
    }

    if interactive() {
        let span = tracing::info_span!("adc_info");
        span.pb_set_style(&info_style());
        span.pb_set_message(message);
        span.pb_set_finish_message(message);
        let _entered = span.enter();
    } else {
        print_line('\u{2139}', "info", message);
    }
}

pub fn print_line(icon: char, label: &str, message: &str) {
    print_scoped_line("ADC", icon, label, message);
}

/// `print_line` with an explicit scope tag instead of `ADC` — backend-owned
/// lines (`http_debug`/`sync_debug`) use their backend's own
/// `BackendMetadata::log_scope` (`APISIX`), matching the TS CLI's
/// `SignaleRenderer`.
pub fn print_scoped_line(scope: &str, icon: char, label: &str, message: &str) {
    eprintln!("{}", format_scoped_line(scope, icon, label, message));
}

/// `print_scoped_line`, minus the printing — `sync_debug` buffers lines
/// (each keeping its own timestamp) and flushes a whole event's block in
/// one write instead of interleaving with concurrent events.
pub fn format_scoped_line(scope: &str, icon: char, label: &str, message: &str) -> String {
    let now = chrono::Local::now().format("%I:%M:%S %p");
    let meta = format!("[{now}] [{scope}] \u{203a}");
    let meta = if std::io::stderr().is_terminal() {
        format!("\u{1b}[90m{meta}\u{1b}[0m")
    } else {
        meta
    };
    format!("{meta} {}{message}", colored_icon_and_label(icon, label))
}

/// Matches `node_modules/signale/types.js`'s palette: `start`/`success`
/// green, `error`/`debug` red, `star` yellow, everything else (`info`,
/// `progress` — no TS counterpart) blue.
fn colored_icon_and_label(icon: char, label: &str) -> String {
    let padded = format!("{label:<10}");
    if !std::io::stderr().is_terminal() {
        return format!("{icon}  {padded}");
    }
    let color = match label {
        "start" | "success" => "\u{1b}[32m",
        "error" | "debug" => "\u{1b}[31m",
        "star" => "\u{1b}[33m",
        _ => "\u{1b}[34m",
    };
    let trimmed_len = label.len();
    let pad = &padded[trimmed_len..];
    format!("{color}{icon}\u{1b}[0m  {color}\u{1b}[4m{label}\u{1b}[0m{pad}")
}

/// `12s` / `3m05s` / `1h02m`.
pub fn compact_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_style_ends_on_a_green_checkmark() {
        let final_frame = spinner_style().get_final_tick_str().to_string();
        assert!(
            final_frame.contains('\u{2713}'),
            "{final_frame:?} should contain a checkmark"
        );
        assert!(
            final_frame.contains("\u{1b}[32m"),
            "{final_frame:?} should be green"
        );
    }
}
