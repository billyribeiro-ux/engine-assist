//! `app::os` (design doc §3.4; ADR-014): the only module that talks to the OS on the webview's
//! behalf. Native confirmations (SR-24a, IPC-R11), Rust-side file dialogs (SR-24), the clipboard
//! on explicit actions only (SR-56), the clock, and emitting §6 events.
//!
//! Everything goes through the [`Os`] trait so the commands are tested with [`ScriptedOs`], which
//! answers dialogs from a script. `run()` always installs [`TauriOs`]; the webview can't pick an
//! implementation or answer a dialog (`ipc::security_confirmations_are_native`).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use foreman_core::urls::OpenUrl;
use foreman_sys::spawn::SpawnCtx;

use crate::ipc::dto::events::Event;
use crate::ipc::error::ClipboardProblem;

/// The largest clipboard text Foreman reads (SR-56).
pub const CLIPBOARD_MAX: usize = 1024 * 1024;

/// A native confirmation (SR-24a): Foreman-written title and message, an OK label naming the
/// action, and Cancel. The webview never supplies any of this text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    /// The dialog title.
    pub title: String,
    /// The body: what happens, with repo-derived text already shown with visible escapes.
    pub message: String,
    /// The OK button's label ("Accept change", "Remove project", …).
    pub ok: String,
}

impl Confirm {
    /// A confirmation.
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        ok: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            ok: ok.into(),
        }
    }
}

/// What a file dialog is for (its title and filters come from Rust).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// The blueprint repo folder (`blueprint_pick_repo { mode: "choose" }`).
    BlueprintRepo,
    /// A project folder (`setup_pick_folder`).
    ProjectFolder,
    /// A tool binary (`settings_pick_tool`).
    Tool(&'static str),
}

/// The OS seam.
pub trait Os: Send + Sync {
    /// Shows a native confirmation and returns true only for OK (SR-24a).
    fn confirm(&self, c: &Confirm) -> bool;
    /// A Rust-side folder or file dialog (SR-24); `None` when Billy cancels.
    fn pick(&self, what: &Pick) -> Option<PathBuf>;
    /// A Rust-side save dialog for a `.md` export (`WritePurpose::UserExport`).
    fn save(&self, title: &str, file_name: &str) -> Option<PathBuf>;
    /// Reads the clipboard as text, ≤ 1 MB (SR-56). Called only from explicit commands.
    ///
    /// # Errors
    /// Empty, not text, or too large.
    fn clipboard_read(&self) -> Result<String, ClipboardProblem>;
    /// Writes text to the clipboard (already sanitized by the caller, SR-28).
    ///
    /// # Errors
    /// When the OS clipboard refuses.
    fn clipboard_write(&self, text: &str) -> Result<(), String>;
    /// Emits a §6 event to the webview (identifiers and revisions only).
    fn emit(&self, e: &Event);
    /// Opens a checked URL in the default browser through `foreman-sys`'s `/usr/bin/open -u`
    /// (SR-25, SR-29). Never in the webview.
    ///
    /// # Errors
    /// The spawn's failure, as text.
    fn open_url(&self, url: &OpenUrl, ctx: &SpawnCtx) -> Result<(), String>;
    /// Now, UTC.
    fn now(&self) -> jiff::Timestamp;
    /// Today's local date (wait steps, DR-067).
    fn today(&self) -> jiff::civil::Date {
        self.now().to_zoned(jiff::tz::TimeZone::system()).date()
    }
}

/// RFC 3339 UTC with seconds, the form every Foreman file stores.
#[must_use]
pub fn rfc3339(t: jiff::Timestamp) -> String {
    t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// The real OS: `tauri-plugin-dialog` and `tauri-plugin-clipboard-manager`, Rust side only.
pub struct TauriOs<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> std::fmt::Debug for TauriOs<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TauriOs")
    }
}

