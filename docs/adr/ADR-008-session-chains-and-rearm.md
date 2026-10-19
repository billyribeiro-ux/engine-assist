# ADR-008: Session chains with Foreman-owned UUIDs; re-arm by graceful end and relaunch, user-initiated only

## Status
Proposed

## Date
2026-10-19

## Context

Many blueprint steps say "same session" but need a different model, effort or mode than the previous step. Typing `/model`, `/effort` and Shift+Tab into the TUI is fragile (r2 P-05, rejected). F-010 defines chains and re-arm. The constraints:

- `--session-id` only at chain start (SR-14);
- end the session with Ctrl-C then `/exit`, never signal a non-idle session without confirmation (SR-13);
- re-arm only from Billy's action (SR-18);
- broken chains when transcripts are cleaned up (R-09).

Spikes S-3 (effort on resume) and S-8 (graceful end, fork IDs, latency) decide branches already specified in MASTER-SPEC §9.

## Decision

- **Chain model** (in `progress.json`): `Chain { id: SessionId (UUID v4 from uuid 1.28.0), projectId, cwd, steps[], status: open | closed | superseded | handed-off | broken, forkedFrom? }`. `core::chain::plan_session(step, chains, new_id)` is pure.
  - `session=new` → `New { id }`, launched with `--session-id <id>`.
  - `session=continue` → `Continue { chain }`, launched with `--resume <id>` and no `--session-id`, unless fork (below).
  - Transcript missing (checked by the app in `<CLAUDE_CONFIG_DIR or ~/.claude>/projects/<project dir>/<id>.jsonl`) → `Broken` → "Start a fresh chain with context": a new session whose prompt is the step's prompt preceded by the last signed packet and the declared input paths.
  - Re-running a chain-start step → a new chain; the old one becomes `superseded`.
- **Re-arm** (`foreman-app::rearm`), a state machine with one entry point, the `rearm_start` command (user invoke only):
  1. *Idle.* P1: Billy clicks "Claude is idle and nothing is running in the background" (the button text is the assertion). P2: the sensor reports idle (last event `Stop`, no outstanding `SubagentStart`/`TaskCreated`, no running background Bash); otherwise "N background tasks still running: Wait / End anyway".
  2. *End.* Write `\x03`, wait 150 ms, write `/exit\r`. Wait ≤ 10 s for process exit (P1) or `SessionEnd` with reason `prompt_input_exit` (P2). On timeout: "Claude didn't exit. End it yourself, then press Continue." No signal is sent unless Billy then chooses Terminate (ADR-006) with its confirmation.
  3. *Relaunch* in the same tab through the normal launch pipeline (ADR-021): archive the buffer, write the divider `── <step> · <model> · <effort> · <mode> · permission grants reset ──`, and spawn with `--resume <id>` plus the full flags and `--settings`. A "Resuming 2.3…" state shows until the first output.
- **Effort on resume** (S-3): pass → `--effort`; fail → `--settings` with `effortLevel`; else show the `/effort <level>` line for Billy to type, warning that it saves as his default. Foreman never types it.
- **Desktop handoff** (P2, F-024): `claude --desktop --resume <id>` marks the chain `handed-off`. The next continuing step relaunches with `--fork-session`, unless Billy confirms Desktop is closed. The fork's ID (S-8): pass → Foreman passes `--session-id <new>` with `--resume <id> --fork-session` (the single allowed combination, guarded by a test); fail → the only new `.jsonl` in the transcript folder after launch; if that's ambiguous → a fresh chain with context. S-23 decides whether a handoff needs a fork at all.

## Alternatives considered

- **Typing `/model`, `/effort` and Shift+Tab into the live TUI.** Fails silently on timing and on TUI changes; rejected in r2 and r6 §6.
- **`claude --bg` + attach as the substrate.** Research preview, worktree default, undocumented flag support (r6 §6). Also, "resume" of a running background session attaches and refuses config flags (r1 §C).
- **SIGTERM/SIGHUP to end a session.** It can lose an unsent input box or kill background work; S-8 measures it, but the spec forbids it without idle confirmation.
- **Naming sessions only (`--name`, `--resume <name>`).** Ambiguous names open the session picker (r1 E5); UUIDs are owned by Foreman.

## Consequences

- Each re-arm resets Claude Code's per-session permission grants; this is written into the divider every time (r5b minor 20).
- The P1 idle check relies on Billy's assertion, so the button text must be precise and is covered by a copy test.
- Chains depend on Claude Code's transcript retention (`cleanupPeriodDays`); Doctor shows the effective value.

## Verification

- Fake-claude tests: `argv::session_id_only_on_chain_start_or_fork` (SR-14), `rearm::ctrl_c_then_exit_order`, `rearm::no_kill_without_idle_or_confirm`, `rearm::only_from_user_action` (no internal caller of `rearm_start`), `chain::superseded_on_rerun`, `chain::broken_when_transcript_missing`, `chain::fork_argv`.
- Proptest over random sequences of step runs: no argv has both `--session-id` and `--resume` except the fork form.
- S-3, S-8 and S-23 on Billy's Mac (Stage 5.3).

## Revisit when

- Claude Code gains a supported way to change model, effort or mode on a running session from outside (for example a control socket), or deprecates `/exit`.
- S-8 fails.

## Exit path

S-8 fail branch: re-arm becomes "end the session yourself, then press Continue". The relaunch half is unchanged. Chains are plain data, so dropping re-arm entirely degrades to "new session with context" per step.
