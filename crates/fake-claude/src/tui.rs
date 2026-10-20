//! Interactive TUI emulation (M3): what Foreman's PTY, paste, re-arm and sensor code meets in a
//! real session, reduced to observable behavior.
//!
//! - Raw mode on the PTY (as the real TUI does), alternate screen and bracketed paste enabled.
//! - A banner `FAKE-CLAUDE READY session=<id> mode=<new|resume|fork>`, then the positional prompt,
//!   if any, is "submitted".
//! - Input: `ESC[200~ … ESC[201~` is one paste block (echoed `PASTE <n> bytes`); Ctrl-C clears
//!   the input box (`^C`); Enter submits: `/exit` ends the session (exit 0, `SessionEnd` hook with
//!   `reason: prompt_input_exit`) unless the script says `ignoreExit`; any other text echoes
//!   `ECHO: <text>` and fires `Stop` with `last_assistant_message`.
//! - `SessionStart` (on start) and `Stop`/`SessionEnd` hooks come from `--settings` exactly as
//!   Claude Code runs exec-form hooks: `command` with `args`, the hook input JSON on stdin.
//! - A transcript is written to `<config>/projects/<cwd as Claude encodes it>/<id>.jsonl` for new
//!   sessions; `--resume` of an unknown session prints "No conversation found" and exits 1.
//! - Everything it saw is appended as JSON lines to `<record>.tui.jsonl`:
//!   `{"type":"paste","text":…}`, `{"type":"ctrlC"}`, `{"type":"submit","text":…}`,
//!   `{"type":"key","bytes":[…]}`, `{"type":"resize","cols":…,"rows":…}`, `{"type":"hook",…}`.
//!
//! Script keys under `"tui"`: `ignoreExit` (bool), `burstBytes` (output on start),
//! `exitCode` (for `/exit`), `banner` (extra text).

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// The session flags the TUI understood.
#[derive(Debug, Default)]
pub struct Flags {
    pub session_id: Option<String>,
    pub resume: Option<String>,
    pub fork: bool,
    pub settings: Option<PathBuf>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub effort: Option<String>,
    pub prompt: Option<String>,
}

pub fn parse_flags(args: &[&str]) -> Flags {
    let mut f = Flags::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i];
        let next = args.get(i + 1).map(|s| (*s).to_owned());
        match a {
            "--session-id" => {
                f.session_id = next;
                i += 1;
            }
            "--resume" => {
                f.resume = next;
                i += 1;
            }
            "--settings" => {
                f.settings = next.map(PathBuf::from);
                i += 1;
            }
            "--model" => {
                f.model = next;
                i += 1;
            }
            "--permission-mode" => {
                f.mode = next;
                i += 1;
            }
            "--effort" => {
                f.effort = next;
                i += 1;
            }
            "--name" => i += 1,
            "--fork-session" => f.fork = true,
            s if !s.starts_with('-') && i + 1 == args.len() => f.prompt = Some(s.to_owned()),
            _ => {}
        }
        i += 1;
    }
    f
}

/// Claude Code's project folder name (see `foreman_core::chain::project_dir_name`).
pub fn project_dir_name(cwd: &str) -> String {
    let units: Vec<u16> = cwd.encode_utf16().collect();
    let name: String = units
        .iter()
        .map(|&u| {
            let c = char::from_u32(u32::from(u)).unwrap_or('-');
            if c.is_ascii_alphanumeric() { c } else { '-' }
        })
        .collect();
    if name.len() <= 200 {
        return name;
    }
    let mut h: i32 = 0;
    for &u in &units {
        h = h.wrapping_shl(5).wrapping_sub(h).wrapping_add(i32::from(u));
    }
    let mut n = h.unsigned_abs();
    let mut d = Vec::new();
    while n > 0 {
        d.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
        n /= 36;
    }
    d.reverse();
    format!("{}-{}", &name[..200], String::from_utf8_lossy(&d))
}

pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude")))
}

pub fn transcript(id: &str) -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    Some(
        config_dir()?
            .join("projects")
            .join(project_dir_name(&cwd.to_string_lossy()))
            .join(format!("{id}.jsonl")),
    )
}

pub struct Log(Option<PathBuf>);

impl Log {
    pub fn new(record: Option<PathBuf>) -> Self {
        Self(record.map(|r| {
            let mut s = r.into_os_string();
            s.push(".tui.jsonl");
            PathBuf::from(s)
        }))
    }