impl<R: tauri::Runtime> TauriOs<R> {
    /// Wraps the app handle.
    #[must_use]
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> Os for TauriOs<R> {
    fn confirm(&self, c: &Confirm) -> bool {
        use tauri_plugin_dialog::{DialogExt as _, MessageDialogButtons, MessageDialogKind};
        // Commands run on the async runtime's blocking pool (ipc::run), never the main thread.
        self.app
            .dialog()
            .message(c.message.clone())
            .title(c.title.clone())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                c.ok.clone(),
                "Cancel".to_owned(),
            ))
            .blocking_show()
    }

    fn pick(&self, what: &Pick) -> Option<PathBuf> {
        use tauri_plugin_dialog::DialogExt as _;
        let d = self.app.dialog().file();
        let picked = match what {
            Pick::BlueprintRepo => d
                .set_title("Choose your blueprint repo folder")
                .blocking_pick_folder(),
            Pick::ProjectFolder => d
                .set_title("Choose the project folder")
                .blocking_pick_folder(),
            Pick::Tool(name) => d
                .set_title(format!("Choose the {name} program"))
                .blocking_pick_file(),
        };
        picked.and_then(|p| p.into_path().ok())
    }

    fn save(&self, title: &str, file_name: &str) -> Option<PathBuf> {
        use tauri_plugin_dialog::DialogExt as _;
        self.app
            .dialog()
            .file()
            .set_title(title)
            .set_file_name(file_name)
            .add_filter("Markdown", &["md"])
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    }

    fn clipboard_read(&self) -> Result<String, ClipboardProblem> {
        use tauri_plugin_clipboard_manager::ClipboardExt as _;
        let text = self
            .app
            .clipboard()
            .read_text()
            .map_err(|_| ClipboardProblem::NotText)?;
        check_clipboard(text)
    }

    fn clipboard_write(&self, text: &str) -> Result<(), String> {
        use tauri_plugin_clipboard_manager::ClipboardExt as _;
        self.app
            .clipboard()
            .write_text(text)
            .map_err(|e| e.to_string())
    }

    fn emit(&self, e: &Event) {
        use tauri::Emitter as _;
        // An emit failure (no window yet) only means nobody is listening; stores refetch on open.
        let _ = self.app.emit(e.name(), e.payload());
    }

    fn open_url(&self, url: &OpenUrl, ctx: &SpawnCtx) -> Result<(), String> {
        sys_open_url(url, ctx)
    }

    fn now(&self) -> jiff::Timestamp {
        jiff::Timestamp::now()
    }
}

/// `/usr/bin/open -u <url>` through `foreman-sys::spawn`: cwd in app data, a minimal environment,
/// a 10 s timeout (SR-01, SR-25).
///
/// # Errors
/// The spawn failure or a non-zero exit, as text.
pub fn sys_open_url(url: &OpenUrl, ctx: &SpawnCtx) -> Result<(), String> {
    use foreman_sys::spawn::{self, Cwd, RunOpts, SpawnRequest};
    let opts = RunOpts {
        env: vec![
            ("HOME".to_owned(), ctx.home.display().to_string()),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ],
        timeout: std::time::Duration::from_secs(10),
        output_cap: 64 * 1024,
        cwd: Cwd::AppData,
    };
    let out =
        spawn::run(&SpawnRequest::OpenUrl(url.clone()), &opts, ctx).map_err(|e| e.to_string())?;
    if out.code == Some(0) {
        Ok(())
    } else {
        Err(format!("open exited with {:?}", out.code))
    }
}

/// SR-56: empty and over-1-MB clipboards are typed errors.
///
/// # Errors
/// [`ClipboardProblem::Empty`] or [`ClipboardProblem::TooLarge`].
pub fn check_clipboard(text: String) -> Result<String, ClipboardProblem> {
    if text.is_empty() {
        Err(ClipboardProblem::Empty)
    } else if text.len() > CLIPBOARD_MAX {
        Err(ClipboardProblem::TooLarge)
    } else {
        Ok(text)
    }
}

/// The test seam: dialogs answer from a script (default OK), picks come from a queue, the clipboard
/// is a string, the clock is fixed, and emitted events are recorded. Only Rust code can construct
/// it; `run()` never does.
#[derive(Debug)]
pub struct ScriptedOs {
    answers: Mutex<VecDeque<bool>>,
    default_answer: Mutex<bool>,
    asked: Mutex<Vec<Confirm>>,
    picks: Mutex<VecDeque<Option<PathBuf>>>,
    saves: Mutex<VecDeque<Option<PathBuf>>>,
    clipboard: Mutex<Option<String>>,
    events: Mutex<Vec<Event>>,
    opened: Mutex<Vec<String>>,
    open_fails: Mutex<bool>,
    now: Mutex<jiff::Timestamp>,
}

