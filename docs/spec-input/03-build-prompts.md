# Foreman — Build Prompts

Every prompt to build Foreman, in order, from the signed App Brief to Certificate of Occupancy. This follows the Claude Build Blueprint v2.1, tailored for a personal macOS tool on the **Max plan**, with **no Fable models anywhere**: Opus 5.5 at xhigh takes every role Fable had.

Foreman's conditions: `idea_known` yes · `has_ui` yes · everything else no. So these are not applicable and are recorded as skipped in their packets: Stage 0, demand validation, the competitor teardown, pricing, legal pages, migrations and auth (no database, single user), Stage 10 payments, SLOs and load tests (not a server), marketing, onboarding for other users, beta and rollout rings.

---

## How to run this file

- Run the prompts in order. Each starts with a **RUN WITH** line: open that surface with that model, effort, and mode, then paste the prompt.
- **New CLI session:** use the command shown. **Mid-session:** type the `/model` and `/effort` commands shown, and press Shift+Tab to change mode.
- **Desktop steps:** in the Code tab, set the model (Cmd+Shift+I), effort (Cmd+Shift+E), and mode (Cmd+Shift+M), and turn worktree on.
- Every Claude Code stage ends with the **Stage Gate** (three prompts at the end of this section). Sign each packet before starting the next stage.
- Session names (`--name fm-…`) make every session easy to resume with `claude --resume <name>`.

### Model routing for this build

| Work | Model · Effort |
|---|---|
| Council chair, architecture, threat model, HARD tickets, audits, stuck problems | **Opus 5.5 · xhigh** (use `max` only for one truly stuck problem, session only) |
| Planning, normal features, reviews, inspections | **Opus 5.5 · high or medium** |
| Spec-complete tickets | **Sonnet 5.5 · medium** |
| Mechanical edits, search, test runs | **Haiku 5.5 · low** |

---

## The Crew Loop (used in Stages 6–13)

| Ticket class | Model · Effort | Mode |
|---|---|---|
| SPEC-COMPLETE | Sonnet 5.5 · medium | Accept edits (or the `implementer` agent) |
| NEEDS-DESIGN | Opus 5.5 · medium | Plan → Accept edits |
| HARD | Opus 5.5 · xhigh | Plan → Manual |
| MECHANICAL | Haiku 5.5 · low | Accept edits |
| BUG | Opus 5.5 · high | Plan → Accept edits (or the `debugger` agent) |
| STUCK | Opus 5.5 · xhigh (max if needed) | Plan → Auto |

**Loop 1: Plan.** RUN WITH · Desktop, new session, worktree ON · model and effort from the table · **Plan** · agents: `Explore`

```text
Ticket [ID] from docs/tickets.md. Read the ticket, every ADR it lists,
docs/domain-rules.md, docs/lessons.md, and the latest docs/handoff/
packet. Use the Explore agent to find every file this ticket touches.
In plan mode, give me:
1. the implementation plan: an ordered file list with the change in each
2. the tests you'll write, mapped one-to-one to the acceptance criteria
3. any ambiguity or ADR conflict, as numbered questions
Do not write code yet.
```

**Loop 2: Build.** RUN WITH · same session · same model · **Accept edits** (HARD: **Manual**) · agents: `test-runner`

```text
Approved. Write the tests for every acceptance criterion first and show
me they fail. Then implement until they pass. Afterwards, use the
test-runner agent for the full verify suite (pnpm verify). For UI work,
check the screen in the Browser pane (mock IPC mode) at 1280 and 1600
widths, in Night and Vellum themes, including empty, loading, error,
and not-applicable states.
```

**Spec-complete shortcut.** RUN WITH · Desktop · **Sonnet 5.5 · medium** · **Accept edits** · agents: `implementer`, `test-runner`, `code-reviewer`

```text
Use the implementer agent on ticket [ID] from docs/tickets.md. When it
reports back, run the test-runner agent, then the code-reviewer agent.
```

**Loop 3: Review.** RUN WITH · same session · `/model opus` · `/effort high` · **Accept edits** · agents: `code-reviewer`. Run `/code-review`, use **Walk through in diff**, then:

```text
Use the code-reviewer agent on this diff. Fix every CRITICAL and WARNING.
List the SUGGESTIONs and Nits and I'll decide.
```

**Loop 4: Commit.** RUN WITH · same session · `/model sonnet` · `/effort low` · **Accept edits**

```text
Write a conventional commit message referencing [ID] and the ADRs it
touches, and commit. If the repo has a remote, open a PR whose
description lists each acceptance criterion and how it was verified.
```

**Loop 5: Close.** Archive the Desktop session once the work is merged.

---

## The Stage Gate (ends every Claude Code stage)

**Gate A: Inspection.** RUN WITH · CLI, fresh session · **Opus 5.5 · high** · **Plan** · agents: `test-runner` + the stage's reviewer

`claude --model opus --effort high --permission-mode plan --name fm-foreman-gate-s[NN]`

```text
Run the Stage [N] inspection for Foreman. Read docs/handoff/stage-[N-1].md
for context and docs/lessons.md. For each inspection item below, verify
it with real evidence (command output, test names, file paths) and mark
PASS or FAIL. Use the test-runner agent for every automated check.
Do not fix anything; just report.
INSPECTION ITEMS:
[paste the stage's inspection list]
```

**Gate B: Packet.** RUN WITH · same session · **Opus 5.5 · medium** · Shift+Tab → **Accept edits**

```text
All items PASS. Write docs/handoff/stage-[N].md in exactly this format
(see docs/templates/handoff-packet.md), and commit it as
"handoff: stage [N]":

=== HANDOFF PACKET · Stage [N] → Stage [N+1] ===
PROJECT: Foreman
APPROVED BY: <blank>
COMPLETED: what this stage delivered, with file paths
EVIDENCE: each inspection item → PASS + its evidence
DECISIONS: new or amended ADRs (IDs + one line each)
STATE: what now exists and works (commands, builds)
SKIPPED: not-applicable or optional steps, each with its reason
KNOWN LIMITS: deliberate deferrals, each with its ticket ID
OPEN ITEMS: none
NEXT: Stage [N+1] · <name> · <surface> · <model> · <effort> · <mode>
INPUTS FOR NEXT STAGE: the files, tickets, and facts it must read first
```

