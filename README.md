# engine-assist

**Foreman**: a personal macOS desktop app that runs the Claude Build Blueprint, stage by stage, from first idea to Certificate of Occupancy. It fills in every prompt and handoff packet and launches Claude Code with the exact model, effort, permission mode, and agents for each step.

## Status

Pre-Construction is complete and awaiting signatures: Stages 1 (re-sign), 2, 3 and 4, in that order. Stage 5 onward runs on your Mac.

| Stage | Status |
|---|---|
| 1 · Meet the Architect | Signed (`docs/handoff/stage-01.md`); Billy to re-sign in standard form |
| 2 · Design Studio | `docs/MASTER-SPEC.md` FROZEN v1.0 on sign; packet `docs/handoff/stage-02.md` awaiting signature (PR from `stage/02-design-studio`) |
| 3 · Permits | Drafted on `stage/03-permits` |
| 4 · Construction Drawings | Build package complete on `stage/04-drawings`; packet `docs/handoff/stage-04.md` awaiting signature |
| 5–18 | Not started; run on your Mac with `docs/build-prompts.md` |

## Where things are

| Path | Contents |
|---|---|
| `docs/spec-input/` | App Brief, draft Master Spec, design direction, the build prompts, and the blueprint v2.1 |
| `docs/handoff/` | Signed handoff packets, one per stage |
| `docs/lessons.md` | Lessons captured at every gate |
| `docs/templates/` | Handoff packet, Change Request, and postmortem templates |
| `.claude/agents/` | The eight global agents (Explore, test-runner, implementer, code-reviewer, debugger, design-reviewer, security-reviewer, sre-reviewer) |
| `spec/.claude/agents/` | The Stage 2 design council agents |
| `spec/council/` | Every council round (r1–r8, r5b) |
| `spec/blueprint-v2.2/PROPOSAL.md` | Blueprint changes Foreman v1 needs, for you to apply in claude-blueprint |
| `docs/MASTER-SPEC.md` | The frozen product spec |
| `docs/metrics-baseline.md` | Your by-hand timing log for the kill criterion |

## How to run it

Follow `docs/spec-input/03-build-prompts.md` in order. Each prompt has a RUN WITH line with the surface, model, effort, mode, and agents to use.
