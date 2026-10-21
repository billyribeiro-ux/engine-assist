#!/usr/bin/env python3
"""M1-01: applies spec/blueprint-v2.2/PROPOSAL.md (B-1…B-8) and the domain-rules reference
annotations (DR-038) to the v2.1 blueprint. Reads docs/spec-input/claude-build-blueprint.md (never
edits it) and writes the annotated v2.2 text to the paths given on the command line.

Every edit is an exact, unique string replacement; the script fails loudly if any anchor is
missing or ambiguous, so a changed baseline can't be annotated silently wrong."""
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
src = (root / "docs/spec-input/claude-build-blueprint.md").read_text(encoding="utf-8")
text = src

def sub(old, new, count=1):
    global text
    n = text.count(old)
    if n != count:
        sys.exit(f"anchor found {n} times (expected {count}):\n{old[:200]}")
    text = text.replace(old, new)

def step_after(run_with_last_line, annotation):
    """Inserts an fm:step directly under the RUN WITH whose last line is given (DR-006)."""
    sub(run_with_last_line + "\n", run_with_last_line + "\n" + annotation + "\n")

# ---- grammar version and blueprint version (DR-021) ----
sub("# The Claude Build Blueprint\n\n**From", "# The Claude Build Blueprint\n<!-- fm:grammar v=1 -->\n\n**From")
sub("Version 2.1 · October 19, 2026 ·", "Version 2.2 · October 20, 2026 ·")

# ---- B-6a, B-2, B-6b: the relay section ----
sub("""NEXT:         Stage N+1 · surface · model · effort · mode
```

- **Chat stages (0–4):** the packet comes out as a fenced block. Paste it into the next prompt, and also save it to the claude.ai Project's knowledge.
""", """NEXT:         Stage N+1 · surface · model · effort · mode
```

Every packet is written with `APPROVED BY: <blank>`. Only you fill it in, as `<name> on <YYYY-MM-DD>`.

- **Chat stages (0, 1, 3, 4):** the packet comes out as a fenced block. Save it to `docs/handoff/stage-NN.md` in the project repo and to the claude.ai Project's knowledge, then paste it into the next prompt.
- **Stage 2** runs in Claude Code and writes its packet straight to `docs/handoff/stage-02.md`.
""")
sub("""- **Claude Code stages (5–18):** the packet is a file, `docs/handoff/stage-NN.md`. The next prompt tells Claude Code to read it. Files beat pasting in Claude Code: they're versioned, reviewable, and survive new sessions.
""", """- **Claude Code stages (5–18):** the packet is a file, `docs/handoff/stage-NN.md`. The next prompt tells Claude Code to read it. Files beat pasting in Claude Code: they're versioned, reviewable, and survive new sessions.

When a chat reply contains more than one file, start each file with a line `FILE: <path>` and nothing else on that line.
""")

# ---- The Stage Gate (B-1, B-3, B-6a) ----
sub("""> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** mode · agents: `test-runner` + the stage's reviewer agent
>
> `claude --model opus --effort high --permission-mode plan`
""", """> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** mode · agents: `test-runner` + the stage's reviewer agent
>
> `claude --model opus --effort high --permission-mode plan`
<!-- fm:step id=gate.A tag=core surface=cli cwd=project session=new start=opus@high/plan -->
""")
step_after("> **RUN WITH** · same session · **Opus 5.5** · effort **medium** · Shift+Tab → **Accept edits**",
           "<!-- fm:step id=gate.B tag=core session=continue chain=gate.A start=opus@medium/acceptEdits\n     out=docs/handoff/stage-NN.md -->")
sub("APPROVED BY: <leave blank for me to sign>\n", "APPROVED BY: <blank>\n")
sub('INPUTS FOR NEXT STAGE: the files, tickets, and facts it must read first\n\nCommit it with the message "handoff: stage [N]".\n```',
    'INPUTS FOR NEXT STAGE: the files, tickets, and facts it must read first\n\nDo not commit it. I review it, sign it, and commit it myself.\n```')
