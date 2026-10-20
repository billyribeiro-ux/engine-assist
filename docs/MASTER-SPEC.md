# Foreman · Master Product Spec

**Status: FROZEN v1.0 — 2026-10-19.** The freeze takes effect when Billy signs the Stage 2 packet (`docs/handoff/stage-02.md`). After that, every change needs a Change Request (§10).

Supersedes: `docs/spec-input/01-master-spec.md` (DRAFT v0.9).
Built from: the App Brief (Stage 1 packet) and the Stage 2 council, `spec/council/r1-engineering.md` through `r8-nfr.md`, plus the independent freeze review `r5b-freeze-review.md`. Detailed feature behavior lives in `spec/council/r6-feature-specs.md`, which is part of this frozen spec.

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
| Keep gates honest | Tick boxes from memory | — | — | Kinds per item; attest needs a real note or a not-applicable reason; Sign stays locked until close-out is complete |
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
| Gate | A stage's inspection items, each `attest` or `time:<n>d` in v1 (`auto:cmd=…` in v1.1), plus the stage's close-out sequence (§8) and Sign. |
| Decision files | `.foreman/project.json`, `overlay.json`, `blueprint.lock.json`. Changes from outside Foreman need Billy's review. |

## 4. Feature catalog

Full specifications: `spec/council/r6-feature-specs.md` (starting situation, behavior, inputs and outputs, edge cases, done criteria, what it beats). Phase **P1** ends at the kill check; **P2** completes v1 (§9).

| ID | Feature | Phase | Beats |
|---|---|---|---|
| F-001 | Template engine (annotations only; prose lint; explicit item IDs with text hashes; grammar v1; `fm:if` parts) | P1 (Stages 1–2, F.2) + P2 (0–5, Gate) | Retyping RUN WITH lines by hand |
| F-002 | Blueprint lock | P1 | No record of which blueprint produced a project |
| F-003 | Template health + TradeLog acceptance harness | P1 (harness, Stages 1–2) + P2 (screen, Stages 0–5) | Gaps found mid-run |
| F-004 | Rule engine: tags, tri-state conditions, applicable path | P1 | "Does this apply?" from memory |
| F-005 | Run configs + model policy (alternatives, substitutions, overrides, explain on hover) | P1 | Max/Pro and no-Fable rules from memory |
| F-006 | Prompt resolver (six token classes including optional parts, remembered inputs, packet remap) | P1 + P2 | Manual prompt edits |
| F-007 | Step kinds (`prompt`, `followup`, `action`, `manual`, `wait`, `repeatable`) and states | P1 | Steps that live in Billy's head |
| F-008 | Launch pipeline (`prepare_launch`) | P1 (contract probes in P2) | Flags from memory; billing leaks |
| F-009 | Terminal runner (portable-pty + xterm 6; P2: Now/Next, round tracker, focus) | P1 + P2 | A terminal that doesn't know the step |
| F-010 | Session chains and re-arm (manual idle confirmation in P1) | P1 | `/model`, `/effort`, Shift+Tab typed mid-session |
| F-011 | Session sensor (per-launch hooks, exec form, tokened events) | P2 | Not knowing which step waits on Billy |
| F-012 | Chat bridge (setup chips, links, paste-back, attachments, fix request; ADR split in P2) | P1 + P2 | Hand-copying chat output into files |
| F-013 | Packet engine (P1: watch, validate, normalize; P2: repair, sensor capture) | P1 + P2 | Packet errors found a stage late |
| F-014 | Signing and provenance (two-phase, pathspec, read-back, tamper events; current branch by default) | P2 (P1: Billy signs by hand) | `git add -A` signing commits |
| F-015 | Gates (attest with note or n/a; `time`; interactive Gate A with Import results; `auto` items are v1.1) | P2 (P1: by hand) | Boxes ticked from memory |
| F-016 | Lessons capture (F.2 per stage, Gate C detection) | P2 (P1: by hand) | Lessons skipped |
| F-017 | Today and Do next (Next step in P1; full inbox in P2) | P1 + P2 | Opening each repo to find what's next |
| F-018 | Projects and setup (one form in P1; wizard in P2) | P1 + P2 | `new-repo-docs.sh` without tailoring |
| F-019 | First run, Doctor, zero-token contract probes | P1 (first run, version, auth) + P2 | Breaks found inside a terminal tab |
| F-020 | State, persistence, resilience | P1 | Lost run state |
| F-021 | Path policy for every Foreman write | P1 | — (protects Foreman's writes) |
| F-022 | Outside-change review for decision files | P1 | The governed agent editing the rules |
| F-023 | Knowledge manifest + build-package assembler | P2 | The Stage 4 → 5 file job by hand |
| F-024 | Desktop handoff | P2 | Desktop starting with the wrong settings |
| F-025 | Add prompt / inspection item, this project only | P2 | Extra steps in side notes |
| F-026 | Run log + Export metrics (P1); metric panel and run log screen (P2) | P1 + P2 | A kill decision by feeling |
| F-027 | Run sheet export | P2 | Dependence on Foreman (escape hatch) |
| F-028 | Command palette and keyboard (P1: core shortcuts; P2: palette) | P1 + P2 | — (table stakes) |
| F-029 | Settings (P1: paths and DRI name; P2: the rest) | P1 + P2 | — (table stakes) |
| F-030 | Stage workspace (P1, minimal); sheet strip and title block (P2) | P1 + P2 | — (table stakes; the drawing-set design) |

**v1.1** (scheduled after Foreman's own Certificate of Occupancy): F-101 headless Gate A (hardened argv, after S-2′), `auto` items and approved verify commands · F-102 Crew Loop on CLI worktrees · F-103 Change Request track · F-104 full overlay authoring + promote to blueprint + pin upgrade · F-105 usage meter · F-106 live send and "go" · F-107 lessons view, retro starter · F-108 metrics dashboard · adopt an existing project.

## 5. Rules

- **Core** can never be skipped. **Conditional** applies when its `when` expression is true; otherwise it's not applicable with the reason. **Optional** may be skipped with a one-line reason. Gates, packets and lessons are core.
- **Early conditions:** `idea_known`, `commercial`, `payments`, `multi_user`, `public`, `has_ui`, `others_data`, `server`. **Late conditions** (asked at the Stage 4 sign): `has_database`, `has_uploads`, `has_email`, `has_integrations`.
- Conditions are editable with an audit line until the Stage 2 packet is signed; `undecided` blocks that sign. Afterwards a change needs a CR.
- `tag=optional` with a `when` expression: not applicable when the expression is false, skippable (with a reason) when true. An unknown condition ID in `when` is a template error.
- **Parts of a step:** a conditional part is wrapped in `<!-- fm:if <expr> -->` … `<!-- fm:end -->` and removed when the expression is false (recorded under SKIPPED). An optional part is written `[optional: …]` and shown with an Include toggle (default on); turning it off removes it and records it under SKIPPED.
- A stage in which nothing applies has no packet; its reason is written into the next signed packet's SKIPPED.

**Default tags** (carried into the blueprint by annotations, B-1, for Stages 0–5; Stages 6–18 are annotated in a later blueprint release):

| Stage / step | Tag | Applies when |
|---|---|---|
| 0 Land Survey | optional | `!idea_known` |
| 1.1–1.2 Interview, brainstorm | core | — |
| 1.3 Demand validation | conditional | `commercial` |
| 1.4 App Brief packet · press release + FAQ part | core · optional part | — |
| 2.1 Competitor teardown | conditional | `commercial` |
| 2.2 council: market-analyst, growth-strategist | conditional | `commercial` |
| 2.2–2.6 council, specs, stack, NFRs, freeze | core | — |
| 3.1 Business model and pricing | conditional | `commercial \| payments` |
| 3.2 Security and privacy requirements · legal part | core · conditional part | legal: `commercial \| others_data \| multi_user` |
| 4.x Scope, design doc, ADRs, threat model, data/API, tickets, independent review, CLAUDE.md | core | — |
| 4.5 Design system and screens | conditional | `has_ui` |
| 5 Site prep · staging deploy part | core · conditional part | staging: `server` |
| 6 Migrations · auth · domain core | conditional · conditional · core | `has_database` · `multi_user \| public` · — |
| 7 Framing · 8 Roofing | core | — |
| 9 Plumbing (uploads and email parts) | core (conditional parts) | `has_uploads`, `has_email` |
| 10 Electrical | conditional | `payments` |
| 11 HVAC · SLO part · analytics part | core · conditional parts | `server` · `multi_user` |
| 12 Drywall | core | — |
| 13 Exterior (accessibility core when it applies) | conditional | `has_ui` |
| 14 Inspections · load test part | core · conditional part | `server` |
| 15 Paint & Trim: marketing · onboarding · legal pages | conditional | `commercial` · `multi_user` · `commercial \| others_data` |
| 16 Walkthrough · beta ring part | core · conditional part | `multi_user` |
| 17 CO · rings 2–4 · live payments | core · conditional parts | `multi_user` · `payments` |
| 18 Maintenance | core | — |
- **Foreman's own build:** `idea_known` yes, `has_ui` yes, everything else no. Model policy: `plan = max`, substitution `fable → opus@xhigh`.

## 6. Claude Code integration contract

**Interactive launch** (every CLI step):

```text
claude --session-id <uuid>                       # chain start only
       | --resume <uuid> [--fork-session]        # continuing step
       --model <alias> --effort <level> --permission-mode <mode>
       --name fm-<project>-s<NN>-<step>
       --settings <app-data>/runs/<runId>/settings.json
       [--ax-screen-reader]                      # when Settings → Screen reader mode is on
       "<resolved prompt>"
```

- argv array, no shell, absolute allowlisted path to `claude`.
- Child environment: explicit allowlist; `ANTHROPIC_*`, `CLAUDE_CODE_USE_*`, AWS, GCP, Azure variables always removed; `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` passed when present; PATH from `/etc/paths`, `/etc/paths.d`, Homebrew, cargo and pnpm homes, and the captured login PATH.
- Run settings (SR-09): `disableAllHooks: false`, `permissions.disableBypassPermissionsMode: "disable"`, deny rules with absolute paths for the repo's `.foreman/` and for app data, and (P2) sensor hooks in exec form.
- Everything sent toward a Claude session is stripped of control and escape sequences (SR-28); terminal links and webview navigation go through Rust (SR-29).
- Confirmations before a launch (SR-19): overlay-added step, mode or model different from the template, first launch in a project, `auto`/`dontAsk`, and the first launch after Claude Code's version changed.
- Never `--bare`, `bypassPermissions`, `--dangerously-skip-permissions`, or `--allow-dangerously-skip-permissions`.

**Preflight** (`prepare_launch`; the full rules are SR-06 to SR-08 in `docs/requirements.md`): version vs the computed floor (2.1.293 with the current kit); contract probes on version change (P2); a config fingerprint over user settings, managed settings, every `CLAUDE.md` from the cwd to the repo root, `CLAUDE.local.md`, `.claude/settings*.json`, `.mcp.json`, `.claude/agents/**`, `.claude/skills/**`, `.claude/commands/**` and `.claude/rules/**`, with auth-changing keys blocked wherever they appear; `claude auth status` in the step cwd (`claude.ai` or `oauth_token`), cached against the fingerprint; transcript exists for continuing steps; inputs exist; run inputs filled; prompt doesn't start with `-`, `/` or `!`.

**Re-arm** (always started by Billy's click, never by an event): idle check (P1: Billy confirms; P2: sensor, including background work) → Ctrl-C, then `/exit` → wait for exit → relaunch with a divider that notes "permission grants reset". Never SIGTERM a non-idle session without confirmation. After a fork, the new chain's UUID comes from S-8's result (r5b M13).

**Desktop:** `claude --desktop` (no other flags); `claude --desktop --resume <uuid>` marks the chain handed off; the next Foreman step forks.

**Headless:** v1 makes **no `claude -p` calls**. The hardened headless argv for v1.1 is in `r5-resolution.md` §2.1.

**Contract probes** (P2): run in a fixed `<app-data>/probe/` folder Billy trusts once at first run, 20 s timeout, outcomes pass / flag failure / inconclusive (inconclusive keeps the last verified state). One `claude --init-only` call with Foreman's flags and a marker hook; one `--init-only --resume <random uuid>` parse probe.

**Version floors:** Fable 5.1 2.1.257 · `--permission-prompts` 2.1.259 · Opus 5.5 2.1.280 · `--agents` file form 2.1.281 · Sonnet 5.5 2.1.284 · `--desktop` 2.1.285 · Haiku 5.5 2.1.293. Effective floor = maximum over the steps' models, flags, and the pinned kit's agent models.

## 7. Files and data

| Where | File | Contents | Committed |
|---|---|---|---|
| Repo | `.foreman/project.json` | Variables, conditions + audit, model policy, spec folder, chat URLs, schemaVersion | Before Stage 5: at the moment of change, pathspec. From Stage 5: in the signing commit. |
| Repo | `.foreman/overlay.json` | Skips with reasons, project-only additions | As above |
| Repo | `.foreman/blueprint.lock.json` | Blueprint commit, file SHA-256, blueprint and grammar versions, and per inspection item `{textSha256, kind}` | As above |
| Repo | `.foreman/stages/stage-NN.json` | Provenance (immutable) | In the signing commit |
| Repo | `docs/handoff/stage-NN.md` (+ `-attachments.md`) | Packets | In the signing commit |
| Repo | `docs/lessons.md`, `docs/tickets.md`, `docs/change-requests/` | As in the blueprint | By Claude or Billy |
| App data | `settings.json`, `projects.json` | Settings, registry | — |
| App data | `projects/<id>/progress.json` | Step states, chains, gate runs, run inputs, pending decision changes | — |
| App data | `approvals/<id>.json` | Approved hashes of decision files; re-confirmed item kinds | — |
| App data | `runs/<runId>/{run.json, output.log, settings.json}`, `runs/index-YYYY-MM.jsonl`, `events/<runId>/` | Run record, output tail, run settings, monthly index, sensor events | — |
| App data | `templates/<hash>.md`, `bin/foreman-sensor-<hash>`, `probe/` | Locked blueprint text, sensor binary, probe folder | — |

Every JSON file has `schemaVersion` and a migration. Writes are atomic (`.<name>.foreman-tmp`, fsync, rename, fsync dir). A file newer than Foreman's last read is never overwritten. Exact schemas are written in Stage 4.4 (`docs/contracts/files.md`).

## 8. Packet rules

A packet is valid when:
1. The header is `=== HANDOFF PACKET · Stage N → Stage M ===`, N matches the file name, and M is the next applicable stage.
2. The stage's required fields are present.
3. `OPEN ITEMS:` is exactly `none`.
4. `APPROVED BY:` is unsigned (`<blank>` and its variants normalize to unsigned) or `<name> on <YYYY-MM-DD>`.
5. For Stage 4: `BLUEPRINT VERSION` equals the lock.
6. At signing, Foreman writes `SKIPPED:`, `SUBSTITUTIONS:` and `ATTESTED:` deterministically.

**Close-out per stage type** (P2 automates it; in P1 Billy closes stages by hand, guided by a checklist step):

| Stage type | Sequence |
|---|---|
| Chat stages (0, 1, 3, 4) | Packet pasted back and valid → attest the inspection items → F.2 lessons in the same chat, pasted back → Sign |
| Stage 2 (CLI council) | 2.6 writes the packet → attest the inspection items → F.2 as a followup in the 2.2 chain, writing to `docs/lessons.md` → Sign |
| Stages 5–17 (CLI) | Gate A (interactive) → Gate B writes the packet → Gate C lessons → Sign |

F.2 is one template step instantiated per stage with `chain=@stage` (the chain of the stage's packet-producing step). A stage's lessons requirement is met by an entry headed `## L-NNN · Stage N` or a "clean stage" entry for that stage.

**Signing** is hash-bound and verified: re-read and compare; write APPROVED BY and Foreman's fields atomically; commit **by git plumbing only** on the current branch (the default; Billy works on a branch per stage): `hash-object -w --no-filters` per path, a temporary index from HEAD, `update-index --cacheinfo`, `write-tree`, `commit-tree --no-gpg-sign -m "handoff: stage NN"`, `update-ref HEAD <new> <old>`, then `update-index --cacheinfo` on the real index for those paths only. Foreman never runs `git add`, `git commit`, `git status` or `git diff` (repo filter drivers and `gpg.program` would run), and v1 never GPG-signs commits. Every git call passes the fixed `-c` overrides (hooks, fsmonitor, pager, editors, gpg signing, ssh, credential helper and protocols off) and `GIT_TERMINAL_PROMPT=0` (`docs/requirements.md` SR-42, SR-45). Read every path back from the commit. A repo with no commits yet gets a parentless first commit; a detached HEAD is refused ("Check out a branch first"). "Sign on another branch" uses the same plumbing against that branch's tip and leaves HEAD, the index and the working tree untouched. Stage records hold intended and observed models; the commit's ID lives in app data, because the record is inside the commit.

## 9. Delivery plan

| Phase | Contents | Ends with |
|---|---|---|
| 0 · Prove the risky parts (Stage 5) | Scaffold; spikes S-0, S-1′, S-3, S-7′, S-8, S-12, S-23 (below), each with a decision rule | Spike reports in `docs/spikes/` |
| 1 · Thin end-to-end: start every TradeLog Stage 1–2 step through Foreman | F-001 (Stages 1–2, F.2), F-002, F-003 (harness 1–2), F-004, F-005, F-006 (tokens of Stages 1–2), F-007, F-008 (without probes), F-009 (tabs, PTY, key routing, archive), F-010 (manual idle), F-012 (copy and open, chips, links, paste-back of packets and outputs, normalization, attachments, fix request), F-013 (watch, validate), F-017 (Next step + ⌘⇧↵), F-018 (setup form), F-019 (first run, version, auth), F-020, F-021, F-022, F-026 (log + Export metrics), F-028 (core shortcuts), F-029 (paths, DRI), F-030 (minimal stage workspace). Close-out (gates, lessons, signing) by hand. | **TradeLog Stages 1–2 through Foreman; kill check** |
| 2 · Complete v1 | F-001 (Stages 0–5, Gate), F-003 (screen), F-006 (Stages 3–5), F-009 (Now/Next, round tracker, focus), F-011, F-012 (ADR split), F-013 (repair, capture), F-014, F-015, F-016, F-017 (inbox), F-018 (wizard), F-019 (Doctor, probes), F-023, F-024, F-025, F-026 (panel, run log screen), F-027, F-028 (palette), F-029 (rest), F-030 (sheet strip, title block) | **TradeLog Stages 3–5 through Foreman; success metrics read** |
| v1.1 | F-101 … F-108 | v1.1 metric |

**Estimate** (restated from the Stage 4.6 tickets, each sized for one Claude Code session of 1–4 hours of Billy's time): **110 tickets come before the kill check** (M0–M3), about **110–440 hours**; v1 is 160 tickets, about 160–640 hours. At Billy's pace that is roughly 3–11 weeks to the kill check at 40 hours a week, longer at part time; calendar time is longer still. The earlier week estimate from the r6 sizes (kill check at 8–14 weeks) is superseded by this count.

**Success metrics (App Brief):** TradeLog Stages 1–5 run entirely through Foreman; zero manual prompt edits; zero gates skipped by accident; starting any step takes under 10 seconds (measured as **p95 ≤ 10 s**, excluding time spent typing run inputs).

**Kill criterion (App Brief), made measurable.** Two measures, defined the same way for Foreman and for the by-hand baseline (`docs/metrics-baseline.md`), compared per surface (chat, CLI):
- **Time to start a step:** from the moment Billy begins looking for the next step (opening the blueprint or prompts file by hand; focusing Foreman or pressing Do next) to the step being ready to run, with the same end point on both sides: for CLI, the spawn (by hand, Enter on the typed command); for chat, the filled prompt on the clipboard and the target chat open.
- **Active time per step:** from that start point to the step done (packet saved, output pasted back, session exited), excluding waits (Claude working, the 7-day test), so Foreman's own overhead (dialogs, confirmations, re-arm, paste-back) counts. Compared as the median per surface.

After TradeLog Stages 1–2: if Foreman's median on either measure isn't lower than the by-hand median for the same surface, or any step needed a manual prompt edit, stop and simplify. The by-hand baseline starts with Foreman's own Stage 5 on Billy's Mac (CLI); for chat, at least 10 chat steps Billy runs by hand, in any project, between Stage 5 and the kill check are logged (a surface with fewer can't pass).

**v1.1 metric:** one TradeLog milestone through the CLI Crew Loop with zero manual routing and zero lost ticket state. **v1.1 kill criterion:** if routing a ticket through Foreman's loop is slower than the Desktop loop by hand over that milestone, reduce the loop to a launcher.

### Spikes (none can run before the freeze; each has both branches specified)

| ID | Question | Pass | Fail |
|---|---|---|---|
| S-0 | PTY + xterm 6 + real Claude TUI in WKWebView: Shift+Tab, 5 KB paste, resize while streaming, ⌘K routing, ⌘C/⌘V in the terminal, VoiceOver, memory with 3 tabs after 30 min; and the strict CSP (`style-src 'self'` + hashes) with Bits UI, Svelte transitions and xterm | As specified | Scrollback cap and archive; a key-routing layer; `--ax-screen-reader`; memory NFR raised to measured + 20%; CSP: `style-src 'unsafe-inline'` only (scripts stay strict), recorded as accepted risk in the ADR; a fundamental TUI failure reopens the shell choice (SwiftUI + SwiftTerm) |
| S-1′ | Per-launch hooks fire, run as the contract probe in the trusted probe folder | Sensor ships in P2 | No sensor; manual idle confirmation stays |
| S-3 | `--effort` on resume | Flag | `effortLevel` via `--settings`; else Billy types `/effort`, warned that it saves as his default |
| S-7′ | Settings `env` / `apiKeyHelper` in a trusted repo | Fingerprint and block rules suffice | Also block any project `env` block until Billy approves it |
| S-8 | Graceful end with text in the input box and with background work; Ctrl-C vs `/exit` vs SIGTERM vs SIGHUP; `--resume <uuid> --fork-session --session-id <new>`; re-arm latency | As specified; fork IDs chosen by Foreman | Graceful end: Billy ends the session, then Foreman relaunches. Fork IDs: the only new transcript after launch; if ambiguous, a fresh chain with context |
| S-12 | PATH in a Finder-launched build | As specified | Login-PATH capture mandatory at first run |
| S-23 | Does `--desktop --resume` continue the same transcript? | Shared: the chain is handed off and the next Foreman step forks | Copied into Desktop's own session: no fork needed; the CLI chain continues unaffected |
| S-2′ | Hostile-repo headless gate (v1.1 only) | Headless Gate A ships in v1.1 | It doesn't; interactive stays |

## 10. Stack (final)

Evidence and alternatives: `spec/council/r7-stack.md`. Frontend: **Billy's decision is SvelteKit 3**; the council's alternative (plain Svelte 5 + Vite) is recorded in r7.

| Layer | Choice | Reason |
|---|---|---|
| Desktop shell | Tauri 2.12.4 | Small bundle; only Rust spawns or touches files; native window effects built in. Electron needs an 8-week upgrade cycle and an app-applied security checklist; SwiftUI + SwiftTerm loses on solo build speed and is the fallback if S-0 fails fundamentally. |
| Frontend | SvelteKit 3.1.0 (SPA: adapter-static 4.0.0, `fallback: 'index.html'`, `ssr = false`) + Svelte 5.57.2 + Vite 8.3.4; `vite.config.ts` holds the SvelteKit config (`svelte.config.js` isn't supported in v3); `#lib` imports; tsconfig extends `$app/tsconfig` | Billy's stack across all his projects: file-based routes and layouts, one mental model. Verified to build and type-check clean (r1 §A4). SvelteKit's one inline bootstrap script is hash-pinned by Tauri at compile time; never `'unsafe-inline'`. |
| Language | TypeScript 6.0.3 strict · Rust stable ≥ 1.90 (pinned in `rust-toolchain.toml`) | TS 7 breaks `svelte-check` and `typescript-eslint` today; Tauri crates need Rust ≥ 1.90. |
| Package manager / runtime | pnpm 12.14.0 · Node ≥ 22.17 | Strict, reproducible. |
| Terminal | @xterm/xterm 6.0.0 (+ fit, webgl, unicode11, search, web-links addons) | The standard; screen-reader mode. |
| PTY | portable-pty 0.9.0 in Foreman's own wrapper | Rust picks the program. |
| Process spawning | `std::process` / `tokio::process`, argv only | No shell plugin. |
| File watching | notify 8.2.0 + notify-debouncer-full 0.7.0 | FSEvents; 9.x is an RC. |
| Markdown | comrak =0.56.0 | Line and column on every node. |
| UI primitives | Hand-built, keyboard-complete (ADR-020 amendment 2026-10-20) | bits-ui was not adopted: its Command primitive (⌘K) ships static `style` attributes that violate `style-src 'self'`. |
| State | Svelte 5 runes in `.svelte.ts`, mirroring Rust over commands, Channels and events | Rust is the source of truth. |
| Tests | cargo test + proptest 1.12.0 + insta 1.49.0 + criterion 0.8.2 (benchmarks) · Vitest 5.0.5 (browser mode) · Playwright 1.64.1 + axe 4.14.0 with mocked IPC · @wdio/tauri-service 1.5.0 smoke (webdriverio 9.32.0) behind an `e2e` feature | `tauri-driver` has no macOS support. |
| Lint and format | eslint 10.13.0 + eslint-plugin-svelte 3.23.1 + typescript-eslint 8.72.0 · svelte-check 4.7.7 · prettier 3.9.10 · rustfmt + clippy `-D warnings` | Zero-warning gates. |
| Tauri plugins | opener 2.7.2, clipboard-manager 2.5.0, dialog 2.8.3, window-state 2.5.0; notification 2.7.0 in **P1** with the `wait` steps (a wait whose date passed while Foreman was closed notifies at the next launch); P2 adds inbox notifications | No shell, fs or pty plugins. |
| Hardening | `freezePrototype: true`; `AppManifest::commands`; capabilities per window; explicit `csp` (`script-src 'self'`, `connect-src` Tauri IPC only) and separate `devCsp` | SR-06, SR-08. |

Full pinned list (Rust crates and JS packages): `r7-stack.md` PART 3.

## 11. Non-functional requirements

Full list with measurement methods: `spec/council/r8-nfr.md` (NFR-01 to NFR-48). The key numbers:

| Area | Target |
|---|---|
| Cold start to interactive Today | p95 ≤ 1.5 s |
| Project switch | p95 ≤ 300 ms (19 stages, 200 run records) |
| Template load | p95 ≤ 50 ms (cached ≤ 5 ms) |
| Resolution + preflight | p95 ≤ 50 ms |
| Keystroke to spawn | p95 ≤ 250 ms |
| Packet visible after write | p95 ≤ 1 s |
| Time to start a step | p95 ≤ 10 s (excluding typing run inputs) |
| Memory | ≤ 300 MB with 3 terminal tabs (verified in S-0) |
| Data safety | No loss or corruption, ever; crash-safe run state; external edits win |
| Security | ASVS 5.0 L2 for applicable chapters; Tauri hardening in full; threat model includes the malicious repo |
| Accessibility | WCAG 2.2 AA; full keyboard; VoiceOver; Reduce Motion / Transparency / Increase Contrast |
| Platform | macOS 27 and macOS 26, Apple Silicon; Claude Code ≥ 2.1.293 (computed floor), subscription login |
| Network | Foreman makes no network calls of its own |
| Cost | $0 infrastructure; no Claude usage without an explicit action |
| Quality | TS strict (+ `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), zero warnings; clippy pedantic on the core crate; no `unwrap` outside tests; `unsafe` forbidden |

## 12. Security and privacy requirements (input to Stage 3)

**Superseded by `docs/requirements.md`** (Stage 3, after its security review), which has the complete list (SR-01 to SR-64, PR-01 to PR-08, AR-01) and a mapping from the IDs below. Where they differ, `docs/requirements.md` wins.

| ID | Requirement |
|---|---|
| SR-01 | Processes are spawned only from allowlisted absolute paths, with argv arrays. No shell, except the single consented login-PATH capture in Doctor. |
| SR-02 | Never `bypassPermissions`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, or `--bare`. |
| SR-03 | The exact argv and prompt are visible before every run; every run is logged. |
| SR-04 | Every Foreman write passes the path policy (F-021: repo-relative, deny by path component at any depth, symlink refusal); the webview holds no plugin permissions. |
| SR-05 | Repo content (packets, lessons, tickets, overlay, chat output) is data: rendered as text, never as HTML, never executed, never a source of write destinations except through the path policy. |
| SR-06 | Strict CSP: `script-src 'self'`, no inline scripts, no remote origins. |
| SR-07 | Overlay-added steps and any mode different from the template's are highlighted and confirmed before running. |
| SR-08 | No telemetry and no network calls by Foreman. |
| SR-09 | Secrets are never read or displayed; `.env*` excluded from views and watchers; sensor events retain no assistant text beyond the last 20 events per run, deleted at stage sign. |
| SR-10 | Child environment is an explicit allowlist; provider and API-key variables are always removed. |
| SR-11 | Auth-changing keys in a step cwd's Claude config block the launch; any change to that config needs acknowledgement; auth is checked in the step's cwd. |
| SR-12 | Decision files changed from outside Foreman take effect only after Billy accepts the diff. |
| SR-13 | v1 makes no `claude -p` calls. (v1.1: verify commands are approved objects, linted against shells, interpreters, generic runners, network tools and git writes; risk register: approved verify commands execute repository code.) |
| SR-14 | Sensor events carry a per-run token (it stops cross-run mix-ups and other users' processes, not a forger running as Billy); hooks use exec form; no sensor event triggers an action by itself. |
| SR-15 | Signing commits include only named paths and are verified by read-back; every git call Foreman makes disables repo hooks, fsmonitor, pagers, external diff and editors. |
| PR-01 | All data stays on Billy's Mac. |

Stage 3.2 turns these into the full SR/PR list in `docs/requirements.md`.

## 13. Non-goals and rejected ideas

**Non-goals (v1):** cloud sync · other users or teams · payments · Windows or Linux · Intel Macs · App Store or notarized distribution · calling the Claude API or using API keys · editing code inside Foreman · replacing Claude Code's UI · editing the blueprint (Foreman proposes; Billy applies) · Fable in Foreman's own build.

**Rejected:** see `r6-feature-specs.md` §6 (YAML source, inference as a source, positional IDs, headless Gate A in v1, user-level hooks, typed TUI keystrokes, `--bg` substrate, Agent SDK/API, browser automation of claude.ai, pty/shell/fs plugins, plain Svelte 5 + Vite with a hand-rolled router, Electron, a v1 dashboard).

## 14. Design direction

Appendix A (below) is the design direction, with the council's changes applied.

## 15. Open questions

None.

## 16. Change control

After v1.0, every change requires a Change Request (`docs/change-requests/CR-NNN.md`, from `docs/templates/change-request.md`): what, why, impact on scope, ADRs, timeline and cost, approved by Billy. Until the v1.1 CR track exists, CRs are handled by hand with that template.

## 17. Changes from the App Brief (Billy accepts or rejects these at the Stage 2 sign)

| App Brief item | Frozen spec | Why |
|---|---|---|
| Runs stage gates with "optional headless inspection that returns structured PASS/FAIL results" | v1 gates are interactive with Import results; headless Gate A, `auto` items and verify commands are v1.1 (F-101) | The red team's fatal objection R-01: a `claude -p` run loads the repo's hooks, env, helpers and MCP servers; little v1 use (R-14) |
| Reads `docs/tickets.md` and runs the Crew Loop per ticket | v1.1 (F-102); the strict ticket format (B-4) is in v1 | No Stage 1–5 step uses tickets; the metric run doesn't need it |
| Add: prompts, inspection items, **or agents**, "this project only" **or "promote to blueprint"** | v1: Add prompt and Add inspection item, this project only (F-025, P2). Agents and promote-to-blueprint: v1.1 (F-104) | Keeps v1 to what the metric run needs |
| Lessons: "turns promote-to-blueprint into a proposed, versioned edit" | v1 captures lessons (F-016); promoting is v1.1 (F-104, F-107) | Same |
| Change Requests: records CRs; a CR can reactivate a not-applicable stage | v1.1 (F-103); conditions are freely editable until the Stage 2 sign, so the metric run needs no CR | Same |
| Dashboard: progress, time per stage, runs per ticket, CO checklist | Today + Next step in v1; metric panel in P2; dashboard v1.1 (F-108) | Today answers "what next"; the dashboard isn't on the metric path |
| Stage map with live status | Sheet strip in P2; P1 has a minimal stage workspace | Phase 1 is cut to reach the kill check sooner |
| Starting any step takes under 10 seconds | p95 ≤ 10 s, excluding typing run inputs | Makes the metric measurable |
| Kill criterion after two TradeLog stages | Unchanged, measured per surface on two defined measures (§9); reached after 110 tickets, about 110–440 hours (§9, restated at Stage 4.7) | The freeze review's re-estimate (r5b B1), restated from the Stage 4.6 tickets |
| Lets me sign packets | In P1, Billy signs by hand as today; Foreman's signing ships in P2 | Same as above |
| Blueprint v2.1 as the template | Foreman v1 targets blueprint v2.2 (`spec/blueprint-v2.2/PROPOSAL.md`); signing the Stage 2 packet approves applying it | Annotations, packet files and strict tickets are needed to run the blueprint without guesses |

---

## Appendix A · Design direction (v1.0, council amendments applied)

Source: `docs/spec-input/02-design-direction.md` v0.9, folded in here with the Stage 2 council's changes. Stage 4.5 turns it into tokens, components and reviewable mockups.

### A.1 The idea: a drawing set, not a dashboard

Construction runs on a **drawing set**: a bound stack of numbered sheets, each with a **title block** in the corner (project, sheet number, revision, who approved it), all printed in cyanotype blue. Builders flip to the sheet they need, read the callouts, build, and get it stamped.

Foreman borrows that vernacular, because it *is* the subject: the 19 stages are the sheets, each step is a callout, each signed packet is a stamp. That gives Foreman an identity no generic tool has, and every one of these devices carries real information.

**Where the boldness goes:** one place only, the **sheet strip**. It's a cyanotype band across the top of a project, showing all 19 stages as numbered sheets with fine white linework, phase colors, and approval stamps. Everything else stays quiet, native, and disciplined, so the strip and the next action are what your eye finds.

### A.2 Palette

Two themes: **Night** (default, dark) and **Vellum** (light). They follow the system setting unless you override it.

| Token | Night | Vellum | Use |
|---|---|---|---|
| `--cyanotype` | `#1E4E8C` | `#1E4E8C` | Sheet strip background; the primary action |
| `--linework` | `#E6EEF8` | `#E6EEF8` | Linework and text on cyanotype |
| `--ground` | `#191D22` | `#F2F4F6` | Window background |
| `--sheet` | `#20252B` | `#FFFFFF` | Content surface |
| `--rule` | `#313841` | `#D6DBE2` | Hairlines and dividers |
| `--ink` | `#E4E8ED` | `#1C232B` | Primary text |
| `--ink-muted` | `#9AA4B0` | `#5A6573` | Secondary text |
| `--pass` | `#3E9B63` | `#2E7D4F` | Inspection passed, signed |
| `--fail` | `#D9534A` | `#B8352D` | Inspection failed, invalid packet |
| `--attention` | `#F07A22` | `#C65F12` | Needs your decision or signature (used sparingly) |

Phase colors identify stages, muted so they don't compete with status:

| Phase | Token | Hex |
|---|---|---|
| Pre-Construction (0–4) | `--phase-pre` | `#4A7CC0` |
| Structure (5–8) | `--phase-structure` | `#B0663A` |
| Systems (9–11) | `--phase-systems` | `#3A938C` |
| Finishes (12–15) | `--phase-finish` | `#8064B0` |
| Occupancy (16–17) | `--phase-co` | `#4A9A5E` |
| Ownership (18) | `--phase-maint` | `#77808C` |

Rules:
- Status always uses shape plus color (a stamp, a check, a cross, a lock), never color alone.
- `--attention` appears only for something waiting on you: an unsigned valid packet, a decision, a failing gate you must look at.
- Every text and background pair must meet WCAG 2.2 AA (4.5:1 for text, 3:1 for large text and UI components). **The hex values above are provisional:** the freeze review computed `--fail` on Night `--sheet` at about 3.9:1, `--attention` on Vellum white at about 4.2:1, and `--pass` on Night `--sheet` just under 4.5:1. Stage 4.5 adjusts the tokens and ships a computed contrast table that must pass before the design review; Stage 13 re-checks.

### A.3 Type

| Role | Face | Notes |
|---|---|---|
| Interface | SF Pro (`system-ui`, `-apple-system`) | Native macOS; Foreman should feel like a Mac app, not a website |
| Prompts, commands, packets | SF Mono (`ui-monospace`) | Only for real code and literal text you'll run or paste |
| Sheet numbers and title blocks | IBM Plex Sans Condensed, SemiBold (bundled locally, OFL) | The drafting-lettering voice, used only in the sheet strip and title blocks |

Scale (pt): 11 · 12 · 13 (body) · 15 · 18 · 22 · 28. Body line height 1.45. Prompt text in mono at 12.5 pt with line height 1.5 and a measure of at most 88 characters. Sentence case everywhere; no all-caps labels.

### A.4 Layout

```text
┌────────────┬──────────────────────────────────────────────────────┬───────────────┐
│ Projects   │  Sheet strip: 0 1 2 3 4 │ 5 6 7 8 │ 9 10 11 │ …  18     │  Inspector    │
│  TradeLog  │══════════════════════════════════════════════════════│               │
│  Foreman   │  Stage 6 · Foundation              ┌ title block ──┐ │  Packet       │
│  Swings    │  Database, auth, domain core       │ sheet 6 of 19 │ │  Gate         │
│            │                                    │ rev  bp 2.1   │ │  Lessons      │
│ Stages     │                                    │ DRI  Billy    │ │               │
│  ✓ 0 … 5   │                                    │ in progress   │ │  (contextual  │
│  ● 6       │                                    └───────────────┘ │   to the      │
│  ○ 7 … 18  │  6.1  Migrations          CLI · Opus 5.5 · high · Plan│   selected    │
│            │       ───────────────────────────────────────────────│   step or     │
│            │       resolved prompt (mono)…                        │   gate)       │
│            │       [Run in terminal ⌘↵] [Copy prompt]             │               │
│            │  +                                                   │               │
│            │  6.2  Authentication   n/a · single-user app         │               │
│            │  +                                                   │               │
├────────────┴──────────────────────────────────────────────────────┴───────────────┤
│  Terminal drawer: tabs per run (fm-tradelog-s06-6-1 …)                           ⌃` │
└────────────────────────────────────────────────────────────────────────────────────┘
```

- Left-aligned throughout. The sidebar uses macOS sidebar vibrancy; the title bar is transparent with the traffic lights inset.
- Steps are **not cards**. They're callouts on a sheet: a step number in the left margin (a real sequence), a one-line spec line (surface · model · effort · mode · agents), then the prompt. Hairline rules separate steps. Radius is 6 px on interactive controls only; the sheet itself has square edges, like paper.
- The inspector on the right is contextual: the selected step's packet, the stage's gate, or lessons. It collapses with ⌘⌥I.
- The terminal drawer slides up from the bottom, one tab per run, resizable, and remembers its height.

### A.5 Screens

1. **Today** (home): everything waiting on you across all projects, then each project's Next step with its spec line and primary action (P1 shows the Next step rows; P2 adds the inbox). Empty state: "Start your first project" with the button that opens setup (the form in P1, the wizard in P2).
2. **New project wizard** (P2; P1 is a single setup form with the same fields), four short steps: name and folder → variables → the eight condition questions (each with one line on what it changes) → review the tailored path (which stages apply, which are not applicable, and why) → Create.
3. **Project** (P1: stage workspace and terminal drawer; P2: sheet strip, title block, inspector): sheet strip + stage workspace + inspector + terminal drawer (the layout above).
4. **Gate** (P2): the stage's inspection items, each showing its kind: attest (Done with a note, or Not applicable with a reason) or `time` (date); `auto` items (Run, with exit code and output) arrive in v1.1. "Run Gate A in terminal" and Import results; review order FAIL → waiting → PASS; then Packet → Lessons → Sign. Sign is disabled until the gate passes, and the reason is stated. (Headless Gate A is v1.1.)
5. **Tickets** (the Crew Loop, v1.1): tickets grouped by milestone, each showing class, routed model and effort, loop step (Plan → Build → Review → Commit → Close), and its session. One action: "Run next loop step".
6. **Lessons** (v1.1, F-107): list with filters (stage, root cause, status). Detail with "Promote to blueprint". In v1, lessons are captured by F.2 and Gate C into `docs/lessons.md`.
7. **Change requests** (v1.1): list and detail, with the conditions and stages each CR changes.
8. **Run log** (P2, F-026): searchable history of every run, with argv and session name; "Resume" where the transcript still exists.
9. **Doctor** (P1: first-run checks; P2: full): each environment check with a status, the version found, and how to fix any failure.
10. **Settings**.
11. **Command palette** (⌘K): projects, stages, steps, tickets, and every action.

### A.6 Components

| Component | Notes |
|---|---|
| Sheet strip | 19 sheets grouped by phase. Each shows its number, phase color edge, and a status mark: blank (locked), outline (ready), filled dot (in progress), magnifier (inspecting), stamp (signed), hatched (not applicable), dashed outline (needs annotation), question mark (a condition is undecided). Hover shows the stage name; click opens it. |
| Title block | Bottom-right drafting box per stage: sheet number, revision (pinned blueprint version), DRI, status, date signed. |
| Spec line | The run config: surface icon, model, effort, mode, agents. Edited values (from the overlay) are underlined with a diff on hover. |
| Prompt block | Mono, wrapped, read-only by default. Resolved variables show subtly; unresolved ones show as attention pills you can click to fill. Run inputs appear as a form above it; optional parts show an Include toggle. (Editing a prompt for one project is v1.1, F-104.) |
| Step actions | One primary action that matches the surface: "Run in terminal", "Open in Desktop", or "Copy for chat". Secondary: Copy prompt, Copy command, Skip (only when allowed, with a reason field). |
| Insert point (+) (P2) | Between steps: Add prompt, Add inspection item, both "this project only" (F-025). Add agent, Add note and "Propose for the blueprint" are v1.1 (F-104). |
| Lock | On core items: "Core: protects production quality. Change it in the blueprint with a Change Request." |
| Not-applicable row | Hatched, one line: "Not applicable: this app doesn't take payments (payments = no)." Reactivates if a CR flips the condition. |
| Packet view | The packet's fields as a readable form, with validation problems inline. Sign shows your name and the date before confirming. |
| Inspection row | Item, status stamp, evidence, artifacts (paths and commands are copyable). |
| Terminal tab | Named after the session; states running · idle (P2) · blocked (P2) · exited · errored · interrupted. Closing a running tab asks first. |

### A.7 Interaction and keyboard

| Action | Shortcut |
|---|---|
| Command palette | ⌘K |
| Run the selected step | ⌘↵ |
| Copy prompt · copy command | ⌘⇧C · ⌘⌥C |
| Next / previous step | J / K (or ↓ / ↑) |
| Next / previous stage | ⌘] / ⌘[ |
| Jump to a stage | ⌘G, then type the number |
| Toggle terminal drawer · inspector | ⌃` · ⌘⌥I |
| Open gate for this stage | ⌘⇧G |
| New project | ⌘N |
| Settings | ⌘, |
| Do next (top item, from anywhere) | ⌘⇧↵ |
| Paste back into the selected chat step | ⌘⇧V |
| Mark a chat step done | ⌘D |
| Terminal focus mode · return focus from terminal | ⌘⇧T · ⌘⇧E |
| Pinned projects | ⌘1 … ⌘9 |

- Focus is always visible (a 2 px ring with a 2 px offset: `--cyanotype` in Vellum, `--linework` in Night, where `--cyanotype` on `--ground` is only about 2:1), and the full flow works without a mouse.
- Destructive or irreversible actions (closing a running terminal, signing, applying a CR) confirm, and say exactly what will happen.

### A.8 Motion

One orchestrated moment: **the stamp**. When you sign a packet, the stage's sheet in the strip gets an approval stamp (a 220 ms scale-and-settle) and the next stage turns ready. Everything else moves only in answer to an action: the terminal drawer slides (160 ms), the inspector expands (140 ms), with ease-out. All motion turns off under `prefers-reduced-motion`.

### A.9 Writing

- Name things by what they do: "Run in terminal", not "Execute". The same verb follows through: "Sign packet" → toast "Packet signed".
- Errors say what happened and how to fix it: "Claude Code 2.1.270 found. Opus 5.5 needs 2.1.280 or later. Run `claude update`, then check again."
- Empty states invite the next action: "No tickets yet. They come from Stage 4.6. Open Stage 4."
- Not-applicable and locked states always explain why, in one line.

### A.10 Quality floor

WCAG 2.2 AA contrast in both themes, full keyboard operation, VoiceOver labels on every control and status, `prefers-reduced-motion` honored, and a layout that works from a 1280 × 800 window up to full screen. In narrower windows, the inspector collapses first, then the sidebar collapses to icons.

### A.11 Additional surfaces and platform notes from the council

- **Step block:** the resolved prompt and the exact argv are always one click away (collapsed under the primary action); run inputs appear as an inline form above the prompt; remapped packet paths show a subtle note.
- **Terminal drawer:** tab states running · idle · blocked · exited · errored · interrupted; after a re-arm, the previous buffer is an archived, collapsed section; the Now/Next strip (P2) sits above each tab.
- **New surfaces:** Pending outside change (diff with Accept / Reject), tamper event, broken chain, config-changed banner, Template health, Interrupted sessions, Doctor capability table.
- **Wizard review step** lists the exact file writes: create, leave alone, refuse to overwrite.
- **Platform:** macOS 27 and macOS 26 both support Liquid Glass window effects; a Visual Effect material is the fallback; Reduce Transparency turns window effects off.
