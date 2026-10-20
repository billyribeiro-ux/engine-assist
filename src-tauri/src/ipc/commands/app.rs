//! App group (`docs/contracts/ipc.md` §4.1).

use serde::{Deserialize, Serialize};

use crate::ipc::types::{DoctorLevel, ProjectId, ReadOnly, Rev, Stage};
use crate::ipc::{IpcError, Req};

/// `AppBootstrapReq {}`: no fields; anything extra is `Invalid` (IPC-R1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AppBootstrapReq {}

/// `ProjectSummary` (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectSummary {
    pub id: ProjectId,
    pub name: String,
    pub display_path: String,
    pub rev: Rev,
    pub read_only: Option<ReadOnly>,
    pub current_stage: Option<Stage>,
    pub pinned: Option<u8>,
}

/// `AppBootstrap` (§4.1).
///
/// M0 skeleton: the contract's `interrupted: Vec<RunSummary>` and `capabilities: Capabilities`
/// fields are not here yet because `docs/contracts/ipc.md` doesn't define either type; they arrive
/// with M2-01 once the contract defines them (see `docs/dev-notes.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AppBootstrap {
    pub first_run: bool,
    pub settings_rev: Rev,
    pub projects: Vec<ProjectSummary>,
    pub doctor: DoctorLevel,
}

/// `app_bootstrap` (P1, mutates nothing). M0 returns the first-run state; reading
/// `settings.json`, `projects.json` and `progress.json` arrives with the store (M2).
///
/// # Errors
/// `Invalid` for a malformed request.
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn app_bootstrap(req: Req<AppBootstrapReq>) -> Result<AppBootstrap, IpcError> {
    let AppBootstrapReq {} = req.into_inner()?;
    Ok(AppBootstrap {
        first_run: true,
        settings_rev: Rev(0),
        projects: Vec::new(),
        doctor: DoctorLevel::Unknown,
    })
}
