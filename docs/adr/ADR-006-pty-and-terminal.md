# ADR-006: PTY via an own wrapper over portable-pty 0.9.0; terminal is xterm 6 fed by a raw-bytes Channel

## Status
Proposed

## Date
2026-10-19

## Context

F-009 runs the interactive Claude Code TUI per run, one tab per chain. It needs:

- full-screen rendering, Shift+Tab, a 5 KB paste, resize, mouse;
- keyboard routing between app shortcuts and the PTY;
- VoiceOver;
- scrollback caps;
- archived buffers on re-arm;
- a crash-safe run state.

Rust must choose the program (SR-01, SR-02). Memory must stay ≤ 300 MB with 3 tabs (NFR-23), and a 1 MB burst must not freeze the UI (NFR-08).

Source checks of `portable-pty` 0.9.0 (crates.io; 22 M downloads, 662 reverse deps; source maintained in wezterm through 2026-09):

- `CommandBuilder::new(prog)`; `env`, `env_remove` and `env_clear` exist (`src/cmdbuilder.rs` 299–324).
- `as_command()` always calls `env_clear()` and then sets `SHELL` from the builder or the passwd entry, then applies the builder's env.
- **If the builder's `cwd` is not an existing directory, `as_command()` silently falls back to `$HOME`** (`.filter(|dir| Path::new(dir).is_dir()).unwrap_or(home)`).
- An absolute program path skips the PATH search; `new_default_prog()` would start a login shell, which we must never use.
- The Unix spawn does `setsid()` and `TIOCSCTTY`, so the child leads its own session and process group.
- `ChildKiller::kill` sends **SIGHUP**, polls for about 250 ms, then SIGKILL (`src/lib.rs` 341–370).

xterm `@xterm/xterm` 6.0.0 typings include `screenReaderMode`, `scrollback`, `macOptionIsMeta` and `attachCustomKeyEventHandler`. `@xterm/addon-canvas` is xterm-5-only (r1).

## Decision

**Rust side (`foreman-app::pty::PtySession`, about 300 lines):**

- `native_pty_system().openpty(PtySize{rows, cols, ..})`, then `CommandBuilder::new(<absolute allowlisted claude path>)` with the argv from `core::launch::build_argv`. Then call `env_clear()` and set **only** the entries from `core::launch::build_env` (`SHELL` is on the allowlist, so portable-pty's own `SHELL` insertion is harmless).
- `cwd` is set only after `store::Root` has opened it as a directory with no symlink components. Just before spawn, Foreman re-checks `is_dir` and refuses (tab `errored`) rather than letting portable-pty fall back to `$HOME`.
- **Output:** a dedicated reader thread on `try_clone_reader()` reads up to 64 KB, appends to an in-memory ring (the last 2,000 lines, for `pty_attach` replay and `output.log`), and sends `tauri::ipc::Response::new(chunk)` on a `Channel<tauri::ipc::Response>`. That's `InvokeResponseBody::Raw`, which arrives as an `ArrayBuffer`; a `Channel<Vec<u8>>` would serialize a JSON number array (tauri 2.12.4 `src/ipc/mod.rs` blanket impl). One Channel per attached tab, replaced on re-attach.
- **Input:** `take_writer()` is held behind a mutex. `pty_write(runId, bytes ≤ 64 KB)` writes; `pty_resize` calls `MasterPty::resize`; `pty_interrupt` writes `\x03`.
- **Signals:** we don't use `ChildKiller::kill` for Terminate, because its SIGHUP-then-SIGKILL-after-250 ms doesn't match F-009's "SIGTERM, then SIGKILL after 5 s, with a confirmation when not idle". Foreman sends `SIGTERM` to the child's process group with `rustix::process::kill_process_group` (safe API, rustix 1.1.5), waits 5 s, then `SIGKILL`. `ChildKiller::kill` is used only on app quit after the quit guard.
- `output.log` is flushed from the ring every 5 s and at exit (best effort, not fsync'd: it's a diagnostic tail, SR-53). The run record (`run.json`) is the durable state (ADR-010).

**Webview side (`#lib/terminal`):**

- `@xterm/xterm` 6.0.0 with `@xterm/addon-fit` 0.11.0, `@xterm/addon-webgl` 0.19.0 (DOM fallback on context loss), `@xterm/addon-unicode11` 0.9.0, `@xterm/addon-search` 0.16.0 and `@xterm/addon-web-links` 0.12.0. Link activation goes through `IpcClient` (opener allowlist, SR-25), never `window.open`.
- `scrollback: 10000`; `screenReaderMode` follows VoiceOver/Settings; `macOptionIsMeta` per Settings.
- **Key routing** in `attachCustomKeyEventHandler`: ⌘-chords and ⌃` return `false` (the app handles them), except ⌘C (copy selection), ⌘V (paste via the PTY), ⌘A and ⌘F, which the terminal keeps. ⌘⇧E returns focus to the workspace, so there's no keyboard trap.
- Writes use xterm's `write(data, callback)` for flow control. If more than 1 MB is pending, the UI asks Rust to pause reads (`pty_flow(runId, paused)`), so the reader thread blocks on a condvar and the kernel PTY buffer applies back-pressure to `claude`.
- **Re-arm:** the old buffer is serialized to text, kept as a collapsed read-only archive section, and the xterm instance is disposed (memory).

## Alternatives considered

- **`tauri-plugin-pty` 0.3.1.** Its JS `spawn(file, args, {cwd, env})` lets the webview pick the program (violates SR-01), and its `read` command polls ≤ 4 KB per IPC call. Single maintainer; the README says "Developing!".
- **`pty-process` 0.5.4** (async, tokio). Credible and active, but its adoption is 16× smaller (40 reverse deps), and its API wasn't verified in depth. It's the fallback crate behind the same `PtySession` interface.
- **Raw `openpty` + `posix_spawn` through rustix.** No third-party PTY crate, but it needs `pre_exec`/`setsid` handling that is `unsafe` in std (`CommandExt::pre_exec`), and `unsafe` is forbidden in Foreman's crates.
- **ghostty-web 0.4.0** for rendering. Pre-1.0 WASM, with accessibility undocumented.

## Consequences

- We own about 300 lines of lifecycle code and its tests. In exchange, every security decision (program, argv, env, cwd) stays in Foreman's Rust.
- portable-pty's last release is 2025-02. Its source is maintained in wezterm, but publishing is infrequent; if a macOS fix lands only in git, we'd vendor it (Exit path).
- The terminal is a separate lazy-loaded chunk, so cold start isn't charged for xterm.

## Verification

- S-0 (pass/fail branches in MASTER-SPEC §9).
- Fake-claude integration tests: `pty::spawn_output_input_resize`, `pty::exit_codes`, `pty::terminate_sigterm_then_sigkill`, `pty::refuses_missing_cwd` (no `$HOME` fallback), `pty::env_is_exactly_built_env`, `pty::burst_1mb_backpressure` (NFR-08), `pty::attach_replays_tail`.
- `spawn::no_shell_in_any_code_path` (no `new_default_prog`).

## Revisit when

- S-0 fails on memory or rendering.
- portable-pty goes unmaintained for macOS (no wezterm commits to `pty/` for 12 months).
- xterm 6.1 or 7 changes the renderer or addon APIs.

## Exit path

- `PtySession` is a trait-backed module (`spawn`, `write`, `resize`, `signal`, `wait`, `reader`); swap in `pty-process` or a vendored `portable-pty` without touching IPC.
- On S-0's fundamental failure, ADR-001's SwiftUI + SwiftTerm fallback applies.
