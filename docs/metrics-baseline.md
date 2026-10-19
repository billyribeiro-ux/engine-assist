# Metrics baseline: running the blueprint by hand

This is the "by hand" side of Foreman's kill criterion (`docs/MASTER-SPEC.md` §9). Starting with Foreman's own Stage 5 on your Mac, log every step you run **without** Foreman. At the end of Foreman's phase 1, the same measures from Foreman's run log are compared against this file.

Both sides use the same two measures (Master Spec §9), compared per surface (chat, CLI):

1. **Time to start a step:** from the moment you begin looking for the next step to the step being ready to run. The end point is the same for Foreman and by hand (AC-KC-01):
   - **CLI:** the command is started (Enter on the typed `claude …` command; in Foreman, the spawn).
   - **Chat:** the filled prompt is on the clipboard **and** the target chat is open (by hand: you've copied the edited prompt and the right conversation or Project is in front; in Foreman, Copy and open has returned). Pressing Send isn't part of it, on either side.
2. **Active time per step:** from that same start point to the step done (packet saved, output pasted back, or the session exited), excluding waits (Claude working, the 7-day test). Compared as the median per surface (AC-KC-02).

## How to log a step

- **Decided at:** the moment you decide to run the step (you open the blueprint or the prompts file to find it).
- **Ready at:** CLI, the moment you press Enter on the `claude …` command; chat, the moment the filled prompt is on your clipboard and the right chat is open (before you paste and send).
- **Seconds to start:** ready minus decided.
- **Done at / waits:** when the step is done (packet saved, output pasted back, session exited), and the minutes spent waiting on Claude or on a real-world wait inside the step.
- **Active minutes:** done minus decided, minus waits.
- **Manual edits:** how many times you changed the prompt text by hand beyond filling a declared input (variables, pasted packets, paths, flags typed or retyped).
- **Mistakes:** anything run wrong: a wrong flag, model, effort or mode; a wrong or stale packet; a skipped step or gate item; a mid-session `/model`, `/effort` or Shift+Tab forgotten.

Use a stopwatch or your clock; whole seconds are enough.

## Log

| Date | Project | Stage.step | Surface | Decided at | Ready at | Seconds to start | Done at | Waits (min) | Active minutes | Manual edits | Mistakes (describe) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | | | |

## Summary (fill in at the end of Foreman's phase 1)

| Measure | Surface | By hand (this file) | Foreman (Export metrics) |
|---|---|---|---|
| Steps logged | chat · CLI | | |
| Median seconds to start | chat · CLI | | |
| p95 seconds to start | chat · CLI | | |
| Median active minutes per step | chat · CLI | | |
| Manual edits per step | all | | |
| Mistakes per stage | all | | |

**Kill check:** if Foreman's median on either measure isn't lower than the by-hand median for the same surface, or any Foreman step needed a manual prompt edit, stop and simplify. Start with Foreman's own Stage 5 (CLI). For the chat baseline, log **at least 10 chat steps** you run by hand, in any project, between M0 and the kill check (ticket M0-19, AC-KC-04); with fewer, the chat surface can't pass.
