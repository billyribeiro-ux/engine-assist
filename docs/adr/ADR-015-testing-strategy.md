# ADR-015: Layered testing; real-app E2E is a thin WDIO smoke because tauri-driver doesn't support macOS

## Status
Proposed

## Date
2026-10-19

## Context

Requirements:

- NFR-47: every rule (tags and conditions, packets, resolver, signing) has unit tests; the parser has goldens and property tests.
- NFR-48: the TradeLog harness.
- NFR-10/11: crash and kill tests.
- NFR-31: axe zero violations.
- SR tests are named in `docs/requirements.md`.

The official Tauri WebDriver docs say `tauri-driver` desktop support is Windows and Linux only ("macOS has no WKWebView driver tool"). `@wdio/tauri-service` with the embedded `tauri-plugin-wdio-webdriver` supports macOS (r1 §A3). CrabNebula's macOS driver needs a paid key. Real `claude` can't run in CI (subscription login).

## Decision

Six layers, cheapest first. Each SR and NFR test lives at the lowest layer that can prove it.

| Layer | Tools (pinned) | Scope | Runs |
|---|---|---|---|
| 1. Core | `cargo test` + `proptest` 1.12.0 + `insta` 1.49.0 | All of `foreman-core`: goldens on the real v2.2 blueprint, property tests (grammar round-trip, applicable path never drops core, argv invariants, signable gate), crash injection on `store`, the F-003 TradeLog harness (Stages 1–2 in M1, 1–5 in M4) | CI (Linux + macOS) |
| 2. App integration | Tauri `test` feature (`mock_builder`, `mock_app`, `get_ipc_response`, `assert_ipc_response`, verified in tauri 2.12.4 `src/test/mod.rs`) + `fake-claude` + fixture git repos | Commands, DTO validation and fuzz, spawn, PTY lifecycle, re-arm, git and signing, watcher, sensor ingest | CI (macOS) |
| 3. Component | Vitest 5.0.5 browser mode (`@vitest/browser-playwright` 5.0.5, `vitest-browser-svelte` 3.1.0) with `MockIpcClient`; `@tauri-apps/api/mocks` (`mockIPC` with `shouldMockEvents`, `clearMocks`) for the client adapter | Components, stores, keyboard registry, token contrast table (NFR-35), IPC contract (ADR-004) | CI |
| 4. Flow | `@playwright/test` 1.64.1, WebKit project, against `vite preview` with `VITE_IPC=mock`; `@axe-core/playwright` 4.14.0 | P1 keyboard-only loop, ⌘K (P2), focus order, axe, CSP injection test, reload on deep routes | CI |
| 5. Real-app smoke | `tauri-plugin-wdio-webdriver` 1.5.0 (cargo feature `e2e` only) + an in-repo W3C WebDriver client (`e2e/webdriver.ts`, Node 22, no dependencies) + `node:test`. *Amended 2026-10-20: replaces `@wdio/tauri-service` 1.5.0 + webdriverio 9.32.0; see the amendment below.* | About 5 paths in the real WKWebView with fake-claude: launch, open project, run a step, packet lands, sign (P2) | Locally on Billy's Mac, before each stage gate |
| 6. Benchmarks | `criterion` 0.8.2 | NFR-02/03/04, sensor exit time | Locally + CI trend (no gate on shared runners) |

- **fake-claude** (`crates/fake-claude`):
  - records argv, cwd and env to a JSON file named by `FOREMAN_RUN_ID`;
  - plays scripted TUI output (alternate screen, colors);
  - honors resize;
  - responds to `\x03` and `/exit`;
  - emulates `-v`, `auth status` (each `authMethod`) and `--init-only` outcomes;
  - writes transcript files.
- **Spikes** with a real `claude` (S-0, S-1′, S-3, S-7′, S-8, S-12, S-23) run on Billy's Mac in M0 and produce `docs/spikes/S-*.md`.
- The `e2e` feature is never in release builds. CI check `release::no_webdriver` greps the release binary's linked crates (`cargo tree -e normal --no-default-features --features …`) and fails if `tauri-plugin-wdio-webdriver` or `axum` appears.
- CI also runs (SR-63, SR-61):
  - `pnpm verify` (svelte-check `--fail-on-warnings`, eslint, prettier `--check`, vitest);
  - `cargo fmt --check`, clippy with `disallowed-methods`, `cargo deny`, `cargo audit`;
  - `pnpm audit` and `pnpm-policy` (ADR-024);
  - Semgrep (Rust and TypeScript rule packs), a gitleaks secret scan, and CycloneDX SBOMs for cargo and pnpm;
  - `actions-pinned`, and the bindings drift check.

  The SR-20 CSP proof (injected inline script raises `securitypolicyviolation`) runs in layer 5 against the built app, because the CSP header is injected by Tauri, not by the Vite preview. The Stage 5 gate proves that a planted type error, a planted secret and a vulnerable dependency each fail CI.

## Alternatives considered

- **`tauri-driver`.** No macOS support.
- **CrabNebula's driver.** Paid on macOS; an external service dependency.
- **Playwright `_electron`-style driving of the real app.** Not applicable to WKWebView.
- **Real `claude` in CI.** Needs a subscription login in CI (not allowed by the brief) and costs usage. The contract is covered by fake-claude, version floors and contract probes.