step_after("> **RUN WITH** · same session · **Opus 5.5** · effort **medium** · **Accept edits**",
           "<!-- fm:step id=gate.C kind=followup tag=core session=continue chain=gate.A start=opus@medium/acceptEdits\n     appends=docs/lessons.md -->")
sub("You read it, add your name and date under APPROVED BY, and commit. **Then** the next stage starts.\n",
    "You read it, fill in APPROVED BY with your name and the date, and commit only that file:\n"
    "`git add -- docs/handoff/stage-NN.md docs/lessons.md` then `git commit --only -m \"handoff: stage NN\" -- docs/handoff/stage-NN.md docs/lessons.md` "
    "(plain `git commit -- <paths>` fails for a new, untracked packet). **Then** the next stage starts.\n")

# ---- Stage 0 ----
step_after('> **RUN WITH** · Claude chat, new Project "Opportunity Research" · **Opus 5.5** · effort adaptive · **Research ON**',
           "<!-- fm:step id=0.1 tag=optional when=!idea_known surface=chat session=new start=opus@adaptive -->")
sub("""> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
Take the top 3.""", """> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive
<!-- fm:step id=0.2 tag=optional when=!idea_known session=continue chain=0.1 start=opus@adaptive
     out=docs/handoff/stage-00.md -->

```text
Take the top 3.""")
sub("""> - A validation experiment with a numeric pass threshold
""", """> - A validation experiment with a numeric pass threshold
<!-- fm:verify candidates=attest pick-on-evidence=attest experiment-threshold=attest -->
""")

# ---- Stage 1 ----
step_after("> **RUN WITH** · Claude chat, new Project `[APP_NAME]` · **Opus 5.5** · effort adaptive · web search ON",
           "<!-- fm:step id=1.1 tag=core surface=chat session=new start=opus@adaptive -->")
sub("""> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
We're going with concept""", """> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive
<!-- fm:step id=1.2 tag=core session=continue chain=1.1 start=opus@adaptive in=X:text in=changes:text -->

```text
We're going with concept""")
sub("""> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
Before anything gets built""", """> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive
<!-- fm:step id=1.3 tag=conditional when=commercial session=continue chain=1.1 start=opus@adaptive -->

```text
Before anything gets built""")
sub("Run the test yourself, then paste the numbers into the same chat.\n",
    "Run the test yourself, then paste the numbers into the same chat.\n<!-- fm:step id=1.3b kind=wait tag=conditional when=commercial -->\n")
sub("""> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
Here are the validation results""", """> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive
<!-- fm:step id=1.4 tag=core session=continue chain=1.1 start=opus@adaptive in=paste_numbers:multiline
     out=docs/handoff/stage-01.md -->

```text
Here are the validation results""")
# B-5: the optional part
sub("""threshold, tell me plainly and stop. Otherwise, write a press release
for launch day as if the product already exists (Amazon "working
backwards" style): headline, the problem, the solution, a customer
quote, and how to get started. Add an FAQ of the 8 hardest questions a
customer and an investor would ask, with answers.
""", """threshold, tell me plainly and stop. Otherwise, [optional: write a press release for launch day as if the
product already exists (Amazon "working backwards" style): headline,
the problem, the solution, a customer quote, and how to get started.
Add an FAQ of the 8 hardest questions a customer and an investor would
ask, with answers.]
""")
sub("PRESS RELEASE + FAQ: <attached below the packet>\n", 'PRESS RELEASE + FAQ: <attached below the packet, or "skipped: <reason>">\n')
sub("""> - The App Brief's OPEN ITEMS line is empty
""", """> - The App Brief's OPEN ITEMS line is empty
<!-- fm:verify validation-threshold=attest press-release-tested=attest brief-open-items=attest -->
""")
sub("""> **HANDOFF** → the App Brief + press release and FAQ. Save them to Project knowledge and copy them into a new folder `[app]-spec/01-app-brief.md`.
""", """> **HANDOFF** → the App Brief + press release and FAQ. Create the project repo now (private, with `main` protected later in Stage 5). Save the packet as `docs/handoff/stage-01.md`, copy the brief and press release to `spec/01-app-brief.md`, and save both to Project knowledge.
<!-- fm:step id=1.4b kind=action tag=core -->
""")

