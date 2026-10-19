# Round 2 · Product-visionary · Proposals

Author: product-visionary (Stage 2 council) · Date: 2026-10-19
Inputs: `00-app-brief.md`, `01-master-spec.md` v0.9, `02-design-direction.md` v0.9, `council/r1-workflow.md` (main input), `claude-build-blueprint.md` v2.1, `03-build-prompts.md`.
`council/r1-engineering.md` did not exist when this round started. Every proposal that depends on a Claude Code behavior I could not confirm in the official docs is marked **needs engineering confirmation (NEC)** and tied to a spike in §6.

**The v1 rule used throughout.** Billy's success metric is: *TradeLog runs Stages 1–5 through Foreman with zero manual prompt edits, and any step starts in under 10 seconds from opening the app.* A feature is **v1** only if that run needs it, or if it protects the "zero gates skipped by accident" metric during that run. Everything else is **v1.x**, even when the brief lists it. Where a v1.x feature affects a data shape, the shape is fixed in v1 so nothing needs a migration later.

Definition used for the metric: a **manual prompt edit** is any change Billy makes to resolved prompt text outside the step's declared run inputs. Typing the answer to `[paste numbers]` into a declared input field does not count. Copying a prompt out and editing it elsewhere does count.

Build sizes for a solo principal engineer working with Claude Code: **S** ≤ 2 days · **M** ≤ 1 week · **L** ≤ 3 weeks · **XL** > 3 weeks.

---

## 1. Summary

Five bets carry Foreman from "a prompt library with a terminal" to "the process runs itself correctly":

1. **The blueprint becomes executable without stopping being prose.** Foreman infers structure from the prose and reads a thin layer of `fm:` HTML-comment annotations where inference can't. A drift detector flags any disagreement between the two (P-01). A golden harness proves before Billy's first click that every TradeLog step in Stages 1–5 resolves with zero unresolved tokens (P-03).
2. **Every step launches with exact settings, including "same session" steps.** A *session chain* owns one Claude session UUID. Each step that continues the chain **re-arms** it: Foreman ends the idle process and relaunches `claude --resume <uuid>` with that step's own `--model`, `--effort` and `--permission-mode`. The docs confirm that `--model` and `--permission-mode` override the restored values. Billy never types `/model` or presses Shift+Tab (P-05).
3. **Foreman can see inside the sessions it launches, with no global install.** Every launch carries a per-run `--settings` file that adds a tiny Foreman hook. The hook reports idle, blocked, permission mode, effort and the last assistant message. Those events drive the Today inbox, safe re-arming, the mode guard, and capture of packets printed in the terminal (P-06, NEC).
4. **Gates can't lie, and signing can't go wrong.** Each inspection item has a verification kind. A headless run can never pass an `attest` item. Signing is bound to a hash, commits only named paths, works in two phases, and records provenance (P-13, P-14).
5. **One screen answers "what do I do now?"** The Today inbox spans every project. ⌘⇧↵ runs the top item, so getting from opening the app to a running step is two keystrokes (P-15).

### 1.1 Summary table

| ID | Proposal | Solves (r1 IDs) | Scope | Size | Risk |
|---|---|---|---|---|---|
| P-01 | Template annotation layer + prose inference + drift detector | J4-1, J4-2, J4-3, J4-7, J4-9, J3-2, J3-5, J4-5, J6-1, J6-7, J10-1, J10-2, §12, spec Q1/Q6 | v1 | L | Med |
| P-02 | Stable step IDs + blueprint lock (source snapshot at pinned SHA) | J2-5, J9-8 (base), J10-5 (base) | v1 | M | Low |
| P-03 | Template health screen + TradeLog acceptance harness | J1-1 (proof), riskiest assumption, success metric | v1 | S | Low |
| P-04 | Run config v2: surface, cwd, start, switches, alternatives, model policy | J4-2, J4-3, J4-9, F-021 | v1 | M | Low |
| P-05 | Session chains with re-arm continuation | **J4-1 (blocker)**, J6-3, J7-2, J9-4 | v1 | M | Med (NEC: effort on resume, graceful exit) |
| P-06 | Session sensor: per-launch hooks via `--settings` | J4-4, J4-5, J4-8, J8-1, J9-6 | v1 (degrades gracefully) | M | Med (NEC) |
| P-07 | Launch pipeline: preflight, env allowlist, auth guard, preview-not-modal | J1-2, J1-3, J9-1, J4-10, J10-2, J1-5 | v1 | M | Low |
| P-08 | Terminal ergonomics: Now/Next strip, round tracker, focus mode, key routing, tab states | J4-3, J4-5, J4-6, J4-11, J9-7 | v1 | M | Low |
| P-09 | Resolver v2: token classes, remembered run inputs, path aliases, applicable-path remap | J3-2, J5-5, J7-6, J2-1 | v1 | M | Med |
| P-10 | Step kinds and step states | J3-1, J4-7, J2-8 (enum) | v1 | M | Low |
| P-11 | Chat bridge | J3-3, J3-5, J3-6, J3-7, J3-8, J3-9, J2-4 | v1 | L | Low |
| P-12 | Build-package assembler + Project-knowledge manifest | J3-4, J3-5, J2-3 | v1 | M | Low |
| P-13 | Packet engine + safe signing + provenance | J5-1…J5-8, J5-10, J3-7, J9-3, J3-10 | v1 | M | Med |
| P-14 | Honest gates | **J6-1 (blocker)**, J6-2…J6-5, J6-8 | v1 | L | Med (NEC: S-2) |
| P-15 | Today inbox, attention model, Do next | J8-1, J8-2, J8-3, J8-4, J5-10 | v1 | M | Low |
| P-16 | First run, Doctor, wizard | J1-1…J1-7, J2-1…J2-7 | v1 | L | Low |
| P-17 | State layout (repo vs app data) + resilience | J7-3 (decision), J9-4, J9-5, J9-9, r1 §17 Q2 | v1 | M | Low |
| P-18 | Metric panel (kill criterion, live) | J8-7 | v1 | S | Low |
| P-19 | Crew Loop on CLI worktrees (designed now, ships v1.x) | **J7-1 (blocker)**, J7-2…J7-9, J6-6 | v1.x (+ v1 blueprint ticket format) | L | Med (NEC) |
| P-20 | Amendments: the CR track (designed now, ships v1.x) | **J11-1 (blocker)**, J11-2…J11-5 | v1.x (data shape v1) | L | Med |
| P-21 | Overlay authoring: add step, project edits, stale-edit, proposals, upgrade pin | J10-3, J10-5, J10-6, J9-8, J9-10 | v1.x | L | Low |
| P-22 | Usage meter from Claude Code's own rate-limit data + Fable guard | J8-5, J6-9 | v1.x | S | Low (NEC) |
| P-23 | Desktop handoff, including opening a CLI session in Desktop | F-009, J7-1 (opt-in), J7-9 | v1.x | S | Low |
| P-24 | Live send-to-session and the "go" button | J4-1 (alt.), J4-5 | v1.x (stretch v1 if S-4 passes early) | S | Med |
| P-25 | Lessons view, conditional prompt blocks, retro starter, run sheet export | J10-1, J6-7, J10-7, J10-8, J5-9, J5-11, J2-9, J4-12, J8-6 | v1.x | M | Low |

The five r1 blockers map to P-05 (J4-1), P-04 (J4-2), P-14 (J6-1), P-19 (J7-1) and P-20 (J11-1). Two of them, J7-1 and J11-1, can't be reached in a Stage 1–5 run: the Crew Loop starts at Stage 6, and CRs only matter after a freeze, when a condition flips. Both are **fully designed here** so the v1 data model already carries them, and **shipped in v1.x**. §8 argues this call explicitly.

---

## 2. Evidence this round relied on (beyond r1 §13)

Checked against official docs on 2026-10-19. Each fact changes a design decision below.

