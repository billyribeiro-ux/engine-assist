# ADR-020: UI primitives from bits-ui 2.19.5, styled with Foreman's own tokens

## Status
Proposed

## Date
2026-10-19

## Context

The UI needs dialogs and alert dialogs (sign, terminate, confirmations), menus, tabs (terminal drawer), tooltips (explain-on-hover), select and toggle groups (setup and tri-state conditions), scroll areas, and the ⌘K command palette (F-028 P2). It all must meet WCAG 2.2 AA with full keyboard operation and VoiceOver (NFR-31 to NFR-33), look native (Appendix A: our tokens, SF Pro), and work under the strict CSP (ADR-014).

Checked: `bits-ui` `latest` 2.19.5 (2026-10-03), peer `svelte ^5.33.0`, no `@sveltejs/kit` import in `dist` (r1), and it ships `command`, `dialog`, `alert-dialog`, `context-menu`, `dropdown-menu`, `tabs`, `tooltip`, `select`, `toggle-group` and `scroll-area`. Its peer `@internationalized/date` is pinned at 3.12.4 (r7).

## Decision

- **`bits-ui` 2.19.5** (exact) for headless primitives. Every component is wrapped once in `#lib/components/ui/` (for example `Dialog.svelte`, `Menu.svelte`, `Palette.svelte`) with Foreman's tokens and behavior defaults (focus ring, reduced motion), so screens never import bits-ui directly (ESLint `no-restricted-imports`).
- No CSS framework. Plain CSS with custom properties from Appendix A (tokens finalized in Stage 4.5 with the computed contrast table), scoped component styles, and `prefers-reduced-motion`, `prefers-contrast` and `prefers-reduced-transparency` media queries.
- Transitions use Svelte's built-ins, wrapped to respect reduced motion. S-0 checks their inline-style behavior under the CSP (ADR-014's fallback rule).
- Accessibility claims are the vendor's own, so proof comes from axe in Playwright (zero violations), keyboard-only scripts and the M5 VoiceOver pass.

## Alternatives considered

- **`@ark-ui/svelte` 5.24.3** (Zag state machines). Broad coverage and a close second, but less direct coverage for a Command palette, and it adds the Zag runtime.
- **`melt` 0.44.0** (builders). More control, but more assembly per component; last release 2026-01.
- **Hand-rolled primitives.** Full control, but accessible dialogs, menus and comboboxes are where hand-rolled code most often fails WCAG; not a good use of solo time.

## Consequences

- The wrapper layer isolates bits-ui API changes to one folder.
- bits-ui's runtime inline styles (positioning) depend on S-0's `style-src` outcome.

## Verification

Playwright + axe (zero violations) across every screen with mock data; keyboard-only P1 loop; component tests for each wrapper's focus behavior; S-0 CSP check; the M5 VoiceOver pass.

## Revisit when

- bits-ui 3 changes APIs broadly, or axe or VoiceOver findings trace to bits-ui defects we can't patch in the wrappers.
- S-0 requires `style-src 'unsafe-inline'` only because of bits-ui (then consider Ark UI).

## Exit path

Screens use only the `#lib/components/ui` wrappers. Swapping the underlying library (Ark UI or hand-rolled) rewrites that folder, not the screens.