## Consequences

- WKWebView-only bugs are caught by layer 5 and the spikes, not CI. Layer 4's WebKit project is a close proxy (r1 noted this as UNVERIFIED against WKWebView; S-0 checks the CSP and key-routing differences in the real app).
- The mock client must be kept in step with Rust; ADR-004's contract test enforces it.

## Verification

- Coverage reports for `foreman-core` (line ≥ 90 %, rules modules 100 % of branches), reviewed at the M1 gate.
- Every SR test name in `docs/requirements.md` exists in the tree; `scripts/check-sr-tests` greps for them at each stage gate.

## Revisit when

- Tauri ships official macOS WebDriver support.
- WDIO's plugin goes unmaintained.

## Exit path

Layer 5 is replaceable on its own: its five smoke paths are scripted against WebDriver, so any compliant driver works. If none is available, layer 5 becomes a manual checklist run at stage gates.

## Amendment 2026-10-20: the in-repo WebDriver client replaces WebdriverIO

**Decision.** Layer 5 keeps `tauri-plugin-wdio-webdriver` 1.5.0 under the `e2e` feature, but the client is a small in-repo W3C WebDriver client (`e2e/webdriver.ts`, about 210 lines with comments, no dependencies, global `fetch` in Node 22) driven by `node:test`. `@wdio/tauri-service`, webdriverio and the other `@wdio/*` packages are not installed. This is the exit path this ADR already describes ("any compliant driver works"), taken at Stage 5 instead of later.

**Evidence** (installing the pinned stack, `@wdio/tauri-service` 1.5.0 with `webdriverio`, `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework` 9.32.0 and `@wdio/globals` 9.31.3, into a copy of the repo with the real `pnpm-workspace.yaml` policy):

- **Trust:** `pnpm install` fails with a trust downgrade on `@wdio/repl@9.16.2`, which every webdriverio 9.x pins. It needs a `trustPolicyExclude` entry.
- **Build scripts:** `strictDepBuilds` fails on `esbuild@0.28.2` (via `tsx`), `edgedriver@6.3.1` and `geckodriver@6.1.1` (driver downloaders). None is needed for the embedded provider.
- **Audit:** `pnpm audit --audit-level low` reports 6 findings (5 high, 1 moderate): `serialize-javascript@6.0.2` (RCE, fixed in 7.0.3 and 7.0.5; via mocha), `basic-ftp@5.3.1` (fixed in 6.2.1), `extract-zip@2.0.1` (two path-traversal advisories, no fix) and `braces@3.0.3` (no fix). All arrive through the dev tooling.
- **Licenses:** `scripts/license-check.mjs` rejects `@promptbook/utils` (CC-BY-4.0), `argparse` (Python-2.0), `css-value` (no license field), `spdx-exceptions` (CC-BY-3.0) and `stream-buffers` (Unlicense).
- **Size:** about 490 packages for a three-spec smoke test, against a tree of 193 today.

Clearing those gates would have meant one trust exception, two or more overrides, an audit ignore list and five license allowances, all for a tool that never ships, in a project whose stated posture is a short, audited dependency tree (SR-61, ADR-024). The service's second half (`tauri-plugin-wdio` and `@wdio/tauri-plugin`) was already unusable here, because it needs `withGlobalTauri: true` and a `wdio:default` capability that SR-21 and SR-22 forbid, so only plain WebDriver commands were ever going to be used.

**What the client does.** `GET /status` (wait for `ready`), `POST /session`, `DELETE /session/{id}`, `GET|POST /url`, `POST /execute/sync` and `/execute/async`, `POST /element` (CSS), `GET /element/{id}/text` and `/displayed`, `GET /window/handles`. Endpoints, port variable (`TAURI_WEBDRIVER_PORT`, default 4445; the runner picks a free port instead) and response shapes were read from the published `tauri-plugin-wdio-webdriver` 1.5.0 crate (`server/router.rs`, `handlers/`). `e2e/app.ts` spawns the debug binary with an argv array and a free port and kills it afterwards. The client is unit-tested against a fake server in `tests/e2e-client.test.ts`.

**What it costs.**

- We maintain the client ourselves. Its surface is only what the specs use; adding a command means adding about 5 lines. WebdriverIO's retries, reporters, `browser.tauri.*` mocking and log capture are not available (the last three were unusable anyway).
- Fewer protocol conveniences: no automatic element re-finding, only CSS selectors, no actions or alerts. `waitUntil` and `waitForDisplayed` cover the specs' needs.
- If the plugin's wire behavior changes in a release, the client is ours to fix. The plugin is pinned at 1.5.0, and the Mac run before each stage gate catches drift.
- The first run on WKWebView is still unproven; the same suite was run on Linux under Xvfb (WebKitGTK) against the same plugin server, which exercises the client end to end.

**Other documents.** References to `@wdio/tauri-service` in the design doc, scope, MASTER-SPEC, ADR-001, ADR-002, ADR-014, `docs/requirements.md` (SR-20) and the threat model mean "the layer 5 real-app smoke" and are otherwise unchanged. `docs/design-doc.md` §3.2 and §6 are updated; the pins for webdriverio and the `@wdio/*` packages are withdrawn.

