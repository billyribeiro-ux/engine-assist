# Foreman · Security and privacy requirements

Status: DRAFT for Stage 3 sign-off · 2026-10-19
Inputs: `docs/handoff/stage-02.md`, `docs/MASTER-SPEC.md` §6, §7, §11, §12; `spec/council/r4-redteam.md`; `spec/council/r5-resolution.md`; `spec/council/r6-feature-specs.md`; `spec/council/r8-nfr.md`.

This document extends Master Spec §12 into the complete, numbered list Foreman is designed against (Microsoft SDL: requirements before design). Each requirement names **where it's built** (blueprint stage of Foreman's own build, and the feature) and **the test that proves it**. Test names are the names the tests will have; Stage 4.6 tickets reference them.

**Threat posture in one paragraph.** Foreman runs on one trusted Mac for one trusted user, but it reads and acts on content it doesn't control: project repos that Claude Code edits continuously, chat output pasted from claude.ai, the blueprint, and Claude Code's own configuration files in each repo. It spawns the most capable program on the machine (`claude`) with flags that decide what that program may do and whose account it bills. So the security model is: **the user is trusted; repo content, pasted content and Claude Code's behavior are not.** The webview is treated as less trusted than the Rust core.

**Legend.** Stage numbers are Foreman's own build stages (blueprint v2.x): 5 Site prep (scaffold, CI), 6 Foundation (domain core), 7 Framing (walking skeleton), 8 Roofing (security controls), 9 Plumbing (process, file and git integration), 11 HVAC (local logging and diagnostics), 12 Drywall (features), 13 Exterior (design system, accessibility), 14 Inspections (audits).

**Not applicable, with reasons.** 3.1 business model and pricing: not commercial, no payments. 3.2 legal and compliance part: personal tool, no one else's data, no distribution. ASVS chapters for authentication, session management, OAuth/OIDC and web services: Foreman has no accounts, sessions, server or network surface of its own; recorded in the threat model (Stage 4.3).

---

## 1. Process spawning and the Claude Code boundary

