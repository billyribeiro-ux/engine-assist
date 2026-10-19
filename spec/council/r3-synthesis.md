# Round 3 · Chair · Synthesis and decisions

Chair: Opus 5.5 (xhigh) · Date: 2026-10-19
Inputs: `r1-engineering.md` (principal engineer), `r1-workflow.md` (user advocate), `r2-proposals.md` (product visionary), the App Brief (signed Stage 1 packet), Master Spec draft v0.9, design direction v0.9, blueprint v2.1.

This round decides every proposal and finding. Round 4 (red team) attacks these decisions; Round 5 resolves the objections; then the Master Spec is frozen.

---

## 1. Decisions in one table

| Item | Decision | Notes |
|---|---|---|
| P-01 annotation layer + inference + drift detector | **Accept** | Grammar unified with r1-engineering's `foreman:` proposal into one `fm:` grammar (§3.1). |
| P-02 stable IDs + blueprint lock | **Accept** | |
| P-03 template health + TradeLog acceptance harness | **Accept** | The harness is Foreman's most important test. |
| P-04 run config v2 | **Accept** | |
| P-05 session chains with re-arm | **Accept** | `--session-id` on every launch (r1-eng E5). Re-arm cost (permission grants reset) accepted. |
| P-06 session sensor via per-launch `--settings` hooks | **Accept, degrade-safe** | Fallback order as proposed. Spike S-1′ before Stage 6 build. |
| P-07 launch pipeline | **Accept** | Adds r1-eng E11 (never `--bare`) and the Doctor floor (§3.4). |
| P-08 terminal ergonomics | **Accept** | Focus mode stays v1 (cheap, used daily). |
| P-09 resolver v2 | **Accept** | |
| P-10 step kinds and states | **Accept** | |
| P-11 chat bridge | **Accept** | |
| P-12 build-package assembler + knowledge manifest | **Accept** | Manifest is v1; the assembler is the first thing cut if the schedule slips. |
| P-13 packet engine + safe signing + provenance | **Accept** | |
| P-14 honest gates | **Accept** | Schema enum `PASS | FAIL | CANNOT_VERIFY` (attest items never sent, so r1-eng's `NEEDS_HUMAN` isn't needed). |
| P-15 Today inbox + attention + Do next | **Accept** | Replaces F-016 Dashboard as the home screen. |
| P-16 first run, Doctor, wizard | **Accept** | |
| P-17 state layout + resilience | **Accept** | Better than "committed on main only": decisions and signed records in the repo, in-flight state in app data. |
| P-18 metric panel | **Accept** | |
| P-19 Crew Loop on CLI worktrees | **Accept the design; ships in v1.1** | Not exercised by a Stage 1–5 run. v1.1 is the first release after Certificate of Occupancy, scheduled, not "someday". Data shapes reserved in v1. Ticket format (B-4) is v1. |
| P-20 CR track as drawing revisions | **Accept the design; ships in v1.1** | Data shapes reserved in v1. Conditions are freely editable until the Stage 2 packet is signed. |
| P-21 overlay authoring | **Modify: minimal Add is v1** | The brief explicitly asks for an Add button in every stage. v1 ships **Add prompt / Add inspection item, this project only**, using a run config copied from an existing step. Promote-to-blueprint, stale-edit merge and upgrade-pin are v1.1. |
| P-22 usage meter (status line rate limits) | **v1.1** | |
| P-23 Desktop handoff | **Modify: minimal version is v1** | Size S, and the brief lists it. v1: copy prompt + `claude --desktop` in the step's cwd + chips for the model, effort, and mode to pick + "Open this session in Desktop" (`--desktop --resume <uuid>`). |
| P-24 live send and "go" | **v1.1** (v1 stretch only if S-4 passes early) | |
| P-25 calibration and polish bundle | **v1.1** | Run-log data is v1; its search UI is v1.1. |
| B-1 … B-8 blueprint changes | **Accept as a blueprint v2.2 proposal** | Foreman never edits the blueprint. A ready-to-apply proposal is written to `spec/blueprint-v2.2/` for Billy (§4). Foreman v1 targets blueprint v2.2. |
| Spikes S-1′, S-2, S-3, S-7, S-8 | **Run as the first Stage 6 tickets**, before the modules that depend on them | They need a real `claude` binary on Billy's Mac; each has a documented fallback, so the spec can freeze now. |