impl Default for ScriptedOs {
    fn default() -> Self {
        Self {
            answers: Mutex::new(VecDeque::new()),
            default_answer: Mutex::new(true),
            asked: Mutex::new(Vec::new()),
            picks: Mutex::new(VecDeque::new()),
            saves: Mutex::new(VecDeque::new()),
            clipboard: Mutex::new(None),
            events: Mutex::new(Vec::new()),
            opened: Mutex::new(Vec::new()),
            open_fails: Mutex::new(false),
            // ipc.md §7.2: the mock's fixed clock.
            now: Mutex::new(jiff::Timestamp::from_second(1_792_400_400).unwrap_or_default()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl ScriptedOs {
    /// The answer for every dialog without a scripted one (`true` = OK).
    pub fn answer_all(&self, ok: bool) {
        *lock(&self.default_answer) = ok;
    }
    /// Queues the next dialog answer.
    pub fn answer_next(&self, ok: bool) {
        lock(&self.answers).push_back(ok);
    }
    /// Every confirmation shown so far.
    #[must_use]
    pub fn asked(&self) -> Vec<Confirm> {
        lock(&self.asked).clone()
    }
    /// Queues the next folder/file dialog result (`None` = Cancel).
    pub fn pick_next(&self, p: Option<PathBuf>) {
        lock(&self.picks).push_back(p);
    }
    /// Queues the next save dialog result.
    pub fn save_next(&self, p: Option<PathBuf>) {
        lock(&self.saves).push_back(p);
    }
    /// Sets the clipboard.
    pub fn set_clipboard(&self, text: Option<&str>) {
        *lock(&self.clipboard) = text.map(str::to_owned);
    }
    /// The clipboard.
    #[must_use]
    pub fn clipboard(&self) -> Option<String> {
        lock(&self.clipboard).clone()
    }
    /// Every event emitted so far.
    #[must_use]
    pub fn events(&self) -> Vec<Event> {
        lock(&self.events).clone()
    }
    /// Moves the fixed clock.
    pub fn set_now(&self, t: jiff::Timestamp) {
        *lock(&self.now) = t;
    }
    /// Every URL opened so far.
    #[must_use]
    pub fn opened(&self) -> Vec<String> {
        lock(&self.opened).clone()
    }
    /// Makes the next opens fail (as `open` exiting non-zero would).
    pub fn fail_opens(&self, fail: bool) {
        *lock(&self.open_fails) = fail;
    }
}

impl Os for ScriptedOs {
    fn confirm(&self, c: &Confirm) -> bool {
        lock(&self.asked).push(c.clone());
        let next = lock(&self.answers).pop_front();
        next.unwrap_or_else(|| *lock(&self.default_answer))
    }

    fn pick(&self, _what: &Pick) -> Option<PathBuf> {
        lock(&self.picks).pop_front().flatten()
    }

    fn save(&self, _title: &str, _file_name: &str) -> Option<PathBuf> {
        lock(&self.saves).pop_front().flatten()
    }

    fn clipboard_read(&self) -> Result<String, ClipboardProblem> {
        check_clipboard(lock(&self.clipboard).clone().unwrap_or_default())
    }

    fn clipboard_write(&self, text: &str) -> Result<(), String> {
        *lock(&self.clipboard) = Some(text.to_owned());
        Ok(())
    }

    fn emit(&self, e: &Event) {
        lock(&self.events).push(e.clone());
    }

    fn open_url(&self, url: &OpenUrl, _ctx: &SpawnCtx) -> Result<(), String> {
        if *lock(&self.open_fails) {
            return Err("open exited with Some(1)".to_owned());
        }
        lock(&self.opened).push(url.as_str().to_owned());
        Ok(())
    }

    fn now(&self) -> jiff::Timestamp {
        *lock(&self.now)
    }

    fn today(&self) -> jiff::civil::Date {
        self.now().to_zoned(jiff::tz::TimeZone::UTC).date()
    }
}
