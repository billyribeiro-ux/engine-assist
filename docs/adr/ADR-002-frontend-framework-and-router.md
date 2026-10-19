# ADR-002: Frontend is SvelteKit 3 in SPA mode with file-based routing

## Status
Proposed

## Date
2026-10-19

## Context

Foreman's UI runs inside Tauri's WKWebView with no server. It has about ten screens (Today, first run, setup, project/stage workspace, gate, run log, Doctor, settings, palette), all fed by Rust commands (ADR-004). It needs:

- a strict CSP (SR-20: `script-src 'self'`, never `'unsafe-inline'`);
- a fast cold start (NFR-01);
- TypeScript strict with zero warnings (NFR-45);
- good tooling for a solo engineer working with Claude Code.

r7 recommended plain Svelte 5 + Vite 8 + an in-house typed hash router, and the r7 DRI record adopted it after Billy answered "no preference". **Billy has since decided on SvelteKit 3**, for consistency with all his other projects and for file-based routing and layouts. r7's DRI section allows Billy to override at the Stage 2 sign with the reason recorded.

## Decision

Use **SvelteKit 3.1.0 in SPA mode**, configured exactly as verified in r1 §A4 (kit 3.1.0, adapter-static 4.0.0, vite 8.3.4, svelte 5.57.2, TS 6.0.3, svelte-check 4.7.7: 0 errors, 0 warnings):

- `@sveltejs/kit` 3.1.0, `@sveltejs/adapter-static` 4.0.0, `svelte` 5.57.2, `vite` 8.3.4, `@sveltejs/vite-plugin-svelte` 7.3.1.
- Configuration in `vite.config.ts` (SvelteKit 3 no longer supports `svelte.config.js`):
  ```ts
  sveltekit({ adapter: adapter({ fallback: 'index.html' }), version: { pollInterval: 0 } })
  ```
- `export const ssr = false` (and `prerender = false`) in `src/routes/+layout.ts`.
- `#lib` imports through `package.json` `"imports": { "#lib": "./src/lib/index.ts", "#lib/*": "./src/lib/*" }`. `$lib` doesn't exist in SvelteKit 3.
- `tsconfig.json` extends `$app/tsconfig` and lists `include`/`exclude`.
- `tauri.conf.json` `build`: `frontendDist: "../build"`, `devUrl: "http://localhost:5173"`, `beforeDevCommand: "pnpm dev"`, `beforeBuildCommand: "pnpm build"`.
- **Routing:** SvelteKit file-based routes (pathname), `goto` and `$app/state`; no in-house router. Tauri's asset resolver falls back to `index.html` for unknown paths (verified in `tauri-2.12.4/src/manager/mod.rs`), so a reload on `/projects/x/stages/3` works.
- **CSP:** SvelteKit's SPA fallback page has one inline bootstrap `<script>` (r1 E6). Tauri hashes it into `script-src` at compile time; `'unsafe-inline'` is never used (ADR-014).
- **State:** Svelte 5 runes in `.svelte.ts` modules mirroring Rust (ADR-004). SvelteKit `load` functions call only the `IpcClient`, never `fetch`.
- **Rendering of repo content:** no `{@html}` (ESLint rule). Markdown goes through `#lib/render` to a fixed element set (SR-26).

Registry check, 2026-10-19:

- `@sveltejs/kit` `latest` 3.1.0, peers `vite ^8.0.12`, `svelte ^5.57.1`, `typescript ^6.0.0`, `@sveltejs/vite-plugin-svelte ^7.0.0`;
- `@sveltejs/adapter-static` `latest` 4.0.0;
- `svelte` 5.57.2;
- `vite` 8.3.4.

## Alternatives considered

- **Plain Svelte 5 + Vite 8 + in-house typed hash router** (r7's recommendation). Measured on the same two-route app: **11.8 KB gzip in one JS file vs 30.3 KB gzip in 12 files for SvelteKit**, and **no inline script**, so the CSP is literally `script-src 'self'`. It also has fewer config surfaces and no dependency on a weeks-old major. It lost on Billy's priorities: one mental model and one set of conventions across all his SvelteKit projects, plus file routing and layouts for the project/stage/gate hierarchy. The bundle difference (about 18 KB gzip) is small against NFR-01's 1.5 s, and the hash-pinned bootstrap keeps SR-20 intact.
- **React 19 + Vite.** The largest training corpus, but it discards Billy's Svelte skill base and the official Svelte MCP tooling for no Foreman-specific gain (r7 §2).
- **SvelteKit 3 with hash routing** (if offered by kit 3). Not needed, since Tauri's `index.html` fallback makes pathname routing reload-safe. Not evaluated further.

## Consequences

- The release is 18 days old (3.0.0 shipped 2026-10-01). The Tauri SvelteKit guide targets kit 2.20.4 and its `svelte.config.js` part is invalid on kit 3, so r1 §A4 is our reference.
- There's one inline script in `index.html`. The CSP depends on Tauri's compile-time hashing, which `csp::production_config_snapshot` and the Playwright CSP test must prove.
- SvelteKit features that need a server (`+page.server.ts`, form actions, remote functions, hooks.server) are banned by an ESLint `no-restricted-syntax` / file-pattern check in `pnpm verify`.
- **UNVERIFIED:** whether `version.pollInterval: 0` also stops the focus/visibility `_app/version.json` check that SvelteKit 3 added (r1 §A4). Tried: r1's migration-guide read, which states the new checks but not their interaction with `0`. S-0 decides. If a fetch remains, `connect-src` gains `'self'` (a local asset, not a network request) and ADR-014 records it.

## Verification

- `pnpm verify`: `svelte-check --fail-on-warnings`, ESLint (no `{@html}`, no server files, `@tauri-apps/api` imported only in `#lib/ipc`), Vitest.
- Playwright: an injected inline `<script>` doesn't run; navigation and a reload on a deep route render the same screen.
- S-0: CSP check with bits-ui, Svelte transitions, xterm and the SvelteKit bootstrap; DevTools shows no CSP violations and no `version.json` requests.
- NFR-01 startup mark.

## Revisit when

- SvelteKit 3 breaks the static-adapter SPA mode or the Tauri asset fallback.
- The inline bootstrap can't be hash-pinned.
- Cold start misses NFR-01 because of route-chunk loading.

## Exit path

Move to plain Svelte 5 + Vite. Routes become components in a typed route table (about 50 lines). Stores, components, the `IpcClient` and the mock are unchanged because they don't import SvelteKit APIs except `goto`/`$app/state` (wrapped in `#lib/nav`).
