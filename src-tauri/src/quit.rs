//! The quit guard (AC-F009-09, SR-57, SR-24a; M3-13). Quitting Foreman with live sessions asks
//! first in a native dialog raised by Rust (the webview can't answer it); Cancel keeps every
//! session running and sends nothing. After Billy confirms, each child gets SIGHUP, and the runs
//! can be resumed later (their chains keep their session IDs).

use std::sync::atomic::{AtomicBool, Ordering};

use crate::os::Confirm;
use crate::state::App;

/// Set once the guard has let the quit through, so the exit that follows isn't asked again.
static CONFIRMED: AtomicBool = AtomicBool::new(false);
/// Set while the dialog is up, so a second ⌘Q doesn't raise a second dialog.
static ASKING: AtomicBool = AtomicBool::new(false);

/// The quit dialog's text for `n` live sessions (AC-F009-09's wording).
#[must_use]
pub fn message(n: usize) -> String {
    if n == 1 {
        "1 session is running; it ends when Foreman quits; you can resume it later.".to_owned()
    } else {
        format!("{n} sessions are running; they end when Foreman quits; you can resume each later.")
    }
}

/// Whether Foreman may quit now. With no live session it may, without asking. Otherwise Billy is
/// asked in a native dialog; on OK every live session is hung up (SIGHUP) and the answer is yes,
/// on Cancel nothing is sent and the answer is no.
pub fn guard(app: &App) -> bool {
    if CONFIRMED.load(Ordering::SeqCst) {
        return true;
    }
    let n = app.runs().live().len();
    if n == 0 {
        return true;
    }
    let ok = app
        .os()
        .confirm(&Confirm::new("Quit Foreman?", message(n), "Quit"));
    if !ok {
        tracing::info!(target: "app", sessions = n, "quit cancelled");
        return false;
    }
    tracing::info!(target: "app", sessions = n, "quit with live sessions");
    app.runs().hangup_all();
    true
}

/// Claims the dialog: false when one is already up.
pub fn begin_asking() -> bool {
    !ASKING.swap(true, Ordering::SeqCst)
}

/// Releases the dialog after Billy answered.
pub fn end_asking() {
    ASKING.store(false, Ordering::SeqCst);
}

/// Marks the quit as confirmed (the guard said yes; the app is exiting).
pub fn confirm() {
    CONFIRMED.store(true, Ordering::SeqCst);
}

/// Whether the guard already let the quit through.
#[must_use]
pub fn confirmed() -> bool {
    CONFIRMED.load(Ordering::SeqCst)
}
