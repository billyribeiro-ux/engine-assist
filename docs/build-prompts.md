# Foreman · Build prompts, Stages 5–18 (authoritative)

This file replaces Stages 5–18 of `docs/spec-input/03-build-prompts.md`. That file predates the frozen spec and still mentions things that no longer apply, including a headless gate in v1 and a different milestone plan. Stages 1–4 were run from it and are kept as history.

Everything here runs on Billy's Mac, in `~/Code/engine-assist` or wherever the repo is cloned, on the **Max plan**, with **no Fable models**. Opus 5.5 at xhigh takes every role the blueprint gives Fable, including the STUCK / VAGUE route.

**Before you start**
- Merge PRs 1 to 3 after signing their packets (Stage 1 re-signed, then 2, 3, 4).
- `git pull` on `main`.
- Install Claude Code ≥ 2.1.293 from the `latest` channel: `claude install latest`, then `claude -v`.
- Check `claude auth status` shows `"authMethod": "claude.ai"`.

**Branch per stage.** Each stage starts with `git switch -c stage/NN-<name>` from `main`. Each stage ends with its packet signed, a PR, and a merge.

**Every stage from 5 to 17 ends with the Stage Gate:** Gate A (inspection), Gate B (packet), Gate C (lessons), then you sign. The gate prompts are at the end of this file.

**Inputs every session reads first:** `CLAUDE.md`, the latest `docs/handoff/stage-NN.md`, `docs/lessons.md`.

**The plan:**
- `docs/scope.md` (milestones M0–M5, exit criteria EX-*, acceptance criteria AC-*);
- `docs/tickets.md` (160 tickets);
- `docs/design-doc.md` and `docs/adr/`;
- `docs/threat-model.md`;
- `docs/contracts/`;
- `docs/domain-rules.md`;
- `docs/design/`.

---

## Before Stage 6: apply blueprint v2.2 in `claude-blueprint`

Milestone M1 needs the annotated blueprint (scope M1 entry criterion). You approved the changes when you signed the Stage 2 packet.

> **RUN WITH** · CLI in your `claude-blueprint/` repo · **Opus 5.5 · high** · **Plan** → **Accept edits**
>
> `claude --model opus --effort high --permission-mode plan --name blueprint-v2.2`

```text
Apply the blueprint changes in
~/Code/engine-assist/spec/blueprint-v2.2/PROPOSAL.md to
claude-build-blueprint.md, exactly as written: B-2, B-3, B-4, B-5, B-6a,
B-8 now, and the B-1 grammar header plus the fm:step / fm:verify
annotations for Stages 1–2, the Stage Gate and F.2 (Stages 0, 3, 4, 5 come
in Foreman's M4). Use the annotation forms in
~/Code/engine-assist/docs/domain-rules.md, which fix the open points in
the proposal (see docs/review/stage4-carried-findings.md items 1–7).
Re-render the PDF, add the CHANGELOG entry from the proposal, and show me
the full diff before committing. Commit as "blueprint: v2.2".
```

---

## Stage 5: Site Prep & Excavation (milestone M0)

Branch: `stage/05-site-prep`. Tickets: **M0-01 to M0-19**. Exit: AC-M0-01 to AC-M0-17.

**Log the by-hand baseline from the first step.** Every step in this stage is run by hand, so record each one in `docs/metrics-baseline.md` (ticket M0-19). This is the CLI baseline for the kill check.

### 5.1 Scaffold

> **RUN WITH** · CLI · **opusplan** · effort **medium** · **Plan** · agents: `test-runner`
>
> `claude --model opusplan --effort medium --permission-mode plan --name fm-foreman-s05-scaffold`

```text
Read docs/handoff/stage-04.md, CLAUDE.md, docs/design-doc.md, every ADR,
and tickets M0-01 to M0-06 in docs/tickets.md. Plan the scaffold exactly as
the ADRs specify: the Cargo workspace (foreman-core with no Tauri
dependency, foreman-sys, foreman-app, foreman-sensor), the Tauri 2.12.4
app, the SvelteKit 3.1.0 SPA (adapter-static 4.0.0, fallback index.html,
ssr = false, #lib, config in vite.config.ts), the typed IPC layer with
ts-rs and its mock implementation, the fake `claude` test binary, and the
lint/format/type-check baseline with the security lint rules. Verify every
version against npm and crates.io today (they must be the latest stable
unless an ADR pins otherwise: TypeScript stays 6.0.3). List every file
you'll create. Don't write files yet.
```

Approve the plan, then:

```text
Implement it. Then use the test-runner agent to run the full suite and fix
everything until ALL GREEN.
```

### 5.2 Quality gates, supply chain, CI, no-network

> **RUN WITH** · same session · `/model opus` · `/effort high` · Shift+Tab → **Plan** · agents: `security-reviewer`

