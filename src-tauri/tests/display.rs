//! M5-04 (AC-XC-23): the webview reports `prefers-reduced-transparency` and `prefers-contrast:
//! more` through `settings_update.osDisplay`; Rust turns the window's effects off. The value is
//! never stored, and Rust never reads the OS preference itself.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::project::{ProjectApp, req};
use foreman_app::ipc::commands::settings;
use serde_json::{Value, json};

fn patch(display: Value) -> Value {
    json!({
        "driName": null, "editor": null, "theme": null, "terminal": null,
        "envPassthrough": null, "globalVariables": null, "resetTool": null,
        "clearLoginPath": null, "removeAllowlistEntry": null, "screenReaderMode": null,
        "osDisplay": display
    })
}

mod display {
    use super::*;

    fn rev(p: &ProjectApp) -> u64 {
        let v = settings::settings_get(&p.t.app, req(json!({}))).unwrap();
        serde_json::to_value(&v).unwrap()["rev"].as_u64().unwrap()
    }

    /// Reduce Transparency, Increase Contrast, or both, turn window effects off; neither on says so.
    #[test]
    fn os_display_reaches_the_window_effects_seam() {
        let p = ProjectApp::new(json!({}));
        for (rt, hc, expect) in [
            (true, false, true),
            (false, true, true),
            (true, true, true),
            (false, false, false),
        ] {
            let r = rev(&p);
            settings::settings_update(
                &p.t.app,
                req(json!({
                    "expectRev": r,
                    "patch": patch(json!({ "reduceTransparency": rt, "increaseContrast": hc }))
                })),
            )
            .unwrap();
            assert_eq!(p.t.os.reduced_effects().last(), Some(&expect), "{rt} {hc}");
        }
        assert_eq!(p.t.os.reduced_effects().len(), 4);
    }

    /// A patch without `osDisplay` leaves the window alone.
    #[test]
    fn a_patch_without_it_changes_nothing() {
        let p = ProjectApp::new(json!({}));
        let r = rev(&p);
        settings::settings_update(
            &p.t.app,
            req(json!({ "expectRev": r, "patch": patch(Value::Null) })),
        )
        .unwrap();
        assert!(p.t.os.reduced_effects().is_empty());
    }
}