| ID | Requirement | Stage · Feature | Proven by |
|---|---|---|---|
| SR-01 | Foreman spawns processes only from an allowlist of resolved absolute paths (`claude`, `git`, and the toolchain probes `node`, `pnpm`, `cargo` used only with `--version`). Paths are auto-detected from fixed candidate locations or set in Settings, must be absolute, and are re-resolved and checked (exists, executable, not a symlink into a writable project folder) before every spawn. | 9 · F-008, F-019 | `spawn::rejects_relative_path`, `spawn::rejects_unlisted_program`, `spawn::rejects_symlink_into_project` |
| SR-02 | Every spawn uses an argv array. No shell is ever used to run a step, a git command, or a verify command. The single exception is the Doctor action **Capture my login shell PATH**: one `$SHELL -l -c 'printf %s "$PATH"'` run, only on Billy's explicit click, its output shown before it's stored. | 9 · F-008, F-019 | `spawn::no_shell_in_any_code_path` (static check: no `sh`/`-c` construction outside `doctor::capture_login_path`), `doctor::login_path_requires_consent` |
| SR-03 | Foreman never passes `--bare`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, or `--permission-mode bypassPermissions`, and the template engine rejects `bypassPermissions` anywhere in the blueprint or overlay. | 6, 9 · F-001, F-008 | `argv::never_contains_forbidden_flags` (property test over every template step and every overlay step), `template::rejects_bypass_permissions` |
| SR-04 | The resolved prompt is passed as the final positional argument and is rejected if it starts with `-`, `/` or `!`. | 9 · F-008 | `argv::rejects_option_like_prompt`, `argv::positional_prompt_snapshot` (every harness step) |
| SR-05 | The child environment is built from an explicit allowlist: `HOME`, `USER`, `LOGNAME`, `SHELL`, `TMPDIR`, `LANG`, `LC_*`, `TERM=xterm-256color`, `COLORTERM=truecolor`, `PATH` (built per Master Spec §6), `FOREMAN_RUN_ID`, plus `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` when present in Foreman's own environment, plus Billy's pass-through list from Settings. A denylist always wins over the pass-through list: every `ANTHROPIC_*`, `CLAUDE_CODE_USE_*`, `AWS_*`, `GOOGLE_*`/`GCLOUD_*`/`CLOUDSDK_*`, and `AZURE_*` variable. | 9 · F-008 | `env::allowlist_only`, `env::denylist_beats_passthrough`, `env::oauth_token_passed_when_present` |
| SR-06 | Before every CLI launch, Foreman fingerprints every Claude Code configuration source that applies to the step: user settings (`~/.claude/settings.json` or `$CLAUDE_CONFIG_DIR/settings.json`), managed settings, every `CLAUDE.md` from the cwd up to the repo root, `CLAUDE.local.md`, and the cwd's `.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json`, `.claude/agents/**`, `.claude/skills/**`, `.claude/commands/**`, `.claude/rules/**`. A change since the last launch in that cwd shows the diff and needs one acknowledgement. | 8 · F-008 | `preflight::config_change_requires_ack` |
| SR-07 | A launch is **blocked** when any fingerprinted configuration source (user settings included) sets `apiKeyHelper`, `awsAuthRefresh`, `awsCredentialExport` or `gcpAuthRefresh`, or an `env` entry named `ANTHROPIC_*` or `CLAUDE_CODE_USE_*`. The block names the file and key. | 8 · F-008 | `preflight::blocks_auth_changing_keys` (one case per key) |
| SR-08 | `claude auth status` is run in the step cwd with the exact launch environment and cached against the full config fingerprint. Only `authMethod` `claude.ai` or `oauth_token` may launch. Foreman reads only `authMethod` and the exit code from that output. | 8 · F-008, F-019 | `preflight::auth_checked_in_step_cwd`, `preflight::blocks_api_key_auth` |
| SR-09 | Every CLI launch passes a Foreman-generated `--settings` file containing `disableAllHooks: false`, `permissions.deny: ["Read(./.foreman/**)"]`, and (P2) the sensor hooks. The file is written fresh per run, its hash recorded, and it lives only in app data. | 9 · F-008, F-011 | `run_settings::contents_snapshot`, `run_settings::fresh_per_run` |
| SR-10 | Foreman never edits Billy's user-level Claude Code settings (`~/.claude/settings.json`, `~/.claude.json`) or any project's `.claude/settings*.json`. | 9 · all | `fs_policy::denies_claude_settings_writes` (path policy test), code review checklist item |
| SR-11 | **(v1.1, with F-101.)** Verify commands are approved objects: `{id, argv, cwd, timeoutSec}` stored in `.foreman/project.json`, approved in Foreman's UI, with the approval hash (including the text of any referenced `package.json` script) stored in app data. The linter rejects shells and interpreters as the program; generic runners (`pnpm exec`, `pnpm dlx`, `npx`, `npm exec`, `yarn dlx`); network tools; git write subcommands; glob or shell metacharacters in arguments. A changed referenced script invalidates the approval. | 8 · F-015, F-022 | `verify::lint_rejects_*` (one per class), `verify::script_change_invalidates_approval` |
| SR-12 | **(v1.1, with F-101.)** Foreman runs approved verify commands itself (argv, no shell, timeout, output captured), never through Claude. A PASS for an `auto` item requires exit code 0 from that command in that run. Risk register: approved verify commands execute repository code. | v1.1 · F-101 | `gate::auto_pass_requires_exit_zero`, `gate::cannot_pass_without_run` (property) |
| SR-13 | Re-arm never sends SIGTERM or SIGKILL to a session that isn't confirmed idle (by Billy in P1, by the sensor in P2) without an explicit confirmation. The ending sequence is Ctrl-C, then `/exit`. | 9 · F-010 | `rearm::ctrl_c_then_exit_order`, `rearm::no_kill_without_idle_or_confirm` |
| SR-14 | `--session-id` is passed only at chain start; continuing steps use `--resume <uuid>`, never both. | 9 · F-010 | `argv::session_id_only_on_chain_start` |
| SR-15 | Sensor events (P2) are accepted only with the per-run token (scope: stops cross-run mix-ups and other users' processes, not a forger running as Billy; no event triggers an action by itself, SR-18); hooks use exec form (`command` + `args`, never a shell string); the sensor binary runs from `<app-data>/bin/foreman-sensor-<hash>`, whose hash is checked at startup; events are one file each, created atomically. | 9 · F-011 | `sensor::rejects_wrong_token`, `sensor::exec_form_only`, `sensor::binary_hash_checked` |
| SR-16 | v1 makes no `claude -p` calls. (v1.1 headless Gate A uses the hardened argv in `r5-resolution.md` §2.1, after spike S-2′.) | 9 · F-013 | `spawn::no_print_mode_in_v1` (static check that `-p`/`--print` never appears in a built argv) |
| SR-17 | When Claude Code's version changes, the zero-token contract probes (`claude --init-only` with Foreman's flags and a marker hook; `--init-only --resume <random uuid>`) run before the next launch, in the fixed `<app-data>/probe/` folder Billy trusted once, with a 20 s timeout. Outcomes: pass, flag failure (disables only the dependent features, named), inconclusive (keeps the last verified state, warns). | 9 · F-019 (P2) | `probe::runs_on_version_change`, `probe::runs_only_in_probe_dir`, `probe::inconclusive_keeps_state`, `probe::failure_disables_feature` |
| SR-18 | No sensor event, file event or Claude output triggers an action by itself. Every action that affects a Claude session (launch, re-arm, Ctrl-C, `/exit`, terminate) starts from Billy's click or keystroke in Foreman. | 9 · F-010, F-011 | `rearm::only_from_user_action`, `sensor::events_only_update_state` |

