# CR-008b: `metrics_view` compares to the by-hand baseline per surface

STATUS: proposed
REQUESTED BY: the M3-26 / M4-03 build agent (stage/05-metrics) · 2026-10-20
DECIDED BY: Billy Ribeiro · (pending)

## What changes
`BaselineComparison` in `docs/contracts/ipc.md` (`{ baselineMedianMs, foremanMedianMs }`, a single pair) is kept as built for now. As built it is the median time to start over every counted run on both surfaces, against the median of the by-hand "time to start" log in `docs/metrics-baseline.md`. The per-surface comparison, which the kill check needs (the chat baseline needs at least 10 steps; the CLI baseline is separate), lives only in the `metrics_export` Markdown.

This CR proposes, only if Billy wants the per-surface comparison on the Run log strip too: `baseline: Option<BaselineComparison>` becomes `baselines: Vec<SurfaceBaseline>` where `SurfaceBaseline { surface: Surface, baselineMedianMs: u32, foremanMedianMs: u32, steps: u32 }`. That changes a frozen type (ts-rs binding, mock, fixtures, `RunLog.svelte`).

## Why
The single pair mixes chat and CLI steps, whose by-hand times differ a lot. It is a fair headline number but not the kill-check number. The export already holds the per-surface figures, so nothing is lost; the question is only whether the strip should show them.

## Impact
- Scope / feature catalog (F-IDs): F-026, AC-F026-08, AC-SM-04.
- ADRs (new, amended, superseded): none.
- Threat model: none.
- Data model and migrations: none.
- API contract and versioning: `MetricsView.baseline` changes type (ipc.md §2.4 and `metrics_view`); `pnpm bindings`, the mock and the fixtures follow.
- Pricing and entitlements: none.
- Timeline: under a day.
- Cost (build + run): none.

## Recommendation
REJECT for v1 unless the single pair proves misleading in use. The export carries the per-surface table, and the kill check is read from the export.

## Deltas for Stage 4 (filled in when approved)
- docs/scope.md: none
- docs/adr/: none
- docs/threat-model.md: none
- docs/migrations/: none
- docs/tickets.md: none
