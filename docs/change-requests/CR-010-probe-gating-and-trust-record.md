# CR-010: Contract probes before a launch need the trust step and the sensor; opening the probe folder records the trust step

STATUS: proposed
REQUESTED BY: Claude (stage/05-w3-sensor, M3-18/M4-12) · 2026-10-20
DECIDED BY: <Billy> · <date>

## What changes
1. **Probes before a launch (AC-F008-21, AC-F019-15)** run only when `settings.json`
   `probeFolderTrusted` is set **and** this build has the session sensor. Otherwise the contract
   phase reports `skipped`, the launch goes on, and Doctor's `probeNote` says why ("Contract probes
   are off until you trust the probe folder" / "…need the session sensor, which this build doesn't
   include"). `probe_run` (Doctor's Run probes now) runs whenever Billy asks.
2. **`probe_folder_open` (AC-F019-08)** sets `probeFolderTrusted: true` when the tab opens. Foreman
   can't see Billy's answer to Claude Code's trust question without acting on a session event, which
   SR-18 forbids. A later probe that meets the trust prompt is inconclusive and Doctor says so; a
   passing probe A also sets it.
3. **Feature mapping of a flag failure (AC-F019-16):** `launch` and `chains` (`--session-id`, which
   every launch passes) block every launch; `re-arm` (`--resume`) blocks continuing steps; an
   `--effort` failure is a warning only (Claude Code 2.1.304 ignores an unknown effort, exits 0 and
   runs; blocking would stop every launch, not "only the dependent feature").

## Why
- Without the sensor, probe A can never see its `SessionStart` marker, so it is always inconclusive:
  probes would run (up to 3 × 20 s) before every launch and never verify anything.
- Without the trust step, Claude Code shows the trust prompt in the probe folder; the probes would
  again be inconclusive on every launch.
- The FirstRun screen already tells Billy "Skipping keeps contract probes off; Doctor reminds you",
  which is behavior 1.

## Impact
- Scope / feature catalog (F-IDs): F-019 (AC-F008-21, AC-F019-08, -15, -16 wording).
- ADRs (new, amended, superseded): ADR-017 §2 (when probes run; the trust record).
- Threat model: none (probes stay in the probe folder, `--init-only` only).
- Data model and migrations: none (`probeFolderTrusted` exists in settings v1).
- API contract and versioning: ipc.md `probe_folder_open`, `probe_run` as-built notes (already added).
- Pricing and entitlements: n/a.
- Timeline: none; built this way in `stage/05-w3-sensor`.
- Cost (build + run): none.

## Recommendation
APPROVE: the probes can't produce a verdict without the sensor or the trust step, and nothing else
changes for Billy.

## Deltas for Stage 4 (filled in when approved)
- docs/scope.md: AC-F008-21 and AC-F019-15 add "once the probe folder is trusted and the sensor is
  installed"; AC-F019-16 lists the feature mapping.
- docs/adr/: ADR-017 §2 amendment.
- docs/threat-model.md: —
- docs/migrations/: —
- docs/tickets.md: —
