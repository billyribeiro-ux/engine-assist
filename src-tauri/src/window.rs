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

/// What the main window does when the page asks for another window (`window.open`, a
/// `target="_blank"` link, a script-opened popup): always deny (SR-29, AC-XC-27). Foreman has one
/// window; links to the outside go through `terminal_link_open` and `/usr/bin/open -u` only.
#[must_use]
pub fn new_window_response<R: Runtime>(_requested: &Url) -> NewWindowResponse<R> {
    NewWindowResponse::Deny
}

static REDUCED_EFFECTS: AtomicBool = AtomicBool::new(false);

/// Records what the webview's `prefers-reduced-transparency`/`prefers-contrast` report
/// (`settings_update.osDisplay`, AC-XC-23). The window is opaque today (a solid `--sidebar` stands in for vibrancy); `Os::set_window_reduced_effects` clears any window effect when this is on.
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
        .on_new_window(|requested, _| new_window_response(&requested))
        .disable_drag_drop_handler();
    // macOS-only chrome (ADR-014). No window effect is applied yet; Reduce Transparency or Increase Contrast would clear one (`Os::set_window_reduced_effects`).
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    builder.build()
}

#[cfg(test)]
mod webview {
    use super::{build_main, is_app_origin, navigation_allowed, new_window_response};
    use tauri::Url;
    use tauri::webview::NewWindowResponse;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    /// `webview::navigation_denied_to_remote` (AC-XC-27, SR-29): the predicate handed to
    /// `on_navigation` allows only the app origin (and the dev server in debug builds).
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
            "http://localhost:5174/",
            "http://localhost:5173@evil.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
            "about:blank",
            "blob:tauri://localhost/x",
        ] {
            assert!(!navigation_allowed(&url(bad)), "{bad}");
        }
        assert!(is_app_origin(&url("ipc://localhost/app_bootstrap")));
        // The dev server is a debug-build convenience only.
        assert_eq!(
            navigation_allowed(&url("http://localhost:5173/")),
            cfg!(debug_assertions)
        );
    }

    /// `webview::window_open_denied` (AC-XC-27, SR-29): every request for a new window is denied,
    /// the app's own origin included (there is one window).
    #[test]
    fn window_open_denied() {
        for u in [
            "https://claude.ai/",
            "https://example.invalid/",
            "http://tauri.localhost/",
            "tauri://localhost/",
            "about:blank",
            "javascript:alert(1)",
            "file:///Applications/Calculator.app",
        ] {
            assert!(
                matches!(
                    new_window_response::<tauri::test::MockRuntime>(&url(u)),
                    NewWindowResponse::Deny
                ),
                "{u}"
            );
        }
    }

    /// Both handlers are wired into the one place the window is built, and the window is built
    /// in Rust (no `windows` entry in `tauri.conf.json`), so a config edit can't bring back an
    /// unguarded window.
    #[test]
    fn main_window_is_built_in_rust_with_both_handlers() {
        let src = include_str!("window.rs");
        let build = &src[src.find("pub fn build_main").unwrap()..src.find("#[cfg(test)]").unwrap()];
        assert!(build.contains(".on_navigation(navigation_allowed)"));
        assert!(build.contains(".on_new_window(|requested, _| new_window_response(&requested))"));
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let windows = &conf["app"]["windows"];
        assert!(
            windows.is_null() || windows.as_array().is_some_and(Vec::is_empty),
            "tauri.conf.json must not declare windows: {windows}"
        );
        // And it builds, once, as `main`.
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let w = build_main(&app).unwrap();
        assert_eq!(w.label(), "main");
    }
}