**Gate C: Lessons.** RUN WITH · same session · **Opus 5.5 · medium** · **Accept edits**

```text
Before we close Stage [N], capture what this stage taught us. Review
this session, the commits for the stage, and every reviewer finding.
Append entries to docs/lessons.md in the L-NNN format at the top of that
file: anything I had to fix or redo, anything a reviewer or gate caught
that the builder should have caught, any prompt that produced weak
output, any agent that missed something, and anything that wasted usage.
Propose the exact edit that would prevent each one next time. If nothing
went wrong, write "clean stage" with one line on why.
```

Then read the packet, write your name and date under APPROVED BY, and commit.

---

## Stage 0: Land Survey

Not applicable: you already know what you're building (`idea_known` = yes).

---

## Setup: one time, in your terminal

These are commands for you to run, not prompts. Adjust the paths to where you keep the two kits.

```bash
# Prerequisites (skip any you already have)
xcode-select --install                       # Xcode Command Line Tools
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust via rustup
corepack enable                              # pnpm via Node's corepack (Node LTS required)
claude update                                # Haiku 5.5 needs v2.1.293+

# Install the blueprint's global agents and settings (backs up anything it replaces)
cd ~/path/to/claude-blueprint && ./install.sh

# Get the repo (already set up: spec inputs, docs scaffolding, and agents)
mkdir -p ~/Developer && cd ~/Developer
git clone https://github.com/billyribeiro-ux/engine-assist.git
cd engine-assist
```

The repo already contains everything the old manual setup created: `docs/spec-input/` (brief, spec, design direction, these prompts, and the blueprint v2.1), `docs/handoff/`, `docs/lessons.md`, `docs/templates/`, the eight global agents in `.claude/agents/` (so they work in any session, cloud or local), and the Stage 2 council agents in `spec/.claude/agents/`.

---

## Stage 1: Meet the Architect (done, review and sign)

The architect conversation already happened in chat. Its output is `docs/spec-input/00-app-brief.md`.

1. Read it. Change anything that's wrong. It's your brief.
2. Optional: pressure-test it in chat first:

> **RUN WITH** · Claude chat · **Opus 5.5** · effort adaptive

```text
Here is the App Brief for a personal macOS tool I'm about to build:
[paste docs/spec-input/00-app-brief.md]
Act as a skeptical principal product engineer. Find what's missing,
wrong, or over-built for a single-user personal tool, and what would
make it faster to use every day. Give me at most 10 numbered points,
most important first, each with the exact change to the brief.
```

3. Sign it: write your name and date under APPROVED BY. Then:

```bash
cp docs/spec-input/00-app-brief.md docs/handoff/stage-01.md
git add -A && git commit -m "handoff: stage 1"
```

---

## Stage 2: Architect's Design Studio

The council reviews and completes the draft Master Spec. For a personal tool, the commercial council roles (market-analyst, growth-strategist) aren't needed. The principal engineer, the visionary, the user advocate (speaking for you), and the red team are.

### Prompt 2.1: Convene the council

> **RUN WITH** · CLI in `spec/` · **Opus 5.5** · effort **xhigh** · **Plan** first, then **Accept edits** · agents: `principal-engineer`, `product-visionary`, `user-advocate`, `red-team`
>
> `cd spec && claude --model opus --effort xhigh --permission-mode plan --name fm-foreman-s02-council`

```text
You are the chair of the design council for Foreman, a personal macOS
tool. Read these inputs:
- ../docs/handoff/stage-01.md (the signed App Brief)
- ../docs/spec-input/01-master-spec.md (draft Master Spec v0.9)
- ../docs/spec-input/02-design-direction.md (draft design direction)
- ../docs/spec-input/claude-build-blueprint.md (the blueprint Foreman runs)
Goal: a complete, correct, frozen Master Spec. This is a single-user tool,
so judge everything by how well it serves one demanding principal
engineer running several projects, not by market appeal.

Run these rounds, in parallel wherever a round allows:
ROUND 1, Evidence (parallel):
 - principal-engineer: answer every open question in §13 and verify
   every row of §11 against current official docs (Tauri, SvelteKit 3,
   Svelte 5, Vite 8, xterm.js, portable-pty, tauri-plugin-pty, notify,
   comrak, pulldown-cmark, Bits UI, tauri-driver), with citations.
   Also verify every Claude Code flag in §6 against the official CLI
   reference. -> council/r1-engineering.md
 - user-advocate: walk the daily workflow end to end as the single user
   (start a project, run a step, sign a packet, run a gate, work a
   ticket, switch between three projects). Find every point of friction
   and every missing affordance. -> council/r1-workflow.md
ROUND 2, Improve (after Round 1):
 - product-visionary: propose the changes that would make Foreman
   dramatically faster and more reliable to use daily. Keep the v1
   scope tight; mark anything else v1.x. -> council/r2-proposals.md
ROUND 3, Converge (you): decide on every proposal and finding; update
 the feature catalog (F-IDs), the rules (§5), the integration (§6), the
 data layout (§7), the packet rules (§8), the requirements (§9), the
 NFRs (§10), and the stack (§11). -> council/r3-synthesis.md
ROUND 4, Attack: red-team attacks r3-synthesis.md, especially the
 template-parsing assumption, security of process spawning, and data
 integrity of the files Foreman writes. -> council/r4-redteam.md
ROUND 5, Resolve (you): answer every fatal and serious objection, in
 writing. -> council/r5-resolution.md

First show me the plan for the rounds. After each round, give me a
5-line summary and wait for my "go".
```

### Prompt 2.2: Stack decision, evidence first