# ---- Stage 2 ----
step_after("> **RUN WITH** · Claude chat, `[APP_NAME]` Project · **Opus 5.5** · effort adaptive · **Research ON**",
           "<!-- fm:step id=2.1 tag=conditional when=commercial surface=chat session=new start=opus@adaptive\n     out=spec/02-teardown.md -->")
sub("Save the result to `[app]-spec/02-teardown.md`.\n", "Save the result to `spec/02-teardown.md`.\n")
sub("### Council setup (once): agents in `[app]-spec/.claude/agents/`\n", "### Council setup (once): agents in `spec/.claude/agents/`\n")
sub("Create these six files. Copy the folder for every future project.\n",
    "Create these six files. Copy the folder for every future project.\n<!-- fm:step id=council-setup kind=action tag=core -->\n")
sub("> **RUN WITH** · Claude Code CLI in `[app]-spec/` · Chair:", "> **RUN WITH** · Claude Code CLI in `spec/` · Chair:")
step_after("> Pro plan: `claude --model opus --effort xhigh --permission-mode plan`",
           "<!-- fm:step id=2.2 kind=prompt tag=core surface=cli cwd=spec session=new\n"
           "     start=fable@high/plan switch=plan-approved:fable@high/acceptEdits alt=pro-plan:opus@xhigh\n"
           "     out=spec/council/r1-market.md,spec/council/r1-users.md,spec/council/r2-features.md,spec/council/r2-growth.md,spec/council/r2-feasibility.md,spec/council/r3-synthesis.md,spec/council/r4-redteam.md,spec/council/r5-resolution.md -->")
# MASTER-SPEC §5: the commercial-only council roles become fm:if parts (DR-032, DR-038).
sub("""ROUND 1, Evidence (parallel):
 - market-analyst: fill the gaps in the teardown -> council/r1-market.md
 - user-advocate: deep pain mining + jobs-to-be-done -> council/r1-users.md
ROUND 2, Diverge (parallel, after reading Round 1):
 - product-visionary: the ultimate feature set -> council/r2-features.md
 - growth-strategist: revenue loops and tier fit -> council/r2-growth.md
 - principal-engineer:""", """ROUND 1, Evidence (parallel):
```
<!-- fm:if commercial -->
```text
 - market-analyst: fill the gaps in the teardown -> council/r1-market.md
```
<!-- fm:end -->
```text
 - user-advocate: deep pain mining + jobs-to-be-done -> council/r1-users.md
ROUND 2, Diverge (parallel, after reading Round 1):
 - product-visionary: the ultimate feature set -> council/r2-features.md
```
<!-- fm:if commercial -->
```text
 - growth-strategist: revenue loops and tier fit -> council/r2-growth.md
```
<!-- fm:end -->
```text
 - principal-engineer:""")
step_after("> **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits**",
           "<!-- fm:step id=2.3 kind=followup tag=core surface=cli session=continue chain=2.2\n"
           "     start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6-feature-specs.md -->")
step_after("> **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits** · agents: `principal-engineer`, `market-analyst`",
           "<!-- fm:step id=2.4 kind=followup tag=core session=continue chain=2.2 start=fable@high/acceptEdits\n"
           "     alt=pro-plan:opus@xhigh out=spec/council/r7-stack.md -->")
sub("Record your final choice and reason at the bottom of `council/r7-stack.md`. Overriding Claude is fine; the written reason is what the ADRs build on.\n",
    "Record your final choice and reason at the bottom of `council/r7-stack.md`. Overriding Claude is fine; the written reason is what the ADRs build on.\n"
    "<!-- fm:step id=2.4b kind=manual tag=core out=spec/council/r7-stack.md -->\n")
step_after("> **RUN WITH** · same session · same chair model · **Accept edits**",
           "<!-- fm:step id=2.5 kind=followup tag=core session=continue chain=2.2 start=fable@high/acceptEdits\n"
           "     alt=pro-plan:opus@xhigh out=spec/council/r8-nfr.md -->")
