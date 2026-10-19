# Blueprint v2.2 · Change proposal

Status: **PROPOSED**. Signing the Foreman Stage 2 packet approves applying it (its BLUEPRINT DEPENDENCY line). Rejecting any item at that sign needs a note, and the chair revises the affected Foreman features before Stage 4.
Applies to: `claude-build-blueprint.md` v2.1 (the copy in `docs/spec-input/` is the baseline used here)
Source: Stage 2 council for Foreman (`spec/council/r2-proposals.md` §5, decided in `r3-synthesis.md` §4)

Foreman never edits the blueprint. Billy applies these changes in the claude-blueprint repo, renders the PDF, and bumps `CHANGELOG.md`. Foreman's golden fixture then pins the v2.2 file.

Every change below makes the blueprint more precise. None changes the order of stages, a gate, or a model or effort choice. `fm:` annotations are HTML comments, so they don't render. The rendered PDF changes in two ways: the wording edits in B-2 to B-6 and B-8, and the prompts that get an `fm:if` part (3.2's legal part, 2.2's commercial-only council roles), which become two or three consecutive code boxes, because a block-level comment can't sit inside a fenced code block (Foreman domain rule DR-032). The prompt text itself is unchanged.

| ID | Change | Needed for |
|---|---|---|
| B-1 | `fm:` annotations on Stages 0–5, the Stage Gate, F.2, and Council setup | Foreman v1 |
| B-2 | Claude Code steps write packets to files | Foreman v1 |
| B-3 | Gate B writes the packet and doesn't commit | Foreman v1 |
| B-4 | Strict ticket format in 4.6 | Foreman v1 (parsed in v1.1) |
| B-5 | 1.4 is core; the press release and FAQ are an optional part | Foreman v1 |
| B-6a | One `APPROVED BY: <blank>` form in every packet | Foreman v1 |
| B-6b | `FILE: <path>` header for multi-file chat replies | Foreman v1.1 |
| B-7 | New prompt 18.3b: CR delta | Foreman v1.1 |
| B-8 | The spec folder lives inside the project repo as `spec/`; the repo is created at the end of Stage 1 | Foreman v1 |

---

## B-1 · `fm:` annotations

**Normative source.** The grammar and the reference annotations are specified exactly in Foreman's `docs/domain-rules.md`: **DR-020 to DR-038** (lexical form, version, keys and values, expressions, sessions and chains, `fm:verify`, item kinds, `fm:if`, optional parts, item IDs, unknown keys, actions, diagnostics, and DR-038's table of the reference annotation for every step of Stages 0–5, the Gate, F.2 and Council setup), with DR-005 to DR-008 for placement and step IDs. This section summarizes them for review; where the two differ, the domain rules win, and Foreman's golden test (`template::golden_v2_2`) pins the annotated file against DR-038.

### Grammar

Revised after the council's red team (`spec/council/r5-resolution.md`, R-05, R-06, R-26): **annotations are the only source** of every field Foreman acts on. Foreman still reads the prose, but only to warn when the prose and the annotation disagree. Every step Foreman runs must carry a full `fm:step`, and inspection items have explicit IDs.

The first line after the title is the grammar version:

```text
<!-- fm:grammar v=1 -->
```

Two more directives, each a block-level HTML comment on its own lines.

