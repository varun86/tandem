mod support;

use std::time::Duration;
use support::pty_harness::{TestKey, TuiPtyHarness};

#[test]
#[ignore = "requires spawning tandem-tui in PTY; enable for local/nightly runs"]
fn pty_smoke_open_and_close_help_modal() {
    let mut harness = TuiPtyHarness::spawn_tandem_tui().expect("spawn tandem-tui");

    harness
        .wait_for_text("Engine Start", Duration::from_secs(8))
        .expect("initial screen");

    harness.send_key(TestKey::F1).expect("send F1");
    harness
        .wait_for_text("Modal", Duration::from_secs(3))
        .expect("help modal opened");

    harness.send_key(TestKey::Esc).expect("send Esc");
    harness.drain_output();

    let artifact_dir = std::path::Path::new(".tmp/tui-pty-smoke");
    harness
        .dump_artifacts(artifact_dir)
        .expect("dump artifacts");
}
