# ADR-006: PTY via an own wrapper over portable-pty 0.9.0; xterm 6 fed by a raw-bytes Channel, with sanitized input and Rust-routed links

## Status
Proposed

## Date
2026-10-19

## Context

F-009 runs the interactive Claude Code TUI per run, one tab per chain. It needs:

- full-screen rendering, Shift+Tab, a 5 KB paste, resize, mouse;
- keyboard routing;
- VoiceOver;
- scrollback caps;
- archived buffers on re-arm;
- a crash-safe run state.

Rust chooses the program (SR-01, SR-02). Memory must stay ≤ 300 MB with 3 tabs (NFR-23), and a 1 MB burst must not freeze the UI (NFR-08). The revised requirements add:

- **SR-24:** `pty_write` only for a live run; the threat model states that a compromised webview can type into Claude sessions.
- **SR-28:** every paste into a terminal tab is sanitized in Rust. ESC is removed **before** xterm adds bracketed-paste markers, so an embedded `ESC[201~` can't end the paste early. C0 (except TAB, LF, CR), DEL and C1 are stripped with a visible notice.
- **SR-29:** OSC 8 and web-links go to Rust, which opens only `https:` (or `http:` to localhost) after a confirmation; `windowOptions` all false; no `@xterm/addon-clipboard` (no OSC 52); OSC 0/2 titles never become tab names.
- **SR-56:** the clipboard is read only in Rust, on an explicit action, text only, ≤ 1 MB.
- **SR-57:** child lifecycle.

Source checks of `portable-pty` 0.9.0 (22 M downloads, 662 reverse deps; source maintained in wezterm through 2026-09):

- `CommandBuilder::new` with `env`, `env_remove` and `env_clear`.
- `as_command()` calls `env_clear()`, sets `SHELL`, then the builder's env.
- **If the builder's `cwd` is not an existing directory, it silently falls back to `$HOME`.**
- An absolute program skips the PATH search; `new_default_prog()` would start a login shell.
- The Unix spawn does `setsid()`, `TIOCSCTTY` and `close_random_fds()`.
- `ChildKiller::kill` sends **SIGHUP**, polls about 250 ms, then SIGKILL.

`@xterm/xterm` 6.0.0 typings have `screenReaderMode`, `scrollback`, `macOptionIsMeta` and `attachCustomKeyEventHandler`.

## Decision

**Rust side.** `foreman-sys::pty::PtySession` (about 350 lines, no Tauri; `foreman-app` adapts its byte sink to a Channel):

- `openpty(PtySize)`, then `CommandBuilder::new(<absolute allowlisted claude>)` with argv from `build_argv`, `env_clear()`, and only `build_env`'s entries.
- `cwd` is set only after `store::Root` opened it as a directory with no symlink components. Just before spawn, `is_dir` is re-checked and a failure refuses (tab `errored`) rather than allowing the `$HOME` fallback.
- `new_default_prog` is never called (clippy `disallowed-methods`).
- **Output:** a reader thread on `try_clone_reader()` reads ≤ 64 KB chunks into an in-memory ring (the last 2,000 lines, used for `pty_attach` replay and `output.log`) and calls the sink. `foreman-app` sends `tauri::ipc::Response::new(chunk)` on a `Channel<tauri::ipc::Response>`. That's `InvokeResponseBody::Raw`, delivered to JS as an `ArrayBuffer`; a `Channel<Vec<u8>>` would serialize a JSON number array (tauri 2.12.4 `src/ipc/mod.rs`).
- **Input:**
  - `pty_write(runId, bytes ≤ 64 KB)` covers keystrokes only and is refused unless the run is live (SR-24).
  - `pty_paste(runId)` reads the clipboard **in Rust** (SR-56: text only, ≤ 1 MB, on this explicit action, never logged), applies `core::sanitize::for_paste`, and returns the sanitized text plus a "removed N control characters" notice. That function removes every ESC, C0 except TAB/LF/CR, DEL and C1. The webview then calls `term.paste(text)`, so xterm adds the bracketed-paste markers around text that can no longer contain `ESC[201~`. xterm's own paste-event handling is disabled (the custom key handler intercepts ⌘V, and a capture-phase `paste` listener on the terminal element calls `preventDefault`).
  - `pty_resize` calls `MasterPty::resize`; `pty_interrupt` writes `\x03`.
