# ADR-007: Process spawning: one spawn module, allowlisted absolute programs, fixed flags, an explicit child environment, and a defined child lifecycle

## Status
Proposed

## Date
2026-10-19

## Context

Foreman spawns:

- `claude`: interactive in a PTY; `-v`; `auth status`; `--init-only` probes (P2); `--desktop` (P2);
- `git` (ADR-012);
- `/usr/bin/open` (SR-25a forms);
- the toolchain probes `node`, `pnpm` and `cargo` with `--version`.

Revised `docs/requirements.md`, Stage 7 (M2). The `spawn` module and allowlist exist from M1 already, because `git::run` (SR-44) uses them there (`docs/scope.md` §5):

- **SR-01:** allowlist of re-checked absolute paths.
- **SR-02:** one `spawn` module; argv only; the single consented login-PATH shell, with `$SHELL` in `/etc/shells`.
- **SR-03:** a fixed flag list with validated values (`--name` matches `^fm-[a-z0-9-]{1,80}$`, UUIDs parsed); forbidden flags.
- **SR-04:** prompt position and leading-character check after trimming.
- **SR-05:** `env_clear()` plus the allowlist; the always-removed list adds `DYLD_*`, `NODE_OPTIONS`, `BASH_ENV`, `ENV`, `GIT_*`; PATH entries sanitized.
- **SR-19:** preview, confirmations (including the first launch after a Claude Code version change) and the run record before spawn.
- **SR-28:** control characters rejected in prompts.
- **SR-57:** every child gets `env_clear()` plus the allowlist, its own session (`setsid`), no inherited fds above 2, umask 022; SIGHUP on quit; Interrupted after a crash, never auto-relaunched; panics become typed errors.

Source checks, 2026-10-19:

- std 1.99.1 `os/unix/process.rs`: `CommandExt::process_group` is stable since 1.64; `CommandExt::setsid` is **unstable** (`process_setsid`, issue 105376).
- std's macOS spawn path uses `posix_spawn` without `POSIX_SPAWN_CLOEXEC_DEFAULT`.
- portable-pty 0.9.0's Unix spawn calls `setsid()` and `TIOCSCTTY`, then `close_random_fds()` (fds above 2) in the child.
- `rustix::process::umask` is a safe API.

## Decision

1. **`foreman-sys::spawn`** is the only module that constructs `std::process::Command`, `tokio::process::Command` or `portable_pty::CommandBuilder` (clippy `disallowed-methods`, ADR-003). Its API takes typed requests (`ClaudeInteractive`, `ClaudeVersion`, `ClaudeAuthStatus`, `ClaudeProbe`, `ClaudeDesktop`, `Git(GitArgs)`, `Open(OpenForm)`, `ToolVersion(Tool)`, `LoginShellPath(Consent)`), never strings.
2. **Program allowlist** (SR-01). Each program resolves to an absolute path from fixed candidates or Settings:
   - `claude`: `~/.local/bin/claude`, `/opt/homebrew/bin/claude`;
   - `git`: `/usr/bin/git`, `/opt/homebrew/bin/git`;
   - `open`: `/usr/bin/open` only;
   - probes: Homebrew, `~/.cargo/bin` and pnpm-home candidates.

   Before **every** spawn: re-resolve; it must exist, be a regular executable, have no ancestor inside a registered project, and have a canonical target that isn't inside a registered project or a world-writable directory.
3. **Argv** comes from pure core builders.
   - `build_argv` emits only flags from a fixed enum (`--session-id`, `--resume`, `--fork-session`, `--model`, `--effort`, `--permission-mode`, `--name`, `--settings`, `--ax-screen-reader`, `--init-only`, `--desktop`, `-v`, `auth status`), each value from an enum or a validated character set.
   - Forbidden: `--bare`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, `bypassPermissions`, `-p`/`--print`.
   - The prompt is the final positional argument. After trimming leading whitespace it may not start with `-`, `/` or `!`, and it may not contain C0 (except TAB, LF, CR), DEL or C1 characters (SR-28).