## 2. The webview and IPC

| ID | Requirement | Stage · Feature | Proven by |
|---|---|---|---|
| SR-20 | Content Security Policy for production: `default-src 'self'; script-src 'self'; style-src 'self'` (plus the hashes Tauri injects at compile time for any inline style or script, never `'unsafe-inline'` or `'unsafe-eval'`); `img-src 'self' data:`; `font-src 'self'`; `connect-src ipc: http://ipc.localhost`; `object-src 'none'`; `base-uri 'none'`; `form-action 'none'`; `frame-ancestors 'none'`. A separate `devCsp` allows the Vite dev server only. `dangerousDisableAssetCspModification` stays off. Exact directives are fixed in the Stage 4.2 ADR after S-0 confirms Bits UI and xterm work under them. | 5, 8 · F-030 | `csp::production_config_snapshot`, a Playwright test that an injected inline `<script>` doesn't run |
| SR-21 | `app.security.freezePrototype: true`; `app.withGlobalTauri: false`; `assetProtocol.enable: false`; no remote URLs in any capability. | 5 · — | `config::security_settings_snapshot` |
| SR-22 | The webview holds **no plugin permissions**. Clipboard, opener, dialog, notification and window-state functions are called only from Rust, behind Foreman's own commands, which validate their arguments. The main window's capability grants only `core:event:allow-listen`, `core:event:allow-unlisten`, `core:window:allow-start-dragging` (for the transparent title bar's drag region), and Foreman's own app commands. | 5, 8 · all | `capabilities::main_window_snapshot` (fails on any added permission) |
| SR-23 | Foreman's app commands are declared with `AppManifest::commands` in `build.rs`, so each command needs an explicit grant; every command is granted to the main window only. No other window or webview exists in v1. | 5 · — | `capabilities::every_command_declared_and_granted` |
| SR-24 | Every IPC command validates its input in Rust (types via serde with `deny_unknown_fields`, lengths, enums, IDs that must exist) and returns typed errors. The webview never passes a path, program, argv, URL or environment variable that Rust uses without validation; where the user picks a path, it comes from a Rust-side dialog. | 7, 8 · all | `ipc::rejects_unknown_fields` (per command), `ipc::fuzz_*` (proptest per command) |
| SR-25 | URLs opened in the default browser are limited to `https://claude.ai/` and its subpaths (Project and chat links), plus `https://code.claude.com/docs/` links in Doctor help. Files revealed in Finder must pass the path policy. Anything else is refused. | 8 · F-012, F-019 | `opener::only_allowed_urls`, `opener::reveal_requires_path_policy` |
| SR-26 | Repo and pasted content (packets, lessons, tickets, overlay text, prompts, chat output, terminal output) is rendered as text only. No `{@html}` in Svelte components; Markdown is rendered by Foreman's own renderer to a fixed element set with escaped text and no raw HTML, links shown as text with an explicit Open action subject to SR-25. | 7, 13 · F-012, F-013, F-030 | ESLint rule banning `{@html}`; `render::escapes_html_in_packet`, `render::no_raw_html_passthrough` |
| SR-27 | Instructions inside repo or pasted content never trigger actions. Foreman acts only on user input in its own UI and on parsed, validated structure (packet fields, annotations); it never executes or follows text. | 6, 12 · all | Threat-model abuse cases (Stage 4.3) each with a test, e.g. `abuse::packet_text_cannot_change_flags` |

## 3. File system

| ID | Requirement | Stage · Feature | Proven by |
|---|---|---|---|
| SR-30 | One path policy guards every write Foreman makes: repo-relative paths only; no `..`, absolute paths, NUL, control or bidi characters; deny by path component at any depth: `.git`, `.claude` (except the hard-coded `council-setup` destinations), `.husky`, `.vscode`, `.idea`, `.github`, `.foreman` (except the decision-file writers); deny file names `.mcp.json`, `CLAUDE.local.md`, `.envrc`, `.gitmodules`, `.gitattributes`, `.npmrc`; `CLAUDE.md` only at the repo root, only through the assembler, with an explicit confirmation; component-wise resolution with symlink refusal through a directory-handle API; final path inside the project root or app data. | 6, 8 · F-021 | `fs_policy::*` golden suite (`..`, absolute, `.git/hooks/pre-commit`, symlinked `docs/handoff`, Unicode lookalikes, REPO FILES fixture) |
| SR-31 | Reads are limited to registered project folders, the blueprint repo, and app data. The watcher never follows symlinks out of a project. | 9 · F-013, F-020 | `fs_policy::read_scope`, `watcher::no_symlink_follow` |
| SR-32 | Every write is atomic and durable: temp file `.<name>.foreman-tmp` in the same directory, `sync_all`, rename, fsync of the directory. Temp patterns are added to the repo's `.git/info/exclude` and ignored by the watcher. | 6 · F-020 | `atomic::crash_injection` (proptest killing at every step), `atomic::temp_excluded_from_git` |
| SR-33 | Foreman never overwrites a file that changed on disk after Foreman last read it; the write is refused and the conflict shown. | 6 · F-020 | `atomic::refuses_newer_file` (property test with interleaved external writes) |
| SR-34 | Action destinations (`write-blocks`, `copy-file`, `mkdir`) are hard-coded per action ID in Rust. Destinations are never taken from template, packet or overlay text. | 6 · F-007 | `actions::destinations_fixed` |
| SR-35 | Paste-back writes (packets, outputs, ADR sets, REPO FILES) list every target path and show a diff for existing files before writing, and each target passes SR-30. | 12 · F-012, F-023 | `pasteback::lists_targets_before_write`, `assembler::rejects_policy_violations` |

## 4. Decision integrity, signing and git

| ID | Requirement | Stage · Feature | Proven by |
|---|---|---|---|
| SR-40 | Decision files (`.foreman/project.json`, `overlay.json`, `blueprint.lock.json`) take effect only as last approved by Billy. App data stores their approved hashes. A change from outside Foreman to a decision field opens Pending outside change (diff, Accept, Reject) and has no effect until accepted. | 8 · F-022 | `decisions::outside_edit_has_no_effect_until_accepted` (one case per file) |
| SR-41 | The blueprint lock is verified at open against app data's template hash, and against the blueprint repo at the locked commit when available. A mismatch opens the project read-only. | 6 · F-002 | `lock::hash_mismatch_read_only`, `lock::restore_from_git` |
| SR-42 | Signing re-reads the packet and refuses if its hash differs from what Billy reviewed; writes APPROVED BY and Foreman's fields atomically; commits only named paths (`git add -- <paths>`, then `git commit --only -m … -- <paths>`; never `-a`, never `add -A`); verifies the commit by reading every path back with `git show` and comparing hashes. Current-branch signing is the default. | 9 · F-014 | `sign::refuses_changed_file`, `sign::pathspec_only` (fixture with unrelated staged changes), `sign::readback_detects_hook_rewrite` |
| SR-43 | "Sign on another branch" uses git plumbing only (`hash-object`, temporary index, `commit-tree`, `update-ref`) and never changes HEAD, the index or the working tree; in this mode nothing is written to the working tree. | 9 · F-014 | `sign::other_branch_leaves_worktree_head_and_index` |
| SR-44 | Foreman never runs a repo's git hooks: every git call it makes, signing included, passes `-c core.hooksPath=/dev/null` (so pre-commit, prepare-commit-msg and post-commit don't run); this is recorded in each stage record. | 9 · F-014 | `git::no_hooks_on_any_command`, `sign::hooks_disabled_recorded` |
| SR-45 | Git is invoked with a fixed environment and config overrides that disable pagers, credential prompts, fsmonitor, editors and external diff tools (`GIT_TERMINAL_PROMPT=0`, `-c core.pager=cat`, `-c diff.external=`, `-c core.fsmonitor=false`, `-c core.editor=true`), with a 60 s timeout, because a repo's `.git/config` is untrusted input. | 9 · F-014 | `git::invocation_snapshot`, `git::timeout` |
| SR-46 | After signing, a change on disk to a signed packet is a tamper event shown in Today with the diff; the committed version is authoritative. | 12 · F-014, F-017 | `sign::tamper_detected` |
| SR-47 | Gates can't be satisfied without evidence: an attest item needs a note (≥ 15 characters, not repeated within the gate) or a not-applicable reason. Sign stays disabled until the stage type's close-out sequence (Master Spec §8) is complete and a lessons entry exists. | 12 · F-014, F-015 | `gate::signable_requires_all` (property) |