```text
Implement tickets M0-06 to M0-11 from docs/tickets.md: the supply-chain
policy (SR-61, ADR-024), `pnpm verify` (AC-M0-03), CI workflows (AC-M0-05,
AC-M0-17), the Tauri security config and capability snapshots (SR-20 to
SR-23, AC-M0-08, AC-M0-09), the release build and rollback runbook
(AC-M0-11), and no-network enforcement (SR-60, AC-M0-10). Then prove the
gates: plant a type error, a fake secret, a clippy warning and a
known-vulnerable dependency, one at a time, show each fails `pnpm verify`,
and revert. Plan first; implement after I approve.
```

### 5.3 Spikes and ADR reality check

> **RUN WITH** · CLI, fresh session · **Opus 5.5 · xhigh** · **Plan** → **Accept edits**
>
> `claude --model opus --effort xhigh --permission-mode plan --name fm-foreman-s05-spikes`

```text
Run spikes M0-12 (S-0, first) through M0-18 from docs/tickets.md on this
Mac, with the real `claude` binary. Spike code lives only under
docs/spikes/<id>/ and is never imported by the app. For each spike, write
docs/spikes/<id>.md: what was run, the measured results, the branch taken
under its decision rule (MASTER-SPEC §9 and scope.md M0 table), and every
document that branch changes. Ask me before any step that needs me (S-0's
VoiceOver pass, trusting the probe folder). If S-0 fails fundamentally,
stop and tell me: the shell choice reopens.
```

Then, in the same session:

```text
Now re-check every ADR against the real code, the spike reports and
current docs. Report VALID, NEEDS AMENDMENT (what), or CONTRADICTED
(evidence) for each. Don't edit anything.
```

Amend every NEEDS AMENDMENT / CONTRADICTED ADR (Accept edits), then apply each spike's branch to the documents it names.

### 5.4 Claude Code automation for this repo

> **RUN WITH** · same session · `/model sonnet` · `/effort low` · **Accept edits**

```text
Set up this repo's .claude/: the project agents already exist in
.claude/agents/; add a PostToolUse hook that runs `pnpm verify:fast`
(typecheck + lint on changed files) after edits, and permissions that
allow `pnpm verify*`, `pnpm test*`, `cargo test*`, `cargo clippy*` and
read-only git commands without prompting. Never allow `git push`,
`rm -rf`, network tools or `bypassPermissions`. Show me the settings
before writing them.
```

### Stage 5 gate

Run the **Stage Gate** with these items:

> **INSPECTION: pass before Stage 6**
> - AC-M0-01 to AC-M0-17 pass (`pnpm verify` green; planted failures proven)
> - Every spike report exists with its branch applied (AC-M0-13, AC-M0-14)
> - No ADR left CONTRADICTED (AC-M0-16)
> - `docs/metrics-baseline.md` has every Stage 5 step logged (AC-M0-15)

---

## Stages 6–13: the Crew Loop

Stages 6–13 build ticket by ticket. For each ticket, in order, run the **Crew Loop**, routed by the ticket's class:

| Class | Model · Effort | Mode | Notes |
|---|---|---|---|
| SPEC-COMPLETE | Sonnet 5.5 · medium | Accept edits | Or the `implementer` agent shortcut |
| NEEDS-DESIGN | Opus 5.5 · medium | Plan → Accept edits | Only M4-28 |
| HARD | Opus 5.5 · xhigh | Plan → **Manual** | You review every diff. 108 of the 160 tickets are HARD |
| MECHANICAL | Haiku 5.5 · low | Accept edits | Batch several |
| BUG | Opus 5.5 · high | Plan → Accept edits | Or the `debugger` agent |
| STUCK / VAGUE | Opus 5.5 · xhigh | Plan | Replaces the blueprint's Fable route |

**Surface:** use the CLI with a worktree per ticket. This is Foreman's planned v1.1 loop, and it keeps Plan → Manual enforced by flags.

### Loop step 1: Plan

`claude -w <ticket-id> --model <from class> --effort <from class> --permission-mode plan --name fm-foreman-<ticket-id>`

```text
Ticket [ID] from docs/tickets.md. Read the ticket, every ADR, threat, AC
and DR it lists (docs/adr/, docs/threat-model.md, docs/scope.md,
docs/domain-rules.md, docs/contracts/), and the latest
docs/handoff/stage-NN.md. Use the Explore agent to find every file this
ticket touches. In plan mode, give me:
1. the implementation plan: an ordered file list with the change in each
2. the tests you'll write, mapped one-to-one to the acceptance criteria and
   the ticket's Required tests (use those exact test names)
3. any ambiguity or ADR conflict, as numbered questions
Do not write code yet.
```

### Loop step 2: Build, tests first

