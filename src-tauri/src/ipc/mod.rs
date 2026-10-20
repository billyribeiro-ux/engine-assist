//! The IPC boundary (ADR-004, `docs/contracts/ipc.md`). Every command takes exactly one request
//! struct named `req` with `deny_unknown_fields`, wrapped in [`Req`] so a malformed request becomes
//! a typed `Invalid` error instead of Tauri's untyped string (IPC-R1, IPC-R3).

pub mod commands;
pub mod error;
pub mod types;

mod commands_list;

pub use commands_list::COMMANDS;
pub use error::{IpcError, IpcErrorCode, IpcErrorDetail};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

/// A command's request argument. Deserializing it never fails at the Tauri layer; the shape
/// check happens in [`Req::into_inner`], which returns a typed [`IpcError`].
#[derive(Debug)]
pub struct Req<T>(Result<T, IpcError>);

impl<T> Req<T> {
    /// The validated request, or `Invalid`.
    ///
    /// # Errors
    /// `IpcErrorCode::Invalid` when the JSON didn't match `T` (unknown field, wrong type, missing
    /// field).
    pub fn into_inner(self) -> Result<T, IpcError> {
        self.0
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Req<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(Self(serde_json::from_value(raw).map_err(|e| {
            let rule = if e.to_string().starts_with("unknown field") {
                "unknownField"
            } else {
                "shape"
            };
            IpcError::invalid("req", rule)
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, IpcErrorCode, IpcErrorDetail, Req};
    use crate::ipc::commands::app::AppBootstrapReq;

    /// IPC-R1: an extra field is `Invalid` (one case per command; extended per command in M2-01).
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

    /// `ipc::commands_json`: `src/lib/ipc/bindings/commands.json` is `COMMANDS`, sorted.
    #[test]
    fn commands_json() {
        let mut sorted: Vec<&str> = COMMANDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, COMMANDS, "COMMANDS must stay sorted");
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
