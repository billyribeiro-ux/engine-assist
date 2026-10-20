//! `app::logging` (design doc §3.4 and §8; ADR-018 §6; SR-54, SR-57; files.md §2 row 25): what
//! Foreman records about itself.
//!
//! - `tracing` events go through one [`Layer`] to `logs/foreman.log` in app data: local, `0600`,
//!   rotated by size (10 MB × 5: `foreman.log` and `foreman.log.1` … `.4`), never uploaded.
//! - The layer writes field **names** and Foreman's own fixed messages. The value of a field named
//!   for content (`prompt`, `text`, `body`, `packet`, `output`, `env_value`, `clipboard`, `value`,
//!   alone or as a `_`-separated part of the name) is replaced with `‹redacted›`. Other values are
//!   cut to [`VALUE_MAX`] characters with control characters made visible. Event messages are
//!   string literals (`logging::messages_are_literals` checks every `tracing::` call in the app).
//! - The panic hook logs the panic's location and thread, never its payload (which could hold
//!   request or repo content), to the log and stderr. After a crash, the next start marks the runs
//!   that were live `interrupted` (the run-log entry, SR-57) and logs how many; the crash report
//!   itself is macOS's own (nothing is uploaded, SR-60).

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use foreman_core::pathpolicy;
use foreman_core::store::{Root, app_dirs, dirs};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Registry;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt as _};

/// The log file, app-data relative.
pub const LOG_FILE: &str = "logs/foreman.log";
/// Rotate when the file would pass this size (SR-54).
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;
/// Rotated files kept beside `foreman.log` (`.1` … `.4`): five files in all.
pub const KEEP: u32 = 4;
/// The longest value written for a field that isn't redacted.
pub const VALUE_MAX: usize = 120;
/// What a content-bearing field's value becomes.
pub const REDACTED: &str = "‹redacted›";
/// Field names whose values are never written (ADR-018 §6).
pub const CONTENT_FIELDS: [&str; 8] = [
    "prompt",
    "text",
    "body",
    "packet",
    "output",
    "env_value",
    "clipboard",
    "value",
];

static CAUGHT: AtomicU32 = AtomicU32::new(0);

struct Sink {
    root: Root,
    len: u64,
    max: u64,
}

static SINK: Mutex<Option<Sink>> = Mutex::new(None);

fn sink() -> std::sync::MutexGuard<'static, Option<Sink>> {
    SINK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Installs the panic hook (location only, SR-57) and the `tracing` subscriber. Events before
/// [`attach`] (and after a failed one) go nowhere. Safe to call more than once.
pub fn install() {
    install_panic_hook();
    install_subscriber();
}

/// The `tracing` subscriber alone: the registry with [`FileLayer`] (a second call is a no-op).
pub fn install_subscriber() {
    let _ = tracing::subscriber::set_global_default(Registry::default().with(FileLayer));
}

/// The panic hook alone: a panic's location and thread go to the log and stderr, never its
/// payload.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let at = info.location().map_or_else(
            || "unknown".to_owned(),
            |l| format!("{}:{}", l.file(), l.line()),
        );
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_owned();
        tracing::error!(target: "panic", at = at.as_str(), thread = thread.as_str(), "panic");
        let _ = std::io::Write::write_all(
            &mut std::io::stderr(),
            format!("foreman: panic at {at}\n").as_bytes(),
        );
    }));
}

/// Starts writing to `logs/foreman.log` under `app_data` (the folder is created `0700`).
///
/// # Errors
/// When app data can't be opened or the log folder or file is refused (a symlink, a folder).
pub fn attach(app_data: &Path) -> Result<(), String> {
    attach_with_limit(app_data, MAX_BYTES)
}

/// [`attach`] with another rotation size (tests use a small one).
///
/// # Errors
/// As [`attach`].
pub fn attach_with_limit(app_data: &Path, max: u64) -> Result<(), String> {
    let root = Root::open_app_data(app_data).map_err(|e| e.to_string())?;
    let logs = pathpolicy::check_app_data("logs").map_err(|e| e.to_string())?;
    dirs::ensure_dir(&root, &logs).map_err(|e| e.to_string())?;
    let len = app_dirs::file_len(&root, LOG_FILE)
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    *sink() = Some(Sink { root, len, max });
    Ok(())
}