| # | Fact | Source | Used in |
|---|---|---|---|
| E-1 | On resume, "A model you pick for the new launch with `--model` … still takes precedence over the restored model." | [model-config](https://code.claude.com/docs/en/model-config) | P-05 |
| E-2 | Terminal resume restores the session's permission mode; "Pass `--permission-mode` … to override the restored mode." | [sessions](https://code.claude.com/docs/en/sessions) | P-05 |
| E-3 | The docs don't say whether `--effort` applies on resume. `--effort` "Set[s] the effort level for the current session … does not persist." | [cli-reference](https://code.claude.com/docs/en/cli-reference) | P-05 (spike S-3, narrowed) |
| E-4 | `--resume <session-id>` searches the current project and its worktrees, then every project. A name that matches more than one session opens the picker. | sessions | P-05 (use UUIDs, not names) |
| E-5 | "If the session depended on `--mcp-config`, `--settings`, … pass them again when you resume." | sessions | P-05, P-06 (Foreman rebuilds the full argv on every launch) |
| E-6 | Resuming a session that ended inside a worktree returns it to that worktree when launched from the main checkout. | [worktrees](https://code.claude.com/docs/en/worktrees) | P-19 |
| E-7 | `-w <name>` on an existing directory reopens that worktree. Branch `worktree-<name>`. The default base is `"fresh"`, the remote's default branch; `worktree.baseRef: "head"` branches from local HEAD. | worktrees | P-19 |
| E-8 | `--settings` "can set any key your user settings file can set" and sits above user, project and local settings. List keys merge across scopes. | [settings](https://code.claude.com/docs/en/settings) | P-06, P-19 |
| E-9 | Hook input carries `session_id`, `transcript_path`, `cwd`, `permission_mode` and `effort.level`. `Notification` matchers include `permission_prompt`, `idle_prompt` and `agent_needs_input`. `Stop` carries `last_assistant_message`. Hooks inherit the parent environment. | [hooks](https://code.claude.com/docs/en/hooks) | P-06 |
| E-10 | Status-line input includes `session_id`, `model`, `effort.level`, and, for Pro and Max subscribers, `rate_limits.five_hour` and `rate_limits.seven_day` `used_percentage`. | [statusline](https://code.claude.com/docs/en/statusline) | P-22 (r1 said Foreman can't read usage; it can) |
| E-11 | `claude agents --json` reports per-session `status` (`busy`, `waiting`, `idle`), `waitingFor` and `kind` (`interactive` or `background`). Research preview. The docs are ambiguous about whether plain interactive sessions are listed. | [agent-view](https://code.claude.com/docs/en/agent-view) | P-06 fallback (S-9) |
| E-12 | `claude --desktop --resume <session-id>` opens that session in Desktop. It takes an ID only. v2.1.285+. | cli-reference | P-23 |
| E-13 | `claude auth status` `authMethod` is one of `none`, `claude.ai`, `oauth_token`, `api_key`, `api_key_helper` or `third_party`. | cli-reference | P-07 |
| E-14 | `-p` sessions are left out of the picker but can be resumed by ID. `claude -p --resume <id> --output-format json "…"` is a documented way to query an existing session. | sessions | P-13 (packet re-fetch), P-14 (B and C after a headless A) |
| E-15 | Interactive `-w` needs workspace trust. `-p` skips the trust check. | worktrees | P-16, P-19 |

---

## 3. Proposals by theme

Each proposal uses the same shape: **Problem** · **Behavior** · **Why this is the best option** (alternatives considered) · **Scope / Size / Risk** · **Depends on**.

### Theme A · The template: making the blueprint executable

#### P-01 · Annotation layer, prose inference, and drift detector

**Problem.** The parser's input can't express a third of what Foreman must know. r1 measured it: 21 of 64 steps continue a session, cwd varies (`[app]-spec/`, the blueprint repo), RUN WITH holds sequences and alternatives, outputs and run inputs are implicit, manual steps have no prompt, and inspection items carry no verification kind (J4-1, J4-2, J4-3, J4-7, J4-9, J3-2, J3-5, J4-5, J6-1, J6-7, J10-1, J10-2). Master-spec open questions Q1 and Q6 ask whether this needs a new source format.

**Behavior.**
1. **Inference first.** The parser derives every field it can from the prose, with rules that are unit-tested:
   - `same session` / `same chat` gives `session=continue`; `fresh session`, `new session` and `brand-new conversation` give `session=fresh`.
   - `in \`[app]-spec/\`` gives `cwd=spec`; `in your \`claude-blueprint/\` repo` gives `cwd=blueprint`.
   - `X first, then Y`, `X → Y` and `Shift+Tab → Y` give `start` plus `switch` entries.
   - `/model x`, `/effort y` on a continuing step become that step's `start` values for re-arming (see P-05).
   - `— or`, `(or …)` and `if available, otherwise` produce `alt` entries.
   - `-> path`, `Save the result to path` and `Write path` produce `out` entries.
   - Bracket tokens are classified as in P-09.
2. **Annotations only where inference can't decide.** These are HTML comments, which the blueprint already uses (`<!-- -->` separators), so the rendered page and the PDF are unchanged. One `fm:step` comment goes directly under a RUN WITH blockquote. One `fm:verify` comment goes directly under an INSPECTION blockquote, not inside it, so no renderer ever sees a comment inside a list item. Block markers wrap conditional prompt parts (v1.x).

   ```markdown
   > **RUN WITH** · Claude Code CLI in `[app]-spec/` · Chair: **Fable 5.1** · …
   <!-- fm:step id=2.2 cwd=spec session=new
        out=council/r1-market.md,council/r1-users.md,council/r2-features.md,council/r2-growth.md,council/r2-feasibility.md,council/r3-synthesis.md,council/r4-redteam.md,council/r5-resolution.md -->

   > **INSPECTION: pass before Stage 6**
   > - A fresh clone, then `install` and `dev`, works with zero manual steps
   > - CI is green, and an intentional type error … each fail it
   > - Staging deploys automatically, and a rollback has actually been run once
   > - The ADR reality check has no CONTRADICTED left open
   <!-- fm:verify 1=auto 2=attest 3=attest 4=auto -->
   ```

   v1 grammar: `fm:step` with keys `id`, `kind`, `surface`, `cwd`, `session`, `chain`, `start`, `switch` (repeatable), `alt` (repeatable), `in` (repeatable), `out`, `requires`, `tag`, `if`. Also `fm:verify` (`auto` / `attest` / `time:<n>d[>attest|>auto]`, optional `cmd=`), and `fm:if <cond>` … `fm:end` (parsed in v1, applied in v1.x).
3. **Drift detector.** Whenever inference and an annotation both produce a value and the values disagree, F-002 reports it with the line number. This catches the real long-term failure: someone edits the prose in a retro ("same session" becomes "fresh session") but forgets the annotation. An unknown `fm:` key is a hard parse error.
4. **Annotation budget for v1.** Only Stages 0–5, the Stage Gate, F.2 and the Council setup section must be fully covered for the metric run. Every other stage parses on inference alone and shows its gaps in Template health (P-03).

**Why this is the best option.** I considered three alternatives:
- *Structured YAML/JSON as the source, with the Markdown generated from it* (spec Q1). Rejected. Billy and the retro (F.5) edit the blueprint as prose, the PDF is the human artifact, and a generator adds a build step plus a second place to make mistakes.
- *Pure inference.* Rejected. r1 found the outputs only "partly regex-parseable", and verification kinds can't be inferred safely at all.
- *A sidecar file keyed by step ID.* Rejected. It sits far from the text it describes, so it drifts silently.

Inline comments plus a drift detector give structure exactly where the prose can't carry it, and they fail loudly.
**Answers spec Q1:** the Markdown stays the source of truth. **Answers spec Q6:** declared outputs come from inference plus `out=` markers where needed (about a third of steps).

**Scope** v1 · **Size** L (parser + inference + grammar + drift detection + golden tests) · **Risk** Med: the blueprint has many idioms. Mitigated by P-03 and by keeping annotation the escape hatch. **Depends on** blueprint change B-1 (§5), applied by Billy in the blueprint repo. Foreman never edits the blueprint.

#### P-02 · Stable step IDs and the blueprint lock

**Problem.** "Pin the blueprint version" stores no template (J2-5). Overlay edits, state and proposals need anchors that survive edits to the blueprint (J9-8, J10-5).

**Behavior.**
- Every step, inspection item and packet format gets a stable ID: `2.2`, `gate.A`, `loop.1`, `s05.i3`, `council-setup`. IDs come from the heading number or an explicit `id=`. Inspection item IDs are positional, and the drift detector checks the item count.
- **Pinning** stores the blueprint commit SHA, the version string and the SHA-256 of the source in `.foreman/blueprint.lock.json`. It also stores a copy of the source Markdown at that commit in `.foreman/blueprint.lock.md`. The project always opens against its lock and is parsed by whichever parser version is current. A dirty blueprint working tree can't be pinned; the wizard says why.
- The Stage 4 packet's `BLUEPRINT VERSION` is checked against the lock on validation, so the two sources of truth can't diverge.

**Why this is the best option.** Storing the source text instead of parsed JSON means a better parser re-reads the same words. The project still opens offline and when the blueprint repo has moved. `git show <sha>:…` would make every project depend on the blueprint repo being present and unrewritten.

**Scope** v1 · **Size** M · **Risk** Low.

#### P-03 · Template health and the TradeLog acceptance harness

**Problem.** The riskiest assumption, that the blueprint parses reliably, is tested only in Foreman's own CI. Billy has no way to see that it holds for his blueprint today (J1-1). The success metric isn't tested at all before he uses the app.

**Behavior.**
1. **Template health** screen, also the second step of first run. For example: "Blueprint 2.1 · 19 stages · 64 run configs · 0 errors · Stages 0–5 fully resolved". For each step it shows what was inferred, what was annotated, unknown tokens, missing `out=` values and drift. Stage 0–5 rows must all be green before the wizard offers TradeLog.
2. **Acceptance harness** (Foreman's test suite). Fixture: the real blueprint, plus TradeLog `project.json` built from the blueprint's own example variables, plus synthetic signed packets for Stages 1–4. It resolves every step 1.1–5.5 plus the gate prompts and asserts four things: zero unresolved variables and packet markers; every run input declared; every CLI step has `cwd`, `session` and `start`; and argv snapshots match. The metric "zero manual prompt edits" becomes a red/green test.

**Why this is the best option.** It is cheap (it reuses P-01 and P-09) and turns the kill criterion's biggest unknown into evidence before Billy spends time on it. The alternative, finding gaps live during the TradeLog run, is what the kill criterion punishes.

**Scope** v1 · **Size** S · **Risk** Low.

### Theme B · Running steps

#### P-04 · Run config v2

**Problem.** A run config is a single model/effort/mode triple, launched from the project root (J4-2 blocker, J4-3, J4-9). F-021 substitution is global and blunt.

**Behavior.** `RunConfig = { surface, cwd, session, start{model, effort, mode}, switches[], alts[], agents[] }`.
- **cwd** ∈ `project | spec | blueprint | worktree`. `spec` is the project's configured spec folder (default `spec/`); see path aliases in P-09. The watcher scope includes the declared `out` paths under it.
- **switches** are ordered cues: `{when: after-plan-approval | before-prompt | mid-step, set: mode/model/effort}`. Foreman *launches* with `start` and *shows* the switches (P-08). Between steps of a chain, any change is applied by re-arming (P-05), so switches are only needed *within* one step (for example "Plan first, then Accept edits").
- **Model policy** (replaces F-021 as a concept, same file). Per project: `plan: max | pro`, which picks the default among alternatives (2.2 chair: Max gives Fable · high, Pro gives Opus · xhigh), plus substitution rules (`fable → opus@xhigh`). The spec line shows the original and the chosen value: `Fable 5.1 · high (Max)  [or Opus 5.5 · xhigh]`. A per-run chip overrides it, and the choice is logged. Substitutions and alternative choices go into the packet at signing (P-13).
- **Explain on hover.** Hovering any spec-line value says *why* it is set that way: template, alternative (and which rule chose it), substitution, or overlay edit.

**Why this is the best option.** Keeping launch flags (`start`) separate from in-step cues (`switches`) is the only model that matches the blueprint's prose and stays deterministic. The alternative, modeling the whole sequence as automated keystrokes, is fragile (see P-05).

**Scope** v1 · **Size** M · **Risk** Low.

#### P-05 · Session chains with re-arm continuation

**Problem.** Blocker J4-1. One step in three continues a conversation. The spec launches a fresh `claude "prompt"` every time, so pressing Run on 2.3 throws away the council's context. The same gap breaks Gate B/C after A (J6-3), loop steps 2–4 (J7-2) and crash recovery (J9-4).

**Behavior.**
- A **chain** is one Claude conversation: `{chainId = Claude session UUID, projectId, cwd, worktree?, steps[], status: open | closed | superseded}`.
- `session=new` or `fresh` starts a chain. Foreman generates the UUID and launches `claude --session-id <uuid> --name fm-tradelog-s02-2.2 --model … --effort … --permission-mode … --settings <run settings> "<prompt>"`.
- `session=continue` joins the chain of the previous step. `chain=<stepId>` names a different one, for example `gate.B` continues `gate.A`, which may have been a headless run (E-14).
- **Re-arm.** To run a continuing step, Foreman:
  1. Checks the chain's live process is **idle**, using the sensor (P-06). Without the sensor, Billy confirms "Claude is idle" once.
  2. Ends the process gracefully (S-8 decides between writing `/exit` + Enter and sending SIGTERM).
  3. Relaunches in the **same tab** with `claude --resume <uuid>` plus that step's full flags, `--settings`, and the resolved prompt.
  4. Writes a divider into the scrollback, for example `── 2.3 Feature specifications · Fable 5.1 · high · Accept edits ──`.

  If the chain's process has already exited (closed tab, crash, quit), re-arm is simply the relaunch.
- Every chain step therefore starts with **exactly** the template's settings. That includes 5.2 (`/model opus`, `/effort high`, Shift+Tab → Plan), 5.4 (`/model sonnet`, `/effort low`) and Gate B's Shift+Tab → Accept edits. Billy types nothing.
- **Re-running** a chain-start step creates a new chain. The old one is marked `superseded` and kept in the run log, and later steps bind to the new chain.
- `--name` is for display only. The UUID is authoritative (E-4).
- **Chat chains** use the same model without the launch: see P-11.

**Why this is the best option.**
- *Typing `/model x`, `/effort y` and Shift+Tab into the live TUI* (r1's option b) depends on the TUI's state, the mode-cycle order and idle detection anyway. It fails silently when a keystroke lands in the wrong place.
- *`claude --bg` plus attach* is a research preview, defaults to a worktree, and doesn't document `--permission-mode` or `--effort` for `--bg` (E-11, r1 J9-6).
- *The Agent SDK* would move Foreman toward programmatic API usage, against the spirit of the brief.

Re-arm uses only documented flags: model and permission-mode overrides on resume are documented (E-1, E-2), and so are re-passed `--settings` (E-5). The same mechanism serves chains, gates, the Crew Loop and crash recovery.
*Costs, stated honestly:* about 1–3 s to restart, and "allow for this session" permission grants reset. Background subagents must have finished, which the idle check enforces.

**Scope** v1 · **Size** M · **Risk** Med. **NEC:** S-3 (does `--effort` apply on resume? If not, Foreman types `/effort <x>` once right after the relaunch, when the sensor reports idle) and S-8 (graceful exit, and transcript integrity after it). **Depends on** P-04 and P-06 (degrades to a manual confirm without it).

#### P-06 · Session sensor: per-launch hooks via `--settings`

**Problem.** Foreman can't see what a session is doing. It can't tell Billy a session is blocked on a permission prompt or waiting for "go" (J4-4, J4-5, J8-1). It can't re-arm safely without knowing the session is idle (P-05). It can't catch a packet printed in the terminal (J4-8). r1 proposed installing a hook in Billy's user settings, gated by an environment variable, because the hooks page doesn't document per-session hooks.

**Behavior.**
- Every launch passes `--settings <app-data>/runs/<runId>/settings.json`. That file is generated by Foreman and contains only a `hooks` block for `SessionStart`, `Stop`, `StopFailure`, `Notification`, `PermissionRequest` and `SessionEnd`. Each hook entry runs the bundled helper `foreman-sensor` by absolute path. Billy's own `~/.claude/settings.json` is never touched.
- `foreman-sensor` reads the hook's stdin JSON and appends one line to `<app-data>/events/<runId>.jsonl`, using `FOREMAN_RUN_ID` from the inherited environment (E-9). It exits within 10 ms. It never prints anything, so it can't influence Claude's behavior.
- From those events Foreman derives each session's live state:
  - `working`, `idle` (after `Stop`), `blocked:permission`, `blocked:input`, `blocked:plan-approval`, `ended(code)`.
  - The live `permission_mode` and `effort.level`, which drive a **mode guard**: if the session is in Accept edits while the step expects Manual, the Now/Next strip turns `--attention`. Nothing is enforced, it is a visible warning.
  - `last_assistant_message` at `Stop`, which feeds packet capture (P-13) and the interactive gate import (P-14).
- **Fallbacks, in order:**
  1. A user-level hook installed by Doctor that does nothing unless `FOREMAN_RUN_ID` is set (r1's design), used only if per-launch hooks fail S-1.
  2. Polling `claude agents --json` for `status` and `waitingFor`, if S-9 shows plain interactive sessions are listed.
  3. No sensor: every feature that needs it falls back to Billy's confirmation, and Today shows only file-based items.

**Why this is the best option.** `--settings` is documented to accept any user-settings key and to merge lists across scopes (E-8). It is scoped to exactly one run, survives resume because Foreman re-passes it (E-5), and leaves no residue on Billy's machine. Parsing the PTY screen for state is brittle. The JSONL transcript format is documented as internal and unstable. Agent view is a research preview.

**Scope** v1, built so that everything still works without it · **Size** M · **Risk** Med. **NEC:** S-1′ (do hooks passed through `--settings` fire, and do they merge with Billy's own hooks rather than replace them?). Doctor also checks that `disableAllHooks` isn't set.

#### P-07 · Launch pipeline: preflight, environment, auth, preview

**Problem.** Failures surface late, inside a terminal tab (J9-1). An inherited `ANTHROPIC_API_KEY` can silently switch billing (J1-3). Missing input files make Claude improvise (J10-2). Read literally, SR-03 puts a confirmation modal in front of every run (J4-10). A Tauri app launched from Finder has a minimal PATH (J1-5).

**Behavior.** Every launch goes through one Rust function, `prepare_launch(step) -> Result<LaunchPlan, Vec<Blocker>>`, which runs these checks in under 50 ms using cached data:
1. **Version and capability.** The required version is the maximum of the model minimums and the flag minimums the argv uses (`--desktop` 2.1.285, `--permission-prompts` 2.1.259, `manual` 2.1.200, and so on). The `claude -v` result is cached and refreshed on failure. On a mismatch, Run is disabled and an inline banner offers **Run `claude update`**, which opens a terminal tab running the allowlisted binary.
2. **Auth.** The cached `claude auth status` result must show `authMethod` as `claude.ai` or `oauth_token`. Both are subscription-backed; `setup-token` requires a subscription (E-13). Anything else is a hard block, and Doctor explains it.
3. **Environment.** The child environment is built from an allowlist (`HOME`, `USER`, `LOGNAME`, `SHELL`, `LANG`, `LC_*`, `TERM=xterm-256color`, `COLORTERM`, `TMPDIR`, `SSH_AUTH_SOCK`, `FOREMAN_RUN_ID`) plus Billy's pass-through list from Settings (for example `GITHUB_TOKEN`). A hard denylist always wins: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX`, `CLAUDE_CODE_USE_FOUNDRY`. `PATH` is built from `/etc/paths`, `/etc/paths.d/*`, known toolchain directories and Settings overrides, all read as files with no shell (S-12). This is SR-10.
4. **Inputs.** Every repo path in the resolved prompt (`docs/…`, `spec/…`, `council/…`, the remapped packet path) must exist. A missing signed packet is a hard block. Any other missing path is a visible warning, for example "02-teardown.md not found: 2.1 was skipped."
5. **Run inputs.** Every required input must be filled (P-09).
6. **Preview, not a modal.** The step block always shows the resolved prompt and the exact argv (collapsed under the button, one click to expand), and that block counts as the SR-03 preview. ⌘↵ runs. A confirmation modal appears only for an overlay-edited prompt or config (SR-07), a mode different from the template's, the first launch in a new project, or `auto` and `dontAsk` modes in interactive sessions.

**Why this is the best option.** One pipeline gives one place to test, one place for the run record (P-17), and one guarantee that no spawn skips a check. Using an allowlist plus a denylist, rather than a denylist alone, means a new provider variable can't leak through, while Billy still controls what his sessions need.

**Scope** v1 · **Size** M · **Risk** Low. **NEC:** S-7 (minimum clean environment), S-12 (PATH reconstruction).

#### P-08 · Terminal ergonomics

**Problem.** In-step switches are easy to miss (J4-3). A 30-minute council run gives no progress signal (J4-5). The drawer is too small to work in (J4-6). Key ownership between the app and Claude Code is undefined (J4-11). Hung sessions have no recovery (J9-7).

**Behavior.**
- **Now/Next strip** above each tab, for example `Now: Plan · Fable 5.1 · high │ Next: after you approve the plan, choose Accept edits`. Values come from `start`/`switches` and, with the sensor, from live state; a mismatch shows in `--attention`.
- **Round tracker** for steps with several `out=` paths, for example `Round 1 · 2/2 · Round 2 · 1/3 · Round 3 · 0/1`. It is driven by the file watcher and is accurate without the sensor.
- **Focus mode** (⌘⇧T). The terminal fills the content area, with a slim HUD showing step, chain and Now/Next.
- **Open output.** Each written `out` file gets an **Open** action in the macOS default app.
- **Key routing.** When the terminal has focus, only ⌘-chords and ⌃` reach the app; everything else goes to the PTY. ⌘K opens the palette even from the terminal. ⌘⇧E returns focus to the workspace.
- **Tab states:** running · idle · blocked · exited(code) · errored · interrupted. Tab menu: Interrupt (Ctrl-C), Terminate (SIGTERM, then SIGKILL after 5 s), Copy session ID, Resume.

**Scope** v1 · **Size** M · **Risk** Low.

### Theme C · Resolution: zero manual prompt edits

#### P-09 · Resolver v2

**Problem.** F-007 knows seven variables and one marker shape. The prompts contain at least thirteen other token kinds (J3-2), plural markers ("[paste the Stage 2 and Stage 3 Handoff Packets]"), the path alias `[app]-spec/`, and hard-coded packet paths that break when a stage is skipped (J5-5). `[ID]` and `[M#]` can be bound from context (J7-6). `[BUDGET_TIME]` ships as a placeholder, `[your numbers]`, that passes for filled (J2-1).

**Behavior.** Every bracket token is classified once, at parse time, as one of five classes:

| Class | Examples | Resolution |
|---|---|---|
| Project variable | `[APP_NAME]`, `[QUALITY_BAR]` | From `project.json`. A value equal to a blueprint placeholder (`[your numbers]`) counts as unresolved. |
| Packet marker | `[paste the Stage 1 Handoff Packet]`, `[paste the Stage 2 and Stage 3 Handoff Packets]`, `[paste the stage's inspection list]` | Signed packet contents, joined in order. The inspection list is filtered to applicable items, and for headless runs to `auto` items. |
| Path alias | `[app]-spec/`, `docs/handoff/stage-NN.md`, `stage-[N-1].md`, `[N+1]` | `[app]-spec/` becomes the spec folder. Packet paths are remapped to the **latest signed packet on the applicable path**, and `[N+1]` to the **next applicable stage**, so 11.1 reads `stage-09.md` when Stage 10 doesn't apply. Each remap shows as a subtle note on the step. |
| Run input | `[X]`, `[changes]`, `[paste numbers]`, `[jurisdictions]`, `[list screens]`, `[describe the voice]`, `[what I want to change and why]`, `[NNN]` | An inline form on the step (label, type, single or multi-line). Values with `remember=project` (`[jurisdictions]`, `[list screens]`) are stored and prefilled. Context-bound inputs (`[ID]`, `[M#]` from the selected ticket; `[N]` from the stage) fill themselves. |
| Literal | Brackets in code samples, `[1m]`, Markdown links | Left alone. Golden tests pin the classification. |

Unresolved required tokens block Run (P-07) and render as clickable attention pills that focus the matching field. The `in=` annotation declares labels and types; inference proposes them, and the drift detector checks they agree.

**Why this is the best option.** Classifying at parse time makes resolution a pure function, easy to test and fast (≤ 50 ms, NFR). Remembered inputs follow Raycast's dynamic placeholders, which r1 cited. Remapping packet paths silently would hide important behavior, so each remap is shown.

**Scope** v1 · **Size** M · **Risk** Med: classification edge cases, covered by P-03.

#### P-10 · Step kinds and step states

**Problem.** Chat steps have no completion signal (J3-1). Steps without a prompt ("Council setup (once)", "Record your final choice at the bottom of r7-stack.md", "Create the repo with exactly those files", the 7-day test) can't be represented (J4-7). Adopting an existing project needs a status that doesn't exist yet (J2-8).

**Behavior.**
- **Kinds:**
  - `prompt`.
  - `followup`: an extra prompt inside a step, sent in the same session, such as 5.1's "Use the test-runner agent…" after plan approval, or 5.3's per-ADR "Amend ADR-NNN…". v1 sends it as **Copy and focus terminal**; Billy presses ⌘V and Enter. P-24 automates this later.
  - `action`: Foreman does it, with a fixed, audited vocabulary of file operations in Rust: `write-blocks` (write the template's fenced blocks to files, used by Council setup to create `spec/.claude/agents/*.md`), `copy-file` (the Stage 1 brief to `spec/01-app-brief.md`) and `mkdir`.
  - `manual`: a checklist item with an optional **Open file**. A change to the file after the step started satisfies it, as for "record your final choice".
  - `wait`: until a date, with a macOS reminder notification. Used for 1.3's 7-day test.
  - `repeatable`: a run counter, with cadence reminders in v1.x. Covers 17.2 and Stage 18.
- **Step states:** `not-started · copied (awaiting you) · running · done · skipped(reason) · n/a(reason) · waiting-until(date) · adopted`. For stages, `adopted` is in the enum from day one; its UI ships in v1.x.
- Chat steps get **Mark done** (⌘D). It is also implied when Billy pastes a packet or output that the step declares.

**Scope** v1 · **Size** M · **Risk** Low.

### Theme D · The chat bridge

#### P-11 · Chat bridge

**Problem.** Stages 1–4, most of the metric run, happen in claude.ai, where Foreman can't set anything:
- Billy must remember the model, web search, Research, and new chat versus same chat (J3-3).
- Outputs that aren't packets have nowhere to land (J3-5).
- The 1.4 paste mixes the packet and the press release (J3-6).
- `<blank>` fails validation (J3-7).
- An invalid packet is a dead end (J3-8).
- Chat stages have a gate but no runner (J3-9).
- The claude.ai Project is set up by hand (J2-4).

**Behavior.**
1. **Chat setup strip** in the step header, as checkable chips:
   - **`New conversation in Project TradeLog`** or **`Same chat as 1.1`** (bold, because it is the costly mistake).
   - `Opus 5.5`, `Web search ON`, `Research ON`.

   The chips are memory aids only, and the copy says so.
2. **Chat links.** The project stores its claude.ai Project URL, entered in the wizard (P-16). Each chat chain stores its conversation URL: after the first copy, the strip offers **Link this chat** (paste or **From clipboard**, on an explicit click only). "Copy and open" (⌘↵) then opens the right place: the Project for a new conversation, the stored conversation for a continuing one. The clipboard stays the contract; Foreman doesn't depend on undocumented URL prefill.
3. **Paste-back** (⌘⇧V on the step) accepts three kinds of content:
   - **Packet.** Normalized: `<blank>`, `<leave blank…>` and `<sign here…>` are read as unsigned; `None` and `none.` count as `none`, with a quiet note; smart quotes and non-breaking spaces are fixed. The text from the header line to the format's last field is the packet. Anything after it is saved as `docs/handoff/stage-NN-attachments.md`, which covers the 1.4 press release and FAQ. Declared extra destinations are written too (`spec/01-app-brief.md`). Every write is listed in a confirmation line.
   - **Output.** Saved to the step's declared `out` path. If the file exists, Foreman shows a diff and asks before overwriting.
   - **ADR set.** Split on `^# ADR-\d{3}:` into `docs/adr/ADR-NNN.md`, one file per ADR. 4.2 produces 10–15 of them in the metric run, which is why this is v1. A generic multi-file split is v1.x (B-6).
4. **Copy fix request** on an invalid packet. Foreman generates the message for the same chat, for example: "Your packet is missing: X, Y. OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block."
5. **Chat-stage gate.** All inspection items are `attest`: a checkbox plus a required one-line note. Next comes the F.2 lessons step, whose prompt has the lessons-entry format from `docs/lessons.md`'s header appended automatically, so "I'll paste the format" is resolved. Its paste-back appends entries to `docs/lessons.md` with the next free `L-NNN`. Then Sign.

**Why this is the best option.** Foreman can't drive claude.ai, so the best possible bridge makes each crossing one keystroke and makes each return lossless and validated. Browser automation of claude.ai was rejected: it is brittle, it means an outbound integration Foreman doesn't otherwise need, and it pulls against SR-08.

**Scope** v1 · **Size** L · **Risk** Low.

#### P-12 · Build-package assembler and Project-knowledge manifest

**Problem.** Stage 4 chats depend on Project knowledge that Foreman can't see (J3-4). The Stage 4 → 5 transition ("Create the repo with exactly those files") is a large manual job with no tracking. The late conditions (`has_database`, `has_uploads`, `has_email`) have no source (J2-3).

**Behavior.**
- **Knowledge manifest** per project: the files that should be in the claude.ai Project right now (signed packets and declared chat outputs), each with a checkbox, **Reveal in Finder** and **Copy contents**. Stage 4 chat steps show "Knowledge ready 4 of 6" next to Copy. Content hashes and "changed since upload" flags are v1.x.
- **Build-package assembler**, a `manual` step at the end of Stage 4. It parses `REPO FILES` from the 4.8 packet and shows each path as exists / missing / empty, with **Paste into** (clipboard to file, with an overwrite guard) and **Reveal**. The Stage 4 gate's attest item "the package is complete" is pre-checked only when every row exists. The same panel asks the **late conditions**, prefilled from what exists (`docs/migrations/` present gives `has_database = yes`).

**Why this is the best option.** It turns the largest invisible manual step in the metric run into a tracked checklist and ties the late conditions to evidence rather than to memory.

**Scope** v1 · **Size** M · **Risk** Low.

### Theme E · Packets and signing

#### P-13 · Packet engine, safe signing, provenance

**Problem.**
- The sign order contradicts the blueprint (J5-1).
- Commits can happen twice and sweep in other staged work (J5-2).
- The file can change between check and use (J5-3).
- A packet can be signed without a passed gate (J5-4).
- Skips break headers and file reads (J5-5).
- `SKIPPED` and substitutions are never written (J5-6).
- Branch protection blocks the signing commit (J5-7).
- Git failure paths aren't handled (J5-8).
- An invalid Claude Code packet is a dead end (J9-3).
- A packet printed in the terminal never reaches the watcher (J4-8).

**Behavior.**
1. **Validation** (§8 rules, plus):
   - Header M is checked against the **applicable** path, not N+1.
   - Normalization as in P-11.
   - `BLUEPRINT VERSION` is checked against the lock (P-02).
   - Each problem has a fix action: **Repair**, **Open in editor**, or **Copy fix request** for chat packets.
2. **Repair** for Claude Code packets re-arms the stage's chain (P-05) with a generated prompt: "Fix only these problems in docs/handoff/stage-NN.md: … Do not change anything else." When OPEN ITEMS isn't `none`, the view lists the items and states the only two allowed outcomes: resolve them, or move them to KNOWN LIMITS with a ticket ID.
3. **Packet capture** for steps that print the packet instead of writing it (2.6 in blueprint v2.1). Foreman offers **Capture from session**, which takes the packet from the sensor's `last_assistant_message` (P-06). If that isn't available, it uses `claude -p --resume <uuid> --output-format json "Output the handoff packet again, exactly, in one code block."` after the interactive process has exited (E-14), extracts the block, and runs normal validation. Paste-back (P-11) works here too. The real fix is blueprint change B-2.
4. **Order, enforced:** A → B → C → Sign. Sign stays disabled, with the reason stated, until:
   - the gate state is PASS (every applicable item is PASS or attested);
   - a lessons entry for the stage exists ("clean stage" counts);
   - the stage's session is idle or ended. If it is still running, Billy can confirm explicitly to go ahead.
5. **Signing dialog:**
   - Name and date.
   - **Branch:** the current branch name, plus **Sign on new branch `handoff/stage-NN`**. Foreman offers the new branch by default once Stage 5 has been signed, because 5.2 turns on branch protection.
   - **Fields Foreman will add**, shown as a diff: `SKIPPED:` from the overlay and the n/a steps, with reasons; `SUBSTITUTIONS:` from the model policy and alternative chips; `ATTESTED:` with each attest item's note. Foreman writes these deterministically, never Claude.
   - The **content hash**, for example "Signing stage-05.md @ a3f9c1".
6. **Two-phase sign:**
   - **Phase 1:** re-read the file and compare hashes (refuse and re-render on a mismatch); write APPROVED BY and the added fields atomically; write the provenance record `.foreman/stages/stage-NN.json`. The record holds the step states, the run IDs, session UUIDs, models, efforts and modes actually used, prompt SHA-256s, gate results and attestations, durations, and the packet hash after signing.
   - **Phase 2:** `git add -- <paths>` then `git commit -m "handoff: stage N signed" -- <paths>`, with paths limited to the packet, its attachments, the stage record and `project.json` if it changed. No `-a`, ever.
   - The state shows `signed (commit pending)` until git succeeds. On failure: git's stderr, **Retry commit**, **Open terminal here**. The stamp animation runs only after the commit.
7. **Attention ring** on the exact waiting sheet in the strip and on the Today row.

**Why this is the best option.** Pathspec commits and hash binding close the two silent-corruption paths at almost no cost. Foreman-written fields make the packet's record of skips and substitutions deterministic, not dependent on what an LLM chose to include. The provenance record is the free by-product of signing that later powers F.6 metrics, the retro and the dashboard. Signing on a branch respects branch protection without giving Foreman push or PR powers.

**Scope** v1 · **Size** M · **Risk** Med (git edge cases). **Depends on** blueprint change B-3 (Gate B no longer commits), so the signed commit is the only one.

### Theme F · Gates

#### P-14 · Honest gates

**Problem.**
- Blocker J6-1: a headless run can "PASS" items only a human can verify.
- Plan mode is the wrong headless mode (J6-2).
- A headless A leaves no session for B and C (J6-3).
- There is no progress or cancel (J6-4).
- A FAIL has no path forward (J6-5).
- Ten items of evidence take too long to review (J6-8).

**Behavior.**
1. **Verification kinds** from `fm:verify`:
   - `auto`: Claude may verify it with tool evidence.
   - `attest`: human only; checkbox plus a required note. No run can set it.
   - `time:<n>d`: earliest date, then `attest` or `auto`.

   **Untagged items default to `attest`**, so the safe behavior needs no tagging work. Item IDs are stable (P-02).
2. **Gate A, headless** (the default for `auto` items, if S-2 passes):

   ```text
   claude -p --session-id <uuid> --model opus --effort high
     --permission-mode dontAsk --permission-prompts none
     --allowedTools Read Grep Glob "Bash(<each declared verify command>)"
     --output-format stream-json --verbose
     --json-schema <inspection schema>
     --settings <run settings> "<Gate A, resolved with auto items only>"
   ```

   The schema status enum is `PASS | FAIL | CANNOT_VERIFY`, and items outside the prompt can't appear. The stream gives live progress (current item, tool calls), **Cancel** (SIGINT), a timeout, and a `failed` run state with **Open transcript** and **Run interactively instead**. Raw results go to app data and are summarized in the stage record at signing. Using `dontAsk` instead of the template's `plan` is recorded under SUBSTITUTIONS.
3. **Gate A, interactive** (the fallback, or Billy's choice). It runs in the terminal as the blueprint says. Afterwards, **Import results** reads the sensor's `last_assistant_message`, proposes PASS/FAIL per `auto` item by matching item text, and Billy confirms each one.
4. **Gates B and C** continue the gate chain with `chain=gate.A`. After a headless A, Foreman resumes the `-p` session by UUID (E-14) in an interactive tab, through re-arm.
5. **Review order:** FAIL first, then CANNOT_VERIFY, then attest items waiting on Billy; PASS collapsed. Artifacts are copyable.
6. **After a FAIL:**
   - **Fix this** opens a new CLI chain (BUG route: Opus 5.5 · high · Plan → Accept edits), prefilled with the item, evidence and artifacts.
   - **Re-run failed items** runs Gate A headless over only those items.
   - A third failure of the same item suggests the F.4 prompt (automated in v1.x).
7. **Gate state machine:** `not-ready → A-running → A-results → B → packet-valid → C → lessons-found → signable`. It is shown as a short stepper in the inspector.

**Why this is the best option.** Defaulting untagged items to `attest` makes "zero gates skipped by accident" hold from day one, without perfect tagging. Restricting headless runs to `auto` items with a `CANNOT_VERIFY` escape removes the incentive to invent evidence. `dontAsk` with an explicit allow-list is the locked-down unattended mode documented for headless runs (r1 J6-2).
**Answers spec Q5:** headless is the default for `auto` items; interactive is the fallback; `attest` is always human.

**Scope** v1 · **Size** L · **Risk** Med. **NEC:** S-2 (allow-listed Bash runs under `dontAsk` and everything else is denied, subagents included) and S-10 (`stream-json` combined with `--json-schema`).

### Theme G · Today: the daily surface

#### P-15 · Today inbox, attention model, Do next

**Problem.** Nothing defines where "waiting on you" comes from (J8-1). Billy's morning question is "what next, and in which project?" (J8-2). Running sessions vanish when he switches projects (J8-3). Switching is slow (J8-4). Only the stamp area shows attention (J5-10).

**Behavior.**
- **Attention model.** Each item is `{kind, projectId, sheet, title, since, primaryAction, key}`. Kinds, in priority order:
  1. `session-blocked`: permission prompt, input needed, or plan approval (sensor).
  2. `session-your-turn`: idle after Stop on a step expecting Billy's reply, for example the council waiting for "go" (sensor).
  3. `packet-landed` (valid, unsigned) and `packet-invalid`.
  4. `gate-failed` and `gate-signable`.
  5. `chat-awaiting-paste` (copied more than 10 minutes ago, no paste-back).
  6. `wait-elapsed`.
  7. `interrupted-session`.
  8. `step-ready`: one per project, the project's Do next.

  File-based kinds always work. Session kinds need the sensor.
- **Today is the home screen.** Header: "Today · 3 need you · 2 running". Items from all projects follow, then each project's **Do next** as one line with its spec line. J/K moves, Enter performs the primary action, ⌘↵ does the same from anywhere.
- **Global Do next (⌘⇧↵)** performs the top item's primary action from any screen: sign, run, copy and open chat, resume. From a cold start, opening the app puts focus on Today's top row, so starting the next step takes two keystrokes and well under 10 s.
- **Ambient signals:**
  - A Dock badge counting kinds 1–4.
  - macOS notifications for `session-blocked` and `packet-landed`, only when Foreman isn't focused. Clicking one jumps to the item.
  - Sidebar project rows with a state dot (running / blocked / idle) and a count.
  - ⌘1–⌘9 for pinned projects, ⌃Tab to cycle through recently used ones.
  - Terminal tab sets persist per project across switches, and the last selected stage and step are restored.

**Why this is the best option.** It combines Linear's inbox (one triage list, J/K) with GitHub Desktop's single context-sensitive primary button, which r1 cited. Putting the per-project Do next *inside* Today removes the "open project, find stage, find step" path, the slowest part of the core loop. A separate dashboard (F-016) adds nothing for the metric, so it moves to v1.x.

**Scope** v1 · **Size** M · **Risk** Low. Notifications are NEC on the Tauri plugin's macOS behavior.

### Theme H · First run and the wizard

#### P-16 · First run, Doctor, wizard

**Problem.** Setup is three disconnected screens (J1-1). Doctor misses flag minimums (J1-2), the API-key trap (J1-3), the Appendix A artifacts (J1-4), toolchain probing (J1-5), re-run policy (J1-6) and one-click updates (J1-7). The wizard has its own problems:
- It retypes the same variables for every project (J2-1).
- It forces premature condition answers (J2-2) and never asks the hidden ones (J2-3).
- It skips the council folder and the claude.ai Project (J2-4).
- Pinning is vague (J2-5).
- The wow moment falls flat (J2-6).
- It doesn't say what it will create (J2-7).

**Behavior.**
1. **First-run checklist**, which gates the wizard:
   1. Blueprint repo: auto-detect `~/claude-blueprint` and `~/Developer/claude-blueprint`, else **Choose…**.
   2. Template health (P-03): proof that it parses.
   3. DRI name, prefilled from `git config user.name`.
   4. Doctor runs automatically.
   5. **Start your first project**.

   Fatal Doctor failures block; warnings don't.
2. **Doctor:**
   - A **capability table** (feature → minimum version → version found → "blocks N steps", computed from the template).
   - An auth check that requires `authMethod` to be `claude.ai` or `oauth_token`.
   - Every Appendix A artifact (global agents, `~/.claude/settings.json` keys, `install.sh`).
   - That `disableAllHooks` isn't set (P-06).
   - Toolchain probes (`git`, `node`, `pnpm`, `cargo`) from fixed candidate paths, version flags only. SR-01 is amended to allow these probes.

   Each failure has **Copy fix command**, or **Run `claude update`**. Doctor runs at launch (async), caches results for 24 h, re-runs after any launch failure, and shows a footer status dot.
3. **Wizard** (four steps; the side-effect preview is part of step 4):
   1. **Name and folder.**
   2. **Variables.** Global defaults from Settings prefill `[MY_ADVANTAGES]`, `[PREFERRED_STACK]` and `[QUALITY_BAR]`. Long fields are multi-line. Placeholder values count as empty.
   3. **Conditions.** Each is yes / no / **not decided yet**, with one line on what it changes. Conditions stay freely editable, with an audit line, **until the Stage 2 packet is signed**. The Stage 2 sign dialog adds a **Confirm conditions** checkpoint; "not decided" blocks it. After that, a change needs a CR. The late conditions are asked at Stage 4 (P-12).
   4. **Review.**
      - The tailored path.
      - A **side-effect list**: will create / will leave alone / will refuse to overwrite. An existing `docs/lessons.md` is never overwritten.
      - **Setup actions**, each checkable: `git init` if needed; the docs skeleton; the `spec/` folder; a **Manual** row "Create the claude.ai Project named TradeLog", with a field for its URL.
      - The blueprint lock (P-02).

   **Create** lands on the project with focus on **First action: Stage 1.1 · Claude chat · Opus 5.5 · [Copy and open chat ⌘↵]**.

   Council agents are written by the Stage 2 `action` step `council-setup` (P-10), not the wizard. That way, conditions decided by then can drop the commercial roles.
4. **Workspace trust.** The first CLI launch in a new project shows Claude Code's trust dialog in the embedded terminal (E-15). Foreman's Now/Next strip says so, so it doesn't look like an error.

**Scope** v1 · **Size** L · **Risk** Low.

### Theme I · State and resilience

#### P-17 · State layout and resilience

**Problem.** r1 asked (§17 Q2) whether `state.json` is committed. Committing in-flight state on `main` collides with branch protection from Stage 5 on and with worktrees (J5-7, J7-3, J7-5). Session IDs mean nothing on another machine. A crash loses run state (J9-4). Quitting kills sessions silently (J9-5). Invalid or externally edited files have no defined behavior (J9-9).

**Behavior.**

| Where | File | Written when | Committed |
|---|---|---|---|
| Repo `.foreman/` | `project.json` (variables, conditions + audit, model policy, spec folder, verify commands, schemaVersion) | Wizard, settings changes, condition edits | Yes, by Foreman with a pathspec, at the moment of change (before Stage 5), or folded into the next signing commit (from Stage 5 on) |
| Repo `.foreman/` | `blueprint.lock.json`, `blueprint.lock.md` | Pin, upgrade (v1.x) | Yes |
| Repo `.foreman/` | `overlay.json` | Skip, add, edit | Yes, as above |
| Repo `.foreman/` | `stages/stage-NN.json` (provenance, immutable) | Sign | Yes, in the signing commit |
| App data `projects/<id>/` | `progress.json` (step states, chain registry, gate runs, run inputs, ticket loop state) | Continuously, atomic writes | No; machine-local |
| App data | `runs/<runId>.json` + `runs/<runId>.log` (last 2,000 lines of scrollback) + `events/<runId>.jsonl` | At launch, on change, at exit | No |

- **The principle:** the repo holds *decisions and signed truth*, written only at deliberate moments. App data holds *in-flight, machine-specific* state. Losing app data loses only in-flight marks, and most of those can be rebuilt from packets, outputs and git.
- **The run record is written at launch** with status `running`: argv, cwd, chain UUID, step, prompt SHA-256, start time. It is updated on exit. On the next launch, an **Interrupted sessions** list offers one-click **Resume**, which re-arms the chain (P-05). Interrupted sessions also appear in Today.
- **Quit guard:** "3 sessions are running. They end when Foreman quits; you can resume each one later." The same guard applies when closing a project.
- **Invalid or newer files:** if a repo JSON fails to parse, the project opens **read-only** with the exact error and **Open in editor**. Foreman never overwrites an invalid file or a file newer than its last read. Changes are reloaded with a "Reloaded from disk" toast. Entries are sorted by ID so diffs stay small.
- The watcher ignores `.git`, `node_modules`, `target` and `.claude/worktrees/**`.

**Why this is the best option.** It answers r1 §17 Q2 better than "committed on main only". Nothing Foreman writes between signings dirties the working tree that Claude commits from, so stray commits and branch-protection collisions can't happen. The signed record still travels with the repo.

**Scope** v1 · **Size** M · **Risk** Low.

#### P-18 · Metric panel: the kill criterion, live

**Problem.** "Starting any step in under 10 seconds" and "zero manual prompt edits" aren't measured anywhere, but the kill criterion depends on them (J8-7).

**Behavior.** The run log records:
- `ms from app focus or Today open to run start`;
- whether the run came from Do next, the palette or the stage view;
- any unresolved-input interventions;
- every **Copy prompt** on a CLI step, as a proxy for running a step by hand.

A small panel (Settings → Metrics) shows the median and p90 time-to-start, the manual-edit count per stage, and "Stages run through Foreman: 1, 2 of 5".

**Why this is the best option.** It costs one day and makes the kill decision after Stage 2 a reading, not a feeling.

**Scope** v1 · **Size** S · **Risk** Low.

### Theme J · The Crew Loop (designed now, ships v1.x)

#### P-19 · Crew Loop on CLI worktrees

**Problem.**
- Blocker J7-1: the loop runs on Desktop, where Foreman can't set anything.
- Steps 2–4 switch model, effort and mode (J7-2).
- Ticket status has no home (J7-3).
- Loop shapes differ by ticket class (J7-4).
- Worktrees are invisible to the watcher (J7-5).
- `[ID]` should bind from the ticket (J7-6).
- Dependencies aren't tracked (J7-7).
- There is no gate unlock rule (J6-6).

**Behavior.**
- **The CLI is the default loop surface**, recorded as a surface substitution in the packet.
  - Step 1 (Plan): `claude -w <ticket-slug> --session-id <uuid> --name fm-tradelog-m1-03 --model <class> --effort <class> --permission-mode plan --settings <run settings + {"worktree":{"baseRef":"head"}}> "<Plan prompt>"`. Using `baseRef: head` means the worktree includes locally signed packets that may not be pushed yet (E-7, E-8).
  - Steps 2–4: re-arm `claude --resume <uuid> …` from the main checkout, which returns the session to its worktree (E-6). Each step gets its own class-routed flags, so the HARD-class Manual mode is applied by flag, never by memory.
  - Step 3 optional: **Review in Desktop** runs `claude --desktop --resume <uuid>`, opening this exact session for "Walk through in diff" (E-12, P-23).
  - Step 5: Foreman watches git for the merge (the branch is merged into the default branch, or its remote branch is gone). It then offers **Close M1-03** and, if the worktree is clean, `git worktree remove`.
- **Loop shapes by class:**
  - SPEC-COMPLETE: one step, the `implementer` shortcut, on the main checkout.
  - MECHANICAL: multi-select produces one batch prompt listing the IDs.
  - HARD: Manual in steps 1–2, with the mode guard on.
  - NEEDS-DESIGN and BUG: Plan → Accept edits.
  - STUCK: as in the routing table.
- **Tickets.** `docs/tickets.md` is read-only to Foreman and parsed with line-numbered errors. Loop state lives in app data (P-17). Closures are folded into the stage record at signing. Rows show `Ready / Blocked by M1-02 / In loop (3 of 5 · Opus high) / Closed`. **Next ticket** picks the first ready ticket by milestone, then class. The gate unlocks when every ticket for that stage is closed ("4 of 11 Stage 6 tickets open" otherwise).
- **Preflight** warns if the main checkout isn't on the default branch.

**Why this is the best option.** It is the only design where "Run next loop step" is one keystroke with routing applied automatically, using documented flags only. Desktop stays available where it is genuinely better: the Browser pane for UI tickets, and diff review.

**Why v1.x.** Not one Stage 1–5 step uses tickets. But **blueprint change B-4** (a strict ticket format in 4.6) is **v1**, because TradeLog's tickets are written during Stage 4 of the metric run. If the format isn't strict by then, the v1.x parser will have nothing reliable to read.

**Scope** v1.x · **Size** L · **Risk** Med. **NEC:** S-5 (`-w` together with `--session-id` in a PTY; `baseRef` passed via `--settings`; whether `/code-review` exists in the CLI).

### Theme K · Amendments: Change Requests (designed now, ships v1.x)

#### P-20 · The CR track as drawing revisions

**Problem.**
- Blocker J11-1: there is no model for reactivating stages out of order.
- Billy needs to see a CR's impact before applying it (J11-2).
- There is no delta prompt (J11-3).
- CR documents have no lifecycle (J11-4).
- Implied conditions aren't offered (J11-5).

**Behavior.** This borrows the drawing-set vernacular from the design direction: a CR is a **revision**, and signed sheets are never redrawn; they get a revision cloud.
- `Amendment { id: CR-NNN, status: proposed | approved | applied | signed, conditionChanges, amends: [stage], deltaSteps[], reinspect: {stage: [itemId]}, reactivates: [stepId] }`, stored in `.foreman/amendments/CR-NNN.json`.
- **Flow:**
  1. 18.3 runs in chat.
  2. The CR packet is pasted back to `docs/change-requests/CR-NNN.md`, auto-numbered and validated like a packet.
  3. **Apply** stays disabled until the decision is APPROVE.
  4. The **impact dialog** shows conditions changed, steps reactivated, signed stages to be marked "Rev 1 · CR-NNN", gates gaining items, and the delta steps.
  5. An **amendment track** appears above the stage map: delta steps for each amended stage (same run config as the original step; prompt = blueprint 18.3b, B-7, with the CR packet and the original document path); then an amendment gate (new items only, plus attest); then sign `docs/handoff/cr-NNN.md`.
  6. Reactivated stages ahead of the current one become ready in normal order once the amendment is signed.
- Before Stage 2 is signed, a condition change needs no CR (P-16).

**Why v1.x.** A Stage 1–5 run never needs it: conditions are free until Stage 2's sign, and nothing is reactivated before Stage 6. **The data shape is v1:** `amendments/` is reserved, and the stage record has a `revisions[]` field, so v1.x needs no migration.

**Scope** v1.x · **Size** L · **Risk** Med.

### Theme L · Overlay authoring and the calibration loop (v1.x)

#### P-21 · Overlay authoring

Covers J10-3, J10-5, J10-6, J9-8 and J9-10.
- **Add step sheet**, which reuses the step editor: run config, tag, `out`, inputs, a live preview and lint, and "Duplicate from step".
- **Project edits** that store `baseTemplateVersion` and `baseHash`. A **Stale edit** banner offers Keep mine / Take new / Edit again.
- **Create proposal**, one flow for lessons and overlay entries. It verifies the old text exists word for word at blueprint HEAD and writes `proposals/YYYY-MM-DD-L-NNN.md` with the CHANGELOG draft inside it. Promotion status lives in Foreman state, never in `docs/lessons.md`.
- **Upgrade pin**: steps that changed, overlay edits that become stale, and duplicate overlay entries offered for removal.

v1 keeps only **Skip** with a reason (optional and conditional items, undo until signed) and the overlay data shape. The escape hatch in v1 is Copy prompt, which the metric panel (P-18) counts.
**Scope** v1.x · **Size** L · **Risk** Low.

### Theme M · Extras that make Foreman remarkable (v1.x)

#### P-22 · Usage meter and Fable guard

Covers J8-5 and J6-9. The per-run `--settings` file can also set a `statusLine` whose command is `foreman-sensor --statusline`. It records `rate_limits.five_hour` and `rate_limits.seven_day` `used_percentage` (E-10), and prints a compact line with model · effort · mode · weekly usage.
- Today's header shows "Week: 62% · 5 h: 18%".
- Launching a Fable or `max` step above 80% weekly usage asks first.
- Each stage record stores usage at signing, which gives the F.6 metric without a manual `/usage` note.

*Trade-off:* for Foreman-launched sessions this replaces Billy's own status line, because a key set in `--settings` overrides his file. Settings offers "Keep my status line" to turn the meter off.
**Scope** v1.x · **Size** S · **Risk** Low. **NEC:** S-1′.

#### P-23 · Desktop handoff

Covers F-009 and J7-9. Copy the prompt, then run `claude --desktop` in the step's cwd and show the model, effort and mode chips. **Open this session in Desktop** runs `claude --desktop --resume <uuid>` for any CLI chain (E-12). No Stage 1–5 step uses Desktop.
**Scope** v1.x · **Size** S · **Risk** Low.

#### P-24 · Live send-to-session and the "go" button

Covers J4-5 and the live alternative to J4-1. Bracketed paste of follow-ups into the live TUI, without auto-submit, and **Send "go"** when the sensor reports `session-your-turn`. Promote it to v1 only if S-4 passes early. Until then, v1 uses Copy and focus terminal, and Billy types "go".
**Scope** v1.x · **Size** S · **Risk** Med.

#### P-25 · Calibration and polish bundle

All v1.x:
- Lessons list with filters and duplicate `L-NNN` warnings (J10-7).
- Applying `fm:if` blocks inside prompts and inspection lists (J10-1, J6-7).
- **Start retro** at Certificate of Occupancy, with the F.6 table generated from stage records (J10-8).
- **Run sheet export**: a stage's resolved prompts and argv as one Markdown file, as a by-hand fallback that protects the kill criterion.
- Revoke signature (J5-9).
- The EVIDENCE completeness rule (J5-11).
- Condition consistency hints (J2-9).
- The `--ax-screen-reader` toggle (J4-12).
- Activity feed (J8-6).
- Adopt-existing-project UI (J2-8).
- Dashboard (F-016), built from stage records.
- Run-log search UI (F-019; the data is v1).

---

## 4. r1 coverage matrix (every blocker and major)

| r1 ID | Sev | Proposal | Scope |
|---|---|---|---|
| J1-1 | major | P-16, P-03 | v1 |
| J1-2 | major | P-07, P-16 | v1 |
| J1-3 | major | P-07, P-16 | v1 |
| J1-4 | major | P-16 | v1 |
| J2-1 | major | P-16, P-09 | v1 |
| J2-2 | major | P-16 | v1 |
| J2-3 | major | P-12 | v1 |
| J2-4 | major | P-16, P-11 | v1 |
| J2-5 | major | P-02 | v1 |
| J2-8 | major | P-10 (enum), P-25 (UI) | v1 / v1.x |
| J3-1 | major | P-10 | v1 |
| J3-2 | major | P-09, P-01 | v1 |
| J3-3 | major | P-11 | v1 |
| J3-4 | major | P-12 | v1 (hashes v1.x) |
| J3-5 | major | P-11, P-12 | v1 (generic split v1.x) |
| J3-6 | major | P-11 | v1 |
| J3-7 | major | P-11, P-13 | v1 |
| J3-8 | major | P-11 | v1 |
| J3-9 | major | P-11 | v1 |
| **J4-1** | **blocker** | **P-05** (+ P-24 later) | v1 |
| **J4-2** | **blocker** | **P-04**, P-01 | v1 |
| J4-3 | major | P-04, P-08 | v1 |
| J4-4 | major | P-06 | v1 (NEC) |
| J4-5 | major | P-08 (tracker), P-24 ("go") | v1 / v1.x |
| J4-6 | major | P-08 | v1 (preview v1.x) |
| J4-7 | major | P-10 | v1 |
| J4-8 | major | P-13 (capture), B-2 | v1 |
| J4-9 | major | P-04 | v1 |
| J5-1 | major | P-13 | v1 |
| J5-2 | major | P-13, B-3 | v1 |
| J5-3 | major | P-13 | v1 |
| J5-4 | major | P-13 | v1 |
| J5-5 | major | P-09 | v1 |
| J5-6 | major | P-13 | v1 |
| J5-7 | major | P-13 | v1 (both options) |
| J5-8 | major | P-13 | v1 |
| **J6-1** | **blocker** | **P-14** | v1 |
| J6-2 | major | P-14 | v1 (NEC S-2) |
| J6-3 | major | P-14, P-05 | v1 |
| J6-4 | major | P-14 | v1 |
| J6-5 | major | P-14 | v1 (auto F.4 v1.x) |
| J6-6 | major | P-19 | v1.x |
| **J7-1** | **blocker** | **P-19** | v1.x (argued in §8) |
| J7-2 | major | P-19, P-05 | v1.x |
| J7-3 | major | P-17 (decision), B-4 | v1 (decision + format) |
| J7-4 | major | P-19 | v1.x |
| J7-5 | major | P-17 (watcher ignore), P-19 | v1 / v1.x |
| J7-6 | major | P-09 | v1 (binding mechanism) |
| J8-1 | major | P-15, P-06 | v1 |
| J8-2 | major | P-15 | v1 |
| J8-3 | major | P-15 | v1 |
| J8-5 | major | P-22 | v1.x |
| J9-1 | major | P-07 | v1 |
| J9-3 | major | P-13 | v1 |
| J9-4 | major | P-17, P-05 | v1 |
| J9-5 | major | P-17 | v1 |
| J9-8 | major | P-02 (base), P-21 | v1.x |
| J9-9 | major | P-17 | v1 |
| J9-10 | major | P-21 | v1.x |
| J10-1 | major | P-25, B-5 | v1.x (+ 1.4 tag fix v1) |
| J10-2 | major | P-07 | v1 |
| J10-3 | major | P-21 | v1.x |
| J10-5 | major | P-21 | v1.x |
| **J11-1** | **blocker** | **P-20** | v1.x (shape v1, argued in §8) |
| J11-2 | major | P-20 | v1.x |
| J11-3 | major | P-20, B-7 | v1.x |

Minor items are covered in the proposals that touch them: J1-5/6/7 → P-07/P-16; J2-6/7 → P-16; J2-9 → P-25; J3-10 → P-13/P-17; J4-10 → P-07; J4-11 → P-08; J4-12 → P-25; J5-9/11 → P-25; J5-10 → P-13/P-15; J6-7 → P-25; J6-8 → P-14; J6-9 → P-22; J7-7/8/9 → P-19/P-15/P-23; J8-4 → P-15; J8-6 → P-25; J8-7 → P-18; J9-2 → §6 S-13; J9-6 → P-06 fallback / v1.x; J9-7 → P-08; J10-4 → P-21 (skip undo is v1); J10-6/7/8 → P-21/P-25; J11-4/5 → P-20.

---

## 5. Blueprint changes to request (blueprint v2.2, before the TradeLog run)

Foreman never edits the blueprint. Billy applies these in the blueprint repo, in one Claude Code session, before TradeLog starts. Every item marked v1 is a precondition of the metric run.

| ID | Change | Why | Needed |
|---|---|---|---|
| B-1 | Add `fm:` annotations (P-01 grammar) to Stages 0–5, the Stage Gate, F.2 and Council setup; the remaining stages follow | Structure that inference can't supply | v1 |
| B-2 | Every Claude Code step writes its packet to `docs/handoff/stage-NN.md` (2.6 currently says "output … in one code block") | Packets land in the watcher reliably (J4-8) | v1 |
| B-3 | Gate B writes the packet and **does not commit**; the signing commit is the only one | Avoids a double commit and sweeping in other changes (J5-2) | v1 |
| B-4 | 4.6 emits one `### [M#-NN] Title` heading per ticket with labeled fields, one per line | A parseable ticket source for v1.x (J7-3) | v1 |
| B-5 | Tag 1.4 as core (packet), and mark its press release and FAQ as an optional part | No skip can drop a core packet (J10-1) | v1 |
| B-6 | Use the same `APPROVED BY: <blank>` in every packet format. Multi-file chat replies start each file with `FILE: <path>` | Normalization stays simple; generic file split (J3-5) | v1 (first) / v1.x (second) |
| B-7 | Add **18.3b CR delta** prompt (input: CR packet + original document; output: amended document) | Amendment track (J11-3) | v1.x |
| B-8 | State that `[app]-spec/` lives inside the project repo as `spec/` (Foreman's own build already does this) | Single cwd model (J4-2) | v1 |

---

## 6. Spikes (updated from r1 §13)

| ID | Question | Pass condition | Gates | When |
|---|---|---|---|---|
| S-1′ | Do hooks passed through `--settings <file>` fire in a PTY-launched interactive session, and do they merge with the user's own hooks? | `Stop`, `Notification` and `SessionStart` events arrive with `permission_mode` and `effort`; the user's existing hooks still run | P-06, P-22 | Before freeze (Stage 2/4) |
| S-2 | `claude -p --permission-mode dontAsk --permission-prompts none --allowedTools "Bash(<cmd> *)" --json-schema …` runs the listed commands, denies the rest, and the same holds for subagents | Items verified; a command not on the list is denied and reported | P-14 | Before freeze |
| S-3 | Does `--effort` apply to a resumed session? (Model and mode overrides are now documented, E-1 and E-2.) | The status line's `effort.level` matches the flag | P-05 | Before freeze |
| S-8 | Graceful end for re-arm: `/exit` written to the PTY vs SIGTERM when idle. Is the transcript complete, and does the resumed session continue cleanly? | Resume shows the full history; no "interrupted tool" note when idle | P-05 | Before freeze |
| S-9 | Does `claude agents --json` list plain interactive sessions with `status` and `waitingFor`? | Yes, for PTY-launched sessions | P-06 fallback | Stage 6 |
| S-10 | `--output-format stream-json --verbose` together with `--json-schema`: is `structured_output` present in the result event? | Progress events and the schema result both arrive | P-14 | Stage 6 |
| S-11 | `claude -p --resume <uuid> --output-format json "re-output the packet"` after an interactive session ended | The packet is returned word for word | P-13 | Stage 6 |
| S-12 | PATH rebuilt from `/etc/paths`, `/etc/paths.d` and candidate directories gives Claude sessions a working `pnpm`, `cargo` and `node` | Commands found in a Finder-launched app | P-07 | Stage 6 |
| S-7 | Minimum clean environment; does an inherited `ANTHROPIC_API_KEY` change billing? (r1) | Behavior documented | P-07 | Before freeze |
| S-5 | `claude -w <name> --session-id <uuid>` in a PTY; `worktree.baseRef` via `--settings`; does `/code-review` exist in the CLI? (r1, extended) | Worktree created with the chosen ID and base | P-19 | v1.x |
| S-4 | Bracketed paste of 3–8 KB into the live TUI without auto-submit (r1) | Prompt arrives intact | P-24 | v1.x |
| S-6 | `claude --bg` substrate (r1) | as r1 | v1.x substrate | v1.x |
| S-13 | Flag probe: CI test that compares Foreman's argv flags with `claude --help` and the CLI reference (r1 J9-2) | Fails in CI, not for Billy | P-07 | v1.x |

The four spikes marked "Before freeze" are each under an hour with a real `claude` binary. If S-1′ fails, P-06 uses r1's user-level hook gated by an environment variable. If S-3 fails, re-arm types `/effort` once after relaunch. If S-8 fails, re-arm asks Billy to type `/exit`. If S-2 fails, Gate A defaults to the interactive mode with Import results. **No spike result removes a v1 proposal; each only changes its mechanism.**

---

## 7. Answers to the open questions this round can settle

| Question | Recommendation |
|---|---|
| Spec Q1: Markdown or structured source? | Markdown stays the source, with `fm:` comment annotations and a drift detector (P-01). |
| Spec Q5: headless or interactive gate by default? | Headless for `auto` items; interactive with Import results as the fallback; `attest` always human (P-14). |
| Spec Q6: is the declared output file parseable? | Inferred for about two-thirds of steps; `out=` annotations for the rest (P-01). |
| r1 Q1: annotations acceptable? | Yes (P-01, B-1). |
| r1 Q2: where does `state.json` live? | Split it. Decisions and signed records go in the repo, committed only at deliberate moments. In-flight state goes in app data (P-17). |
| r1 Q3: CLI-by-default Crew Loop? | Yes, as a recorded surface substitution, with Desktop one click away for the same session (P-19, P-23). |
| r1 Q4: hook telemetry? | Per-launch hooks via `--settings` (no global install), user-level hook as fallback, everything degrades to confirmation (P-06). |
| r1 Q5: conditions editable until the Stage 2 freeze? | Yes, with an audit line and a Confirm conditions checkpoint at the Stage 2 sign (P-16). |

---

## 8. The v1 cut line

### 8.1 What the metric run actually touches

| Stage | What TradeLog needs (all conditions yes) | Proposals exercised |
|---|---|---|
| 1 | Chat chain 1.1–1.4; run inputs `[X]`, `[changes]`, `[paste numbers]`; 7-day wait; packet + press-release attachment; copy to `spec/`; attest gate; F.2 lessons | P-09, P-10, P-11, P-13 |
| 2 | 2.1 chat (Research) → `spec/02-teardown.md`; council setup action; 2.2 CLI in `spec/` with chair alternatives and Plan → Accept edits; 2.3–2.6 re-armed chain; manual "record your choice"; packet capture; Confirm conditions at sign | P-04, P-05, P-06, P-07, P-08, P-10, P-13, P-16 |
| 3 | Chat chain 3.1–3.2; `[jurisdictions]` remembered | P-09, P-11 |
| 4 | Eight new conversations; plural packet markers; Project knowledge; ADR split; `[list screens]`; independent review; build-package assembler; late conditions | P-09, P-11, P-12 |
| 5 | CLI chain 5.1–5.5 with `/model` and `/effort` changes applied by re-arm; follow-ups; Gate A (headless or interactive), B, C; sign on a branch after branch protection | P-05, P-13, P-14 |
| All | Today + Do next; first run; resilience; metric panel; template proof | P-03, P-15, P-16, P-17, P-18 |

### 8.2 v1 (ships before the TradeLog run)

P-01, P-02, P-03, P-04, P-05, P-06, P-07, P-08, P-09, P-10, P-11, P-12, P-13, P-14, P-15, P-16, P-17, P-18. Also the blueprint changes B-1 to B-6 (B-6 first part only) and B-8, and the v1 data shapes reserved for v1.x: `adopted` status, `amendments/`, `revisions[]`, and ticket loop state in `progress.json`.

Rough total: 4 L (P-01, P-11, P-14, P-16) + 12 M + 2 S (P-03, P-18), about 16–24 weeks of solo work with Claude Code. That is large. If the chair needs it smaller, the first cuts are P-12's assembler (the manifest stays), P-08's focus mode and P-14's Import results. None of them is load-bearing for the metric.

**Suggested build order**, so that the kill criterion is tested as early as possible:
1. **Template core:** P-01, P-02, P-03, P-09. This is the riskiest assumption, so it goes first; it maps onto Foreman's own Stage 6.
2. **Chat path:** P-16, P-10, P-11, P-13 (signing). After this, **TradeLog Stage 1 runs through Foreman**.
3. **CLI path:** P-04, P-05, P-06, P-07, P-08, P-15, P-17, P-18. After this, **TradeLog Stage 2 runs**, and the **kill criterion is checked** with the metric panel.
4. **Finish:** P-12, P-14. After this, **Stages 3–5 run**, and the success metric is evaluated.

### 8.3 ─────────── v1 cut line ───────────

### 8.4 v1.x (in priority order)

1. **P-19 Crew Loop on CLI worktrees.** First, because TradeLog's next stage after 5 needs it.
2. **P-23 Desktop handoff.** Pairs with the loop for diff review and UI tickets.
3. **P-24 Live send and "go".**
4. **P-22 Usage meter.**
5. **P-20 Amendments (CR track).**
6. **P-21 Overlay authoring and proposals.**
7. **P-25 Calibration and polish bundle**, including the adopt-project UI, dashboard and run-log search.

### 8.5 Brief items moved to v1.x, and why

The brief's DOES list includes the Crew Loop, Add, Lessons promotion, Change Requests, the Dashboard and Desktop handoff. None of these is exercised by a Stage 1–5 run, so under the stated rule each is v1.x.
- Their **data shapes are fixed in v1**, so nothing needs migrating later.
- **Two blockers (J7-1, J11-1) are therefore deferred on purpose.** Both are fully designed (P-19, P-20) and depend only on mechanisms v1 already builds: chains, re-arm, the sensor, stage records.
- **The one piece of the Crew Loop that must land in v1 is the ticket format (B-4).** TradeLog writes its tickets during the metric run.

If the chair prefers to honor the brief's DOES list literally, the cheapest addition is P-23 (S). The next is a minimal P-19 without class shapes or batching (M). Neither changes the build order above.

---

## 9. Risks this round introduces, and their mitigations

| Risk | Mitigation |
|---|---|
| Re-arm depends on idle detection; ending a busy session loses a turn | The sensor gates re-arm. Without it, Billy confirms explicitly. S-8 checks transcript integrity. |
| Per-launch hooks might not fire through `--settings` | S-1′ before freeze; user-level hook gated by an environment variable as the fallback; every sensor-backed feature degrades to confirmation. |
| Annotations drift from the prose | The drift detector makes it a parse error with a line number. Golden tests run on every blueprint change. |
| A status line set through `--settings` replaces Billy's own (P-22) | It is v1.x and can be turned off. |
| Headless gate evidence is wrong for `auto` items | Billy still reviews and signs. `CANNOT_VERIFY` is always allowed. Re-run of failed items. |
| Signing on a branch confuses the Stage N+1 start | The Stage N+1 preflight reads the signed packet from the stage record and warns if the signing branch isn't merged. |
