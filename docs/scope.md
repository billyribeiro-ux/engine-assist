# Foreman · Scope, milestones and acceptance criteria

Stage 4.1 · Status: DRAFT for the Stage 4 sign · 2026-10-19
Inputs: `docs/handoff/stage-02.md`, `docs/MASTER-SPEC.md` (frozen v1.0), `spec/council/r6-feature-specs.md` (part of the frozen spec), `docs/requirements.md`, `spec/council/r8-nfr.md`.

This file says what v1 contains, the order it's built in, and how each part is proven. Stage 4.6 tickets reference the acceptance-criterion IDs here (`AC-F008-06`, `AC-M0-03`, …). Behavior comes from r6 and the Master Spec. Where they're silent and a criterion needed a decision, the conservative option was taken and listed in §7.

The frontend is **SvelteKit 3 in SPA mode**: file-based routes, `adapter-static`, `ssr = false`, and shared code under `#lib` (SvelteKit 3 replaces the generated `$lib` alias with a `#lib` Node subpath import declared in `package.json`; [migration guide](https://svelte.dev/docs/kit/migrating-to-sveltekit-3)), as the Master Spec §10 records. Where `docs/requirements.md` is more specific than r6 or the Master Spec (signing by plumbing, run-settings deny rules, the fingerprint set, confirmation reasons, native confirmations, P1 notifications), this file follows `requirements.md`.

---

## 0. How to read the criteria

Each criterion is written **Given** (state) · **when** (action or event) · **then** (observable result). It names one test level and the milestone where it must first pass. A criterion stays green in every later milestone.

| Level | Meaning |
|---|---|
| `unit` | Rust or TypeScript unit test, including golden snapshots (insta) and table-driven cases |
| `prop` | Property test (proptest), including crash-injection and fuzzing |
| `int` | Rust integration test on temp-dir fixtures (repos, git, file system). Wherever Claude Code is spawned, it uses the **fake `claude` binary**, which records argv, environment and cwd, and plays scripted output, exit codes, `-v` and `auth status` replies |
| `comp` | Component test (Vitest browser mode or Playwright + axe) against the **mock IPC** implementation, which shares one contract with the real one |
| `e2e` | Smoke test of the built app (@wdio/tauri-service, behind the `e2e` feature) |
| `bench` | criterion benchmark or timed local performance test |
| `manual` | Manual check on Billy's Mac, with the result recorded in that stage's gate |
| `ci` | A CI or `pnpm verify` job that must fail on a planted violation |

Milestones: **M0**–**M5** (§2). "P1" and "P2" are the phases of Master Spec §9: P1 ends at the kill check (end of M3); P2 is the rest of v1.

---

## 1. v1 scope

### 1.1 v1 features

| ID | Feature | Phase | Milestone |
|---|---|---|---|
| F-001 | Template engine (annotations only, prose lint, item IDs + hashes, grammar v1, `fm:if`) | P1: Stages 1–2 and F.2 · P2: Stages 0–5 and the Stage Gate | M1 · M4 |
| F-002 | Blueprint lock | P1 | M1 |
| F-003 | TradeLog acceptance harness (P1, Stages 1–2) + Template health screen and harness through Stage 5 (P2) | P1 + P2 | M1 (argv snapshots in M3) · M4 |
| F-004 | Rule engine: tags, tri-state conditions, applicable path | P1 | M1 (UI in M2) |
| F-005 | Run configs + model policy | P1 | M1 (UI in M2) |
| F-006 | Prompt resolver | P1: tokens of Stages 1–2, Gate, F.2 · P2: through Stage 5 | M1 · M4 |
| F-007 | Step kinds and states | P1 | M1 (UI in M2, live behavior in M3) |
| F-008 | Launch pipeline (`prepare_launch`) | P1 · P2: contract probes | M2 (spawn controls: argv, environment, run settings, preview, run record) · M3 · M4 |
| F-009 | Terminal runner | P1: tabs, PTY, key routing, archive · P2: Now/Next, round tracker, focus mode | M3 · M4 |
| F-010 | Session chains and re-arm | P1 (manual idle) · P2: sensor idle, Desktop fork | M3 · M4 |
| F-011 | Session sensor | P2 | M4 |
| F-012 | Chat bridge | P1: copy and open, chips, links, paste-back, normalization, attachments, fix request · P2: ADR split, chat close-out | M3 · M4 |
| F-013 | Packet engine | P1: validation (M1), watch and normalize (M3) · P2: repair, capture | M1 · M3 · M4 |
| F-014 | Signing and provenance | P2 (P1: Billy signs by hand) | M4 |
| F-015 | Gates | P2 (P1: close-out checklist step, by hand) | M3 · M4 |
| F-016 | Lessons capture | P2 (P1: by hand) | M4 |
| F-017 | Today and Do next | P1: Next step + ⌘⇧↵ · P2: inbox | M2 · M4 |
| F-018 | Projects and setup | P1: setup form · P2: wizard | M2 · M4 |
| F-019 | First run, Doctor, contract probes | P1: first run, version, auth, git · P2: probe-folder trust, full Doctor, probes | M2 · M4 |
| F-020 | State, persistence, resilience | P1 | M1 (run records and restart in M3) |
| F-021 | Path policy | P1 | M1 |
| F-022 | Outside-change review for decision files | P1 | M1 (review UI in M2, live detection in M3) |
| F-023 | Knowledge manifest + build-package assembler | P2 | M4 |
| F-024 | Desktop handoff | P2 | M4 |
| F-025 | Add prompt / inspection item, this project only | P2 | M4 |
| F-026 | Run log + Export metrics (P1) · run log screen + metric panel (P2) | P1 + P2 | M2 (run-record redaction) · M3 · M4 |
| F-027 | Run sheet export | P2 | M4 |
| F-028 | Core shortcuts (P1) · command palette and remaining shortcuts (P2) | P1 + P2 | M2 · M4 |
| F-029 | Settings: `claude` and `git` paths, DRI name (P1) · the rest (P2) | P1 + P2 | M2 · M4 |
| F-030 | Stage workspace + terminal drawer (P1) · sheet strip, title block, inspector, stamp (P2) | P1 + P2 | M2 · M5 |

Cross-cutting work outside the feature list: design-system application, accessibility (AR-01), audits, readiness, drills and release (M5), and the M0 build and CI requirements.

### 1.2 Non-goals (v1)

From Master Spec §13: cloud sync · other users or teams · payments · Windows or Linux · Intel Macs · App Store or notarized distribution · calling the Claude API or using API keys · editing code inside Foreman · replacing Claude Code's UI · editing the blueprint (Foreman proposes; Billy applies) · Fable in Foreman's own build.

Also out of v1: any `claude -p` call (SR-16); `auto` gate items and verify commands; adopting an existing project (the `adopted` state is reserved); editing a prompt for one project; upgrading a project's blueprint pin after Stage 1 is signed; the `blueprint` and `worktree` cwd values. Rejected options are in r6 §6.

### 1.3 v1.1 (after Foreman's own Certificate of Occupancy)

| ID | Feature |
|---|---|
| F-101 | Headless Gate A (hardened argv, after spike S-2′), `auto` items and approved verify commands (SR-11, SR-12) |
| F-102 | Crew Loop on CLI worktrees |
| F-103 | Change Request track |
| F-104 | Full overlay authoring, Add agent, Add note, promote to blueprint, pin upgrade |
| F-105 | Usage meter |
| F-106 | Live send and "go" |
| F-107 | Lessons view, retro starter |
| F-108 | Project metrics dashboard |
| — | Adopt an existing project |

v1.1 metric and kill criterion: Master Spec §9.

---

## 2. Milestones and exit criteria

A milestone is done when every exit criterion below is demonstrably true, every acceptance criterion assigned to that milestone passes, `pnpm verify` is green on main, and the matching Foreman stage's gate is signed by Billy.

### M0 · Site prep (blueprint Stage 5)

Scaffold, quality gates, CI, release build, and the seven spikes. No feature code.

| ID | Exit criterion | Test |
|---|---|---|
| AC-M0-01 | **Given** a fresh clone **when** Billy runs `pnpm install` then `pnpm tauri dev` **then** the app opens with no manual steps | manual |
| AC-M0-02 | **Given** the scaffold **then** it contains the Tauri 2 app crate, a separate `foreman-core` crate whose `cargo tree` contains no `tauri*` crate, a SvelteKit 3 frontend in SPA mode (`adapter-static`, `ssr = false`, file-based routes, `#lib` alias) and the typed IPC layer with its mock implementation | ci |
| AC-M0-03 | **Given** `pnpm verify` **when** it runs **then** it runs svelte-check, ESLint, Prettier check, TypeScript strict (`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), Vitest, rustfmt check, clippy `-D warnings` (pedantic on `foreman-core`), cargo test, `cargo audit`, `cargo deny`, `pnpm audit`, a secret scan and a license check, and fails on any problem | ci |
| AC-M0-04 | **Given** a planted type error, a planted fake secret, a clippy warning, and a known-vulnerable dependency, each in turn **when** `pnpm verify` and CI run **then** each one fails them (then the plant is reverted) | ci |
| AC-M0-05 | **Given** CI **then** it runs Semgrep (Rust and TypeScript rule packs), a gitleaks secret scan, the full test suite and a CycloneDX SBOM per build (cargo and pnpm) | ci |
| AC-M0-06 | **Given** `cargo deny` **then** its bans list rejects HTTP clients (reqwest, hyper client, ureq, isahc, surf) and the keychain crates `security-framework` and `keyring`, sources are crates.io only, and licenses are checked; lockfiles are committed and direct `foreman-core` crates are pinned to exact versions | ci |
| AC-M0-17 | **Given** the JS toolchain and CI **then** `package.json` uses exact versions, every install uses `--frozen-lockfile`, dependency build scripts are denied by default through `allowBuilds` in `pnpm-workspace.yaml` (only the packages listed there may run them; `strictDepBuilds: true` makes an unapproved one fail the install), a 7-day minimum release age applies (`minimumReleaseAge: 10080`, strict: every pinned JS version must be at least 7 days old on M0 day, or install fails), there are no git or tarball dependencies, and GitHub Actions are pinned by commit SHA with `permissions: contents: read` (CI jobs `pnpm-policy`, `actions-pinned`). **When** a planted dependency with a `postinstall` script is added **then** install fails in CI (then the plant is reverted) | ci |
| AC-M0-07 | **Given** Foreman's crates **then** they carry `#![forbid(unsafe_code)]` and `unwrap`/`expect` are denied outside tests | ci |
| AC-M0-08 | **Given** the Tauri config **then** a snapshot test pins the production CSP (`default-src 'self'; script-src 'self'; style-src 'self'` plus Tauri's compile-time hashes, never `'unsafe-inline'` or `'unsafe-eval'`; `img-src 'self' data:`; `font-src 'self'`; `connect-src ipc: http://ipc.localhost`; `object-src 'none'`; `base-uri 'none'`; `form-action 'none'`; `frame-ancestors 'none'`), SvelteKit's `kit.csp` unset, a separate `devCsp` allowing only the Vite dev server, `dangerousDisableAssetCspModification` off, `freezePrototype: true`, `withGlobalTauri: false`, `assetProtocol.enable: false`, `capabilities: ["main-window"]` listed explicitly, `dragDropEnabled: false`, the devtools feature off in release builds, the `brownfield` IPC pattern, and no remote URL in any capability. `style-src 'unsafe-inline'` appears only if S-0's fail branch was taken and the ADR records it | unit |
| AC-M0-09 | **Given** the capability files **then** a snapshot fails on any permission beyond `core:event:allow-listen`, `core:event:allow-unlisten`, `core:window:allow-start-dragging`, `core:window:allow-internal-toggle-maximize` and Foreman's own commands; every app command is declared through `AppManifest::commands` and granted to the main window only | unit |
| AC-M0-10 | **Given** a `sandbox-exec` deny-network profile **when** a full harness run and the app's first-run path run under it **then** everything passes and Foreman makes no request of its own | int |
| AC-M0-11 | **Given** `pnpm release` **when** it runs **then** it runs `pnpm verify`, builds an ad-hoc signed Apple Silicon `Foreman.app` reproducibly from the lockfiles and `rust-toolchain.toml`, and copies it to `/Applications` only after Billy confirms; the runbook's rollback to `Foreman-previous.app` works | manual |
| AC-M0-12 | **Given** `pnpm dev:mock` **when** it runs **then** the frontend opens in the Desktop Browser pane in mock-IPC mode | manual |
| AC-M0-13 | **Given** each of S-0, S-1′, S-3, S-7′, S-8, S-12, S-23 **when** it has run on Billy's Mac **then** `docs/spikes/<id>.md` records the result, the branch taken under its Master Spec §9 decision rule, and every document that branch changes (the ADR, or the affected ACs in this file) | manual |
| AC-M0-14 | **Given** S-0 **then** its report covers Shift+Tab, a 5 KB paste, resize while streaming, ⌘K routing, ⌘C/⌘V in the terminal, VoiceOver, memory with 3 tabs after 30 minutes, and the strict CSP with Bits UI, Svelte transitions, xterm and the SvelteKit bootstrap script. A fundamental TUI failure stops M1 and reopens the shell choice | manual |
| AC-M0-15 | **Given** Foreman's own Stage 5 **when** Billy runs its steps by hand **then** each is logged in `docs/metrics-baseline.md` (the CLI by-hand baseline for the kill check) | manual |
| AC-M0-16 | **Given** the ADR reality check (5.3) **then** no ADR is left CONTRADICTED | manual |

**Spike decision rules** (Master Spec §9; the branch taken is applied to the ACs named):

| Spike | Pass | Fail | ACs affected |
|---|---|---|---|
| S-0 | xterm 6 + portable-pty as specified | Scrollback cap and archive; key-routing layer; `--ax-screen-reader`; memory NFR raised to measured + 20%; CSP `style-src 'unsafe-inline'` only (scripts stay strict) as an ADR-recorded accepted risk; a fundamental TUI failure reopens the shell (SwiftUI + SwiftTerm) | AC-F009-06, AC-F009-08, AC-M0-08, NFR-23 |
| S-1′ | Sensor ships in P2 | No sensor; manual idle confirmation stays | AC-F011-*, AC-F010-14 |
| S-3 | `--effort` on resume | `effortLevel` via `--settings`; else Billy types `/effort`, warned it saves as his default | AC-F010-10 |
| S-7′ | Fingerprint and block rules suffice | Also block any project `env` block until Billy approves it | AC-F008-07 |
| S-8 | Re-arm as specified; fork IDs chosen by Foreman | Graceful end: Billy ends the session, then Foreman relaunches. Fork IDs: the only new transcript after launch; if ambiguous, a fresh chain with context | AC-F010-04, AC-F010-11, AC-F010-15 |
| S-12 | PATH as specified | Login-PATH capture mandatory at first run (moves AC-F019-13 into M2) | AC-F019-13, AC-F019-14 |
| S-23 | Shared: the chain is handed off, the next Foreman step forks | Copied: no fork needed; the CLI chain continues | AC-F024-03 |

### M1 · Domain core (blueprint Stage 6)

`foreman-core` with no Tauri dependency: F-001 (P1 scope), F-002, F-003 (P1 harness), F-004, F-005, F-006 (P1), F-007, F-013 (P1 validation), F-020, F-021, F-022.

The blueprint v2.2 annotations for Stages 1–2, the Stage Gate and F.2 (approved as PROPOSAL B-1 at the Stage 2 sign) are written by M1's first ticket (M1-01), whose exit item is that they're committed in the `claude-blueprint` repo and parse cleanly; they aren't an entry condition.

| ID | Exit criterion |
|---|---|
| EX-M1-01 | Every M1 criterion of F-001, F-002, F-003, F-004, F-005, F-006, F-007, F-013, F-020, F-021, F-022 and AC-XC-02, AC-XC-04, AC-XC-09, AC-XC-10, AC-XC-11, AC-XC-14, AC-XC-15, AC-XC-30 passes, including the single `git::run` entry point and the read and write path policies. |
| EX-M1-02 | The real v2.2 blueprint parses with zero errors; the golden snapshot of Stages 1–2 and F.2 matches (AC-F001-01). |
| EX-M1-03 | The TradeLog harness for Stages 1–2 is part of `cargo test`, blocks merge, and shows zero unresolved tokens (AC-F003-01, AC-F003-03). |
| EX-M1-04 | Every tagging, condition, packet, resolution and path-policy rule has passing unit tests, with property tests for the grammar, the applicable path, the resolver, atomic writes and the path policy (NFR-47). |
| EX-M1-05 | NFR-03, NFR-04 (resolution part), NFR-10, NFR-12 and NFR-24 benchmarks and fault-injection tests pass. |
| EX-M1-06 | `code-reviewer` reports zero CRITICAL findings on `foreman-core`. |

### M2 · Walking skeleton (blueprint Stage 7)

The Tauri shell with the SvelteKit 3 SPA: one route per P1 screen under the root layout (Today, project setup, project and stage workspace, first run and Doctor, Settings), the typed IPC client in `#lib`, the mock IPC implementation sharing one contract with the real one, and every P1 IPC command wired to the real `foreman-core`. Features: F-030 (P1), F-018 (P1), F-019 (P1), F-029 (P1), F-017 (P1), F-028 (P1), plus the M2 criteria of F-004, F-005, F-006, F-007, F-020 and F-022.

M2 contains Foreman's first process spawns (`claude -v`, `claude auth status`, `git`), so per `docs/requirements.md` (Stage 7) the spawn controls are built here. These are the one `spawn` module, the allowlist, the argv, environment and run-settings builders, prompt checks, the preview and confirmations, the run record before spawn, and child-process lifecycle (SR-01–SR-05, SR-09, SR-16, SR-19, SR-52, SR-57). They are proven against the fake `claude` binary. Launching a step in the PTY follows in M3.

| ID | Exit criterion |
|---|---|
| EX-M2-01 | Every P1 screen is reachable as a route and shows real data from a real project folder, a real blueprint and a real overlay. |
| EX-M2-02 | The frontend talks to Rust only through the typed IPC layer in `#lib`; the mock and real implementations share one contract and pass the same contract tests. |
| EX-M2-03 | One component test per screen loads it and completes its primary action against the mock IPC; one e2e smoke test starts the built app and reaches Today. |
| EX-M2-04 | First run completes on Billy's Mac and creates a project with the setup form (AC-F019-01, AC-F018-03). |
| EX-M2-05 | Every M2 criterion and AC-XC-01, AC-XC-03, AC-XC-05, AC-XC-06, AC-XC-27, AC-XC-28, AC-XC-31, AC-XC-32 passes. |
| EX-M2-06 | No code path can spawn a shell (outside the consented login-PATH capture), pass a flag outside the fixed list, or run `claude -p`; every spawn goes through `spawn` with an argv array and a cleared environment (AC-XC-01, AC-XC-03, AC-F008-14, AC-F008-15, AC-XC-31). |

### M3 · Runners (blueprint Stages 8–9) · end of phase 1 · kill check

F-008 (P1), F-009 (P1), F-010, F-012 (P1), F-013 watcher and normalization, F-026 (P1), plus the M3 criteria of F-015 (close-out checklist), F-020 (run records, restart) and F-022 (live detection). Gates, lessons and signing stay by hand.

| ID | Exit criterion |
|---|---|
| EX-M3-01 | Every M3 criterion and AC-XC-07, AC-XC-08, AC-XC-16, AC-XC-17, AC-XC-19, AC-XC-29 passes. |
| EX-M3-02 | From a real project, Foreman launches Claude Code in the embedded terminal with exactly the argv shown on screen; a continuing step re-arms the same session (AC-F008-14, AC-F010-04). |
| EX-M3-03 | A packet written by Claude Code appears in Foreman within 1 s, validated (AC-F013-07). |
| EX-M3-04 | Doctor and `prepare_launch` detect a too-old `claude`, a missing `claude`, a logged-out state and a wrong auth method (AC-F008-01, AC-F008-03, AC-F008-08). |
| EX-M3-05 | Every threat in `docs/threat-model.md` links to a control and a passing test; `security-reviewer` reports zero CRITICAL or HIGH findings (Stage 8 inspection). |
| EX-M3-06 | The keyboard-only script completes the full P1 loop (AC-F028-02). |
| EX-M3-07 | **Every step of TradeLog Stages 1–2 has been started through Foreman** (chat steps through Copy and open, CLI steps through Run in terminal); close-out (attest, F.2 lessons, APPROVED BY and the commit) by hand, guided by the close-out checklist step (AC-F015-01). |
| EX-M3-08 | **Kill check passed** (below), recorded by Billy in Foreman's Stage 9 packet. |

**The kill check (Master Spec §9), an exit criterion of M3.** Measured identically for Foreman (Export metrics, F-026) and for the by-hand baseline (`docs/metrics-baseline.md`), per surface (chat, CLI).

| ID | Kill-check criterion | Source |
|---|---|---|
| AC-KC-01 | **Time to start a step**, from the moment Billy begins looking for the next step (opening the blueprint or prompts file by hand; focusing Foreman or pressing Do next) to the step being ready to run. The end point is the same on both sides: for CLI, the spawn (by hand: Enter on the typed command); for chat, **the filled prompt on the clipboard and the target chat open** (in Foreman, when Copy and open returns; by hand, when Billy has copied the edited prompt and the right chat is in front). **Given** Foreman's median per surface over TradeLog Stages 1–2 and the by-hand median for the same surface **then** Foreman's median is lower, for chat and for CLI | Export metrics vs baseline |
| AC-KC-02 | **Active time per step**, from the step's time-to-start start point to the step done (packet saved, Mark done or exit), excluding waits (Claude working, the 7-day test), so Foreman's own overhead (dialogs, confirmations, re-arm, paste-back) counts. **Given** Foreman's median per surface over TradeLog Stages 1–2 and the by-hand median per surface **then** Foreman's median is lower, for chat and for CLI | Export metrics vs baseline |
| AC-KC-03 | **Manual prompt edits.** **Given** every TradeLog Stage 1–2 step run through Foreman **then** none needed a manual prompt edit (any change to resolved prompt text outside declared run inputs). The export lists each step's run-input interventions and every Copy prompt on a CLI step (the proxy); Billy's Stage 9 attestation explains each proxy event | Export metrics + attestation |
| AC-KC-04 | **Baseline present.** **Given** a surface **then** the baseline has by-hand entries for it: CLI from Foreman's own Stage 5 onward (ticket M0-19); chat from **at least 10 chat steps** Billy runs by hand, in any project, between M0 and the kill check (named in ticket M0-19). A surface with fewer entries can't pass | `docs/metrics-baseline.md` |
| AC-KC-05 | **Decision.** **Given** AC-KC-01 to AC-KC-04 all hold **then** M4 starts. **Given** any fails **then** work stops and no M4 ticket starts; the simplification is proposed as a Change Request (Master Spec §16) for Billy to approve | Stage 9 packet |

### M4 · Complete v1 (blueprint Stages 11–12; Stage 10 Electrical not applicable, `payments = no`)

Every P2 part of every feature, except F-030 P2 (M5): F-001 (Stages 0–5, Gate), F-003 (screen, harness through Stage 5), F-006 (Stages 3–5), F-008 (probes), F-009 (Now/Next, round tracker, focus), F-010 (sensor idle, Desktop fork), F-011, F-012 (ADR split, chat close-out), F-013 (repair, capture), F-014, F-015, F-016, F-017 (inbox), F-018 (wizard), F-019 (Doctor, probes), F-023, F-024, F-025, F-026 (run log screen, panel, retention), F-027, F-028 (palette), F-029 (the rest).

| ID | Exit criterion |
|---|---|
| EX-M4-01 | Every M4 criterion and AC-XC-13, AC-XC-18 passes. |
| EX-M4-02 | The real v2.2 blueprint's Stages 0–5 and the Stage Gate parse with zero errors; the TradeLog harness covers Stages 1–5 and the Gate prompts with zero unresolved tokens (AC-F001-02, AC-F003-04, NFR-48). |
| EX-M4-03 | Foreman signs a packet end to end on a fixture repo with lint-staged + Prettier hooks, unrelated staged changes and another-branch signing (AC-F014-05 to AC-F014-11). |
| EX-M4-04 | A forced failure in each runner shows a clear message and writes a log entry; a forced crash loses no state (Stage 11 inspection; AC-XC-18, AC-F020-10). |
| EX-M4-05 | Every screen has working empty, loading, error, locked and not-applicable states (Stage 12 inspection). |

**Foreman's own close-out in v1.** Foreman's own build doesn't run inside Foreman in v1: its Stages 11–17 close out by hand (attest, lessons, APPROVED BY, commit), as Stages 5–10 did. Running a project that was started outside Foreman needs adoption, which is v1.1 (§1.3). TradeLog is the project that runs through Foreman (EX-M3-07, EX-M5-06).

### M5 · Finishes (blueprint Stages 13–17)

F-030 P2 (sheet strip, title block, inspector, stamp), design-system application, accessibility (AR-01), the Stage 14 audits, Stage 15 first-run and copy review, Stage 16 readiness and drills, Stage 17 Certificate of Occupancy.

| ID | Exit criterion |
|---|---|
| EX-M5-01 | Every M5 criterion (F-030 P2, AC-XC-20 to AC-XC-26) passes. |
| EX-M5-02 | Zero hard-coded design values (lint); every screen matches its mockup in both themes (AC-XC-26). |
| EX-M5-03 | Stage 14: zero open CRITICAL or HIGH findings across the security review, performance and architecture audits; every SR re-verified; every NFR met in the release build (§6). |
| EX-M5-04 | Stage 15: a fresh install reaches a ready first project through first run alone; every string passes the writing rules (Master Spec A.9). |
| EX-M5-05 | Stage 16: every readiness item DONE, including the degraded modes (`claude` missing, logged out, too old; `git` missing; blueprint repo moved; project folder deleted; disk full); drills recorded in `docs/drills.md`: kill Foreman mid-write (no corruption), corrupt an app-data state file (clear error and recovery), install a broken release and roll back to `Foreman-previous.app`. |
| EX-M5-06 | **TradeLog Stages 3–5 run through Foreman**, so Stages 1–5 have run entirely through Foreman. |
| EX-M5-07 | **Success metrics read** (Master Spec §9), recorded in Foreman's Stage 17 packet: AC-SM-01 to AC-SM-04. |
| EX-M5-08 | Stage 17: Foreman v1.0.0 installed from a release build with rollback proven; every ADR matches the code; packets 1–16 signed; lessons recorded at every gate. |

| ID | Success metric | Measured by |
|---|---|---|
| AC-SM-01 | **Given** TradeLog Stages 1–5 **then** every step was started through Foreman | Run log + stage records |
| AC-SM-02 | **Given** those stages **then** zero manual prompt edits (as AC-KC-03) | Metric panel + attestation |
| AC-SM-03 | **Given** those stages **then** zero gates were skipped by accident: every applicable gate item has a result, note or not-applicable reason in its stage record | Stage records (AC-F015-10) |
| AC-SM-04 | **Given** a week of real use **then** time to start a step is p95 ≤ 10 s, excluding time typing run inputs (NFR-09) | Metric panel (AC-F026-09) |

---

## 3. User stories and acceptance criteria

Stories are numbered `US-Fnnn-n`. "Billy" is the only user.

### F-001 · Template engine

**US-F001-1.** As Billy, I want Foreman to take every run setting from the blueprint's `fm:` annotations only, so that each step runs identically every time and prose can't change behavior.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F001-01 | **Given** the real blueprint v2.2 **when** it is parsed **then** there are zero errors and the snapshot of every step in Stages 1–2 and F.2 (id, kind, tag, when, surface, cwd, session, chain, start, switches, alts, in, out, appends, requires, prompt text) matches the golden file | unit | M1 |
| AC-F001-02 | **Given** the real blueprint v2.2 **when** it is parsed **then** Stages 0–5 and the Stage Gate also parse with zero errors and match the golden file | unit | M4 |
| AC-F001-03 | **Given** a step whose RUN WITH prose says "fresh session" and whose annotation says `session=continue` **when** parsed **then** the step's session is `continue` and a lint warning cites the line; no value is ever taken from prose | unit | M1 |
| AC-F001-04 | **Given** a step with no `fm:step` **when** parsed **then** it is `needs-annotation`: listed, never runnable | unit | M1 |
| AC-F001-05 | **Given** random valid annotation sets in random key order **when** parsed and rendered back **then** the result is identical | prop | M1 |
| AC-F001-06 | **Given** arbitrary input text **when** parsed **then** the parser never panics and every diagnostic carries file, line and column | prop | M1 |
| AC-F001-07 | **Given** a 20,000-line blueprint **when** loaded uncached **then** parse + validate + lint is p95 ≤ 50 ms; a cached load (same source hash) is ≤ 5 ms | bench | M1 |
| AC-F001-08 | **Given** the same blueprint text with CRLF line endings, a BOM, or tabs **when** hashed **then** the source and item hashes equal those of the normalized text | prop | M1 |

**US-F001-2.** As Billy, I want annotation mistakes reported with their location, so that I fix the blueprint before a run instead of during it.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F001-09 | **Given** one fixture per validation rule (duplicate step or item IDs; `session=continue` with no earlier step in the stage; `chain=` naming an unknown or later step; a continuing step with a cwd different from its chain; a model, effort or mode outside the known values; a condition ID in `when` or `fm:if` that isn't one of the twelve; verify entries ≠ bullets; an `out`, `appends` or `requires` path failing the path policy) **when** parsed **then** each produces its own error with file, line and column | unit | M1 |
| AC-F001-10 | **Given** `bypassPermissions` anywhere in the blueprint **when** parsed **then** it is an error | unit | M1 |
| AC-F001-11 | **Given** an item of kind `auto` **when** parsed **then** the diagnostic says "auto items need Foreman v1.1" and the item is treated as `attest` | unit | M1 |
| AC-F001-12 | **Given** an `fm:` comment inside a list item or inline **when** parsed **then** the error says "annotations must be block-level" with the line | unit | M1 |
| AC-F001-13 | **Given** an unknown annotation key **when** the grammar is v1 **then** it is an error; **when** the declared grammar version is higher than Foreman supports **then** a step whose `fm:step` has an unknown key is `needsNewerForeman` (shown, not runnable), an unknown directive or key anywhere else is a warning, and Template health says "Update Foreman" (DR-035) | unit | M1 |
| AC-F001-14 | **Given** an `fm:if` block nested inside another **when** parsed **then** it is an error | unit | M1 |
| AC-F001-15 | **Given** errors only in a stage the project doesn't use **when** the project opens **then** it opens; that stage's steps are not runnable; the errors are listed (Template health in M4) | unit | M1 |

**US-F001-3.** As Billy, I want each inspection item bound to its ID and text hash, so that reordering or rewording an item can't silently change how it's verified.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F001-16 | **Given** a pinned project **when** two inspection bullets are swapped without moving their IDs **then** both show "re-confirm kind" | unit | M1 |
| AC-F001-17 | **Given** an item showing "re-confirm kind" **when** Billy tries to attest it **then** attest is disabled until he confirms the kind in a native dialog raised by Rust; the confirmation is stored in app data `approvals`. *int:* a scripted native dialog; Cancel leaves the kind unconfirmed and attest disabled. *comp:* the webview shows the old and new bullet text as context and has no approve path of its own | int, comp | M4 |
| AC-F001-18 | **Given** the blueprint repo is missing or unreadable **when** Foreman starts **then** first run shows "Choose your blueprint repo", and existing projects open read-only from their cached lock | int | M2 |

### F-002 · Blueprint lock

**US-F002-1.** As Billy, I want each project pinned to the exact blueprint that produced it, so that every packet traces to a known template.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F002-01 | **Given** a clean blueprint repo **when** a project is pinned **then** `.foreman/blueprint.lock.json` holds `schemaVersion, blueprintRepoCommit, blueprintPath, sourceSha256, blueprintVersion, grammarVersion, pinnedAt` and `{textSha256, kind}` per item; the full text is in app data at `templates/<sourceSha256>.md`; the repo holds no copy | int | M1 |
| AC-F002-02 | **Given** the blueprint file has uncommitted changes **when** pinning **then** the pin is refused, showing the working-file and HEAD hashes (`hash-object --no-filters` vs `rev-parse HEAD:<path>`, DR-142) | int | M1 |
| AC-F002-03 | **Given** Stage 1 is signed **when** a re-pin is attempted **then** it is refused ("upgrading the pin is v1.1"); before Stage 1 is signed it is allowed | unit | M1 |

**US-F002-2.** As Billy, I want a project to recover its template on a new machine and to refuse a tampered one, so that I never run steps from the wrong text.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F002-04 | **Given** the template is missing from app data **when** the project opens **then** Foreman runs `git -C <blueprint repo> show <commit>:<path>` (with the fixed git invocation), checks the hash, stores it and opens normally | int | M1 |
| AC-F002-05 | **Given** the restored text or the cached template has a hash different from the lock **when** the project opens **then** it opens read-only and says why | int | M1 |
| AC-F002-06 | **Given** the blueprint repo is absent or the commit is gone **when** the project opens **then** it opens read-only with "Blueprint 2.2 at <commit> not found. Point Foreman at your blueprint repo or re-pin." | int | M1 |
| AC-F002-07 | **Given** git isn't found **when** a template needs restoring **then** the project opens read-only and the message names git as missing | int | M1 |
| AC-F002-08 | **Given** the Stage 4 packet **when** its `BLUEPRINT VERSION` differs from the lock's `blueprintVersion` **then** the packet is invalid with that problem | unit | M1 |

### F-003 · Template health and the TradeLog harness

**US-F003-1.** As Billy, I want an automated proof that every TradeLog step resolves with no manual edit, so that the success metric holds before I run a step.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F003-01 | **Given** the harness fixture (the real blueprint; a TradeLog `project.json` from the blueprint's example variables with all conditions `yes`; synthetic signed packets for Stages 1–4) **when** every runnable step of Stages 1–2 and F.2 is resolved **then** there are zero unresolved tokens, every run input is declared, and every CLI step has cwd, session and start | int | M1 |
| AC-F003-02 | **Given** the harness **when** it runs **then** the argv snapshot of every CLI step matches its golden file | int | M3 |
| AC-F003-03 | **Given** the harness fails **when** `pnpm verify` or CI runs **then** the run fails and merge is blocked | ci | M1 |
| AC-F003-04 | **Given** the harness **when** extended **then** it covers every runnable step through Stage 5 plus the Gate prompts with zero unresolved tokens | int | M4 |

**US-F003-2.** As Billy, I want a Template health screen, so that I see which stages are runnable and what's wrong before I start.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F003-05 | **Given** a loaded template **when** Template health opens **then** it shows the blueprint version; stage, step and item counts; errors and warnings grouped by stage; for each runnable step its annotations, lint warnings and declared outputs; and a row per stage marked runnable, partly or needs annotation | comp | M4 |
| AC-F003-06 | **Given** a template with no errors or warnings **when** Template health opens **then** it says so instead of showing empty groups | comp | M4 |

### F-004 · Rule engine

**US-F004-1.** As Billy, I want each step's applicability decided by tags and conditions, so that I never decide "does this apply?" from memory.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F004-01 | **Given** every combination of tag (`core`, `conditional`, `optional`, `optional` + `when`) and condition value (`yes`, `no`, `undecided`) **when** the applicable path is computed **then** each step's state matches the table in `docs/domain-rules.md`, including every row of the Master Spec §5 default tagging | unit | M1 |
| AC-F004-02 | **Given** any conditions, skips and overlay **when** the applicable path is computed **then** no core item is ever dropped, skipped or not applicable | prop | M1 |
| AC-F004-03 | **Given** a conditional step whose `when` is false **when** shown **then** it is a hatched not-applicable row with one line naming the condition (e.g. "Not applicable: this app doesn't take payments (payments = no)") and no Skip control | comp | M2 |
| AC-F004-04 | **Given** a step whose `when` references an `undecided` condition **when** shown **then** it is `undecided`, not runnable, with "Decide <condition> to continue" | comp | M2 |
| AC-F004-05 | **Given** an optional step **when** Billy skips it without a reason **then** the skip is rejected; with a one-line reason **then** it becomes `skipped(reason)` in `overlay.json` | unit | M1 |
| AC-F004-06 | **Given** an optional step with a `when` **then** it is not applicable when the expression is false and skippable with a reason when true | unit | M1 |
| AC-F004-07 | **Given** a core step **when** shown **then** it carries the lock with "Core: protects production quality. Change it in the blueprint with a Change Request." and no Skip; **when** a skip is requested over IPC **then** it is rejected | comp | M2 |
| AC-F004-08 | **Given** a skip recorded on a step that is now core **when** the path is computed **then** the skip is ignored and flagged | unit | M1 |
| AC-F004-09 | **Given** a stage in which nothing applies **when** the path is computed **then** the stage is not applicable, has no gate and no packet, the next stage's packet remap points at the last signed packet, and the reason is queued for the next signed packet's `SKIPPED:` | unit | M1 |

**US-F004-2.** As Billy, I want conditions editable with an audit trail until Stage 2 is signed and locked after it, so that the path only changes on purpose.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F004-10 | **Given** Stage 2 isn't signed **when** a condition changes **then** `{condition, from, to, at, note}` is appended to `project.json`'s audit list and the applicable path updates without a restart | unit | M1 |
| AC-F004-11 | **Given** Stage 2 is signed **when** Billy tries to change a condition **then** the change is refused with "Change requires a Change Request (v1.1)" and the CR template path | comp | M2 |
| AC-F004-12 | **Given** any early condition is `undecided` **when** a Stage 2 packet signed by hand is detected (P1) **then** Foreman shows the problem "Decide <condition> before signing Stage 2" and doesn't count Stage 2 as signed | int | M3 |
| AC-F004-13 | **Given** any early condition is `undecided` **when** Billy opens the Stage 2 sign dialog **then** the Confirm conditions checkpoint blocks Sign and names the condition | comp | M4 |

### F-005 · Run configs and model policy

**US-F005-1.** As Billy, I want the model, effort and mode picked by my project's policy and shown with their origin, so that Max/Pro alternatives and the no-Fable rule apply without my memory.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F005-01 | **Given** a template value, an `alt=` entry, a substitution and an override in every combination **when** the run config is computed **then** precedence is override > substitution > alternative > template | unit | M1 |
| AC-F005-02 | **Given** `plan: max` or `plan: pro` **when** a step has `alt=` entries **then** the matching alternative is chosen | unit | M1 |
| AC-F005-03 | **Given** Foreman's own policy (`fable → opus@xhigh`) **when** a step starts on Fable 5.1 · high **then** the spec line reads "Fable 5.1 · high (Max) → Opus 5.5 · xhigh (substituted)" | comp | M2 |
| AC-F005-04 | **Given** a spec-line value **when** Billy hovers it **then** it says where the value came from (template, alternative rule, substitution, override) | comp | M2 |
| AC-F005-05 | **Given** Billy sets a per-run override chip **when** the step runs **then** the run record logs the override, and (M4) the signing writes it under `SUBSTITUTIONS:` | int | M3 |
| AC-F005-06 | **Given** a step with `cwd=blueprint` or `cwd=worktree` **when** parsed **then** it is rejected as "v1.1" | unit | M1 |
| AC-F005-07 | **Given** a step with switches **when** it runs **then** no switch is sent to the PTY as keystrokes; switches appear only as cues (Now/Next strip in M4) | int | M3 |

### F-006 · Prompt resolver

**US-F006-1.** As Billy, I want every bracket token filled from the project, signed packets and my declared inputs, so that I never edit a prompt by hand.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F006-01 | **Given** the real blueprint **when** tokens are classified **then** every bracket token in Stages 1–2, the Gate and F.2 falls into exactly one of the six classes, pinned by a golden test (P2: through Stage 5, AC-F006-15) | unit | M1 |
| AC-F006-02 | **Given** a token the classifier doesn't know **when** parsed **then** it is a template error, never a guess | unit | M1 |
| AC-F006-03 | **Given** a project variable whose value equals the blueprint's placeholder **when** resolved **then** it counts as unresolved | unit | M1 |
| AC-F006-04 | **Given** packet markers **when** resolved **then** they become signed packet text joined in order; `[paste the stage's inspection list]` contains only applicable items | unit | M1 |
| AC-F006-05 | **Given** path aliases **when** resolved **then** `[app]-spec/` → the project's spec folder, packet paths → the latest signed packet on the applicable path, `[N+1]` → the next applicable stage, and each remap is shown as a note on the step | unit | M1 |
| AC-F006-06 | **Given** a `cwd=spec` step **when** resolved **then** repo-relative paths render relative to the cwd (`docs/handoff/stage-02.md` → `../docs/handoff/stage-02.md`) | unit | M1 |
| AC-F006-07 | **Given** an `fm:if` part whose expression is false, or an optional part with Include off **when** resolved **then** its text is removed and recorded for `SKIPPED:`; Include defaults to on | unit | M1 |
| AC-F006-08 | **Given** the same template, project, packets, inputs and context **when** resolved twice **then** the output is identical (pure function) | prop | M1 |
| AC-F006-09 | **Given** a signed packet or run input containing text that looks like flags, slash commands or instructions **when** resolved and a launch is planned **then** no flag, mode, model, cwd or write destination changes | prop | M1 |
| AC-F006-10 | **Given** a packet marker for a stage that was not applicable **when** resolved **then** it uses the latest signed packet before it, with the remap note | unit | M1 |
| AC-F006-11 | **Given** a packet marker whose packet is unsigned **when** Billy runs the step **then** Run is blocked with "Sign Stage N first" | unit | M1 |

**US-F006-2.** As Billy, I want run inputs as inline fields with unresolved tokens highlighted, so that I fill only what the step declares.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F006-12 | **Given** a step with `in=` annotations **when** shown **then** each input appears as a field of its declared type above the prompt; a `remember` input is prefilled with its last value for this project; context inputs (`[N]`) fill themselves | comp | M2 |
| AC-F006-13 | **Given** unresolved required tokens **when** shown **then** each is an attention pill; clicking it focuses its field; Run stays disabled until all resolve | comp | M2 |
| AC-F006-14 | **Given** a run input longer than 64 KB **when** entered **then** it is accepted with a warning that chat paste limits may apply | comp | M2 |
| AC-F006-15 | **Given** Stages 3–5 **when** classified and resolved **then** every token is classified and the harness shows zero unresolved tokens | unit | M4 |
| AC-F006-16 | **Given** the resolver **when** benchmarked **then** resolution is p95 ≤ 50 ms | bench | M1 |
| AC-F006-17 | **Given** a resolved prompt containing a C0 control other than TAB, LF or CR, DEL, or a C1 control (U+0080–U+009F), whether from a packet, a variable or a run input **then** the prompt is rejected with the position of the character | unit | M1 |

### F-007 · Step kinds and states

**US-F007-1.** As Billy, I want every blueprint step represented as a kind Foreman understands, so that nothing lives only in my head.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F007-01 | **Given** every (state, event) pair over `not-started, copied, running, done, skipped(reason), n/a(reason), waiting-until(date), undecided, needs-annotation` **when** applied **then** the transition is legal or rejected with a reason; `adopted` is never reachable | prop | M1 |
| AC-F007-02 | **Given** an `action` step (`write-blocks`, `copy-file`, `mkdir`) **when** it runs **then** it writes only to the destinations hard-coded for its action ID (`spec/.claude/agents/<known names>.md`, `spec/01-app-brief.md`); template, packet or overlay text can't change a destination | unit | M1 |
| AC-F007-03 | **Given** an `action` step whose destination already exists **when** it runs **then** the existing file is listed with a diff and written only after Billy confirms; if it changed since Foreman read it, the write is refused | int | M1 |
| AC-F007-04 | **Given** a `manual` step naming a file **when** that file changes after the step started **then** the step is satisfied; Open file opens it | int | M3 |
| AC-F007-05 | **Given** a `wait` step **then** it is `waiting-until(date)` and becomes available after the date | unit | M1 |
| AC-F007-09 | **Given** a `wait` step's date passes **then** a macOS notification fires with Foreman-generated text only (never prompt, packet, terminal or assistant text), and clicking it only focuses Foreman | int | M3 |
| AC-F007-06 | **Given** a `repeatable` step **when** it runs again **then** its run counter increments | unit | M1 |
| AC-F007-07 | **Given** a `followup` step **when** Billy runs it **then** the prompt is copied and the terminal of its chain is focused; nothing is typed into the PTY | int | M3 |
| AC-F007-08 | **Given** a chat step **when** Billy presses ⌘D or a valid paste-back lands **then** the step is `done` | comp | M2 |

### F-008 · Launch pipeline

**US-F008-1.** As Billy, I want every launch checked before it starts, so that a wrong version, a billing leak or a bad input never reaches a session.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F008-01 | **Given** `claude -v` reports a version below the computed floor **when** Billy runs a step **then** Run is disabled and the banner reads like "Claude Code 2.1.270 found. Opus 5.5 needs 2.1.280 or later. Run `claude update`, then check again."; `claude -v` is read on every launch | int | M3 |
| AC-F008-02 | **Given** the steps' models, flags and the pinned kit's agent models **when** the floor is computed **then** it is their maximum (2.1.293 with the current kit) | unit | M1 |
| AC-F008-03 | **Given** `claude` isn't found at the configured or auto-detected path, or the path is relative, missing, not executable, inside a registered project, or a symlink into a writable project folder **when** Billy runs a step **then** Run is blocked, the reason names the path, and the fix points to Settings or Doctor | int | M2 |
| AC-F008-04 | **Given** a fingerprinted source changed since the last launch in that cwd **when** Billy runs a step **then** a banner shows a redacted diff (AC-F008-23) and the launch waits for one Acknowledge | int | M3 |
| AC-F008-05 | **Given** any fingerprinted settings source sets `apiKeyHelper`, `awsAuthRefresh`, `awsCredentialExport`, `gcpAuthRefresh` or `otelHeadersHelper`, or an `env` key matching `ANTHROPIC_*`, `CLAUDE_CODE_*`, `CLAUDE_CONFIG_DIR`, `AWS_*`, `GOOGLE_*`, `CLOUDSDK_*`, `AZURE_*`, `*_PROXY`/`*_proxy`, `NODE_EXTRA_CA_CERTS`, `NODE_TLS_REJECT_UNAUTHORIZED`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `NODE_OPTIONS` or `DYLD_*` **when** Billy runs a step **then** the launch is blocked, naming the file and key (one case per key and pattern) | int | M3 |
| AC-F008-06 | **Given** the fingerprint **then** a change to any one of these changes it: user and managed settings; `mcpServers` in `~/.claude.json` (user scope and this project's path); user memory and `~/.claude/{agents,skills,commands,rules,output-styles}/**`; every `CLAUDE.md` and `CLAUDE.local.md` from the cwd up to `/` and their `@` imports; the project's `.claude/**` (excluding `worktrees/`) and `.mcp.json`; enabled plugins' folders; every local file named as the program of a hook, `statusLine`, `fileSuggestion` or helper command (one test per source; the exact set is verified in 4.2) | int | M3 |
| AC-F008-07 | **Given** S-7′ took its fail branch **when** a project settings file in the step cwd has an `env` block Billy hasn't approved **then** the launch is blocked until he approves it | int | M3 |
| AC-F008-08 | **Given** SR-06 and SR-07 checks have passed **when** `claude auth status` (in the step cwd, with the exact launch environment) returns an `authMethod` other than `claude.ai` or `oauth_token` (wrong auth method), a missing or unknown `authMethod`, unparsable output, a non-zero exit (logged out), or takes longer than 10 s **then** the launch is blocked with the reason and the fix; `auth status` never runs before the fingerprint and block checks pass | int | M3 |
| AC-F008-09 | **Given** the auth result is cached **when** the fingerprint is unchanged **then** `auth status` isn't re-run; **when** it changed **then** it is re-run; only `authMethod` and the exit code are read or stored | int | M3 |
| AC-F008-22 | **Given** a blocked key-value in **user** settings that Billy has allowlisted in Settings (by hash) **then** it no longer blocks; the same key-value in any project source still blocks | int | M3 |
| AC-F008-23 | **Given** a config snapshot kept for diffs **then** values under `env` or `headers`, any `apiKey*` key, any key containing `token`, `secret` or `password` at any depth, and any string matching the secret-scanner patterns are stored as `‹redacted sha256:8›`; diffs show "value changed"; snapshots are 0600 | unit | M3 |
| AC-F008-24 | **Given** a Claude config file in the fingerprint set larger than its cap (256 KB; `~/.claude.json` 64 MB, parsed as a stream that keeps only `mcpServers` paths) **then** the launch is blocked, naming the file. **Given** a read-only fingerprint source outside the repo that is a symlink to a regular file outside every registered project **then** Foreman follows it, hashes the target and records both the link and the target; a symlink inside repo content is still refused | int | M3 |
| AC-F008-10 | **Given** inputs **when** a signed packet the prompt references is missing **then** the launch is blocked; **when** another referenced path is missing **then** it warns with the reason ("02-teardown.md not found: 2.1 was not applicable"); for a continuing step, only the transcript file's existence is checked, never its contents | int | M3 |
| AC-F008-11 | **Given** a required run input is empty **then** Run is blocked | unit | M2 |
| AC-F008-12 | **Given** a resolved prompt that, after leading whitespace is trimmed, starts with `-`, `/` or `!` **then** the launch is blocked | prop | M2 |
| AC-F008-13 | **Given** `prepare_launch` with cached data **when** benchmarked, excluding the `claude -v` spawn (which runs in parallel with the fingerprint and is counted in NFR-05) **then** it is p95 ≤ 50 ms | bench | M3 |

**US-F008-2.** As Billy, I want the exact argv, environment and settings fixed by Foreman and visible before I run, so that the session starts exactly as the template says.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F008-14 | **Given** every template step and every overlay step **when** its argv is built **then** it has the Master Spec §6 shape, contains only flags from the fixed list with values from enums or validated character sets (`--name` matches `^fm-[a-z0-9-]{1,80}$`, UUIDs parsed), the prompt is the final positional argument, and it never contains `--bare`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, `bypassPermissions`, `-p` or `--print` | prop | M2 |
| AC-F008-15 | **Given** Foreman's own environment holds `ANTHROPIC_API_KEY`, `CLAUDE_CODE_USE_BEDROCK`, `AWS_*`, `GOOGLE_*`, `GCLOUD_*`, `CLOUDSDK_*`, `AZURE_*`, `DYLD_*`, `NODE_OPTIONS`, `BASH_ENV`, `ENV`, `GIT_*`, and Billy's pass-through list names some of them **when** the fake binary records the child environment **then** it was built from `env_clear()` and holds only HOME, USER, LOGNAME, SHELL, TMPDIR, LANG, LC_*, PATH, FOREMAN_RUN_ID, TERM=xterm-256color, COLORTERM=truecolor, the pass-through list, and `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` when present; none of the always-removed variables; and a PATH whose entries are all absolute, existing, outside registered projects and not world-writable (empty, `.` and relative entries dropped) | int | M2 |
| AC-F008-16 | **Given** a launch **then** a fresh `<app-data>/runs/<runId>/settings.json` (mode 0600, app data only, hash in the run record) holds `disableAllHooks: false`, `permissions.disableBypassPermissionsMode: "disable"`, and deny rules `Read(//<repo>/.foreman/**)`, `Edit(//<repo>/.foreman/**)`, `Read(//<app-data>/**)`, `Edit(//<app-data>/**)` with absolute paths for both cwd = project and cwd = spec (M4: plus the sensor hooks), and is passed as `--settings` | unit | M2 |
| AC-F008-17 | **Given** any step **when** shown **then** its resolved prompt, exact argv and the names (not values) of the child environment variables are one click away under the primary action | comp | M2 |
| AC-F008-18 | **Given** an overlay-added step, a mode or model different from the template's (after model policy), the first launch in a project, `auto`/`dontAsk` in an interactive session, or the first launch after Claude Code's version changed ("Claude Code updated X → Y") **when** Billy runs it **then** a native confirmation dialog raised by Rust names the reason; otherwise no dialog appears (one case per reason). *int:* a scripted native dialog; Cancel returns `Cancelled`, spawns nothing and writes no run record. *comp:* the webview's preview lists the same reasons as context and `launch_start` carries no approval field | int, comp | M2 |
| AC-F008-19 | **Given** a launch **when** spawn happens **then** the run record with status `running` already exists on disk | int | M2 |
| AC-F008-20 | **Given** Settings → Screen reader mode is on **when** a CLI step launches **then** argv includes `--ax-screen-reader` (and xterm runs with `screenReaderMode`); off, neither | int | M3 |
| AC-F008-21 | **Given** Claude Code's version differs from the last verified version **when** Billy runs a step **then** the contract probes run first (AC-F019-15) | int | M4 |
| AC-F008-25 | **Given** a launch **when** the fake binary checks its process state **then** the PTY child runs in its own session (`setsid`, through portable-pty), with no inherited file descriptors above 2 and umask 022; non-PTY children (`claude -v`, `claude auth status`, git, `/usr/bin/open`) run in their own process group (`process_group(0)`) with the same fd and umask rules | int | M2 |

### F-009 · Terminal runner

**US-F009-1.** As Billy, I want each CLI step in its own named terminal tab, so that I always know which step a session belongs to.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F009-01 | **Given** a CLI step **when** it launches **then** a portable-pty child streams output to xterm over a Channel, input and resize go back as commands, and the tab is named `fm-<project>-s<NN>-<step>`; one tab per chain | int | M3 |
| AC-F009-02 | **Given** a running tab **when** the child exits **then** the tab shows `exited(code)` and the run record is updated | int | M3 |
| AC-F009-03 | **Given** the child exits during spawn or a PTY write fails **then** the tab shows `errored` with stderr and the run record is updated | int | M3 |
| AC-F009-04 | **Given** the tab menu **when** Billy chooses Interrupt **then** Ctrl-C is sent; **when** Terminate **then** Foreman asks first (in P1 always, in M4 only when the session isn't idle), then sends SIGTERM and SIGKILL after 5 s | int | M3 |
| AC-F009-05 | **Given** a tab **then** Copy session ID copies the chain's UUID and Resume re-arms the chain | comp | M3 |
| AC-F009-06 | **Given** a tab **then** scrollback is capped at 10,000 lines (or the S-0 branch value); on re-arm the old buffer becomes a collapsed read-only archive and a fresh buffer starts | comp | M3 |
| AC-F009-07 | **Given** a run **then** its last 2,000 lines are in `runs/<runId>/output.log` | int | M3 |
| AC-F009-08 | **Given** the terminal is focused **when** Billy presses a ⌘-chord or ⌃` **then** it reaches the app; ⌘C copies the selection, ⌘V pastes into the PTY, ⌘A and ⌘F are handled by the terminal; ⌘⇧E returns focus to the workspace; every other key goes to the PTY (confirmed with the real TUI in S-0) | comp | M3 |
| AC-F009-09 | **Given** running tabs **when** Billy quits **then** the quit guard says "3 sessions are running; they end when Foreman quits; you can resume each later", and after he confirms each child gets SIGHUP; **when** he closes one running tab **then** Foreman asks first | comp | M3 |
| AC-F009-10 | **Given** a 1 MB output burst **then** no input is dropped and the UI never freezes for more than 100 ms | int | M3 |
| AC-F009-15 | **Given** a paste into a terminal tab of `\x1b[201~\x03!touch /tmp/pwn\r` **then** Foreman's paste handler removes every ESC and strips C0/C1 controls (other than TAB, LF, CR) and DEL with a visible notice, and the PTY receives one bracketed-paste block with no ESC inside; with the real TUI (S-0) the paste runs nothing | int | M3 |
| AC-F009-16 | **Given** text Foreman puts on the clipboard for a terminal step (e.g. a `followup`) **then** it is sanitized the same way | unit | M3 |
| AC-F009-17 | **Given** an OSC 8 link or a link found by the web-links addon **when** Billy clicks it **then** the URL goes to Rust, which opens it only if it is `https:` (or `http:` to localhost or 127.0.0.1) with no userinfo, after a dialog showing the full URL; every other scheme is refused | int | M3 |
| AC-F009-18 | **Given** the xterm configuration **then** `windowOptions` are all false, `@xterm/addon-clipboard` isn't loaded (no OSC 52), and OSC 0/2 titles never become tab names (options snapshot) | unit | M3 |
| AC-F009-19 | **Given** `pty_write` **when** its run isn't live **then** it is rejected | int | M3 |

**US-F009-2.** As Billy, I want the terminal to tell me what comes next in the step, so that I make the right in-session switch.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F009-11 | **Given** a running step with a switch **then** the Now/Next strip shows "Now: Plan · Opus 5.5 · xhigh │ Next: after you approve the plan, choose Accept edits"; with the sensor, the live mode shows, and a mismatch with the step's mode shows in the `--attention` state | comp | M4 |
| AC-F009-12 | **Given** a step with several `out=` files **when** the watcher sees each written **then** the round tracker advances and Open output appears for each | int | M4 |
| AC-F009-13 | **Given** a terminal **when** Billy presses ⌘⇧T **then** focus mode toggles | comp | M4 |
| AC-F009-14 | **Given** the sensor **then** tabs show `idle` and `blocked` states | comp | M4 |

### F-010 · Session chains and re-arm

**US-F010-1.** As Billy, I want "same session" steps relaunched in the same conversation with the next step's exact flags, so that I never type `/model`, `/effort` or Shift+Tab mid-session.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F010-01 | **Given** a `session=new` step **when** it launches **then** Foreman generates a UUID, passes `--session-id <uuid>` and never `--resume` | int | M3 |
| AC-F010-02 | **Given** every continuing step in the harness **when** its argv is built **then** it has `--resume <uuid>` plus its full flags and never `--session-id` (except with `--fork-session`, and only if S-8 confirms that works, AC-F010-15) | prop | M3 |
| AC-F010-03 | **Given** a continuing step whose chain process is alive **when** Billy presses Run **then** Rust asks him in a native dialog to confirm "Claude is idle and nothing is running in the background" (P1) and sends nothing before he confirms; Cancel changes nothing | comp | M3 |
| AC-F010-04 | **Given** Billy confirms **then** Foreman writes Ctrl-C, then `/exit` + Enter, in that order; waits up to 10 s for exit; relaunches in the same tab with `--resume <uuid>`, the step's flags, `--settings` and the prompt; and writes the divider `── <step> · <model> · <effort> · <mode> · permission grants reset ──` | int | M3 |
| AC-F010-05 | **Given** the process hasn't exited after 10 s **then** Foreman shows "Claude didn't exit. End it yourself, then press Continue." and sends no SIGTERM or SIGKILL without explicit confirmation | int | M3 |
| AC-F010-06 | **Given** the chain's process already exited **when** Billy runs the next step **then** re-arm is just the relaunch | int | M3 |
| AC-F010-07 | **Given** any file event, terminal output, Claude output or sensor event **then** no launch, re-arm, Ctrl-C, `/exit`, termination or paste starts; each starts only from Billy's click or keystroke | prop | M3 |
| AC-F010-08 | **Given** a chain-start step is re-run **then** a new chain is created and the old one is `superseded` | int | M3 |
| AC-F010-09 | **Given** the chain's transcript is missing (broken chain) **when** the step is shown **then** it says "This conversation no longer exists (Claude Code removes transcripts after `cleanupPeriodDays`)" and offers Start a fresh chain with context, whose prompt is the last signed packet and the step's declared input paths followed by the step's prompt | int | M3 |
| AC-F010-10 | **Given** a continuing step **then** effort follows S-3's branch: `--effort` on resume; else `effortLevel` through `--settings`; else the `/effort <level>` line is shown with the note that it changes Billy's saved default | int | M3 |
| AC-F010-11 | **Given** S-8 took its fail branch **when** Billy re-arms **then** Foreman asks him to end the session himself and relaunches after he presses Continue | int | M3 |
| AC-F010-12 | **Given** chains in `progress.json` **when** Foreman restarts **then** each chain's status and steps are restored | int | M3 |

**US-F010-2.** As Billy, I want the sensor to tell Foreman when a session is really idle, so that I don't have to confirm it myself.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F010-13 | **Given** the sensor's last event is `Stop` with no outstanding subagent or background task **then** re-arm proceeds without the manual confirmation; it waits for `SessionEnd` with reason `prompt_input_exit` | int | M4 |
| AC-F010-14 | **Given** outstanding background work **when** Billy presses Run **then** Foreman shows "2 background tasks still running: Wait / End anyway"; without the sensor (S-1′ fail) the P1 confirmation stays | int | M4 |
| AC-F010-15 | **Given** a `handed-off` chain **when** the next continuing step runs **then** it relaunches with `--fork-session` as a new chain recorded as forked from the old one, unless Billy confirms Desktop is closed; the fork UUID follows S-8's branch | int | M4 |

### F-011 · Session sensor

**US-F011-1.** As Billy, I want Foreman to know which session is working, waiting or blocked, so that I know which step needs me.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F011-01 | **Given** a launch **then** the run settings add hooks for `SessionStart`, `Stop`, `StopFailure`, `Notification`, `PermissionRequest`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted` and `PostToolUse` (matcher `Bash`), each in exec form (`command` + `args`) pointing at `<app-data>/bin/foreman-sensor-<hash>` with the per-run token | unit | M4 |
| AC-F011-02 | **Given** the sensor binary in app data doesn't match its expected hash when a run settings file is about to be written **then** it isn't used for that run and the P1 manual behaviors apply | int | M4 |
| AC-F011-03 | **Given** an event with a wrong token **then** it is ignored | unit | M4 |
| AC-F011-04 | **Given** an event **then** the sensor reads at most 256 KB of stdin, writes one file into `<app-data>/events/<runId>/` with `O_EXCL\|O_NOFOLLOW` and mode 0600, prints nothing, and exits within 10 ms | int | M4 |
| AC-F011-05 | **Given** each event sequence **then** the derived state is correct: `working`, `idle`, `blocked:permission`, `blocked:input`, `blocked:plan-approval`, `ended(reason)`, live `permission_mode` and `effort.level`, outstanding subagents and tasks, `last_assistant_message`, observed model | unit | M4 |
| AC-F011-06 | **Given** the live mode or effort differs from the step's **then** a visible warning shows; nothing is enforced | comp | M4 |
| AC-F011-07 | **Given** a run **then** only the last 20 events are kept, and all are deleted when the run's stage is signed or 30 days after the run ends, whichever comes first | int | M4 |
| AC-F011-08 | **Given** any event **then** it only updates displayed state and enabled buttons; no action starts | prop | M4 |
| AC-F011-09 | **Given** the sensor produces no events during a run **then** the step still runs, with the manual idle confirmation | int | M4 |
| AC-F011-10 | **Given** any sensor setup path **then** Foreman never writes `~/.claude/settings.json` | int | M4 |

### F-012 · Chat bridge

**US-F012-1.** As Billy, I want chat steps copied and opened in the right claude.ai conversation, so that starting a chat step takes seconds.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F012-01 | **Given** a chat step **then** its header shows checkable chips: "New conversation in Project <name>" or "Same chat as 1.1" (bold), model, Web search ON, Research ON, labeled as memory aids | comp | M3 |
| AC-F012-02 | **Given** a chat chain **when** Billy uses Link this chat (paste, or From clipboard on his click) **then** the conversation URL is stored; a URL outside `https://claude.ai/` is refused | unit | M3 |
| AC-F012-03 | **Given** a chat step **when** Billy presses Copy and open (⌘↵) **then** the resolved prompt is copied and Rust opens the Project URL (new conversation) or the stored conversation URL (continuing step) in the default browser | int | M3 |
| AC-F012-04 | **Given** no Project URL (new conversation) or no linked conversation (continuing step) **when** Billy presses Copy and open **then** the prompt is copied, nothing is opened, and the step says which link to set | comp | M3 |

**US-F012-2.** As Billy, I want to paste chat output back and have it saved and validated, so that I never hand-copy output into files.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F012-05 | **Given** real packets from Foreman's own `docs/handoff/` **when** pasted back **then** `<blank>`, `<leave blank…>`, `<sign here…>` normalize to unsigned; `None`/`none.` → `none` with a note; smart quotes and NBSP are fixed (golden) | unit | M3 |
| AC-F012-06 | **Given** a packet followed by more text (e.g. the 1.4 press release) **when** pasted **then** the header through the format's last field is the packet and the rest goes to `docs/handoff/stage-NN-attachments.md`; declared extra destinations are written too; one confirmation line lists every write before it happens | int | M3 |
| AC-F012-07 | **Given** an output paste **then** it goes to the step's `out` path; an existing file shows a diff first | int | M3 |
| AC-F012-08 | **Given** a paste-back target changed on disk since Foreman read it, or one refused by the path policy **then** nothing is written and the conflict or rule is shown | int | M3 |
| AC-F012-09 | **Given** the clipboard is empty, not text, or larger than 1 MB **when** Billy pastes back **then** an inline error says so and nothing is written | comp | M3 |
| AC-F012-10 | **Given** the wrong stage's packet **then** "This is the Stage 3 packet; this step expects Stage 2."; **given** two packets **then** "paste one packet at a time"; nothing is written in either case | unit | M3 |
| AC-F012-11 | **Given** an invalid pasted packet **then** Copy fix request copies "Your packet is missing: X, Y. OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block." with the actual problems | unit | M3 |
| AC-F012-12 | **Given** a valid paste-back **then** the step is `done` | unit | M3 |
| AC-F012-13 | **Given** pasted content with HTML, scripts or Markdown links **when** displayed **then** it renders as escaped text; links show as text with an explicit Open action | comp | M3 |
| AC-F012-14 | **Given** a step whose output is declared with `appends` (e.g. F.2 to `docs/lessons.md`) **when** in P1 **then** paste-back offers no write; Billy appends by hand until F-016 (M4) | comp | M3 |
| AC-F012-15 | **Given** an ADR-set paste **then** it is split on `^# ADR-\d{3}:` into `docs/adr/ADR-NNN.md`, listing every target and diffing existing files first | unit | M4 |
| AC-F012-16 | **Given** a chat stage with a valid packet **then** close-out runs attest (F-015) → F.2 lessons (F-016) → Sign (F-014), in that order | comp | M4 |

### F-013 · Packet engine

**US-F013-1.** As Billy, I want every packet validated against the stage's format, so that a bad packet is caught in its own stage.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F013-01 | **Given** one passing and one failing fixture per rule (header form and N matching the file name; M = next applicable stage; required fields; `OPEN ITEMS:` exactly `none`; `APPROVED BY:` unsigned or `<name> on <YYYY-MM-DD>`; Stage 4 `BLUEPRINT VERSION` = lock) **when** validated **then** each passes or fails with that rule's problem | unit | M1 |
| AC-F013-02 | **Given** a stage after a not-applicable stage **when** its header names the skipped stage as M **then** rule 1 fails | unit | M1 |
| AC-F013-03 | **Given** arbitrary packet text, including HTML, flag-like lines and instructions **when** validated **then** the validator never panics, every problem has a line, and content never changes Foreman's behavior beyond the parsed fields | prop | M1 |
| AC-F013-16 | **Given** a packet, lessons file or decision file larger than 1 MB, not a regular file, or with more than one hard link **then** Foreman refuses to read it and says why | int | M1 |

**US-F013-2.** As Billy, I want packets Claude Code writes to appear in Foreman on their own, so that I don't go looking for them.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F013-04 | **Given** the watcher **then** it covers `docs/handoff/` and every runnable step's declared `out` paths, and ignores `.git`, `node_modules`, `target`, `.claude/worktrees/**`, `*.foreman-tmp` and the secrets patterns (SR-50) | int | M3 |
| AC-F013-05 | **Given** a symlink inside the project pointing outside it, or a watch root (or one of its ancestors up to the project root) that is a symlink **then** the watcher refuses it: the root is checked with `lstat` before watching and on every event, and each event path is resolved again before it is read | int | M3 |
| AC-F013-06 | **Given** a packet edited from outside (editor, Claude, git checkout) **then** it is re-validated and the problems shown update | int | M3 |
| AC-F013-07 | **Given** a fake writer creates a packet **then** it is visible and validated in the UI at p95 ≤ 1 s | int | M3 |
| AC-F013-08 | **Given** a CLI-written packet needing normalization **then** the changes are shown and written atomically only after Billy confirms | int | M3 |
| AC-F013-09 | **Given** a packet that validates with `APPROVED BY: <name> on <YYYY-MM-DD>` (signed by hand in P1) **then** the stage counts as signed only after Billy confirms it in a native dialog that binds the packet's SHA-256 (`packet_signature_confirm`, DR-135); then the Next step moves to the next applicable stage. Any later change to the file voids the confirmation; Cancel leaves the stage unsigned | int | M3 |
| AC-F013-10 | **Given** the watcher fails to start or stops **then** a warning shows, steps still run, and packets are validated when Billy opens the stage | int | M3 |

**US-F013-3.** As Billy, I want each packet problem paired with a fix action, so that fixing is one click.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F013-11 | **Given** a CLI packet with problems **when** Billy clicks Repair **then** the stage's chain is re-armed (by his click, with the re-arm rules) with "Fix only these problems in docs/handoff/stage-NN.md: … Do not change anything else." | int | M4 |
| AC-F013-12 | **Given** a problem **then** Open in editor is offered; for chat packets Copy fix request is offered | comp | M4 |
| AC-F013-13 | **Given** `OPEN ITEMS:` isn't `none` **then** the view lists them and states the two allowed outcomes: resolve them, or move them to KNOWN LIMITS with a ticket ID | comp | M4 |
| AC-F013-14 | **Given** a step printed its packet instead of writing it **when** the sensor has `last_assistant_message` **then** Capture from session saves it through the paste-back path; without the sensor, paste-back is offered | int | M4 |
| AC-F013-15 | **Given** recorded skips and not-applicable items for a stage **when** signing **then** each appears under `SKIPPED:` | unit | M4 |

### F-014 · Signing and provenance

**US-F014-1.** As Billy, I want Sign enabled only when the stage's close-out is complete, so that no stage is signed half-done.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F014-01 | **Given** any combination of gate state, lessons entry and chain state **then** Sign is enabled only when the gate is passed, a lessons entry for the stage exists ("clean stage" counts), and the stage's chain is idle or ended (or Billy confirms); otherwise it is disabled with the reason | prop | M4 |
| AC-F014-02 | **Given** the sign dialog **then** it shows the name (DRI from Settings) and date, the branch (current branch by default, or Sign on another branch), the fields Foreman will add (`SKIPPED:`, `SUBSTITUTIONS:`, `ATTESTED:`) as a diff, the content hash ("Signing stage-05.md @ a3f9c1"), and for Stage 2 Confirm conditions | comp | M4 |
| AC-F014-03 | **Given** git isn't found **then** Sign is disabled and the reason names git | comp | M4 |

**US-F014-2.** As Billy, I want signing to commit exactly the stage's files and prove it, so that the record can't be swept up or rewritten.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F014-04 | **Given** the packet changed after Billy reviewed it **when** he signs **then** signing is refused and the dialog re-renders | int | M4 |
| AC-F014-05 | **Given** signing **then** phase 1 writes APPROVED BY and the added fields atomically and writes `.foreman/stages/stage-NN.json` (step states, run IDs, intended and observed models, efforts and modes, prompt SHA-256s, gate results, attestations, durations, packet hash after signing, session UUIDs) | int | M4 |
| AC-F014-06 | **Given** a repo with unrelated staged and unstaged changes **when** signing on the current branch **then** the commit "handoff: stage NN" is made with plumbing only, in this order: a temporary index from HEAD, `hash-object -w --no-filters` per path, `update-index --cacheinfo`, `write-tree`, `commit-tree --no-gpg-sign`, `update-ref HEAD <new> <old>`, then `update-index --cacheinfo` on the real index for those paths only. It contains only the packet, attachments, stage record, decision files with pending changes and `docs/lessons.md` if changed, and the unrelated changes stay as they were. **Given** `index.lock` is held during the real-index refresh **then** Foreman retries 5 times, 200 ms apart; if it still fails, the commit stands, `indexRefreshPending` is recorded, the banner "Index not refreshed; don't commit until this clears" shows, and the retry on the next open or focus clears it | int | M4 |
| AC-F014-07 | **Given** a repo with lint-staged + Prettier hooks, a filter driver and a repo-scope gpg program **when** signing **then** no hook, filter or gpg program runs (all git use goes through `git::run`, AC-XC-04), this is recorded in the stage record, and read-back passes | int | M4 |
| AC-F014-08 | **Given** a new untracked packet and stage record **when** signing **then** both are committed | int | M4 |
| AC-F014-09 | **Given** Sign on another branch **then** the same plumbing runs against that branch's tip (temporary `GIT_INDEX_FILE`, `hash-object -w --no-filters`, `update-index --cacheinfo`, `write-tree`, `commit-tree --no-gpg-sign`, `update-ref`); HEAD, the index and the working tree are unchanged; nothing is written to the working tree; the stage shows "signed on <branch> (not merged)" and the resolver reads the packet from that ref | int | M4 |
| AC-F014-10 | **Given** a commit whose content differs from what Foreman wrote (a mismatch injected in the fixture) **when** every path is read back from the commit **then** Foreman shows "Your repo's commit hooks changed the packet" with the diff | int | M4 |
| AC-F014-11 | **Given** the git commit fails **then** the stage is `signed (commit pending)` with stderr, Retry commit and Open terminal here; the stamp runs only after read-back passes | int | M4 |
| AC-F014-12 | **Given** `commit.gpgSign` or a gpg program set at any scope (repo, global or system) **then** it is never used: every commit is `commit-tree --no-gpg-sign` in v1, and Doctor notes that Foreman's commits aren't GPG-signed (DR-144); **given** any git call that hangs **then** the 60 s timeout ends the wait with a clear message | int | M4 |
| AC-F014-13 | **Given** Foreman is killed between phase 1 and phase 2 **when** it restarts **then** the stage shows `signed (commit pending)` with Retry commit | int | M4 |
| AC-F014-14 | **Given** decision changes pending in app data after Stage 5 is signed **when** the next stage is signed **then** they are written and committed in that signing commit | int | M4 |

**US-F014-3.** As Billy, I want a signed packet changed later to be flagged, so that the record stays authoritative.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F014-15 | **Given** a signed packet changes on disk **then** Today shows a tamper item with the diff, the resolver uses the committed version, and Restore committed version is one click | int | M4 |
| AC-F014-16 | **Given** a stage record on an unmerged branch **then** it reads as "signed on handoff/stage-05 (not merged)" | int | M4 |

### F-015 · Gates

**US-F015-1.** As Billy, in phase 1, I want a checklist of the close-out sequence for each stage, so that I close stages by hand in the right order.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F015-01 | **Given** a stage whose packet is valid in a P1 build **then** the stage shows a `manual` close-out checklist step listing the sequence for its stage type (Master Spec §8: chat stages; Stage 2; Stages 5–17) | comp | M3 |

**US-F015-2.** As Billy, I want each gate item tied to evidence, so that no gate passes from memory.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F015-02 | **Given** an `attest` item **when** Billy marks it Done **then** it needs a note of at least 15 characters not repeated within the gate (a link is optional); **when** he marks it Not applicable **then** it needs a reason; the item's "how to check" text sits next to it | unit | M4 |
| AC-F015-03 | **Given** a `time:<n>d` item **then** it is available only after its date | unit | M4 |
| AC-F015-04 | **Given** Gate A **when** Billy runs it **then** it runs in the terminal in its own chain; Gates B and C continue that chain (`chain=gate.A`) | int | M4 |
| AC-F015-05 | **Given** the sensor's last message **when** Billy clicks Import results **then** PASS/FAIL is proposed per item by matching item text, and each needs his confirmation; without the sensor he marks each item | int | M4 |
| AC-F015-06 | **Given** a gate **then** items are ordered FAIL, then waiting attest items, then PASS (collapsed) | comp | M4 |
| AC-F015-07 | **Given** a FAIL **when** Billy clicks Fix this **then** a new CLI chain opens (Opus 5.5 · high · Plan, then Accept edits) prefilled with the item and evidence; on the same item's third failure the F.4 prompt is shown | int | M4 |
| AC-F015-08 | **Given** the gate state machine `not-ready → A → results → B → packet-valid → C → lessons-found → signable` **then** every out-of-order transition is rejected | unit | M4 |
| AC-F015-09 | **Given** not-applicable items **then** they are excluded from the gate | unit | M4 |
| AC-F015-10 | **Given** any gate **then** it can never reach `signable` with an applicable item unresolved | prop | M4 |

### F-016 · Lessons capture

**US-F016-1.** As Billy, I want each stage's lessons captured and checked, so that lessons are never skipped.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F016-01 | **Given** a chat stage's F.2 step **then** its prompt has the lessons-entry format from `docs/lessons.md`'s header appended | unit | M4 |
| AC-F016-02 | **Given** an F.2 paste-back **then** entries are appended to `docs/lessons.md` with the next free `L-NNN` after a diff; the header is preserved | int | M4 |
| AC-F016-03 | **Given** `docs/lessons.md` changed on disk since Foreman read it **when** appending **then** the write is refused, the file re-read and the diff shown again | int | M4 |
| AC-F016-04 | **Given** `docs/lessons.md` is missing **when** appending **then** nothing is written and the message says the file is missing | int | M4 |
| AC-F016-05 | **Given** Stage 2's F.2 followup in the 2.2 chain (`chain=@stage`), or Gate C for Stages 5–17 **when** Claude writes an entry **then** Foreman detects it for that stage | int | M4 |
| AC-F016-06 | **Given** entries **then** the stage's requirement is met only by `## L-NNN · Stage N · …` for that stage, a "clean stage" entry in that form included; an entry for another stage doesn't count | unit | M4 |

### F-017 · Today and Do next

**US-F017-1.** As Billy, I want to see each project's next step and start it in two keystrokes, so that I never search for what's next.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F017-01 | **Given** registered projects **when** Today opens **then** each row shows the project's Next step (the first runnable step on the applicable path) with its spec line and primary action, and the top row has focus | comp | M2 |
| AC-F017-02 | **Given** any screen **when** Billy presses ⌘⇧↵ **then** the top project's next step starts through the same preflight; blockers are shown and nothing launches | comp | M2 |
| AC-F017-03 | **Given** no projects **when** Today opens **then** it shows "Start your first project" and the button opens the setup form | comp | M2 |
| AC-F017-04 | **Given** a project with no runnable step **then** its row says why (e.g. "Decide payments to continue", "Waiting until <date>", "Sign Stage 2 first", or that all steps are done) | comp | M2 |
| AC-F017-05 | **Given** a registered project whose folder is missing **then** its row shows "Project folder not found" and offers nothing runnable | comp | M2 |
| AC-F017-06 | **Given** a launch from Do next **then** its time-to-start and entry point are recorded (AC-F026-01) | int | M3 |

**US-F017-2.** As Billy, I want an inbox of everything waiting on me across projects, so that nothing stalls unseen.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F017-07 | **Given** items of every kind **then** Today orders them `session-blocked`, `session-your-turn`, `packet-landed`/`packet-invalid`, `gate-failed`/`gate-signable`, `outside-change`, `tamper`, `chat-awaiting-paste`, `wait-elapsed`, `interrupted-session`, `step-ready`; J/K move and Enter runs the primary action | comp | M4 |
| AC-F017-08 | **Given** a chat prompt copied more than 10 minutes ago without paste-back **then** a `chat-awaiting-paste` item appears | unit | M4 |
| AC-F017-09 | **Given** Foreman isn't focused **when** a `session-blocked` or `packet-landed` item appears **then** a macOS notification fires with Foreman-generated text only, and clicking it only focuses Foreman; the Dock badge counts the first four kinds | manual | M4 |
| AC-F017-10 | **Given** projects **then** each sidebar entry shows a status dot and ⌘1–⌘9 open pinned projects | comp | M4 |

### F-018 · Projects and setup

**US-F018-1.** As Billy, I want to set up a project with one form that shows every file it will write, so that setup never surprises me.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F018-01 | **Given** the setup form **then** it asks for name, folder (new or existing), variables (placeholder values count as empty), the eight early conditions (tri-state), the claude.ai Project URL, and the blueprint pin | comp | M2 |
| AC-F018-02 | **Given** a filled form **when** Billy opens Review **then** it lists every file Foreman will create, leave alone or refuse to overwrite, and the actual writes match the list exactly | int | M2 |
| AC-F018-03 | **Given** Create **then** Foreman creates `.foreman/`, `docs/handoff/`, `docs/lessons.md` (template header), `docs/templates/` (packet, CR and postmortem templates from the pinned kit), `docs/change-requests/`, `docs/postmortems/` and `spec/`; it doesn't write council agents and doesn't commit | int | M2 |
| AC-F018-04 | **Given** an existing `docs/lessons.md` (or any listed file that exists) **then** it is never overwritten | int | M2 |
| AC-F018-05 | **Given** a folder that isn't a git repo **then** Foreman runs `git init` with the fixed git invocation; **given** git isn't found **then** setup is blocked with the reason | int | M2 |
| AC-F018-06 | **Given** a folder inside another registered project, the blueprint repo itself, or a folder that isn't writable **then** setup is refused before any write, with the reason | int | M2 |
| AC-F018-07 | **Given** a folder whose `.foreman/` has a newer schema **then** the project opens read-only; **given** a supported schema **then** the project is registered without writing any file | int | M2 |

**US-F018-2.** As Billy, I want a short wizard that shows the tailored path before I create the project, so that I see which stages apply and why.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F018-08 | **Given** the wizard **then** it has four steps (name and folder → variables, prefilled from Settings' global defaults → conditions, each with one line on what it changes → review of the tailored path: which stages apply, which don't, and why, plus the file writes); Create lands on the first action with focus | comp | M4 |

### F-019 · First run, Doctor, contract probes

**US-F019-1.** As Billy, I want first run to set up the blueprint, my name and the environment checks, so that my first project starts on solid ground.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F019-01 | **Given** no settings **when** Foreman starts **then** first run goes: blueprint repo (auto-detect `~/claude-blueprint`, `~/Developer/claude-blueprint`, else Choose… through the Rust dialog) → template check → DRI name (prefilled from `git config user.name`) → Doctor → Start your first project | comp | M2 |
| AC-F019-02 | **Given** the template check step **then** it shows the blueprint version and the parse diagnostics; the full Template health screen arrives in M4 | comp | M2 |
| AC-F019-03 | **Given** auto-detection **then** only the fixed candidate paths are checked; no folder is indexed | unit | M2 |
| AC-F019-04 | **Given** `claude` missing, below the floor, logged out, or with a wrong auth method; or `git` missing **when** Doctor runs **then** each check fails with the version found (when there is one) and how to fix it; project creation stays possible and the launch blockers (F-008) still apply | int | M2 |
| AC-F019-05 | **Given** a fake `claude` reporting each auth method **then** Doctor passes only `claude.ai` and `oauth_token` | int | M2 |
| AC-F019-06 | **Given** git is found **then** Doctor shows its version; no git version floor is enforced | int | M2 |
| AC-F019-07 | **Given** Foreman starts or a launch fails **then** Doctor runs asynchronously and the footer dot shows its status | comp | M2 |
| AC-F019-08 | **Given** first run **then** Foreman opens `claude` in the fixed `<app-data>/probe/` folder in a terminal tab so Billy can accept the trust dialog once | int | M4 |

**US-F019-2.** As Billy, I want Doctor to check everything a blueprint run depends on, so that breaks show up in Doctor, not in a terminal tab.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F019-09 | **Given** the full Doctor **then** it shows the capability table (feature → minimum → found → steps it blocks) | comp | M4 |
| AC-F019-10 | **Given** user settings **then** Doctor reports the effective `cleanupPeriodDays` with a recommendation (never writing it), `disableAllHooks` set, Appendix A artifacts, and broad allow rules (`Bash(*)`, `pnpm exec *`, `npx *`, interpreters) as warnings | int | M4 |
| AC-F019-11 | **Given** toolchain probes **then** `git`, `node`, `pnpm`, `cargo` are run from fixed candidate paths with `--version` only | int | M4 |
| AC-F019-12 | **Given** Billy clicks Capture my login shell PATH **then** `$SHELL -l -c 'printf %s "$PATH"'` runs once, only if `$SHELL` is listed in `/etc/shells`; the result is shown and stored only after he accepts; no other code path spawns a shell | int | M4 |
| AC-F019-13 | **Given** S-12 took its fail branch **when** first run reaches Doctor **then** login-PATH capture is a required step (this criterion moves to M2) | comp | M4 |
| AC-F019-14 | **Given** a Finder-launched build **then** `claude`, `git`, `node`, `pnpm` and `cargo` resolve with the PATH built per Master Spec §6, which Doctor shows | manual | M4 |

**US-F019-3.** As Billy, I want Foreman to verify, at no token cost, that a new Claude Code version still accepts its flags, so that an upgrade can't silently break launches.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F019-15 | **Given** `claude -v` changed **when** Billy next launches **then** the probes run first in `<app-data>/probe/` with a 20 s timeout: `claude --init-only --settings <probe settings> --session-id <fresh uuid> --model <m> --effort <e> --permission-mode <mode> --name <probe>` with a marker `SessionStart` hook, and `claude --init-only --resume <random uuid>` (passes on "no conversation found") | int | M4 |
| AC-F019-16 | **Given** each outcome **then** pass stores "contract verified on X.Y.Z"; a flag failure disables only the dependent feature and names it; inconclusive (timeout, trust prompt, anything else) keeps the last verified state and warns, and the launch proceeds | int | M4 |
| AC-F019-17 | **Given** any probe **then** it never runs in a project folder and never uses anything but `--init-only`; `<app-data>/probe/` is recreated (0700, Foreman's files only) before each run | int | M4 |

### F-020 · State, persistence, resilience

**US-F020-1.** As Billy, I want every write to be atomic and never to clobber a newer file, so that Foreman can't lose or corrupt my work.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F020-01 | **Given** a write killed at every step (temp written, `sync_all`, rename, directory fsync) **then** the target holds the complete old or complete new content | prop | M1 |
| AC-F020-02 | **Given** a write **then** its temp file is `.<name>.<random 64-bit hex>.foreman-tmp`, created through the directory handle with `O_CREAT\|O_EXCL\|O_NOFOLLOW` and mode 0600, followed by `sync_all`, `renameat` and a directory fsync; a planted symlink at the temp path is refused | prop | M1 |
| AC-F020-13 | **Given** a normal repo, a linked worktree, and a `.git` file pointing outside the project **then** the exclude writer appends `.*.foreman-tmp` once (idempotently) to the path from `git rev-parse --git-path info/exclude`, refuses symlinks, and refuses paths outside the project root or the repo's common git directory; it is the only write under `.git` | int | M1 |
| AC-F020-03 | **Given** interleaved external edits **when** Foreman writes **then** it never overwrites a file changed after its last read; the write is refused and the conflict shown | prop | M1 |
| AC-F020-04 | **Given** a disk-full or permission error during a write **then** the original file is intact and the error names the file | int | M1 |
| AC-F020-05 | **Given** a decision file that fails to parse **then** the project opens read-only with the error and Open in editor; the file is never overwritten | int | M1 |
| AC-F020-06 | **Given** an older `schemaVersion` **then** the migration runs; **given** a newer one **then** the project opens read-only | unit | M1 |
| AC-F020-07 | **Given** any JSON Foreman writes **then** keys and IDs are sorted and a parse–write round trip is byte-identical | prop | M1 |

**US-F020-2.** As Billy, I want decision changes committed precisely, so that the repo records every decision without stray files.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F020-08 | **Given** Stage 5 isn't signed and the repo has unrelated staged changes **when** a decision changes **then** Foreman commits only that decision file at that moment, using the signing plumbing and read-back (SR-42), and the unrelated changes stay as they were. **Given** an unborn HEAD (no commits yet) **then** the commit is the repo's first (`read-tree --empty`, `commit-tree` without `-p`, `update-ref HEAD <new> 0000…`). **Given** a detached HEAD **then** the commit is refused with "Check out a branch first" and the decision change stays pending | int | M1 |
| AC-F020-09 | **Given** Stage 5 is signed **when** a decision changes **then** it stays pending in app data and goes into the repo only with the next signing commit | int | M1 |

**US-F020-3.** As Billy, I want sessions and state to survive a crash, so that a force-quit costs me nothing.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F020-10 | **Given** Foreman is killed with a live fake-`claude` child **when** it restarts **then** the session is listed under Interrupted sessions with Resume (re-arm); zero sessions are lost, and no run is relaunched automatically | int | M3 |
| AC-F020-11 | **Given** app data is deleted **when** the project opens **then** stage and step status are rebuilt from the repo's packets (and from M4, stage records) within 1 minute; only unsigned in-progress markers and the run log are lost | int | M3 |
| AC-F020-12 | **Given** a corrupt `progress.json` **then** a clear error names it; the corrupt file is kept; Rebuild from repo writes the rebuilt state only after Billy confirms | int | M3 |

### F-021 · Path policy

**US-F021-1.** As Billy, I want one policy to guard every write Foreman makes, so that repo content can't direct Foreman to write somewhere dangerous.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F021-01 | **Given** the golden suite (`..`, absolute paths, `.git/hooks/pre-commit`, `.GIT/hooks/pre-commit`, `spec/.claude/settings.json`, `spec/.mcp.json`, `.MCP.json`, `spec/CLAUDE.md`, `claude.md` below the root, a symlinked `docs/handoff`, Unicode lookalikes, and a REPO FILES list containing each) **then** every case is refused with the rule that refused it | unit | M1 |
| AC-F021-08 | **Given** case and Unicode-normalization variants of every denied component and file name, on a case-insensitive and on a case-sensitive volume **then** comparisons use NFC and full case folding per component, and also check the name the file system reports for an existing entry, so every variant is refused | int | M1 |
| AC-F021-02 | **Given** any path with NUL, control or bidi characters, a denied component at any depth (`.git`, `.claude`, `.husky`, `.vscode`, `.idea`, `.github`, `.foreman`), or a denied file name (`.mcp.json`, `CLAUDE.local.md`, `.envrc`, `.gitmodules`, `.gitattributes`, `.npmrc`) **then** it is refused | prop | M1 |
| AC-F021-03 | **Given** the `council-setup` destinations under `spec/.claude/agents/` and the decision-file writers under `.foreman/` **then** they are the only allowed exceptions | unit | M1 |
| AC-F021-04 | **Given** a directory component that is a symlink **then** resolution through the directory-handle API refuses the write | int | M1 |
| AC-F021-05 | **Given** any generated path **when** accepted **then** its final location is inside the project root or app data | prop | M1 |
| AC-F021-06 | **Given** `CLAUDE.md` **then** it is writable only at the repo root, only through the assembler, after a native confirmation raised by Rust that says "Claude Code loads this file in every session". *int:* a scripted native dialog; Cancel writes nothing. *comp:* the assembler panel shows the diff as context and has no approve path of its own | int, comp | M4 |
| AC-F021-07 | **Given** a read of repo content **then** it is allowed only in registered project folders, the blueprint repo and app data (plus the fixed list in AC-XC-30), resolves component by component, and refuses a symlink at any component (`O_NOFOLLOW` on the last) | int | M1 |

### F-022 · Outside-change review

**US-F022-1.** As Billy, I want changes to my decision files from outside Foreman held until I accept them, so that the governed agent can't rewrite the rules.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F022-01 | **Given** each decision file (`project.json` decision fields: conditions, model policy, verify commands; `overlay.json`: skips, additions; `blueprint.lock.json`) edited from outside **when** the project loads **then** it is Pending outside change and Foreman's effective config is the last approved one (one case per file) | int | M1 |
| AC-F022-02 | **Given** a pending change **then** the review shows the diff, redacted as in AC-F008-23, with Accept and Reject; Accept raises a native dialog in Rust ("Accept this change to <file>? It changes: <decision keys>") and only then stores the new approved hash (and, before Stage 5 is signed, commits by plumbing); Reject restores the approved content through an atomic write. *int:* a scripted native dialog; Cancel leaves the approved copy, hash and pending change unchanged. *comp:* the review screen shows the redacted diff as context | int, comp | M2 |
| AC-F022-03 | **Given** only variables changed **then** they reload with a toast and no review | int | M1 |
| AC-F022-04 | **Given** Foreman is open **when** Claude, an editor or a git checkout changes a decision file **then** the pending review appears without a restart | int | M3 |
| AC-F022-05 | **Given** the file changes again while the review is open **when** Billy clicks Reject or Accept **then** the action is refused and the new diff is shown | int | M2 |

### F-023 · Knowledge manifest and build-package assembler

**US-F023-1.** As Billy, I want to see which files belong in the claude.ai Project now, so that chat stages always have the right knowledge.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F023-01 | **Given** signed packets and declared chat outputs **then** the manifest lists each with a checkbox, Reveal in Finder and Copy contents, and Stage 4 steps show "Knowledge ready X of Y" by the Copy button | comp | M4 |
| AC-F023-02 | **Given** no signed packets or outputs **then** the manifest shows an empty state pointing to the first step that produces one | comp | M4 |

**US-F023-2.** As Billy, I want the 4.8 package assembled from its REPO FILES list, so that the Stage 4 → 5 file job has a checklist.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F023-03 | **Given** a 4.8 packet fixture **then** REPO FILES is parsed and each path shows exists, missing or empty, with Paste into (clipboard to file through the path policy, with an overwrite diff) and Reveal | int | M4 |
| AC-F023-04 | **Given** a malicious REPO FILES entry for each path-policy case **then** it is refused | int | M4 |
| AC-F023-05 | **Given** any row missing or empty **then** the attest item "the package is complete" isn't offered | comp | M4 |
| AC-F023-06 | **Given** the panel **then** it asks the late conditions that are still undecided, each with a proposal from path evidence per DR-063 (e.g. `docs/migrations/` present → `has_database = yes`); with no evidence a condition stays `undecided` (never prefilled `no`); each proposal takes effect only when Billy confirms it, and each is editable | int | M4 |

### F-024 · Desktop handoff

**US-F024-1.** As Billy, I want Desktop sessions started with the step's prompt and settings in view, so that Desktop doesn't run with whatever I used last.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F024-01 | **Given** a `surface=desktop` step **when** Billy clicks Open in Desktop **then** the prompt is copied and `claude --desktop` runs in the step's cwd with no other flags, through the same allowlisted path, environment allowlist and auth check as F-008; the step shows chips for the model, effort and mode to pick | int | M4 |
| AC-F024-02 | **Given** a CLI chain **when** Billy clicks Open this session in Desktop **then** `claude --desktop --resume <uuid>` runs and the chain becomes `handed-off` | int | M4 |
| AC-F024-03 | **Given** S-23's branch **then** shared: the next continuing step forks (AC-F010-15); copied: no fork and the CLI chain continues | int | M4 |
| AC-F024-04 | **Given** `claude` below 2.1.285 **then** both Desktop actions are blocked by the floor | int | M4 |

### F-025 · Add to a stage, this project only

**US-F025-1.** As Billy, I want to add a prompt or inspection item to one project, so that extra steps aren't lost in side notes.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F025-01 | **Given** the + insert point **when** Billy adds a prompt **then** it copies the run config of a step he picks, lets him edit text and config, and is stored in `overlay.json` as `x-<stage>-<n>`, tag `optional` by default (he may make it `core` for this project) | int | M4 |
| AC-F025-02 | **Given** an added inspection item **then** its kind is `attest` | unit | M4 |
| AC-F025-03 | **Given** an added step **then** it shows "added for this project" and always confirms before running | comp | M4 |
| AC-F025-04 | **Given** overlays **then** they round-trip through JSON unchanged, added steps appear in the applicable path, and packets list them | prop | M4 |
| AC-F025-05 | **Given** an added step with `bypassPermissions` or an `out` path the policy refuses **then** it is rejected | unit | M4 |

### F-026 · Run log and metric panel

**US-F026-1.** As Billy, I want every run timed the same way as my by-hand baseline, so that the kill check is a measurement, not a feeling.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F026-01 | **Given** a run **then** `runs/<runId>/run.json` records argv, cwd, step, chain, timings, outcome, prompt SHA-256, time-to-start, the entry point (Do next, palette, stage view), the number of run-input interventions, and any Copy prompt on a CLI step; a line is added to `runs/index-YYYY-MM.jsonl` | int | M3 |
| AC-F026-02 | **Given** time-to-start **then** it runs from focusing Foreman or pressing Do next to the spawn (CLI), or, for chat, to the moment the filled prompt is on the clipboard and the target chat is open (when Copy and open returns); the by-hand baseline uses the same end points (AC-KC-01) | unit | M3 |
| AC-F026-03 | **Given** a run **then** `run.json`, the monthly index and the Run log screen store the argv with its final positional prompt replaced by `<prompt sha256:…>`, never the prompt text, and only the names of environment variables, never their values | int | M2 |
| AC-F026-04 | **Given** Export metrics **then** it writes, through the Rust save dialog, a Markdown summary of both measures (time to start and active time, each per step) per surface and per stage, in the table layout of `docs/metrics-baseline.md`, including manual-edit proxies | unit | M3 |
| AC-F026-05 | **Given** active time per step in P1 (no sensor) **then** only `wait`-step time is excluded automatically; the export shows elapsed time and excluded waits separately so Billy excludes Claude-working time the same way as in the baseline | unit | M3 |
| AC-F026-06 | **Given** no runs **when** Billy exports **then** the tables are written with a note that no runs were recorded | unit | M3 |

**US-F026-2.** As Billy, I want to search past runs and see the metrics live, so that I can resume work and watch the success metric.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F026-07 | **Given** the Run log screen **then** it searches every run with argv, session name and outcome, offers Resume where the transcript exists, and shows an empty state when there are no runs | comp | M4 |
| AC-F026-08 | **Given** 100,000 index entries **then** the panel reads the index and opens within the project-switch target | bench | M4 |
| AC-F026-09 | **Given** the Run log screen's metrics strip (`docs/design/mockups/run-log.html`) **then** it shows median and p95 time-to-start and median active time per step for each surface, manual-edit count per stage, stages run through Foreman, and the comparison with `docs/metrics-baseline.md` (`metrics_view`), and the numbers match the run log and Export metrics | int | M4 |
| AC-F026-10 | **Given** output tails older than 90 days **then** they are pruned; **when** Billy clicks Clear run history **then** run records, tails and events are removed | int | M4 |

### F-027 · Run sheet export

**US-F027-1.** As Billy, I want any stage exported as a Markdown run sheet, so that the process never depends on Foreman.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F027-01 | **Given** Stage 2 **when** exported **then** the sheet matches the golden file: each applicable step's resolved prompt, exact argv (or chat chips), run inputs as filled, and the gate items with their kinds | unit | M4 |
| AC-F027-02 | **Given** export **then** the file is written only to the path chosen in the Rust save dialog; Cancel writes nothing | int | M4 |
| AC-F027-03 | **Given** unresolved required tokens **then** the sheet is still exported, with each unresolved token marked | unit | M4 |

### F-028 · Command palette and keyboard

**US-F028-1.** As Billy, I want the P1 loop fully on the keyboard, so that starting steps is fast and accessible.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F028-01 | **Given** the app **then** ⌘↵ (run selected step), ⌘⇧↵ (Do next), ⌘⇧V (paste back), ⌘D (mark chat step done), ⌃` (terminal drawer) and ⌘⇧E (return focus from terminal) each invoke their action over IPC | comp | M2 |
| AC-F028-02 | **Given** a keyboard-only script **then** it completes the full P1 loop: create a project, run a chat step, paste back, run a CLI step, re-arm a continuing step, return focus from the terminal | e2e | M3 |
| AC-F028-03 | **Given** a shortcut whose action doesn't apply to the selection **then** nothing happens except a one-line reason | comp | M2 |
| AC-F028-04 | **Given** ⌘K from anywhere, including the terminal **then** the palette opens over projects, stages, steps and every action; a query with no match shows an empty state | comp | M4 |
| AC-F028-05 | **Given** the remaining shortcuts of Master Spec A.7 **then** each invokes its action | comp | M4 |

### F-029 · Settings

**US-F029-1.** As Billy, I want to set tool paths and my name once, so that every launch and signature uses them.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F029-01 | **Given** the `claude` and `git` path settings **then** they are auto-detected and overridable; a relative, missing or non-executable path, or one inside a registered project, is refused with the reason | int | M2 |
| AC-F029-02 | **Given** any setting **when** saved **then** it persists across restarts through an atomic write | int | M2 |
| AC-F029-03 | **Given** `settings.json` is missing **then** Foreman starts with defaults and auto-detection; **given** it changed on disk since read **then** Foreman's write is refused and the file reloaded | int | M2 |
| AC-F029-04 | **Given** the P2 settings (blueprint repo path, global variable defaults, theme, terminal font size, scrollback cap, environment pass-through list, captured login PATH view, re-capture and clear, Clear run history) **then** each persists and takes effect | comp | M4 |
| AC-F029-05 | **Given** the pass-through list **when** it names an always-removed variable **then** that variable is still removed | unit | M4 |
| AC-F029-06 | **Given** Billy removes a project **then** its app-data folder, including its redacted config snapshots, is deleted and the repo is untouched | int | M4 |
| AC-F029-07 | **Given** the editor setting (an app bundle ID) **when** Billy uses any Open in editor action **then** it opens the file with that editor (AC-XC-28); with no editor set, Open in editor is disabled and says to set one | comp | M2 |

### F-030 · Stage workspace and sheet strip

**US-F030-1.** As Billy, I want each stage shown as steps with their exact settings and resolved prompts, so that I can read and run a stage at a glance.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F030-01 | **Given** a selected step **then** the workspace shows exactly the resolver's output for it, its step number, spec line, run-input form, attention pills and a primary action matching its surface ("Run in terminal", "Copy for chat"; "Open in Desktop" from M4) | comp | M2 |
| AC-F030-02 | **Given** not-applicable, core, `undecided` and `needs-annotation` rows **then** none can be run; each states why in one line; a core row never shows Skip | comp | M2 |
| AC-F030-03 | **Given** a project open read-only (lock mismatch, invalid decision file, newer schema, missing blueprint) **then** a banner states the reason and no Run action is enabled | comp | M2 |
| AC-F030-04 | **Given** hostile repo content (`<script>`, `<img onerror>`, raw HTML, links) in a packet, prompt, overlay or ticket **when** shown in any view **then** it renders as escaped text. Markdown crosses IPC only as a typed node tree, never as an HTML string, and lint rejects `{@html}` (`svelte/no-at-html-tags`) and `innerHTML`, `outerHTML` and `insertAdjacentHTML` (`no-restricted-properties`) | comp | M2 |
| AC-F030-05 | **Given** the terminal drawer **then** ⌃` toggles it and it remembers its height | comp | M2 |

**US-F030-2.** As Billy, I want the sheet strip, title block, inspector and stamp, so that the whole project reads like a drawing set.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-F030-06 | **Given** a project **then** the sheet strip shows 19 sheets grouped by phase, each with its phase edge and a status mark by shape and color (locked, ready, in progress, inspecting, signed, not applicable, needs annotation, undecided); hover shows the stage name; click opens it | comp | M5 |
| AC-F030-07 | **Given** a stage **then** the title block shows sheet number, revision (pinned blueprint version), DRI, status and date signed | comp | M5 |
| AC-F030-08 | **Given** a selected step, gate or packet **then** the inspector shows that context; ⌘⌥I collapses it | comp | M5 |
| AC-F030-09 | **Given** read-back passes after signing **then** the stamp plays (220 ms) and the next stage turns ready; under Reduce Motion it doesn't animate | comp | M5 |
| AC-F030-10 | **Given** a 1280 × 800 window **then** the layout works; narrower, the inspector collapses first, then the sidebar collapses to icons | comp | M5 |

---

## 4. Cross-cutting criteria

These criteria apply to Foreman as a whole, not to one feature.

**US-XC-1.** As Billy, I want Foreman to spawn only known programs, in known ways, so that it can't be turned into a shell.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-XC-01 | **Given** the codebase **then** clippy `disallowed-methods` rejects `std::process::Command::new` and `portable_pty::CommandBuilder::new` outside the `spawn` module; every spawn uses an argv array; no shell is constructed outside `doctor::capture_login_path` | ci | M2 |
| AC-XC-02 | **Given** a spawn request **then** relative paths, unlisted programs, paths inside a registered project, and symlinks into a writable project folder are rejected; the path is re-checked (exists, executable) before every spawn; the allowlist is `claude`, `git`, `/usr/bin/open` (only in AC-XC-28's forms) and `node`/`pnpm`/`cargo` with `--version` only | unit | M1 |
| AC-XC-03 | **Given** every argv Foreman builds **then** `-p`/`--print` never appears | ci | M2 |
| AC-XC-04 | **Given** every git call **then** it goes through `git::run` (clippy `disallowed-methods` elsewhere), with an environment from `env_clear()` plus `HOME`, a fixed `PATH`, `LANG`, `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0` (and `GIT_INDEX_FILE` only for Foreman's temporary index); `-c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat -c core.editor=true -c sequence.editor=true -c commit.gpgSign=false -c tag.gpgSign=false -c core.sshCommand=false -c credential.helper= -c protocol.allow=never`; (CR-002) For one call only, `update-index --cacheinfo` on the repo's real index, `-c core.attributesFile=/dev/null` follows those overrides and `GIT_ATTR_SOURCE=4b825dc642cb6eb9a060e54bf8d69288fbee4904` (the empty tree) is added to the environment; no other call gets either. That refresh is skipped, with `indexRefreshPending` recorded, while `$GIT_DIR/info/attributes` is non-empty. Only the plumbing operations of DR-140 (never `status`, `diff`, `add` or `commit`); blobs only via `hash-object -w --no-filters`; commits only via `commit-tree --no-gpg-sign`; and a 60 s timeout. A fixture repo's hooks, filter drivers and gpg program never run | int | M1 |

**US-XC-2.** As Billy, I want the webview treated as less trusted than Rust, so that rendered content can't reach the file system or processes.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-XC-05 | **Given** every IPC command **then** it takes exactly one request struct with `deny_unknown_fields` and no `flatten`; unknown fields are rejected; fuzzed input returns typed errors; lengths, enums and IDs are checked; a panic in a handler returns a typed error and is logged without its payload values | prop | M2 |
| AC-XC-06 | **Given** any IPC command **then** the webview can't supply a path, program, argv, URL or environment variable that Rust uses unvalidated; user-picked paths come from Rust-side dialogs | unit | M2 |
| AC-XC-07 | **Given** the built app **then** the served CSP header matches the production config, and an injected inline `<script>` raises `securitypolicyviolation` and doesn't run | e2e | M3 |
| AC-XC-08 | **Given** a request to open a URL Foreman generates **then** only `https://claude.ai/…` and `https://code.claude.com/docs/…` open; terminal links follow AC-F009-17; anything else is refused | unit | M3 |
| AC-XC-09 | **Given** the abuse cases from the threat model (packet, ticket, overlay or pasted text trying to change flags, destinations, modes or trigger actions) **then** each has a test and none succeeds | prop | M1 |
| AC-XC-10 | **Given** any write **then** `~/.claude/settings.json`, `~/.claude.json` and any `.claude/settings*.json` are never written | unit | M1 |

**US-XC-3.** As Billy, I want secrets and session content kept to the minimum and local, so that Foreman never leaks what a session printed.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-XC-11 | **Given** `.env*`, `*.pem`, `*.key`, `id_*`, `*.p12`, `.npmrc`, `.netrc`, `.ssh/`, `.aws/`, `.config/gcloud/`, directly or through a symlink **then** Foreman never opens, displays or watches them | int | M1 |
| AC-XC-12 | **Given** the dependency tree **then** `security-framework` and `keyring` are banned by `cargo deny`; auth state comes only from `claude auth status` | ci | M0 |
| AC-XC-13 | **Given** a full harness run **then** the `tracing` log (local, rotated 10 MB × 5) contains no prompt text, packet content, terminal output or environment values | unit | M4 |
| AC-XC-14 | **Given** app data **then** every file is `0600` and every folder `0700`, from the first write | int | M1 |
| AC-XC-15 | **Given** every file Foreman writes **then** it lands at the location and data class listed in `docs/contracts/files.md`, and validates against its schema | unit | M1 |
| AC-XC-16 | **Given** chat steps **then** Foreman stores no chat content beyond what Billy pastes into a packet or declared output, and no transcript copies | int | M3 |

**US-XC-4.** As Billy, I want Foreman's own failures never to stand between me and my work, and no Claude usage without my action.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-XC-17 | **Given** the watcher (M3), sensor, Doctor checks or probes (M4) fail **then** the step still runs by hand with one extra confirmation | int | M3 |
| AC-XC-18 | **Given** a crash **then** a local crash report and a run-log entry are written; nothing is uploaded; state is preserved | int | M4 |
| AC-XC-19 | **Given** any code path **then** no Claude run starts without Billy's explicit action, and probes use only `--init-only` | int | M3 |

**US-XC-6.** As Billy, I want the webview, file opening, clipboard and child processes locked down, so that content can't navigate, open programs or leak through them.

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-XC-27 | **Given** the main webview **then** it is built in Rust with `on_navigation` allowing only the app origin (and the dev server in debug builds) and `on_new_window` returning Deny; a navigation or `window.open` to a remote URL is refused | e2e | M2 |
| AC-XC-28 | **Given** an Open file request **then** it goes through one Rust function: the path passes the read policy, the extension is `.md`, `.json` or `.txt`, and it opens with `/usr/bin/open -b <editor bundle id> <absolute path>`, never the default app; a `.md` symlink to an `.app` and executable types are refused; Reveal uses `open -R` on a policy-checked path; Open terminal here opens Terminal by bundle ID; the opener plugin has `open_js_links_on_click(false)` | int | M2 |
| AC-XC-29 | **Given** clipboard reads **then** they happen only in Rust, only on Billy's explicit action, as text only, capped at 1 MB, never polled and never logged | unit | M3 |
| AC-XC-30 | **Given** reads outside project folders, the blueprint repo and app data **then** only the fixed list in one Rust module is read: user and managed settings; `mcpServers` in `~/.claude.json`; `~/.claude/{CLAUDE.md,agents,skills,commands,rules,output-styles}`; ancestor `CLAUDE.md`/`CLAUDE.local.md` and their `@` imports; transcript existence (never contents); `/etc/paths`, `/etc/paths.d/*`, `/etc/shells`; the blueprint auto-detect candidates; `git config user.name`. clippy `disallowed-methods` rejects `std::fs` everywhere except the modules allowed in `clippy.toml`: `foreman-core::store` (including `store::outside` and `store::git_exclude`), `foreman-app::logging` (the size-rotating diagnostics writer, `logs/` in app data only), the `foreman-sensor` crate (its event-file writer), and test code (`#[cfg(test)]`, `tests/`) | ci | M1 |
| AC-XC-31 | **Given** child processes **then** each starts from `env_clear()` plus the allowlist, the PTY child in its own session (`setsid`) and every non-PTY child in its own process group (`process_group(0)`), with no inherited fds above 2 and umask 022 (AC-F008-25); on quit, after the guard, children get SIGHUP; after a crash, runs show as Interrupted and are never relaunched automatically | int | M2 |
| AC-XC-32 | **Given** bidi controls, zero-width characters or C0/C1 controls in content **when** shown in any diff, confirmation or packet view **then** they appear as visible escapes and are flagged | comp | M2 |

**US-XC-5.** As Billy, I want Foreman fully usable by keyboard and VoiceOver in both themes, so that it meets WCAG 2.2 AA (AR-01).

| ID | Criterion | Test | M |
|---|---|---|---|
| AC-XC-20 | **Given** every screen in Night and Vellum **then** axe-core finds zero violations, and regressions fail `pnpm verify` | comp | M5 |
| AC-XC-21 | **Given** the full v1 workflow **then** it completes keyboard-only with visible focus (2 px ring, 2 px offset) everywhere and no keyboard trap | e2e | M5 |
| AC-XC-22 | **Given** VoiceOver **then** every control and status is labeled and status changes are announced through a live region; with Settings → Screen reader mode on, xterm's `screenReaderMode` is on and CLI launches add `--ax-screen-reader` | manual | M5 |
| AC-XC-23 | **Given** Reduce Motion, Reduce Transparency and Increase Contrast, each on **then** motion stops (`prefers-reduced-motion`); the webview reports `prefers-reduced-transparency: reduce` to Rust and window effects turn off; and the `prefers-contrast: more` tokens apply, with every text/background pair in the contrast table at ≥ 7:1 (checked by `contrast-check.py`) | unit, manual | M5 |
| AC-XC-24 | **Given** the design tokens **then** every text pair is ≥ 4.5:1 and large text and UI components ≥ 3:1 in both themes | unit | M5 |
| AC-XC-25 | **Given** any status **then** it is shown by shape plus color, never color alone | comp | M5 |
| AC-XC-26 | **Given** the components **then** a lint rule finds no hard-coded color, size or spacing, and every screen matches its mockup in both themes | ci | M5 |

---

## 5. Traceability: security, privacy and accessibility requirements

Source: `docs/requirements.md` (revised after the Stage 3 security review). Milestones follow its stage column (5 → M0, 6 → M1, 7 → M2, 8–9 → M3, 11–12 → M4, 13–17 → M5) unless a feature's phase moves the work. Those rows are marked ◆ and explained under the table. Under its rule that a control is built no later than the first code that needs it, an earlier milestone is always allowed.

| Req | Milestone | Proven by |
|---|---|---|
| SR-01 | M1 (git via `git::run`) · M2 ◆ | AC-XC-02, AC-F008-03, AC-F029-01 |
| SR-02 | M2 · M4 (login-PATH capture) | AC-XC-01, AC-F019-12 |
| SR-03 | M1 (template) · M2 (argv) · M4 (overlay additions) | AC-F001-10, AC-F008-14, AC-F025-05 |
| SR-04 | M2 | AC-F008-12, AC-F008-14 |
| SR-05 | M2 | AC-F008-15, AC-F029-05 |
| SR-06 | M3 | AC-F008-04, AC-F008-06, AC-F008-24 |
| SR-07 | M3 | AC-F008-05, AC-F008-07, AC-F008-22 |
| SR-08 | M2 (Doctor, environment) · M3 (launch) | AC-F019-05, AC-F008-08, AC-F008-09 |
| SR-09 | M2 · M4 (sensor hooks) | AC-F008-16, AC-F011-01 |
| SR-10 | M1 · M4 (sensor) | AC-XC-10, AC-F011-10 |
| SR-11 | v1.1 (F-101) | — |
| SR-12 | v1.1 (F-101) | — |
| SR-13 | M3 | AC-F010-04, AC-F010-05, AC-F009-04 |
| SR-14 | M3 · M4 (fork) | AC-F010-01, AC-F010-02, AC-F010-15 |
| SR-15 | M4 ◆ | AC-F011-01, AC-F011-02, AC-F011-03, AC-F011-04 |
| SR-16 | M2 | AC-XC-03, AC-F008-14 |
| SR-17 | M4 ◆ | AC-F019-15, AC-F019-16, AC-F019-17, AC-F008-21 |
| SR-18 | M3 · M4 (sensor) | AC-F010-07, AC-F011-08 |
| SR-19 | M2 | AC-F008-17, AC-F008-18, AC-F008-19 |
| SR-20 | M0 (config) · M3 (runtime) | AC-M0-08, AC-M0-14, AC-XC-07 |
| SR-21 | M0 | AC-M0-08 |
| SR-22 | M0 · M2 | AC-M0-09, AC-XC-06 |
| SR-23 | M0 | AC-M0-09 |
| SR-24 | M2 · M3 (`pty_write`) | AC-XC-05, AC-XC-06, AC-F009-19 |
| SR-24a | M2 (launch, decisions, settings) · M3 (re-arm, Terminate, config ack, outside signatures, terminal links) · M4 (signing, assembler, item kinds) | AC-F008-18, AC-F022-02, AC-F010-03, AC-F009-04, AC-F013-09, AC-F021-06, AC-F001-17; tests `ipc::security_confirmations_are_native`, `settings::security_fields_require_native_confirm` |
| SR-25 | M3 · M4 (Doctor links) | AC-XC-08, AC-F012-02, AC-F012-03 |
| SR-25a | M2 ◆ · M3 | AC-XC-28, AC-F029-07 |
| SR-26 | M2 · M3 · M5 (rendering re-check) | AC-F030-04, AC-XC-32, AC-F012-13, AC-XC-20 |
| SR-27 | M1 · M4 | AC-XC-09, AC-F006-09, AC-F013-03 |
| SR-28 | M1 (prompts) · M3 (terminal pastes) ◆ | AC-F006-17, AC-F009-15, AC-F009-16 |
| SR-29 | M2 (navigation) · M3 (terminal links) ◆ | AC-XC-27, AC-F009-17, AC-F009-18 |
| SR-30 | M1 · M3 · M4 | AC-F021-01, AC-F021-02, AC-F021-03, AC-F021-04, AC-F021-05, AC-F021-06, AC-F021-08, AC-F012-08, AC-F023-04 |
| SR-31 | M1 · M3 (watcher) | AC-F021-07, AC-F013-16, AC-F008-24, AC-F013-05 |
| SR-31a | M1 | AC-XC-30, AC-F008-10 |
| SR-32 | M1 | AC-F020-01, AC-F020-02, AC-F020-13 |
| SR-33 | M1 · M3 · M4 | AC-F020-03, AC-F012-08, AC-F016-03, AC-F022-05 |
| SR-34 | M1 | AC-F007-02 |
| SR-35 | M3 · M4 | AC-F012-06, AC-F012-07, AC-F012-15, AC-F023-03, AC-F023-04 |
| SR-40 | M1 (store) · M2 (review UI) · M3 (live) | AC-F022-01, AC-F022-02, AC-F022-04 |
| SR-41 | M1 | AC-F002-04, AC-F002-05, AC-F002-06 |
| SR-42 | M1 (P1 decision commits) · M4 (signing) | AC-F020-08, AC-F014-04, AC-F014-05, AC-F014-06, AC-F014-08, AC-F014-10 |
| SR-43 | M4 | AC-F014-09 |
| SR-44 | M1 | AC-XC-04, AC-F014-07 |
| SR-45 | M1 | AC-XC-04, AC-F014-12 |
| SR-46 | M4 | AC-F014-15 |
| SR-47 | M3 (outside signatures) · M4 | AC-F013-09, AC-F014-01, AC-F015-02, AC-F015-10; test `packet::outside_signature_needs_confirmation` |
| SR-50 | M1 · M3 (watcher) | AC-XC-11, AC-F013-04 |
| SR-51 | M0 ◆ | AC-XC-12, AC-M0-06, AC-F008-09 |
| SR-52 | M2 | AC-F026-03, AC-F026-01 |
| SR-53 | M1 (permissions) · M3 (tails) · M4 (retention) | AC-XC-14, AC-F009-07, AC-F026-10, AC-F011-07 |
| SR-54 | M4 | AC-XC-13 |
| SR-55 | M3 | AC-F008-23, AC-F022-02, AC-F029-06 |
| SR-56 | M3 ◆ · M4 (inbox notifications) | AC-XC-29, AC-F012-09, AC-F007-09, AC-F017-09 |
| SR-57 | M2 · M3 (quit) | AC-F008-25, AC-XC-31, AC-XC-05, AC-F009-09, AC-F020-10 |
| SR-60 | M0 · M5 (Stage 14 re-check) | AC-M0-06, AC-M0-10 |
| SR-61 | M0 | AC-M0-03, AC-M0-06, AC-M0-17 |
| SR-62 | M0 | AC-M0-03, AC-M0-07 |
| SR-63 | M0 | AC-M0-04, AC-M0-05 |
| SR-64 | M0 · M5 (release) | AC-M0-11 |
| PR-01 | M0 | AC-M0-10 |
| PR-02 | Stage 4.4 (`docs/contracts/files.md`) · M1 | AC-XC-15 |
| PR-03 | M2 · M3 (chat steps) | AC-F026-03, AC-XC-16 |
| PR-04 | M3 (index) · M4 (pruning, events, snapshots) | AC-F026-10, AC-F011-07, AC-F029-06 |
| PR-05 | M4 | AC-F026-10, AC-F029-06 |
| PR-06 | M1 | AC-XC-15, AC-F020-07 |
| PR-07 | M0 (no outbound requests; statement in the 4.3 threat model) | AC-M0-10 |
| PR-08 | M1 | AC-XC-30, AC-F019-03 |
| AR-01 | M3 (`--ax-screen-reader`) · M5 | AC-F008-20, AC-XC-20, AC-XC-21, AC-XC-22, AC-XC-23, AC-XC-24, AC-XC-25 |

Every SR, PR and AR in `docs/requirements.md` is mapped, including the new SR-19, SR-24a, SR-25a, SR-28, SR-29, SR-31a, SR-55, SR-56 and SR-57. SR-11 and SR-12 are v1.1. Every requirement is re-verified by the Stage 14 security review (EX-M5-03).

◆ **Where this differs from `requirements.md`'s stage column, and why:**
- SR-01: `git` is spawned through `git::run` from M1 (Stage 6, SR-44), so the allowlist applies there before Stage 7.
- SR-15, SR-17: the sensor and contract probes are P2 features, so they ship in M4, not Stage 9 (M3).
- SR-25a: the first Open in editor action (a read-only decision file, AC-F020-05) is in M2, so the file-opening rules and the editor setting are built there, before Stage 8.
- SR-28, SR-29: the prompt check is in the resolver (M1); terminal pastes and terminal links arrive with the terminal runner (F-009), which is M3 in this plan. Webview navigation is M2.
- SR-51: the dependency ban is a CI job, so it applies from M0.
- SR-56: clipboard reads first happen with paste-back and Link this chat (M3); `wait`-step notifications ship in P1 (M3); inbox notifications are M4.

---

## 6. Traceability: non-functional requirements

| NFR | Requirement | Milestone | How it's measured |
|---|---|---|---|
| NFR-01 | Cold start to interactive Today p95 ≤ 1.5 s | M2 (instrumented) · M5 (release build) | Timestamp from process start to first Today render in the run log; local perf test; Stage 14.2 |
| NFR-02 | Project switch p95 ≤ 300 ms (19 stages, 200 run records) | M2 · M5 | criterion on the load path + frontend mark |
| NFR-03 | Template load p95 ≤ 50 ms (cached ≤ 5 ms) | M1 | criterion on the golden blueprint (AC-F001-07) |
| NFR-04 | Resolution + `prepare_launch` p95 ≤ 50 ms | M1 (resolution) · M3 (preflight) | criterion (AC-F006-16, AC-F008-13), measured excluding the `claude -v` spawn, which runs in parallel with the fingerprint and counts inside NFR-05 |
| NFR-05 | Keystroke to spawn p95 ≤ 250 ms | M3 | Run-log timestamps |
| NFR-06 | Packet visible ≤ 1 s p95 | M3 | Integration test with a fake writer (AC-F013-07) |
| NFR-07 | Input to visual feedback ≤ 100 ms; no main-thread task > 50 ms | M2 · M5 | Playwright trace in the mock-IPC suite |
| NFR-08 | 1 MB terminal burst without dropped input or > 100 ms freeze | M3 | PTY integration test (AC-F009-10) |
| NFR-09 | Time to start a step p95 ≤ 10 s, excluding typing run inputs | M3 (kill check, median) · M5 (p95 over a week) | Export metrics (AC-F026-04); metric panel (AC-F026-09); AC-SM-04 |
| NFR-10 | No data loss or corruption | M1 | Crash-injection proptest at every write point (AC-F020-01); drill in M5 |
| NFR-11 | Crash-safe run state, zero sessions lost | M3 | Kill Foreman with a live fake child, restart (AC-F020-10) |
| NFR-12 | External edits win | M1 | Property test with interleaved external writes (AC-F020-03) |
| NFR-13 | Degrade, don't block | M3 (watcher) · M4 (sensor, Doctor, probes) | Fallback tests (AC-XC-17, AC-F011-09, AC-F019-16, AC-F013-10) |
| NFR-14 | Escape hatch: Markdown run sheet per stage | M4 | Golden test of the export (AC-F027-01) |
| NFR-15 | Zero crashes per week after the first month; local crash report | M4 (crash handler) · after M5 (measured over the first month of use after CO) | Run log + macOS crash reports (AC-XC-18) |
| NFR-16 | Signed records RPO 0 after each signing commit; RTO immediate (re-clone) | M4 · M5 (drill) | Signing tests (AC-F014-06); re-clone and open drill |
| NFR-17 | In-flight state RPO last write; RTO ≤ 1 min rebuild from repo | M3 · M5 (drill) | Delete app data and reopen (AC-F020-11) |
| NFR-18 | Settings: defaults plus auto-detection | M2 | AC-F029-03 |
| NFR-19 | 5 projects, 50 without degrading §1 targets | M5 | Stage 14.2 benchmark with a 50-project registry fixture |
| NFR-20 | 200 tickets per project, 2,000 without degrading | M5 | Stage 14.2: open a project whose `docs/tickets.md` has 200 and 2,000 tickets, within NFR-02 |
| NFR-21 | 1,000 run-log entries, 100,000 without degrading | M4 | criterion over a 100,000-entry index (AC-F026-08) |
| NFR-22 | 3 concurrent tabs, 12 without degrading | M3 · M5 | PTY integration test with 3 and 12 fake sessions; Stage 14.2 |
| NFR-23 | ≤ 300 MB with 3 tabs; ≤ 800 MB with 12 | M0 (S-0) · M5 | S-0 measurement with three real TUIs after 30 min (AC-M0-14); release build in Stage 14.2 |
| NFR-24 | 20,000-line blueprint still meets NFR-03 | M1 | criterion (AC-F001-07) |
| NFR-25 | App data ≤ 50 MB per year | M4 · M5 | Per-run size × runs per year from the run log, with 90-day pruning (AC-F026-10) |
| NFR-26 | ASVS 5.0 L2 for applicable chapters | M5 | Stage 14.1 security review against the threat model; inapplicable chapters recorded in 4.3 |
| NFR-27 | Tauri security guidance in full | M0 · M3 | AC-M0-08, AC-M0-09, AC-XC-07 |
| NFR-28 | Process spawning rules | M1 · M2 | AC-XC-01, AC-XC-02, AC-XC-31, AC-F008-14, AC-F008-15 |
| NFR-29 | Dependency governance clean at every gate | M0 (then every gate) | CI `deny`, `audit`, `pnpm-audit`, `pnpm-policy`, `actions-pinned` (AC-M0-03, AC-M0-06, AC-M0-17) |
| NFR-30 | Threat model covers the malicious repo | Stage 4.3 · M1 · M5 | Abuse-case tests (AC-XC-09); Stage 14.1 |
| NFR-31 | WCAG 2.2 AA | M5 | axe-core zero violations (AC-XC-20) + manual VoiceOver pass |
| NFR-32 | Full keyboard, visible focus, no traps | M3 (P1 loop) · M5 | Keyboard-only scripts (AC-F028-02, AC-XC-21) |
| NFR-33 | VoiceOver labels, live-region announcements, xterm screen-reader mode | M5 | Manual pass (AC-XC-22) |
| NFR-34 | Reduce Motion, Increase Contrast, Reduce Transparency | M5 | Manual pass with each setting (AC-XC-23, AC-F030-09) |
| NFR-35 | Contrast ≥ 4.5:1 / 3:1 in both themes | M5 | Token contrast test in Vitest (AC-XC-24) |
| NFR-36 | macOS 27 and 26, Apple Silicon | M0 · M5 | CI on a macOS runner; release build smoke on each macOS version available to Billy |
| NFR-37 | Claude Code ≥ 2.1.293, subscription login | M2 · M3 | Doctor and preflight (AC-F019-04, AC-F008-01, AC-F008-08) |
| NFR-38 | Built locally, ad-hoc signed | M0 | AC-M0-11 |
| NFR-39 | No network calls by Foreman | M0 | `sandbox-exec` deny-network harness run; `cargo deny` HTTP-client ban (AC-M0-10, AC-M0-06) |
| NFR-40 | All data on the Mac | M0 | AC-M0-10, AC-XC-15 |
| NFR-41 | Run index kept indefinitely; tails pruned after 90 days; Clear run history | M3 · M4 | AC-F026-01, AC-F026-10 |
| NFR-42 | Secrets never read or displayed | M1 · M3 (watcher) | AC-XC-11, AC-F013-04 |
| NFR-43 | $0 infrastructure | M0 · M5 | No server or paid service in the stack; confirmed at CO |
| NFR-44 | No Claude run without an explicit action; probes use no tokens; no `claude -p` in v1 | M3 · M4 | AC-XC-19, AC-XC-03, AC-F019-17 |
| NFR-45 | TypeScript strict, zero warnings | M0 | `pnpm verify` (AC-M0-03) |
| NFR-46 | Rust lint, no `unwrap`, `unsafe` forbidden | M0 | AC-M0-03, AC-M0-07 |
| NFR-47 | Every rule unit-tested; parser golden and property tests | M1 · M4 | EX-M1-04; AC-F001-01, AC-F001-05, AC-F001-06 |
| NFR-48 | TradeLog harness: zero manual edits and unresolved variables | M1 (Stages 1–2) · M4 (Stages 1–5) | AC-F003-01, AC-F003-04 |

Every NFR-01 to NFR-48 is mapped.

---

## 7. Decisions made while writing scope

Each fills a point where r6 and the Master Spec are silent, choosing the more conservative option. Billy can change any of them at the Stage 4 sign.

1. No git version floor is defined anywhere, so none is enforced: Doctor only shows git's version, and a missing git blocks the git-dependent actions (setup `git init`, lock restore, signing) (AC-F019-06, AC-F002-07, AC-F018-05, AC-F014-03).
2. Doctor failures don't block creating a project; launches stay blocked by F-008's preflight (AC-F019-04, NFR-13).
3. The P1 first-run template step shows the version and diagnostics only; the full Template health screen is P2 (AC-F019-02).
4. Probe-folder trust needs a terminal tab and isn't needed before P2's probes, so it moves from M2 to M4 (AC-F019-08; moved from M3 in the Stage 4.7 fix pass).
5. (Deleted in the Stage 4.7 fix pass. An outside signature counts only after Billy's native confirmation, AC-F013-09, SR-47.)
6. In P1, a hand-signed Stage 2 packet doesn't count as signed while any early condition is undecided (AC-F004-12).
7. In P1, steps that `append` to a file (F.2 to `docs/lessons.md`) offer no paste-back write; Billy appends by hand until F-016 (AC-F012-14).
8. Normalizing a packet Claude wrote is shown first and written only after Billy confirms (AC-F013-08).
9. If the watcher fails, steps still run and packets are validated when Billy opens the stage (AC-F013-10).
10. An `action` step whose destination exists shows a diff and needs confirmation (AC-F007-03).
11. In P1 (no sensor), Terminate always asks for confirmation (AC-F009-04).
12. A sensor binary with the wrong hash isn't used, and the P1 manual behaviors apply (AC-F011-02).
13. Copy and open with no Project or conversation link copies the prompt, opens nothing, and says which link to set (AC-F012-04).
14. Setup doesn't commit anything; the first decision change commits its file by pathspec (AC-F018-03, AC-F020-08).
15. Setup refuses a folder inside another project, the blueprint repo, or an unwritable folder; an existing `.foreman/` with a supported schema is registered without writes (AC-F018-06, AC-F018-07).
16. Today rows with no runnable step say why; a missing project folder shows "Project folder not found" (AC-F017-04, AC-F017-05).
17. A corrupt app-data state file is kept, and the rebuild from the repo is written only after Billy confirms (AC-F020-12).
18. A missing `docs/lessons.md` refuses the append rather than recreating the file (AC-F016-04).
19. Desktop launches use the same allowlisted path, environment allowlist and auth check as CLI launches (AC-F024-01).
20. A run sheet with unresolved tokens is exported with each one marked, not refused (AC-F027-03).
21. In P1, active time per step excludes only `wait`-step time automatically; Billy excludes Claude-working time by hand, as he does in the baseline (AC-F026-05).
22. Kill check: a surface without by-hand baseline data can't pass; each Copy-prompt-on-CLI proxy event needs an explanation in Billy's attestation; a failed check stops M4 and goes to a CR (AC-KC-03, AC-KC-04, AC-KC-05).
23. The inspector is part of F-030 P2 and ships in M5 with the sheet strip; until then the M4 gate, packet and lessons views live in the stage workspace and their own screens.
24. The TradeLog harness's argv snapshots join it in M3, when the argv builder exists (AC-F003-02).
25. For S-23's fail branch, the Master Spec §9 wording is used (no fork; the CLI chain continues), not r5 §4's (fork only).
26. A shortcut that doesn't apply to the selection does nothing except show a one-line reason (AC-F028-03).
27. Where `docs/requirements.md` is more specific than r6 or the Master Spec, the requirement wins in this file (signing plumbing, run-settings deny rules, fingerprint set, confirmation reasons, native confirmations, P1 notifications).
28. The editor setting (an app bundle ID, required by SR-25a) joins Settings in M2, with the first Open in editor action; with no editor set, Open in editor is disabled (AC-F029-07).
29. The spawn controls that `requirements.md` places in Stage 7 are built in M2 and proven with the fake binary, while the PTY launch stays in M3 as the milestone plan says (§2, M2).