4. **Environment** (`core::launch::build_env`, SR-05): `env_clear()`, then:
   - `HOME`, `USER`, `LOGNAME`, `SHELL`, `TMPDIR`, `LANG`, `LC_*`, `PATH`, `FOREMAN_RUN_ID`, `TERM=xterm-256color`, `COLORTERM=truecolor`;
   - `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` if present in Foreman's environment;
   - Billy's pass-through list.

   The always-removed list (SR-05, a superset of SR-07's blocked `env` patterns) beats the pass-through: `ANTHROPIC_*`, `CLAUDE_CODE_*` (except `CLAUDE_CODE_OAUTH_TOKEN` taken from Foreman's own environment), `AWS_*`, `GOOGLE_*`, `GCLOUD_*`, `CLOUDSDK_*`, `AZURE_*`, `*_PROXY`/`*_proxy`, `NODE_EXTRA_CA_CERTS`, `NODE_TLS_REJECT_UNAUTHORIZED`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `DYLD_*`, `NODE_OPTIONS`, `BASH_ENV`, `ENV`, `GIT_*` (test `env::removed_list_covers_blocked_patterns`). git gets its own fixed environment (ADR-012).
   **Working directory.** A step's `claude` session runs in the step's cwd; `git::run` runs with `-C <root>`. Every other child (`claude -v`, `claude auth status` outside a launch, toolchain probes, the login-shell capture, `open`) runs with its cwd set to app data, so no repo file (for example a `packageManager` field triggering Corepack) can influence it (SR-01, `spawn::helper_cwd_is_app_data`).
5. **PATH** (r5 §2.4 plus SR-05 sanitizing): `/etc/paths`, `/etc/paths.d/*`, `/opt/homebrew/bin`, `/opt/homebrew/sbin`, `~/.cargo/bin`, the pnpm home, then the captured login PATH. Every entry must be absolute, existing, not inside a registered project, and not world-writable. Empty, `.` and relative entries are dropped. S-12 decides whether login-PATH capture is mandatory at first run.
6. **Child lifecycle** (SR-57):
   - Foreman calls `rustix::process::umask(0o022)` once at startup; children inherit it.
   - **PTY children:** portable-pty gives `setsid`, a controlling TTY, and closes fds above 2.
   - **Non-PTY children** (git, `claude -v`, `auth status`, probes, `open`): `tokio::process::Command` with `env_clear()`, `process_group(0)`, `stdin(Stdio::null())`, `kill_on_drop(true)`, piped stdout/stderr, a timeout (git 60 s, `-v`/`auth status` 10 s, probes 20 s) and capped output.
     - **Non-PTY children get `process_group(0)`, not `setsid`** (accepted in the Stage 4.7 review; SR-57 amended to say so). Stable std has no safe `setsid`, and `pre_exec` would need `unsafe`, which is forbidden. `process_group(0)` gives each child its own process group (signals and kill-on-drop are group-scoped). A Finder-launched Foreman has no controlling terminal to detach from, so `setsid`'s remaining effect is moot. If a later need appears, non-PTY children can move to portable-pty's spawner.
     - Inherited fds: Rust std and rustix open every fd with `O_CLOEXEC`. Fds opened by system frameworks are not guaranteed CLOEXEC. **`pty::no_inherited_fds` and `spawn::no_inherited_fds`** (the fake child lists `/dev/fd`) are the tests. If a leak shows up in M2, the fallback is spawning non-PTY children through portable-pty's `CommandBuilder` (which closes fds above 2), reading its merged output.
   - On quit, after the quit guard, children get SIGHUP (portable-pty `ChildKiller::kill` for PTYs; `kill_process_group(SIGHUP)` for the rest).
   - After a crash, `running` records become `interrupted` on the next start and are never relaunched automatically.
   - Command handlers run under `catch_unwind` at the IPC layer and return `IpcError::Internal` without payload values (`ipc::panic_returns_typed_error`).
