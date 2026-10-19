# Round 5b · Independent freeze review and resolution

Reviewer: design-reviewer agent (fresh context, did not write any council file) · Chair resolution: Opus 5.5 (xhigh) · Date: 2026-10-19

Before freezing, an independent reviewer checked `docs/MASTER-SPEC.md` (with Appendix A), `r5`, `r6`, `r7`, `r8` and the blueprint v2.2 proposal against the red team, the App Brief and the Claude Code CLI reference. **Verdict: READY AFTER FIXES** (3 blockers, 17 major, 20 minor). The reviewer confirmed every CLI flag the spec quotes (`--init-only`, `--restricted`, `--desktop --resume`, `--ax-screen-reader`, `--agents` file form, `--permission-prompts`, `manual`, `authMethod` values).

**All 40 findings are accepted and applied.** The edits are in `docs/MASTER-SPEC.md`, `r6-feature-specs.md`, `r8-nfr.md`, `spec/blueprint-v2.2/PROPOSAL.md`, `docs/requirements.md` and the Stage 2 packet. Where a fix changes what the App Brief promised, it is listed in Master Spec §17 for Billy to accept at the sign.

## Blockers

| ID | Finding | Resolution |
|---|---|---|
| B1 | P1 held 25 features (about 21 weeks at the upper bounds), contradicting a kill check at 6–8 weeks. | **P1 cut to the steps needed to start every TradeLog Stage 1–2 step through Foreman.** Close-out (gates, lessons, signing) moves to P2; in P1, Billy closes each stage by hand exactly as today, guided by a checklist step. Re-estimated from the r6 sizes: P1 is 7–13 weeks (typical to upper bound), so the kill check lands at about 8–14 weeks, after roughly half of the v1 work (it was 84%). Listed in §17. |
| B2 | Stage close-out sequence undefined (Stage 2 is a CLI stage without Gate C; F.2 has no chain). | Master Spec §8 now defines close-out per stage type: **chat stages (0, 1, 3, 4):** packet → attest items → F.2 lessons (same chat) → Sign. **Stage 2 (CLI council):** 2.6 writes the packet → attest items → F.2 as a followup in the 2.2 chain, writing to `docs/lessons.md` → Sign. **Stages 5–17:** Gate A → B → C → Sign. F.2 is one template step instantiated per stage with `chain=@stage` (the chain of the stage's packet-producing step). |
| B3 | v1 kept one unhardened `claude -p` call (packet re-output) whose safety probe didn't exist. | **Removed.** v1 makes no `claude -p` calls at all. Packet capture is paste-back (P1) and the sensor's last message (P2). SR-16 now states this. |

## Major

| ID | Finding | Resolution |
|---|---|---|
| M1 | Kill-criterion windows inconsistent; baseline not like-for-like; Foreman overhead outside the window. | One definition for both sides: **time-to-start** = from the moment Billy begins looking for the next step (opening the blueprint, or focusing Foreman) to the prompt being sent. Plus **per-stage active time** from first step started to packet signed, which counts Foreman's close-out overhead. The baseline is logged by surface (chat, CLI), starting with Foreman's own Stage 5 on Billy's Mac; the comparison is per surface. Limitation stated: no by-hand chat baseline exists until Billy logs one; the first chat steps he runs by hand (any project) are logged. |
| M2 | Brief says "under 10 seconds"; spec said median. | **p95 ≤ 10 s**, excluding time spent typing run inputs. NFR-09 updated. |
| M3 | `auto` items had no source in v1. | **`auto` items, verify commands and their linter move to v1.1** with headless Gate A. v1 items are `attest` and `time`. SR-11 and SR-12 are tagged v1.1. |
| M4 | Item text hashes had nowhere to live. | `blueprint.lock.json` gains `items: { <itemId>: { textSha256, kind } }`, written at pin. On open, each parsed bullet's hash is compared with it; a difference requires re-confirming the kind (stored in app data `approvals`). |
| M5 | Annotation paths: cwd-relative in one place, repo-relative in another; `../docs/...` unwritable. | **All annotation paths are repo-relative.** The resolver renders them relative to the step's cwd inside prompts. PROPOSAL B-1 and B-2 updated. |
| M6 | Examples used non-existent condition IDs; unknown IDs not validated. | Examples fixed (`payments`, `!public`). An unknown condition ID in `when` is a template error. |
| M7 | B-5's `[optional: …]` bracket would be an unknown token. | New sixth token class **optional part**: `[optional: …]` renders with an "Include" toggle (default on); turning it off removes the text and records it under SKIPPED. Conditional parts use `fm:if … fm:end`, which **moves from v1.1 to v1** (needed for v0.9 §5.3's part-of-step conditions). |
| M8 | Config fingerprint missed user and managed settings, parent `CLAUDE.md`, `CLAUDE.local.md`, skills, commands, rules. | Fingerprint covers: user settings (`~/.claude/settings.json` or `$CLAUDE_CONFIG_DIR/settings.json`), managed settings at their documented location, every `CLAUDE.md` from the cwd up to the repo root, `CLAUDE.local.md`, `.claude/settings*.json`, `.mcp.json`, `.claude/agents/**`, `.claude/skills/**`, `.claude/commands/**`, `.claude/rules/**`. Auth-changing keys are blocked wherever they appear, user settings included. The auth cache is keyed on the full fingerprint. |
| M9 | Path-policy denies applied only at the repo root. | Deny by **path component at any depth**: `.git`, `.claude`, `.husky`, `.vscode`, `.idea`, `.github`; file names `.mcp.json`, `CLAUDE.local.md`, `.envrc`, `.gitmodules`, `.gitattributes`, `.npmrc`. `CLAUDE.md` is writable only at the repo root, only through the build-package assembler, with an explicit "Claude Code loads this file in every session" confirmation. |
| M10 | `git commit -- <paths>` fails for new files. | `git add -- <paths>` then `git commit --only -m … -- <paths>`. Fixed in r6 and PROPOSAL B-3. |
| M11 | New-branch signing left signed files dirty on the current branch. | **Current-branch signing is the default** (Billy works on a branch per stage, so branch protection doesn't block it). "Sign on another branch" stays as an option and writes **only into that branch's commit** (blobs via `hash-object`, temporary index); the working tree is untouched and the stage shows "signed on <branch> (not merged)". |
| M12 | Contract probe could fail falsely (untrusted temp dir, undocumented unknown-option behavior). | Probes run in a fixed `<app-data>/probe/` directory that Billy trusts once during first run. Each probe has a 20 s timeout and classified outcomes: **pass**, **flag failure** (an "unknown option" style error), or **inconclusive** (timeout, trust prompt, anything else). Inconclusive keeps the last verified state and warns; it never trips a fail branch. S-1′ runs in exactly this setup. Added `--resume <random uuid>` parse probe ("no conversation found" = flag parsed). |
| M13 | After a fork, the new chain's UUID is unknown. | S-8 tests `--resume <uuid> --fork-session --session-id <new uuid>`. Pass: use it. Fail: identify the new transcript as the only `.jsonl` created in the project's transcript folder after launch; if that isn't unambiguous, start a fresh chain with context. |
| M14 | The frozen spec depends on an unapproved blueprint proposal. | The Stage 2 packet has a **BLUEPRINT DEPENDENCY** line: signing approves applying `spec/blueprint-v2.2/PROPOSAL.md`. Rejecting an item at the sign needs a note, and the chair revises the affected features before Stage 4. |
| M15 | Appendix A showed v1.1 surfaces as v1. | Each screen and component is tagged P1, P2 or v1.1; "Edit for this project" removed (v1.1, F-104); the searchable run log screen is specified in F-026 (P2); the P1 empty state opens the setup form; tab states aligned to six. |
| M16 | Palette fails WCAG AA in places; Night focus ring under 3:1. | Hex values marked **provisional**; the Night focus ring uses `--linework`; Stage 4.5 adjusts tokens with a computed contrast table that must pass before the design review. |
| M17 | App Brief items moved to v1.1 without being listed. | Master Spec §17 **Changes from the App Brief**, accepted or rejected by Billy at the sign. |

## Minor (all applied)

1. Version floor is 2.1.293 everywhere (r7, r8 aligned with R-19).
2. r8 references updated (S-0 in Stage 5.3; benchmark uses 19 stages and 200 run records, since tickets are v1.1).
3. F-026 P1 includes **Export metrics as Markdown**, so the kill check needs no P2 panel.
4. F-003's Stage 1–2 harness listed in P1.
5. Contract probes are P2; P1 checks the version on every launch only.
6. Grammar aligned: `session=new|continue`; modes include `manual`; `tag=optional` with `when=`: not applicable when false, skippable when true.
7. One run-record layout: `runs/<runId>/{run.json, output.log, settings.json}` plus a monthly index `runs/index-YYYY-MM.jsonl`.
8. Terminal keys: ⌘C, ⌘V, ⌘A and ⌘F are handled by the terminal (copy selection, paste, select all, find); ⌘⇧E is in P1 (no keyboard trap).
9. S-23 rewritten: Desktop shares the transcript → handed-off and fork rule; Desktop copies into its own session → no fork needed, the CLI chain is unaffected.
10. Sensor threat model narrowed: the token stops cross-run mix-ups and events from other users, not a forger running as Billy. No sensor event triggers an action by itself; re-arm always starts from Billy's click.
11. Every git call Foreman makes passes `-c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat -c diff.external= -c core.editor=true` (signing included, which also skips `prepare-commit-msg` and `post-commit`), with `GIT_TERMINAL_PROMPT=0` and a timeout.
12. Risk-register entry for v1.1: approved verify commands execute repository code.
13. Default tag table copied into Master Spec §5; part-of-step conditions use `fm:if` in v1.
14. CSP `style-src` added to S-0 with a decision rule (pass: strict; fail: `style-src 'unsafe-inline'` only, scripts stay strict, recorded as accepted risk in the ADR).
15. **Stage 1 packet signature:** it was filled in by the chair from Billy's chat approval, which breaks the "never sign a packet" rule and isn't in the `<name> on <date>` form. Billy must re-sign Stage 1 in the standard form. Flagged in the Stage 2 packet and the PR.
16. F-029 and F-030 have behavioral done criteria; the pass-through list can never override the always-removed variables.
17. F-018 names the `docs/templates/` contents (handoff packet, change request, postmortem, from the kit).
18. A wholly not-applicable stage's reason is written into the next signed packet's SKIPPED.
19. `criterion` 0.8.2 added to the pinned dev-dependencies (crates.io, 2026-10-19).
20. The "permission grants reset" notice is written into the terminal divider at every re-arm (P1).
