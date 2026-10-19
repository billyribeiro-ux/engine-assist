# Lessons — Foreman

Format for every entry:

## L-NNN · Stage [N] · <date> · <short title>
WHAT HAPPENED: what went wrong or was wasted, with evidence
CAUGHT BY: me | code-reviewer | security-reviewer | sre-reviewer |
           design-reviewer | CI gate | inspection | production
SHOULD HAVE BEEN CAUGHT BY: the earliest point it could have been caught
ROOT CAUSE: prompt | agent | CLAUDE.md | spec gap | model choice |
            effort choice | missing gate | external change
COST: rework time, extra sessions, usage, or user impact
FIX (proposed): the exact edit — file, section, old text → new text
FIX (applied): blueprint vX.Y | project only | rejected (why)
STATUS: open | applied | rejected

---

## L-001 · Stage 2 · 2026-10-19 · The draft spec stated unverified CLI and framework facts as settled
WHAT HAPPENED: Master Spec v0.9 (written in Stage 1) used a SvelteKit 2 config for SvelteKit 3, left TypeScript unpinned, ran headless gates in `plan` mode, launched without `--session-id`, and missed version floors. The engineering round found 11 such errors (r1-engineering E1–E11).
CAUGHT BY: inspection (Stage 2 council, principal-engineer)
SHOULD HAVE BEEN CAUGHT BY: Stage 1, by marking every technical claim in a draft spec as UNVERIFIED
ROOT CAUSE: spec gap
COST: one council round of corrections; no rework of code (none existed)
FIX (proposed): blueprint 1.4, add to the prompt: "Any version, flag, API or config you mention is a hypothesis: mark it UNVERIFIED. Stage 2 verifies it."
FIX (applied): open (proposal for blueprint v2.2 retro)
STATUS: open

## L-002 · Stage 2 · 2026-10-19 · An "unattended, read-only" run was neither
WHAT HAPPENED: The engineering round called the headless gate "read-only by construction". The red team showed that a `claude -p` run in a repo loads that repo's hooks, settings `env`, `apiKeyHelper`, `.mcp.json` servers, local allow rules and project agents without a trust prompt (R-01, fatal). Headless Gate A moved to v1.1 with a hardened argv (`--restricted`, `--strict-mcp-config`, `--tools` without Bash).
CAUGHT BY: inspection (Stage 2 council, red-team)
SHOULD HAVE BEEN CAUGHT BY: the principal-engineer round, by enumerating everything Claude Code loads from the working directory
ROOT CAUSE: agent
COST: one fatal objection; a v1 feature (headless Gate A) moved to v1.1
FIX (proposed): `principal-engineer` agent, add: "For any unattended or headless Claude Code run, list everything Claude Code loads from the working directory and user settings (hooks, settings env, helper commands, MCP servers, local allow rules, project agents) and verify each against the permissions and settings docs before calling the run contained."
FIX (applied): open (proposal for blueprint v2.2 retro)
STATUS: open

## L-003 · Stage 2 · 2026-10-19 · The first build order put the kill check at 84% of the build
WHAT HAPPENED: Round 2's build order placed the kill criterion after 16 of 18 v1 proposals; the red team computed 84% of the sized work (R-04). The plan was rebuilt around a thin end-to-end phase (re-estimated by the freeze review to put the kill check at about 8–14 weeks, after roughly half of the v1 work), plus a by-hand baseline.
CAUGHT BY: inspection (Stage 2 council, red-team)
SHOULD HAVE BEEN CAUGHT BY: Round 3 (chair synthesis)
ROOT CAUSE: prompt
COST: one round of re-planning
FIX (proposed): blueprint 2.2, ROUND 3 line, add: "Order v1 so the kill criterion can be measured as early as possible, and state what share of the v1 work is done at that point."
FIX (applied): open (proposal for blueprint v2.2 retro)
STATUS: open

## L-004 · Stage 2 · 2026-10-19 · Council output file names drifted from the blueprint
WHAT HAPPENED: The stack round was briefed to write `council/r6-stack.md`; the blueprint names it `r7-stack.md` (r6 is feature specs, r8 NFRs). Corrected mid-round.
CAUGHT BY: me (the chair)
SHOULD HAVE BEEN CAUGHT BY: the chair's round plan
ROOT CAUSE: prompt
COST: a file move; no rework
FIX (proposed): blueprint 2.2, add to the chair prompt: "Before Round 1, list the exact output file of every round and of prompts 2.3–2.5 (council/r1-…md to r8-nfr.md) and use only those names."
FIX (applied): open (proposal for blueprint v2.2 retro)
STATUS: open

## L-005 · Stage 2 · 2026-10-19 · Spikes couldn't run before the freeze
WHAT HAPPENED: The council wanted seven spikes before freezing, but this stage ran in a cloud session without a logged-in Claude Code on Billy's Mac. Each spike was given a decision rule with both outcomes fully specified, and scheduled for Stage 5.3, so the spec froze with zero open questions.
CAUGHT BY: me (the chair)
SHOULD HAVE BEEN CAUGHT BY: n/a (environment constraint)
ROOT CAUSE: external change
COST: residual risk carried into Stage 5 (bounded by the decision rules)
FIX (proposed): blueprint 2.6, item 7, add: "A spike that can't run before the freeze isn't an open question if the spec defines what ships for each outcome."
FIX (applied): project only
STATUS: applied

## L-006 · Stage 2 · 2026-10-19 · The chair's resolution didn't fully reach the frozen documents
WHAT HAPPENED: After Round 5, an independent freeze review found 3 blockers, 17 major and 20 minor issues: phase 1 still held about 21 weeks of work while promising a kill check at 6–8 weeks; Stage 2's own close-out (no Gate C) was undefined; one unhardened `claude -p` call remained; `auto` gate items had no source in v1; and the Brief's v1 cuts weren't listed for approval. All were fixed (r5b).
CAUGHT BY: design-reviewer
SHOULD HAVE BEEN CAUGHT BY: the chair, while compiling the Master Spec (2.6)
ROOT CAUSE: missing gate
COST: one review round and a broad edit pass across six files before the freeze
FIX (proposed): blueprint 2.6, before "Then output the MASTER SPEC PACKET", add: "Have an independent reviewer who wrote none of the council files check that every red-team resolution is reflected in MASTER-SPEC.md, that phases and estimates add up from the feature sizes, and that every change from the App Brief is listed for my approval. Fix every BLOCKER and MAJOR before the packet."
FIX (applied): project only (r5b)
STATUS: applied