## 5. Secrets, logging and data retention

| ID | Requirement | Stage · Feature | Proven by |
|---|---|---|---|
| SR-50 | Foreman never opens, displays or watches `.env*` files, `*.pem`, `*.key`, `id_*`, `*.p12`, `.npmrc`, `.netrc`, or anything under `.ssh/`, `.aws/`, `.config/gcloud/`. | 9 · F-013, F-030 | `secrets::excluded_from_views_and_watcher` |
| SR-51 | Foreman never reads Claude Code's credential store or keychain items. It learns auth state only from `claude auth status`'s `authMethod` and exit code. | 9 · F-008 | Code review checklist; `secrets::no_keychain_api_linked` (dependency check) |
| SR-52 | The run log stores argv, cwd, step, chain, timings, outcomes and prompt hashes. It stores the **prompt hash, not the prompt text**, and never stores the child environment's values (only the variable names). | 11 · F-026 | `runlog::no_env_values`, `runlog::prompt_hash_only` |
| SR-53 | Terminal output tails (`runs/<id>.log`, last 2,000 lines) and sensor events stay in app data with `0600` file permissions inside a `0700` directory; tails are deleted after 90 days; sensor events are capped at 20 per run and deleted when the stage is signed; Settings has **Clear run history**. | 11 · F-009, F-011, F-026 | `retention::tail_pruned_after_90_days`, `retention::events_deleted_on_sign`, `fs::app_data_permissions` |
| SR-54 | Foreman's own diagnostic logs (`tracing`) are local, rotated (10 MB × 5), contain no prompt text, packet content, terminal output or environment values, and are never uploaded. | 11 · — | `logging::redaction` (snapshot of log lines from a full harness run) |

## 6. Network, supply chain and build

| ID | Requirement | Stage · Feature | Proven by |
|---|---|---|---|
| SR-60 | Foreman makes no network requests of its own: no telemetry, no update checks, no crash upload, no remote fonts or assets. The Rust dependency tree contains no HTTP client; the CSP's `connect-src` allows only Tauri IPC. (Opening a URL in the default browser, SR-25, isn't a request by Foreman.) | 5, 14 · — | `cargo deny` bans list (reqwest, hyper client, ureq, etc.); a test run under a blocked network |
| SR-61 | Dependencies: exact versions for direct Rust crates in the core, lockfiles committed, `cargo deny` (advisories, licenses, sources: crates.io only), `cargo audit`, and `pnpm audit` clean at every Stage Gate; new dependencies need a line in the ADR log saying why. | 5 · — | CI jobs `deny`, `audit`, `pnpm-audit` |
| SR-62 | `unsafe` is forbidden in Foreman's crates (`#![forbid(unsafe_code)]`), `unwrap`/`expect` are denied outside tests, and clippy runs with `-D warnings` (pedantic on the core crate). | 5 · — | CI `clippy`, crate attributes |
| SR-63 | CI includes static analysis (CodeQL or Semgrep for Rust and TypeScript), secret scanning, an SBOM per build, and the full test suite; a planted secret, a type error and a known-vulnerable dependency each fail it (Stage 5 gate). | 5 · — | Stage 5 gate items |
| SR-64 | The app is built locally and ad-hoc signed; the build is reproducible from the lockfiles and `rust-toolchain.toml`. | 5, 17 · — | `scripts/build-release` documented and run at the Stage 5 gate |