## 2. Engineering findings adopted (from r1-engineering)

| ID | Finding | Adopted rule |
|---|---|---|
| E1 | SvelteKit 3 doesn't support `svelte.config.js` | All SvelteKit config lives in `vite.config.ts` via `sveltekit({ adapter })`; `#lib` replaces `$lib` (verified in the official migration guide). |
| E2 | TypeScript 7 breaks the Svelte toolchain | Pin `typescript@6.0.3`. Re-evaluate when TS 7.1 ships a stable programmatic API. |
| E3 | `plan` mode in `-p` lets the auto-mode classifier approve commands outside the allowlist | Headless Gate A uses `--permission-mode dontAsk`. |
| E4 | Version table incomplete | Doctor floor 2.1.285; 2.1.293 when any step uses Haiku 5.5. Capability table per flag. |
| E5 | `--name` isn't unique | Every launch passes `--session-id <uuid>` plus `--name` for display. |
| E6 | SPA `index.html` has one inline bootstrap script | CSP: `script-src 'self'` with Tauri's compile-time hashes; never `'unsafe-inline'`. |
| E7 | `tauri-driver` doesn't support macOS | Layered test strategy (Rust unit/property → Tauri `mock_builder` → fake `claude` binary → Vitest + `mockIPC` → Playwright + `mockIPC` → a thin `@wdio/tauri-service` smoke suite behind an `e2e` cargo feature). |
| E8 | Desktop takes no flags | Crew Loop runs in the CLI (v1.1); Desktop steps get the minimal handoff (P-23). |
| E9 | Verify commands from CLAUDE.md are fragile | `.foreman/project.json` is the only allowlist source. |
| E10 | Tags and conditions aren't in the blueprint text | Carried by `fm:` annotations (B-1). |
| E11 | `--bare` skips subscription login | Foreman never passes `--bare`. |
| — | PTY | `portable-pty` 0.9.0 directly; no `tauri-plugin-pty` (it lets the webview choose the program). |
| — | Plugins | No `tauri-plugin-shell`, no `tauri-plugin-fs`. `tauri-plugin-opener` for opening URLs and files in the default app. Clipboard and notifications via their official plugins. |
| — | Window look | Tauri's built-in `windowEffects` (sidebar material, Liquid Glass with a Visual Effect fallback) and `titleBarStyle`; no `window-vibrancy` crate. |
| — | Hardening | `freezePrototype: true`; app commands restricted with `AppManifest::commands`; explicit `csp` and separate `devCsp`. |
| — | Markdown | comrak, pinned exactly (`=0.56.0`), for line and column on every node. |
| — | Child environment | `TERM=xterm-256color`, `COLORTERM=truecolor`; PATH built explicitly (Finder-launched apps don't inherit the login shell's PATH); `ANTHROPIC_API_KEY` and provider variables always stripped. |

## 3. Unified designs

### 3.1 One annotation grammar

r1-engineering proposed `<!-- foreman: id= tag= when= outputs= -->` under each prompt heading; r2 proposed `<!-- fm:step … -->` under the RUN WITH and `<!-- fm:verify … -->` under each INSPECTION. Decision: **one prefix, `fm:`, two directives**, both block-level HTML comments (block level is verified to parse with line numbers; inline-in-list-item isn't, so it isn't used):

```text
<!-- fm:step id=<id> [kind=prompt|followup|action|manual|wait|repeatable]
     [tag=core|conditional|optional] [when=<expr>] [cwd=project|spec|blueprint|worktree]
     [session=new|continue|fresh] [chain=<stepId>] [surface=chat|cli|desktop]
     [start=<model>@<effort>/<mode>] [switch=<when>:<set>]* [alt=<rule>:<model>@<effort>]*
     [in=<name>:<type>[:remember]]* [out=<path,…>] [appends=<path>] [requires=<path,…>] -->
<!-- fm:verify <n>=auto[:cmd=<verify-id>]|attest|time:<days>d[>attest|>auto] … -->
```

- `when` is an expression over the condition IDs (`!`, `&`, `|`, parentheses).
- Inference fills every field it can; annotations are required only where inference can't decide. The drift detector makes any disagreement a parse error with a line number.
- Untagged inspection items default to `attest`.
- Unknown keys are hard errors.

### 3.2 Conditions

Eight early conditions (wizard) plus four late conditions (`has_database`, `has_uploads`, `has_email`, `has_integrations`, asked at the Stage 4 sign, prefilled from the Build Package). Tri-state: yes / no / not decided yet. Freely editable with an audit line until the Stage 2 packet is signed; a "not decided" condition blocks the Stage 2 sign. After that, changes need a CR (v1.1 track; v1 shows "requires a Change Request" and the CR template).

### 3.3 State layout (P-17)

Repo `.foreman/`: `project.json`, `overlay.json`, `blueprint.lock.json` + `blueprint.lock.md`, `stages/stage-NN.json` (immutable provenance), `amendments/` (reserved). App data: `projects/<id>/progress.json`, `runs/`, `events/`. Repo files are committed only at deliberate moments, always with pathspecs.

### 3.4 Doctor floor and capability table

| Capability | Minimum Claude Code |
|---|---|
| Fable 5.1 alias | 2.1.257 |
| `--permission-prompts none` | 2.1.259 |
| Opus 5.5 | 2.1.280 |
| Sonnet 5.5 | 2.1.284 |
| `--desktop` | 2.1.285 |
| Haiku 5.5 | 2.1.293 |

Hard floor: 2.1.285. The effective floor is the maximum over the capabilities the pinned template uses. Auth must be `claude.ai` or `oauth_token`.

## 4. Blueprint v2.2 proposal

Foreman v1 needs B-1 (annotations, Stages 0–5, Stage Gate, F.2, Council setup), B-2 (every CLI packet written to a file), B-3 (Gate B doesn't commit), B-4 (strict ticket format), B-5 (1.4 tagged core with an optional part), B-6 first part (uniform `APPROVED BY: <blank>`), and B-8 (`spec/` inside the project repo). B-7 (CR delta prompt) is v1.1.

These go into `spec/blueprint-v2.2/PROPOSAL.md` with exact old → new text. Billy applies them in the claude-blueprint repo, and Foreman's golden fixture pins the result. Annotating Stages 0–5 is itself a Stage 6 ticket (it needs the parser to verify the annotations), so the proposal defines the edits and the grammar now, and the ticket produces the annotated file.

## 5. v1 scope (final, pending red team)

**v1:** P-01 to P-18, minimal P-21 (Add, project-only), minimal P-23 (Desktop handoff), blueprint v2.2 (B-1 to B-6 first part, B-8), reserved data shapes (`adopted`, `amendments/`, `revisions[]`, ticket loop state).

**v1.1, the scheduled release after CO:** P-19 Crew Loop, P-20 CR track, the rest of P-21, P-22, P-24, P-25.

**Build order** (from r2 §8.2, kept): template core → chat path (TradeLog Stage 1 runs) → CLI path (Stage 2 runs; kill criterion read from the metric panel) → finish (Stages 3–5 run).

**Estimate:** 16–24 weeks solo with Claude Code (r2). This is honest and large. The kill criterion is checked after Stage 2 of the TradeLog run, roughly halfway, so the investment is bounded.

## 6. Questions for the red team

1. Is re-arming (ending and resuming the Claude process between chain steps) safe and acceptable in daily use?
2. Do per-launch `--settings` hooks create a security or reliability risk (the helper binary, the events directory, a malicious repo's settings)?
3. Can a malicious or careless repo (overlay, packets, tickets, `.foreman/project.json` verify commands) make Foreman run something Billy didn't intend?
4. Is the Markdown-plus-annotations template robust enough, or is it the project's biggest liability?
5. Is the v1 scope realistic for one person, and is the build order right?
6. Is anything in the integration built on undocumented Claude Code behavior that could break with an update?
