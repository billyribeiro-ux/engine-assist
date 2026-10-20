# CR-011: v1 ships an opaque window; Reduce Transparency still reaches Rust and clears any window effect

STATUS: proposed
REQUESTED BY: Claude (stage/05-w3-ui, M5-04) · 2026-10-20
DECIDED BY: <Billy> · <date>

## What changes
Two sources describe the window differently:

- ADR-001, ADR-014 and MASTER-SPEC A.4 say the window is built with `windowEffects` (sidebar vibrancy) and that
  Rust turns it off when the webview reports Reduce Transparency or Increase Contrast (AC-XC-23).
- `docs/design/components.md` (Sidebar row) says a solid `--sidebar` stands in for vibrancy, and the
  built webview is opaque (no `transparent(true)`, which `Window::set_effects` requires).

By the truth order (ADR before design notes) the ADR wins, but vibrancy can't be built or looked at
on the Linux build host (it needs a transparent window and macOS materials), and a half-working
effect would be worse than none. Built instead: the webview reports both media queries
(`src/lib/os-display.ts`), `settings_update.osDisplay` reaches `Os::set_window_reduced_effects`, and
that clears the main window's effects (`set_effects(None)`) and records the flag
(`window::reduced_effects`). No effect is applied at build time yet, so nothing turns off visibly.

Proposed: keep v1 opaque (the solid `--sidebar` is the design), and amend ADR-014 and ADR-001 to
say vibrancy is a post-v1 option, with the reporting path kept so it can be added without an IPC
change. Or: approve adding `.transparent(true)` plus `.effects(...)` on macOS only, checked by
Billy on his Mac (Mac check 12 in `docs/progress.md`).

## Why
AC-XC-23 is met by what is testable here: the tokens (7:1), the stamp under Reduce Motion, and the
report to Rust. The visible effect needs a Mac.

## Impact
- Scope / feature catalog (F-IDs): F-030 (AC-F030-09, AC-XC-23) unchanged.
- ADRs: ADR-001 and ADR-014 amended (window effects deferred) if the first option is approved.
- Threat model: none.
- Data model and migrations: none.
- API contract and versioning: none (`osDisplay` stays).
- Pricing and entitlements: none.
- Timeline: none.
- Cost (build + run): none.

## Recommendation
MODIFY: keep the report and the clear, defer the effect until it can be tried on a Mac.

## Deltas for Stage 4 (filled in when approved)
- docs/scope.md:
- docs/adr/: ADR-001, ADR-014
- docs/threat-model.md:
- docs/migrations/:
- docs/tickets.md:
