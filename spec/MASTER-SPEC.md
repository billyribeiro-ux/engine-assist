# Foreman · Master Product Spec

**Status: FROZEN v1.0 — 2026-10-19.** The freeze takes effect when Billy signs the Stage 2 packet (`docs/handoff/stage-02.md`). After that, every change needs a Change Request (§10).

Supersedes: `docs/spec-input/01-master-spec.md` (DRAFT v0.9).
Built from: the App Brief (Stage 1 packet) and the Stage 2 council, `spec/council/r1-engineering.md` through `r8-nfr.md`. Detailed feature behavior lives in `spec/council/r6-feature-specs.md`, which is part of this frozen spec.

---

## 1. Vision and positioning

Foreman is the site office for building software with Claude: it holds the blueprint, knows where every project stands, and starts each step with exactly the right crew, model, effort, mode and inputs. It catches every handoff packet, keeps gates honest, and signs the record, so the process is followed every time without effort. It wins because it's the only tool that turns Billy's blueprint into an executable, verified process while leaving Claude Code, Claude chat and the blueprint itself exactly as they are.

## 2. What it replaces, and how it's better

Foreman is a personal tool with no market. The comparison that matters is with the ways Billy could run the blueprint without it. (The Stage 2.1 teardown doesn't apply: `commercial = no`.)

| Job | By hand (the baseline) | Claude Desktop / Claude Code alone | Prompt or snippet managers | Foreman |
|---|---|---|---|---|
| Pick the next step across projects | Open each repo's latest packet | No notion of stages | No notion of stages | Today + Do next (⌘⇧↵) |
| Fill variables and packets into prompts | Copy and paste each time | — | Static placeholders | Resolver with typed run inputs; zero manual edits (tested) |
| Launch with the right model, effort, mode | Type flags from memory; `/model`, `/effort`, Shift+Tab mid-session | Last-used settings | — | Exact argv per step; re-arm for "same session" steps |
| Keep billing on the Max plan | Hope no `ANTHROPIC_API_KEY` or `apiKeyHelper` leaks in | Same | — | Env allowlist + config fingerprint + auth check in the step's cwd |
| Catch and validate packets | Read them | — | — | Watcher, validation, repair, normalization |
| Keep gates honest | Tick boxes from memory | — | — | Kinds per item; `auto` runs real commands; attest needs a note |
| Sign the record | `git add -A` and hope | — | — | Hash-bound, pathspec-only, read-back verified |
| Know what ran | Nothing | Transcripts (deleted after 30 days by default) | — | Run log + stage records with intended vs observed |

Differentiation in one line per alternative: **by hand** loses on every row; **Claude Desktop / Claude Code** run sessions but don't know the process; **prompt managers** store text but don't resolve packets, launch with flags, or gate anything.

## 3. Concepts

| Term | Meaning |
|---|---|
| Template | The blueprint (v2.2+) parsed into typed stages, steps, run configs, items and packet formats. `fm:` annotations are the only source of what Foreman acts on. |
| Lock | A project's pin: blueprint commit, file hash, versions. The text lives in app data, recoverable from the blueprint repo. |
| Project | One product. A repo on disk with `.foreman/`. |
| Condition | A tri-state project fact (`yes` / `no` / `undecided`) that decides conditional steps. Eight early, four late. |
| Tag | `core`, `conditional`, `optional`. |
| Applicable path | The stages and steps that apply after tags, conditions and skips. |
| Run config | Surface, cwd, session, start (model · effort · mode), switches, alternatives, agents. |
| Chain | One Claude conversation spanning several steps, identified by its session UUID. |
| Re-arm | End an idle chain process and relaunch `claude --resume <uuid>` with the next step's exact flags. |
| Packet | A Handoff Packet file in `docs/handoff/stage-NN.md`. |
| Gate | A stage's inspection items, each `attest`, `auto:cmd=…` or `time:<n>d`, plus prompts A, B, C and Sign. |
| Decision files | `.foreman/project.json`, `overlay.json`, `blueprint.lock.json`. Changes from outside Foreman need Billy's review. |

## 4. Feature catalog

Full specifications: `spec/council/r6-feature-specs.md` (starting situation, behavior, inputs and outputs, edge cases, done criteria, what it beats). Phase **P1** ends at the kill check; **P2** completes v1 (§9).

| ID | Feature | Phase | Beats |
|---|---|---|---|
| F-001 | Template engine (annotations only; prose lint; explicit item IDs with text hashes; grammar v1) | P1 + P2 | Retyping RUN WITH lines by hand |
| F-002 | Blueprint lock | P1 | No record of which blueprint produced a project |
| F-003 | Template health + TradeLog acceptance harness | P1 + P2 | Gaps found mid-run |
| F-004 | Rule engine: tags, tri-state conditions, applicable path | P1 | "Does this apply?" from memory |
| F-005 | Run configs + model policy (alternatives, substitutions, overrides, explain on hover) | P1 | Max/Pro and no-Fable rules from memory |
| F-006 | Prompt resolver (five token classes, remembered inputs, packet remap) | P1 + P2 | Manual prompt edits |
| F-007 | Step kinds (`prompt`, `followup`, `action`, `manual`, `wait`, `repeatable`) and states | P1 | Steps that live in Billy's head |
| F-008 | Launch pipeline (`prepare_launch`) | P1 | Flags from memory; billing leaks |
| F-009 | Terminal runner (portable-pty + xterm 6; P2: Now/Next, round tracker, focus) | P1 + P2 | A terminal that doesn't know the step |
| F-010 | Session chains and re-arm (manual idle confirmation in P1) | P1 | `/model`, `/effort`, Shift+Tab typed mid-session |
| F-011 | Session sensor (per-launch hooks, exec form, tokened events) | P2 | Not knowing which step waits on Billy |
| F-012 | Chat bridge (setup chips, links, paste-back, attachments, ADR split, fix request) | P1 | Hand-copying chat output into files |
| F-013 | Packet engine (watch, validate, repair, capture) | P1 | Packet errors found a stage late |
| F-014 | Signing and provenance (two-phase, pathspec, plumbing branch commits, read-back, tamper events) | P1 | `git add -A` signing commits |
| F-015 | Gates (attest with note or n/a; `auto` = Foreman runs the approved command; interactive Gate A import in P2) | P1 + P2 | Boxes ticked from memory |
| F-016 | Lessons capture | P1 | Lessons skipped |
| F-017 | Today and Do next (Next step in P1; full inbox in P2) | P1 + P2 | Opening each repo to find what's next |
| F-018 | Projects and setup (one form in P1; wizard in P2) | P1 + P2 | `new-repo-docs.sh` without tailoring |
| F-019 | First run, Doctor, zero-token contract probes | P1 + P2 | Breaks found inside a terminal tab |
| F-020 | State, persistence, resilience | P1 | Lost run state |
| F-021 | Path policy for every Foreman write | P1 | — (protects Foreman's writes) |
| F-022 | Outside-change review for decision files | P1 | The governed agent editing the rules |
| F-023 | Knowledge manifest + build-package assembler | P2 | The Stage 4 → 5 file job by hand |
| F-024 | Desktop handoff | P2 | Desktop starting with the wrong settings |
| F-025 | Add prompt / inspection item, this project only | P2 | Extra steps in side notes |
| F-026 | Run log (P1) and metric panel (P2) | P1 + P2 | A kill decision by feeling |
| F-027 | Run sheet export | P1 | Dependence on Foreman (escape hatch) |
| F-028 | Command palette and keyboard | P1 + P2 | — (table stakes) |
| F-029 | Settings | P1 | — (table stakes) |
| F-030 | Sheet strip and stage workspace | P1 | — (table stakes; the drawing-set design) |

**v1.1** (scheduled after Foreman's own Certificate of Occupancy): F-101 headless Gate A (hardened argv, after S-2′) · F-102 Crew Loop on CLI worktrees · F-103 Change Request track · F-104 full overlay authoring + promote to blueprint + pin upgrade · F-105 usage meter · F-106 live send and "go" · F-107 lessons view, conditional prompt blocks, retro starter · F-108 metrics dashboard · adopt an existing project.

## 5. Rules

- **Core** can never be skipped. **Conditional** applies when its `when` expression is true; otherwise it's not applicable with the reason. **Optional** may be skipped with a one-line reason. Gates, packets and lessons are core.
- **Early conditions:** `idea_known`, `commercial`, `payments`, `multi_user`, `public`, `has_ui`, `others_data`, `server`. **Late conditions** (asked at the Stage 4 sign): `has_database`, `has_uploads`, `has_email`, `has_integrations`.
- Conditions are editable with an audit line until the Stage 2 packet is signed; `undecided` blocks that sign. Afterwards a change needs a CR.
- Default tags for Stages 0–18: v0.9 §5.3, carried into the blueprint by annotations (B-1) for Stages 0–5.
- **Foreman's own build:** `idea_known` yes, `has_ui` yes, everything else no. Model policy: `plan = max`, substitution `fable → opus@xhigh`.

## 6. Claude Code integration contract

**Interactive launch** (every CLI step):

```text
claude --session-id <uuid>                       # chain start only
       | --resume <uuid> [--fork-session]        # continuing step
       --model <alias> --effort <level> --permission-mode <mode>
       --name fm-<project>-s<NN>-<step>
       --settings <app-data>/runs/<runId>/settings.json
       [--ax-screen-reader]                      # when VoiceOver is on
       "<resolved prompt>"
```

- argv array, no shell, absolute allowlisted path to `claude`.
- Child environment: explicit allowlist; `ANTHROPIC_*`, `CLAUDE_CODE_USE_*`, AWS, GCP, Azure variables always removed; `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` passed when present; PATH from `/etc/paths`, `/etc/paths.d`, Homebrew, cargo and pnpm homes, and the captured login PATH.
- Run settings: sensor hooks in exec form (P2), `disableAllHooks: false`, `permissions.deny: ["Read(./.foreman/**)"]`.
- Never `--bare`, `bypassPermissions`, `--dangerously-skip-permissions`, or `--allow-dangerously-skip-permissions`.

**Preflight** (`prepare_launch`): version vs the computed floor (2.1.293 with the current kit); contract probes on version change; config fingerprint of the step cwd with auth-changing keys blocked; `claude auth status` in the step cwd (`claude.ai` or `oauth_token`); transcript exists for continuing steps; inputs exist; run inputs filled; prompt doesn't start with `-`, `/` or `!`.

**Re-arm:** idle check (P1: Billy confirms; P2: sensor, including background work) → Ctrl-C, then `/exit` → wait for exit → relaunch. Never SIGTERM a non-idle session without confirmation.

**Desktop:** `claude --desktop` (no other flags); `claude --desktop --resume <uuid>` marks the chain handed off; the next Foreman step forks.

**Headless:** v1 makes no headless gate run. The only `-p` call in v1 is optional packet re-output, used only when the contract probe confirms `-p` still uses the subscription login. The hardened headless argv for v1.1 is in `r5-resolution.md` §2.1.

**Version floors:** Fable 5.1 2.1.257 · `--permission-prompts` 2.1.259 · Opus 5.5 2.1.280 · `--agents` file form 2.1.281 · Sonnet 5.5 2.1.284 · `--desktop` 2.1.285 · Haiku 5.5 2.1.293. Effective floor = maximum over the steps' models, flags, and the pinned kit's agent models.

## 7. Files and data

| Where | File | Contents | Committed |
|---|---|---|---|
| Repo | `.foreman/project.json` | Variables, conditions + audit, model policy, spec folder, verify commands, schemaVersion | Before Stage 5: at the moment of change, pathspec. From Stage 5: in the signing commit. |
| Repo | `.foreman/overlay.json` | Skips with reasons, project-only additions | As above |
| Repo | `.foreman/blueprint.lock.json` | Blueprint commit, file SHA-256, blueprint and grammar versions | As above |
| Repo | `.foreman/stages/stage-NN.json` | Provenance (immutable) | In the signing commit |
| Repo | `docs/handoff/stage-NN.md` (+ `-attachments.md`) | Packets | In the signing commit |
| Repo | `docs/lessons.md`, `docs/tickets.md`, `docs/change-requests/` | As in the blueprint | By Claude or Billy |
| App data | `settings.json`, `projects.json` | Settings, registry | — |
| App data | `projects/<id>/progress.json` | Step states, chains, gate runs, run inputs, pending decision changes | — |
| App data | `approvals/<id>.json` | Approved hashes of decision files and verify commands | — |
| App data | `runs/<runId>.json`, `.log`, `settings.json`; `events/<runId>/` | Run records, output tail, run settings, sensor events | — |
| App data | `templates/<hash>.md`, `bin/foreman-sensor-<hash>` | Locked blueprint text, sensor binary | — |

Every JSON file has `schemaVersion` and a migration. Writes are atomic (`.<name>.foreman-tmp`, fsync, rename, fsync dir). A file newer than Foreman's last read is never overwritten. Exact schemas are written in Stage 4.4 (`docs/contracts/files.md`).

## 8. Packet rules

A packet is valid when:
1. The header is `=== HANDOFF PACKET · Stage N → Stage M ===`, N matches the file name, and M is the next applicable stage.
2. The stage's required fields are present.
3. `OPEN ITEMS:` is exactly `none`.
4. `APPROVED BY:` is unsigned (`<blank>` and its variants normalize to unsigned) or `<name> on <YYYY-MM-DD>`.
5. For Stage 4: `BLUEPRINT VERSION` equals the lock.
6. At signing, Foreman writes `SKIPPED:`, `SUBSTITUTIONS:` and `ATTESTED:` deterministically.

Signing order: Gate A → B → C → Sign. Signing is hash-bound, commits only named paths, verifies by reading back the commit, and records intended and observed models.

## 9. Delivery plan

| Phase | Contents | Ends with |
|---|---|---|
| 0 · Prove the risky parts (Stage 5) | Scaffold; spikes S-0, S-1′, S-3, S-7′, S-8, S-12, S-23, each with a decision rule (r5 §4) | Spike reports in `docs/spikes/` |
| 1 · Thin end-to-end | F-001 (Stages 1–2, Gate, F.2), F-002, F-004, F-005, F-006 (subset), F-007, F-008, F-009 (base), F-010 (manual idle), F-012, F-013, F-014, F-015 (attest), F-016, F-017 (Next step), F-018 (form), F-019 (base), F-020, F-021, F-022, F-026 (log), F-027, F-028 (base), F-029, F-030 | **TradeLog Stages 1–2 through Foreman; kill check against the baseline** |
| 2 · Complete v1 | F-003 (full), F-006 (Stages 3–5), F-009 (Now/Next etc.), F-011, F-015 (`auto` + import), F-017 (inbox), F-018 (wizard), F-019 (full), F-023, F-024, F-025, F-026 (panel), F-028 (full) | **TradeLog Stages 3–5 through Foreman; success metrics read** |
| v1.1 | F-101 … F-108 | v1.1 metric |

**Estimate:** about 1 week (phase 0) + 5–7 weeks (phase 1) → kill check at about 6–8 weeks; phase 2: 7–12 weeks. Total v1 about 13–20 working weeks. Calendar time is longer.

**Success metrics (App Brief):** TradeLog Stages 1–5 run entirely through Foreman; zero manual prompt edits; zero gates skipped by accident; starting any step takes under 10 seconds.

**Kill criterion (App Brief), made measurable:** after TradeLog Stages 1–2 (end of phase 1), if Foreman's median time from deciding to run a step to the prompt being sent is not lower than the by-hand baseline in `docs/metrics-baseline.md`, or if any step needed a manual prompt edit that the by-hand run didn't, stop and simplify.

**v1.1 metric:** one TradeLog milestone through the CLI Crew Loop with zero manual routing and zero lost ticket state. **v1.1 kill criterion:** if routing a ticket through Foreman's loop is slower than the Desktop loop by hand over that milestone, reduce the loop to a launcher.

### Spikes (none can run before the freeze; each has both branches specified)

| ID | Question | Pass | Fail |
|---|---|---|---|
| S-0 | PTY + xterm 6 + real Claude TUI in WKWebView: Shift+Tab, 5 KB paste, resize while streaming, ⌘K routing, VoiceOver, memory with 3 tabs after 30 min | As specified | Scrollback cap and archive; key-routing layer; `--ax-screen-reader`; NFR raised to measured + 20%; a fundamental failure reopens the shell choice (SwiftUI + SwiftTerm) |
| S-1′ | Per-launch hooks fire (zero-token `--init-only` probe) | Sensor ships in P2 | No sensor; manual idle confirmation stays |
| S-3 | `--effort` on resume | Flag | `effortLevel` via `--settings`; else Billy types `/effort` with a warning |
| S-7′ | Settings `env` / `apiKeyHelper` in a trusted repo | R-03 controls suffice | Block any project `env` block until approved |
| S-8 | Graceful end with input, background work; `--session-id` + `--resume`; latency | As specified | Billy ends the session himself, then Foreman relaunches |
| S-12 | PATH in a Finder-launched build | As specified | Login-PATH capture mandatory at first run |
| S-23 | Does `--desktop --resume` share the transcript? | Fork rule as specified | Desktop handoff always starts a new chain |
| S-2′ | Hostile-repo headless gate (v1.1 only) | Headless Gate A ships in v1.1 | It doesn't; interactive stays |

## 10. Stack (final)

Evidence and alternatives: `spec/council/r7-stack.md`. Billy's input: "no preference" between the two frontend options; the evidence-backed recommendation is adopted (r5 §5). Billy can override at the Stage 2 sign.

| Layer | Choice | Reason |
|---|---|---|
| Desktop shell | Tauri 2.12.4 | Small bundle; only Rust spawns or touches files; native window effects built in. Electron needs an 8-week upgrade cycle and an app-applied security checklist; SwiftUI + SwiftTerm loses on solo build speed and is the fallback if S-0 fails fundamentally. |
| Frontend | Svelte 5.57.2 + Vite 8.3.4 + in-house typed hash router | An SPA uses none of SvelteKit's server features; 2.6× smaller measured; no inline script, so the CSP is literal. |
| Language | TypeScript 6.0.3 strict · Rust stable ≥ 1.90 (pinned in `rust-toolchain.toml`) | TS 7 breaks `svelte-check` and `typescript-eslint` today; Tauri crates need Rust ≥ 1.90. |
| Package manager / runtime | pnpm 12.14.0 · Node ≥ 22.17 | Strict, reproducible. |
| Terminal | @xterm/xterm 6.0.0 (+ fit, webgl, unicode11, search, web-links addons) | The standard; screen-reader mode. |
| PTY | portable-pty 0.9.0 in Foreman's own wrapper | Rust picks the program. |
| Process spawning | `std::process` / `tokio::process`, argv only | No shell plugin. |
| File watching | notify 8.2.0 + notify-debouncer-full 0.7.0 | FSEvents; 9.x is an RC. |
| Markdown | comrak =0.56.0 | Line and column on every node. |
| UI primitives | bits-ui 2.19.5 | Svelte 5 native, including Command for ⌘K. |
| State | Svelte 5 runes in `.svelte.ts`, mirroring Rust over commands, Channels and events | Rust is the source of truth. |
| Tests | cargo test + proptest 1.12.0 + insta 1.49.0 · Vitest 5.0.5 (browser mode) · Playwright 1.64.1 + axe 4.14.0 with mocked IPC · @wdio/tauri-service 1.5.0 smoke (webdriverio 9.32.0) behind an `e2e` feature | `tauri-driver` has no macOS support. |
| Lint and format | eslint 10.13.0 + eslint-plugin-svelte 3.23.1 + typescript-eslint 8.72.0 · svelte-check 4.7.7 · prettier 3.9.10 · rustfmt + clippy `-D warnings` | Zero-warning gates. |
| Tauri plugins | opener 2.7.2, clipboard-manager 2.5.0, dialog 2.8.3, window-state 2.5.0; notifications plugin in P2 (version verified in Stage 4.2) | No shell, fs or pty plugins. |
| Hardening | `freezePrototype: true`; `AppManifest::commands`; capabilities per window; explicit `csp` (`script-src 'self'`, `connect-src` Tauri IPC only) and separate `devCsp` | SR-06, SR-08. |

Full pinned list (Rust crates and JS packages): `r7-stack.md` PART 3.

## 11. Non-functional requirements

Full list with measurement methods: `spec/council/r8-nfr.md` (NFR-01 to NFR-48). The key numbers:

| Area | Target |
|---|---|
| Cold start to interactive Today | p95 ≤ 1.5 s |
| Project switch | p95 ≤ 300 ms (200 tickets) |
| Template load | p95 ≤ 50 ms (cached ≤ 5 ms) |
| Resolution + preflight | p95 ≤ 50 ms |
| Keystroke to spawn | p95 ≤ 250 ms |
| Packet visible after write | p95 ≤ 1 s |
| Time to start a step | median ≤ 10 s |
| Memory | ≤ 300 MB with 3 terminal tabs (verified in S-0) |
| Data safety | No loss or corruption, ever; crash-safe run state; external edits win |
| Security | ASVS 5.0 L2 for applicable chapters; Tauri hardening in full; threat model includes the malicious repo |
| Accessibility | WCAG 2.2 AA; full keyboard; VoiceOver; Reduce Motion / Transparency / Increase Contrast |
| Platform | macOS 27 and macOS 26, Apple Silicon; Claude Code ≥ the computed floor, subscription login |
| Network | Foreman makes no network calls of its own |
| Cost | $0 infrastructure; no Claude usage without an explicit action |
| Quality | TS strict (+ `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), zero warnings; clippy pedantic on the core crate; no `unwrap` outside tests; `unsafe` forbidden |

## 12. Security and privacy requirements (input to Stage 3)

| ID | Requirement |
|---|---|
| SR-01 | Processes are spawned only from allowlisted absolute paths, with argv arrays. No shell, except the single consented login-PATH capture in Doctor. |
| SR-02 | Never `bypassPermissions`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, or `--bare`. |
| SR-03 | The exact argv and prompt are visible before every run; every run is logged. |
| SR-04 | Every Foreman write passes the path policy (F-021); Tauri capabilities are scoped per window to the minimum. |
| SR-05 | Repo content (packets, lessons, tickets, overlay, chat output) is data: rendered as text, never as HTML, never executed, never a source of write destinations except through the path policy. |
| SR-06 | Strict CSP: `script-src 'self'`, no inline scripts, no remote origins. |
| SR-07 | Overlay-added steps and any mode different from the template's are highlighted and confirmed before running. |
| SR-08 | No telemetry and no network calls by Foreman. |
| SR-09 | Secrets are never read or displayed; `.env*` excluded from views and watchers; sensor events retain no assistant text beyond the last 20 events per run, deleted at stage sign. |
| SR-10 | Child environment is an explicit allowlist; provider and API-key variables are always removed. |
| SR-11 | Auth-changing keys in a step cwd's Claude config block the launch; any change to that config needs acknowledgement; auth is checked in the step's cwd. |
| SR-12 | Decision files changed from outside Foreman take effect only after Billy accepts the diff. |
| SR-13 | Verify commands are approved objects, linted against shells, interpreters, generic runners, network tools and git writes. |
| SR-14 | Sensor events are authenticated with a per-run token; hooks use exec form. |
| SR-15 | Signing commits include only named paths and are verified by read-back. |
| PR-01 | All data stays on Billy's Mac. |

Stage 3.2 turns these into the full SR/PR list in `docs/requirements.md`.

## 13. Non-goals and rejected ideas

**Non-goals (v1):** cloud sync · other users or teams · payments · Windows or Linux · Intel Macs · App Store or notarized distribution · calling the Claude API or using API keys · editing code inside Foreman · replacing Claude Code's UI · editing the blueprint (Foreman proposes; Billy applies) · Fable in Foreman's own build.

**Rejected:** see `r6-feature-specs.md` §6 (YAML source, inference as a source, positional IDs, headless Gate A in v1, user-level hooks, typed TUI keystrokes, `--bg` substrate, Agent SDK/API, browser automation of claude.ai, pty/shell/fs plugins, SvelteKit 3 SPA, Electron, a v1 dashboard).

## 14. Design direction amendments

`docs/spec-input/02-design-direction.md` v0.9 stands, with these changes from the council:

1. **Home is Today**, not a Projects grid: items that need Billy across all projects, then each project's Next step with its spec line. P1 shows the Next step rows; P2 adds the inbox.
2. **Sheet strip marks** add **needs annotation** (dashed outline) and **undecided** (question mark) to the six existing marks.
3. **Gate screen:** no "Run inspection headless" button in v1. Items show their kind: attest (Done with note / Not applicable with reason), `auto` (Run, with exit code and output), `time` (date). Review order FAIL → waiting → PASS.
4. **Step block:** the resolved prompt and exact argv are always one click away (collapsed); run inputs appear as an inline form above the prompt; remapped packet paths show a subtle note.
5. **Terminal drawer:** tab states running · idle · blocked · exited · errored · interrupted; archived buffer sections after re-arm; the **Now/Next strip** (P2) above each tab.
6. **New surfaces:** Pending outside change (a diff with Accept / Reject), tamper event, broken chain, config-changed banner, Template health, Interrupted sessions.
7. **Keyboard additions:** ⌘⇧↵ Do next · ⌘⇧V paste-back · ⌘D mark done · ⌘⇧T focus mode · ⌘⇧E return from terminal · ⌘1–⌘9 pinned projects.
8. **Platform:** macOS 27 and 26 both support Liquid Glass; Visual Effect materials remain the fallback; Reduce Transparency turns window effects off.
9. **Wizard review step** lists exact file writes (create / leave alone / refuse to overwrite).

## 15. Open questions

None.

## 16. Change control

After v1.0, every change requires a Change Request (`docs/change-requests/CR-NNN.md`, from `docs/templates/change-request.md`): what, why, impact on scope, ADRs, timeline and cost, approved by Billy. Until the v1.1 CR track exists, CRs are handled by hand with that template.
