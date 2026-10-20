//! criterion bench for the sensor's 10 ms budget (SR-15, AC-F011-04, ADR-009 "criterion on the
//! binary"): spawn the installed binary with a typical `Stop` payload and wait for it to exit.
//! The pass/fail gate is the test `sensor::exits_under_10ms`; this bench tracks the trend.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "bench code: unwrap/expect are denied only outside tests (SR-62)"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "the bench runs the sensor binary and sets up a fake app-data folder (test code, ADR-003)"
)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use criterion::{Criterion, criterion_group, criterion_main};

const RUN: &str = "0b5c9d8e-1f2a-4b3c-9d4e-5f6a7b8c9d0e";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const STOP: &[u8] = br#"{"session_id":"s1","hook_event_name":"Stop","last_assistant_message":"done","stop_hook_active":false}"#;

fn exit_time(c: &mut Criterion) {
    let tmp = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let bin = tmp.path().join("bin");
    let run_dir = tmp.path().join("events").join(RUN);
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&run_dir).unwrap();
    for d in [tmp.path().join("events"), run_dir] {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let exe = bin.join("foreman-sensor-0123456789ab");
    std::fs::hard_link(env!("CARGO_BIN_EXE_foreman-sensor"), &exe).unwrap();
    let argv = [
        "--run", RUN, "--token", TOKEN, "--event", "Stop", "--v", "1",
    ];

    c.bench_function("sensor_stop_event_spawn_to_exit", |b| {
        b.iter(|| {
            let mut child = Command::new(&exe)
                .args(argv)
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(STOP).unwrap();
            assert!(child.wait().unwrap().success());
        });
    });
}

criterion_group!(benches, exit_time);
criterion_main!(benches);
