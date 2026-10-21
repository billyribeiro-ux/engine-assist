# CR-013: Runtime styles under the strict CSP (xterm takes Tauri's style nonce; SvelteKit's announcer loses its inline style)

STATUS: proposed
REQUESTED BY: the wave-4 M3 build agent (ticket M3-08) · 2026-10-20
DECIDED BY: <Billy Ribeiro> · <date>

## What changes

The production policy stays exactly as ADR-014 and `csp::production_config_snapshot` pin it (`style-src 'self'`, no `'unsafe-inline'`, no `'unsafe-hashes'`, no remote source). Two pieces of third-party code that the policy refuses are made to fit it:

1. **xterm's runtime `<style>` elements take Tauri's style nonce.** `src/app.html` gains one empty `<style id="fm-style-nonce"></style>`. Tauri (2.12.4, `tauri-utils` `inject_nonce_token` and `manager::set_csp`) gives every `<style>` in the HTML a fresh nonce per load and adds `'nonce-…'` to `style-src`. While a tab's xterm opens, `#lib/terminal/style-nonce.ts` (`withStyleNonce`) sets that nonce on each `<style>` element xterm creates (its DOM renderer and viewport build three or four: cell sizes, theme colors, scrollbar). Only during `Terminal.open`; `document.createElement` is restored afterwards.
2. **SvelteKit's route announcer loses its static `style` attribute.** `node_modules/@sveltejs/kit/src/runtime/components/root.svelte` renders `<div id="svelte-announcer" … style="position: absolute; …">`. A static attribute in a template is governed by `style-src-attr` and refused. A `vite.config.ts` plugin (`foreman:announcer-without-inline-style`) drops the attribute before the component compiles (the build fails if the attribute isn't found, so a kit update can't silently bring it back); `base.css` hides `#svelte-announcer` with the same rules as `.vh`.

`scripts/check-release-bundle.mjs` now also fails on an inline style attribute in any compiled template, on a second or non-empty `<style>` in `index.html`, and on a remote URL in it.

## Why

- Measured in Chromium against the mock build served the way Tauri serves it (`tests/flows/csp.spec.ts`, `csp::production_policy_holds_on_every_screen`): Today raised one `style-src-attr` violation (the announcer), and every screen with a terminal raised `style-src-elem` violations for each style element xterm creates, and drew the terminal without its row layout and colors. With both changes: zero violations on all 15 screens, the terminal draws, and an injected inline script, `<style>` and `style` attribute are still refused (`csp::injected_inline_script_blocked`).
- Without this, S-0 rows B1 and B2 fail on the Mac and ADR-014's fallback (`style-src 'unsafe-inline'` for the whole app, threat T-013) becomes the only way out. This keeps `style-src` strict: a nonce is never in markup that arrives as HTML (SR-26), so injected markup still can't style the page.
- ADR-014 already relies on Tauri's nonces for `<style>` tags in the HTML; this extends that nonce to elements Foreman's own code creates while xterm opens. ADR-020 already bans static `style` attributes in our templates; the announcer is SvelteKit's.

## Impact
- Scope / feature catalog (F-IDs): F-009 (terminal rendering), none visible otherwise. No acceptance criterion changes.
- ADRs (new, amended, superseded): ADR-014 amended (Decision, "Hashes and nonces": runtime `<style>` elements created while xterm opens carry the carrier's nonce; the announcer's attribute is removed at build); ADR-020 (the release-bundle check now covers compiled templates).
- Threat model: T-013 (CSS injection) stays closed instead of being opened by the S-0 fallback. New residual: a script already running in the webview can read the nonce from the carrier and create allowed `<style>` elements; it can already restyle the page through CSSOM, so nothing is added.
- Data model and migrations: none.
- API contract and versioning: none.
- Pricing and entitlements: none.
- Timeline: none; built in `stage/05-w4-m3`.
- Cost (build + run): none; no new dependency.

## Recommendation
APPROVE. It keeps the strict policy that the Stage 2 council chose and removes the two concrete reasons S-0 would otherwise have to loosen it.

**Mac task:** `pnpm test:e2e` runs `02-csp.e2e.ts`, which now checks that the carrier has Tauri's nonce and that the served policy lists it; S-0 B1/B2 (zero violations at boot and with a streaming terminal in WKWebView) confirm the rest.

## Deltas for Stage 4 (filled in when approved)
- docs/scope.md: none.
- docs/adr/: ADR-014 (nonce carrier, xterm), ADR-020 (release-bundle covers compiled templates).
- docs/threat-model.md: T-013 residual note.
- docs/migrations/: none.
- docs/tickets.md: none.
