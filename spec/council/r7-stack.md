# Council · Prompt 2.4 · Stack Decision, Evidence First

**Project:** Foreman · **Input:** Master Spec draft v0.9, NFRs §10, SR-01…SR-09, design direction v0.9, and `council/r1-engineering.md` · **Date:** 2026-10-19
**Author:** principal-engineer · **Status:** recommendation; the final choice is Billy's (bottom section)

**How this was decided.**

- **Reused from r1:** registry versions read live on 2026-10-19, the comrak vs pulldown-cmark probe on the real blueprint, the SvelteKit 3 SPA build, and the Tauri, PTY and Claude Code source and doc reads. They are cited as "r1 §X" rather than re-fetched.
- **New this round:**
  - Electron, SwiftTerm, node-pty and swift-markdown evidence (docs, registries, git history);
  - two macOS shell benchmarks;
  - Svelte's own guidance on SvelteKit vs plain Vite;
  - a **head-to-head build of the same two-route app** as a SvelteKit 3 SPA and as plain Svelte 5 + Vite.
- **UNVERIFIED** marks anything not confirmed, with what was tried.

---

## Requirements that drive every layer

Everything below is judged against these, all taken from the spec:

| ID | Requirement | Source |
|---|---|---|
| R-PLAT | macOS on Apple Silicon only, current and previous major; personal use; no notarization or App Store | §10, §12, brief |
| R-NATIVE | "Foreman should feel like a Mac app, not a website": SF Pro and SF Mono, sidebar vibrancy, transparent title bar with traffic lights inset | design direction §3–§4 |
| R-START | Cold start to usable ≤ 1.5 s; open a project with 200 tickets ≤ 300 ms; prompt resolution ≤ 50 ms; packet detected ≤ 1 s after it's written | §10 |
| R-MEM | ≤ 300 MB with 3 terminal tabs open | §10 |
| R-TERM | An embedded terminal that hosts the interactive Claude Code TUI (full screen, colors, resize, paste, mouse), one tab per run, reachable by VoiceOver | F-008, §10 |
| R-SPAWN | Spawn only allowlisted absolute paths with argv arrays, never a shell; show the exact argv; log every run | SR-01, SR-02, SR-03 |
| R-FS | File access limited to registered projects, the blueprint repo and app data, enforced in the backend | SR-04, SR-09 |
| R-DATA | Repo content is rendered as text, never HTML or executed; strict CSP; no remote content; no telemetry or network | SR-05, SR-06, SR-08, PR-01 |
| R-PARSE | Parse the blueprint Markdown (GFM tables, blockquotes, fenced info strings, HTML-comment markers) with line numbers for errors | F-002, r1 §A1/§A6 |
| R-QUAL | Strict types with zero warnings, lint-clean, a test for every §5 and §8 rule, WCAG 2.2 AA, full keyboard operation | §10 |
| R-SOLO | One engineer building with Claude Code: good docs, tooling Claude can drive, broad training coverage | brief, blueprint 2.4 criteria |

Note on R-MEM: on macOS, WKWebView's content runs in separate WebKit processes, and Chromium apps use multiple processes. **R-MEM must be measured as the sum of the app's whole process tree** (the method in the betterstack benchmark below), not the main process alone.

---

## PART 1 · Evidence first (preferences ignored)

### 1. Desktop shell

**Requirements:** R-PLAT, R-NATIVE, R-START, R-MEM, R-SPAWN, R-FS, R-DATA, R-SOLO.

