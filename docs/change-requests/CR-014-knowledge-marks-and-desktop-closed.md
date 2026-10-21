# CR-014: Knowledge-manifest marks in memory, and "Desktop is closed" through `desktop_handoff`

STATUS: proposed
REQUESTED BY: Claude (wave 4, branch stage/05-w4-m4) · 2026-10-20
DECIDED BY: <Billy> · <date>

## What changes
**A. Knowledge-manifest marks (M4-27).** `manifest_mark`'s "in the claude.ai Project" marks have
no home in the frozen files: `progress.json` v1 is `additionalProperties: false` (files.md §4.3)
and no other app-data file holds them. As built, they are kept in memory for the app session
(`Inner::knowledge`), like `screenReaderMode` before settings v2. Proposal: add
`knowledge: { [repoPath]: true }` to `progress.json` (schema v2 with a migration), so the marks
survive a restart.

**B. "Unless Billy confirms Desktop is closed" (AC-F010-15, DR-110, M4-29).** The contract has no
command for this confirmation. As built, `desktop_handoff` called on a chain that is already
`handedOff` spawns nothing and raises a native dialog ("Is Claude Desktop closed?"); OK sets the
chain back to `open` (`foreman_core::chain::desktop_closed`), so the next continuing step resumes
it without a fork; Cancel returns `Cancelled`. The session menu's "Open this session in Desktop"
is the only caller. Proposal: accept this as the contract (ipc.md §4.14 has the as-built note), or
add a separate command (`desktop_closed_confirm { chain }`).

## Why
A: AC-F023-01 only needs the checkbox; persisting it is a schema change, which needs a CR.
B: AC-F010-15 names the exception but no document gives it a path; reusing the handoff command
keeps the IPC surface (92 commands) unchanged and the decision native (SR-24a).

## Impact
- Scope / feature catalog (F-IDs): F-023, F-024, F-010.
- ADRs (new, amended, superseded): ADR-008 (Desktop handoff: the closed-Desktop confirmation).
- Threat model: none (the confirmation is native; no new write path).
- Data model and migrations: A needs `progress.json` v2 and a v1 → v2 migration.
- API contract and versioning: A none (same command); B either the as-built note or one command.
- Pricing and entitlements: none.
- Timeline: small.
- Cost (build + run): small.

## Recommendation
APPROVE B as built. APPROVE A as a follow-up ticket (schema v2), keeping the marks in memory
until then.

## Deltas for Stage 4 (filled in when approved)
- docs/scope.md:
- docs/adr/: ADR-008
- docs/threat-model.md:
- docs/migrations/: progress.json v2 (A)
- docs/tickets.md:
