# engine-assist

**Foreman**: a personal macOS desktop app that runs the Claude Build Blueprint, stage by stage, from first idea to Certificate of Occupancy. It fills in every prompt and handoff packet and launches Claude Code with the exact model, effort, permission mode, and agents for each step.

## Status

Pre-Construction. Stage 1 (the App Brief) is written and waiting for sign-off.

| Stage | Status |
|---|---|
| 1 · Meet the Architect | Brief written: `docs/spec-input/00-app-brief.md`, awaiting sign-off |
| 2–18 | Not started |

## Where things are

| Path | Contents |
|---|---|
| `docs/spec-input/` | App Brief, draft Master Spec, design direction, the build prompts, and the blueprint v2.1 |
| `docs/handoff/` | Signed handoff packets, one per stage |
| `docs/lessons.md` | Lessons captured at every gate |
| `docs/templates/` | Handoff packet, Change Request, and postmortem templates |
| `.claude/agents/` | The eight global agents (Explore, test-runner, implementer, code-reviewer, debugger, design-reviewer, security-reviewer, sre-reviewer) |
| `spec/.claude/agents/` | The Stage 2 design council agents |

## How to run it

Follow `docs/spec-input/03-build-prompts.md` in order. Each prompt has a RUN WITH line with the surface, model, effort, mode, and agents to use.