| Criterion | **Tauri 2** | **Electron** | **Native SwiftUI/AppKit + SwiftTerm** |
|---|---|---|---|
| Current stable | tauri 2.12.4 (2026-10-14); 3.0.0-alpha.5 in development (r1 §B) | electron **44.9.0** (2026-10-17); a new major every 8 weeks, and the "latest three stable major versions are supported" ([timelines](https://www.electronjs.org/docs/latest/tutorial/electron-timelines)) | macOS SDK (Xcode); SwiftTerm HEAD 2026-10-18, **607 commits in the last 12 months**, tags up to v1.99.0, a 2.0 API in the README |
| Support horizon | Stable major; v3 in alpha, so plan one major migration in the app's life | About 24 weeks per major means an **upgrade every ~2 months** to stay supported | Apple platform: longest horizon |
| macOS native feel | System WKWebView. Built-in `windowEffects` (sidebar, under-window, HUD materials, Liquid Glass on macOS 26) and `titleBarStyle: Overlay\|Transparent` (r1 §B) | Chromium. `vibrancy` and `titleBarStyle` exist (not re-fetched; **UNVERIFIED** this round) | Native by definition: real `NSVisualEffectView`, menus, accessibility |
| Bundle size (macOS, same app) | **57 MB** (includes FFmpeg; author: "could fall under 15 MB" without it) | **323 MB** | Smallest (no web runtime). **UNVERIFIED** (no measurement found) |
| Startup (median of 10, macOS) | 311 ms | 273 ms | Not measured; native launch is typically fastest (**UNVERIFIED**) |
| Idle memory (macOS, whole process tree) | **109 MB** (main ≈ 36 MB + WebKit helpers) | 128 MB | Not measured (**UNVERIFIED**) |
| Counter-evidence on memory | tauri-apps/tauri#5889: loading *heavy remote sites* (postman.com, vscode.dev) on macOS 12.6, Tauri used 421–429 MB vs Electron 332–337 MB. The method wasn't stated; the issue was closed as "a discussion with no specific action items" | — | — |
| Process spawn / PTY | Rust `std::process` + `portable-pty` (r1 §A2); allowlist enforced in Rust | Node `child_process` + **node-pty 1.1.0** (Microsoft; used by VS Code, Hyper, Theia; 202 commits in 12 months) | `Process`/`posix_spawn` + SwiftTerm `LocalProcess` (uses `forkpty`; `startProcess(executable:args:environment:currentDirectory:)` takes argv) |
| Security model | Capabilities and permissions; compile-time CSP hashing; `freezePrototype`; isolation pattern; webview has no Node (r1 §B) | 20-item security checklist the app must apply itself: contextIsolation, sandbox, no nodeIntegration, CSP, fuses, IPC sender validation… ([security](https://www.electronjs.org/docs/latest/tutorial/security)). "Security is everyone's responsibility" | No webview means no XSS class at all. SR-05 and SR-06 become trivial |
| Claude Code fit | Two first-party AI tool sets help: Svelte's official MCP server, skills and Claude Code plugin ([svelte ai/claude-plugin](https://svelte.dev/docs/ai/claude-plugin)), plus Claude's strong Rust and TS coverage. Smaller Tauri-specific corpus than Electron (**UNVERIFIED**, no corpus data) | Largest web-desktop corpus; Playwright drives Electron (experimental, v21+) ([Playwright Electron](https://playwright.dev/docs/api/class-electron)) | Xcode 26.3 integrates the Claude Agent SDK, so Claude can capture Previews ([Anthropic, 2026-02-03](https://anthropic.com/news/apple-xcode-claude-agent-sdk)). But the SwiftUI/AppKit iteration loop runs through Xcode, and the spec's quality bar (TS strict, Vitest, Svelte a11y tooling) is web-shaped |
| E2E on macOS | Embedded WebDriver plugin + `@wdio/tauri-service` (r1 §A3) | Playwright `_electron` (experimental) | XCUITest (native, mature) |
| Adoption | Desktop Insights: 1,798 of 28,699 tracked apps (6.3%) use Tauri, 968 on macOS ([desktopinsights](https://desktopinsights.com/tech/tauri)) | VS Code, Slack, many more (common knowledge; not re-verified) | Every Mac app; SwiftTerm is used by Secure Shellfish, La Terminal and CodeEdit (README) |

**Verdict: Tauri 2.**

- **Electron** meets every functional requirement and has the best terminal story (node-pty and xterm.js, as in VS Code). It loses on three points:
  1. a bundle about 5.7× larger;
  2. an 8-week upgrade treadmill to stay supported;
  3. a security model where SR-01, SR-04 and SR-06 depend on the app correctly applying a 20-item checklist. In Tauri, the webview has no Node, and Rust is the only place that can spawn processes or touch files.

  The startup and idle-memory numbers are within noise of Tauri for a local app. Issue #5889 shows WebKit can cost more than Chromium for heavy remote web apps, but Foreman loads only a small local SPA.
- **Native SwiftUI/AppKit + SwiftTerm** has the best native feel, likely the lowest memory, and removes the whole webview attack surface. It's the strongest alternative, and **it would be the winner if R-NATIVE were the top criterion**. It loses on R-SOLO and R-QUAL:
  - the spec's quality gates, the planned design-system workflow (Stage 4.5 "reviewable HTML mockups"), the blueprint's web tooling (Vitest, Playwright, svelte-check) and the domain code (packet rules, rule engine) would all have to move to Swift;
  - SwiftTerm's own author notes it trails xterm.js on "Selection/Accessibility" (README History section), which matters for the VoiceOver requirement.
- **Confidence: medium-high.** The decisive inputs (bundle, security model, upgrade cadence, tooling fit) are documented. The memory comparison is noisy and must be confirmed by the Stage 5 spike (3 tabs ≤ 300 MB).

### 2. Frontend framework

**Requirements:** R-START, R-DATA (CSP), R-QUAL, R-SOLO, plus five or six screens with routes (dashboard, project, gate, lessons, settings, doctor).

| Criterion | **SvelteKit 3 SPA (adapter-static)** | **Svelte 5 + Vite + client router** | Others (React 19 + Vite; SolidJS) |
|---|---|---|---|
| Versions | kit 3.1.0 (3.0.0 shipped 2026-10-01; 18 days old), adapter-static 4.0.0, svelte 5.57.2, vite 8.3.4 | svelte 5.57.2 (5.0.0 on 2024-10-19, two years of runes), vite-plugin-svelte 7.3.1, vite 8.3.4. Router options: `sv-router` 0.19.0 ("Type-safe routing for Svelte SPAs"), `svelte-spa-router` 5.1.1, or a 20-line hash router | Not evaluated in depth (see note) |
| Official stance | Svelte docs: "We recommend using SvelteKit"; SPA mode is supported, and server files can be ignored ([project types](https://svelte.dev/docs/kit/project-types)) | Svelte docs list it as the alternative: "use Svelte directly with Vite … you will probably need to choose a routing library" ([getting started](https://svelte.dev/docs/svelte/getting-started)) | — |
| **Measured build** (same two-route app, 2026-10-19) | 12 JS files, **82.2 KB raw / 30.3 KB gzip**; **one inline bootstrap `<script>`** (388 chars) in `index.html` | 1 JS file, **30.6 KB raw / 11.8 KB gzip**; **no inline script** | — |
| Type check | `svelte-check`: 0 errors, 0 warnings (r1 §A4) | `svelte-check`: 0 errors, 0 warnings (after adding `lang="ts"`) | — |
| What it adds for Foreman | File-based routing, layouts, `load`, `$app/state`, `goto`, error boundaries. **None of the server features apply** (no SSR, actions, remote functions or hooks.server) | Nothing extra; routes are components | — |
| Risk | v3.0 is 18 days old (the first 3.x patch landed 5 days after release). The Tauri guide was written for kit 2 and needs the r1 §A4 changes. Extra config (`ssr = false`, fallback, `#lib`, `$app/tsconfig`, `version.pollInterval`) | A third-party router is the weakest link (sv-router is 0.x), or we own a tiny router | — |
| SR-06 CSP | The inline bootstrap must be hash-allowed (Tauri does this at compile time, r1 E6) | Strict `script-src 'self'` with nothing inline, so it's simpler to audit | — |
| Claude Code fit | Svelte MCP, skills and plugin cover both. SvelteKit 3 is too new for training data; Claude must rely on the MCP docs | Same tools; Svelte 5 runes are well documented | React has the largest training corpus |

**Verdict: Svelte 5 + Vite with a small in-house router** (a typed route table driven by `location.hash`, about 50 lines and fully unit-testable), over SvelteKit 3 SPA.

- An SPA in Tauri uses none of SvelteKit's server-side value.
- Plain Vite gives a bundle about 2.6× smaller on the same app (initial parse cost matters for R-START), a stricter CSP with no inline script, fewer moving parts, and no exposure to an 18-day-old major.
- The cost is losing file-based layouts and `load`, which barely matter for six screens whose data comes from Tauri commands.
- **Note on React and Solid:** not evaluated in depth. React's training-corpus advantage is real, but switching frameworks would discard Billy's skill base and the official Svelte MCP tooling, for no Foreman-specific gain. Svelte 5 is within noise on performance for this app size (**UNVERIFIED**, no benchmark run).
- **Confidence: medium.** Both choices work (both built and type-checked). The margin is simplicity and CSP strictness, not capability.

### 3. Language and toolchain

**Requirements:** R-QUAL (strict, zero warnings), R-SOLO.

| Layer | Candidates | Evidence | Winner |
|---|---|---|---|
| Frontend language | **TypeScript 6.0.3** · TypeScript 7.0.2 · plain JS + JSDoc | TS 7.0 "ships without a stable programmatic API, which the team expects to land in 7.1"; svelte-check peers `^5\|\|^6`, typescript-eslint `<6.1.0`, kit 3.1 `^6.0.0` (r1 §B, E2) | **TypeScript 6.0.3**, strict |
| Backend language | **Rust** (with Tauri) · Node (with Electron) · Swift (native) | Follows the shell decision | **Rust stable**, pinned via `rust-toolchain.toml`. Floor: Tauri crates need ≥ 1.90 (`tauri-build` 2.7.3 MSRV 1.90). Latest stable Rust **UNVERIFIED** (sandbox has 1.97.0) |
| Package manager | **pnpm 12.14.0** · npm 10/12 · bun | pnpm: strict, content-addressed store; the r1 build worked. No Foreman-specific advantage for the others (**UNVERIFIED**, no benchmark run) | **pnpm**, pinned via `packageManager` |
| Runtime | Node ≥ 22.17 (kit floor) / ≥ 22.12 (vitest 5 floor) | r1 §B; the nodejs.org index was blocked | **Node 22 LTS ≥ 22.17** (current LTS line **UNVERIFIED**) |

### 4. Terminal emulator component

**Requirements:** R-TERM, R-MEM, R-NATIVE (SF Mono), VoiceOver.

| | **@xterm/xterm 6.0.0** | **ghostty-web 0.4.0** | SwiftTerm (native only) |
|---|---|---|---|
| Maturity | 6.0.0 (2025-12-22), 6.1 betas active. Used by VS Code, JupyterLab, Azure Cloud Shell, Hyper, Replit, Theia and 90+ others ([xtermjs.org](https://xtermjs.org/)) | 0.4.0 (2025-12-09), "Web-based terminal emulator using Ghostty's VT100 parser via WebAssembly" (coder/ghostty-web). libghostty had no tagged stable embedding release as of 2026-06-23 ([webteractive](https://webteractive.co/blog/ghostty-and-libghostty-the-terminal-core-quietly-reshaping-the-ecosystem)) | Active (607 commits in 12 months) but needs the native shell |
| Accessibility | `screenReaderMode` option in the typings (r1 §B) | **UNVERIFIED** | Has `MacAccessibilityService.swift`; the author says it trails on accessibility |
| Renderer | WebGL addon 0.19.0 or DOM. **The canvas addon is xterm-5-only** (r1) | WASM parser + canvas | Metal optional |

**Verdict: @xterm/xterm 6.0.0** with `@xterm/addon-fit` 0.11.0, `@xterm/addon-webgl` 0.19.0 (DOM fallback), `@xterm/addon-unicode11` 0.9.0, `@xterm/addon-search` 0.16.0 and `@xterm/addon-web-links` 0.12.0. **Confidence: high.**

### 5. PTY

**Requirements:** R-TERM, R-SPAWN.

| | **portable-pty 0.9.0** | **pty-process 0.5.4** | tauri-plugin-pty 0.3.1 |
|---|---|---|---|
| Adoption | 22.0 M downloads; **662 reverse dependencies** (atuin, skim, wezterm-ssh, tui-term) | 5.6 M downloads; 40 reverse dependencies; released 2026-10-19 | 51 k downloads |
| Maintenance | Published 2025-02; source still maintained in wezterm (commits through 2026-09) | Active | Single maintainer, "Developing!" |
| Fit | Sync reader and writer threads, resize, kill, wait; `openpty` on macOS | Async (tokio) API (from its description; **UNVERIFIED** in depth) | The webview chooses the program, which **violates SR-01** |

**Verdict: portable-pty 0.9.0** (r1 §A2). pty-process is a credible async alternative, but its adoption is 16× smaller. **Confidence: high.**

### 6. Process spawning (non-interactive)

**Requirements:** R-SPAWN (no shell; allowlisted absolute paths), Doctor and headless gate output capture.

| Candidate | Evidence | Verdict |
|---|---|---|
| **`tokio::process::Command` / `std::process::Command`** with argv arrays | Never invokes a shell; Rust owns the allowlist | **Winner** |
| `tauri-plugin-shell` 2.5.0 | Exposes spawning to the webview behind a capability scope (r1 §B) | Rejected: it adds a webview-reachable spawn surface |
| `which` 8.0.6 (resolution helper) | Searches `PATH`, but a Finder-launched app gets a minimal PATH (r1 §A2) | Optional; Foreman still checks known install locations explicitly |

### 7. File watching

**Requirements:** packet detected ≤ 1 s; ignore `.git`, `node_modules`, `target`, `.env*` (SR-09).

| | **notify 8.2.0 + notify-debouncer-full 0.7.0** | notify 9.0.0-rc.6 | watchexec 8.4.4 |
|---|---|---|---|
| Evidence | 174 M downloads, 3,168 reverse dependencies, FSEvents default on macOS (r1 §B) | Release candidate | A CLI and library built *on* notify; more than needed |

**Verdict: notify 8.2.0 + notify-debouncer-full 0.7.0**, about 250 ms debounce. **Confidence: high.**

### 8. Markdown parsing

**Requirements:** R-PARSE.

| | **comrak 0.56.0** | pulldown-cmark 0.13.4 | markdown-rs 1.0.0 |
|---|---|---|---|
| Fidelity on the real blueprint | Identical structure (r1 §A1) | Identical (r1 §A1) | Not probed |
| Line numbers | **`sourcepos` (line and column) on every AST node** | Byte offsets on events; build your own index and tree | **UNVERIFIED** |
| Markers | `HtmlBlock` with line number (probed, r1 §A6) | Raw HTML events | — |
| Maintenance | 13 releases in 12 months (breaking 0.x), 315 reverse deps | 1,867 reverse deps; last release 2026-05 | 1.0.0 (2025-04) |

**Verdict: comrak 0.56.0**, pinned exactly. **Confidence: high.**

### 9. UI primitives

**Requirements:** R-QUAL (WCAG 2.2 AA, keyboard), the Cmd+K palette, dialogs, menus, tabs and tooltips. Native look comes from our own CSS tokens.

| | **bits-ui 2.19.5** | @ark-ui/svelte 5.24.3 (Zag state machines) | melt 0.44.0 (builder API) |
|---|---|---|---|
| Svelte 5 | peer `svelte ^5.33.0`; no SvelteKit import (r1 §B) | peer `svelte >=5.20.0` | peer `svelte ^5.30.1` |
| Coverage for Foreman | `command`, `dialog`, `alert-dialog`, `context-menu`, `dropdown-menu`, `tabs`, `tooltip`, `select`, `toggle-group`, `scroll-area`… (r1 §B) | Broad (Zag machines) | Builders, not components |
| Activity | Released 2026-10-03 | Released 2026-10-05 | Released 2026-01-04 |

**Verdict: bits-ui 2.19.5.** It has the most direct component coverage, including Command for F-017. Ark UI is a close second. Accessibility claims are the vendors' own, so axe tests in our Playwright layer are the proof. **Confidence: medium-high.**

### 10. State management

**Requirements:** Rust is the source of truth for files (§11); the UI mirrors backend state; external edits win (§7).

| Candidate | Evidence | Verdict |
|---|---|---|
| **Svelte 5 runes in `.svelte.ts` modules** (`$state`, `$derived`) fed by Tauri commands and `Channel`/events | Svelte docs: with runes, the store use cases "have greatly diminished"; shared state goes in `.svelte.ts` files ([stores](https://svelte.dev/docs/svelte/stores), [.svelte.ts](https://svelte.dev/docs/svelte/svelte-js-files)) | **Winner** |
| `svelte/store` | "Still a good solution when you have complex asynchronous data streams" (same page) | Use only for the PTY byte stream if needed |
| `@tanstack/svelte-query` 6.3.5 | A server-cache library; no HTTP server here | Rejected (no fit) |

### 11. Testing

| Level | Winner | Alternatives considered | Evidence |
|---|---|---|---|
| Rust unit + property | `cargo test` + **proptest 1.12.0** + **insta 1.49.0** (golden snapshots of the parsed template) | quickcheck | r1 §B; insta for the F-002 golden tests |
| Rust integration | `tauri` `test` feature: `mock_builder`, `get_ipc_response` + a fake-`claude` binary | — | r1 §A3 |
| Component | **vitest 5.0.5** + `@vitest/browser-playwright` 5.0.5 + `vitest-browser-svelte` 3.1.0 + `@tauri-apps/api/mocks` | jsdom 30.1.3 + `@testing-library/svelte` 5.4.2 | Svelte testing docs recommend Vitest ([testing](https://svelte.dev/docs/svelte/testing)) |
| Flow | **@playwright/test 1.64.1** (WebKit project) + `@axe-core/playwright` 4.14.0 against the web build with mockIPC | Cypress | WCAG 2.2 AA automation |
| Real app (macOS) | **@wdio/tauri-service 1.5.0** + `tauri-plugin-wdio-webdriver` 1.5.0 (feature `e2e` only) + webdriverio **9.32.0** (the service peers `webdriverio ^9`) | tauri-driver (no macOS); CrabNebula (paid on macOS) | r1 §A3 |

### 12. Lint and format

| | Winner | Alternatives | Evidence |
|---|---|---|---|
| TS/Svelte lint | **eslint 10.13.0** + `eslint-plugin-svelte` 3.23.1 + `typescript-eslint` 8.72.0 + `svelte-eslint-parser` 1.9.0 | Biome 2.5.15; oxlint 1.89.0 | Svelte's official lint plugin is ESLint-only; Biome and oxlint Svelte support **UNVERIFIED** |
| Types | **svelte-check 4.7.7** `--fail-on-warnings` | — | r1 §A4 |
| Format | **prettier 3.9.10** + `prettier-plugin-svelte` 4.1.1 | Biome | Official Svelte formatter plugin |
| Rust | **rustfmt + clippy** `-D warnings` | — | §10 |

---

## PART 2 · Compared with Billy's preference

**Preference:** Tauri · SvelteKit 3 + Svelte 5 · TypeScript strict · pnpm · Rust.

| Layer | Preference | Part 1 winner | Difference | What he gains by switching | What he loses | Confidence |
|---|---|---|---|---|---|---|
| Shell | Tauri | Tauri 2 | **Equal** | — | — | — |
| Frontend | **SvelteKit 3** + Svelte 5 | **Svelte 5 + Vite** + in-house router | **Differs** | A 2.6× smaller bundle (11.8 vs 30.3 KB gzip, measured); no inline bootstrap script, so SR-06 CSP is literal; fewer config surfaces (no `ssr`, fallback, `#lib`, `$app/tsconfig`, polling); no dependency on an 18-day-old major whose Tauri guide is stale | File-based routing and layouts; `$app/state`/`goto`; consistency with his other SvelteKit projects (muscle memory, shared conventions); Svelte's "we recommend SvelteKit" default | **Medium.** Both are verified to build and type-check. If he values one mental model across all his projects, SvelteKit 3 SPA is a sound choice and costs nothing that matters at this app's size. |
| Language (TS) | TypeScript strict | TypeScript **6.0.3** strict | **Equal** (version pinned) | — | — | High |
| Language (backend) | Rust | Rust stable | **Equal** | — | — | — |
| Package manager | pnpm | pnpm 12.14.0 | **Equal** | — | — | — |
| All other layers | (not specified) | xterm 6, portable-pty, notify 8.2, comrak, Bits UI, runes, Vitest/Playwright/WDIO, ESLint/Prettier | No conflict | — | — | — |

The one real fork in the road is **SvelteKit 3 vs plain Svelte 5 + Vite**. Everything else in his preference matches the evidence-first winners.

---

## PART 3 · Final recommendation

| Layer | Recommendation | One line of reasoning |
|---|---|---|
| Desktop shell | **Tauri 2.12.4** | Smallest bundle, and a security model where only Rust can spawn or touch files (SR-01, SR-04). Native effects are built in. Electron is close on speed, but runs an 8-week upgrade cycle and depends on a security checklist the app must apply itself. |
| Frontend | **Svelte 5 + Vite 8 + in-house hash router** (SvelteKit 3 SPA is an acceptable alternative; DRI choice) | An SPA uses no SvelteKit server features. Plain Vite is 2.6× smaller, has no inline script, and has fewer moving parts. |
| Language | **TypeScript 6.0.3 strict** · **Rust stable** (≥ 1.90; pin in `rust-toolchain.toml`) | TS 7 breaks svelte-check and typescript-eslint until 7.1. Tauri crates need Rust ≥ 1.90. |
| Package manager / runtime | **pnpm 12.14.0** · Node 22 LTS ≥ 22.17 | Strict and reproducible; matches the floors of every tool. |
| Terminal component | **@xterm/xterm 6.0.0** + WebGL/DOM renderer | The industry default (VS Code and others); it has a screen-reader mode. |
| PTY | **portable-pty 0.9.0** (own wrapper) | 662 dependents, macOS `openpty`, and Rust picks the program (SR-01). |
| Process spawning | **tokio::process / std::process**, argv only; no `tauri-plugin-shell` | No shell, no webview-reachable spawn API. |
| File watching | **notify 8.2.0 + notify-debouncer-full 0.7.0** | FSEvents, the standard crate; the 9.x line is still an RC. |
| Markdown | **comrak 0.56.0** (exact pin) | AST with line and column on every node; identical fidelity on the real blueprint. |
| UI primitives | **bits-ui 2.19.5** | Svelte-5-native headless components, including Command for Cmd+K. |
| State | **Svelte 5 runes in `.svelte.ts`**, mirroring Rust state over commands, Channel and events | The official guidance for Svelte 5 shared state; Rust stays the source of truth. |
| Testing | **cargo test + proptest 1.12.0 + insta 1.49.0** · **vitest 5.0.5** (browser mode) · **@playwright/test 1.64.1** + axe · **@wdio/tauri-service 1.5.0** smoke | Layered; `tauri-driver` has no macOS support. |
| Lint / format | **eslint 10.13.0** + eslint-plugin-svelte · **svelte-check 4.7.7** · **prettier 3.9.10** · **rustfmt + clippy -D warnings** | Official Svelte tooling; zero-warning gates. |

### Pinned versions (registries, 2026-10-19)

**Rust (`src-tauri/Cargo.toml`)**

| Crate | Version | Notes |
|---|---|---|
| tauri | 2.12.4 | features as needed; `test` in dev-deps |
| tauri-build | 2.7.3 | MSRV 1.90 |
| portable-pty | 0.9.0 | |
| notify | 8.2.0 | default `macos_fsevent` |
| notify-debouncer-full | 0.7.0 | |
| comrak | =0.56.0 | `default-features = false` |
| tokio | 1.54.0 | `process`, `rt-multi-thread`, `macros`, `sync` |
| serde / serde_json | 1.0.229 / 1.0.154 | |
| thiserror | 2.0.21 | |
| uuid | 1.28.0 | `--session-id` values (r1 E5) |
| semver | 1.0.28 | Doctor version floor |
| tempfile | 3.27.0 | atomic writes (temp + rename) |
| tracing | 0.1.44 | local logs only (SR-08) |
| tauri-plugin-opener | 2.7.2 | open claude.ai in the browser |
| tauri-plugin-clipboard-manager | 2.5.0 | F-009, F-010 |
| tauri-plugin-dialog | 2.8.3 | folder picker in the wizard |
| tauri-plugin-window-state | 2.5.0 | remember window and drawer size |
| tauri-plugin-wdio-webdriver | 1.5.0 | **optional, `e2e` feature only** |
| proptest (dev) | 1.12.0 | |
| insta (dev) | 1.49.0 | golden template snapshots |

**JS (`package.json`, exact versions)**

| Package | Version |
|---|---|
| svelte | 5.57.2 |
| vite | 8.3.4 |
| @sveltejs/vite-plugin-svelte | 7.3.1 |
| typescript | 6.0.3 |
| @tauri-apps/api | 2.12.4 |
| @tauri-apps/cli | 2.12.3 |
| @tauri-apps/plugin-opener · plugin-clipboard-manager · plugin-dialog · plugin-window-state | 2.7.2 · 2.5.0 · 2.8.3 · 2.5.0 |
| @xterm/xterm | 6.0.0 |
| @xterm/addon-fit · addon-webgl · addon-unicode11 · addon-search · addon-web-links | 0.11.0 · 0.19.0 · 0.9.0 · 0.16.0 · 0.12.0 |
| bits-ui | 2.19.5 (peer `@internationalized/date` 3.12.4). **Not adopted (2026-10-20):** not in `package.json`; the UI uses hand-built primitives (ADR-020 amendment). Kept here as the council record. |
| vitest · @vitest/browser-playwright · @vitest/coverage-v8 | 5.0.5 |
| vitest-browser-svelte | 3.1.0 |
| @playwright/test | 1.64.1 |
| @axe-core/playwright | 4.14.0 |
| @wdio/tauri-service | 1.5.0 |
| webdriverio · @wdio/cli · @wdio/local-runner · @wdio/mocha-framework | 9.32.0 |
| eslint · @eslint/js | 10.13.0 · 10.0.1 |
| eslint-plugin-svelte · svelte-eslint-parser | 3.23.1 · 1.9.0 |
| typescript-eslint | 8.72.0 |
| eslint-config-prettier · globals | 10.1.8 · 17.13.0 |
| svelte-check | 4.7.7 |
| prettier · prettier-plugin-svelte | 3.9.10 · 4.1.1 |
| pnpm (`packageManager`) | 12.14.0 |

**If the DRI keeps SvelteKit 3 SPA**, add `@sveltejs/kit` 3.1.0 and `@sveltejs/adapter-static` 4.0.0, and use the r1 §A4 config (`sveltekit({ adapter: adapter({ fallback: 'index.html' }), version: { pollInterval: 0 } })`, `ssr = false`, `#lib`, `$app/tsconfig`).

**Toolchain floors:** Rust ≥ 1.90 · Node ≥ 22.17 · Claude Code ≥ 2.1.293 (the kit's `Explore` and `test-runner` agents use Haiku 5.5; r4 R-19). Dev-dependency added after the freeze review: criterion 0.8.2 (benchmarks for NFR-02/03/04).

### Stage 5 spikes that confirm this decision

1. **Memory and start:** three xterm tabs each running a real `claude` TUI. Sum the process-tree RSS against the 300 MB target; measure cold start to first interactive frame against 1.5 s. If memory fails badly, revisit the native SwiftUI + SwiftTerm option (the only candidate likely to beat it).
2. **VoiceOver:** check xterm `screenReaderMode` inside WKWebView with VoiceOver.
3. **CSP:** confirm the strict CSP with Bits UI and Svelte style handling (`style-src`) (r1 §B, UNVERIFIED).

### Could not verify (this round)

| Item | Tried |
|---|---|
| Electron vibrancy and titleBarStyle API detail | not re-fetched (time); widely documented |
| Native app memory and bundle numbers | no comparable benchmark found |
| Biome and oxlint Svelte support level | not fetched |
| ghostty-web accessibility | not documented in sources found |
| llms.txt availability for Tauri, Svelte and Electron docs | proxy 403 |
| Latest stable Rust; Node LTS line | rust-lang.org not fetched; nodejs.org 403 (r1) |
| Relative training coverage (Tauri vs Electron vs SwiftUI) | no corpus data exists publicly |

### Sources (new this round; r1 sources still apply)

- https://www.electronjs.org/docs/latest/tutorial/electron-timelines
- https://www.electronjs.org/docs/latest/tutorial/security
- https://www.electronjs.org/docs/latest/tutorial/performance
- https://registry.npmjs.org/electron · /electron-builder · /@electron-forge/cli · /node-pty
- https://github.com/microsoft/node-pty (read-only clone: README, history)
- https://betterstack.com/community/guides/scaling-nodejs/tauri-vs-electron-vs-deno-vs-electrobun/ (macOS benchmark, updated 2026-07-20)
- https://www.levminer.com/blog/tauri-vs-electron (2022; Windows; informal, noted but weighted low)
- https://github.com/tauri-apps/tauri/issues/5889 (memory counter-evidence)
- https://desktopinsights.com/tech/tauri
- https://github.com/migueldeicaza/SwiftTerm (read-only clone: README, `LocalProcess.swift`, tags, history)
- https://anthropic.com/news/apple-xcode-claude-agent-sdk
- https://github.com/swiftlang/swift-markdown (read-only clone)
- https://webteractive.co/blog/ghostty-and-libghostty-the-terminal-core-quietly-reshaping-the-ecosystem · https://registry.npmjs.org/ghostty-web
- https://xtermjs.org/
- https://svelte.dev/docs/svelte/getting-started · https://svelte.dev/docs/kit/project-types · https://svelte.dev/docs/svelte/stores · https://svelte.dev/docs/svelte/svelte-js-files · https://svelte.dev/docs/svelte/testing · https://svelte.dev/docs/ai/claude-plugin (through the official Svelte MCP)
- https://registry.npmjs.org/sv-router · /svelte-spa-router · /bits-ui · /@ark-ui/svelte · /melt · /@tanstack/svelte-query · /@biomejs/biome · /oxlint · /webdriverio · /@axe-core/playwright · /insta-related npm n/a
- https://crates.io/crates/pty-process · /markdown · /watchexec · /insta · /uuid · /semver · /tempfile · /tracing · /which · /tauri-build · /tauri-plugin-opener · /tauri-plugin-clipboard-manager · /tauri-plugin-dialog · /tauri-plugin-window-state
- https://playwright.dev/docs/api/class-electron
- Probe builds (scratchpad, 2026-10-19): SvelteKit 3.1.0 SPA vs Svelte 5.57.2 + Vite 8.3.4, the same two-route app; both pass `svelte-check --fail-on-warnings`.

---

## DRI decision: final choice

The recommendation is in PART 3 above: Tauri 2.12.4, plus Svelte 5 + Vite 8 with an in-house router (SvelteKit 3 SPA is an acceptable alternative), TypeScript 6.0.3, Rust stable, pnpm 12.14.0 and the pinned versions listed. Blueprint Prompt 2.4 says to record the final choice and the reason here. Overriding the recommendation is fine; the written reason is what the ADRs build on.

**Final choice (Billy, 2026-10-19): SvelteKit 3 SPA** — `@sveltejs/kit` 3.1.0 + `@sveltejs/adapter-static` 4.0.0 with the r1 §A4 config, on Svelte 5.57.2 and Vite 8.3.4. Reason: one stack, file routing and layouts, and shared conventions across all his projects; the size and CSP differences are small at this app's scale, and the inline bootstrap script is hash-pinned by Tauri. Every other layer is the PART 3 recommendation.

*Superseded note:* an earlier entry recorded the plain Svelte 5 + Vite recommendation after a "no preference" answer; Billy corrected that.