- **Signals:** Terminate sends `SIGTERM` to the child's process group (`rustix::process::kill_process_group`), waits 5 s, then `SIGKILL`, with a confirmation when not idle (F-009, SR-13). On quit after the quit guard, `ChildKiller::kill` (SIGHUP, SR-57).
- `output.log` is flushed from the ring every 5 s and at exit, best effort and not fsync'd (a diagnostic tail, SR-53). `run.json` is the durable state (ADR-010).

**Webview side (`#lib/terminal`):**

- `@xterm/xterm` 6.0.0 with `@xterm/addon-fit` 0.11.0, `@xterm/addon-webgl` 0.19.0 (DOM fallback on context loss), `@xterm/addon-unicode11` 0.9.0, `@xterm/addon-search` 0.16.0, and `@xterm/addon-web-links` 0.12.0 **with a custom handler**. `@xterm/addon-clipboard` is never installed.
- Options: `scrollback: 10000`; `screenReaderMode` follows VoiceOver/Settings; `macOptionIsMeta` per Settings; `windowOptions` all false; a `linkHandler` for OSC 8.
- **Links:** both the OSC 8 handler and the web-links handler call `ipc.terminalLinkOpen(url)`. Rust parses the URL and allows only `https:` (or `http:` to `localhost`/`127.0.0.1`) without userinfo. It shows a native confirmation with the full URL (bidi and zero-width characters made visible, SR-26), then opens through the opener plugin. Everything else is refused (SR-29).
- **Titles:** `onTitleChange` is not subscribed. Tab names come from the run's `--name`.
- **Key routing** in `attachCustomKeyEventHandler`: ⌘-chords and ⌃` go to the app, except ⌘C (copy selection), ⌘V (Rust paste path above), ⌘A and ⌘F. ⌘⇧E returns focus to the workspace (no keyboard trap).
- **Flow control:** `write(data, callback)`. Above 1 MB pending, `pty_flow(runId, paused)` makes the reader thread wait on a condvar, so the kernel PTY buffer back-pressures `claude` (NFR-08).
- **Re-arm:** the old buffer is serialized to text into a collapsed read-only archive section; the xterm instance is disposed.

## Alternatives considered

- **`tauri-plugin-pty` 0.3.1.** The webview picks the program (violates SR-01); it polls ≤ 4 KB per IPC call; single maintainer.
- **`pty-process` 0.5.4** (async). Adoption 16× smaller; API not verified in depth. It's the fallback behind `PtySession`.
- **Raw `openpty` + spawn via rustix.** Needs `pre_exec` (`unsafe`) for `setsid`/`TIOCSCTTY`, and `unsafe` is forbidden.
- **Sanitizing pastes in the webview.** The webview is less trusted (T1), and SR-28 puts sanitizing in Rust.
- **ghostty-web 0.4.0.** Pre-1.0; accessibility undocumented.

## Consequences

- We own the lifecycle code and its tests. Every security decision (program, argv, env, cwd, paste content, link targets) stays in Rust.
- A compromised webview can still send keystrokes to a live session through `pty_write`. That's the residual risk SR-24 names, bounded by ADR-014's CSP and supply-chain controls (ADR-024).
- portable-pty publishes infrequently. If a macOS fix lands only in git, we vendor it (Exit path).

## Verification

- S-0 (MASTER-SPEC §9), plus the SR-28 check that the fixture paste runs nothing in the real TUI.
- Fake-claude integration tests: `pty::spawn_output_input_resize`, `pty::exit_codes`, `pty::terminate_sigterm_then_sigkill`, `pty::refuses_missing_cwd`, `pty::child_env_exact`, `pty::no_inherited_fds`, `pty::burst_1mb_backpressure` (NFR-08), `pty::attach_replays_tail`, `pty::write_requires_live_run`.
- `pty::paste_strips_escape_sequences` (fixture `\x1b[201~\x03!touch /tmp/pwn\r`: the PTY receives one bracketed block with no ESC inside).
- `terminal::link_handler_routes_to_rust`, `opener::terminal_link_scheme_policy`, `terminal::options_snapshot` (no clipboard addon, `windowOptions` false, no title subscription).
- `clipboard::read_only_on_user_action`, `clipboard::size_cap`.

## Revisit when

- S-0 fails on memory or rendering.
- portable-pty's macOS code goes 12 months without wezterm commits.
- xterm 6.1 or 7 changes the renderer, paste or link APIs.

## Exit path

`PtySession` is a trait-backed module (`spawn`, `write`, `paste`, `resize`, `signal`, `wait`, `sink`). Swap in `pty-process` or a vendored portable-pty without touching IPC. On a fundamental S-0 failure, ADR-001's SwiftUI + SwiftTerm fallback applies.
