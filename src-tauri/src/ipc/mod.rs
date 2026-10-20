//! The IPC boundary (ADR-004, `docs/contracts/ipc.md`). Every command takes exactly one request
//! struct named `req` with `deny_unknown_fields`, wrapped in [`Req`] so a malformed request becomes
//! a typed `Invalid` error instead of Tauri's untyped string (IPC-R1, IPC-R3). Handlers run on the
//! blocking pool (never the main thread, so native dialogs can block) with panics caught.

pub mod commands;
pub mod dto;
pub mod error;
pub mod handlers;
pub mod types;

mod commands_list;

pub use commands_list::COMMANDS;
pub use error::{IpcError, IpcErrorCode, IpcErrorDetail};

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

use crate::state::App;

/// The managed app state every command receives.
pub type Shared = Arc<App>;

/// A command's request argument. Deserializing it never fails at the Tauri layer; the shape
/// check happens in [`Req::into_inner`], which returns a typed [`IpcError`].
#[derive(Debug)]
pub struct Req<T>(Result<T, IpcError>);

impl<T> Req<T> {
    /// The validated request, or `Invalid`.
    ///
    /// # Errors
    /// `IpcErrorCode::Invalid` when the JSON didn't match `T` (unknown field, wrong type, missing
    /// field, or a scalar that broke its rule).
    pub fn into_inner(self) -> Result<T, IpcError> {
        self.0
    }

    /// A request built in Rust (tests, `do_next` calling `launch_prepare`).
    #[must_use]
    pub fn new(v: T) -> Self {
        Self(Ok(v))
    }
}

/// The `Invalid` rule for a serde error: `unknownField`, a scalar's own rule (`fm-invalid:<rule>`,
/// see [`types::INVALID_MARKER`]) or `shape`. Never echoes the value (SR-57).
fn rule_of(e: &serde_json::Error) -> String {
    let text = e.to_string();
    if text.starts_with("unknown field") {
        return "unknownField".to_owned();
    }
    if let Some(rest) = text
        .find(types::INVALID_MARKER)
        .map(|i| &text[i + types::INVALID_MARKER.len()..])
    {
        let rule: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if !rule.is_empty() {
            return rule;
        }
    }
    "shape".to_owned()
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Req<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(Self(
            serde_json::from_value(raw).map_err(|e| IpcError::invalid("req", &rule_of(&e))),
        ))
    }
}

/// Runs a handler for a command (IPC-R3): validates the request, runs on the blocking pool, and
/// turns a panic into `Internal` (logged with the location only, SR-57).
///
/// # Errors
/// `Invalid` for a malformed request; whatever the handler returns; `Internal` on a panic.
pub async fn call<T, R, F>(
    state: &tauri::State<'_, Shared>,
    req: Req<T>,
    f: F,
) -> Result<R, IpcError>
where
    T: Send + 'static,
    R: Send + 'static,
    F: FnOnce(&App, T) -> Result<R, IpcError> + Send + 'static,
{
    let req = req.into_inner()?;
    let app: Shared = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || run_caught(&app, req, f))
        .await
        .unwrap_or_else(|_| Err(IpcError::internal()))
}

/// [`call`]'s body without Tauri: the handler with panics caught (also used by tests).
///
/// # Errors
/// As the handler; `Internal` on a panic.
pub fn run_caught<T, R, F>(app: &App, req: T, f: F) -> Result<R, IpcError>
where
    F: FnOnce(&App, T) -> Result<R, IpcError>,
{
    std::panic::catch_unwind(AssertUnwindSafe(|| f(app, req))).unwrap_or_else(|_| {
        crate::logging::panic_caught();
        Err(IpcError::internal())
    })
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, IpcErrorCode, IpcErrorDetail, Req};
    use crate::ipc::dto::requests::{AppBootstrapReq, StepViewReq};

    /// IPC-R1: an extra field is `Invalid` (the per-command cases are in `tests/ipc.rs`).
    #[test]
    fn rejects_unknown_fields() {
        let req: Req<AppBootstrapReq> =
            serde_json::from_value(serde_json::json!({ "extra": 1 })).unwrap();
        let err = req.into_inner().unwrap_err();
        assert_eq!(err.code, IpcErrorCode::Invalid);
        assert_eq!(
            err.detail,
            Some(IpcErrorDetail::Invalid {
                field: "req".into(),
                rule: "unknownField".into()
            })
        );
        assert!(
            !err.message.contains("extra"),
            "messages never echo request content"
        );

        let ok: Req<AppBootstrapReq> = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(ok.into_inner().is_ok());
    }

    /// IPC-R2: a scalar that breaks its rule names the rule, never the value.
    #[test]
    fn scalar_rules_are_named() {
        let req: Req<StepViewReq> = serde_json::from_value(serde_json::json!({
            "projectId": "7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11",
            "step": "../../etc/passwd"
        }))
        .unwrap();
        let err = req.into_inner().unwrap_err();
        assert_eq!(
            err.detail,
            Some(IpcErrorDetail::Invalid {
                field: "req".into(),
                rule: "stepKey".into()
            })
        );
        assert!(!err.message.contains("passwd"));
    }

    /// `ipc::commands_json`: `src/lib/ipc/bindings/commands.json` is `COMMANDS`, sorted.
    #[test]
    fn commands_json() {
        let mut sorted: Vec<&str> = COMMANDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, COMMANDS, "COMMANDS must stay sorted");
        assert_eq!(COMMANDS.len(), 91, "docs/contracts/ipc.md §4.15");
        let expected = format!("{}\n", serde_json::to_string_pretty(&sorted).unwrap());
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/lib/ipc/bindings/commands.json"
        );
        if cfg!(feature = "ts") {
            #[allow(
                clippy::disallowed_methods,
                reason = "test code writes the generated bindings"
            )]
            std::fs::write(path, &expected).unwrap();
        }
        let on_disk = include_str!("../../../src/lib/ipc/bindings/commands.json");
        if !cfg!(feature = "ts") {
            assert_eq!(
                on_disk, expected,
                "regenerate with `cargo test -p foreman-app --features ts`"
            );
        }
    }
}
