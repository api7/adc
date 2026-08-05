//! A dedicated bootstrap step, meant to run once in CI before every other
//! e2e test file (see `.github/workflows/e2e.yaml`'s `api7-rust` job): logs
//! in, rotates the admin password, activates the license, and mints a
//! token — the same dance `tests/common/mod.rs::bootstrap_token` runs
//! lazily on first use, but performed here exactly once and shared via
//! `$GITHUB_ENV`'s `TOKEN`, so every other test binary (each its own
//! process, with no state shared between them) picks up the result instead
//! of independently repeating the dance against the same live dashboard —
//! `common::token()` already prefers an externally-set `TOKEN` over running
//! `bootstrap_token()` itself.
//!
//! Running a single e2e test file locally, without this step first, still
//! works: `bootstrap_token`'s own login step tolerates an already-rotated
//! admin password (see its doc comment), so nothing here is load-bearing
//! outside CI's multi-binary run.

use std::io::Write;

mod common;

#[tokio::test]
#[ignore]
async fn bootstrap_shared_token() {
    let token = common::token().await;

    let Ok(github_env) = std::env::var("GITHUB_ENV") else {
        // Not running in CI — nothing to share the token with.
        return;
    };
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&github_env)
        .unwrap_or_else(|e| panic!("opening $GITHUB_ENV ({github_env}): {e}"));
    writeln!(file, "TOKEN<<EOTOKEN").unwrap();
    writeln!(file, "{token}").unwrap();
    writeln!(file, "EOTOKEN").unwrap();
}
