# CR-008a: The run index keeps the latest line per run, and a chat step has a second line

STATUS: proposed
REQUESTED BY: the M3-26 build agent (stage/05-metrics) · 2026-10-20
DECIDED BY: Billy Ribeiro · (pending)

## What changes
`docs/contracts/files.md` §4.7 gains two sentences. Nothing else in the schema changes (still v1, no new field).

1. **The latest line for a `runId` wins.** A reader that finds more than one line with the same `runId` (in file order, months in order) uses the last one. Lines are still never rewritten.
2. **A chat step writes two lines.** Copy and open appends the line with `status: "copied"`, `timeToStartMs` set and `durationMs: null`. When the step is later marked done (or its packet is pasted back), the app appends a second line with the same `runId`, `status: "exited"` and `durationMs` set (copy to done). The chat clock for "time to start" ends at "prompt on the clipboard and chat open" (`docs/metrics-baseline.md`), so only the first line supplies `timeToStartMs`; the second repeats it.

Also stated, so the readers agree: `timeToStartMs` of an entry with `entryPoint` `rearm` or `repair` is excluded from the time-to-start statistics (they are not first launches), and "active time per step" sums `timeToStartMs + durationMs - waitMs` per (surface, project, step), counted only when some run of that step finished and none of its lines is `interrupted`.

## Why
§4.7 says one line "at run exit (or at Copy-and-open for chat)". For a chat step that gives no place for the duration the kill check needs (active time per step). The run record's state machine already rewrites `run.json` in place, so the index (append-only, FILE-R4) can only express an update as a new line. `foreman_core::runlog::merge_latest` implements the rule above; `metrics::runlog::*` and the `bench_metrics_100k` benchmark (about 169 ms for 100k lines against the 300 ms NFR-02 budget) were measured with it.

## Impact
- Scope / feature catalog (F-IDs): F-026 (AC-F026-08, AC-F026-09) read the same numbers; no AC changes.
- ADRs (new, amended, superseded): none (ADR-018's "append-only index" still holds).
- Threat model: none.
- Data model and migrations: none. No file is rewritten and `schemaVersion` stays 1.
- API contract and versioning: files.md §4.7 text only. ipc.md unchanged.
- Pricing and entitlements: none.
- Timeline: none; already built.
- Cost (build + run): a line is about 300 bytes, so the file grows by that for each finished chat step.

## Recommendation
APPROVE. The alternative (rewriting the first line) breaks the append-only rule, and a separate duration file adds a second source of truth.

## Deltas for Stage 4 (filled in when approved)
- docs/scope.md: none
- docs/adr/: none
- docs/threat-model.md: none
- docs/migrations/: none
- docs/tickets.md: none (M3-26 note)
- docs/contracts/files.md: §4.7, the two paragraphs above
