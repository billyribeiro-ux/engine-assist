//! `fake-claude`: a scripted stand-in for the `claude` CLI (ADR-015, M0-05). Never bundled.
//!
//! On every invocation it records argv, cwd and the full environment as JSON to:
//! - `$FAKE_CLAUDE_RECORD` if set, else
//! - `$FAKE_CLAUDE_RECORD_DIR/$FOREMAN_RUN_ID.json` if both are set.
//!
//! Behaviour comes from the JSON script named by `$FAKE_CLAUDE_SCRIPT` (all keys optional):
//! `{ "version": "2.1.293 (Claude Code)", "auth": { ... }, "authExit": 0, "initOnlyExit": 0,
//!    "initOnlyOutput": "", "output": "", "exitCode": 0 }`
//! - `-v` / `--version` prints `version` and exits 0;
//! - `auth status` prints `auth` as JSON and exits `authExit`;
//! - any argv containing `--init-only` prints `initOnlyOutput` and exits `initOnlyExit`;
//! - anything else prints `output` and exits `exitCode`.
//!
//! The interactive TUI emulation (alternate screen, resize, `\x03`, `/exit`, transcripts) arrives
//! with the PTY tickets (M3).
#![forbid(unsafe_code)]
// fake-claude is test-only code (ADR-003 lists it outside the product crates); it writes its record
// file with std::fs on purpose.
#![allow(
    clippy::disallowed_methods,
    reason = "test double; never bundled (ADR-003, ADR-015)"
)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{Map, Value, json};

const DEFAULT_VERSION: &str = "2.1.293 (Claude Code)";

fn record_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("FAKE_CLAUDE_RECORD") {
        return Some(PathBuf::from(p));
    }
    let dir = std::env::var_os("FAKE_CLAUDE_RECORD_DIR")?;
    let run = std::env::var("FOREMAN_RUN_ID").ok()?;
    Some(PathBuf::from(dir).join(format!("{run}.json")))
}

fn record(argv: &[String]) -> Result<(), String> {
    let Some(path) = record_path() else {
        return Ok(());
    };
    let env: Map<String, Value> = std::env::vars_os()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                Value::String(v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let doc = json!({ "argv": argv, "cwd": cwd, "env": env });
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn script() -> Result<Value, String> {
    let Some(path) = std::env::var_os("FAKE_CLAUDE_SCRIPT") else {
        return Ok(Value::Object(Map::new()));
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("script: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("script: {e}"))
}

fn exit_code(v: Option<&Value>) -> u8 {
    v.and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
        .unwrap_or(0)
}

fn text<'a>(s: &'a Value, key: &str, default: &'a str) -> &'a str {
    s.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn emit(out: &str) -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(out.as_bytes())
        .map_err(|e| e.to_string())?;
    if !out.is_empty() && !out.ends_with('\n') {
        stdout.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    stdout.flush().map_err(|e| e.to_string())
}

fn run() -> Result<u8, String> {
    let argv: Vec<String> = std::env::args().collect();
    record(&argv)?;
    let s = script()?;
    let args: Vec<&str> = argv.iter().skip(1).map(String::as_str).collect();

    match args.as_slice() {
        ["-v" | "--version"] => {
            emit(text(&s, "version", DEFAULT_VERSION))?;
            Ok(0)
        }
        ["auth", "status", ..] => {
            let auth = s
                .get("auth")
                .cloned()
                .unwrap_or_else(|| json!({ "loggedIn": true, "authMethod": "claude.ai" }));
            emit(&serde_json::to_string(&auth).map_err(|e| e.to_string())?)?;
            Ok(exit_code(s.get("authExit")))
        }
        a if a.contains(&"--init-only") => {
            emit(text(&s, "initOnlyOutput", ""))?;
            Ok(exit_code(s.get("initOnlyExit")))
        }
        _ => {
            emit(text(&s, "output", ""))?;
            Ok(exit_code(s.get("exitCode")))
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "fake-claude: {e}");
            ExitCode::from(70)
        }
    }
}