Same session. Shift+Tab to **Accept edits**, or to **Manual** for HARD tickets.

```text
Approved. Write the tests first and show me they fail. Then implement
until they pass. Then use the test-runner agent for `pnpm verify`. For UI
work, run `pnpm dev:web` and check the screen against its mockup in
docs/design/mockups/ in both themes, including empty, error, locked and
not-applicable states.
```

**SPEC-COMPLETE shortcut:** `claude -w <id> --model sonnet --effort medium --permission-mode acceptEdits`

```text
Use the implementer agent on ticket [ID] from docs/tickets.md. When it
reports back, run the test-runner agent, then the code-reviewer agent.
```

### Loop step 3: Review

Same session. `/model opus`, `/effort high`.

```text
Use the code-reviewer agent on this diff (and the security-reviewer agent
if the ticket lists a threat). Fix every CRITICAL and WARNING. List the
SUGGESTIONs and nits and I'll decide.
```

### Loop step 4: Commit and PR

Same session. `/model sonnet`, `/effort low`.

```text
Write a conventional commit message referencing [ID] and its ADRs. Open a
PR into stage/NN-<name> whose description lists each acceptance criterion
and required test and how it was verified.
```

### Loop step 5: Close out

When CI is green and you've merged, end the session. The worktree is removed.

---

## Stage 6: Foundation (M1)

Branch `stage/06-foundation`. Tickets **M1-01 to M1-33**, in order, through the Crew Loop. Entry: blueprint v2.2 applied (see "Before Stage 6").

> **INSPECTION: pass before Stage 7**
> - EX-M1-01 to EX-M1-06

## Stage 7: Framing (M2)

Branch `stage/07-framing`. Tickets **M2-01 to M2-28**. This stage contains the first process spawns, so the spawn, environment and argv controls land here (SR-01 to SR-05, SR-09, SR-16, SR-19, SR-24a, SR-57).

> **INSPECTION: pass before Stage 8**
> - EX-M2-01 to EX-M2-06

## Stages 8–9: Roofing and Plumbing (M3, the end of phase 1)

Branches `stage/08-roofing`, then `stage/09-plumbing`. Tickets **M3-01 to M3-08** in Stage 8 (security controls), **M3-09 to M3-31** in Stage 9 (runners and the kill check).

**Stage 8 gate**, run in a fresh session with `security-reviewer`:

> - Every threat in `docs/threat-model.md` links to a control and a passing test
> - Zero CRITICAL or HIGH findings (EX-M3-05)

**Stage 9 work: run TradeLog Stages 1–2 through Foreman.** Use the TradeLog blueprint run, starting every step from Foreman. Close each stage out by hand, following the close-out checklist step, and sign by hand. Then run **M3-31, the kill check**: Export metrics, compared with `docs/metrics-baseline.md`.

> **INSPECTION: pass before Stage 11**
> - EX-M3-01 to EX-M3-08
> - Kill check: AC-KC-01 to AC-KC-05, recorded in `docs/handoff/stage-09.md`. **If any fails, stop.** Write a Change Request with the simplification; no M4 ticket starts.

## Stage 10: Electrical

Not applicable: `payments = no`. Record it in the Stage 11 packet's SKIPPED line.

## Stages 11–12: HVAC and Drywall (M4)

Branches `stage/11-hvac`, then `stage/12-drywall`. Tickets **M4-01 to M4-35**.
- M4-28 is NEEDS-DESIGN: run Loop step 1 with Opus 5.5 · medium, and produce the missing mockup and domain rules first.
- From EX-M4-06 on, Foreman's own gates run inside Foreman (dogfood).

Then apply the remaining blueprint v2.2 annotations (Stages 0, 3, 4, 5) in `claude-blueprint`, using the same prompt as "Before Stage 6", and run **TradeLog Stages 3–5 through Foreman**.

> **INSPECTION: pass before Stage 13**
> - EX-M4-01 to EX-M4-06

## Stage 13: Siding & Exterior (M5, part 1)

Tickets **M5-01 to M5-07**: the sheet strip, title block, inspector and stamp, the design system applied, and accessibility. Run the Crew Loop with the `design-reviewer` agent in step 3.

> **INSPECTION:** EX-M5-01, EX-M5-02, AR-01 (axe-core zero violations, the keyboard-only script, and a manual VoiceOver pass by you)

## Stage 14: Inspections (fresh sessions, report only)

> **RUN WITH** · CLI, a **brand-new session** for each prompt · **Opus 5.5 · xhigh** · **Plan**

```text
14.1 Use the security-reviewer agent: re-verify every SR and PR in
docs/requirements.md against the code and tests, and run the abuse cases
in docs/threat-model.md by hand where a test can't (with a hostile fixture
repo). Report CRITICAL / HIGH / MEDIUM / LOW with evidence. Don't fix.
```

