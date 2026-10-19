# ADR-007: Process spawning: allowlisted absolute programs, argv only, explicit child environment

## Status
Proposed

## Date
2026-10-19

## Context

Foreman spawns `claude` (interactive in a PTY; `-v`, `auth status`, `--init-only` probes in P2, `--desktop` in P2), `git`, and toolchain probes (`node`, `pnpm`, `cargo` with `--version`). It must:

- never use a shell, except the one consented login-PATH capture (SR-01, SR-02);
- never pass forbidden flags (SR-03);
- protect the positional prompt (SR-04);
- build an explicit environment where provider and API-key variables never pass (SR-05; MASTER-SPEC §6).

A Finder-launched app gets a minimal PATH (r1 §A2; S-12). v1 makes no `claude -p` calls (SR-16).

## Decision

1. **Program allowlist** (`foreman-app::spawn::Program`): `Claude`, `Git`, `Probe(Node | Pnpm | Cargo)`, and `LoginShell` (Doctor only). Each resolves to an absolute path from fixed candidates or Settings:
   - claude: `~/.local/bin/claude` (native installer, a symlink into `~/.local/share/claude/versions/`), `/opt/homebrew/bin/claude`;
   - git: `/usr/bin/git`, `/opt/homebrew/bin/git`.

   Before **every** spawn the path is re-resolved: it must exist and be executable, and its canonical target must not lie inside any registered project root (blocking a repo-planted binary). Relative or missing paths are refused with the reason (F-029).
2. **One constructor:** `spawn.rs` and `pty.rs` are the only places that build a `Command` or `CommandBuilder` (static test). Non-interactive runs use `tokio::process::Command` (tokio 1.54.0) with:
   - `env_clear()` and then the built environment;
   - `current_dir` from a policy-checked root;
   - `stdin(Stdio::null())`;
   - `kill_on_drop(true)`;
   - a timeout (git 60 s, `claude -v`/`auth status` 10 s, probes 20 s);
   - output capture capped at 1 MB.
3. **Argv** comes from pure functions in the core (`core::launch::build_argv`, `core::git::args`), never strings concatenated in the app. The forbidden set (`--bare`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, `bypassPermissions` as any value, `-p`, `--print`) is checked as a property over every harness and overlay step. The prompt is the final positional argument, and `check_prompt` rejects a leading `-`, `/` or `!`.
4. **Environment** (`core::launch::build_env`, SR-05):
   - Allowlist: `HOME`, `USER`, `LOGNAME`, `SHELL`, `TMPDIR`, `LANG`, `LC_*`, `TERM=xterm-256color`, `COLORTERM=truecolor`, the built `PATH`, `FOREMAN_RUN_ID`, plus `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` when present, plus Billy's pass-through list.
   - Denylist, which **always wins** over the pass-through: `ANTHROPIC_*`, `CLAUDE_CODE_USE_*`, `AWS_*`, `GOOGLE_*`, `GCLOUD_*`, `CLOUDSDK_*`, `AZURE_*`.
   - git gets its own fixed environment (ADR-012).
5. **PATH** (r5 §2.4), in this order: `/etc/paths`, `/etc/paths.d/*`, `/opt/homebrew/bin`, `/opt/homebrew/sbin`, `~/.cargo/bin`, the pnpm home, then the captured login PATH if present. Shown in Doctor. S-12 decides whether login-PATH capture becomes mandatory at first run.
6. **The single shell:** `doctor::capture_login_path` runs `$SHELL -l -c 'printf %s "$PATH"'` only from an explicit click, shows the output, and stores it only on confirmation. `$SHELL` must be listed in `/etc/shells`.
7. Every spawn is logged in the run log or Doctor log with its argv (prompt replaced by its hash) and env **names** only (SR-52).

## Alternatives considered

- **`tauri-plugin-shell` 2.5.0.** It exposes spawning to the webview behind a capability scope, adding a webview-reachable spawn surface (SR-22 says the webview holds no plugin permissions).
- **Inheriting Foreman's environment and stripping a denylist.** Easier, but any new provider variable Anthropic adds would leak through. An allowlist fails closed.
- **The `which` crate (8.0.6) for PATH resolution.** Searching PATH is exactly the ambiguity we avoid: fixed candidates plus Settings are explicit and auditable. `which` adds nothing.

## Consequences

- Tools installed somewhere unusual need a Settings entry. Doctor names the candidates it checked.
- Pass-through variables are Billy's explicit choice and are shown in the argv preview's environment panel (names only).

## Verification

- `spawn::rejects_relative_path`, `spawn::rejects_unlisted_program`, `spawn::rejects_symlink_into_project`, `spawn::no_shell_in_any_code_path`, `spawn::no_print_mode_in_v1`.
- `argv::never_contains_forbidden_flags` (property), `argv::rejects_option_like_prompt`, `argv::positional_prompt_snapshot`.
- `env::allowlist_only`, `env::denylist_beats_passthrough`, `env::oauth_token_passed_when_present`.
- The fake-claude test records argv/env/cwd for every harness step (F-008 done criterion). S-12.

## Revisit when

- Claude Code adds an auth or provider environment variable outside the denylist families (Doctor's contract notes; ADR-017).
- S-12 fails.

## Exit path

The allowlist and builders are data plus pure functions. Adding a program or variable is a one-line change with a test. Replacing tokio's `Command` with std or another runtime touches only `spawn.rs`.