> **RUN WITH** · same session · **Opus 5.5 · xhigh** · **Accept edits** · agents: `principal-engineer`

```text
Recommend Foreman's stack layer by layer: desktop shell, frontend
framework, language and tooling, terminal and PTY, file watching,
Markdown parsing, UI primitives, state management, testing (unit,
integration, e2e), and linting.

PART 1, evidence-first (ignore my preferences): for each layer, the
requirements from the spec and NFRs, the 2-3 strongest candidates, and
the winner. Judge on fit, maturity and support horizon, macOS behavior,
performance, security record, maintenance activity, and how well Claude
Code works with it. The principal-engineer agent verifies every version
and claim against current official docs, cited.

PART 2, compare with my preference: Tauri, SvelteKit 3 + Svelte 5,
TypeScript strict, pnpm, Rust. For each layer where you differ, say what
I gain, what I lose, and your confidence. Where they're equal, say so.

PART 3: the final recommendation in one table. I decide.
-> council/r6-stack.md
```

Write your final choice and reason at the bottom of `council/r6-stack.md`.

### Prompt 2.3: Freeze the Master Spec

> **RUN WITH** · same session · **Opus 5.5 · xhigh** · Shift+Tab → **Manual**

```text
Compile everything into ../docs/MASTER-SPEC.md v1.0: vision, the user
and jobs, concepts, the feature catalog (F-IDs, each with the problem,
behavior, edge cases, and release v1 or v1.x), the rules (tags,
conditions, the default tagging table), the Claude Code integration
(exact argv rules, headless gate contract, version minimums), files and
data, packet rules, security and privacy requirements, NFRs, the final
stack table with evidence links and my decision, non-goals, and change
control (every change after v1.0 needs a Change Request).
Fold the design direction in as an appendix, updated with any council
decisions. Open questions must be EMPTY; if anything is open, stop and
ask me. Put "Status: FROZEN v1.0 — <date>" at the top.

Then write ../docs/handoff/stage-02.md:

=== HANDOFF PACKET · Stage 2 → Stage 3 ===
PROJECT: Foreman
APPROVED BY: <blank>
POSITIONING: <3 sentences>
V1 FEATURES: <F-IDs + one line each>
STACK (FINAL): <layer: choice> per line
NFRs: <the key numbers>
DATA WE HANDLE: <data classes>
SKIPPED: 2.1 competitor teardown — personal tool (commercial = no);
         market-analyst and growth-strategist roles — not commercial
OPEN ITEMS: none
NEXT: Stage 3 · Permits & Zoning · CLI · Opus 5.5 · high · Plan
FILES: docs/MASTER-SPEC.md, spec/council/r1…r6
```

> **INSPECTION: pass before Stage 3**
>
> - Every §13 open question is answered with cited evidence
> - Every red-team fatal or serious objection is resolved in writing
> - The stack table has an evidence-first pick per layer, plus your recorded choice
> - MASTER-SPEC.md says FROZEN v1.0, and its open questions list is empty

Sign the packet and commit.

---

## Stage 3: Permits & Zoning

