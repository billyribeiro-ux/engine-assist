# Foreman · Tickets

Stage 4.6 · Status: DRAFT for the Stage 4 sign · 2026-10-19
Inputs: `docs/scope.md` (milestones M0–M5, AC IDs), `docs/design-doc.md`, `docs/adr/` (ADR-001…ADR-024), `docs/threat-model.md` (T-IDs, tests), `docs/requirements.md` (SR/PR IDs, tests), `docs/contracts/ipc.md`, `docs/contracts/files.md`, `docs/domain-rules.md` (DR-IDs, tests), `docs/design/components.md`, `docs/design/mockups/`, `docs/MASTER-SPEC.md` §9.

Each ticket is sized for one Claude Code session (1–4 hours of Billy's time). Format: blueprint v2.2 B-4.

**Conventions**
- **Stage** is Foreman's own build stage: M0 = 5; M1 = 6; M2 = 7; M3 = 8 (security controls) and 9 (runners); M4 = 11 (logging, retention) and 12 (features); M5 = 13–17.
- **Class:** HARD = touches parsing, the rule engine, file writes, process spawning, git, the PTY, IPC validation or a security control. MECHANICAL = pure config or boilerplate. SPEC-COMPLETE = fully specified, none of the HARD areas. NEEDS-DESIGN = something a builder would have to invent (the ticket says what).
- **Likely files** follow design doc §3.2 and §3.3: `crates/foreman-core`, `crates/foreman-sys`, `crates/foreman-sensor`, `crates/fake-claude`; `foreman-app` is the Tauri crate in `src-tauri/`; the SvelteKit 3 SPA is `src/routes` and `src/lib` (imported as `#lib/...`).
- A test named in **Required tests** is created by the first ticket that lists it; a later ticket that lists the same name extends it with new cases.
- Spike tickets are run by Billy on his Mac; each writes `docs/spikes/<id>.md`. Every later ticket that a spike branch changes names that spike in **Depends on**.
- The order below is the build order: every ticket's dependencies appear above it. One exception: M3-18 was moved into the M4 section in the Stage 4.7 fix pass and keeps its ID (it says "moved from M3").
- A ticket that ends with **`Manual: yes`** is run by Billy on his Mac (spikes, real runs, the kill check): its proof is a report or a recorded run, and no automated test is expected from it.

---

## M0 · Site prep (Stage 5)

### [M0-01] Workspace and app scaffold
Class: MECHANICAL
Stage: 5
Depends on: none
ADRs: ADR-001, ADR-002, ADR-003, ADR-019
Threats mitigated: none
Likely files: Cargo.toml, rust-toolchain.toml, crates/foreman-core/Cargo.toml, crates/foreman-core/src/lib.rs, crates/foreman-sys/Cargo.toml, crates/foreman-sys/src/lib.rs, crates/foreman-sensor/Cargo.toml, crates/foreman-sensor/src/main.rs, crates/fake-claude/Cargo.toml, src-tauri/Cargo.toml, src-tauri/src/main.rs, src-tauri/tauri.conf.json, package.json, pnpm-workspace.yaml, vite.config.ts, tsconfig.json, src/app.html, src/routes/+layout.ts, src/routes/+page.svelte
Acceptance criteria:
- AC-M0-01
- AC-M0-02 (crates and frontend part: Tauri 2.12.4 app crate, `foreman-core` whose `cargo tree` has no `tauri*` crate, SvelteKit 3.1.0 SPA with `adapter-static` fallback `index.html`, `ssr = false`, file-based routes, `#lib` subpath import in `package.json`)
- Given a fresh clone, when `cargo tree -p foreman-core` runs, then no `tauri`, `tokio` or `portable-pty` crate appears (checked by a script used later by CI)
Required tests:
- none named (the `cargo tree` check is a script in `scripts/check-core-deps.sh`)
Out of scope: IPC layer (M0-04), lint and verify (M0-02, M0-07).

### [M0-02] Lint, format and type-check baseline
Class: MECHANICAL
Stage: 5
Depends on: M0-01
ADRs: ADR-003, ADR-019
Threats mitigated: T-105
Likely files: Cargo.toml ([workspace.lints]), clippy.toml, rustfmt.toml, crates/*/src/lib.rs (crate attributes), eslint.config.js, .prettierrc, tsconfig.json, package.json (TypeScript 6.0.3 pin)
Acceptance criteria:
- AC-M0-07
- Given the workspace, then clippy runs with `-D warnings` and `clippy::pedantic` on `foreman-core`; TypeScript is 6.0.3 strict with `noUncheckedIndexedAccess` and `exactOptionalPropertyTypes`; svelte-check, ESLint and Prettier pass on the scaffold
Required tests:
- none named (crate attributes and lint config, proven by M0-07's planted violations)
Out of scope: security-specific lint rules (M0-03).

### [M0-03] Security lint rules with planted-violation fixtures
Class: HARD
Stage: 5
Depends on: M0-02
ADRs: ADR-003, ADR-004, ADR-007, ADR-011
Threats mitigated: T-007, T-015
Likely files: clippy.toml (disallowed-methods), eslint.config.js, scripts/lint-plants/, crates/foreman-core/src/store/mod.rs (allow attribute home), crates/foreman-sys/src/spawn.rs (allow attribute home), crates/foreman-sys/src/git.rs
Acceptance criteria:
- AC-XC-01 (rule part: clippy `disallowed-methods` rejects `std::process::Command::new` and `portable_pty::CommandBuilder::new` outside `foreman-sys::spawn`)
- AC-XC-30 (rule part: `std::fs` rejected outside `foreman-core::store`)
- AC-F030-04 (lint part: `svelte/no-at-html-tags`, `no-restricted-properties` on `innerHTML`, `outerHTML`, `insertAdjacentHTML`)
- Given a planted violation of each rule (Command::new in foreman-app, std::fs in foreman-core::rules, `{@html}` in a component, `innerHTML` in a store, an `@tauri-apps/api` import outside `#lib/ipc`, a `+page.server.ts`), when lint runs, then each fails; the plants live in a fixture folder excluded from the normal build and are exercised by a script
Required tests:
- none named (`scripts/lint-plants/run.sh` exercises every plant; `git::single_entry_point` uses this rule from M1-08)
Out of scope: the spawn and store modules themselves (M1-04, M1-07).

### [M0-04] Typed IPC skeleton and mock mode
Class: HARD
Stage: 5
Depends on: M0-01
ADRs: ADR-004, ADR-002
Threats mitigated: T-005
Likely files: src-tauri/src/ipc/mod.rs, src-tauri/src/ipc/error.rs, src-tauri/src/ipc/commands/app.rs, src-tauri/build.rs, src/lib/ipc/contract.ts, src/lib/ipc/tauri.ts, src/lib/ipc/mock.ts, src/lib/ipc/index.ts, src/lib/ipc/bindings/, src/lib/ipc/fixtures/, package.json (`dev:mock`)
Acceptance criteria:
- AC-M0-02 (typed IPC layer with its mock implementation)
- AC-M0-12
- Given the `app_bootstrap` command, then it takes one request struct with `deny_unknown_fields`, its TS type is generated by ts-rs into `src/lib/ipc/bindings/`, `commands.json` lists it, and `MockIpcClient implements IpcClient` compiles
Required tests:
- ipc::rejects_unknown_fields (first command; extended per command in M2-01)
Out of scope: every other command (M2), the fuzz suite (M2-01).

### [M0-05] Test infrastructure and the fake `claude`
Class: SPEC-COMPLETE
Stage: 5
Depends on: M0-01, M0-04
ADRs: ADR-015, ADR-017
Threats mitigated: T-010
Likely files: crates/fake-claude/src/main.rs, crates/fake-claude/scripts/, vitest.config.ts, playwright.config.ts, wdio.conf.ts, src-tauri/Cargo.toml (`e2e` feature with tauri-plugin-wdio-webdriver 1.5.0), crates/foreman-core/benches/, tests/e2e/smoke.e2e.ts
Acceptance criteria:
- Given the fake `claude`, when spawned, then it records argv, environment and cwd to a file named by an env var, plays scripted output and exit codes, and answers `-v` and `auth status` from its script
- Given Vitest browser mode, Playwright (WebKit) + axe and the WDIO `e2e` feature, then each has one passing example test; criterion, proptest and insta are wired in `foreman-core`
- Given a release build, then the `e2e` feature and the webdriver plugin are absent
Required tests:
- release::no_webdriver
Out of scope: feature tests (each feature ticket adds its own).

### [M0-06] Supply-chain policy for Rust and JS
Class: HARD
Stage: 5
Depends on: M0-01
ADRs: ADR-016, ADR-024
Threats mitigated: T-001, T-069, T-101, T-103
Likely files: deny.toml, Cargo.lock, crates/foreman-core/Cargo.toml (exact pins), pnpm-workspace.yaml (allowBuilds default deny, strictDepBuilds, minimumReleaseAge, trustPolicy), .npmrc, package.json, pnpm-lock.yaml, scripts/check-lockfile-sources.mjs
Acceptance criteria:
- AC-M0-06
- AC-M0-17 (package and lockfile part)
- AC-XC-12
- Given a planted `reqwest` dependency, a `keyring` dependency, and a git-sourced npm package, each in turn, when `cargo deny` or the lockfile scan runs, then each fails
- Given `minimumReleaseAge: 10080` is strict once set, then every pinned JS version is at least 7 days old on M0 day (checked against its publish date before the scaffold is committed); a too-new pin is stepped back and logged in ADR-024
Required tests:
- none named (CI jobs `deny`, `audit`, `pnpm-audit`, `pnpm-policy`; plants run in M0-07)
Out of scope: GitHub Actions pinning (M0-08).

### [M0-07] `pnpm verify`
Class: MECHANICAL
Stage: 5
Depends on: M0-02, M0-03, M0-05, M0-06
ADRs: ADR-015, ADR-024
Threats mitigated: T-104
Likely files: package.json (`verify` script), scripts/verify.sh, .gitleaks.toml, scripts/license-check.sh, scripts/plants/
Acceptance criteria:
- AC-M0-03
- AC-M0-04 (`pnpm verify` part: a planted type error, fake secret, clippy warning and known-vulnerable dependency each fail it, then are reverted)
- AC-M0-17 (plant part: a planted dependency with a `postinstall` script, not in `allowBuilds`, fails `pnpm install` with `strictDepBuilds: true`, then is reverted)
Required tests:
- none named (the plant script `scripts/plants/run.sh` is the proof)
Out of scope: CI-only jobs (Semgrep, SBOM) in M0-08.

### [M0-08] CI workflows
Class: MECHANICAL
Stage: 5
Depends on: M0-07
ADRs: ADR-015, ADR-024
Threats mitigated: T-104
Likely files: .github/workflows/ci.yml, .github/workflows/security.yml, scripts/check-actions-pinned.sh, .semgrep.yml
Acceptance criteria:
- AC-M0-05
- AC-M0-04 (CI part)
- AC-M0-17 (CI part: every install `--frozen-lockfile`, Actions pinned by SHA, `permissions: contents: read`, jobs `pnpm-policy` and `actions-pinned`)
- Given CI on a macOS runner, then jobs `verify`, `deny`, `audit`, `pnpm-audit`, `pnpm-policy`, `actions-pinned`, `semgrep`, `gitleaks`, `sbom` (CycloneDX for cargo and pnpm), `bindings-drift` and `test` run on every push (SR-61, SR-62, SR-63)
Required tests:
- none named (CI jobs listed above)
Out of scope: e2e on CI (local only, ADR-015).

### [M0-09] Tauri security config and capability snapshots
Class: HARD
Stage: 5
Depends on: M0-04
ADRs: ADR-014, ADR-001
Threats mitigated: T-009, T-010, T-011, T-013
Likely files: src-tauri/tauri.conf.json, src-tauri/capabilities/main-window.json, src-tauri/build.rs (AppManifest::commands), src-tauri/Cargo.toml (devtools feature off in release), src-tauri/tests/config_snapshot.rs, src-tauri/tests/snapshots/
Acceptance criteria:
- AC-M0-08
- AC-M0-09
Required tests:
- csp::production_config_snapshot, config::security_settings_snapshot
- capabilities::main_window_snapshot, capabilities::every_command_declared_and_granted
Out of scope: the runtime CSP check on the built app (M3-08); the Rust-built window (M2-03).

### [M0-10] Release build and rollback runbook
Class: MECHANICAL
Stage: 5
Depends on: M0-07, M0-09
ADRs: ADR-001, ADR-015
Threats mitigated: T-106, T-010
Likely files: scripts/build-release, package.json (`release`), docs/runbooks/release.md
Acceptance criteria:
- AC-M0-11
- Given two release builds from the same commit, then the `Foreman.app` binaries hash the same (or the documented difference is listed in the runbook)
Required tests:
- release::no_webdriver (re-run against the release binary)
Out of scope: notarization (non-goal).

### [M0-11] No-network enforcement
Class: HARD
Stage: 5
Depends on: M0-06, M0-10
ADRs: ADR-016
Threats mitigated: T-101, T-102
Likely files: scripts/sandbox/deny-network.sb, scripts/no-network-run.sh, tests/flows/no-external-requests.spec.ts, deny.toml
Acceptance criteria:
- AC-M0-10 (profile and runner built now against `cargo test` and the scaffold app; the full harness run and first-run path are re-run under it in M3-29 and M5-08)
- Given macOS 27, when `sandbox-exec` with the deny-network profile is unavailable or ineffective, then the `pf` anchor fallback in ADR-016 is used and the choice is recorded in the runbook
Required tests:
- network::blocked_network_run
Out of scope: none.

### [M0-12] Spike S-0: PTY, xterm 6, the real TUI and the strict CSP
Class: HARD
Stage: 5
Depends on: M0-05, M0-09
ADRs: ADR-006, ADR-014, ADR-002, ADR-020
Threats mitigated: T-012, T-013, T-091
Likely files: spikes/s0/ (throwaway Tauri route + portable-pty wrapper), docs/spikes/S-0.md, docs/adr/ADR-006-pty-and-terminal.md, docs/adr/ADR-014-csp-and-tauri-capabilities.md, docs/scope.md
Acceptance criteria:
- AC-M0-13 (S-0)
- AC-M0-14
- Given the S-0 checks (Shift+Tab, 5 KB paste, resize while streaming, ⌘K routing, ⌘C/⌘V, VoiceOver, memory with 3 tabs after 30 min, strict CSP with Bits UI, Svelte transitions, xterm and the SvelteKit bootstrap script, SvelteKit 3 `version.json` polling with `pollInterval: 0`), when all pass, then xterm 6 + portable-pty ship as specified and the report says so
- Given any check fails, then the fail branch applies: scrollback cap and archive, a key-routing layer, `--ax-screen-reader`, the memory NFR raised to measured + 20%, CSP `style-src 'unsafe-inline'` only (scripts stay strict) recorded in ADR-014 as an accepted risk; AC-F009-06, AC-F009-08, AC-M0-08 and NFR-23 are updated in `docs/scope.md`; a fundamental TUI failure stops M1 and reopens the shell choice (SwiftUI + SwiftTerm)
- Given the bracketed-paste fixture `\x1b[201~\x03!touch /tmp/pwn\r` pasted into the real TUI through a sanitizing paste path, then nothing runs
- Given `version.json` is fetched, then `connect-src` gains `'self'` and ADR-014 records it
Required tests:
- pty::paste_strips_escape_sequences (real-TUI manual check, recorded in the report)
- csp::production_config_snapshot (updated to the branch taken)
Out of scope: production terminal code (M3-09 to M3-13).
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M0-13] Spike S-1′: per-launch hooks as a contract probe
Class: HARD
Stage: 5
Depends on: M0-01
ADRs: ADR-009, ADR-021
Threats mitigated: T-024, T-026
Likely files: spikes/s1/probe-settings.json, spikes/s1/marker-hook.sh, docs/spikes/S-1prime.md, docs/adr/ADR-009-session-sensor-per-launch-hooks.md, docs/scope.md
Acceptance criteria:
- AC-M0-13 (S-1′)
- Given `claude --init-only --settings <file with exec-form hooks>` in a trusted probe folder, when the hooks fire for `SessionStart` and the listed events, then the pass branch applies: the sensor ships in P2 (M4-07 to M4-10 stay)
- Given the hooks don't fire, then the fail branch applies: no sensor, the manual idle confirmation stays; AC-F011-* and AC-F010-14's sensor part are removed from M4 in `docs/scope.md`, and M4-07 to M4-10 are cancelled except the P1 fallbacks
- Given the run settings deny rules `Read(//<repo>/.foreman/**)`, when a session tries `.FOREMAN/`, then the report says whether the case variant is denied; if not, the variants are listed explicitly in the run settings (M2-09)
- Given a probe repo whose absolute path contains a space (e.g. `~/Developer/Trade Log`), then the report says whether the `//` deny rules still parse and deny; if not, the escaping rule is recorded in ADR-021 and encoded in M2-09
Required tests:
- run_settings::deny_case_variant_probe (manual, recorded in the report)
Out of scope: the sensor binary (M4-07).
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M0-14] Spike S-3: effort on resume
Class: HARD
Stage: 5
Depends on: M0-01
ADRs: ADR-008
Threats mitigated: none
Likely files: docs/spikes/S-3.md, docs/adr/ADR-008-session-chains-and-rearm.md, docs/scope.md
Acceptance criteria:
- AC-M0-13 (S-3)
- Given `claude --resume <uuid> --effort <level>`, when the resumed session reports that effort, then the pass branch applies: `--effort` is passed on resume
- Given it doesn't, then the fail branch applies: `effortLevel` through the run `--settings` file; if that also fails (or the level is `max`/`ultracode`, which `effortLevel` rejects), Foreman shows the `/effort <level>` line with the warning that it saves as Billy's default; AC-F010-10 records the branch
Required tests:
- none named (manual; the branch is encoded by chain::effort_on_resume_branches in M3-14)
Out of scope: none.
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M0-15] Spike S-7′: settings `env` and `apiKeyHelper` in a trusted repo
Class: HARD
Stage: 5
Depends on: M0-01
ADRs: ADR-021
Threats mitigated: T-020
Likely files: spikes/s7/fixture-repo/, docs/spikes/S-7prime.md, docs/adr/ADR-021-launch-preflight-config-fingerprint.md, docs/scope.md
Acceptance criteria:
- AC-M0-13 (S-7′)
- Given a trusted repo whose `.claude/settings.json` sets `env` and `apiKeyHelper`, when the fingerprint and block rules (SR-06, SR-07) are applied by hand to it, then the pass branch applies if they catch every billing-changing input: rules as specified
- Given an input they miss, then the fail branch applies: any project `env` block also blocks the launch until Billy approves it (AC-F008-07 is kept and built in M3-03)
- Given Billy's real `~/.claude` and `~/.claude.json`, then the report records their sizes (against the 256 KB and 64 MB caps) and every symlink among the fingerprint sources (link and target), so ADR-021's caps and symlink rule are checked against his machine
Required tests:
- none named (manual; encoded by preflight::blocks_<key> in M3-03)
Out of scope: none.
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M0-16] Spike S-8: graceful end, signals and forks
Class: HARD
Stage: 5
Depends on: M0-01
ADRs: ADR-008
Threats mitigated: T-029, T-030
Likely files: docs/spikes/S-8.md, docs/adr/ADR-008-session-chains-and-rearm.md, docs/scope.md
Acceptance criteria:
- AC-M0-13 (S-8)
- Given a session with text in the input box and with background work, when Ctrl-C then `/exit`, SIGTERM and SIGHUP are tried, and `--resume <uuid> --fork-session --session-id <new>` is tried, then the report records exit behavior, re-arm latency and whether the forked transcript uses Foreman's ID
- Given all work as specified, then the pass branch applies: re-arm as specified; fork IDs chosen by Foreman
- Given they don't, then the fail branch applies: Billy ends the session himself, then Foreman relaunches (AC-F010-11); fork IDs are the only new transcript after launch, and if ambiguous a fresh chain with context (AC-F010-15); AC-F010-04 records the branch
Required tests:
- none named (manual; encoded by rearm::ctrl_c_then_exit_order and chain::fork_argv)
Out of scope: none.
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M0-17] Spike S-12: PATH in a Finder-launched build
Class: HARD
Stage: 5
Depends on: M0-10
ADRs: ADR-007
Threats mitigated: T-019
Likely files: spikes/s12/, docs/spikes/S-12.md, docs/adr/ADR-007-process-spawning-and-environment.md, docs/scope.md
Acceptance criteria:
- AC-M0-13 (S-12)
- Given the release build launched from Finder, when the PATH is built per Master Spec §6 (`/etc/paths`, `/etc/paths.d/*`, fixed candidates), then the pass branch applies if `claude`, `git`, `node`, `pnpm` and `cargo` resolve: PATH as specified
- Given any of them doesn't, then the fail branch applies: login-PATH capture is mandatory at first run, and AC-F019-13 moves into M2 (M4-14's capture work moves before M2-16)
Required tests:
- none named (manual; AC-F019-14 is re-checked in M4-14)
Out of scope: none.
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M0-18] Spike S-23: `--desktop --resume`
Class: HARD
Stage: 5
Depends on: M0-01
ADRs: ADR-008
Threats mitigated: T-030
Likely files: docs/spikes/S-23.md, docs/adr/ADR-008-session-chains-and-rearm.md, docs/scope.md
Acceptance criteria:
- AC-M0-13 (S-23)
- Given `claude --desktop --resume <uuid>`, when Desktop continues the same transcript file, then the shared branch applies: the chain is handed off and the next Foreman step forks (AC-F010-15)
- Given Desktop copies it into its own session, then the copied branch applies: no fork is needed and the CLI chain continues unaffected; AC-F024-03 records the branch
Required tests:
- none named (manual; encoded in M4-29)
Out of scope: none.
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M0-19] By-hand baseline and Stage 5 close
Class: MECHANICAL
Stage: 5
Depends on: M0-08, M0-10, M0-11, M0-12, M0-13, M0-14, M0-15, M0-16, M0-17, M0-18
ADRs: ADR-001, ADR-024
Threats mitigated: none
Likely files: docs/metrics-baseline.md, docs/spikes/README.md, docs/adr/README.md, docs/handoff/stage-05.md
Acceptance criteria:
- AC-M0-15
- AC-M0-16
- AC-M0-13 (all seven reports present, each naming its branch and the documents it changed)
- AC-KC-04 (baseline sources named): the CLI baseline is every Stage 5 step Billy runs by hand; the **chat baseline is at least 10 chat steps Billy runs by hand, in any project, between M0 and the kill check**, logged in `docs/metrics-baseline.md` with the same end points as Foreman (filled prompt on the clipboard and the target chat open; AC-KC-01)
Required tests:
- none named
Out of scope: none.
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

---

## M1 · Domain core (Stage 6)

### [M1-01] Annotate blueprint v2.2: Stages 1–2, Stage Gate, F.2, Council setup
Class: SPEC-COMPLETE
Stage: 6
Depends on: M0-19
ADRs: ADR-005
Threats mitigated: none
Likely files: claude-blueprint repo `claude-build-blueprint.md` (v2.2), crates/foreman-core/tests/fixtures/blueprint-v2.2.md
Acceptance criteria:
- Given DR-038's reference annotations, when they are written into the blueprint (block-level `fm:` comments per DR-006/DR-007, `fm:grammar v=1`, `fm:verify` per INSPECTION, `fm:if` parts per DR-032), then the PDF render is unchanged except for the prompt split DR-032 describes, and a pinned copy is stored as the core fixture
- Given domain-rules §15 gaps 5, 14 and 15, then the annotation follows the choice each rule states (`in=X` on 1.2, a concrete `BUDGET_TIME` example value in the harness fixture, no `fm:if commercial` around 1.4)
- Exit (formerly M1's entry criterion): blueprint v2.2 with the B-1 annotations for Stages 1–2, the Stage Gate, F.2 and Council setup is committed in the `claude-blueprint` repo, parses with zero errors, and its commit is the one the TradeLog and Foreman fixtures pin
Required tests:
- template::golden_v2_2 (pinned in M1-16)
Out of scope: Stages 0 and 3–5 (M4-04).

### [M1-02] Core types, canonical JSON and schema versioning
Class: HARD
Stage: 6
Depends on: M0-19
ADRs: ADR-023, ADR-003
Threats mitigated: T-076
Likely files: crates/foreman-core/src/ids.rs, crates/foreman-core/src/schema.rs, crates/foreman-core/src/json.rs, crates/foreman-core/src/diagnostic.rs, crates/foreman-core/tests/schema.rs
Acceptance criteria:
- AC-F020-06
- AC-F020-07
Required tests:
- schema::all_versions_migrate, schema::newer_version_read_only, schema::read_does_not_rewrite
- schema::older_build_header_read_banner (a newer file: the tolerant header read gives the read-only banner and identity, ADR-023)
Out of scope: individual file schemas (M1-27, M1-29).

### [M1-03] Path policy: lexical write and read checks
Class: HARD
Stage: 6
Depends on: M1-02
ADRs: ADR-011
Threats mitigated: T-051, T-052, T-053, T-077
Likely files: crates/foreman-core/src/pathpolicy.rs, crates/foreman-core/tests/fs_policy.rs, crates/foreman-core/tests/fixtures/fs_policy_golden.json
Acceptance criteria:
- AC-F021-01
- AC-F021-02
- AC-F021-03
- AC-F021-05
- AC-XC-10
Required tests:
- fs_policy::* (golden suite), fs_policy::allowlist_only, fs_policy::rejects_overlong_path
- fs_policy::case_and_normalization_variants (lexical cases), fs_policy::denies_claude_settings_writes
Out of scope: the fd walk and reported-name check (M1-04).

### [M1-04] Store: no-symlink walk and read policy
Class: HARD
Stage: 6
Depends on: M1-03
ADRs: ADR-011, ADR-010
Threats mitigated: T-054, T-055, T-058, T-071
Likely files: crates/foreman-core/src/store/mod.rs, crates/foreman-core/src/store/walk.rs, crates/foreman-core/tests/store_read.rs
Acceptance criteria:
- AC-F021-04
- AC-F021-07
- AC-F021-08
- AC-F013-16
- AC-XC-14
Required tests:
- fs_policy::read_refuses_symlink_at_any_component, fs_policy::read_refuses_hardlink, fs_policy::read_size_cap
- fs_policy::reported_name_checked, fs_policy::case_and_normalization_variants (case-sensitive and case-insensitive volumes)
- fs::app_data_permissions
Out of scope: writes (M1-05).

### [M1-05] Atomic durable writes that never clobber a newer file
Class: HARD
Stage: 6
Depends on: M1-04
ADRs: ADR-010
Threats mitigated: T-054, T-059, T-060
Likely files: crates/foreman-core/src/store/atomic.rs, crates/foreman-core/tests/atomic.rs
Acceptance criteria:
- AC-F020-01
- AC-F020-02
- AC-F020-03
- AC-F020-04
Required tests:
- atomic::crash_injection, atomic::refuses_planted_tmp_symlink, atomic::refuses_newer_file
- atomic::noreplace_conflict, atomic::swap_unsupported_fallback, atomic::dir_sync_fallback
- atomic::leftover_temp_differs_moved_to_recovered (a stale `.foreman-tmp` that differs from its target goes to `<app-data>/recovered/` and is shown, never deleted; ADR-010)
Out of scope: the `info/exclude` writer (M1-09).

### [M1-06] Reads outside projects and secret-file exclusion
Class: HARD
Stage: 6
Depends on: M1-04
ADRs: ADR-011
Threats mitigated: T-057, T-068
Likely files: crates/foreman-core/src/store/outside.rs, crates/foreman-core/src/pathpolicy.rs (secret patterns), crates/foreman-core/tests/read_scope.rs, clippy.toml (`std::fs` allowed only in the modules AC-XC-30 lists)
Acceptance criteria:
- AC-XC-30
- AC-XC-11
Required tests:
- fs_policy::read_scope, secrets::excluded_from_views_and_watcher (including case variants, AB-08), secrets::symlink_to_secret_refused
Out of scope: the watcher's use of the patterns (M3-19).

### [M1-07] Spawn module core
Class: HARD
Stage: 6
Depends on: M1-02
ADRs: ADR-007
Threats mitigated: T-014, T-034, T-038
Likely files: crates/foreman-sys/src/spawn.rs, crates/foreman-sys/tests/spawn.rs
Acceptance criteria:
- AC-XC-02
- Given any non-session spawn (`claude -v`, toolchain probes, `auth status` outside a launch), then its cwd is app data
Required tests:
- spawn::rejects_relative_path, spawn::rejects_unlisted_program, spawn::rejects_symlink_into_project
- spawn::helper_cwd_is_app_data, spawn::no_inherited_fds
Out of scope: argv and environment builders (M2-06, M2-07).

### [M1-08] `git::run`: the single git entry point
Class: HARD
Stage: 6
Depends on: M1-07, M0-03
ADRs: ADR-012
Threats mitigated: T-039, T-040, T-041, T-042, T-043
Likely files: crates/foreman-core/src/git.rs (closed `GitOp` enum, arg builders), crates/foreman-sys/src/git.rs, crates/foreman-sys/tests/git.rs, crates/foreman-sys/tests/fixtures/hostile-repo/, the closed `GitOp` enum equal to DR-140's list (no `Status`, `Diff`, `Add`, `Commit`, `LsFiles`; ADR-012 decision 1)
Acceptance criteria:
- AC-XC-04
- Given a folder with no repo of its own inside an enclosing repo, when any git op runs, then `git::run` refuses (rev-parse `--show-toplevel` must equal the project root)
- Given the `GitOp` enum, then it is exactly DR-140's list, every `CommitTree` carries `--no-gpg-sign`, and `ReadTree` is refused without Foreman's temporary `GIT_INDEX_FILE`
Required tests:
- git::single_entry_point, git::invocation_snapshot, git::env_cleared, git::timeout
- git::no_hooks_on_any_command, git::filter_driver_never_runs, git::status_and_diff_never_run_filters
- git::repo_gpg_program_never_runs, git::refuses_enclosing_repo
Out of scope: commits (M1-10).

### [M1-09] `info/exclude` writer
Class: HARD
Stage: 6
Depends on: M1-05, M1-08
ADRs: ADR-010, ADR-012
Threats mitigated: T-061, T-062
Likely files: crates/foreman-core/src/store/git_exclude.rs, crates/foreman-core/tests/git_exclude.rs
Acceptance criteria:
- AC-F020-13
Required tests:
- atomic::exclude_writer_scope, atomic::temp_excluded_from_git
Out of scope: none.

### [M1-10] Plumbing commit with read-back
Class: HARD
Stage: 6
Depends on: M1-08, M1-05
ADRs: ADR-012
Threats mitigated: T-080, T-082
Likely files: crates/foreman-sys/src/git/commit.rs, crates/foreman-sys/tests/commit.rs, crates/foreman-sys/tests/fixtures/staged-changes-repo/
Acceptance criteria:
- Given a repo with unrelated staged and unstaged changes, when `commit_paths(paths, message)` runs on the current branch, then it uses only the SR-42 plumbing sequence, the commit holds exactly those paths, the unrelated changes stay, and every path is read back from the commit and compared by hash
- Given the ref moved between `commit-tree` and `update-ref`, then `update-ref <new> <old>` fails and nothing is overwritten
- Given an unborn HEAD (a fresh `git init`), then the commit uses `read-tree --empty`, `commit-tree` without `-p` and `update-ref HEAD <new> 0000…`, and becomes the repo's first commit (DR-143)
- Given a detached HEAD (`symbolic-ref -q HEAD` exits 1), then the commit is refused with "Check out a branch first" and nothing is written
- Given the paths to commit, then only those whose `hash-object --no-filters` differs from `rev-parse HEAD:<path>` are committed, and every commit is `--no-gpg-sign` (DR-142, DR-144)
Required tests:
- sign::pathspec_only, sign::readback_detects_mismatch, sign::untracked_new_packet, sign::concurrent_ref_move_fails
- sign::unborn_head_first_commit, sign::detached_head_refused
Out of scope: signing UX and other-branch mode (M4-20, M4-21).

### [M1-11] Template parser: structure, items and bounds
Class: HARD
Stage: 6
Depends on: M1-02
ADRs: ADR-005
Threats mitigated: T-066, T-105
Likely files: crates/foreman-core/src/template/mod.rs, crates/foreman-core/src/template/structure.rs, crates/foreman-core/src/template/items.rs, crates/foreman-core/tests/template_structure.rs
Acceptance criteria:
- AC-F001-04
- AC-F001-06
- AC-F001-08
Required tests:
- template::normalized_hash_crlf_bom_tabs, template::stage_headings, template::step_headings, template::run_with_attached_to_step
- template::step_keys, template::inspection_items, template::handoff_note_is_display_only, template::missing_annotation_not_runnable
- template::item_text_hash_golden, template::deep_nesting_bounded, template::never_panics
Out of scope: `fm:` directives (M1-12).

### [M1-12] `fm:` grammar: directives and values
Class: HARD
Stage: 6
Depends on: M1-11
ADRs: ADR-005
Threats mitigated: T-016, T-066
Likely files: crates/foreman-core/src/template/grammar.rs, crates/foreman-core/src/template/expr.rs, crates/foreman-core/tests/grammar.rs
Acceptance criteria:
- AC-F001-10
- AC-F001-12
- AC-F001-13 (a newer grammar: a step with an unknown key is `needsNewerForeman`, other unknowns warn, Template health says "Update Foreman"; DR-021, DR-035)
- AC-F005-06
Required tests:
- grammar::block_level_only, grammar::version_required, grammar::key_table, grammar::start_vocabulary, grammar::alt_rules_closed_set
- grammar::input_binding, grammar::paths_repo_relative, grammar::nn_instantiation, grammar::expression_parser, grammar::duplicate_id_error
- template::rejects_bypass_permissions
Out of scope: cross-step validation (M1-14).

### [M1-13] Step binding, prompts, gate and F.2 instances, packet formats
Class: HARD
Stage: 6
Depends on: M1-12
ADRs: ADR-005
Threats mitigated: none
Likely files: crates/foreman-core/src/template/binding.rs, crates/foreman-core/src/template/packet_format.rs, crates/foreman-core/tests/template_binding.rs
Acceptance criteria:
- Given the v2.2 fixture, when parsed, then each `fm:step` binds per DR-006/DR-007, prompts bind per DR-008 (5.1's second fence binds to 5.1b), gate instances exist for Stages 5–17 only, F.2 instances for Stages 0–4 only, and packet formats are extracted from inside prompt fences (DR-017)
Required tests:
- template::fm_step_must_follow_run_with, template::standalone_steps_in_source_order, template::prompt_binding, template::prompt_parts_joined
- template::gate_instances_stages_5_to_17, template::f2_instances_stages_0_to_4, template::packet_formats_stages_0_to_4_and_gate
Out of scope: none.

### [M1-14] Grammar validation and the diagnostic catalog
Class: HARD
Stage: 6
Depends on: M1-13, M1-03
ADRs: ADR-005, ADR-011
Threats mitigated: T-051, T-066
Likely files: crates/foreman-core/src/template/validate.rs, crates/foreman-core/src/template/diagnostics.rs, crates/foreman-core/tests/fixtures/validation/, crates/foreman-core/tests/grammar_validation.rs
Acceptance criteria:
- AC-F001-09
- AC-F001-11
- AC-F001-14
- AC-F001-15
Required tests:
- grammar::chain_validation, grammar::continuing_inherits_cwd_surface, grammar::verify_count, grammar::auto_treated_as_attest
- grammar::fm_if_blocks, grammar::optional_parts, grammar::item_ids_unique, grammar::unknown_key_by_version
- template::errors_scoped_to_stage
Out of scope: action destinations (M1-25).

### [M1-15] Prose lint and chat chips
Class: HARD
Stage: 6
Depends on: M1-13
ADRs: ADR-005
Threats mitigated: none
Likely files: crates/foreman-core/src/template/lint.rs, crates/foreman-core/tests/lint.rs
Acceptance criteria:
- AC-F001-03
Required tests:
- template::prose_change_lints_only, template::inference_rules, template::followup_start_lint, template::chat_chips
Out of scope: none.

### [M1-16] Golden v2.2, round-trip property and template cache
Class: HARD
Stage: 6
Depends on: M1-01, M1-14, M1-15, M1-05
ADRs: ADR-005, ADR-023
Threats mitigated: none
Likely files: crates/foreman-core/tests/golden_v2_2.rs, crates/foreman-core/tests/snapshots/, crates/foreman-core/src/template/render.rs, crates/foreman-core/src/template/cache.rs
Acceptance criteria:
- AC-F001-01
- AC-F001-05
- EX-M1-02
- Given a cached template (`templates/<sourceSha256>.p<parserVersion>.json`) with a matching source hash, then it is loaded instead of parsed
Required tests:
- template::golden_v2_2
Out of scope: Stages 0 and 3–5 (M4-04).

### [M1-17] Blueprint lock: pin, verify, restore, item drift
Class: HARD
Stage: 6
Depends on: M1-16, M1-08
ADRs: ADR-012, ADR-022
Threats mitigated: T-064, T-065, T-067
Likely files: crates/foreman-core/src/lock.rs, crates/foreman-sys/src/git/show.rs, crates/foreman-core/tests/lock.rs, crates/foreman-sys/tests/lock_restore.rs
Acceptance criteria:
- AC-F002-01
- AC-F002-02
- AC-F002-03
- AC-F002-04
- AC-F002-05
- AC-F002-06
- AC-F002-07
- AC-F001-16
Required tests:
- lock::pin_contents, lock::repin_window, lock::restore_from_git, lock::hash_mismatch_read_only, lock::dirty_blueprint_refused
- template::swapped_bullets_reconfirm
Out of scope: re-confirm kind UI (M4-17).

### [M1-18] Rule engine: tags, tri-state conditions, applicable path
Class: HARD
Stage: 6
Depends on: M1-14
ADRs: none
Threats mitigated: none
Likely files: crates/foreman-core/src/rules.rs, crates/foreman-core/tests/rules.rs, crates/foreman-core/tests/fixtures/conditions/
Acceptance criteria:
- AC-F004-01
- AC-F004-02
- AC-F004-05
- AC-F004-06
- AC-F004-08
- AC-F004-09
- AC-F004-10
Required tests:
- rules::tag_condition_table, rules::core_never_dropped, rules::kleene_truth_table, rules::condition_edit_windows
- rules::packet_producer, rules::stage_not_applicable, rules::skipped_queue, rules::skip_rules, rules::gate_items_unskippable
Out of scope: overlay additions (M4-30).

### [M1-19] Run configs and model policy
Class: HARD
Stage: 6
Depends on: M1-14
ADRs: none
Threats mitigated: none
Likely files: crates/foreman-core/src/policy.rs, crates/foreman-core/tests/policy.rs
Acceptance criteria:
- AC-F005-01
- AC-F005-02
Required tests:
- policy::run_config_from_annotation, policy::alternative_selection, policy::substitution_rules
- policy::override_scope, policy::precedence, policy::switches_never_typed
Out of scope: the spec-line UI (M2-21).

### [M1-20] Claude Code version floor
Class: HARD
Stage: 6
Depends on: M1-16, M1-19
ADRs: ADR-017
Threats mitigated: T-036
Likely files: crates/foreman-core/src/compat.rs, crates/foreman-core/tests/compat.rs
Acceptance criteria:
- AC-F008-02
Required tests:
- compat::parses_version_output
Out of scope: checking the floor at launch (M3-05).

### [M1-21] Token classification and the blueprint inventory
Class: HARD
Stage: 6
Depends on: M1-16
ADRs: none
Threats mitigated: none
Likely files: crates/foreman-core/src/resolver/tokens.rs, crates/foreman-core/tests/resolver_tokens.rs
Acceptance criteria:
- AC-F006-01
- AC-F006-02
- AC-F006-03
Required tests:
- resolver::unknown_token_is_error, resolver::token_syntax, resolver::blueprint_token_inventory
- resolver::placeholder_is_unresolved, resolver::stage_tokens_padding
Out of scope: Stages 3–5 tokens (M4-05).

### [M1-22] Prompt resolution
Class: HARD
Stage: 6
Depends on: M1-21, M1-18
ADRs: none
Threats mitigated: T-016, T-087
Likely files: crates/foreman-core/src/resolver/mod.rs, crates/foreman-core/tests/resolver.rs
Acceptance criteria:
- AC-F006-04
- AC-F006-05
- AC-F006-06
- AC-F006-07
- AC-F006-08
- AC-F006-09
- AC-F006-10
- AC-F006-11
Required tests:
- resolver::packet_markers, resolver::inspection_list_filtered, resolver::app_spec_alias, resolver::remap_not_applicable
- resolver::inputs_required_only_if_present, resolver::optional_part_toggle, resolver::fm_if_removal, resolver::cwd_relative_render
- resolver::deterministic, abuse::packet_text_cannot_change_flags
Out of scope: the run-input UI (M2-22).

### [M1-23] Sanitize: prompt hygiene, paste stripping, visible escapes
Class: HARD
Stage: 6
Depends on: M1-02
ADRs: ADR-006
Threats mitigated: T-017, T-088, T-092
Likely files: crates/foreman-core/src/sanitize.rs, crates/foreman-core/tests/sanitize.rs
Acceptance criteria:
- AC-F006-17
- Given `for_paste`, then ESC, C0 (other than TAB, LF, CR), DEL and C1 are stripped with a report; given `visible_escapes`, then bidi controls, zero-width, tag characters and every Default_Ignorable_Code_Point are returned as spans
Required tests:
- resolver::rejects_control_chars_in_prompt, argv::rejects_option_like_prompt (check_prompt unit cases)
- render::default_ignorables_visible (unit part), pty::paste_strips_escape_sequences (for_paste unit part)
Out of scope: the PTY paste path (M3-12), rendering (M2-05).

### [M1-24] Step kinds and states
Class: HARD
Stage: 6
Depends on: M1-18
ADRs: none
Threats mitigated: none
Likely files: crates/foreman-core/src/steps.rs, crates/foreman-core/tests/steps.rs
Acceptance criteria:
- AC-F007-01
- AC-F007-05
- AC-F007-06
Required tests:
- steps::transitions_exhaustive, steps::wait_until_date, steps::repeatable_counter
- steps::finish_rules, steps::followup_copy_or_relaunch
Out of scope: live followup and manual behavior (M3-17).

### [M1-25] Action steps with fixed destinations
Class: HARD
Stage: 6
Depends on: M1-24, M1-05
ADRs: ADR-010
Threats mitigated: T-051
Likely files: crates/foreman-core/src/steps/actions.rs, crates/foreman-core/tests/actions.rs
Acceptance criteria:
- AC-F007-02
- AC-F007-03
Required tests:
- actions::destinations_fixed
Out of scope: the action preview UI (`action_preview`/`action_apply`, wired in M2-21).

### [M1-26] Packet validation
Class: HARD
Stage: 6
Depends on: M1-13, M1-18, M1-17
ADRs: none
Threats mitigated: T-087, T-105
Likely files: crates/foreman-core/src/packet/mod.rs, crates/foreman-core/src/packet/validate.rs, crates/foreman-core/tests/packet.rs, crates/foreman-core/tests/fixtures/packets/
Acceptance criteria:
- AC-F013-01
- AC-F013-02
- AC-F013-03
- AC-F002-08
Required tests:
- packet::validation_rules, packet::never_panics, packet::signature_forms, packet::na_values_need_skips
Out of scope: normalization and splitting (M3-20).

### [M1-27] Decision files and approved copies
Class: HARD
Stage: 6
Depends on: M1-05, M1-18
ADRs: ADR-022, ADR-023
Threats mitigated: T-025, T-026, T-063
Likely files: crates/foreman-core/src/decisions.rs, crates/foreman-core/src/schema/project.rs, crates/foreman-core/src/schema/overlay.rs, crates/foreman-core/src/schema/lock.rs, crates/foreman-core/tests/decisions.rs
Acceptance criteria:
- AC-F022-01
- AC-F022-03
- AC-F020-05
Required tests:
- decisions::outside_edit_has_no_effect_until_accepted, decisions::variables_only_autoaccept
Out of scope: the review UI (M2-24), live detection (M3-22).

### [M1-28] Foreman's own decision commits
Class: HARD
Stage: 6
Depends on: M1-27, M1-10
ADRs: ADR-012, ADR-022
Threats mitigated: T-080
Likely files: crates/foreman-core/src/decisions.rs, src-tauri/src/decisions.rs, crates/foreman-sys/tests/decision_commit.rs
Acceptance criteria:
- AC-F020-08 (including the unborn-HEAD first commit and the detached-HEAD refusal, where the decision change stays pending)
- AC-F020-09
Required tests:
- decisions::pending_from_stage5_not_flagged, sign::pathspec_only (decision-file case)
- sign::unborn_head_first_commit (setup's `git init`, then the first `condition_set`), sign::detached_head_refused (decision change stays pending)
Out of scope: including pending decisions in a signing commit (M4-20).

### [M1-29] App-data file contracts
Class: HARD
Stage: 6
Depends on: M1-02, M1-05
ADRs: ADR-023, ADR-010
Threats mitigated: T-076
Likely files: crates/foreman-core/src/schema/settings.rs, crates/foreman-core/src/schema/projects.rs, crates/foreman-core/src/schema/progress.rs, crates/foreman-core/src/schema/approvals.rs, crates/foreman-core/src/schema/run.rs, crates/foreman-core/tests/file_contracts.rs
Acceptance criteria:
- AC-XC-15
- Given each file in `docs/contracts/files.md` §2, then a typed schema with `schemaVersion` exists, its JSON Schema validates the golden example from files.md, and its data class matches PR-02
Required tests:
- schema::all_versions_migrate (one case per file)
Out of scope: writers that use them (later tickets).

### [M1-30] TradeLog harness: Stages 1–2 and F.2
Class: HARD
Stage: 6
Depends on: M1-22, M1-19, M1-24, M1-26
ADRs: ADR-015
Threats mitigated: none
Likely files: crates/foreman-core/tests/harness_tradelog.rs, crates/foreman-core/tests/fixtures/tradelog/project.json, crates/foreman-core/tests/fixtures/tradelog/handoff/
Acceptance criteria:
- AC-F003-01
- AC-F003-03
- EX-M1-03
Required tests:
- none beyond the harness test itself (`harness::tradelog_stages_1_2`)
Out of scope: argv snapshots (M3-27).

### [M1-31] Abuse-case suite
Class: HARD
Stage: 6
Depends on: M1-22, M1-25, M1-26, M1-14
ADRs: ADR-005
Threats mitigated: T-016, T-051, T-087
Likely files: crates/foreman-core/tests/abuse.rs, crates/foreman-core/tests/fixtures/abuse/
Acceptance criteria:
- AC-XC-09
- Given the AB-01 fixture (a golden packet and overlay containing every forbidden flag, a leading `/`, `bypassPermissions`, a `.git/hooks` destination and an instruction line), then the launch inputs (effective start, session plan, destinations) are byte-identical to the clean fixture; M2-06 extends the assertion to the built argv
Required tests:
- abuse::*, abuse::packet_text_cannot_change_flags, abuse::overlay_text_cannot_change_flags
- abuse::ticket_text_cannot_trigger_actions, abuse::annotation_cannot_escape_policy
Out of scope: none.

### [M1-32] M1 benchmarks
Class: SPEC-COMPLETE
Stage: 6
Depends on: M1-16, M1-22
ADRs: ADR-015
Threats mitigated: none
Likely files: crates/foreman-core/benches/template.rs, crates/foreman-core/benches/resolver.rs, crates/foreman-core/tests/fixtures/blueprint-20k.md
Acceptance criteria:
- AC-F001-07
- AC-F006-16
- EX-M1-05 (NFR-03, NFR-04 resolution part, NFR-24; NFR-10 and NFR-12 via M1-05)
Required tests:
- none named (criterion benches)
Out of scope: none.

### [M1-33] M1 exit review
Class: SPEC-COMPLETE
Stage: 6
Depends on: M1-01, M1-02, M1-03, M1-04, M1-05, M1-06, M1-07, M1-08, M1-09, M1-10, M1-11, M1-12, M1-13, M1-14, M1-15, M1-16, M1-17, M1-18, M1-19, M1-20, M1-21, M1-22, M1-23, M1-24, M1-25, M1-26, M1-27, M1-28, M1-29, M1-30, M1-31, M1-32
ADRs: ADR-003, ADR-015
Threats mitigated: none
Likely files: docs/handoff/stage-06.md
Acceptance criteria:
- EX-M1-01
- EX-M1-04
- EX-M1-06
Required tests:
- none named (full `cargo test`; `code-reviewer` report)
Out of scope: none.

---

## M2 · Walking skeleton (Stage 7)

### [M2-01] IPC command framework
Class: HARD
Stage: 7
Depends on: M0-04, M1-33
ADRs: ADR-004, ADR-014
Threats mitigated: T-005, T-006, T-011
Likely files: src-tauri/src/ipc/mod.rs, src-tauri/src/ipc/error.rs, src-tauri/src/ipc/events.rs, src-tauri/src/ipc/commands/, src-tauri/tests/ipc_fuzz.rs, src/lib/ipc/bindings/commands.json
Acceptance criteria:
- AC-XC-05
- AC-XC-06
- Given every P1 command in `docs/contracts/ipc.md` §4.15, then it is listed in `ipc::COMMANDS`, declared through `AppManifest::commands` and has a request fixture that deserializes in Rust
Required tests:
- ipc::rejects_unknown_fields, ipc::fuzz_*, ipc::panic_returns_typed_error
Out of scope: command behavior (feature tickets).

### [M2-02] Mock IPC and contract tests
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-01
ADRs: ADR-004, ADR-015
Threats mitigated: none
Likely files: src/lib/ipc/mock.ts, src/lib/ipc/fixtures/, src/lib/ipc/contract.test.ts, src/lib/ipc/mock.coverage.test.ts
Acceptance criteria:
- EX-M2-02
- Given the mock rules in `docs/contracts/ipc.md` §7.2 (every command, deterministic clock and IDs, scenarios, scripted PTY, scripted native dialogs, mirrored security behaviors, not in the release bundle), then each has a passing test
- Given the commands and fields the Stage 4.7 review added (`launch_prepare.mode`, `paste_back_preview.source`, `chain_resume_prepare`, `packet_repair_prepare`, `metrics_view`, `project_pin_set`), then each has the request and response fixtures `docs/contracts/ipc.md` names (P2 commands answer from fixtures until their feature ticket lands)
Required tests:
- ipc::rejects_unknown_fields (mirrored in the mock)
Out of scope: none.

### [M2-03] Rust-built main window and startup hardening
Class: HARD
Stage: 7
Depends on: M2-01
ADRs: ADR-014
Threats mitigated: T-008, T-097
Likely files: src-tauri/src/window.rs, src-tauri/src/main.rs, tests/e2e/navigation.e2e.ts
Acceptance criteria:
- AC-XC-27
- Given startup, then `umask(0o022)` is set and app-data folders are created 0700
Required tests:
- webview::navigation_denied_to_remote, webview::window_open_denied, fs::app_data_permissions (startup case)
Out of scope: none.

### [M2-04] App shell, routes and UI primitives
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-02
ADRs: ADR-002, ADR-020
Threats mitigated: none
Likely files: src/routes/+layout.svelte, src/routes/+page.svelte, src/routes/first-run/+page.svelte, src/routes/setup/+page.svelte, src/routes/doctor/+page.svelte, src/routes/settings/+page.svelte, src/routes/projects/[projectId]/+layout.svelte, src/routes/projects/[projectId]/stages/[stage]/+page.svelte, src/lib/components/ui/, src/lib/stores/, src/lib/nav/, docs/design/tokens.css → src/lib/styles/tokens.css
Acceptance criteria:
- EX-M2-01 (routes exist; real data arrives with each screen ticket)
- Given each P1 route, then it renders under the root layout (sidebar, terminal drawer host, aria-live region) in mock mode, with bits-ui 2.19.5 wrappers styled only by tokens
Required tests:
- none named (one component test per route stub)
Out of scope: screen content (later M2 tickets).

### [M2-05] Safe rendering: node trees and visible escapes
Class: HARD
Stage: 7
Depends on: M2-04, M1-23
ADRs: ADR-004, ADR-005
Threats mitigated: T-007, T-077, T-088, T-089
Likely files: crates/foreman-core/src/render.rs, src/lib/render/, src/lib/components/Diff.svelte, src/lib/components/PacketView.svelte, src/lib/render/render.test.ts
Acceptance criteria:
- AC-F030-04
- AC-XC-32
Required tests:
- render::escapes_html_in_packet, render::no_raw_html_passthrough, render::bidi_visible_in_diff
- render::default_ignorables_visible, render::hostile_filename_escaped
Out of scope: none.

### [M2-06] Argv builder
Class: HARD
Stage: 7
Depends on: M1-19, M1-23, M1-07, M1-31
ADRs: ADR-007
Threats mitigated: T-015, T-016, T-017, T-035
Likely files: crates/foreman-core/src/launch/argv.rs, crates/foreman-core/tests/argv.rs
Acceptance criteria:
- AC-F008-12
- AC-F008-14
- AC-XC-01
- AC-XC-03
Required tests:
- argv::only_known_flags, argv::never_contains_forbidden_flags, argv::no_print_mode_in_v1, argv::name_format
- argv::positional_prompt_snapshot, argv::rejects_option_like_prompt, spawn::no_shell_in_any_code_path
- abuse::packet_text_cannot_change_flags (extended to the built argv)
Out of scope: session flags (M3-14).

### [M2-07] Child environment and PATH
Class: HARD
Stage: 7
Depends on: M1-07
ADRs: ADR-007
Threats mitigated: T-018, T-019
Likely files: crates/foreman-core/src/launch/env.rs, crates/foreman-core/src/launch/path.rs, crates/foreman-core/tests/env.rs
Acceptance criteria:
- AC-F008-15
Required tests:
- env::allowlist_only, env::denylist_beats_passthrough, env::oauth_token_passed_when_present
- env::path_entries_sanitized, env::removed_list_covers_blocked_patterns
Out of scope: the pass-through list setting (M4-34).

### [M2-08] Child process lifecycle
Class: HARD
Stage: 7
Depends on: M2-07
ADRs: ADR-007
Threats mitigated: T-034
Likely files: crates/foreman-sys/src/spawn.rs, src-tauri/src/state.rs (run table), crates/foreman-sys/tests/lifecycle.rs
Acceptance criteria:
- AC-F008-25
- AC-XC-31
Required tests:
- spawn::no_inherited_fds (launch case), lifecycle::crash_leaves_interrupted
Out of scope: the quit guard UI (M3-13).

### [M2-09] Run settings file
Class: HARD
Stage: 7
Depends on: M1-05, M0-13
ADRs: ADR-021
Threats mitigated: T-023, T-025, T-026, T-064
Likely files: crates/foreman-core/src/launch/run_settings.rs, crates/foreman-core/tests/run_settings.rs
Acceptance criteria:
- AC-F008-16
Required tests:
- run_settings::contents_snapshot, run_settings::deny_paths_absolute (including a repo path with a space), run_settings::fresh_per_run
- run_settings::deny_case_variant_probe (S-1′ result encoded: case variants listed if needed)
Out of scope: sensor hooks (M4-08).

### [M2-10] Run record and redacted run log
Class: HARD
Stage: 7
Depends on: M1-29, M2-06
ADRs: ADR-018
Threats mitigated: T-033, T-059, T-072
Likely files: crates/foreman-core/src/runlog.rs, src-tauri/src/launch.rs, crates/foreman-core/tests/runlog.rs
Acceptance criteria:
- AC-F008-19
- AC-F026-03
Required tests:
- runlog::record_before_spawn, runlog::argv_prompt_redacted, runlog::no_env_values, runlog::index_torn_line_skipped
Out of scope: metrics fields (M3-26).

### [M2-11] `claude -v` and `claude auth status` readers
Class: HARD
Stage: 7
Depends on: M2-07, M1-20, M0-05
ADRs: ADR-017, ADR-021
Threats mitigated: T-036
Likely files: crates/foreman-sys/src/claude.rs, crates/foreman-sys/tests/claude.rs, crates/fake-claude/scripts/auth-*.json
Acceptance criteria:
- AC-F019-05
- Given `auth status` output, then only `authMethod` and the exit code are read; unparsable output, unknown method, non-zero exit or a 10 s timeout are distinct typed results
Required tests:
- compat::parses_version_output (live output cases), preflight::blocks_api_key_auth, preflight::auth_unknown_output_blocks
Out of scope: running auth in the launch pipeline (M3-04).

### [M2-12] Settings backend and tool paths
Class: HARD
Stage: 7
Depends on: M2-01, M1-07, M1-29
ADRs: ADR-007, ADR-014
Threats mitigated: T-004, T-014
Likely files: src-tauri/src/ipc/commands/settings.rs, src-tauri/src/os.rs (dialog), crates/foreman-core/src/settings.rs, src-tauri/tests/settings.rs
Acceptance criteria:
- AC-F029-01
- AC-F029-02
- AC-F029-03
- AC-F008-03
Required tests:
- settings::security_fields_require_native_confirm, spawn::rejects_relative_path, spawn::rejects_symlink_into_project
Out of scope: P2 settings (M4-34).

### [M2-13] File opening and reveal
Class: HARD
Stage: 7
Depends on: M2-12, M1-04
ADRs: ADR-014, ADR-007
Threats mitigated: T-044
Likely files: crates/foreman-sys/src/open.rs, src-tauri/src/ipc/commands/files.rs, crates/foreman-sys/tests/open.rs
Acceptance criteria:
- AC-XC-28
- AC-F029-07 (backend part)
Required tests:
- opener::open_file_refuses_executable_types, opener::uses_editor_bundle_id
Out of scope: URL opening (M3-06).

### [M2-14] Settings screen (P1)
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-12, M2-13, M2-04
ADRs: ADR-020
Threats mitigated: none
Likely files: src/routes/settings/+page.svelte, src/lib/stores/settings.svelte.ts, src/routes/settings/settings.test.ts (mockup: docs/design/mockups/settings.html)
Acceptance criteria:
- AC-F029-07 (UI part: Open in editor disabled with the reason when no editor is set)
- Given the Settings screen, then `claude` and `git` paths, DRI name and editor are shown and changed through Rust dialogs, with refusals shown inline
Required tests:
- none named (component test against the mock)
Out of scope: P2 settings (M4-34).

### [M2-15] Doctor (P1)
Class: HARD
Stage: 7
Depends on: M2-11, M2-12, M2-04
ADRs: ADR-017
Threats mitigated: T-036
Likely files: src-tauri/src/doctor.rs, src-tauri/src/ipc/commands/doctor.rs, src/routes/doctor/+page.svelte, src/lib/stores/doctor.svelte.ts (mockup: docs/design/mockups/doctor.html)
Acceptance criteria:
- AC-F019-04
- AC-F019-06
- AC-F019-07
Required tests:
- none named (integration tests with fake-claude scenarios; component test)
Out of scope: capability table and probes (M4-12, M4-13).

### [M2-16] First run
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-15, M1-16, M1-06
ADRs: ADR-002
Threats mitigated: T-068
Likely files: src-tauri/src/ipc/commands/first_run.rs, src/routes/first-run/+page.svelte (mockup: docs/design/mockups/first-run.html)
Acceptance criteria:
- AC-F019-01
- AC-F019-02
- AC-F019-03
- AC-F001-18
Required tests:
- fs_policy::read_scope (auto-detect candidates only)
Out of scope: probe-folder trust (M3-18, now in M4), full Template health (M4-06).

### [M2-17] Project setup backend
Class: HARD
Stage: 7
Depends on: M1-17, M1-27, M1-09, M1-08
ADRs: ADR-010, ADR-012, ADR-022
Threats mitigated: T-041, T-051
Likely files: src-tauri/src/ipc/commands/projects.rs, crates/foreman-core/src/setup.rs, src-tauri/tests/setup.rs
Acceptance criteria:
- AC-F018-02
- AC-F018-03
- AC-F018-04
- AC-F018-05
- AC-F018-06
- AC-F018-07
Required tests:
- git::refuses_enclosing_repo (setup offers `git init`), atomic::refuses_newer_file (existing files never overwritten)
Out of scope: the form UI (M2-18).

### [M2-18] Setup form
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-17, M2-04
ADRs: ADR-020
Threats mitigated: none
Likely files: src/routes/setup/+page.svelte, src/lib/components/setup/ (mockup: docs/design/mockups/setup.html)
Acceptance criteria:
- AC-F018-01
Required tests:
- none named (component test: fill, review, create against the mock)
Out of scope: the wizard (M4-26).

### [M2-19] Project open, registry and read-only states
Class: HARD
Stage: 7
Depends on: M2-17, M1-26, M1-22
ADRs: ADR-022, ADR-023
Threats mitigated: T-065
Likely files: src-tauri/src/state.rs, src-tauri/src/ipc/commands/projects.rs, src/lib/stores/projects.svelte.ts, src/lib/components/ReadOnlyBanner.svelte
Acceptance criteria:
- AC-F030-03
- Given `project_open`, then the runtime holds the template, lock check, approved decisions and applicable path, and `project:changed` carries only `{projectId, rev}`
Required tests:
- lock::hash_mismatch_read_only (end to end), schema::newer_version_read_only (end to end)
Out of scope: none.

### [M2-20] Today (P1)
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-19
ADRs: ADR-002
Threats mitigated: none
Likely files: src/routes/+page.svelte, src/lib/stores/today.svelte.ts, src-tauri/src/ipc/commands/app.rs (`today_get`, `do_next`) (mockup: docs/design/mockups/today.html)
Acceptance criteria:
- AC-F017-01
- AC-F017-02
- AC-F017-03
- AC-F017-04
- AC-F017-05
Required tests:
- none named (component tests)
Out of scope: the inbox (M4-25).

### [M2-21] Stage workspace: step rows and spec line
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-19, M2-05, M1-25
ADRs: ADR-020
Threats mitigated: none
Likely files: src/routes/projects/[projectId]/stages/[stage]/+page.svelte, src/lib/components/StepRow.svelte, src/lib/components/SpecLine.svelte, src/lib/components/Lock.svelte, src/lib/components/NotApplicableRow.svelte, src-tauri/src/ipc/commands/steps.rs (`step_view`, `step_skip`, `action_preview`, `action_apply`) (mockup: docs/design/mockups/workspace.html)
Acceptance criteria:
- AC-F030-01
- AC-F030-02
- AC-F004-03
- AC-F004-04
- AC-F004-07
- AC-F005-03
- AC-F005-04
Required tests:
- rules::skip_rules (IPC rejection of a core skip)
Out of scope: inputs and prompt block (M2-22).

### [M2-22] Run inputs and prompt block
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-21
ADRs: ADR-020
Threats mitigated: none
Likely files: src/lib/components/RunInputForm.svelte, src/lib/components/PromptBlock.svelte, src/lib/components/OptionalPartToggle.svelte, src-tauri/src/ipc/commands/steps.rs (`run_input_set`, `optional_part_set`)
Acceptance criteria:
- AC-F006-12
- AC-F006-13
- AC-F006-14
- AC-F008-11
Required tests:
- resolver::inputs_required_only_if_present (through IPC)
Out of scope: none.

### [M2-23] Redacted config and decision snapshots
Class: HARD
Stage: 7
Depends on: M1-02
ADRs: ADR-021, ADR-022
Threats mitigated: T-070
Likely files: crates/foreman-core/src/fingerprint/redact.rs, crates/foreman-core/tests/redact.rs
Acceptance criteria:
- AC-F008-23
Required tests:
- preflight::config_snapshot_redacts_secrets, preflight::diff_shows_redacted_change
Out of scope: fingerprint sources (M3-01).

### [M2-24] Conditions, variables and the outside-change review
Class: HARD
Stage: 7
Depends on: M2-23, M1-27, M1-28, M2-19
ADRs: ADR-022
Threats mitigated: T-003, T-063
Likely files: src-tauri/src/ipc/commands/decisions.rs, src/lib/components/OutsideChangeReview.svelte, src/lib/components/ConditionControl.svelte (mockup: docs/design/mockups/outside-change.html)
Acceptance criteria:
- AC-F004-11
- AC-F022-02 (int: a scripted native dialog, Cancel leaves the approved copy, hash and pending change unchanged; comp: the review shows the redacted diff as context)
- AC-F022-05
Required tests:
- decisions::reject_restores_via_cas, ipc::security_confirmations_are_native (`pending_change_accept`)
Out of scope: live detection (M3-22).

### [M2-25] Launch preview and native confirmations
Class: HARD
Stage: 7
Depends on: M2-06, M2-07, M2-09, M2-10, M2-11, M2-21
ADRs: ADR-021, ADR-007
Threats mitigated: T-003, T-032
Likely files: crates/foreman-core/src/launch/mod.rs (`needs_confirmation`), src-tauri/src/launch.rs, src-tauri/src/ipc/commands/launch.rs (`launch_prepare`), src/lib/components/StepActions.svelte, src/lib/components/LaunchPreview.svelte
Acceptance criteria:
- AC-F008-17
- AC-F008-18 (int: Rust raises the native dialog from a scripted answer, Cancel returns `Cancelled` with nothing spawned and no run record; comp: the preview shows the reasons as context and `launch_start` takes only `planId`)
- AC-F008-03 (Run blocked with the path named)
- Given a plan, then its `planId` is single-use, valid 60 s, and bound to the project revision
- Given `launch_prepare { mode: "freshWithContext" }` on a non-continuing step, then `NotAllowed("fresh-not-continuing")` (the fresh-chain behavior itself is M3-15)
Required tests:
- preflight::confirm_required_for_{overlay_step,mode_change,auto_and_dontask,first_launch,version_change}, one test per reason: preflight::confirm_required_for_overlay_step, preflight::confirm_required_for_mode_change, preflight::confirm_required_for_auto_and_dontask, preflight::confirm_required_for_first_launch, preflight::confirm_required_for_version_change
- ipc::security_confirmations_are_native (launch confirmation; static check over every `*Req.ts` binding)
Out of scope: spawning in the PTY (M3-10); fingerprint and auth phases (M3-01 to M3-05).

### [M2-26] Core shortcuts and the terminal drawer shell
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-21, M2-25
ADRs: ADR-002
Threats mitigated: none
Likely files: src/lib/keyboard/registry.ts, src/lib/keyboard/shortcuts.test.ts, src/routes/+layout.svelte, src/lib/components/TerminalDrawer.svelte, src-tauri/src/ipc/commands/steps.rs (`step_mark_done`)
Acceptance criteria:
- AC-F028-01
- AC-F028-03
- AC-F007-08
- AC-F030-05
Required tests:
- none named (component tests)
Out of scope: palette and remaining shortcuts (M4-33).

### [M2-27] Screen component tests, e2e smoke and performance marks
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-14, M2-16, M2-18, M2-20, M2-22, M2-24, M2-26
ADRs: ADR-015
Threats mitigated: none
Likely files: src/routes/**/*.test.ts, tests/flows/*.spec.ts, tests/e2e/smoke.e2e.ts, src/lib/perf.ts
Acceptance criteria:
- EX-M2-03
- Given the run log and frontend marks, then NFR-01, NFR-02 and NFR-07 are instrumented and reported (targets enforced in M5-09)
Required tests:
- none named
Out of scope: none.

### [M2-28] M2 exit
Class: SPEC-COMPLETE
Stage: 7
Depends on: M2-03, M2-08, M2-13, M2-27
ADRs: ADR-007
Threats mitigated: none
Likely files: docs/handoff/stage-07.md
Acceptance criteria:
- EX-M2-04
- EX-M2-05
- EX-M2-06
Required tests:
- none named (full suite; first run on Billy's Mac)
Out of scope: none.

---

## M3 · Runners (Stages 8–9) · kill check

### [M3-01] Fingerprint sources and stat cache
Class: HARD
Stage: 8
Depends on: M2-23, M1-06
ADRs: ADR-021
Threats mitigated: T-021, T-058, T-070
Likely files: crates/foreman-core/src/fingerprint/mod.rs, crates/foreman-core/src/fingerprint/sources.rs, crates/foreman-core/src/store/outside.rs, crates/foreman-core/tests/fingerprint.rs
Acceptance criteria:
- AC-F008-06
- AC-F008-24
- Given the plugin folder layout is unverified (design doc §7), then the whole `plugins/` directory of each enabled plugin is fingerprinted
- Given `~/.claude.json`, then it is parsed as a stream under its own 64 MB cap and only the `mcpServers` paths are kept; given a read-only source outside the repo that is a symlink to a regular file outside every registered project, then the target is hashed and both link and target are recorded (ADR-021)
Required tests:
- preflight::hook_script_change_detected, preflight::ancestor_claude_md_detected, preflight::user_mcp_server_detected (one test per source)
- fingerprint::claude_json_streamed_64mb, fingerprint::outside_symlink_followed_to_regular_file, fingerprint::outside_symlink_into_project_blocks
Out of scope: the banner (M3-02).

### [M3-02] Config-change banner and acknowledgement
Class: HARD
Stage: 8
Depends on: M3-01, M2-25
ADRs: ADR-021
Threats mitigated: T-003, T-021
Likely files: src-tauri/src/launch.rs (`config_ack`), src/lib/components/ConfigChangedBanner.svelte
Acceptance criteria:
- AC-F008-04
Required tests:
- preflight::diff_shows_redacted_change, ipc::security_confirmations_are_native (`config_ack`)
Out of scope: none.

### [M3-03] Auth-changing key blocks
Class: HARD
Stage: 8
Depends on: M3-01, M0-15
ADRs: ADR-021
Threats mitigated: T-020
Likely files: crates/foreman-core/src/fingerprint/auth_keys.rs, src-tauri/src/ipc/commands/launch.rs (`auth_key_allowlist_add`, `project_env_approve`), crates/foreman-core/tests/auth_keys.rs
Acceptance criteria:
- AC-F008-05
- AC-F008-07 (built per S-7′'s branch)
- AC-F008-22
Required tests:
- preflight::blocks_<key> (one per key and pattern), preflight::blocks_apiKeyHelper, preflight::allowlisted_user_key_passes
- settings::security_fields_require_native_confirm (allowlisted hashes)
Out of scope: none.

### [M3-04] Auth check, fail closed
Class: HARD
Stage: 8
Depends on: M3-03, M2-11
ADRs: ADR-021
Threats mitigated: T-020, T-022
Likely files: src-tauri/src/launch.rs, crates/foreman-core/src/launch/preflight.rs, src-tauri/tests/auth.rs
Acceptance criteria:
- AC-F008-08
- AC-F008-09
Required tests:
- preflight::auth_checked_in_step_cwd, preflight::blocks_api_key_auth, preflight::auth_unknown_output_blocks, preflight::auth_after_fingerprint
Out of scope: none.

### [M3-05] Launch pipeline order and the version floor
Class: HARD
Stage: 8
Depends on: M3-02, M3-04, M2-25
ADRs: ADR-021, ADR-017
Threats mitigated: T-036, T-068
Likely files: crates/foreman-core/src/launch/preflight.rs (`evaluate_preflight`), src-tauri/src/launch.rs, src-tauri/tests/launch_prepare.rs, crates/foreman-core/benches/prepare_launch.rs
Acceptance criteria:
- AC-F008-01
- AC-F008-10
- AC-F008-13 (NFR-04 measured excluding the `claude -v` spawn, which runs in parallel with the fingerprint and counts toward NFR-05)
- EX-M3-04
Required tests:
- launch::progress_phase_order, compat::blocks_below_floor
Out of scope: probes (M4-12).

### [M3-06] URL opener and clipboard discipline
Class: HARD
Stage: 8
Depends on: M2-01
ADRs: ADR-014
Threats mitigated: T-006, T-099, T-100
Likely files: src-tauri/src/os.rs, src-tauri/tests/opener.rs, src-tauri/tests/clipboard.rs
Acceptance criteria:
- AC-XC-08
- AC-XC-29
Required tests:
- opener::only_allowed_urls, clipboard::read_only_on_user_action, clipboard::size_cap
Out of scope: terminal links (M3-12).

### [M3-07] Wait-step notifications
Class: HARD
Stage: 8
Depends on: M1-24, M2-21
ADRs: ADR-014
Threats mitigated: T-098
Likely files: src-tauri/src/os.rs (notification templates), src-tauri/src/ipc/commands/steps.rs (`wait_start`), src-tauri/tests/notify.rs
Acceptance criteria:
- AC-F007-09
- Given a `wait` date that passed while Foreman wasn't running, then the notification fires at the next launch (notifications are P1, Master Spec §12)
Required tests:
- notify::body_template_snapshot
Out of scope: inbox notifications (M4-25).

### [M3-08] CSP on the built app
Class: HARD
Stage: 8
Depends on: M2-27
ADRs: ADR-014, ADR-015
Threats mitigated: T-001, T-007
Likely files: tests/e2e/csp.e2e.ts
Acceptance criteria:
- AC-XC-07
Required tests:
- csp::production_config_snapshot (served header matches it)
Out of scope: none.

### [M3-09] PTY session
Class: HARD
Stage: 9
Depends on: M2-08, M0-12
ADRs: ADR-006
Threats mitigated: T-012, T-034
Likely files: crates/foreman-sys/src/pty.rs, crates/foreman-sys/tests/pty.rs
Acceptance criteria:
- AC-F009-02
- AC-F009-03
- AC-F009-07
- AC-F009-10
- Given the PTY cwd, then it is reached through the no-symlink walk (portable-pty never falls back to `$HOME`); given 3 and 12 fake sessions, then NFR-22 holds
Required tests:
- pty::child_env_exact, pty::no_inherited_fds, pty::burst_1mb_backpressure
Out of scope: the frontend terminal (M3-11).

### [M3-10] Launch start and terminal tabs
Class: HARD
Stage: 9
Depends on: M3-09, M3-05
ADRs: ADR-006, ADR-018
Threats mitigated: T-002
Likely files: src-tauri/src/launch.rs (`launch_start`), src-tauri/src/ipc/commands/terminal.rs (`pty_attach`, `pty_write`, `pty_resize`, `pty_flow`), src/lib/stores/terminals.svelte.ts
Acceptance criteria:
- AC-F009-01
- AC-F009-19
- EX-M3-02 (argv on screen equals argv spawned)
Required tests:
- pty::write_requires_live_run, runlog::record_before_spawn (PTY path)
Out of scope: re-arm (M3-15).

### [M3-11] Terminal host and key routing
Class: HARD
Stage: 9
Depends on: M3-10
ADRs: ADR-006
Threats mitigated: T-094, T-095, T-096
Likely files: src/lib/terminal/host.ts, src/lib/terminal/keys.ts, src/lib/terminal/options.ts, src/lib/terminal/archive.ts, src/lib/terminal/terminal.test.ts
Acceptance criteria:
- AC-F009-06 (cap per S-0's branch)
- AC-F009-08
- AC-F009-18
- AC-F008-20 (driven by the Settings "Screen reader mode" toggle, `settings_update.screenReaderMode`; Foreman doesn't detect VoiceOver)
Required tests:
- terminal::options_snapshot, terminal::no_answerback_injection
Out of scope: paste and links (M3-12).

### [M3-12] Terminal paste and links
Class: HARD
Stage: 9
Depends on: M3-11, M3-06
ADRs: ADR-006, ADR-014
Threats mitigated: T-091, T-092, T-093
Likely files: crates/foreman-sys/src/pty.rs (paste), src-tauri/src/ipc/commands/terminal.rs (`pty_paste`, `terminal_link_open`), src/lib/terminal/links.ts, src/lib/terminal/paste.ts
Acceptance criteria:
- AC-F009-15
- AC-F009-16
- AC-F009-17
Required tests:
- pty::paste_strips_escape_sequences, terminal::link_handler_routes_to_rust, opener::terminal_link_scheme_policy
Out of scope: none.

### [M3-13] Tab menu, terminate and the quit guard
Class: HARD
Stage: 9
Depends on: M3-10
ADRs: ADR-006, ADR-007
Threats mitigated: T-029, T-034
Likely files: src-tauri/src/ipc/commands/terminal.rs (`pty_interrupt`, `pty_terminate`), src-tauri/src/main.rs (quit guard), src/lib/components/TerminalTab.svelte
Acceptance criteria:
- AC-F009-04
- AC-F009-05
- AC-F009-09
Required tests:
- rearm::no_kill_without_idle_or_confirm (terminate path), lifecycle::crash_leaves_interrupted (SIGHUP on quit)
- ipc::security_confirmations_are_native (`pty_terminate` and the quit guard: a scripted Cancel sends no signal)
Out of scope: none.

### [M3-14] Session chains: planning and session flags
Class: HARD
Stage: 9
Depends on: M1-24, M2-06, M0-14, M0-16
ADRs: ADR-008
Threats mitigated: T-030
Likely files: crates/foreman-core/src/chain.rs, crates/foreman-core/tests/chain.rs
Acceptance criteria:
- AC-F010-01
- AC-F010-02
- AC-F010-08
- AC-F010-10 (S-3's branch)
Required tests:
- chain::plan_session_table, chain::superseded_on_rerun, chain::broken_when_transcript_missing, chain::start_not_applicable_becomes_fresh
- chain::effort_on_resume_branches, chain::fork_argv, argv::session_id_only_on_chain_start_or_fork
Out of scope: re-arm sequencing (M3-15).

### [M3-15] Re-arm
Class: HARD
Stage: 9
Depends on: M3-14, M3-10, M3-13
ADRs: ADR-008
Threats mitigated: T-029, T-031
Likely files: src-tauri/src/rearm.rs, src-tauri/src/ipc/commands/terminal.rs (`rearm_start`, `rearm_continue`), src-tauri/src/ipc/commands/launch.rs (`launch_prepare` `mode: freshWithContext`), src-tauri/tests/rearm.rs
Acceptance criteria:
- AC-F010-03
- AC-F010-04 (S-8's branch)
- AC-F010-05
- AC-F010-06
- AC-F010-07
- AC-F010-09
- AC-F010-11
- AC-XC-19
Required tests:
- rearm::ctrl_c_then_exit_order, rearm::no_kill_without_idle_or_confirm, rearm::only_from_user_action, rearm::stale_plan_not_spawned
- ipc::security_confirmations_are_native (`rearm_start`: the P1 idle assertion is a native dialog; a scripted Cancel sends nothing)
Out of scope: sensor idle (M4-10).

### [M3-16] Restart, interrupted sessions and rebuild from the repo
Class: HARD
Stage: 9
Depends on: M3-15
ADRs: ADR-010, ADR-023
Threats mitigated: T-059
Likely files: src-tauri/src/state.rs, src-tauri/src/ipc/commands/decisions.rs (`progress_rebuild_preview`, `progress_rebuild_apply`), src-tauri/src/ipc/commands/launch.rs (`chain_resume_prepare`), src-tauri/tests/restart.rs
Acceptance criteria:
- AC-F010-12
- AC-F020-10 (Resume goes through `chain_resume_prepare`, then the normal `launch_start` with its preflight and native confirmation)
- AC-F009-05 (Resume part, same command)
- AC-F020-11
- AC-F020-12
Required tests:
- lifecycle::crash_leaves_interrupted (restart case)
Out of scope: none.

### [M3-17] Followup and manual steps, switches and overrides
Class: HARD
Stage: 9
Depends on: M3-15
ADRs: ADR-008
Threats mitigated: none
Likely files: src-tauri/src/ipc/commands/steps.rs (`clipboard_copy`, `run_override_set`), src-tauri/tests/followup.rs
Acceptance criteria:
- AC-F007-04
- AC-F007-07
- AC-F005-05 (run record part)
- AC-F005-07
Required tests:
- steps::followup_copy_or_relaunch, steps::finish_rules, policy::switches_never_typed, policy::override_scope
Out of scope: Now/Next strip (M4-11).

### [M3-19] Watcher
Class: HARD
Stage: 9
Depends on: M1-04, M1-06, M2-19
ADRs: ADR-013
Threats mitigated: T-031, T-056, T-057, T-078
Likely files: src-tauri/src/watch.rs, src-tauri/tests/watcher.rs
Acceptance criteria:
- AC-F013-04
- AC-F013-05
- AC-F013-10
- AC-XC-17 (watcher part)
Required tests:
- watcher::interest_set_exact_names, watcher::symlinked_root_refused, watcher::no_symlink_follow
- watcher::ignores_tmp_and_secrets, watcher::events_never_act
Out of scope: none.

### [M3-20] Packet normalization, splitting and fix requests
Class: HARD
Stage: 9
Depends on: M1-26
ADRs: none
Threats mitigated: T-090
Likely files: crates/foreman-core/src/packet/normalize.rs, crates/foreman-core/src/packet/split.rs, crates/foreman-core/tests/packet_paste.rs, crates/foreman-core/tests/fixtures/packets/foreman/
Acceptance criteria:
- AC-F012-05
- AC-F012-10
- AC-F012-11
Required tests:
- packet::normalization_golden, packet::split_paste, packet::fix_request_text
Out of scope: writing (M3-24).

### [M3-21] Packet landing and hand-signed packets
Class: HARD
Stage: 9
Depends on: M3-19, M3-20
ADRs: ADR-013
Threats mitigated: T-085
Likely files: src-tauri/src/watch.rs, src-tauri/src/ipc/commands/packets.rs (`packet_view`, `packet_normalize_apply`, `packet_signature_confirm`), src-tauri/tests/packet_landing.rs (mockup: docs/design/mockups/packet.html)
Acceptance criteria:
- AC-F013-06
- AC-F013-07
- AC-F013-08
- AC-F013-09 (counts as signed only after Billy's native confirmation, SR-47)
- AC-F004-12
- EX-M3-03
Required tests:
- packet::outside_signature_needs_confirmation
Out of scope: tamper detection (M4-22).

### [M3-22] Live outside-change detection and config-changed runs
Class: HARD
Stage: 9
Depends on: M3-19, M2-24, M3-01
ADRs: ADR-013, ADR-022, ADR-021
Threats mitigated: T-027, T-063
Likely files: src-tauri/src/watch.rs, src-tauri/tests/outside_change_live.rs
Acceptance criteria:
- AC-F022-04
Required tests:
- watcher::config_changed_marks_open_runs, decisions::outside_edit_has_no_effect_until_accepted (live case)
Out of scope: none.

### [M3-23] Chat chips, links and Copy and open
Class: HARD
Stage: 9
Depends on: M3-06, M3-14, M2-21
ADRs: ADR-008
Threats mitigated: T-100
Likely files: src-tauri/src/ipc/commands/chat.rs (`chat_copy_and_open`, `chat_link_set`), src/lib/components/ChatChips.svelte (mockup: docs/design/mockups/workspace-chat.html)
Acceptance criteria:
- AC-F012-01
- AC-F012-02
- AC-F012-03
- AC-F012-04
Required tests:
- chain::chat_links, opener::only_allowed_urls (chat URLs)
Out of scope: paste-back (M3-24).

### [M3-24] Paste-back preview and apply
Class: HARD
Stage: 9
Depends on: M3-20, M3-06, M2-05
ADRs: ADR-010
Threats mitigated: T-060, T-089, T-090
Likely files: src-tauri/src/ipc/commands/chat.rs (`paste_back_preview`, `paste_back_apply`), src/lib/components/PasteBack.svelte, src-tauri/tests/pasteback.rs
Acceptance criteria:
- AC-F012-06
- AC-F012-07
- AC-F012-08
- AC-F012-09
- AC-F012-12
- AC-F012-13
- AC-F012-14
- AC-XC-16
Required tests:
- pasteback::lists_targets_before_write, atomic::refuses_newer_file (paste-back conflict)
Out of scope: ADR-set split (M4-24).

### [M3-25] Close-out checklist step (P1)
Class: SPEC-COMPLETE
Stage: 9
Depends on: M3-21
ADRs: none
Threats mitigated: none
Likely files: crates/foreman-core/src/steps/closeout.rs, src/lib/components/CloseOutChecklist.svelte
Acceptance criteria:
- AC-F015-01
Required tests:
- steps::closeout_sequences
Out of scope: gates (M4-15 to M4-17).

### [M3-26] Run metrics and Export metrics
Class: HARD
Stage: 9
Depends on: M3-10, M3-23, M2-10
ADRs: ADR-018
Threats mitigated: T-072
Likely files: crates/foreman-core/src/runlog.rs (`export_metrics`), src-tauri/src/ipc/commands/packets.rs (`metrics_export`, written through `WritePurpose::UserExport`), crates/foreman-core/tests/metrics.rs
Acceptance criteria:
- AC-F026-01
- AC-F026-02 (chat time-to-start ends when the filled prompt is on the clipboard and the target chat is open, the baseline's end point)
- AC-F026-04 (time to start and active time per step, median per surface)
- AC-F026-05
- AC-F026-06
- AC-F017-06
Required tests:
- runlog::argv_prompt_redacted (metrics fields added)
Out of scope: run log screen and panel (M4-03).

### [M3-27] TradeLog harness argv snapshots
Class: HARD
Stage: 9
Depends on: M1-30, M3-14, M2-06
ADRs: ADR-015
Threats mitigated: T-016
Likely files: crates/foreman-core/tests/harness_tradelog.rs, crates/foreman-core/tests/snapshots/argv/
Acceptance criteria:
- AC-F003-02
Required tests:
- argv::positional_prompt_snapshot (every harness CLI step)
Out of scope: none.

### [M3-28] Keyboard-only P1 loop
Class: SPEC-COMPLETE
Stage: 9
Depends on: M3-15, M3-24, M3-11
ADRs: ADR-015
Threats mitigated: none
Likely files: tests/e2e/keyboard-p1.e2e.ts
Acceptance criteria:
- AC-F028-02
- EX-M3-06
Required tests:
- none named
Out of scope: none.

### [M3-29] Stage 8 security inspection
Class: SPEC-COMPLETE
Stage: 9
Depends on: M3-03, M3-05, M3-07, M3-08, M3-12, M3-16, M3-17, M3-21, M3-22, M3-25, M3-26, M3-27, M3-28, M0-11
ADRs: ADR-016
Threats mitigated: T-101
Likely files: docs/security/stage-08-review.md, docs/threat-model.md (control → test links)
Acceptance criteria:
- EX-M3-05
- EX-M3-01
- AC-M0-10 (re-run: full harness and first-run path under the deny-network profile)
Required tests:
- network::blocked_network_run
Out of scope: none.

### [M3-30] TradeLog Stages 1–2 through Foreman
Class: SPEC-COMPLETE
Stage: 9
Depends on: M3-29
ADRs: none
Threats mitigated: none
Likely files: TradeLog repo (`docs/handoff/stage-01.md`, `stage-02.md`), Foreman app data run log
Acceptance criteria:
- EX-M3-07
Required tests:
- none named (manual run)
Out of scope: close-out automation (by hand in P1).
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

### [M3-31] Kill check
Class: SPEC-COMPLETE
Stage: 9
Depends on: M3-30, M0-19
ADRs: ADR-018
Threats mitigated: none
Likely files: docs/metrics-baseline.md, Export metrics output, docs/handoff/stage-09.md
Acceptance criteria:
- AC-KC-01 (chat ends at "filled prompt on the clipboard and the target chat open" on both sides)
- AC-KC-02 (active time per step, median per surface)
- AC-KC-03
- AC-KC-04 (chat baseline: at least 10 by-hand chat steps between M0 and the kill check, M0-19)
- AC-KC-05
- EX-M3-08
- Given TradeLog Stages 1–2 run through Foreman, when Export metrics is compared with `docs/metrics-baseline.md` per surface, then the result and every proxy-event explanation are recorded in the Stage 9 packet; if any criterion fails, no M4 ticket starts and a Change Request is drafted
Required tests:
- none named
Out of scope: none.
Manual: yes (Billy runs it on his Mac; the result is a report or a recorded run, not an automated test)

---

## M4 · Complete v1 (Stages 11–12)

### [M3-18] Probe-folder trust at first run (moved from M3)
Class: HARD
Stage: 12
Depends on: M3-10, M2-16
ADRs: ADR-017
Threats mitigated: T-037
Likely files: src-tauri/src/doctor.rs (`probe_folder_open`), src/routes/first-run/+page.svelte
Acceptance criteria:
- AC-F019-08
Required tests:
- probe::runs_only_in_probe_dir (trust-tab case)
Out of scope: contract probes (M4-12). Moved from M3 in the Stage 4.7 fix pass: probe-folder trust is first needed by P2's contract probes, so it ships with them (AC-F019-08 is M4).

### [M4-01] Diagnostic logging and crash reports
Class: HARD
Stage: 11
Depends on: M3-31
ADRs: ADR-018
Threats mitigated: T-073
Likely files: src-tauri/src/logging.rs, src-tauri/tests/logging.rs
Acceptance criteria:
- AC-XC-13
- AC-XC-18
Required tests:
- logging::redaction, ipc::panic_returns_typed_error (log line has no payload)
Out of scope: none.

### [M4-02] Retention, Clear run history and project removal
Class: HARD
Stage: 11
Depends on: M4-01
ADRs: ADR-018, ADR-010
Threats mitigated: T-028, T-074, T-075
Likely files: crates/foreman-core/src/retention.rs, src-tauri/src/ipc/commands/packets.rs (`runs_clear`), src-tauri/src/ipc/commands/projects.rs (`project_remove`), src-tauri/tests/retention.rs
Acceptance criteria:
- AC-F026-10
- AC-F029-06
Required tests:
- retention::tail_pruned_after_90_days, privacy::clear_history, privacy::remove_project_leaves_repo
Out of scope: sensor event retention (M4-09).

### [M4-03] Run log screen and metric panel
Class: SPEC-COMPLETE
Stage: 11
Depends on: M4-02, M3-26
ADRs: ADR-018
Threats mitigated: none
Likely files: src/routes/projects/[projectId]/runs/+page.svelte (run list and the metrics strip), src-tauri/src/ipc/commands/packets.rs (`runs_search`, `metrics_view`), src-tauri/src/ipc/commands/launch.rs (`chain_resume_prepare` for Resume), crates/foreman-core/benches/run_index.rs (mockup: docs/design/mockups/run-log.html)
Acceptance criteria:
- AC-F026-07
- AC-F026-08
- AC-F026-09 (the metrics strip on the Run log screen, from `metrics_view`, not a Settings page)
Required tests:
- none named (component tests; criterion bench over 100,000 entries)
Out of scope: none.

### [M4-04] Annotate blueprint v2.2: Stages 0 and 3–5; golden through Stage 5
Class: SPEC-COMPLETE
Stage: 12
Depends on: M3-31
ADRs: ADR-005
Threats mitigated: none
Likely files: claude-blueprint repo `claude-build-blueprint.md`, crates/foreman-core/tests/fixtures/blueprint-v2.2.md, crates/foreman-core/tests/snapshots/
Acceptance criteria:
- AC-F001-02
Required tests:
- template::golden_v2_2 (Stages 0–5 and the Gate)
Out of scope: Stages 6–18 (stay needs-annotation in v1).

### [M4-05] Resolver and harness through Stage 5
Class: HARD
Stage: 12
Depends on: M4-04
ADRs: none
Threats mitigated: none
Likely files: crates/foreman-core/src/resolver/tokens.rs, crates/foreman-core/tests/harness_tradelog.rs
Acceptance criteria:
- AC-F006-15
- AC-F003-04
- EX-M4-02
Required tests:
- resolver::blueprint_token_inventory (Stages 3–5)
Out of scope: none.

### [M4-06] Template health screen
Class: SPEC-COMPLETE
Stage: 12
Depends on: M4-04
ADRs: ADR-005
Threats mitigated: none
Likely files: src/routes/template-health/+page.svelte, src-tauri/src/ipc/commands/doctor.rs (`template_health`) (mockup: docs/design/mockups/template-health.html)
Acceptance criteria:
- AC-F003-05
- AC-F003-06
Required tests:
- none named (component tests)
Out of scope: none.

### [M4-07] Sensor binary
Class: HARD
Stage: 12
Depends on: M3-31, M0-13
ADRs: ADR-009
Threats mitigated: T-045, T-048
Likely files: crates/foreman-sensor/src/main.rs, crates/foreman-sensor/tests/sensor.rs, src-tauri/src/sensor_ingest.rs (install to `<app-data>/bin/foreman-sensor-<hash>`)
Acceptance criteria:
- AC-F011-03
- AC-F011-04 (including a symlinked `events/<runId>` directory, AB-12)
Required tests:
- sensor::rejects_wrong_token, sensor::stdin_cap, sensor::exits_under_10ms
Out of scope: everything sensor-related if S-1′ took its fail branch.

### [M4-08] Sensor hooks in the run settings
Class: HARD
Stage: 12
Depends on: M4-07, M2-09
ADRs: ADR-009, ADR-021
Threats mitigated: T-024, T-046, T-047, T-050
Likely files: crates/foreman-core/src/launch/run_settings.rs, src-tauri/src/launch.rs, crates/foreman-core/tests/run_settings.rs
Acceptance criteria:
- AC-F011-01
- AC-F011-02
- AC-F011-10
Required tests:
- sensor::exec_form_only, sensor::hash_checked_per_run, fs_policy::denies_claude_settings_writes (sensor setup paths)
Out of scope: none.

### [M4-09] Sensor ingest and derived session state
Class: HARD
Stage: 12
Depends on: M4-08
ADRs: ADR-009
Threats mitigated: T-031, T-045, T-049
Likely files: src-tauri/src/sensor_ingest.rs, crates/foreman-core/src/session_state.rs, src-tauri/tests/sensor_ingest.rs
Acceptance criteria:
- AC-F011-05
- AC-F011-06
- AC-F011-07
- AC-F011-08
- AC-F011-09
Required tests:
- sensor::events_only_update_state, retention::events_deleted_on_sign, retention::events_max_age
Out of scope: none.

### [M4-10] Sensor idle, background tasks and tab states
Class: HARD
Stage: 12
Depends on: M4-09, M3-15
ADRs: ADR-008, ADR-009
Threats mitigated: T-029
Likely files: src-tauri/src/rearm.rs, src/lib/components/TerminalTab.svelte
Acceptance criteria:
- AC-F010-13
- AC-F010-14
- AC-F009-14
Required tests:
- rearm::only_from_user_action (sensor events never start re-arm)
Out of scope: none.

### [M4-11] Now/Next strip, round tracker and focus mode
Class: SPEC-COMPLETE
Stage: 12
Depends on: M4-09, M3-19
ADRs: ADR-020
Threats mitigated: none
Likely files: src/lib/components/NowNextStrip.svelte, src/lib/components/RoundTracker.svelte, src/lib/keyboard/registry.ts (mockup: docs/design/mockups/workspace.html)
Acceptance criteria:
- AC-F009-11
- AC-F009-12
- AC-F009-13
Required tests:
- none named (component tests)
Out of scope: none.

### [M4-12] Contract probes
Class: HARD
Stage: 12
Depends on: M3-18, M3-05
ADRs: ADR-017
Threats mitigated: T-036, T-037
Likely files: crates/foreman-sys/src/claude.rs (`probe_*`), src-tauri/src/doctor.rs (`probe_run`), src-tauri/tests/probe.rs
Acceptance criteria:
- AC-F019-15
- AC-F019-16
- AC-F019-17
- AC-F008-21
Required tests:
- probe::runs_on_version_change, probe::runs_only_in_probe_dir, probe::dir_reset_before_run
- probe::inconclusive_keeps_state, probe::failure_disables_feature, preflight::confirm_required_for_version_change
Out of scope: none.

### [M4-13] Full Doctor
Class: HARD
Stage: 12
Depends on: M4-12
ADRs: ADR-017
Threats mitigated: T-027, T-038
Likely files: src-tauri/src/doctor.rs, src/routes/doctor/+page.svelte (mockup: docs/design/mockups/doctor.html)
Acceptance criteria:
- AC-F019-09
- AC-F019-10
- AC-F019-11
Required tests:
- spawn::helper_cwd_is_app_data (toolchain probes)
Out of scope: login PATH (M4-14).

### [M4-14] Login-shell PATH capture
Class: HARD
Stage: 12
Depends on: M4-13, M0-17
ADRs: ADR-007
Threats mitigated: T-015, T-019
Likely files: src-tauri/src/doctor.rs (`doctor_capture_login_path`, `doctor_login_path_apply`), crates/foreman-sys/src/spawn.rs (`capture_login_path`), src/routes/first-run/+page.svelte
Acceptance criteria:
- AC-F019-12
- AC-F019-13 (per S-12's branch; moves before M2-16 on the fail branch)
- AC-F019-14
Required tests:
- doctor::login_path_requires_consent
Out of scope: none.

### [M4-15] Gate model
Class: HARD
Stage: 12
Depends on: M3-31
ADRs: none
Threats mitigated: T-084
Likely files: crates/foreman-core/src/gate.rs, crates/foreman-core/tests/gate.rs
Acceptance criteria:
- AC-F015-02
- AC-F015-03
- AC-F015-08
- AC-F015-09
- AC-F015-10
Required tests:
- gate::signable_requires_all, steps::attest_note_rules, steps::gate_item_order
Out of scope: gate runs and UI (M4-16, M4-17).

### [M4-16] Gate runs, Import results and Fix this
Class: HARD
Stage: 12
Depends on: M4-15, M4-09
ADRs: ADR-008
Threats mitigated: T-031
Likely files: src-tauri/src/ipc/commands/gates.rs (`gate_view`, `gate_import_results`, `gate_import_confirm`, `gate_fix_start`), src-tauri/tests/gates.rs
Acceptance criteria:
- AC-F015-04
- AC-F015-05
- AC-F015-07
Required tests:
- sensor::events_only_update_state (import proposes, never applies)
Out of scope: none.

### [M4-17] Gate screen and item re-confirmation
Class: SPEC-COMPLETE
Stage: 12
Depends on: M4-16, M1-17
ADRs: ADR-020
Threats mitigated: T-067
Likely files: src/routes/projects/[projectId]/stages/[stage]/gate/+page.svelte, src/lib/components/InspectionRow.svelte, src-tauri/src/ipc/commands/decisions.rs (`item_kind_confirm`) (mockup: docs/design/mockups/gate.html)
Acceptance criteria:
- AC-F015-06
- AC-F001-17 (int: the kind is confirmed in a scripted native dialog, Cancel leaves attest disabled; comp: the old and new bullet text shown as context)
Required tests:
- template::swapped_bullets_reconfirm (through IPC)
- ipc::security_confirmations_are_native (`item_kind_confirm`)
Out of scope: sign dialog (M4-19).

### [M4-18] Lessons capture
Class: HARD
Stage: 12
Depends on: M4-15, M3-24
ADRs: ADR-010
Threats mitigated: none
Likely files: crates/foreman-core/src/lessons.rs, src-tauri/src/ipc/commands/lessons.rs (`lessons_status`), crates/foreman-core/tests/lessons.rs
Acceptance criteria:
- AC-F016-01
- AC-F016-02
- AC-F016-03
- AC-F016-04
- AC-F016-05
- AC-F016-06
Required tests:
- lessons::append_numbering, steps::lessons_entry_detected
Out of scope: none.

### [M4-19] Sign preview and Sign gating
Class: HARD
Stage: 12
Depends on: M4-15, M4-18
ADRs: ADR-012
Threats mitigated: T-084
Likely files: src-tauri/src/ipc/commands/signing.rs (`sign_preview`), crates/foreman-core/src/packet/sign_fields.rs, src/lib/components/SignDialog.svelte (mockup: docs/design/mockups/gate.html#sign)
Acceptance criteria:
- AC-F014-01
- AC-F014-02
- AC-F014-03
- AC-F004-13
- AC-F013-15
- AC-F005-05 (SUBSTITUTIONS part)
Required tests:
- packet::added_fields_golden, rules::skipped_queue, gate::signable_requires_all (sign dialog gating)
Out of scope: committing (M4-20).

### [M4-20] Sign apply on the current branch
Class: HARD
Stage: 12
Depends on: M4-19, M1-10, M1-28
ADRs: ADR-012
Threats mitigated: T-003, T-039, T-079, T-080, T-082
Likely files: src-tauri/src/ipc/commands/signing.rs (`sign_apply`), crates/foreman-sys/src/git/commit.rs, src-tauri/tests/sign.rs, crates/foreman-sys/tests/fixtures/lint-staged-repo/
Acceptance criteria:
- AC-F014-04
- AC-F014-05
- AC-F014-06 (including the `index.lock` retry, `indexRefreshPending` and its banner)
- AC-F014-07
- AC-F014-08
- AC-F014-10
- AC-F014-14
- EX-M4-03
Required tests:
- sign::refuses_changed_file, sign::pathspec_only, sign::readback_detects_mismatch, sign::untracked_new_packet
- sign::concurrent_ref_move_fails, ipc::security_confirmations_are_native (`sign_apply`)
- sign::index_locked_refresh_retried
- packet::outside_signature_needs_confirmation (P2: shown as unverified)
Out of scope: other-branch signing (M4-21).

### [M4-21] Sign on another branch, commit pending and git edge cases
Class: HARD
Stage: 12
Depends on: M4-20
ADRs: ADR-012
Threats mitigated: T-043, T-081, T-086
Likely files: src-tauri/src/ipc/commands/signing.rs (`sign_retry_commit`), crates/foreman-sys/src/git/commit.rs, src-tauri/tests/sign_branch.rs
Acceptance criteria:
- AC-F014-09
- AC-F014-11
- AC-F014-12
- AC-F014-13
- AC-F014-16
Required tests:
- sign::other_branch_leaves_worktree_head_and_index, git::repo_gpg_program_never_runs, git::timeout (signing case)
- decisions::other_branch_sign_no_false_drift (`committedOnOtherBranches`, ADR-022)
Out of scope: none.

### [M4-22] Tamper detection and restore
Class: HARD
Stage: 12
Depends on: M4-20, M3-19
ADRs: ADR-012, ADR-013
Threats mitigated: T-083
Likely files: src-tauri/src/watch.rs, src-tauri/src/ipc/commands/signing.rs (`tamper_restore`), src-tauri/tests/tamper.rs
Acceptance criteria:
- AC-F014-15
Required tests:
- sign::tamper_detected
Out of scope: none.

### [M4-23] Packet repair, capture and open items
Class: HARD
Stage: 12
Depends on: M4-09, M3-15, M3-24
ADRs: ADR-008
Threats mitigated: T-031
Likely files: src-tauri/src/ipc/commands/packets.rs (`packet_repair_prepare`), src-tauri/src/ipc/commands/chat.rs (`paste_back_preview { source: "sensor" }`), src/lib/components/PacketView.svelte (mockup: docs/design/mockups/packet.html)
Acceptance criteria:
- AC-F013-11 (Repair is `packet_repair_prepare`, then `rearm_start` with its native idle dialog)
- AC-F013-12
- AC-F013-13
- AC-F013-14 (Capture from session is `paste_back_preview { source: "sensor" }`, then `paste_back_apply`)
Required tests:
- rearm::only_from_user_action (Repair starts only from a click), pasteback::lists_targets_before_write (capture path)
Out of scope: none.

### [M4-24] ADR-set split and chat close-out
Class: HARD
Stage: 12
Depends on: M3-24, M4-19
ADRs: none
Threats mitigated: T-090
Likely files: crates/foreman-core/src/packet/split.rs (ADR split), src-tauri/src/ipc/commands/chat.rs, src/lib/components/CloseOut.svelte
Acceptance criteria:
- AC-F012-15
- AC-F012-16
Required tests:
- pasteback::lists_targets_before_write (ADR targets), steps::closeout_sequences (chat order)
Out of scope: none.

### [M4-25] Today inbox and notifications
Class: SPEC-COMPLETE
Stage: 12
Depends on: M4-22, M4-10, M3-07
ADRs: ADR-014
Threats mitigated: T-098
Likely files: src/routes/+page.svelte, src/lib/stores/today.svelte.ts, src-tauri/src/os.rs (inbox notification templates, Dock badge), src-tauri/src/ipc/commands/projects.rs (`project_pin_set`) (mockup: docs/design/mockups/today.html)
Acceptance criteria:
- AC-F017-07
- AC-F017-08
- AC-F017-09
- AC-F017-10 (pins set with `project_pin_set`; ⌘1–⌘9 open the pinned slots)
Required tests:
- notify::body_template_snapshot (inbox kinds)
Out of scope: none.

### [M4-26] Setup wizard
Class: SPEC-COMPLETE
Stage: 12
Depends on: M3-31
ADRs: ADR-020
Threats mitigated: none
Likely files: src/routes/setup/wizard/+page.svelte (mockup: docs/design/mockups/wizard.html)
Acceptance criteria:
- AC-F018-08
Required tests:
- none named (component and flow tests)
Out of scope: none.

### [M4-27] Knowledge manifest
Class: SPEC-COMPLETE
Stage: 12
Depends on: M3-31
ADRs: ADR-020
Threats mitigated: none
Likely files: crates/foreman-core/src/manifest.rs, src-tauri/src/ipc/commands/manifest.rs (`manifest_view`, `manifest_mark`), src/lib/components/KnowledgeManifest.svelte (mockup: docs/design/mockups/workspace-chat.html "Knowledge for this chat")
Acceptance criteria:
- AC-F023-01
- AC-F023-02
Required tests:
- none named (component tests)
Out of scope: the assembler (M4-28).

### [M4-28] Build-package assembler and root `CLAUDE.md`
Class: HARD
Stage: 12
Depends on: M4-27, M3-24
ADRs: ADR-011, ADR-014
Threats mitigated: T-003, T-051, T-052, T-090
Likely files: crates/foreman-core/src/assembler.rs, crates/foreman-core/src/rules/evidence.rs (DR-063), src-tauri/src/ipc/commands/assembler.rs (`assembler_view`, `assembler_paste_preview`, `assembler_paste_apply`), src/lib/components/Assembler.svelte, crates/foreman-core/tests/assembler.rs (component: docs/design/components.md "Build-package assembler"; mockup: docs/design/mockups/assembler.html)
Acceptance criteria:
- AC-F023-03
- AC-F023-04
- AC-F023-05 (rows Foreman may write; a refused row doesn't block)
- AC-F023-06 (proposals from path evidence per DR-063; no evidence → `undecided`, never `no`; each proposal takes effect only when Billy confirms it)
- AC-F021-06 (int: the root `CLAUDE.md` write waits for the native confirmation, Cancel writes nothing; comp: the panel shows the diff as context)
Required tests:
- assembler::rejects_policy_violations, ipc::security_confirmations_are_native (root `CLAUDE.md` write)
- rules::late_condition_evidence
Out of scope: none. (Was NEEDS-DESIGN; the Stage 4.7 fix pass added the component entry, the mockup and DR-063's evidence rules.)

### [M4-29] Desktop handoff
Class: HARD
Stage: 12
Depends on: M3-14, M3-04, M0-18
ADRs: ADR-008, ADR-017
Threats mitigated: T-030
Likely files: crates/foreman-sys/src/claude.rs (`desktop`), src-tauri/src/ipc/commands/desktop.rs (`desktop_open`, `desktop_handoff`), src-tauri/tests/desktop.rs
Acceptance criteria:
- AC-F024-01
- AC-F024-02
- AC-F024-03 (S-23's branch)
- AC-F024-04
- AC-F010-15 (S-8's branch for fork IDs)
Required tests:
- chain::fork_argv (handed-off chain)
Out of scope: none.

### [M4-30] Overlay additions: rules and validation
Class: HARD
Stage: 12
Depends on: M3-31
ADRs: ADR-022
Threats mitigated: T-016
Likely files: crates/foreman-core/src/rules/overlay.rs, crates/foreman-core/tests/overlay.rs
Acceptance criteria:
- AC-F025-02
- AC-F025-04
- AC-F025-05
Required tests:
- rules::overlay_add_only, rules::added_step_rules, rules::added_item_attest_only, rules::added_step_validated, rules::overlay_roundtrip
- template::rejects_bypass_permissions (overlay case)
Out of scope: UI (M4-31).

### [M4-31] Add a prompt or inspection item
Class: SPEC-COMPLETE
Stage: 12
Depends on: M4-30
ADRs: ADR-020
Threats mitigated: none
Likely files: src-tauri/src/ipc/commands/desktop.rs (`add_step`, `add_item`), src/lib/components/InsertPoint.svelte
Acceptance criteria:
- AC-F025-01
- AC-F025-03
Required tests:
- none named (component tests)
Out of scope: none.

### [M4-32] Run sheet export
Class: HARD
Stage: 12
Depends on: M3-31
ADRs: ADR-018
Threats mitigated: none
Likely files: crates/foreman-core/src/runsheet.rs, src-tauri/src/ipc/commands/desktop.rs (`run_sheet_export`), crates/foreman-core/tests/runsheet.rs
Acceptance criteria:
- AC-F027-01
- AC-F027-02
- AC-F027-03
Required tests:
- none named (golden `runsheet::stage_2_golden`)
Out of scope: none.

### [M4-33] Command palette and remaining shortcuts
Class: SPEC-COMPLETE
Stage: 12
Depends on: M3-31
ADRs: ADR-020
Threats mitigated: none
Likely files: src/lib/components/CommandPalette.svelte, src/lib/keyboard/registry.ts (mockup: docs/design/mockups/palette.html)
Acceptance criteria:
- AC-F028-04
- AC-F028-05
Required tests:
- none named (component tests)
Out of scope: none.

### [M4-34] Settings (P2)
Class: HARD
Stage: 12
Depends on: M4-14, M4-02
ADRs: ADR-007
Threats mitigated: T-004, T-018
Likely files: src/routes/settings/+page.svelte, src-tauri/src/ipc/commands/settings.rs, crates/foreman-core/src/settings.rs (mockup: docs/design/mockups/settings.html)
Acceptance criteria:
- AC-F029-04
- AC-F029-05
Required tests:
- env::denylist_beats_passthrough (setting path), settings::security_fields_require_native_confirm (pass-through list)
Out of scope: none.

### [M4-35] Stage 11–12 inspections and M4 exit
Class: SPEC-COMPLETE
Stage: 12
Depends on: M4-03, M4-05, M4-06, M4-10, M4-11, M4-13, M4-17, M4-21, M4-23, M4-24, M4-25, M4-26, M4-28, M4-29, M4-31, M4-32, M4-33, M4-34
ADRs: ADR-015
Threats mitigated: none
Likely files: docs/handoff/stage-11.md, docs/handoff/stage-12.md
Acceptance criteria:
- EX-M4-01
- EX-M4-04
- EX-M4-05
- Given Foreman's own Stages 11 and 12, then they close out by hand (attest, lessons, APPROVED BY, commit), as Stages 5–10 did; running Foreman's own build inside Foreman is v1.1 (adoption)
Required tests:
- none named (forced-failure checks per runner; per-screen state review)
Out of scope: none.

---

## M5 · Finishes (Stages 13–17)

### [M5-01] Token lint and contrast test
Class: SPEC-COMPLETE
Stage: 13
Depends on: M4-35
ADRs: ADR-020
Threats mitigated: none
Likely files: src/lib/styles/tokens.css, eslint.config.js / stylelint config (no hard-coded values), src/lib/styles/contrast.test.ts
Acceptance criteria:
- AC-XC-24
- AC-XC-26 (lint part)
Required tests:
- none named (token contrast test in Vitest)
Out of scope: none.

### [M5-02] Sheet strip
Class: SPEC-COMPLETE
Stage: 13
Depends on: M5-01
ADRs: ADR-020
Threats mitigated: none
Likely files: src/lib/components/SheetStrip.svelte, src/routes/projects/[projectId]/+layout.svelte
Acceptance criteria:
- AC-F030-06
Required tests:
- none named (component tests)
Out of scope: none.

### [M5-03] Title block, inspector and responsive layout
Class: SPEC-COMPLETE
Stage: 13
Depends on: M5-02
ADRs: ADR-020
Threats mitigated: none
Likely files: src/lib/components/TitleBlock.svelte, src/lib/components/Inspector.svelte, src/routes/projects/[projectId]/+layout.svelte
Acceptance criteria:
- AC-F030-07
- AC-F030-08
- AC-F030-10
Required tests:
- none named (component tests)
Out of scope: none.

### [M5-04] Stamp and system display settings
Class: SPEC-COMPLETE
Stage: 13
Depends on: M5-03
ADRs: ADR-020
Threats mitigated: none
Likely files: src/lib/components/Stamp.svelte, src/lib/styles/motion.css, src/lib/styles/tokens.css (`prefers-contrast: more` set), docs/design/contrast-check.py (7:1 pairs), src/lib/os-display.ts (media queries reported through `settings_update.osDisplay`), src-tauri/src/window.rs (vibrancy off)
Acceptance criteria:
- AC-F030-09
- AC-XC-23 (Reduce Transparency and Increase Contrast detected with `prefers-reduced-transparency` and `prefers-contrast: more` in the webview and reported to Rust, which turns window effects off; the high-contrast tokens reach ≥ 7:1 for every pair)
Required tests:
- none named (component test; manual pass)
Out of scope: none.

### [M5-05] Accessibility: axe and shape-plus-color
Class: SPEC-COMPLETE
Stage: 13
Depends on: M5-04
ADRs: ADR-015, ADR-020
Threats mitigated: T-007
Likely files: tests/flows/a11y.spec.ts, src/lib/components/StatusGlyph.svelte
Acceptance criteria:
- AC-XC-20
- AC-XC-25
Required tests:
- render::escapes_html_in_packet (SR-26 rendering re-check)
Out of scope: none.

### [M5-06] Keyboard-only workflow and VoiceOver pass
Class: SPEC-COMPLETE
Stage: 13
Depends on: M5-05
ADRs: ADR-015
Threats mitigated: none
Likely files: tests/e2e/keyboard-full.e2e.ts, docs/handoff/stage-13.md
Acceptance criteria:
- AC-XC-21
- AC-XC-22
Required tests:
- none named
Out of scope: none.

### [M5-07] Mockup parity in both themes
Class: SPEC-COMPLETE
Stage: 13
Depends on: M5-06
ADRs: ADR-020
Threats mitigated: none
Likely files: tests/flows/visual.spec.ts, docs/design/screenshots/
Acceptance criteria:
- AC-XC-26
- EX-M5-01
- EX-M5-02
Required tests:
- none named
Out of scope: none.

### [M5-08] Stage 14 security review
Class: SPEC-COMPLETE
Stage: 14
Depends on: M5-07
ADRs: ADR-016, ADR-014
Threats mitigated: T-001, T-101, T-106
Likely files: docs/security/stage-14-review.md
Acceptance criteria:
- EX-M5-03 (security part: every SR re-verified, ASVS 5.0 L2 applicable chapters, zero open CRITICAL or HIGH)
Required tests:
- network::blocked_network_run (release build), release::no_webdriver
Out of scope: none.

### [M5-09] Stage 14 performance and architecture audits
Class: SPEC-COMPLETE
Stage: 14
Depends on: M5-08
ADRs: ADR-003, ADR-015
Threats mitigated: none
Likely files: docs/audits/stage-14-performance.md, docs/audits/stage-14-architecture.md, crates/foreman-core/benches/
Acceptance criteria:
- EX-M5-03 (performance and architecture part: every NFR met in the release build, including NFR-01, NFR-02, NFR-19, NFR-20, NFR-22, NFR-23, NFR-25)
Required tests:
- none named
Out of scope: none.

### [M5-10] Stage 15 first run and copy review
Class: SPEC-COMPLETE
Stage: 15
Depends on: M5-09
ADRs: none
Threats mitigated: none
Likely files: src/lib/copy/, docs/handoff/stage-15.md
Acceptance criteria:
- EX-M5-04
Required tests:
- none named
Out of scope: none.

### [M5-11] Stage 16 readiness and drills
Class: SPEC-COMPLETE
Stage: 16
Depends on: M5-10
ADRs: ADR-010, ADR-001
Threats mitigated: T-059
Likely files: docs/drills.md, docs/readiness.md, docs/runbooks/release.md
Acceptance criteria:
- EX-M5-05
Required tests:
- none named (drills recorded in `docs/drills.md`)
Out of scope: none.

### [M5-12] TradeLog Stages 3–5 through Foreman
Class: SPEC-COMPLETE
Stage: 16
Depends on: M5-11
ADRs: none
Threats mitigated: none
Likely files: TradeLog repo `docs/handoff/stage-03.md` … `stage-05.md`
Acceptance criteria:
- EX-M5-06
Required tests:
- none named (manual run)
Out of scope: none.

### [M5-13] Success metrics
Class: SPEC-COMPLETE
Stage: 17
Depends on: M5-12
ADRs: ADR-018
Threats mitigated: none
Likely files: docs/handoff/stage-17.md
Acceptance criteria:
- AC-SM-01
- AC-SM-02
- AC-SM-03
- AC-SM-04
- EX-M5-07
Required tests:
- none named
Out of scope: none.

### [M5-14] Certificate of Occupancy
Class: SPEC-COMPLETE
Stage: 17
Depends on: M5-13
ADRs: ADR-001
Threats mitigated: T-106
Likely files: docs/handoff/stage-17.md, docs/adr/, docs/runbooks/release.md
Acceptance criteria:
- EX-M5-08
Required tests:
- release::no_webdriver (v1.0.0 build)
Out of scope: v1.1.

---

## Deferred to v1.1

No tickets. Listed so the coverage check can account for them.

| Item | What | Its tests (not built in v1) |
|---|---|---|
| SR-11 | Verify commands as approved objects | verify::lint_rejects_*, verify::script_change_invalidates_approval |
| SR-12 | Running approved verify commands; `auto` items | gate::auto_pass_requires_exit_zero |
| F-101 | Headless Gate A (after S-2′), `auto` items, verify commands | (SR-11, SR-12 tests above); threat T-107 |
| F-102 | Crew Loop on CLI worktrees | — |
| F-103 | Change Request track | — |
| F-104 | Full overlay authoring, Add agent, Add note, promote to blueprint, pin upgrade | — |
| F-105 | Usage meter | — |
| F-106 | Live send and "go" | — |
| F-107 | Lessons view, retro starter | — |
| F-108 | Project metrics dashboard | — |
| — | Adopt an existing project | — |
## Coverage

Generated by a stdlib Python check (kept outside the repo; re-run in the Stage 4.7 fix pass) from this file, `docs/scope.md`, `docs/requirements.md`, `docs/threat-model.md`, `docs/domain-rules.md` and `docs/adr/README.md`. The script also checks the ticket format (all nine fields in order, one class, stage 5–17) and that every dependency appears earlier in the file.

**Tickets by milestone and class**

| Milestone | HARD | SPEC-COMPLETE | MECHANICAL | NEEDS-DESIGN | Total |
|---|---|---|---|---|---|
| M0 | 12 | 1 | 6 | 0 | 19 |
| M1 | 30 | 3 | 0 | 0 | 33 |
| M2 | 17 | 11 | 0 | 0 | 28 |
| M3 | 25 | 5 | 0 | 0 | 30 |
| M4 | 25 | 11 | 0 | 0 | 36 |
| M5 | 0 | 14 | 0 | 0 | 14 |
| **All** | 109 | 45 | 6 | 0 | **160** |

**Coverage**

| Item | In source | Covered by a ticket | Orphans |
|---|---|---|---|
| Acceptance criteria (`AC-…`, scope.md) | 380 | 380 | none |
| Exit criteria (`EX-…`, scope.md) | 33 | 33 | none |
| Named tests (requirements, threat model, domain rules), v1 | 273 | 273 | none |
| Named tests deferred to v1.1 | 3 | listed under "Deferred to v1.1" | none |
| ADRs | 24 | 24 | none |
| Threats T-001…T-107 | 107 | 106 (T-107 is v1.1) | none |

Format and ordering problems: none. IDs cited that don't exist in the sources: none. With the Stage 4.7 changes to `docs/requirements.md` (made on `stage/03-permits` and merged here) the same check counts 276 named v1 tests, all covered.

Not counted as tests: module and API names the documents mention in backticks (`template::grammar`, `app::rearm`, `app::watch`, `git::run`, `launch::prepare`, `doctor::capture_login_path`, `core::sanitize`, `std::fs`, `std::process::Command::new`, `portable_pty::CommandBuilder::new`).

Longest dependency chain to the kill check (M3-31): 32 tickets. To Certificate of Occupancy (M5-14): 53 tickets.

---

## Critical path

The longest dependency chain (unit weight per ticket), in five legs:

1. **Stage 5 gates:** M0-01 → M0-02 → M0-03 → M0-07 → M0-10 → M0-11 → M0-19. The seven spikes (M0-12 S-0 first, then M0-13 to M0-18) run alongside once M0-09 and M0-10 exist; a fundamental S-0 failure stops here and reopens the shell choice.
2. **Parser to harness:** M1-02 → M1-11 → M1-12 → M1-13 → M1-14 → M1-16 → M1-21 → M1-22 → M1-30 → M1-33. The store (M1-03 to M1-05) and spawn/git (M1-07, M1-08) are off the path but must land before M2.
3. **Skeleton to launch preview:** M2-01 → M2-02 → M2-04 → M2-05 → M2-21 → M2-25.
4. **Runners to the kill check:** M3-02 → M3-05 → M3-10 → M3-13 → M3-15 → M3-16 → M3-29 → M3-30 → **M3-31 (kill check)**. 32 tickets end to end.
5. **After the kill check:** M4-15 → M4-18 → M4-19 → M4-20 → M4-22 → M4-25 → M4-35, then M5-01 → … → M5-14 (53 tickets end to end).

By count, 110 of 160 tickets (69%) come before the kill check (M3-18 moved to M4 in the Stage 4.7 fix pass). At 1–4 hours each that is about 110–440 hours of Billy's time before the kill check, and 160–640 hours for v1. The Master Spec §9 estimate is restated in those terms (no "about half" claim).