/// Stops writing (tests).
pub fn detach() {
    *sink() = None;
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

/// Whether a field's value is content and must not be written.
#[must_use]
pub fn is_content_field(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    CONTENT_FIELDS.iter().any(|w| {
        name == *w
            || name.starts_with(&format!("{w}_"))
            || name.ends_with(&format!("_{w}"))
            || name.contains(&format!("_{w}_"))
    })
}

/// A value as written: control characters escaped, at most [`VALUE_MAX`] characters.
#[must_use]
pub fn clean_value(v: &str) -> String {
    let mut out = String::new();
    for (n, c) in v.chars().enumerate() {
        if n >= VALUE_MAX {
            out.push('…');
            break;
        }
        if c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}') {
            out.push_str(&c.escape_unicode().to_string());
        } else {
            out.push(c);
        }
    }
    out
}

struct Line {
    message: String,
    fields: String,
}

impl Visit for Line {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, &format!("{value:?}"));
    }
}

impl Line {
    fn put(&mut self, field: &Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = clean_value(value);
            return;
        }
        let shown = if is_content_field(name) {
            REDACTED.to_owned()
        } else {
            clean_value(value)
        };
        self.fields.push(' ');
        self.fields.push_str(name);
        self.fields.push('=');
        self.fields.push_str(&shown);
    }
}

/// Formats one event as a log line (without writing it).
#[must_use]
pub fn format_event(event: &Event<'_>) -> String {
    let mut line = Line {
        message: String::new(),
        fields: String::new(),
    };
    event.record(&mut line);
    let meta = event.metadata();
    format!(
        "{} {} {}: {}{}\n",
        jiff::Timestamp::now()
            .round(jiff::Unit::Millisecond)
            .unwrap_or_default(),
        meta.level(),
        meta.target(),
        line.message,
        line.fields
    )
}

fn write_line(text: &str) {
    let mut guard = sink();
    let Some(s) = guard.as_mut() else {
        return;
    };
    let Ok(rel) = pathpolicy::check_app_data(LOG_FILE) else {
        return;
    };
    let bytes = text.as_bytes();
    let n = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if s.len.saturating_add(n) > s.max {
        if app_dirs::rotate(&s.root, LOG_FILE, KEEP).is_err() {
            return;
        }
        s.len = 0;
    }
    if s.root.append_line(&rel, bytes).is_ok() {
        s.len = s.len.saturating_add(n);
    }
}

/// The layer that writes redacted lines to the log file.
#[derive(Debug, Clone, Copy)]
pub struct FileLayer;

impl<S: Subscriber> Layer<S> for FileLayer {
    fn enabled(&self, metadata: &tracing::Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        *metadata.level() <= Level::INFO
    }

    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut line = format_event(event);
        // `append_line` takes at most 4 KB.
        if line.len() > 4000 {
            let mut cut = 3990;
            while !line.is_char_boundary(cut) {
                cut -= 1;
            }
            line.truncate(cut);
            line.push_str(" …\n");
        }
        write_line(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_field_names() {
        for n in [
            "prompt",
            "text",
            "prompt_text",
            "env_value",
            "packet_body",
            "clipboard",
            "value",
            "old_value_hash",
        ] {
            assert!(is_content_field(n), "{n}");
        }
        for n in ["run_id", "phase", "command", "code", "ms", "at", "textual"] {
            assert!(!is_content_field(n), "{n}");
        }
    }

    #[test]
    fn values_are_cut_and_escaped() {
        assert_eq!(clean_value("a\u{1b}[2Jb"), "a\\u{1b}[2Jb");
        assert_eq!(clean_value("x\u{202e}y"), "x\\u{202e}y");
        let long = "z".repeat(500);
        assert_eq!(clean_value(&long).chars().count(), VALUE_MAX + 1);
    }
}
