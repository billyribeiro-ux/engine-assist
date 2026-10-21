# Dev notes · M0 (Stage 5 site prep)

How to run the scaffold, what was verified on 2026-10-20 in a Linux cloud container, and what is left for Billy's Mac.

## Run it

```bash
pnpm install --frozen-lockfile
pnpm dev:mock            # frontend in the browser against MockIpcClient (vite --mode mock, .env.mock)
pnpm tauri dev           # the app (Mac)
pnpm verify              # every gate (scripts/verify.mjs); --bail to stop at the first failure
pnpm verify:fast         # svelte-check, ESLint, tsc
pnpm test                # Vitest: unit (Node) + browser (Chromium) projects
pnpm test:flows          # Playwright flows against `vite preview --mode mock` (--project webkit|chromium)
pnpm bindings            # regenerate src/lib/ipc/bindings (cargo test -p foreman-app --features ts)
scripts/lint-plants/run.sh   # proves ESLint + clippy security rules fire (M0-03)
scripts/plants/run.sh        # Stage 5 gate plants: type error, secret, clippy, vulnerable deps, postinstall, reqwest, keyring, git source
pnpm release             # Mac only; see docs/runbooks/release.md
```

Tools `pnpm verify` expects on PATH: `cargo-deny` 0.20.2 and `cargo-audit` 0.22.2 (`cargo install --locked <tool> --version <v>`), `gitleaks` 8.30.1 (`brew install gitleaks`, or the release tarball checked against its published SHA-256 as in `.github/workflows/ci.yml`). A missing tool fails its step with the install command; nothing is skipped.

For browser-mode Vitest and Playwright without Playwright's own browser download, set `PLAYWRIGHT_CHROMIUM_PATH` to a Chromium binary. On the Mac, `pnpm exec playwright install chromium webkit` once is enough.

## Dependency freshness

- **Check:** `pnpm outdated` skips versions still inside the 7-day window. On 2026-10-20 it listed only `typescript` and `@types/node`, hiding six newer dev-tool releases. For the true latest, run `npm view <pkg> version`, and `npm view <pkg> time --json` for publish dates.
- **Drop the exceptions:** after 2026-10-21 22:36 UTC (6:36 PM ET; kit 3.1.0 was published 2026-10-14 22:35 UTC, the last of the three), delete the 14 `minimumReleaseAgeExclude` entries in `pnpm-workspace.yaml` (kit 3.1.0, `@tauri-apps/api` 2.12.4, `@tauri-apps/cli` 2.12.3 and its 11 binaries), run `pnpm verify`, and log the removal in ADR-024. The pinned versions don't change.
- **Pins vs latest (2026-10-20):**

| Package | Pinned | Latest | Status |
|---|---|---|---|
| `svelte` | 5.57.2 | 5.57.2 | current |
| `@sveltejs/kit` | 3.1.0 | 3.1.0 | current; exemption until 2026-10-21 |
| `@sveltejs/adapter-static` | 4.0.0 | 4.0.0 | current |
| `vite` | 8.3.4 | 8.3.4 | current |
| `@tauri-apps/api` | 2.12.4 | 2.12.4 | current; exemption until 2026-10-21 |
| `@tauri-apps/cli` | 2.12.3 | 2.12.3 | current; exemption until 2026-10-21 |
| `typescript` | 6.0.3 | 7.0.2 | held at 6.0.3 (ADR-019); TS 7 breaks svelte-check and typescript-eslint |
| `vitest` | 5.0.4 | 5.0.5 | one patch behind; eligible 2026-10-22 |
| `eslint` | 10.12.0 | 10.13.0 | one patch behind; eligible 2026-10-23 |
| `playwright` | 1.64.0 | 1.64.1 | one patch behind; eligible 2026-10-21 |
| `svelte-check` | 4.7.6 | 4.7.7 | one patch behind; eligible 2026-10-23 |
| `typescript-eslint` | 8.71.2 | 8.72.0 | one patch behind; eligible 2026-10-26 |
| `@axe-core/playwright` | 4.13.0 | 4.14.0 | one patch behind; eligible 2026-10-22 |