`fm:step` goes directly under a RUN WITH blockquote, with no blank line between them (DR-006). A step that has no RUN WITH (Council setup, the 7-day wait after 1.3, the manual "record your final choice" after 2.4, 5.1's second prompt, the 1.4 handoff copy) gets a standalone `fm:step` on its own lines directly after the paragraph or HANDOFF blockquote that describes it, with a letter-suffixed ID of the step it follows: `1.3b`, `1.4b`, `2.4b`, `5.1b` (DR-005, DR-007):

```text
<!-- fm:step id=<id> [kind=prompt|followup|action|manual|wait|repeatable]
     [tag=core|conditional|optional] [when=<expr>]
     [surface=chat|cli|desktop] [cwd=project|spec]
     [session=new|continue] [chain=<stepId>|@stage]
     [start=<model>@<effort>/<mode>] [switch=<trigger>:<model>@<effort>/<mode>]*
     [alt=<rule>:<model>@<effort>]* [in=<name>:<type>[:remember]]*
     [out=<path>[,<path>…]] [appends=<path>] [requires=<path>[,<path>…]] -->
```

`fm:verify` goes directly under an INSPECTION blockquote, one entry per item, in bullet order, each with an explicit item ID:

```text
<!-- fm:verify fresh-clone=attest ci-gates=attest staging=attest adr-check=attest -->
```

Rules:

- Values contain no spaces. Lists are comma-separated. **Every path is repo-relative** (never relative to the step's `cwd`), with no `..`; Foreman renders paths relative to the cwd when it puts them into a prompt.
- `when` is an expression over condition IDs, using `!`, `&`, `|`, and parentheses (for example `when=payments&!public`). Condition IDs: `idea_known`, `commercial`, `payments`, `multi_user`, `public`, `has_ui`, `others_data`, `server`, `has_database`, `has_uploads`, `has_email`, `has_integrations`; any other ID is an error.
- `<model>` is one of `fable`, `opus`, `sonnet`, `haiku`, `opusplan`. `<effort>` is one of `low`, `medium`, `high`, `xhigh`, `max`, `ultracode` for CLI and Desktop steps, and `adaptive` only for chat steps (the chat "effort adaptive" in the RUN WITH lines) (DR-023).
- `cwd` is `project` or `spec`; `blueprint` and `worktree` are reserved for v1.1 and are errors in v1.
- A `session=continue` step inherits its chain's `cwd`; declaring a different one is an error. `chain=@stage` (used by F.2) means the chain of the stage's packet-producing step.
- `<mode>` is one of `plan`, `default`, `manual` (alias of `default`), `acceptEdits`, `auto`, `dontAsk`. CLI and Desktop steps must have one; chat steps omit it. `bypassPermissions` is always an error.
- `tag=optional` with `when`: not applicable when false, skippable with a reason when true.
- `fm:verify` kinds: `attest` (Billy confirms with a note, or records "not applicable" with a reason), `time:<n>d` (a waiting period, then `>attest`). Reserved for Foreman v1.1: `auto:cmd=<verify-id>` (Foreman runs that approved command itself); Foreman v1 warns and treats it as `attest`. The number of entries must equal the number of bullets.
- Item IDs are stable and **unique across the whole template**, not just within a stage, because the lock keys items by ID alone (DR-034): a later stage's staging check is `staging-load`, not a second `staging`. Foreman stores a normalized hash of each bullet's text in the lock. If a bullet's text changes, Foreman asks to re-confirm its kind. A retro that reorders bullets keeps each ID with its bullet.
- Unknown directives and keys: within the declared grammar version, an error. When the file declares a newer grammar version than the Foreman reading it, a step whose `fm:step` has an unknown key becomes "needs a newer Foreman" (shown, not runnable, because Foreman can't know what the key changes); an unknown directive or key anywhere else is a warning, and Template health says "Update Foreman" (DR-021, DR-035).
- `kind=wait` steps have no duration key: Billy enters the date the wait ends when he starts the step, and Foreman notifies him on that date (DR-067).
- `kind=followup` is a prompt sent into the chain's **live** session: Foreman copies it and focuses the chain's terminal tab (or, if the session has ended, resumes it with `--resume` and the step's start). A followup can't change model, effort or mode; a step that does must be `kind=prompt`, which re-arms the session (DR-042, DR-068).
- `<!-- fm:if <expr> -->` … `<!-- fm:end -->` wraps a conditional part of a prompt (block-level, not nested). Foreman removes the part when the expression is false and records it under SKIPPED. An optional part is written inline as `[optional: …]`; Foreman shows it with an Include toggle.

### What to annotate

Every step Foreman runs in v1 gets a complete `fm:step`, and every inspection list in those stages gets an `fm:verify`. A step without one shows in Foreman as "not runnable: needs annotation". The prose is unchanged; Foreman uses it only to warn about disagreements (for example, the prose says "same session" and the annotation says `session=new`).

Required for v1 (about 40 comment lines):

| Location | What the annotations add |
|---|---|
| Stage 0 (0.1, 0.2) | Full `fm:step` on each; tag `optional` with `when=!idea_known` (an idea can arrive already chosen) |
| Stage 1 (1.1 to 1.4) | `in=X:text` and `in=changes:text` on 1.2 (where `[X]` and `[changes]` appear), `in=paste_numbers:multiline` on 1.4; the 7-day wait as `1.3b kind=wait` after 1.3; the handoff copy as `1.4b kind=action` (B-8); tags (B-5) |
| Stage 2 (2.1 to 2.6, Council setup) | `out=` for 2.1 and every council round; Council setup as `kind=action`; the manual "record your final choice" after 2.4 as `2.4b kind=manual`; 2.6 `out=spec/MASTER-SPEC.md,docs/handoff/stage-02.md` (B-2) |
| Stage 3 (3.1, 3.2) | `in=jurisdictions:text:remember`; tags (3.1 `conditional`, `when=commercial\|payments`); `fm:if commercial\|others_data\|multi_user` around 3.2's legal part |
| Stage 4 (4.1 to 4.8) | `session=new` on each; `out=` per document; tags (4.5 `conditional`, `when=has_ui`); late conditions asked at the 4.8 sign |
| Stage 5 (5.1 to 5.5, gate) | `start`/`switch` for the opusplan → Opus → Sonnet sequence; 5.1's second prompt as `5.1b kind=followup`; `fm:verify` on the gate |
| Stage Gate (A, B, C) | A and B `kind=prompt` (B continues A's chain with a new start, so it re-arms); C `kind=followup` in A's chain (same start as B, so it's sent into the live session), `appends=docs/lessons.md`; A's headless eligibility (v1.1) comes from `fm:verify` on each stage's inspection list |
| F.2 | `kind=followup`, `session=continue`, `chain=@stage`, `appends=docs/lessons.md` (in chat stages Foreman appends the pasted entries; in Stage 2 Claude writes them) |

The remaining stages (6–18) show as "needs annotation" in Foreman's Template health screen until a later v2.x change annotates them.

### Worked examples

Stage 2.2, the council (annotation under the existing RUN WITH):

```markdown
> **RUN WITH** · Claude Code CLI in `spec/` · Chair: **Fable 5.1** · effort **high** (Max plan) — or **Opus 5.5** · effort **xhigh** (Pro plan / to save usage) · **Plan** mode first, then **Accept edits** · agents: all six council agents
<!-- fm:step id=2.2 kind=prompt tag=core surface=cli cwd=spec session=new
     start=fable@high/plan switch=plan-approved:fable@high/acceptEdits alt=pro-plan:opus@xhigh
     out=spec/council/r1-market.md,spec/council/r1-users.md,spec/council/r2-features.md,spec/council/r2-growth.md,spec/council/r2-feasibility.md,spec/council/r3-synthesis.md,spec/council/r4-redteam.md,spec/council/r5-resolution.md -->
```

Stage 2.3, a continuing step. It's `kind=followup` because its start (Fable, high, Accept edits) equals 2.2's live start after its plan-approved switch, so Foreman sends it into the running session instead of re-arming it (DR-042, DR-068); 2.6, which switches to Manual mode, is `kind=prompt`:

```markdown
> **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits**
<!-- fm:step id=2.3 kind=followup tag=core surface=cli session=continue chain=2.2
     start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6-feature-specs.md -->
```

The Stage 5 gate:

```markdown
> **INSPECTION: pass before Stage 6**
>
> - A fresh clone, then `install` and `dev`, works with zero manual steps
> - CI is green, and an intentional type error, a planted fake secret, and a known-vulnerable dependency each fail it (prove the gates work, then revert)
> - Staging deploys automatically, and a rollback has actually been run once
> - The ADR reality check has no CONTRADICTED left open
<!-- fm:verify fresh-clone=attest ci-gates=attest staging=attest adr-check=attest -->
```

All four stay `attest` in the shared blueprint. A fresh clone, CI failures and a staging rollback can't be checked by a command run in place, and "no CONTRADICTED left open" is judged by reading. (Items a project adds through its overlay are `attest` too in v1; `auto` checks are v1.1.)

The full annotated text for Stages 0–5 is produced by Foreman Stage 6 and 11 tickets (M1-01, M4-04) from DR-038's reference annotations, because only Foreman's parser can prove each annotation loads cleanly. This proposal fixes the grammar and the coverage list.

**Harness fixture values.** The blueprint's Project Variables example for `[BUDGET_TIME]` is itself a placeholder (`[your numbers]`), which Foreman treats as unresolved (DR-083). The TradeLog acceptance fixture therefore uses a real value (for example `BUDGET_TIME = "$0 cash; 10 hours a week for 12 weeks"`); the blueprint text doesn't change.

---

## B-2 · Claude Code steps write packets to files

**Why:** A packet that exists only in terminal output can be lost, and Foreman can't watch for it. Every Claude Code step that ends a stage writes its packet to `docs/handoff/stage-NN.md`.

**2.6, old:**

```text
Then output the MASTER SPEC PACKET in one code block:
```

**2.6, new:**

```text
Then write the MASTER SPEC PACKET to ../docs/handoff/stage-02.md,
exactly in this format (do not commit it; I sign and commit it):
```

**2.6 packet `FILES:` line, old:** `FILES: MASTER-SPEC.md, council/r1…r8, 02-teardown.md`
**new:** `FILES: spec/MASTER-SPEC.md, spec/council/r1…r8, spec/02-teardown.md`

**2.6 trailing line, old:** `Upload \`MASTER-SPEC.md\` and the packet to the \`[APP_NAME]\` claude.ai Project.`
**new:** `Upload \`spec/MASTER-SPEC.md\` and \`docs/handoff/stage-02.md\` to the \`[APP_NAME]\` claude.ai Project.`

**Relay section, "Chat stages (0–4)" bullet, old:**

```text
- **Chat stages (0–4):** the packet comes out as a fenced block. Paste it into the next prompt, and also save it to the claude.ai Project's knowledge.
```

**new:**

```text
- **Chat stages (0, 1, 3, 4):** the packet comes out as a fenced block. Save it to `docs/handoff/stage-NN.md` in the project repo and to the claude.ai Project's knowledge, then paste it into the next prompt.
- **Stage 2** runs in Claude Code and writes its packet straight to `docs/handoff/stage-02.md`.
```

---

## B-3 · Gate B doesn't commit

**Why:** Gate B committed the packet, and then the signature needed a second commit. A broad commit at Gate B can also sweep in unrelated changes. The signing commit should be the only commit of a packet, and it should include only the packet.

**Gate prompt B, old (last line of the prompt):**

```text
Commit it with the message "handoff: stage [N]".
```

**new:**

```text
Do not commit it. I review it, sign it, and commit it myself.
```

**After Gate C, old:**

```text
You read it, add your name and date under APPROVED BY, and commit. **Then** the next stage starts.
```

**new:**

```text
You read it, fill in APPROVED BY with your name and the date, and commit only that file:
`git add -- docs/handoff/stage-NN.md docs/lessons.md` then `git commit --only -m "handoff: stage NN" -- docs/handoff/stage-NN.md docs/lessons.md` (plain `git commit -- <paths>` fails for a new, untracked packet). **Then** the next stage starts.
```

This by-hand commit is how every stage is signed until Foreman's Sign ships in its phase 2 (P2). From then on, for projects run through Foreman, Foreman's Sign makes the signing commit by git plumbing instead (it never runs `git add` or `git commit`); the blueprint text stays as above for anyone running it by hand.

---

## B-4 · Strict ticket format in 4.6

**Why:** A pipe-separated line is hard to parse when titles or criteria contain `|`. One heading per ticket with one labeled field per line is easy for people and parsers.

**4.6, old (from "For each ticket:" to the end of the field list):**

```text
Break every milestone into tickets sized for one Claude Code session
(1-4 hours of human work). For each ticket:
ID [M#-NN] | Title | Class: SPEC-COMPLETE / NEEDS-DESIGN / HARD /
MECHANICAL | Stage (6-13) | Depends on | ADRs | Threats mitigated |
Likely files | Acceptance criteria (Given/When/Then) | Required tests
(named) | Out of scope
```

**new:**

```text
Break every milestone into tickets sized for one Claude Code session
(1-4 hours of human work). Write each ticket in exactly this format,
one field per line, in this order, with nothing between tickets but a
blank line:

### [M#-NN] <Title>
Class: SPEC-COMPLETE | NEEDS-DESIGN | HARD | MECHANICAL
Stage: <6-13>
Depends on: <ticket IDs, comma-separated, or none>
ADRs: <ADR IDs, or none>
Threats mitigated: <threat IDs, or none>
Likely files: <paths, comma-separated>
Acceptance criteria:
- Given <…> When <…> Then <…>
Required tests:
- <test name>
Out of scope: <one line, or none>
```

(The class line lists the allowed values; each ticket uses exactly one.)

---

## B-5 · 1.4 is core; its press release and FAQ are an optional part

**Why:** The App Brief packet is a core handoff, so 1.4 can never be skipped. The press release and FAQ are valuable but not required; skipping them shouldn't skip the packet.

**1.4 prompt, old (first paragraph after the validation sentence):**

```text
Otherwise, write a press release
for launch day as if the product already exists (Amazon "working
backwards" style): headline, the problem, the solution, a customer
quote, and how to get started. Add an FAQ of the 8 hardest questions a
customer and an investor would ask, with answers.
```

**new:**

```text
Otherwise, [optional: write a press release for launch day as if the
product already exists (Amazon "working backwards" style): headline,
the problem, the solution, a customer quote, and how to get started.
Add an FAQ of the 8 hardest questions a customer and an investor would
ask, with answers.]
```

Plus the annotation `<!-- fm:step id=1.4 tag=core -->` under its RUN WITH. In the packet, `PRESS RELEASE + FAQ: <attached below the packet>` becomes `PRESS RELEASE + FAQ: <attached below the packet, or "skipped: <reason>">`.

---

## B-6 · Packet normalization

### B-6a · One APPROVED BY form

Every packet format already uses `APPROVED BY: <blank>` except Gate B.

**Gate prompt B, old:** `APPROVED BY: <leave blank for me to sign>`
**new:** `APPROVED BY: <blank>`

The relay example at the top keeps `[you, the DRI] on [date]`, because it shows a signed packet. Add one line under it:

```text
Every packet is written with `APPROVED BY: <blank>`. Only you fill it in, as `<name> on <YYYY-MM-DD>`.
```

### B-6b · `FILE:` headers (v1.1)

Add to the relay section:

```text
When a chat reply contains more than one file, start each file with a line `FILE: <path>` and nothing else on that line.
```

---

## B-7 · New prompt 18.3b: CR delta (v1.1)

Add after 18.3:

````markdown
### Prompt 18.3b: Apply an approved Change Request to a document

> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** mode, then **Accept edits**

```text
Read the approved Change Request docs/change-requests/CR-[NNN].md and
the document it amends: [path]. Produce the amended document with the
smallest change that satisfies the CR. Mark the document's revision
(Rev [n], CR-[NNN], <date>) in its header and list every changed
section. Do not change anything the CR doesn't require. Show me the
diff before writing.
```
````

---

## B-8 · The spec folder lives in the project repo

**Why:** One repo per project, from the start. The spec work in Stage 2 then has a stable location, the packets in Stages 1–4 have a place to live (B-2), and Foreman needs only two working directories per project: the repo root and `spec/`.

**1.4 handoff, old:**

```text
> **HANDOFF** → the App Brief + press release and FAQ. Save them to Project knowledge and copy them into a new folder `[app]-spec/01-app-brief.md`.
```

**new:**

```text
> **HANDOFF** → the App Brief + press release and FAQ. Create the project repo now (private, with `main` protected later in Stage 5). Save the packet as `docs/handoff/stage-01.md`, copy the brief and press release to `spec/01-app-brief.md`, and save both to Project knowledge.
```

**Every other `[app]-spec/` reference becomes `spec/`:**

| Line in v2.1 | Old | New |
|---|---|---|
| 2.1 | ``Save the result to `[app]-spec/02-teardown.md`.`` | ``Save the result to `spec/02-teardown.md`.`` |
| Council setup heading | ``agents in `[app]-spec/.claude/agents/` `` | ``agents in `spec/.claude/agents/` `` |
| 2.2 RUN WITH | ``Claude Code CLI in `[app]-spec/` `` | ``Claude Code CLI in `spec/` `` |

**4.8 handoff, old:**

```text
> **HANDOFF** → the Build Package. Create the repo with exactly those files, then save the packet itself as `docs/handoff/stage-04.md`.
```

**new:**

```text
> **HANDOFF** → the Build Package. Add exactly those files to the project repo (it has existed since Stage 1), then save the packet itself as `docs/handoff/stage-04.md`.
```

**4.8 packet `REPO FILES` list:** `docs/MASTER-SPEC.md` stays (copied from `spec/MASTER-SPEC.md` at the Stage 4 sign, so `spec/` holds the working history and `docs/` holds the frozen build input).

---

## CHANGELOG entry

```markdown
## 2.2 · <date>
- Foreman support: `fm:` annotations on Stages 0–5, the Stage Gate, F.2, and Council setup (HTML comments; the PDF shows prompts with a conditional part as consecutive code boxes, text unchanged).
- Every packet is a file in `docs/handoff/`, including Stage 2's. Gate B no longer commits; the signing commit is the only one.
- 4.6 tickets use one heading per ticket with labeled fields.
- 1.4 is core; its press release and FAQ are optional.
- One `APPROVED BY: <blank>` form everywhere.
- The project repo is created at the end of Stage 1 and holds the spec work in `spec/`.
```
