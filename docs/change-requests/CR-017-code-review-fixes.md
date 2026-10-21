# CR-017: Behavior the code review fixes changed (Desktop steps, tamper events, pending-change IDs)

STATUS: proposed
REQUESTED BY: Claude (code review fixes, branch stage/05-codefix) · 2026-10-20
DECIDED BY: <Billy> · <date>

## What changes
**A. Desktop steps progress like chat steps (code review W4).** `steps::cli()` counted
`surface=desktop` as CLI, so a Desktop step's Copy was refused and Mark done answered "a CLI step
finishes when its run exits", while `desktop_open` never spawns a run that could exit. The step
could never be done. As built, a Desktop step takes Copy (`desktop_open`: `copied`) and Mark done
(`done`), as a chat step does; it never takes `Spawn`. DR-065 lists the chat and CLI rows only.
Proposal: add to DR-065 "a `surface=desktop` step: `notStarted|copied|done → copied` (Open in
Desktop), `copied → done` (Mark done)". Test: `steps::transitions_exhaustive` (snapshot with the
Desktop rows) and `desktop_open::desktop_step_copied_then_marked_done`.

**B. `tamper:detected` once per tampered version (code review C1).** ipc.md §6 named `app::watch`
as the source; the code emitted it from every `project_open`, and the UI refetched the project on
it, so a tampered packet caused an endless refetch loop. As built, `project_open` emits it once per
(project, stage, working-file SHA-256) per app run; a new edit or tampering again after a restore
is announced again. The watcher's `packet:changed` makes the UI call `project_open`, which is how a
watcher-seen tamper is announced. The client's §6.2 revision rule now covers `packet:changed` and
`tamper:detected` too. Proposal: accept ipc.md §6 as amended.

**C. Pending-change IDs are derived (code review W1).** A `Read` open no longer writes
`progress.json`, so the ID of a pending outside change found by a read must be the same on every
read until the file changes. As built, `ChangeId` is derived from the file and its current bytes
(SHA-256, written in UUID v4 form); `is_uuid_v4` still holds. Proposal: accept the ipc.md §4.9
as-built note.

## Why
A: AC-F024-01's Desktop step otherwise stays `copied` forever and blocks the stage.
B: the loop spawned three git processes and an inbox sync per pass for as long as the packet stayed
tampered.
C: a read path must not write state (the lock discipline the review asked for), and Accept/Reject
must still find the change the UI showed.

## Impact
- Scope / feature catalog (F-IDs): F-024 (A), F-014 (B), F-022 (C).
- ADRs (new, amended, superseded): none.
- Threat model: none (no new write path; B removes a self-inflicted load).
- Data model and migrations: none (`progress.json` unchanged; a stored random change ID stays
  valid until its file changes).
- API contract and versioning: ipc.md §4.6 `launch_start`, §4.9, §4.14 `desktop_open`, §6, §6.2
  amended as built.
- Pricing and entitlements: none.
- Timeline: done on the branch.
- Cost (build + run): none.

## Recommendation
APPROVE A, B and C as built.

## Deltas for Stage 4 (filled in when approved)
- docs/domain-rules.md: DR-065 (A)
- docs/contracts/ipc.md: already amended, each place marked "CR-017"
- docs/tickets.md:
