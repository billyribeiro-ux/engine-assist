# ADR-009: Session sensor through per-launch hooks in exec form, writing token-tagged event files (P2)

## Status
Proposed

## Date
2026-10-19

## Context

F-011 (P2) lets Foreman know whether a session is working, idle or blocked, the live mode and effort, outstanding subagents and tasks, the last assistant message (for packet capture and gate import), and the observed model (provenance). Constraints:

- Foreman never edits `~/.claude/settings.json` (R-16, SR-10).
- Hooks use exec form (SR-15).
- A per-run token stops cross-run mix-ups but not a forger running as Billy, so **no event may trigger an action** (SR-18).
- Retention is 20 events per run, deleted at sign (SR-53).
- If S-1′ fails, there's no sensor and the P1 manual behavior stays (NFR-13).

Hooks docs spot-check (code.claude.com/docs/en/hooks, 2026-10-19):

- command hooks accept `args`: "When present, `command` is resolved as an executable and spawned directly with `args` as the argument vector, with no shell involved";
- `Stop` and `SubagentStop` inputs carry `last_assistant_message`;
- `SessionEnd` matcher values include `prompt_input_exit`;
- the events `SessionStart`, `Stop`, `StopFailure`, `Notification`, `PermissionRequest`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted` and `PostToolUse` exist.

**UNVERIFIED:** the exact JSON field name of the SessionEnd reason in the hook input (the page excerpt shows matcher values, not the input schema; tried WebFetch of the hooks page, first 100 k characters). The M4 contract probe records a real payload.

## Decision

- The run settings file (ADR-021, `--settings <app-data>/runs/<runId>/settings.json`) adds, per event above, `{"type":"command","command":"<app-data>/bin/foreman-sensor-<sha256-12>","args":["--run","<runId>","--token","<token>","--event","<EventName>"]}`. `PostToolUse` uses matcher `Bash`. It also sets `disableAllHooks: false`.
- `<token>`: the hex concatenation of two `uuid` 1.28.0 v4 values (244 random bits from the OS generator; no new crate), generated per run and stored only in `run.json` and the settings file.
- **`foreman-sensor`** (std + serde_json + rustix):
  1. read stdin up to 256 KB (SR-15); anything beyond is dropped and the event marked `truncated`;
  2. open `<app-data>/events/<runId>/` with `O_DIRECTORY|O_NOFOLLOW` (refuse if missing);
  3. write `{seq}-{event}.json` with `O_CREAT|O_EXCL|O_NOFOLLOW`, mode `0600`, containing `{token, event, receivedAt, payload}`;
  4. print nothing; exit 0 within 10 ms; never block Claude Code.

  It doesn't check the token itself: it has nothing to compare against. Foreman does.
- At startup Foreman copies the bundled sensor into `bin/foreman-sensor-<hash>` (only if absent). **Before each run settings file is written**, it verifies the binary's SHA-256 against the build-time constant (SR-15); on a mismatch it re-copies once, then omits the hooks for that run and warns.
- **Ingest** (`foreman-app::sensor_ingest`): the watcher on `events/` reads new files and discards any whose token ≠ the run's token (logged). It derives `working | idle | blocked:{permission,input,plan-approval} | ended(reason)`, live `permission_mode` and `effort.level` (the mode guard: a warning only), outstanding subagents and tasks, `last_assistant_message`, and the observed model. Then it prunes to the last 20 files per run.
- **Events only update displayed state and enable buttons.** No code path from ingest calls launch, re-arm, interrupt, terminate, sign or write (enforced by a module-dependency test).
- All events for a stage's runs are deleted when the stage is signed, or 30 days after the run ends, whichever comes first (SR-53).

## Alternatives considered

- **A user-level hook in `~/.claude/settings.json`.** It runs in every session Billy starts and contradicts "never touched" (R-16, rejected).
- **A local socket or HTTP endpoint that hooks post to.** A listener (SR-60 spirit), plus a curl-like client in the hook, plus a daemon; one-file-per-event needs neither.
- **Parsing the TUI output or transcript files.** Fragile; the transcript is written asynchronously and lags the current turn (the hooks docs say to use `last_assistant_message` instead).
- **Hook `command` as a shell string.** Violates SR-15; quoting bugs and shell injection from paths.

## Consequences

- One file per event is cheap, crash-safe (`O_EXCL`), and needs no lock. The `seq` comes from a nanosecond timestamp plus pid, so ordering is by `seq`, then file mtime.
- A forger running as Billy can read the settings file and fake events. That's accepted within the trust boundary because events trigger nothing.
- The sensor adds one exec per hook event; Claude Code's own hook timeouts bound any stall.

## Verification

- S-1′: `claude --init-only` with the run settings in the trusted probe folder writes a `SessionStart` marker.
- `sensor::rejects_wrong_token`, `sensor::exec_form_only` (snapshot of run settings), `sensor::hash_checked_per_run`, `sensor::stdin_cap`, `sensor::events_only_update_state` (dependency test), `retention::events_deleted_on_sign`, `retention::events_max_age`, `sensor::exits_under_10ms` (criterion on the binary).
- Derived-state unit tests from recorded payload fixtures.

## Revisit when

- Claude Code changes the hook input schemas: contract probes flag it (ADR-017).
- Claude Code offers an official session-status API.

## Exit path

S-1′ fail, or a later probe failure: hooks are omitted from run settings, the sensor isn't installed, and every feature falls back to its P1 manual path (idle confirmation, paste-back capture, manual gate marking).
