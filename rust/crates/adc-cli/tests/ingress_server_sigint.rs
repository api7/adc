//! Black-box test of the ingress-server daemon's graceful shutdown —
//! spawns the real `adc` binary as a subprocess and sends it a real SIGINT.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[tokio::test]
async fn sigint_shuts_down_both_listeners_gracefully_and_exits_zero() {
    let listen_port = free_port();
    let status_port = free_port();

    let mut child = Command::new(env!("CARGO_BIN_EXE_adc"))
        .args([
            "ingress-server",
            "--listen",
            &format!("http://127.0.0.1:{listen_port}"),
            "--listen-status",
            &format!("{status_port}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn the adc binary");
    let pid = child.id().expect("child should have a pid right after spawning") as i32;

    let ready = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(response) = reqwest::get(format!("http://127.0.0.1:{status_port}/healthz/ready")).await
                && response.status().is_success()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(ready.is_ok(), "server never became ready");

    // SAFETY: `pid` is this test's own freshly-spawned, still-alive child.
    let rc = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(rc, 0, "failed to send SIGINT: {}", std::io::Error::last_os_error());

    let exit = timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("process did not exit within the timeout — SIGINT was not handled gracefully")
        .expect("failed to wait on child");
    assert!(exit.success(), "expected exit code 0, got {exit:?}");
}

/// `Ctrl+C` fired before the server finishes starting up should still shut it down promptly.
#[tokio::test]
async fn sigint_before_the_server_is_ready_still_lets_it_exit() {
    let listen_port = free_port();
    let status_port = free_port();

    let mut child = Command::new(env!("CARGO_BIN_EXE_adc"))
        .args([
            "ingress-server",
            "--listen",
            &format!("http://127.0.0.1:{listen_port}"),
            "--listen-status",
            &format!("{status_port}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn the adc binary");
    let pid = child.id().expect("child should have a pid right after spawning") as i32;

    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let _ = timeout(Duration::from_secs(10), lines.next_line()).await;

    let rc = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(rc, 0, "failed to send SIGINT: {}", std::io::Error::last_os_error());

    let exit = timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("process did not exit within the timeout")
        .expect("failed to wait on child");
    assert!(exit.success(), "expected exit code 0, got {exit:?}");
}
