# ADR-018: Run log as per-run JSON plus monthly JSONL indexes; metrics exported as Markdown; local redacted diagnostics

## Status
Proposed

## Date
2026-10-19

## Context

F-026 must produce the kill-check numbers at the end of P1 (M3): time to start a step and active time per step, median per surface, compared with `docs/metrics-baseline.md`. Requirements:

- **NFR-21:** 100,000 run entries without degrading.
- **SR-52:** argv stored with the prompt replaced by `<prompt sha256:…>`; env names only.
- **SR-53/PR-04:** tails 90 days, `0600`/`0700`.
- **SR-54:** diagnostic logs local, 10 MB × 5, no prompt, packet, terminal or env content.
- **PR-05:** Clear run history.
- **NFR-15:** a crash gives a local report and a run-log entry.

## Decision

1. **Per run** `runs/<runId>/run.json`, written via `store` (ADR-010) at launch (`running`) and at exit:
   ```json
   { "schemaVersion": 1, "runId": "…", "projectId": "…", "stage": 2, "stepId": "2.3", "surface": "cli",
     "chainId": "…", "argv": ["/Users/…/claude", "--resume", "…", "<prompt sha256:ab12…>"],
     "envNames": ["HOME", "PATH", …], "cwd": "spec", "promptSha256": "…", "runSettingsSha256": "…",
     "intended": { "model": "opus", "effort": "xhigh", "mode": "acceptEdits" }, "observed": null,
     "entryPoint": "do-next|palette|stage-view", "t": { "lookStart": …, "prepared": …, "confirmed": …, "spawned": …, "exited": … },
     "timeToStartMs": …, "runInputInterventions": 0, "copyPromptOnCli": false,
     "status": "running|exited|errored|interrupted", "exitCode": null, "claudeVersion": "2.1.303" }
   ```
   - Times are monotonic-derived milliseconds plus a wall-clock RFC 3339 start (`jiff` 0.2.38).
   - `lookStart` is when Foreman was focused or Do next was pressed (MASTER-SPEC §9 definition). The end point is the same as the by-hand baseline's (AC-KC-01): for CLI the spawn; for chat, the moment the filled prompt is on the clipboard and the target chat is open (when Copy and open returns).
2. **Index** `runs/index-YYYY-MM.jsonl`: one summary line per run, appended at exit with `O_APPEND` and `sync_data` (ADR-010). Readers skip a torn last line. The metrics strip on the Run log screen (`metrics_view`) and Export read only indexes (NFR-21: about 200 bytes × 100 k ≈ 20 MB across files; a month file is scanned linearly).
3. **Active time per step:** derived from the index (the step's `lookStart` → the step done: exit, packet saved or Mark done), minus waits recorded as intervals (`waiting-until`, Claude-working periods from run spans); compared as the median per surface (AC-KC-02).
4. **Export metrics** (P1): `core::runlog::export_metrics(index, baseline) -> String` produces Markdown tables per surface and per stage in `docs/metrics-baseline.md`'s layout (median, p95, n; manual-edit count; Copy-prompt usage), saved through a Rust-side save dialog (`WritePurpose::UserExport`, `.md` only).
5. **Output tails:** `runs/<runId>/output.log`, the last 2,000 lines (ADR-006), pruned after 90 days by a startup pruner, which also prunes templates unused for 90 days and events past retention.
6. **Diagnostics:**
   - `tracing` 0.1.44 + `tracing-subscriber` 0.3.23 to `logs/foreman.log`, through Foreman's own size-rotating writer (10 MB × 5, about 60 lines; `tracing-appender` rotates by time, not size). Files are `0600`.
   - A redaction `Layer` drops values of fields named `prompt`, `text`, `body`, `packet`, `output`, `env_value`, `clipboard`, `value` and replaces them with `‹redacted›`. A Rust panic hook writes the panic location (not payload values) to the log and the run log (SR-57). macOS crash reports remain the OS's own.
7. **Clear run history** deletes `runs/` and `events/` (PR-05). Removing a project deletes its app-data folder and approvals.

## Alternatives considered

- **SQLite (rusqlite)** for runs. Fast queries, but opaque to Billy, a binary dependency, and against PR-06's plain-files rule. JSONL handles 100 k entries comfortably.
- **One growing JSONL file.** Simpler, but unbounded growth and slower scans. Monthly files bound both.
- **`tracing-appender`** for log rotation. It rotates by time, not size, and SR-54 specifies 10 MB × 5.

## Consequences

- Prompt text is never stored, so the run log can't reproduce a prompt; the template, lock and inputs can (the resolver is deterministic).
- Metrics depend on Foreman's own timestamps. The by-hand baseline is logged by Billy with the same definitions.

## Verification

- `runlog::no_env_values`, `runlog::argv_prompt_redacted`, `runlog::record_before_spawn`, `runlog::index_torn_line_skipped`, `runlog::export_matches_baseline_layout` (golden).
- `retention::tail_pruned_after_90_days`, `privacy::clear_history`, `privacy::remove_project_leaves_repo`, `fs::app_data_permissions`.
- `logging::redaction` (snapshot of log lines from a full harness run contains no fixture prompt or packet strings).
- `criterion` `bench_metrics_100k`.

## Revisit when

- Run volume exceeds about 1 M entries, or the v1.1 dashboard (F-108) needs ad-hoc queries.

## Exit path

JSONL imports into SQLite or DuckDB trivially. A future store can be built from the existing files without loss.