- **Paired packages move together:** `@playwright/test` with `playwright`; `@vitest/browser-playwright` with `vitest` (ADR-024 log).
- **Re-pin** a one-patch-behind package on or after its eligible date: change the version in `package.json`, run `pnpm install`, run `pnpm verify`, and note it in ADR-024.
- `@types/node` (pinned 22.20.5) is not in this table; `pnpm outdated` reports 26.6.5.

## Layout added in M0

- `Cargo.toml` workspace (resolver 3, edition 2024, shared `[workspace.lints]`), `clippy.toml` (`disallowed-methods`: `Command::new`/`CommandBuilder::new` and `std::fs` + `tokio::fs`), `rustfmt.toml`, `rust-toolchain.toml` (1.99.1), `deny.toml`, `deny-e2e.toml`, `.cargo/config.toml` (`TS_RS_EXPORT_DIR`), `.cargo/audit.toml`.
- `crates/foreman-core` (`#![forbid(unsafe_code)]`, `clippy::pedantic`; no runtime deps yet; proptest/insta/criterion wired), `crates/foreman-sys` (stub, links core), `crates/foreman-sensor` (stub, exits 0), `crates/fake-claude` (records argv/env/cwd; scripted `-v`, `auth status`, `--init-only`, output and exit codes).
- `src-tauri` (foreman-app, tauri 2.12.4): `ipc::COMMANDS` (`src/ipc/commands_list.rs`, shared verbatim with `build.rs` → `AppManifest::commands`), `app_bootstrap`, `Req<T>` (a malformed request is a typed `Invalid`, IPC-R1), `IpcError` (§3, complete), the Rust-built `main` window (`on_navigation` app-origin only, `on_new_window` → Deny, drag-drop off; macOS title bar under `cfg(target_os = "macos")`), `tauri.conf.json` (SR-20/21, ad-hoc signing), `capabilities/main-window.json` (SR-22 exactly).
- SvelteKit SPA at the repo root: `vite.config.ts` (ADR-002 config), `src/routes/+layout.ts` (`ssr = false`), Today placeholder (`+page.ts` load → `IpcClient`), `src/lib/ipc/` (`contract.ts`, `tauri.ts`, `mock.ts`, `errors.ts`, `index.ts`, `fixtures/`, ts-rs `bindings/` checked in).
- Tooling: `eslint.config.js`, `.prettierrc`, `tsconfig.json` (extends `$app/tsconfig`; strict, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), `vitest.config.ts`, `playwright.config.ts`, `pnpm-workspace.yaml` (ADR-024 policy), `.gitleaks.toml`, `.semgrep.yml`, `.github/workflows/ci.yml`.

## Verified here (Linux x86_64 container, 2026-10-20)

Rust ran with `RUSTUP_TOOLCHAIN=1.97.0`: rust 1.99.1 can't be downloaded here (static.rust-lang.org is blocked). The container's stable toolchain directory was symlinked as `1.97.0-x86_64-unknown-linux-gnu` so the override resolves without a download. Nothing in the code needs a feature newer than 1.90 (the workspace `rust-version`).

`pnpm verify`: **all 18 steps pass, zero warnings**:

| Step | Result |
|---|---|
| pnpm-policy | pass: exact versions, registry-only sources, one TypeScript (6.0.3) |
| svelte-check `--fail-on-warnings` | 0 errors, 0 warnings |
| eslint `--max-warnings 0` (type-aware) | clean |
| prettier `--check` | clean |
| tsc (strict + `noUncheckedIndexedAccess` + `exactOptionalPropertyTypes`) | clean; tooling scripts are type-checked too (`checkJs`) |
| vitest (unit + browser/Chromium) | 5 files, 13 tests pass (contract, mock coverage, errors, lint plants, Today component) |
| release-bundle | release build has no mock IPC marker, exactly 1 inline script (SvelteKit bootstrap), no inline styles |
| rustfmt `--check` | clean |
| clippy `--workspace --all-targets -D warnings` (and again with `--features ts`) | clean |
| cargo test `--workspace` | all pass (incl. `csp::production_config_snapshot`, `csp::dev_csp_allows_only_the_vite_dev_server`, `config::security_settings_snapshot`, `config::devtools_feature_off`, `config::drag_drop_disabled_in_rust_builder`, `capabilities::main_window_snapshot`, `capabilities::every_command_declared_and_granted`, `ipc::rejects_unknown_fields`(+ over the real invoke path), `ipc::commands_json`, `ipc::fixture_requests_deserialize`, `ipc::fixture_responses_roundtrip`, `webview::navigation_denied_to_remote` (predicate), `release::no_webdriver`, fake-claude ×5, core tooling) |
| bindings-drift | regenerating the bindings changes nothing |
| core-deps | foreman-core has no tauri/tokio/portable-pty/notify/reqwest/hyper; foreman-sys no tauri |
| cargo-deny (graph `aarch64-apple-darwin`) | advisories, bans, licenses, sources ok; also `deny-e2e.toml` with `--features e2e` |
| cargo-audit `--deny warnings` | ok (five Linux-only advisories ignored with reasons in `.cargo/audit.toml`; RUSTSEC-2026-0370, -0371 and -0372 (the zbus family) arrived with the notification plugin, M4-25) |
| pnpm audit `--audit-level low` | no known vulnerabilities |
| secret-scan (gitleaks 8.30.1, `dir`) | no leaks |
| license-check | 193 JS packages, all allowed; Rust licenses via cargo-deny |

Also run here:

- `scripts/lint-plants/run.sh`: all 7 plants caught (`{@html}`; `innerHTML`/`outerHTML`/`insertAdjacentHTML`; `@tauri-apps/api` outside `#lib/ipc`; `+page.server.ts`; `fetch`; `Command::new` in foreman-app; `std::fs` in `foreman-core::rules`).
- `scripts/plants/run.sh`: all 9 plants caught and reverted, each by its specific error (TS2322; gitleaks `leaks found`; clippy `needless_return`; `pnpm audit` on `minimist@1.2.5`; `cargo audit` RUSTSEC-2020-0071 on `time@0.1.45`; `ERR_PNPM_IGNORED_BUILDS` on `esbuild@0.25.0`; cargo-deny bans on `reqwest` and `keyring`; the lockfile scan on a git source).
- `pnpm exec playwright test --project chromium`: Today smoke + axe (0 violations) + no requests outside the app origin.
- `cargo test`/`clippy` for `foreman-app` compile on Linux (webkit2gtk-4.1); macOS-only window chrome is behind `cfg(target_os = "macos")`.

## Deviations and decisions to review