## 7. Privacy

| ID | Requirement | Data class | Stage · Feature | Proven by |
|---|---|---|---|---|
| PR-01 | All data stays on Billy's Mac. Nothing is sent anywhere by Foreman (SR-60). | All | 5 · — | SR-60 tests |
| PR-02 | **Data classes and where each lives.** (a) Project decisions and signed records: repo, versioned. (b) In-flight state, run records, prompt hashes: app data. (c) Terminal output tails: app data, may incidentally contain secrets printed by a session; treated as sensitive. (d) Sensor events, which may contain the last assistant message: app data, sensitive, short retention. (e) Settings, including the captured login PATH: app data. (f) The claude.ai Project and chat URLs: repo `project.json` (they identify Billy's private chats, not their contents). | — | 4.4 · F-020 | `docs/contracts/files.md` lists each file's class |
| PR-03 | Minimization: Foreman stores no chat content beyond what Billy pastes into a packet or declared output; no transcript copies; prompt text not logged (SR-52). | b, c, d | 11 · F-026 | SR-52 tests |
| PR-04 | Retention: run index kept for metrics; output tails 90 days; sensor events until the stage is signed (max 20 per run); templates cache until unused by any project for 90 days. | b, c, d | 11 · F-020 | SR-53 tests |
| PR-05 | Deletion: **Clear run history** removes run records, tails and events; removing a project from Foreman deletes its app-data folder and leaves the repo untouched. | b, c, d | 12 · F-029 | `privacy::clear_history`, `privacy::remove_project_leaves_repo` |
| PR-06 | Export: every file Foreman owns is plain JSON or Markdown in documented schemas (`docs/contracts/files.md`), readable without Foreman. | All | 4.4 · F-020 | Schema tests |
| PR-07 | Processors: none. Foreman shares data with no third party. Claude Code, launched by Foreman, sends prompts to Anthropic under Billy's own subscription; that's Claude Code's behavior and Billy's existing relationship. | — | — | Stated in the threat model |
| PR-08 | Foreman never reads files outside the read scope (SR-31) and never indexes Billy's home folder; auto-detection checks only the fixed candidate paths. | All | 9 · F-019 | `fs_policy::read_scope` |

## 8. Accessibility (requirement, not security, recorded here for traceability)

| ID | Requirement | Stage · Feature | Proven by |
|---|---|---|---|
| AR-01 | WCAG 2.2 AA in both themes; full keyboard operation; VoiceOver labels; Reduce Motion, Reduce Transparency and Increase Contrast honored; CLI launches add `--ax-screen-reader` when VoiceOver is on. | 13 · F-009, F-030 | axe-core (zero violations), keyboard-only script, manual VoiceOver pass at the Stage 13 gate |

---

## Traceability summary

| Stage | Requirements first proven there |
|---|---|
| 5 Site prep | SR-20 (config), SR-21, SR-22, SR-23, SR-60, SR-61, SR-62, SR-63, SR-64, PR-01 |
| 6 Foundation | SR-03 (template), SR-30, SR-32, SR-33, SR-34, SR-41 |
| 7 Framing | SR-24, SR-26 |
| 8 Roofing | SR-06, SR-07, SR-08, SR-20 (runtime), SR-22 (runtime), SR-25, SR-40 |
| 9 Plumbing | SR-01, SR-02, SR-04, SR-05, SR-09, SR-10, SR-13, SR-14, SR-15, SR-16, SR-17, SR-18, SR-31, SR-42–SR-45, SR-50, SR-51, PR-08 |
| 11 HVAC | SR-52, SR-53, SR-54, PR-03, PR-04 |
| 12 Drywall | SR-27, SR-35, SR-46, SR-47, PR-05 |
| v1.1 | SR-11, SR-12 (with F-101) |
| 13 Exterior | SR-26 (rendering), AR-01 |
| 14 Inspections | Every SR re-verified by the security review and penetration test |