7. **Preview and confirmation** (SR-19): each `launch_prepare` result carries the resolved prompt, the exact argv and the environment **names**. `needs_confirmation` returns a reason for:
   - an overlay-added step;
   - a mode or model different from the template's (after policy);
   - the first launch in a project;
   - `auto`/`dontAsk` interactively;
   - the first launch after a Claude Code version change ("Claude Code updated X → Y").

   The run record is written before spawn. The confirmation itself is a native dialog raised by Rust when `launch_start` runs (SR-24a); the preview shows the reasons only as context.
8. **The single shell** (SR-02): `doctor::capture_login_path` runs `$SHELL -l -c 'printf %s "$PATH"'` only from an explicit click. `$SHELL` must appear in `/etc/shells`. The output is shown, and stored only on confirmation.

## Alternatives considered

- **`tauri-plugin-shell` 2.5.0.** A webview-reachable spawn surface (SR-22).
- **Inheriting Foreman's environment minus a denylist.** Fails open for any variable Anthropic or a toolchain adds later. An allowlist fails closed.
- **The `which` crate (8.0.6) for resolution.** PATH search is the ambiguity we avoid; fixed candidates plus Settings are auditable.
- **`CommandExt::pre_exec(setsid)`.** Gives real `setsid` but needs `unsafe`. The nightly `setsid` is unstable. Both lose to `process_group(0)` under `forbid(unsafe_code)` and a stable toolchain.

## Consequences

- Tools in unusual places need a Settings entry; Doctor lists every candidate checked.
- One spawn module makes SR-01 to SR-05 and SR-57 reviewable in one file.

## Verification

- `spawn::rejects_relative_path`, `spawn::rejects_unlisted_program`, `spawn::rejects_symlink_into_project`, `spawn::no_shell_in_any_code_path`, clippy `disallowed-methods`, `doctor::login_path_requires_consent`.
- `argv::only_known_flags` (property over every template and overlay step), `argv::never_contains_forbidden_flags`, `argv::rejects_option_like_prompt`, `argv::positional_prompt_snapshot`, `argv::no_print_mode_in_v1`, `resolver::rejects_control_chars_in_prompt`.
- `env::allowlist_only`, `env::denylist_beats_passthrough`, `env::oauth_token_passed_when_present`, `env::path_entries_sanitized`.
- `pty::child_env_exact`, `pty::no_inherited_fds`, `spawn::no_inherited_fds`, `lifecycle::crash_leaves_interrupted`, `ipc::panic_returns_typed_error`.
- `preflight::confirm_required_for_{overlay_step,mode_change,auto_and_dontask,first_launch,version_change}`, `runlog::record_before_spawn`.
- S-12.

## Revisit when

- `CommandExt::setsid` stabilizes: adopt it for non-PTY children.
- Claude Code adds an auth or provider variable outside the removed families (ADR-017 contract notes).
- S-12 fails.

## Exit path

The allowlist and builders are data plus pure functions; adding a program or variable is a one-line change with a test. Replacing tokio's `Command` touches only `spawn.rs`.

## Amended 2026-10-20 (M1-07)

`foreman-sys::spawn` runs non-PTY children with `std::process::Command` (`env_clear`, `process_group(0)`, null or piped stdin, a polling wait that kills the process group on timeout, reader threads that cap each stream), synchronously; tokio isn't needed until the PTY and launch tickets. The world-writable check treats a sticky world-writable folder (`/tmp`, mode 1777) as acceptable, as the OS prevents other users from replacing files there; any non-sticky world-writable ancestor is refused. `spawn::no_inherited_fds` proves Foreman's own (CLOEXEC) descriptors don't reach the child; a descriptor opened without CLOEXEC by a system framework would still be inherited (the documented residual, fallback unchanged).
