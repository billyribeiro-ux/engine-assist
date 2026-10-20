//! The main window, built in Rust so navigation and new windows can be denied (SR-29, ADR-014).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::webview::NewWindowResponse;
use tauri::{Manager, Runtime, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// The webview may navigate only within the app origin (SR-29).
#[must_use]
pub fn is_app_origin(u: &Url) -> bool {
    if u.scheme() == "ipc" {
        return true;
    }
    let clean = u.username().is_empty() && u.password().is_none() && u.port().is_none();
    clean
        && matches!(
            (u.scheme(), u.host_str()),
            ("tauri", Some("localhost")) | ("http", Some("tauri.localhost"))
        )
}

/// The Vite dev server (debug builds only).
#[must_use]
pub fn is_dev_server(u: &Url) -> bool {
    u.scheme() == "http"
        && u.host_str() == Some("localhost")
        && u.port() == Some(5173)
        && u.username().is_empty()
        && u.password().is_none()
}

/// SR-29 navigation rule.
#[must_use]
pub fn navigation_allowed(u: &Url) -> bool {
    is_app_origin(u) || (cfg!(debug_assertions) && is_dev_server(u))
}

static REDUCED_EFFECTS: AtomicBool = AtomicBool::new(false);

/// Records what the webview's `prefers-reduced-transparency`/`prefers-contrast` report
/// (`settings_update.osDisplay`, AC-XC-23). Window vibrancy arrives with M2-03 and reads this.
pub fn set_reduced_effects(on: bool) {
    REDUCED_EFFECTS.store(on, Ordering::Relaxed);
}

/// Whether window effects are off (reduce transparency or increase contrast).
#[must_use]
pub fn reduced_effects() -> bool {
    REDUCED_EFFECTS.load(Ordering::Relaxed)
}

/// Builds the `main` window (the only window in v1; `capabilities/main-window.json`).
///
/// # Errors
/// Any Tauri window-creation error.
pub fn build_main<R: Runtime, M: Manager<R>>(manager: &M) -> tauri::Result<WebviewWindow<R>> {
    let builder = WebviewWindowBuilder::new(manager, "main", WebviewUrl::App("index.html".into()))
        .title("Foreman")
        .inner_size(1280.0, 820.0)
        .min_inner_size(960.0, 600.0)
        .on_navigation(navigation_allowed)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .disable_drag_drop_handler();
    // macOS-only chrome (ADR-014). Window effects (vibrancy) arrive with M2-03.
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::{is_app_origin, navigation_allowed};
    use tauri::Url;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    /// `webview::navigation_denied_to_remote` (predicate part; the in-app check is M2-03).
    #[test]
    fn navigation_denied_to_remote() {
        for ok in [
            "tauri://localhost/",
            "tauri://localhost/projects/x",
            "http://tauri.localhost/",
        ] {
            assert!(navigation_allowed(&url(ok)), "{ok}");
        }
        for bad in [
            "https://claude.ai/",
            "http://localhost.evil.com/",
            "tauri://localhost.evil/",
            "http://user@tauri.localhost/",
            "http://tauri.localhost:8080/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
        ] {
            assert!(!navigation_allowed(&url(bad)), "{bad}");
        }
        assert!(is_app_origin(&url("ipc://localhost/app_bootstrap")));
    }
}