step_after("> **RUN WITH** · same session · same chair model · **Manual** mode (Shift+Tab)",
           "<!-- fm:step id=2.6 kind=prompt tag=core session=continue chain=2.2 start=fable@high/default\n"
           "     alt=pro-plan:opus@xhigh out=spec/MASTER-SPEC.md,docs/handoff/stage-02.md -->")
# B-2
sub("Then output the MASTER SPEC PACKET in one code block:\n",
    "Then write the MASTER SPEC PACKET to ../docs/handoff/stage-02.md,\nexactly in this format (do not commit it; I sign and commit it):\n")
sub("FILES: MASTER-SPEC.md, council/r1…r8, 02-teardown.md\n", "FILES: spec/MASTER-SPEC.md, spec/council/r1…r8, spec/02-teardown.md\n")
sub("Upload `MASTER-SPEC.md` and the packet to the `[APP_NAME]` claude.ai Project.\n",
    "Upload `spec/MASTER-SPEC.md` and `docs/handoff/stage-02.md` to the `[APP_NAME]` claude.ai Project.\n")
sub("""> - Open questions: zero. Status: FROZEN v1.0
""", """> - Open questions: zero. Status: FROZEN v1.0
<!-- fm:verify feature-matrix=attest beats-competitor=attest redteam-resolved=attest stack-choice=attest frozen=attest -->
""")

# ---- Stage 3 ----
step_after("> **RUN WITH** · Claude chat, `[APP_NAME]` Project · **Opus 5.5** · effort adaptive · web search ON",
           "<!-- fm:step id=3.1 tag=conditional when=commercial|payments surface=chat session=new start=opus@adaptive -->")
step_after("> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive · web search ON",
           "<!-- fm:step id=3.2 tag=core session=continue chain=3.1 start=opus@adaptive in=jurisdictions:text:remember\n"
           "     out=docs/handoff/stage-03.md -->")
sub("""   consent, and the processors we'll share data with
3. Legal and compliance for selling to [TARGET_USERS] in [jurisdictions]:
   privacy laws (e.g., GDPR, CCPA), terms of service, refunds and
   cancellations, sales tax/VAT, cookie consent, accessibility
   obligations, and anything domain-specific
For each item:""", """   consent, and the processors we'll share data with
```
<!-- fm:if commercial|others_data|multi_user -->
```text
3. Legal and compliance for selling to [TARGET_USERS] in [jurisdictions]:
   privacy laws (e.g., GDPR, CCPA), terms of service, refunds and
   cancellations, sales tax/VAT, cookie consent, accessibility
   obligations, and anything domain-specific
```
<!-- fm:end -->
```text
For each item:""")
sub("""> - Every security, privacy, and legal requirement is numbered and assigned to a stage
""", """> - Every security, privacy, and legal requirement is numbered and assigned to a stage
<!-- fm:verify margin-positive=attest payment-terms=attest requirements-numbered=attest -->
""")

# ---- Stage 4 ----
step_after("> **RUN WITH** · Claude chat, new conversation in the Project · **Opus 5.5** · effort adaptive",
           "<!-- fm:step id=4.1 tag=core surface=chat session=new start=opus@adaptive -->")
step_after("> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** (switch to **Fable 5.1** if the system is platform-scale or genuinely ambiguous) · effort adaptive · web search ON",
           "<!-- fm:step id=4.2 tag=core surface=chat session=new start=opus@adaptive -->")
for sid, first in [("4.3", "Inputs: the design doc, the ADRs, and the Stage 3 packet"),
                   ("4.4", "Inputs: the design doc, ADRs, threat model, and scope"),
                   ("4.5", "Inputs: the scope and Master Spec"),
                   ("4.6", "Inputs: scope, ADRs, threat model, data model, design")]:
    ann = {"4.5": "<!-- fm:step id=4.5 tag=conditional when=has_ui surface=chat session=new start=opus@adaptive in=list_screens:text -->"}.get(
        sid, f"<!-- fm:step id={sid} tag=core surface=chat session=new start=opus@adaptive -->")
    sub(f"> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive\n\n```text\n{first}",
        f"> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive\n{ann}\n\n```text\n{first}")