- **Pins stepped back** at M0 for the strict 7-day `minimumReleaseAge` (full table in ADR-024's log). Since 2026-10-20, `@sveltejs/kit` 3.1.0, `@tauri-apps/api` 2.12.4 and `@tauri-apps/cli` 2.12.3 are at their latest releases under the exemption list (ADR-024 log; "Dependency freshness" above). Six dev tools remain one patch behind: vitest, playwright, `@axe-core/playwright`, eslint, typescript-eslint, svelte-check.
- **`allowBuilds: {}`**: nothing in the tree needs a build script. **`trustPolicyExclude: [undici-types@6.21.0]`**, reason in ADR-024's log.
- **`app_bootstrap` is an M0 skeleton.** It returns first-run state. The contract's `interrupted: Vec<RunSummary>` and `capabilities: Capabilities` fields are left out because `docs/contracts/ipc.md` never defines `RunSummary` or `Capabilities`. **Contract gap**: define both in ipc.md (M2-01) before adding them. `ProjectId` is a string newtype without UUID validation until a command takes one.
- **Bindings path** is `src/lib/ipc/bindings/` (ADR-004, ipc.md §7, design doc §3.3), not `generated/`.
- **Rejections are `IpcFailure`** (an `Error` subclass that is also a structural `IpcError`: `code`, `message`, `detail`). typescript-eslint's `only-throw-error`/`prefer-promise-reject-errors` reject plain objects. The UI still branches only on `code`/`detail.kind`.
- **Integration-test files** (`tests/*.rs`) carry a file-level `#![allow(clippy::unwrap_used, clippy::expect_used)]` with a reason. `allow-unwrap-in-tests` covers only `#[test]` fns and `#[cfg(test)]`, not helpers in `tests/`. Tests that spawn or read files also allow `disallowed_methods` (design doc §3.2: test code may use `std::fs`). `fake-claude` allows `disallowed_methods` crate-wide (test double, ADR-003).
- **clippy `disallowed-methods`** has no per-module allowlist mechanism. The allowed homes (`foreman-sys::spawn`, `foreman-core::store`, `foreman-app::logging`, `foreman-sensor`) add a module-level `#[allow(clippy::disallowed_methods)]` when they're written (M1-04, M1-07, M2). Reviewers grep for that allow.
- **cargo-deny duplicates**: tauri 2.12.4's graph has 8 crates in two versions; they're listed in `[bans] skip` (older version each) so the output is warning-free while any new duplicate still warns. **License list** fixed from the real graph (ADR-016 log).
- **cargo-audit** scans all targets, so two warnings from Linux-only GTK crates are ignored with reasons (ADR-016 log). cargo-deny remains the target-filtered gate.
- **Capability `platforms: ["macOS"]`** (ADR-014): on Linux the ACL wouldn't grant `app_bootstrap`, so `tauri dev` on Linux would refuse the command. The product is macOS-only; the Rust tests use `mock_context`.
- **Runbook path** is `docs/runbooks/release.md` (ticket M0-10), not `docs/runbook-release.md`.
- **No git in scripts**: `pnpm verify`'s bindings-drift step compares content hashes of `src/lib/ipc/bindings` before and after regenerating. The CI job also uses `git diff --exit-code`.
- **CI jobs** are split so they cover exactly `pnpm verify` without running it twice: `verify` (static checks), `test` (vitest, release-bundle, cargo test, lint plants, Playwright WebKit), `bindings-drift`, `deny`, `deny-e2e`, `audit`, `pnpm-audit`, `pnpm-policy`, `actions-pinned`, `gitleaks` (history + tree), `semgrep` (p/rust, p/typescript, p/javascript + `.semgrep.yml`), `sbom` (cargo-cyclonedx + `pnpm sbom`, uploaded). Action SHAs were resolved with `git ls-remote` (checkout v7.0.1, setup-node v7.1.0, pnpm/action-setup v6.1.0, rust-cache v2.9.2, upload-artifact v7.0.2).

## UI against the mock (stage/05-ui)

What exists: every v1 screen (Today, first run, setup form and wizard, stage workspace with the chat and CLI step views, gate and signing, packet, outside change, Doctor, run log with metrics, settings, template health, assembler), the ⌘K palette, the terminal drawer (xterm 6.0.0) and every A.7 shortcut plus ⌘⇧↵, ⌘⇧V, ⌘D, ⌘⇧T, ⌘⇧E and ⌘1–⌘9. `pnpm dev:mock` shows the whole app; `?scenario=<name>` picks a world (`src/lib/ipc/mock/world.ts`: happy, firstRun, empty, blockedVersion, blockedAuth, configChanged, conflict, readOnly, brokenChain, outsideChange, tamper, stage1, stage4, stage5, invalidPacket) and `?dialog=cancel` makes every native dialog answer Cancel.

Tests: `src/lib/screens/*.svelte.test.ts` (each screen's primary action plus its empty, error, locked and not-applicable states, through `#lib/test/screen.ts`), `src/lib/render/render.svelte.test.ts` (the five `render::` tests), `src/lib/styles/styles.lint.test.ts` (tokens are a byte copy of `docs/design/tokens.css`, no hard-coded design values, every pair in `contrast-check.py` passes in both themes), `tests/flows/p1-loop.spec.ts` (keyboard only) and `tests/flows/a11y.spec.ts` (axe on every route, Night and Vellum). Fixtures regenerate with `FIXTURES_WRITE=1 pnpm exec vitest run --project unit src/lib/ipc/fixtures.test.ts`.

Contract gaps: the 67 `GAP:` notes once in `src/lib/ipc/types.ts` are settled in `docs/contracts/ipc.md` §2.4 (M2-01, stage/05-app). `types.ts` now only re-exports the ts-rs bindings generated from `src-tauri/src/ipc/`, the contract test compares `COMMAND_MAP` with Rust's `commands.json` (all 91) and every response fixture round-trips through its Rust type.

Deviations from the mockups, with reasons:
- No drawn traffic lights: the real window uses the macOS overlay title bar.
- bits-ui isn't used: its Command primitive (⌘K) ships static `style="display: contents;"` attributes that the CSP (`style-src 'self'`) reports as violations (evidence and the narrower scope of the problem: ADR-020 amendment 2026-10-20). Dialog, palette, segmented control, tabs and the strip are hand-built and keyboard-complete; dynamic sizes go through CSSOM.
- P2 tags on step rows are not shown; redacted config diffs have no context lines (the redacted diff carries none); setup lists writes per path rather than grouped; Settings doesn't show an invalid `claude` path by default (paths come only from Rust's picker).
- The mock's TradeLog world follows the mockups (commercial = no, payments undecided), not ipc.md §7.2's "all yes".
- Terminal dim text (SGR 2) uses `--term-muted` instead of 50% alpha, so it passes AA.

## Handoff to the Mac (2026-10-21)

Everything that can be built and tested in a Linux container is on `stage/05-site-prep`. The authoritative ticket state is in `docs/progress.md`.

- **Milestones:**
  - M1, M2 and M3 are `done` except for their exit gates (`billy`).
  - M4 has every ticket done except M4-03's mockup cross-check (Mac check 7) and the M4-35 gate.
  - M5's build tickets (M5-01…M5-07) are done; M5-08…M5-14 are Billy's stages.
- **What only Billy can do:**
  - "Waiting on Billy" in `docs/progress.md` lists every open CR and decision.
  - "Mac checks" in `docs/progress.md` lists 29 checks, each with the exact command or steps.
  - The runbooks are `docs/runbooks/p1-kill-check.md` (M3-29 Stage 8 inspection, M3-30 TradeLog Stages 1–2, M3-31 kill check) and `docs/runbooks/p2-finishes.md` (M5-08…M5-14).
- **Reviews:** an independent security review and code review ran on 2026-10-20. Every finding is fixed with a failing-first test or recorded as a follow-up; see the "Security review" and "Code review" sections of `docs/progress.md`.
- **`pnpm verify`:** has 20 steps. The `flows` step runs Playwright on WebKit and Chromium on macOS, and Chromium only elsewhere. On the Mac, run `pnpm exec playwright install chromium webkit` once.
- **Branches and PRs:**
  - PRs #1–#3 (Stages 2–4 docs) are still unmerged. Merge them in order before this branch.
  - `stage/05-site-prep` is based on `stage/04-drawings`.
  - The `stage/05-*` working branches are all merged into it, and none needs merging separately.
- **First session on the Mac:**
  1. `rustup show`, which installs 1.99.1.
  2. `pnpm install --frozen-lockfile`.
  3. `pnpm verify`.
  4. `pnpm tauri dev`.
  5. Then work the "Mac checks" list, then the spikes (`pnpm spikes`), then `docs/runbooks/p1-kill-check.md`.

## Left for Billy's Mac (M0 list)

1. **Rust 1.99.1**: `rustup show` in the repo installs it from `rust-toolchain.toml`. Then run `pnpm verify` without `RUSTUP_TOOLCHAIN`; fix anything new that 1.99.1's clippy reports.
2. **`pnpm tauri dev`** (AC-M0-01): first real launch. Watch for (a) the dev CSP: `devCsp` keeps `style-src 'self'`, and Vite's dev server injects `<style>` tags from JS, so dev styling may be blocked. That's an S-0 finding to record, not a reason to loosen the production CSP. (b) The overlay title bar and the window builder on macOS.
3. **`pnpm dev:mock`** in the Desktop browser pane (AC-M0-12).
4. **`pnpm release`** (AC-M0-11): ad-hoc signed build, install, the rollback drill, and two-build reproducibility hashes (runbook tables).
5. **M0-11**: `cargo test -p foreman-app --test network` (`network::blocked_network_run`, macOS-only) and `scripts/no-network-run.sh` on the scaffold app; record sandbox-exec vs pf in the runbook. The `.sb` profile syntax for Unix sockets is UNVERIFIED.
6. **M0-05 leftovers**: the real-app smoke is written (`e2e/`, section "Real-app smoke" below) but has never run against WKWebView. Run `pnpm test:e2e` on the Mac. Playwright's WebKit project also only runs on the Mac/CI.
7. **Spikes M0-12…M0-18 and the baseline M0-19**: by hand, per the tickets.
8. **CI first run**: push the branch and check every job on `macos-latest`. Likely first-run fixes: `pnpm why` / Playwright browser install time, the `cargo cyclonedx` output file location, and Semgrep registry packs needing network.
9. **Re-pin** the six one-patch-behind dev tools on their eligible dates, and drop the exemptions after 2026-10-21 (see "Dependency freshness").

## Real-app smoke (`pnpm test:e2e`, ADR-015 layer 5)

Three specs run against the built debug app in the real WKWebView, through an in-repo W3C WebDriver client (`e2e/webdriver.ts`) and `node:test`. WebdriverIO is not used: its tree failed the audit, license and trust gates (ADR-015 amendment 2026-10-20 has the evidence and the cost).

| Spec | Proves |
|---|---|
| `e2e/specs/01-launch.e2e.ts` | the app starts and reaches Today (EX-M2-03), on the app origin and the real IPC bridge, the `app_bootstrap` call succeeded (no start-up error banner), one window |
| `e2e/specs/02-csp.e2e.ts` | SR-20: an injected inline `<script>` does not run and raises an enforced `securitypolicyviolation`; the policy the app serves equals `app.security.csp` from `tauri.conf.json` (not `devCsp`), allowing only Tauri's generated script hash and style nonce, with no `'unsafe-inline'`/`'unsafe-eval'` in `script-src` and a hash for the bootstrap script |
| `e2e/specs/03-navigation.e2e.ts` | SR-29: `window.open` (`_blank`, `_self`), `location.assign` and clicked links to a remote URL change nothing (no new window, same document and URL); control: a same-origin navigation still reloads the app |

**Run it (Mac only).** `pnpm install --frozen-lockfile`, then `pnpm test:e2e`. It runs `tauri build --debug --no-bundle --features e2e` (builds the frontend, then the app at `target/debug/foreman-app`), then `node --test` over `e2e/specs/*.e2e.ts`, one app launch per spec file. `FOREMAN_E2E_SKIP_BUILD=1` reuses an existing build; `FOREMAN_E2E_APP=<path>` points at another binary; `CARGO_TARGET_DIR` is honored. On any other OS `pnpm test:e2e` prints `SKIPPED (macOS only)` and exits 0. Each launch gets a free loopback port through `TAURI_WEBDRIVER_PORT`.

**How it is wired** (checked against the published `tauri-plugin-wdio-webdriver` 1.5.0 crate and tauri-cli 2.12.3 sources, 2026-10-20):

- `src-tauri/Cargo.toml` feature `e2e = ["dep:tauri-plugin-wdio-webdriver"]` (optional dependency, `default = []`); `foreman_app::builder` registers `tauri_plugin_wdio_webdriver::init()` under `#[cfg(feature = "e2e")]`. The plugin runs an HTTP WebDriver server on `127.0.0.1:$TAURI_WEBDRIVER_PORT` (default 4445). It has no commands, so no capability permission is added (`wdio-webdriver:default` grants none, and `capabilities::main_window_snapshot` fails if any permission is added).
- `tauri build --debug` rather than `cargo build` because only the CLI adds `tauri/custom-protocol` (tauri-cli `build_options`). A plain `cargo build` binary loads the Vite dev server and `devCsp`, so it would test the wrong policy.
- Page scripts are plain strings (`e2e/helpers/scripts.ts`). The plugin runs a script body as an `async` function, so `await` and `return` work in `execute/sync`.
- The page's `connect-src` has no `'self'`, so it can't fetch its own headers. The served policy is read from the violation event's `originalPolicy` instead.
- `compareServedCsp` (`e2e/helpers/csp.ts`) is pure and unit-tested on every OS (`tests/e2e-helpers.test.ts`); the client is unit-tested against a fake server (`tests/e2e-client.test.ts`). If Tauri adds a token the spec doesn't expect, spec 2 fails with the exact directive; record it in ADR-014 rather than loosening the check.

**Dry run on Linux (2026-10-20, Xvfb + WebKitGTK, not WKWebView).** The plugin supports Linux, so the same suite ran against a Linux `tauri build --debug --no-bundle --features e2e` binary with `node --test`: CSP spec 3/3, navigation spec 6/6, launch spec 2/3. The one failure is expected there: `main-window.json` is `platforms: ["macOS"]`, so `app_bootstrap` is refused on Linux and Today shows its error banner, which is the assertion working. This validates the specs, the client and the plugin wiring, not WKWebView behavior; the first Mac run is still the real test.

**Known limits.**

- WebDriver script calls carry no user activation, so WebKit may refuse `window.open` and link clicks by itself. Spec 3 then still passes (outcome-based) but proves less; the rule itself is proven by `webview::navigation_denied_to_remote` and the handlers being installed in `window::build_main`. A passing control test shows navigation isn't just broken.
- The target is `https://example.invalid/` (reserved TLD), so even a regressed denial reaches no network.
- When the app starts writing state (M1+), `startApp` must pass an env pointing it at a temp state directory so the smoke never touches Billy's real data.
- The Today heading is the constant `TODAY_HEADING` in `e2e/helpers/scripts.ts`; update it there if M2 renames it.
- The client supports only what the specs use (CSS selectors, scripts, URL, window handles); extend `e2e/webdriver.ts` and its fake-server test together.

**Release builds never contain the plugin.** `e2e` is off by default and in `pnpm release`. Proofs: `cargo test -p foreman-app --test release` (`release::no_webdriver`: `cargo tree -e normal --target aarch64-apple-darwin` for the default graph has no `tauri-plugin-wdio-webdriver`, `axum` or `hyper`, and the same command with `--features e2e` does contain the plugin, so the check can fail); `scripts/build-release` repeats the check before building; CI greps `cargo tree -e features`; `deny.toml` bans `axum` and `hyper` outright and `deny-e2e.toml` allows them only as dependencies of the plugin (`wrappers`), which the `deny-e2e` job runs with `--features e2e`.

**Lockfile scan fix.** `scripts/check-lockfile-sources.mjs` treated `get-caller-file: 2.0.5` as a `file:` source. The pattern is now `(?<![\w-])(file|link):`; `tests/lockfile-sources.test.ts` covers both the false positive and real `file:`/`link:` sources.
