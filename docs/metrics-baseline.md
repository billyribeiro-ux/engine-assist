# Metrics baseline: running the blueprint by hand

This is the "by hand" side of Foreman's kill criterion (`docs/MASTER-SPEC.md` §9). Starting with Foreman's own Stage 5 on your Mac, log every step you run **without** Foreman. At the end of Foreman's phase 1, the same measures from Foreman's run log are compared against this file.

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

## Summary (fill in at the end of Foreman's phase 1)

| Measure | By hand (this file) | Foreman (run log) |
|---|---|---|
| Steps logged | | |
| Median seconds to start | | |
| p90 seconds to start | | |
| Manual edits per step | | |
| Mistakes per stage | | |

**Kill check:** if Foreman's median seconds-to-start isn't lower than the by-hand median, or Foreman needed a manual prompt edit the by-hand run didn't, stop and simplify.