# B-4
sub("""Break every milestone into tickets sized for one Claude Code session
(1-4 hours of human work). For each ticket:
ID [M#-NN] | Title | Class: SPEC-COMPLETE / NEEDS-DESIGN / HARD /
MECHANICAL | Stage (6-13) | Depends on | ADRs | Threats mitigated |
Likely files | Acceptance criteria (Given/When/Then) | Required tests
(named) | Out of scope
""", """Break every milestone into tickets sized for one Claude Code session
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

(The class line lists the allowed values; each ticket uses exactly one.)
""")
step_after("> **RUN WITH** · Claude chat, a **brand-new conversation** (fresh context: it didn't write any of this) · **Fable 5.1** if available, otherwise **Opus 5.5** · effort adaptive",
           "<!-- fm:step id=4.7 tag=core surface=chat session=new start=fable@adaptive -->")
sub("""> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive

```text
Generate the project CLAUDE.md.""", """> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive
<!-- fm:step id=4.8 tag=core surface=chat session=new start=opus@adaptive out=docs/handoff/stage-04.md -->

```text
Generate the project CLAUDE.md.""")
sub("""> - Your test: you'd hand this package to a contractor you've never met
""", """> - Your test: you'd hand this package to a contractor you've never met
<!-- fm:verify review-clean=attest requirements-mapped=attest formulas=attest tickets-complete=attest contractor-test=attest -->
""")
sub("> **HANDOFF** → the Build Package. Create the repo with exactly those files, then save the packet itself as `docs/handoff/stage-04.md`.\n",
    "> **HANDOFF** → the Build Package. Add exactly those files to the project repo (it has existed since Stage 1), then save the packet itself as `docs/handoff/stage-04.md`.\n")

# ---- Stage 5 ----
sub("""> `claude --model opusplan --effort medium --permission-mode plan`
""", """> `claude --model opusplan --effort medium --permission-mode plan`
<!-- fm:step id=5.1 tag=core surface=cli cwd=project session=new start=opusplan@medium/plan
     switch=plan-approved:opusplan@medium/acceptEdits -->
""")
sub("Approve the plan: Opus hands off and Sonnet executes. Then:\n",
    "Approve the plan: Opus hands off and Sonnet executes. Then:\n<!-- fm:step id=5.1b kind=followup tag=core session=continue chain=5.1 -->\n")
step_after("> **RUN WITH** · same session · `/model opus` · `/effort high` · Shift+Tab → **Plan** · agents: `security-reviewer`",
           "<!-- fm:step id=5.2 tag=core session=continue chain=5.1 start=opus@high/plan\n     switch=plan-approved:opus@high/acceptEdits -->")
step_after("> **RUN WITH** · same session · **Opus 5.5** · effort **high** · **Plan**",
           "<!-- fm:step id=5.3 tag=core session=continue chain=5.1 start=opus@high/plan -->")
step_after("> **RUN WITH** · same session · `/model sonnet` · `/effort low` · **Accept edits**",
           "<!-- fm:step id=5.4 tag=core session=continue chain=5.1 start=sonnet@low/acceptEdits -->")
step_after("> **RUN WITH** · same session · `/model opus` · `/effort high` · **Plan** → **Accept edits**",
           "<!-- fm:step id=5.5 tag=conditional when=server session=continue chain=5.1 start=opus@high/plan\n     switch=plan-approved:opus@high/acceptEdits -->")
sub("""> - The ADR reality check has no CONTRADICTED left open
""", """> - The ADR reality check has no CONTRADICTED left open
<!-- fm:verify fresh-clone=attest ci-gates=attest staging=attest adr-check=attest -->
""")

# ---- Appendix F: F.2 ----
step_after("> **RUN WITH** · Claude chat, same conversation · **Opus 5.5** · effort adaptive",
           "<!-- fm:step id=F.2 kind=followup tag=core session=continue chain=@stage appends=docs/lessons.md -->")

for out in sys.argv[1:]:
    p = Path(out)
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
print(f"wrote {len(text.splitlines())} lines to {', '.join(sys.argv[1:])}")
