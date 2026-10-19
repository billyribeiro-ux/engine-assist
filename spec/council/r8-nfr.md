# Round 8 · Non-functional requirements (prompt 2.5)

Chair: Opus 5.5 (xhigh) · Date: 2026-10-19
Inputs: App Brief (`docs/handoff/stage-01.md`), draft spec §9–§10, `r1-engineering.md`, `r1-workflow.md`, `r2-proposals.md`, `r3-synthesis.md`; revised after `r4-redteam.md`, `r5-resolution.md` and `r5b-freeze-review.md`.

Prompt 2.5 asks for latency per endpoint class, availability and error budget, backup RPO/RTO, scale at launch and at 100x, security level, accessibility, platform support, retention, privacy, and infrastructure cost per user. Foreman is a local, single-user desktop app with no server, so each item is translated to what it means here. Where a web concept doesn't apply, this file says so and gives the local equivalent.

Every number has an ID, a measurement method, and the stage where it's enforced. A target without a measurement isn't a requirement.

---

## 1. Responsiveness

The success metric that matters most: **starting any step takes under 10 seconds from opening the app** (App Brief). Everything below serves that.

| ID | Interaction class | Target (Apple Silicon, M1 or later) | Measured by |
|---|---|---|---|
| NFR-01 | Cold start to an interactive Today screen | p95 ≤ 1.5 s | Timestamp from process start to first Today render, in the run log; checked in CI-equivalent local perf test |
| NFR-02 | Warm switch between projects | p95 ≤ 300 ms for a project with 19 stages and 200 run records | Rust benchmark (`criterion`) on the load path + frontend mark |
| NFR-03 | Template load (parse + validate + prose lint of the full blueprint) | p95 ≤ 50 ms; cached by file hash, so most loads are ≤ 5 ms | `criterion` benchmark against the golden blueprint (r1-engineering measured comrak at 1.7 ms for the parse alone) |
| NFR-04 | Prompt resolution and `prepare_launch` checks | p95 ≤ 50 ms | `criterion` |
| NFR-05 | Launch: keystroke to the `claude` process spawned in the PTY | p95 ≤ 250 ms (excludes Claude Code's own startup) | Run-log timestamps |
| NFR-06 | Packet visible in the UI after the file is written | p95 ≤ 1 s (debounce ~250 ms) | Integration test with a fake writer |
| NFR-07 | UI input latency: any click or key to visual feedback | ≤ 100 ms; no main-thread task over 50 ms during normal use | Playwright trace in the mocked-IPC suite |
| NFR-08 | Terminal throughput | Renders a 1 MB burst of output without dropping input or freezing the UI for more than 100 ms | PTY integration test with xterm's WebGL renderer |
| NFR-09 | Time to start a step (the success metric) | p95 ≤ 10 s, excluding time typing run inputs, from focusing Foreman or pressing Do next to the prompt being sent, over a week of real use | Export metrics (P1) and the metric panel (P2), F-026 |

## 2. Availability and reliability

There's no service to keep up. Availability means: Foreman never stands between Billy and his work, and never damages anything.

| ID | Requirement | Measured by |
|---|---|---|
| NFR-10 | **No data loss or corruption, ever.** Every write is atomic (temp file in the same directory, `fsync`, rename, `fsync` of the directory). Tested by killing the process at every write point. | Fault-injection tests in Rust |
| NFR-11 | **Crash-safe run state.** A run record is written at launch (status `running`) and updated at exit. After a crash or force quit, the next start lists interrupted sessions with one-click Resume. Zero sessions lost. | Integration test: kill Foreman with a live fake-`claude` child; restart; assert the session is listed |
| NFR-12 | **External edits win.** If a file changed on disk after Foreman read it, Foreman refuses to write and shows the conflict. Foreman never overwrites a newer file. | Property test on the write path with interleaved external edits |
| NFR-13 | **Degrade, don't block.** If the session sensor, file watcher, Doctor checks, or contract probes fail, the step still runs by hand with one extra confirmation. Foreman's failure never blocks starting a step. | Tests for each fallback path (F-011, F-019) |
| NFR-14 | **Escape hatch.** Every stage can be exported as a Markdown run sheet (resolved prompts and exact commands), so the blueprint can always be run without Foreman. | Golden test of the export |
| NFR-15 | Stability target: zero crashes per week of daily use after the first month. Any crash produces a local crash report (no upload) and a run-log entry. | Run log + macOS crash reports |

Error budget: not applicable (no SLO-bearing service). The equivalent rule: **any data-loss or corruption bug is a stop-the-line defect**, fixed before further feature work, with a postmortem in `docs/postmortems/`.

## 3. Backup and recovery (RPO/RTO)

| ID | Data | RPO | RTO |
|---|---|---|---|
| NFR-16 | Signed records and decisions (repo `.foreman/`, `docs/handoff/`, `docs/lessons.md`) | 0 after each signing commit (in git; pushed when Billy pushes) | Immediate: re-clone the repo; Foreman rebuilds everything else from it |
| NFR-17 | In-flight state (app data: `progress.json`, `runs/`, `events/`) | Last completed write (atomic) | ≤ 1 minute: if app data is lost, Foreman rebuilds stage and step status from the repo's signed records and packets; only unsigned in-progress markers and the run log are lost |
| NFR-18 | Settings | Last write | Defaults plus auto-detection on first start |

Foreman doesn't run its own backups. Time Machine and git remotes cover it. Requirement: app data contains nothing that can't be rebuilt from the repos except the run log and metrics.

## 4. Scale

| ID | Dimension | At launch | 100x headroom (must not degrade beyond the targets in §1) |
|---|---|---|---|
| NFR-19 | Projects registered | 5 | 50 (the realistic ceiling for one person; 100x of active projects isn't meaningful) |
| NFR-20 | Tickets per project | 200 | 2,000 |
| NFR-21 | Run-log entries | 1,000 | 100,000 (JSON Lines, one file per month; the metric panel reads an index, not the raw log) |
| NFR-22 | Concurrent terminal tabs | 3 | 12 |
| NFR-23 | Memory | ≤ 300 MB resident with 3 terminal tabs (scrollback capped at 10,000 lines per tab) | ≤ 800 MB with 12 tabs |
| NFR-24 | Blueprint size | 2,235 lines (v2.1) | 20,000 lines, still meeting NFR-03 |
| NFR-25 | Disk use by app data | ≤ 50 MB per year of daily use (run log + terminal tail files, 2,000 lines per run, pruned after 90 days) | — |

NFR-23 is UNVERIFIED until spike S-0 in Stage 5.3 (xterm 6 with the WebGL renderer and three real Claude TUIs); its fail branch is in Master Spec §9.

## 5. Security level

ASVS 5.0 is written for web applications and services; Foreman has no server, no authentication, and no network surface of its own. The applicable standard:

| ID | Requirement |
|---|---|
| NFR-26 | **ASVS 5.0 Level 2 for the chapters that apply to a local app with a webview**: encoding and sanitization (rendering repo content), validation and business logic (packet rules, signing), file handling, configuration (CSP, hardening), and secure coding and architecture (dependency governance). Web-only chapters (session management, authentication, OAuth, web services) are marked not applicable in the threat model, with the reason. |
| NFR-27 | Tauri's security guidance applied in full: capabilities scoped per window to the minimum, `freezePrototype: true`, an explicit CSP with no `unsafe-inline` and no remote origins, `AppManifest::commands` restricting app commands, no shell or fs plugins. |
| NFR-28 | Process spawning: allowlisted absolute paths, argv arrays only, no shell ever, an explicit clean environment (API-key and provider variables always removed), never `--bare`, never `bypassPermissions`, never `--dangerously-skip-permissions`. |
| NFR-29 | Dependency governance: `cargo audit` and `cargo deny` (advisories, licenses, sources) and `pnpm audit` are clean at every Stage Gate; lockfiles committed; every dependency pinned to an exact version for direct deps in the Rust core. |
| NFR-30 | A threat model (Stage 4.3) covering the malicious-repo case: a repo Foreman opens is untrusted input, including its `.foreman/`, packets, tickets, overlay, and the `.claude/` settings, hooks, and CLAUDE.md that Claude Code itself loads. |

The detailed SR-NN list is written in Stage 3.2 (`docs/requirements.md`).

## 6. Accessibility

| ID | Requirement | Measured by |
|---|---|---|
| NFR-31 | WCAG 2.2 Level AA for all Foreman UI | axe-core in the Playwright suite (zero violations) + a manual VoiceOver pass at the Stage 13 gate |
| NFR-32 | Every action reachable by keyboard; visible focus everywhere; no keyboard traps (the terminal releases focus with a documented shortcut) | Keyboard-only test script |
| NFR-33 | VoiceOver: every control labeled; status changes announced through a live region; xterm's `screenReaderMode` available in Settings | Manual pass |
| NFR-34 | Respects macOS Reduce Motion, Increase Contrast, and Reduce Transparency (window effects turn off) | Manual pass with each setting |
| NFR-35 | Text contrast ≥ 4.5:1 (≥ 3:1 for large text and UI components) in both themes | Token-level contrast test in Vitest |

## 7. Platform support

| ID | Requirement |
|---|---|
| NFR-36 | macOS 27 (Golden Gate, released 2026-09-14) and macOS 26 (Tahoe), Apple Silicon only. Liquid Glass window effects on both, with a Visual Effect material as the fallback. Intel Macs and Windows/Linux are not supported. |
| NFR-37 | Claude Code CLI ≥ 2.1.293, the floor computed from the steps' models and flags and the pinned kit's agent models (R-19), checked by Doctor. Logged in with a Claude subscription (`authMethod` `claude.ai` or `oauth_token`). |
| NFR-38 | Built and run locally (ad-hoc signed, no notarization), per the App Brief. |

## 8. Data retention and privacy

| ID | Requirement |
|---|---|
| NFR-39 | Foreman makes **no network calls** of its own. No telemetry, no update checks, no crash upload. (Claude Code, launched by Foreman, uses the network as usual; that's Claude Code's behavior, not Foreman's.) Enforced by CSP `connect-src` with only Tauri's IPC origin and by having no HTTP client in the Rust dependency tree (checked by `cargo deny`). |
| NFR-40 | All data stays on the Mac: the project repos and app data. |
| NFR-41 | Run-log retention: indefinite for the run index (it backs the metrics); terminal tail files pruned after 90 days. A "Clear run history" action exists. |
| NFR-42 | Foreman never reads or displays secrets: `.env*` and files matching common key patterns are excluded from every watcher and file view. Terminal output isn't logged beyond the per-run tail, which stays in app data. |

## 9. Cost

| ID | Requirement |
|---|---|
| NFR-43 | Infrastructure cost: $0. No servers, no paid services. |
| NFR-44 | Claude usage: Foreman never starts a Claude run without an explicit action from Billy. Contract probes (`--init-only`) use no tokens. v1 makes no `claude -p` calls; in v1.1, headless Gate A shows its model and effort before running and records its cost from the JSON result in the run log. |

## 10. Engineering quality

| ID | Requirement |
|---|---|
| NFR-45 | TypeScript strict (plus `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), zero errors and zero warnings in `svelte-check` and ESLint. |
| NFR-46 | Rust: `clippy -D warnings` (pedantic on the core crate), `rustfmt`, no `unwrap`/`expect` outside tests, `unsafe` forbidden in Foreman's own crates. |
| NFR-47 | Every rule in the tag/condition engine, packet rules, resolver, and signing has unit tests; the parser has golden tests against the real blueprint and property tests for the annotation grammar. |
| NFR-48 | The TradeLog acceptance harness (F-003) passes: Stages 1–5 of the TradeLog blueprint run produce zero manual prompt edits and zero unresolved variables. |
