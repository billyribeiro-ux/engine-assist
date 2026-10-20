//! `app::logging` (design doc §3.4; SR-57): what Foreman records about itself. A panic is logged
//! with its location only, never its message (which could hold request or repo content).

use std::sync::atomic::{AtomicU32, Ordering};

static CAUGHT: AtomicU32 = AtomicU32::new(0);

/// Installs the panic hook: location only (SR-57).
pub fn install() {
    std::panic::set_hook(Box::new(|info| {
        let at = info.location().map_or_else(
            || "unknown".to_owned(),
            |l| format!("{}:{}", l.file(), l.line()),
        );
        // stderr only until the rotating log file lands (files.md §2 row 25).
        let _ = std::io::Write::write_all(
            &mut std::io::stderr(),
            format!("foreman: panic at {at}\n").as_bytes(),
        );
    }));
}

/// Counts a panic a command handler caught (IPC-R3).
pub fn panic_caught() {
    CAUGHT.fetch_add(1, Ordering::Relaxed);
}

/// How many handler panics were caught (tests).
#[must_use]
pub fn caught() -> u32 {
    CAUGHT.load(Ordering::Relaxed)
}