    pub fn put(&self, v: &Value) {
        if let Some(p) = &self.0
            && let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
        {
            let _ = writeln!(f, "{v}");
        }
    }
}

/// Runs the exec-form hooks registered for `event` in the settings file, feeding `input`.
pub fn fire_hooks(settings: Option<&Path>, event: &str, input: &Value, log: &Log) {
    let Some(text) = settings.and_then(|p| std::fs::read_to_string(p).ok()) else {
        return;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if v.get("disableAllHooks").and_then(Value::as_bool) == Some(true) {
        return;
    }
    let groups = v
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for g in groups {
        for h in g
            .get("hooks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(cmd) = h.get("command").and_then(Value::as_str) else {
                continue;
            };
            let args: Vec<String> = h
                .get("args")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let status = Command::new(cmd)
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut c| {
                    if let Some(mut w) = c.stdin.take() {
                        let _ = w.write_all(input.to_string().as_bytes());
                    }
                    c.wait()
                });
            log.put(&json!({ "type": "hook", "event": event, "command": cmd,
                             "ok": status.map(|s| s.success()).unwrap_or(false) }));
        }
    }
}

fn hook_input(f: &Flags, session: &str, event: &str, extra: Value) -> Value {
    let mut v = json!({
        "session_id": session,
        "hook_event_name": event,
        "cwd": std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "permission_mode": f.mode.clone().unwrap_or_else(|| "default".into()),
        "model": f.model.clone().unwrap_or_default(),
        "effort": { "level": f.effort.clone().unwrap_or_else(|| "medium".into()) },
    });
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
    v
}

fn out(s: &str) {
    let mut o = std::io::stdout().lock();
    let _ = o.write_all(s.as_bytes());
    let _ = o.flush();
}

fn winsize() -> Option<(u16, u16)> {
    rustix::termios::tcgetwinsize(rustix::stdio::stdin())
        .ok()
        .map(|w| (w.ws_col, w.ws_row))
}

