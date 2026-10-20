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

## Amendment 2026-10-20: hand-built primitives; bits-ui is not adopted

**Decision.** The UI uses hand-built, keyboard-complete primitives (`Dialog`, `Palette`, `Segmented`, tabs and the strip, in `src/lib/components/`), styled with the tokens. `bits-ui` is not in `package.json`, `pnpm-workspace.yaml` or `pnpm-lock.yaml`, and no code imports it. The Decision, Alternatives and Exit-path sections above are the Stage 4 record. This amendment replaces the Decision.

**Why.** Under the production CSP (`style-src 'self'`, SR-20), bits-ui's Command primitive, the one this ADR chose it for (the ⌘K palette, F-028), ships `style` attributes in its markup. A `style` attribute written in a component template is parsed from HTML and is governed by `style-src-attr`, which falls back to `style-src`. Nonces and hashes don't apply to attributes without `'unsafe-hashes'`, and ADR-014 allows `style-src 'unsafe-inline'` only as the S-0 fallback; it is not worth taking for a UI library.

**Evidence** (2026-10-20: `bits-ui@2.19.5` tarball from the npm registry, integrity `sha512-TEwkLSYE…ngCoew==`, published 2026-10-03; `svelte` 5.57.2; Chromium 141.0.7390.37 under Playwright):

1. **The mechanism.** Three templates contain a literal `style="display: contents;"`: `dist/bits/command/components/command-item.svelte:40` (also carries `data-item-wrapper`), `command-link-item.svelte:40` and `command-group-items.svelte:28`. These are the only static `style="…"` attributes in the package (`grep -rn 'style="' dist --include=*.svelte`). A wrapper component can't remove them, because they sit inside the library's own markup.
2. **Measured.** A probe app (Vite build, one of each primitive, served with `default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:`, a `securitypolicyviolation` listener) mounted `Command.Root` with one `Command.Item`. It raised exactly one violation, `style-src-attr`, blocked URI `inline`, with the console error "Refused to apply inline style". Dropping `Command.*` from the page gave zero violations. The same page with `'unsafe-inline'` added to `style-src` gave zero.
3. **What does not violate.** Everything else bits-ui sets at runtime goes through `mergeProps`, which turns a style object into a string (`svelte-toolbelt` 0.10.6 `dist/utils/merge-props.js:112-113`, `styleToString`). The element is a spread (`<div {...props}>`), and Svelte 5.57.2's `set_attributes` sends `style` to `set_style` (`svelte/src/internal/client/dom/elements/style.js`), which assigns `dom.style.cssText`. That is a CSSOM write, which CSP does not govern (a `setAttribute('style', …)` control in the same page was blocked). In the probe, Dialog, AlertDialog, Select, DropdownMenu, ContextMenu, Tooltip, Popover, Tabs, ToggleGroup, ScrollArea and a Svelte `fly` transition produced zero violations, and the floating wrapper's `position`, `transform` and `min-width` and the dialog's `pointer-events` and `contain` all applied (`dist/bits/utilities/floating-layer/use-floating-layer.svelte.js:104-131`, `dist/bits/dialog/dialog.svelte.js:270-280`).
4. **Other hazards.** `PinInput` creates a `<style>` element and calls `insertRule` (`dist/bits/pin-input/pin-input.svelte.js:201,425`); a `<style>` element without a nonce is blocked. We don't use it. The `<style>` blocks in Select and ScrollArea viewports are compiled into the bundle's CSS file and are fine.
5. **Chromium applied the Command wrapper's `display: contents` even while reporting the violation** (computed `display` was `contents`). That is browser-specific and not a pass: the violation event, the console error and the S-0 criterion "DevTools shows no CSP violations" (ADR-002) all still fail, and WKWebView hasn't been tried.

**So the earlier shorthand "bits-ui's inline styles break the CSP" was too broad.** Only Command is a hard conflict, and only because of static attributes. The runtime positioning styles in the other primitives are CSP-safe in Chromium with Svelte 5.57.2's `cssText` path (WKWebView unverified).

**What it costs.**

- We own the accessibility behavior of every primitive: focus trap and return, `aria-modal`, roving tabindex, type-ahead, Escape and outside-click handling, reduced motion. This is the "hand-rolled primitives" risk this ADR listed. Mitigation: axe in Playwright in both themes on every route and the keyboard-only P1 loop (`tests/flows/`), the screen component tests, and the M5 VoiceOver pass.
- Dynamic sizes and positions are written through CSSOM by our own code (`el.style.setProperty`), and no primitive may use a `style` attribute or `style=` in a template (`release-bundle` fails on inline styles).
- No library-maintained fixes for focus and dismissal edge cases. Floating-position logic (anchored menus, tooltips) is the part most expensive to hand-build, so a feature that needs it triggers the revisit below.

**Revisit when** (adds to the section above):

- S-0 records `style-src 'unsafe-inline'` for any reason (ADR-014's fallback rule). Then the Command conflict disappears and bits-ui is worth re-evaluating, starting with Command, Select and Menu, behind the same one-folder wrapper layer.
- S-0 shows on WKWebView that CSSOM-written styles are clean and bits-ui ships Command without static `style` attributes (check `command-item.svelte`, `command-link-item.svelte` and `command-group-items.svelte` in the new release).
- A hand-built primitive fails axe, keyboard or VoiceOver checks that a library would fix, or a v1.x feature needs anchored popovers or menus.