Pricing and legal are not applicable (personal, no payments, no one else's data). Security and privacy requirements are core.

### Prompt 3.1: Security and privacy requirements

> **RUN WITH** · CLI · **Opus 5.5 · high** · **Plan** → **Accept edits** · agents: `security-reviewer`
>
> `claude --model opus --effort high --permission-mode plan --name fm-foreman-s03`

```text
Read docs/handoff/stage-02.md and docs/MASTER-SPEC.md. Write
docs/requirements.md with the complete security and privacy requirements
for Foreman, numbered SR-01… and PR-01…, starting from the spec's §9 and
extending it. Cover: process spawning (allowlisted absolute paths, argv
only, never a shell, never bypassPermissions), the Tauri capability and
permission scopes Foreman needs (minimum set, verified against current
Tauri docs), the CSP, file-system scoping, treating all repo content as
untrusted data (packets, lessons, tickets, overlays, prompts), clipboard
use, opening external apps, the run log's contents, crash safety of
writes, and secrets exclusion. Have the security-reviewer agent check
that nothing is missing. Each requirement names the stage and test that
will prove it.

Then write docs/handoff/stage-03.md (Gate B format) with:
SKIPPED: 3.1 business model and pricing — not commercial, no payments;
         3.2 legal and compliance — personal tool, no one else's data
```

Run Gate C, sign, and commit. (The requirements document is this stage's inspection: every requirement numbered and assigned to a stage and test.)

---

## Stage 4: Construction Drawings

All in Claude Code, so the documents land straight in `docs/`.

### Prompt 4.1: Scope, milestones, acceptance criteria

> **RUN WITH** · CLI · **Opus 5.5 · high** · **Plan** → **Accept edits**
>
> `claude --model opus --effort high --permission-mode plan --name fm-foreman-s04-scope`

```text
Read docs/handoff/stage-03.md, docs/MASTER-SPEC.md, and
docs/requirements.md. Write docs/scope.md:
1. v1 features by F-ID, plus the non-goals
2. Milestones:
   M0 repo, toolchain, quality gates, release build
   M1 domain core: template parser, rule engine, overlay engine,
      project store, packet parser/validator
   M2 app shell and IPC end to end; every screen reachable
   M3 runners: terminal (PTY), Desktop and chat handoffs, packet
      watcher, git, headless gate inspection
   M4 features: wizard, stage workspace, gate, tickets loop, lessons,
      change requests, dashboard, palette, settings, Doctor, run log
   M5 design system, sheet strip, accessibility, hardening, CO
3. For every feature: user stories with acceptance criteria in
   Given/When/Then form, covering empty states, errors, external file
   edits, concurrent writes, missing tools, and locked/not-applicable
   rules
4. Map every SR and PR requirement to the milestone that delivers it
```

### Prompt 4.2: Design doc and ADRs

> **RUN WITH** · same session · `/effort xhigh` · **Plan** → **Accept edits** · agents: none (the council agents live in `spec/`; verify with web search and official docs directly)

```text
Act as a principal architect designing Foreman to last 20 years. The
final stack in docs/handoff/stage-02.md is fixed input.

PART 1: docs/design-doc.md, in Google's structure: context and scope;
goals and non-goals; the design (system-context diagram, Rust core
modules, the Svelte app structure, the IPC boundary, the data flow for
"run a step" and "packet lands", the file layout); alternatives
considered, each with why it lost; cross-cutting concerns (security,
privacy, observability, accessibility, performance, data integrity,
upgrade and migration paths).

Required design decisions:
- the domain core lives in Rust (template parser, rule engine, overlay,
  packet validator, prompt resolver), fully unit- and property-tested
- the frontend talks to Rust only through a typed IPC layer, with a mock
  implementation used for browser preview and component tests
- all file writes are atomic, versioned, and never overwrite a newer
  external edit

PART 2: one ADR per significant decision in docs/adr/, in the
blueprint's ADR format (Status, Date, Context, Decision, Alternatives
considered, Consequences, Verification, Revisit when, Exit path).
Verify every library and version against current official docs.
```

### Prompt 4.3: Threat model

> **RUN WITH** · CLI, fresh session · **Opus 5.5 · xhigh** · **Plan** → **Accept edits** · agents: `security-reviewer`
>
> `claude --model opus --effort xhigh --permission-mode plan --name fm-foreman-s04-threats`

```text
Read docs/design-doc.md, the ADRs, and docs/requirements.md. Write
docs/threat-model.md, Microsoft SDL style: data flow diagrams for each
trust boundary (webview ↔ Rust, Rust ↔ child processes, Rust ↔ file
system, repo content → UI), a STRIDE analysis per element, and for each
threat its likelihood, impact, mitigating control, the SR/PR it
satisfies, and the test that proves it. Include abuse cases: a malicious
repo whose packet, ticket, or overlay text tries to inject commands or
change a run's flags; a symlink escaping a project folder; a crash
mid-write; a tampered blueprint file; a hostile filename. Have the
security-reviewer agent review it before you finish.
```

### Prompt 4.4: Contracts and domain rules

> **RUN WITH** · same session · `/effort high` · **Accept edits**

```text
Write:
1. docs/contracts/ipc.md: every IPC command and event between the
   frontend and Rust, with typed request/response shapes, errors, and
   which feature uses it
2. docs/contracts/files.md: the JSON schema for every file Foreman owns
   (settings, projects, project, overlay, state, run log), each with
   schemaVersion and the migration rule
3. docs/domain-rules.md: the blueprint grammar Foreman parses (stages,
   steps, RUN WITH lines, prompts, inspections, packets), the tagging and
   condition rules, the overlay rules, the packet validation rules, and
   the prompt resolution rules, each numbered, each with worked examples
   from the real blueprint text, and each assigned to exactly one module
```

### Prompt 4.5: Design system and screen mockups

> **RUN WITH** · Desktop, new session · **Opus 5.5 · high** · **Plan** → **Accept edits**

```text
Read the design appendix in docs/MASTER-SPEC.md. Produce:
1. docs/design/tokens.css: every design token as CSS custom properties
   for Night and Vellum themes, plus the phase colors
2. docs/design/mockups/: one self-contained HTML file per screen
   (projects, wizard, project workspace with the sheet strip and
   terminal drawer, gate, tickets, lessons, change requests, run log,
   Doctor, settings, command palette), with realistic TradeLog content,
   including empty, error, locked, and not-applicable states
3. docs/design/components.md: every component with its states, keyboard
   behavior, and accessibility notes
Open each mockup in the Browser pane and check both themes before you
finish. Keep the boldness in the sheet strip and the stamp moment;
everything else stays quiet and native.
```

### Prompt 4.6: Tickets

> **RUN WITH** · CLI · **Opus 5.5 · high** · **Plan** → **Accept edits**
>
> `claude --model opus --effort high --permission-mode plan --name fm-foreman-s04-tickets`

```text
Read docs/scope.md, docs/design-doc.md, the ADRs, docs/threat-model.md,
docs/contracts/, docs/domain-rules.md, and docs/design/. Write
docs/tickets.md: every milestone broken into tickets sized for one
Claude Code session. For each:
ID [M#-NN] | Title | Class: SPEC-COMPLETE / NEEDS-DESIGN / HARD /
MECHANICAL | Stage (6-13) | Depends on | ADRs | Threats mitigated |
Likely files | Acceptance criteria (Given/When/Then) | Required tests
(named) | Out of scope
Anything touching parsing, the rule engine, file writes, process
spawning, or the PTY is HARD.
```

### Prompt 4.7: Independent design review

> **RUN WITH** · CLI, **brand-new session** (fresh context) · **Opus 5.5 · xhigh** · **Plan** · agents: `design-reviewer`
>
> `claude --model opus --effort xhigh --permission-mode plan --name fm-foreman-s04-review`

```text
Use the design-reviewer agent. It did not write any of these documents.
Review docs/MASTER-SPEC.md, docs/scope.md, docs/design-doc.md,
docs/adr/, docs/threat-model.md, docs/contracts/, docs/domain-rules.md,
docs/design/, and docs/tickets.md against each other. Find:
requirements with no design, designs with no requirement, ADRs without
real alternatives, threats without tests, contract gaps, missing failure
modes, untestable acceptance criteria, and anything that will hurt in 5
years. Rate each BLOCKER / MAJOR / MINOR, with the fix. Report only.
```

Fix every BLOCKER and MAJOR (same model, Accept edits), then mark all ADRs Accepted.

### Prompt 4.8: CLAUDE.md and the Build Package

> **RUN WITH** · CLI · **Opus 5.5 · medium** · **Accept edits**
>
> `claude --model opus --effort medium --permission-mode acceptEdits --name fm-foreman-s04-package`

```text
Write the repo's CLAUDE.md, under 150 lines: a one-line description;
the stack with exact versions; the repo layout; exact commands (install,
dev, dev in browser with mock IPC, verify, unit, integration, e2e,
build, release build); conventions; the definition of done; paths to
every doc; the never rules (never spawn a shell, never pass
bypassPermissions, never write a file non-atomically, never render repo
content as HTML, never use Fable in this project); and "before planning,
read the latest docs/handoff/ packet and docs/lessons.md".

Then write docs/handoff/stage-04.md (Gate B format), with
BLUEPRINT VERSION: 2.1 and SKIPPED: none.
```

> **INSPECTION: pass before Stage 5**
>
> - The independent review has zero open BLOCKER or MAJOR findings
> - Every SR/PR maps to a design element; every threat maps to a control and a test
> - Every domain rule has worked examples from the real blueprint
> - Every ticket has a class, a stage, and named tests

Run Gate C, sign, and commit.

---

## Stage 5: Site Prep & Excavation

### Prompt 5.1: Scaffold

> **RUN WITH** · CLI · **opusplan** · effort **medium** · **Plan** · agents: `test-runner`
>
> `claude --model opusplan --effort medium --permission-mode plan --name fm-foreman-s05-scaffold`

```text
Read docs/handoff/stage-04.md, CLAUDE.md, docs/design-doc.md, and every
ADR. Plan the M0 scaffold exactly as the ADRs specify: the Tauri app
with the SvelteKit 3 frontend in SPA mode, the Rust workspace (an app
crate plus a separate domain-core crate with no Tauri dependency), the
typed IPC layer with its mock implementation, pnpm workspace, strict
TypeScript, svelte-check, ESLint, Prettier, rustfmt, clippy with
-D warnings, Vitest, cargo test with proptest, and the e2e harness chosen
in the ADRs. Use the latest stable versions, verified against official
docs and registries. Pin with lockfiles. List every file you'll create.
Do not write files yet.
```

Approve the plan, then:

```text
Use the test-runner agent to run the full suite and fix everything until
ALL GREEN.
```

### Prompt 5.2: Quality gates

> **RUN WITH** · same session · `/model opus` · `/effort high` · Shift+Tab → **Plan** · agents: `security-reviewer`

```text
Add a single `pnpm verify` command that runs, and fails on any problem:
svelte-check, ESLint, Prettier check, TypeScript strict, Vitest, rustfmt
check, clippy -D warnings, cargo test, cargo audit, pnpm audit, a
secret scan, and a license check. Add a git pre-push hook that runs it.
If the repo is pushed to GitHub, add a GitHub Actions workflow on a
macOS runner that runs the same command on every PR. Verify each tool's
current setup in its official docs. Implement after I approve.
```

### Prompt 5.3: ADR reality check

> **RUN WITH** · same session · **Opus 5.5 · high** · **Plan**

```text
Now that real code and real library versions exist, re-check every ADR
against them and their current docs. Report VALID, NEEDS AMENDMENT
(what), or CONTRADICTED (evidence) for each. Don't edit anything.
```

### Prompt 5.4: Claude Code automation

> **RUN WITH** · same session · `/model sonnet` · `/effort low` · **Accept edits**

```text
Create .claude/settings.json with a PostToolUse hook matching Edit|Write
that formats only the edited file (prettier for TS/Svelte, rustfmt for
.rs) via scripts/claude-format.sh, which reads tool_input.file_path from
stdin JSON and must be executable. Create .claude/launch.json with a
"web" configuration that runs the frontend dev server in mock-IPC mode
(pnpm dev:web, port 5173, autoPort true), so the Desktop Browser pane
can preview every screen without the Tauri shell.
```

### Prompt 5.5: Release build

> **RUN WITH** · same session · `/model opus` · `/effort high` · **Plan** → **Accept edits**

```text
Set up the release build: `pnpm release` runs `pnpm verify`, then
`pnpm tauri build` for Apple Silicon, then copies Foreman.app to
/Applications after confirming with me. Version comes from one place
and appears in the About window. Add docs/runbook-release.md with build,
install, rollback (keep the previous .app as Foreman-previous.app), and
how to find the logs.
```

> **INSPECTION: pass before Stage 6**
>
> - Fresh clone → `pnpm install` → `pnpm tauri dev` works with no manual steps
> - `pnpm verify` is green, and fails on a planted type error, a planted fake secret, and a clippy warning (then revert)
> - `pnpm release` produces a working Foreman.app; the rollback in the runbook works
> - The browser preview opens in the Desktop Browser pane in mock-IPC mode
> - ADR reality check: no CONTRADICTED left open

Run the Stage Gate (A, B, C). In packet B, record: SKIPPED: staging deploy — not a server (`server` = no).

---

## Stage 6: Foundation (the domain core)

Every ticket here runs through the **Crew Loop**, almost all as HARD. These four prompts kick off the core modules; the tickets in `docs/tickets.md` carry the details.

### Prompt 6.1: Template parser

> **RUN WITH** · Desktop, worktree ON · **Opus 5.5 · xhigh** · **Plan** → **Manual** · agents: `Explore`, `test-runner`, `code-reviewer`

```text
ultrathink. Ticket [M1-…]: the template parser in the domain-core crate.
docs/domain-rules.md defines the grammar. Copy
docs/spec-input/claude-build-blueprint.md into test fixtures as the
golden input (blueprint v2.1). Before writing the parser:
1. Write golden tests asserting the exact parse of the real blueprint:
   19 stages with names and phases, every step with its RUN WITH config
   (surface, model, effort, mode, agents, command), every prompt's exact
   text, every inspection list, every handoff description.
2. Write property tests: parsing never panics on arbitrary input; every
   error has a line number; a parse followed by a render reproduces
   the structure.
3. Then implement until the tests pass. Unknown or malformed sections
   are errors with line numbers, never silently dropped.
```

### Prompt 6.2: Rule engine and overlay engine

> **RUN WITH** · Desktop, new session, worktree ON · **Opus 5.5 · xhigh** · **Plan** → **Manual** · agents: `test-runner`, `code-reviewer`

```text
ultrathink. Tickets [M1-…]: the rule engine and the overlay engine in
domain-core, per the tagging and overlay rules in docs/domain-rules.md.
Tests first: table-driven tests for every row of the default tagging
table, against every combination of the eight conditions that changes
an outcome; property tests that core items can never become skipped or
not applicable by any overlay; that a Change Request flipping a
condition reactivates exactly the right items; that a skip without a
reason is rejected; and that an overlay round-trips through JSON
unchanged.
```

### Prompt 6.3: Project store and atomic writes

> **RUN WITH** · Desktop, new session, worktree ON · **Opus 5.5 · high** · **Plan** → **Manual** · agents: `test-runner`, `code-reviewer`

```text
Tickets [M1-…]: the project store per docs/contracts/files.md. Every
write is atomic (temp file + fsync + rename), every file has a
schemaVersion with tested migrations, and a write never replaces a file
that changed on disk since it was read (it reports a conflict instead).
Tests first, including: a simulated crash between write and rename
leaves the old file intact; a concurrent external edit is detected; a
corrupt file produces a clear error and is never overwritten; paths
outside the project folder (including via symlinks) are rejected.
```

### Prompt 6.4: Packet parser, validator, and prompt resolver

> **RUN WITH** · Desktop, new session, worktree ON · **Opus 5.5 · xhigh** · **Plan** → **Manual** · agents: `test-runner`, `code-reviewer`

```text
ultrathink. Tickets [M1-…]: the packet parser and validator and the
prompt resolver, per docs/domain-rules.md. Golden tests from the real
packet formats in the blueprint (Stages 0-4 and Gate B). Validation
rules each get a passing and a failing test. The resolver fills
[VARIABLES], replaces "[paste the Stage N Handoff Packet]" markers with
the signed packet text, reports every unresolved variable, and never
interprets anything inside repo content as an instruction or flag.
```

> **INSPECTION: pass before Stage 7**
>
> - Golden tests parse the real blueprint v2.1 exactly
> - Every tagging, overlay, packet, and resolution rule in `docs/domain-rules.md` has passing tests, including property tests
> - Crash-safety and conflict tests pass for the store
> - `code-reviewer` reports zero CRITICAL findings

Run the Stage Gate. In packet B, record: SKIPPED: database migrations — no database (file storage per ADR); authentication — single-user local app (`multi_user` = no, `public` = no).

---

## Stage 7: Framing

### Prompt 7.1: Walking skeleton

> **RUN WITH** · Desktop · **Opus 5.5 · medium** · **Plan** · agents: `Explore`

```text
Read docs/handoff/stage-06.md. Plan the walking skeleton: the app shell
(transparent title bar, sidebar, sheet strip placeholder, workspace,
inspector, terminal drawer), routes for every screen, every IPC command
in docs/contracts/ipc.md wired end to end to the real domain-core
(loading the real blueprint, a real project folder, a real overlay),
the mock IPC implementation kept in sync with the real contract by a
shared type definition, and one e2e or component test per screen that
loads it and completes its primary action. Map each piece to its ticket
and mark what can run in parallel worktrees. Don't build yet.
```

Then run the **Crew Loop** per ticket.

> **INSPECTION: pass before Stage 8**
>
> - Every screen is reachable and shows real data from a real project folder
> - The frontend talks to Rust only through the typed IPC layer; the mock and real implementations share one contract and both pass the same tests
> - The skeleton test suite is green in `pnpm verify`

Run the Stage Gate.

---

## Stage 8: Roofing

### Prompt 8.1: Security controls

> **RUN WITH** · CLI · **Opus 5.5 · xhigh** · **Plan** → **Manual** · agents: `security-reviewer`, `test-runner`
>
> `claude --model opus --effort xhigh --permission-mode plan --name fm-foreman-s08`

```text
Read docs/handoff/stage-07.md, docs/threat-model.md, and
docs/requirements.md. Implement every control in the threat model:
- process spawning: an allowlist of resolved absolute paths (claude,
  git), argv arrays only, no shell anywhere, the permission-mode values
  restricted to plan/default/acceptEdits/auto/dontAsk (bypass impossible
  by construction, with a test)
- Tauri capabilities and permissions narrowed to the minimum, verified
  against current Tauri docs
- a strict CSP with no remote or inline script
- file-system scoping enforced in Rust (project folders, blueprint repo,
  app data), with symlink-escape tests
- repo content rendered as text only, never HTML; a test feeds hostile
  packet, ticket, and overlay content through every view
- overlay edits to prompts or run configs flagged with a diff before
  any run
- .env files excluded from every view and watcher
Update docs/threat-model.md so every threat links to its control and
passing test. Then have the security-reviewer agent review everything.
```

> **INSPECTION: pass before Stage 9**
>
> - Every threat links to a control and a passing test
> - No code path can spawn a shell or pass bypassPermissions (tests prove it)
> - `security-reviewer` reports zero CRITICAL or HIGH findings

Run the Stage Gate.

---

## Stage 9: Plumbing (the runners)

### Prompt 9.1: Runner plan

> **RUN WITH** · Desktop · **Opus 5.5 · high** · **Plan** · agents: `Explore`, `security-reviewer`

```text
Read docs/handoff/stage-08.md. Plan the runner tickets:
- the PTY terminal runner: spawn `claude` with the exact argv from the
  spec (model, effort, permission mode, --name, prompt), stream to
  xterm.js, resize, multiple tabs, exit status, and resume by session name
- the Desktop handoff (copy prompt + `claude --desktop` in the project
  folder) and the chat handoff (copy prompt, open Claude chat, paste-back
  packet validation)
- the packet watcher (notify, debounced; ignores .git, node_modules,
  target, .env*), with detection under 1 second
- git: status and committing a signed packet only, through the process
  allowlist
- headless gate inspection: `claude -p` with --output-format json,
  --json-schema, --allowedTools limited to the project's verify
  commands, and --permission-prompts none; results mapped into the gate
  checklist
- Doctor: claude version vs the minimums, `claude auth status`, git,
  pnpm, node, cargo, blueprint repo, installed agents
Mark which tickets are HARD (the PTY runner and headless inspection are).
Don't build yet.
```

Then run the **Crew Loop** per ticket.

> **INSPECTION: pass before Stage 10**
>
> - From a real project, Foreman launches Claude Code in the embedded terminal with the exact argv shown on screen, and the session resumes by name
> - A packet written by Claude Code appears in Foreman within 1 second, validated
> - Headless inspection returns structured results that fill a gate checklist
> - Doctor detects a too-old `claude` and a logged-out state correctly (tested with fakes)

Run the Stage Gate.

---

## Stage 10: Electrical

Not applicable: Foreman doesn't take payments (`payments` = no). Record it in the Stage 11 packet's SKIPPED line and move on.

---

## Stage 11: HVAC

### Prompt 11.1: Logs, run log, diagnostics

> **RUN WITH** · Desktop · **Sonnet 5.5 · medium** · **Accept edits** · agents: `test-runner`, `sre-reviewer`

```text
Read docs/handoff/stage-09.md. Implement: structured JSON logs written
to the app data folder (rotated, size-capped), never containing prompt
secrets or file contents beyond names; the run log (one JSON Lines file
per month) with every run's time, project, stage, step, ticket, argv,
session name, and outcome; a crash handler that records the error and
preserves state; and the data behind the dashboard metrics (time per
stage, runs per ticket, gate results). Then have the sre-reviewer agent
check that every failure a user could hit produces a clear message and
a log entry.
```

> **INSPECTION: pass before Stage 12**
>
> - A forced failure in each runner shows a clear message and writes a log entry
> - The run log records every run, and the dashboard numbers match it
> - A forced crash loses no state

Run the Stage Gate. SKIPPED: 10 Electrical — no payments; SLOs, error budgets, and analytics — not a server, single user.

---

## Stage 12: Insulation & Drywall

### Prompt 12.1: Milestone sweep

> **RUN WITH** · Desktop · **Opus 5.5 · medium** · **Plan** · agents: `Explore`

```text
Read docs/handoff/stage-11.md. List every open ticket in M4 from
docs/tickets.md with its class, dependencies, build order, the routed
model and effort from the Crew Loop table, and which can run in
parallel worktrees.
```

Then run the **Crew Loop** on every ticket: the wizard, stage workspace, gate, tickets loop, lessons and promote-to-blueprint, change requests, dashboard, command palette, settings, and run log screens.

**From here on, dogfood:** once the stage workspace and gate work, run Foreman's own remaining gates inside Foreman.

> **INSPECTION: pass before Stage 13**
>
> - Every ticket's acceptance criteria are verified by named tests
> - Every screen has working empty, loading, error, locked, and not-applicable states
> - `pnpm verify` is green on main

Run the Stage Gate.

---

## Stage 13: Siding & Exterior

### Prompt 13.1: Design system and the sheet strip

> **RUN WITH** · Desktop · **Opus 5.5 · high** · **Plan** → **Accept edits** · agents: `code-reviewer`

```text
Read docs/handoff/stage-12.md and docs/design/. Apply docs/design/tokens.css
across every component (no hard-coded colors, sizes, or spacing; add a
lint rule that fails on them). Build the sheet strip and the title block
exactly as designed, and the one signature moment: the approval stamp
when a packet is signed (220 ms, off under prefers-reduced-motion).
Bundle IBM Plex Sans Condensed locally (OFL). Use macOS sidebar
vibrancy and an inset-traffic-light transparent title bar via Tauri's
current window APIs, verified in its docs. Check every screen against
its mockup in both themes.
```

### Prompt 13.2: Accessibility and keyboard

> **RUN WITH** · Desktop · **Sonnet 5.5 · medium** · **Accept edits** · agents: `test-runner`

```text
Audit every screen for WCAG 2.2 AA: contrast in both themes, focus order
and visible focus, VoiceOver names for every control and status, status
never by color alone, and target sizes. Implement every shortcut in the
design appendix and confirm the full workflow works without a mouse.
Add automated accessibility checks (axe) to the component tests so
regressions fail pnpm verify.
```

> **INSPECTION: pass before Stage 14**
>
> - Zero hard-coded design values (enforced by lint)
> - Zero axe violations; the full workflow is keyboard-only capable; VoiceOver reads every status
> - Every screen matches its mockup in both themes

Run the Stage Gate.

---

## Stage 14: Inspections (fresh sessions, report only)

### Prompt 14.1: Final security review

> **RUN WITH** · CLI, fresh session · **Opus 5.5 · high** · **Plan** · agents: `security-reviewer`
>
> `claude --model opus --effort high --permission-mode plan --name fm-foreman-s14-security`

```text
Use the security-reviewer agent as an external auditor who has never
seen this code. Audit Foreman against docs/threat-model.md and
docs/requirements.md: process spawning, capabilities, CSP, file scoping
and symlinks, hostile repo content in every view, overlay tampering,
the run log's contents, secrets exclusion, and dependencies (cargo
audit, pnpm audit). Run the scanners fresh. Report each finding with
severity, evidence, and fix. Don't fix anything.
```

### Prompt 14.2: Performance

> **RUN WITH** · CLI, fresh session · **Opus 5.5 · high** · **Plan** → **Accept edits** (writes benchmarks only) · agents: `sre-reviewer`, `test-runner`
>
> `claude --model opus --effort high --permission-mode plan --name fm-foreman-s14-perf`

```text
Write and run benchmarks against the NFRs in docs/MASTER-SPEC.md: cold
start, opening a project with 200 tickets, prompt resolution, packet
detection latency, and memory with three terminal tabs. Use a release
build. Report every miss with the cause, the fix, and the expected gain.
```

### Prompt 14.3: Architecture audit

> **RUN WITH** · CLI, fresh session · **Opus 5.5 · xhigh** · **Plan** · agents: `design-reviewer`
>
> `claude --model opus --effort xhigh --permission-mode plan --name fm-foreman-s14-arch`

```text
Use the design-reviewer agent as an independent principal architect
reviewing Foreman for a 20-year life. Compare the code with
docs/design-doc.md and every ADR. Find drift, hidden coupling, places
where a future blueprint change would break the parser, data that can't
be migrated, dependencies with short support horizons, and anything
painful to change in 5 years. Prioritize by risk. Report only.
```

Fix every CRITICAL and HIGH finding with the **Crew Loop**, then re-run the audit that found it.

> **INSPECTION: pass before Stage 15**
>
> - Zero open CRITICAL or HIGH findings across the three audits
> - Every NFR is met in the release build
> - Every drift is fixed or recorded in a superseding ADR

Run the Stage Gate.

---

## Stage 15: Paint & Trim

### Prompt 15.1: First-run setup, copy, and README

> **RUN WITH** · Desktop · **Sonnet 5.5 · medium** · **Accept edits** · agents: `code-reviewer`

```text
Read docs/handoff/stage-14.md and the writing rules in the design
appendix. Implement the first-run setup: choose the blueprint repo,
enter your DRI name, run Doctor, and create or open the first project.
Review every string in the app against the writing rules: sentence case,
plain verbs, the same verb through each flow, errors that say what
happened and how to fix it, and empty states that point to the next
action. Write README.md: what Foreman is, setup, daily use, the
keyboard map, and where logs and data live.
```

> **INSPECTION: pass before Stage 16**
>
> - A fresh install reaches a ready first project through first-run setup alone
> - Every string passes the writing rules review

Run the Stage Gate. SKIPPED: marketing site, lifecycle emails, legal pages, multi-user onboarding — personal tool.

---

## Stage 16: Final Walkthrough

### Prompt 16.1: Readiness review and drills

> **RUN WITH** · CLI, fresh session · **Opus 5.5 · high** · **Plan** → **Manual** · agents: `sre-reviewer`, `security-reviewer`
>
> `claude --model opus --effort high --permission-mode plan --name fm-foreman-s16`

```text
Read docs/handoff/stage-15.md. Run a readiness review with the
sre-reviewer agent adapted to a local desktop app: dependencies, failure
and degraded modes (claude missing, logged out, too old; git missing;
blueprint repo moved; project folder deleted; disk full), data safety
(atomic writes, conflict handling, recovery from corrupt files), crash
recovery, upgrade path (schema migrations), and rollback
(Foreman-previous.app). Mark each DONE or MISSING with evidence.
Then walk me through three drills and record them in docs/drills.md:
1. Kill Foreman mid-write and confirm no file is corrupted
2. Corrupt .foreman/state.json and confirm a clear error and recovery
3. Install a broken release and roll back to Foreman-previous.app
```

**Dogfood (Ring 0):** run TradeLog through Foreman from Stage 1 through Stage 5. Every bug goes through the Crew Loop (BUG class), and every friction point becomes a lesson.

> **INSPECTION: pass before Stage 17**
>
> - Every readiness item is DONE; all three drills pass
> - TradeLog Stages 1–5 were run entirely through Foreman with zero manual prompt edits
> - Zero open CRITICAL bugs

Run the Stage Gate. SKIPPED: beta ring — single user.

---

## Stage 17: Certificate of Occupancy

### Prompt 17.1: Release

> **RUN WITH** · CLI · **Sonnet 5.5 · medium** · **Manual**
>
> `claude --model sonnet --effort medium --permission-mode default --name fm-foreman-s17`

```text
Read docs/handoff/stage-16.md. Prepare the v1.0.0 release: bump the
version, update CHANGELOG.md, run `pnpm release`, and walk me through
installing Foreman.app to /Applications (keeping the previous build as
Foreman-previous.app). Don't install anything without my confirmation.
```

> **CERTIFICATE OF OCCUPANCY: all must be true**
>
> - Foreman v1.0.0 is installed from a release build, with rollback proven
> - A real project (TradeLog) has run Stages 1–5 entirely through Foreman
> - Zero open CRITICAL/HIGH findings or bugs; `pnpm verify` green
> - Every ADR matches the code; all packets 1–16 signed; lessons recorded at every gate
> - It meets the success metrics in the App Brief: starting any step takes under 10 seconds, and no gate was skipped by accident

Write the final packet with Gate B as `docs/handoff/stage-17.md`, run Gate C, and sign it. SKIPPED: rollout rings 2–4 and live payments — personal tool.

---

## Stage 18: Maintenance

| Task | Run with |
|---|---|
| Bug | Desktop · Opus 5.5 · high · Plan → Accept edits · `debugger` |
| Stuck or intermittent problem | CLI · Opus 5.5 · xhigh (max if needed) · Plan → Auto · `debugger` |
| Monthly dependency updates | CLI · Sonnet 5.5 · medium · Accept edits · `test-runner`, `security-reviewer` |
| Blueprint changes | When the blueprint's grammar changes, update the golden fixture first; parser tests must fail before they pass |
| New feature | Change Request (docs/templates/change-request.md) → Stage 4 deltas → Crew Loop |

### Prompt 18.1: Blueprint retro (after CO)

> **RUN WITH** · CLI in your `claude-blueprint/` repo · **Opus 5.5 · xhigh** · **Plan** → **Accept edits** · agents: `design-reviewer`
>
> `claude --model opus --effort xhigh --permission-mode plan --name blueprint-retro-foreman`

```text
Here are the lessons from building Foreman: [path to foreman/docs/lessons.md].
Group them by root cause and stage, and find the patterns. For each
pattern, propose a concrete edit to claude-build-blueprint.md (old text
→ new text). Have the design-reviewer agent challenge each edit. Apply
the survivors after I approve, bump the version per the CHANGELOG rules,
add a CHANGELOG entry linking each change to its lesson IDs, and
re-render the PDF with render.py. Foreman's golden tests must then be
updated to the new blueprint version in a separate Foreman ticket.
```
