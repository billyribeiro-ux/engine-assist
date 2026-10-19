# Metrics baseline: running the blueprint by hand

This is the "by hand" side of Foreman's kill criterion (`docs/MASTER-SPEC.md` §9). Starting with Foreman's own Stage 5 on your Mac, log every step you run **without** Foreman. At the end of Foreman's phase 1, the same measures from Foreman's run log are compared against this file.

Both sides use the same two measures (Master Spec §9), compared per surface (chat, CLI):

1. **Time to start a step:** from the moment you begin looking for the next step to the prompt being sent.
2. **Active time per stage:** from the first step started to the packet signed, excluding waits (Claude working, the 7-day test).

## How to log a step

- **Decided at:** the moment you decide to run the step (you open the blueprint or the prompts file to find it).
- **Sent at:** the moment the prompt is sent (Enter in Claude Code, or Send in Claude chat).
- **Seconds to start:** sent minus decided.
- **Manual edits:** how many times you changed the prompt text by hand beyond filling a declared input (variables, pasted packets, paths, flags typed or retyped).
- **Mistakes:** anything run wrong: a wrong flag, model, effort or mode; a wrong or stale packet; a skipped step or gate item; a mid-session `/model`, `/effort` or Shift+Tab forgotten.

Use a stopwatch or your clock; whole seconds are enough.

## Log

| Date | Project | Stage.step | Surface | Decided at | Sent at | Seconds to start | Manual edits | Mistakes (describe) |
|---|---|---|---|---|---|---|---|---|
| | | | | | | | | |

## Stage log

| Date | Project | Stage | Surface(s) | First step started | Packet signed | Waits excluded (min) | Active minutes |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

## Summary (fill in at the end of Foreman's phase 1)

| Measure | Surface | By hand (this file) | Foreman (Export metrics) |
|---|---|---|---|
| Steps logged | chat · CLI | | |
| Median seconds to start | chat · CLI | | |
| p95 seconds to start | chat · CLI | | |
| Median active minutes per stage | chat · CLI | | |
| Manual edits per step | all | | |
| Mistakes per stage | all | | |

**Kill check:** if Foreman's median on either measure isn't lower than the by-hand median for the same surface, or any Foreman step needed a manual prompt edit, stop and simplify. Start with Foreman's own Stage 5 (CLI); log the first chat steps you run by hand in any project for the chat baseline.