```text
14.2 Measure every NFR in spec/council/r8-nfr.md on the release build on
this Mac (NFR-01 to NFR-09, NFR-23 memory with 3 real Claude tabs, NFR-10
fault injection). Report each with the measured value and PASS/FAIL.
```

```text
14.3 Use the design-reviewer agent: architecture audit of the code against
docs/design-doc.md and every ADR. Anything that will hurt in 5 years?
Report only.
```

Tickets **M5-08 and M5-09** carry this stage's work. Fix every CRITICAL and HIGH through the Crew Loop (BUG route), then re-run the report that found it.

> **INSPECTION:** EX-M5-03

## Stage 15: Paint & Trim

Ticket **M5-10**: first-run polish, copy review against Master Spec A.9, and the README.

> **INSPECTION:** EX-M5-04

## Stage 16: Final Walkthrough

> **RUN WITH** · CLI, fresh session · **Opus 5.5 · xhigh** · **Plan** · agents: `sre-reviewer`

```text
Use the sre-reviewer agent for a readiness review of Foreman v1: every
degraded mode in scope.md EX-M5-05 (claude missing/logged out/too old, git
missing, blueprint repo moved, project folder deleted, disk full), crash
recovery, rollback. Then run the drills with me and record them in
docs/drills.md: kill Foreman mid-write, corrupt an app-data state file,
install a broken release and roll back to Foreman-previous.app.
```

Tickets **M5-11 and M5-12**. Then use Foreman daily on TradeLog for a week (dogfood) and read the success metrics.

> **INSPECTION:** EX-M5-05, EX-M5-06, EX-M5-07 (AC-SM-01 to AC-SM-04)

## Stage 17: Certificate of Occupancy

Tickets **M5-13 and M5-14**. Run `pnpm release`, install it, prove rollback, and tag `v1.0.0`.

> **INSPECTION:** EX-M5-08. Signing the Stage 17 packet is Foreman's Certificate of Occupancy.

## Stage 18: Maintenance

- **Blueprint retro** (blueprint F.5) in `claude-blueprint`, using every L-NNN entry in `docs/lessons.md`. This includes L-001 to L-006 from Stage 2, which propose blueprint edits.
- **Plan v1.1:** headless Gate A with `auto` items (after spike S-2′), the Crew Loop, the CR track and the rest, through a Change Request, with the v1.1 metric and kill criterion from Master Spec §9.

---

## The Stage Gate (Stages 5–17)

### Gate A: Inspection

> **RUN WITH** · CLI, **fresh session** · **Opus 5.5 · high** · **Plan** · agents: `test-runner` + the stage's reviewer agent
>
> `claude --model opus --effort high --permission-mode plan --name fm-foreman-sNN-gate`

```text
Run the Stage [N] inspection for Foreman. Read docs/handoff/stage-[N-1].md
for context. For each inspection item below, verify it with real evidence
(command output, test names, file paths) and mark PASS or FAIL. Use the
test-runner agent for every automated check. Don't fix anything; just
report.
INSPECTION ITEMS:
[the stage's items from this file]
```

### Gate B: Handoff packet

Same session. Shift+Tab → **Accept edits**, `/effort medium`.

```text
All items PASS. Write docs/handoff/stage-[N].md in exactly this format, and
don't commit it (I sign and commit it):

=== HANDOFF PACKET · Stage [N] → Stage [N+1] ===
PROJECT: Foreman
APPROVED BY: <blank>
COMPLETED: what this stage delivered (bullets, with file paths and ticket IDs)
EVIDENCE: each inspection item → PASS + its evidence
DECISIONS: any new or amended ADRs (IDs + one line each)
STATE: what now exists and works (commands, screens)
KNOWN LIMITS: deliberate deferrals, each with its ticket ID
SKIPPED: not-applicable stages or items, with reasons
OPEN ITEMS: none
NEXT: Stage [N+1] · <name> · <surface> · <model> · <effort> · <mode>
INPUTS FOR NEXT STAGE: the files, tickets and facts it must read first
```

### Gate C: Lessons

Same session.

```text
Before we close Stage [N], capture what this stage taught us. Review this
session, the stage's PRs and every reviewer finding. Append an entry to
docs/lessons.md in the format at its top: anything I had to fix or redo,
anything a reviewer or gate caught that the builder should have caught,
any prompt that produced weak output, any agent that missed something, and
anything that wasted usage, each with the exact proposed fix. If nothing
went wrong, write a "clean stage" entry.
```

**Sign:**
1. Fill in `APPROVED BY: Billy Ribeiro on <YYYY-MM-DD>`.
2. Commit only the packet and `docs/lessons.md` (`git add -- <paths>`, then `git commit --only -m "handoff: stage NN" -- <paths>`).
3. Push, open the stage PR into `main`, and merge.