/// The interactive session. Returns the exit code.
pub fn run(args: &[&str], script: &Value, record: Option<PathBuf>) -> u8 {
    let f = parse_flags(args);
    let log = Log::new(record);
    let tui = script.get("tui").cloned().unwrap_or(Value::Null);
    let session = match (&f.resume, &f.session_id, f.fork) {
        (Some(r), _, false) => {
            if transcript(r).is_none_or(|p| !p.exists()) {
                out(&format!("No conversation found with session ID: {r}\r\n"));
                return 1;
            }
            r.clone()
        }
        (Some(_), Some(n), true) | (None, Some(n), _) => n.clone(),
        (Some(r), None, true) => format!("{r}-fork"),
        (None, None, _) => "00000000-0000-4000-8000-000000000000".into(),
    };
    let mode = if f.fork {
        "fork"
    } else if f.resume.is_some() {
        "resume"
    } else {
        "new"
    };
    if let Some(p) = transcript(&session)
        && mode != "resume"
    {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(
            &p,
            format!("{}\n", json!({ "type": "session", "id": session })),
        );
    }
    // Raw mode, as the real TUI: Ctrl-C arrives as a byte, not SIGINT.
    let stdin = rustix::stdio::stdin();
    if let Ok(mut t) = rustix::termios::tcgetattr(stdin) {
        t.make_raw();
        let _ = rustix::termios::tcsetattr(stdin, rustix::termios::OptionalActions::Now, &t);
    }
    out("\x1b[?1049h\x1b[?2004h");
    out(&format!(
        "FAKE-CLAUDE READY session={session} mode={mode} model={} effort={} permission={}{}\r\n",
        f.model.as_deref().unwrap_or("-"),
        f.effort.as_deref().unwrap_or("-"),
        f.mode.as_deref().unwrap_or("-"),
        tui.get("banner").and_then(Value::as_str).unwrap_or("")
    ));
    let source = match mode {
        "resume" => "resume",
        "fork" => "fork",
        _ => "startup",
    };
    fire_hooks(
        f.settings.as_deref(),
        "SessionStart",
        &hook_input(&f, &session, "SessionStart", json!({ "source": source })),
        &log,
    );
    if let Some(n) = tui.get("burstBytes").and_then(Value::as_u64) {
        let line = "x".repeat(1023) + "\n";
        let mut left = usize::try_from(n).unwrap_or(0);
        let mut o = std::io::stdout().lock();
        while left > 0 {
            let k = left.min(line.len());
            if o.write_all(&line.as_bytes()[..k]).is_err() {
                break;
            }
            left -= k;
        }
        let _ = o.write_all(b"BURST-END\r\n");
        let _ = o.flush();
    }
    let submit = |text: &str| -> Option<u8> {
        log.put(&json!({ "type": "submit", "text": text }));
        if text == "/exit" {
            if tui.get("ignoreExit").and_then(Value::as_bool) == Some(true) {
                out("still here\r\n");
                return None;
            }
            fire_hooks(
                f.settings.as_deref(),
                "SessionEnd",
                &hook_input(
                    &f,
                    &session,
                    "SessionEnd",
                    json!({ "reason": "prompt_input_exit" }),
                ),
                &log,
            );
            out("\x1b[?2004l\x1b[?1049lbye\r\n");
            return Some(
                tui.get("exitCode")
                    .and_then(Value::as_u64)
                    .and_then(|c| u8::try_from(c).ok())
                    .unwrap_or(0),
            );
        }
        if !text.is_empty() {
            let reply = format!("ECHO: {text}");
            out(&format!("{reply}\r\n"));
            fire_hooks(
                f.settings.as_deref(),
                "Stop",
                &hook_input(
                    &f,
                    &session,
                    "Stop",
                    json!({ "last_assistant_message": reply }),
                ),
                &log,
            );
        }
        None
    };
    if let Some(p) = &f.prompt
        && let Some(c) = submit(p)
    {
        return c;
    }
    let mut input = String::new();
    let mut paste: Option<Vec<u8>> = None;
    let mut pending: Vec<u8> = Vec::new();
    let mut size = winsize();
    let mut buf = [0u8; 4096];
    let mut stdin_r = std::io::stdin().lock();
    loop {
        let mut fds = [rustix::event::PollFd::new(
            &stdin,
            rustix::event::PollFlags::IN,
        )];
        let ready = rustix::event::poll(
            &mut fds,
            Some(&rustix::event::Timespec {
                tv_sec: 0,
                tv_nsec: 100_000_000,
            }),
        )
        .unwrap_or(0);
        let now = winsize();
        if now != size {
            size = now;
            if let Some((c, r)) = now {
                log.put(&json!({ "type": "resize", "cols": c, "rows": r }));
                out(&format!("RESIZED {c}x{r}\r\n"));
            }
        }
        if ready == 0 {
            continue;
        }
        let n = match stdin_r.read(&mut buf) {
            Ok(0) | Err(_) => return 0,
            Ok(n) => n,
        };
        pending.extend_from_slice(&buf[..n]);
        loop {
            if let Some(p) = paste.as_mut() {
                if let Some(i) = find(&pending, b"\x1b[201~") {
                    p.extend_from_slice(&pending[..i]);
                    pending.drain(..i + 6);
                    let text = String::from_utf8_lossy(p).into_owned();
                    log.put(&json!({ "type": "paste", "text": text,
                                     "hasEsc": p.contains(&0x1b) }));
                    out(&format!("PASTE {} bytes\r\n", p.len()));
                    input.push_str(&text);
                    paste = None;
                    continue;
                }
                p.append(&mut pending);
                break;
            }
            let Some(&b) = pending.first() else { break };
            if pending.starts_with(b"\x1b[200~") {
                pending.drain(..6);
                paste = Some(Vec::new());
                continue;
            }
            if b == 0x1b {
                if pending.len() < 6 && b"\x1b[200~".starts_with(&pending) {
                    break; // wait for the rest of the marker
                }
                let k = pending.len().min(3);
                log.put(&json!({ "type": "key", "bytes": pending[..k].to_vec() }));
                pending.drain(..k);
                continue;
            }
            pending.remove(0);
            match b {
                0x03 => {
                    log.put(&json!({ "type": "ctrlC" }));
                    input.clear();
                    out("^C\r\n");
                }
                b'\r' | b'\n' => {
                    let text = std::mem::take(&mut input);
                    if let Some(c) = submit(&text) {
                        return c;
                    }
                }
                0x7f => {
                    input.pop();
                }
                _ => {
                    log.put(&json!({ "type": "key", "bytes": [b] }));
                    input.push(char::from(b));
                    out(&String::from(char::from(b)));
                }
            }
        }
    }
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}
