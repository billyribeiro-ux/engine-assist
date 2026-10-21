# ADR-024: JS supply chain under pnpm 12: exact pins, frozen lockfile, allowBuilds, a 7-day minimum release age, no exotic sources, SHA-pinned CI actions

## Status
Proposed

## Date
2026-10-19

## Context

The webview runs every JS dependency with the power to call Foreman's commands, including typing into Claude sessions (`pty_write`; SR-24 and the threat posture). SR-61 requires:

- exact versions;
- `--frozen-lockfile` everywhere;
- install scripts disabled except an explicit list;
- a minimum release age of 7 days;
- no git or tarball dependencies;
- `pnpm audit` clean;
- GitHub Actions pinned by commit SHA with `permissions: contents: read`.

It also says "pnpm 12 setting names verified in 4.2".

Verified from the pnpm 12.14.0 package (npm `latest` 12.14.0; `CHANGELOG.md` in the tarball, 2026-10-19):

- **`allowBuilds`** (in `pnpm-workspace.yaml`) is the build-script allowlist. `onlyBuiltDependencies`, `onlyBuiltDependenciesFile`, `neverBuiltDependencies` and `ignoredBuiltDependencies` "were replaced by `allowBuilds` in pnpm 11 and silently ignored since". A newer `permissions` setting's `build` capability takes precedence over `allowBuilds` when present.
- **`minimumReleaseAge`** is in **minutes**: "the built-in 1440-minute default stays non-strict". Setting it explicitly makes **`minimumReleaseAgeStrict` default to `true`** (12.x). `minimumReleaseAgeExclude` exists.
- **`trustPolicy`** (for example `no-downgrade`) exists, with `trustPolicyExclude`.
- An unrecognized setting in `pnpm-workspace.yaml` **fails** with `ERR_PNPM_UNRECOGNIZED_WORKSPACE_SETTINGS` when the project pins a pnpm version the running pnpm satisfies, so misspelled policy keys can't silently drop policy.
- `pnpm install --frozen-lockfile` is the CI default.
- **`strictDepBuilds`** exists in 12.x (CHANGELOG: an install "no longer fails when that dependency's own dependencies have build scripts nobody approved … as it does without `strictDepBuilds`"), and unapproved builds fail with `ERR_PNPM_IGNORED_BUILDS` (CHANGELOG, `pnpm add`/`update`/`remove`). Its default value wasn't readable from the 12.14.0 package (the CLI ships as a native binary), so Foreman sets it explicitly (Stage 4.7 fix pass, checked 2026-10-19).

## Decision

- **`package.json`:**
  - `"packageManager": "pnpm@12.14.0"`;
  - every dependency at an exact version (no `^`/`~`), from the r7 PART 3 list plus `@sveltejs/kit` 3.1.0 and `@sveltejs/adapter-static` 4.0.0 (ADR-002), minus the unused `@tauri-apps/plugin-*` JS packages (ADR-014);
  - `"engines": { "node": ">=22.17" }`.
- **`pnpm-workspace.yaml`:**
  ```yaml
  minimumReleaseAge: 10080          # 7 days, in minutes; strict by default when set explicitly
  trustPolicy: no-downgrade
  strictDepBuilds: true             # an unapproved build script fails the install (ERR_PNPM_IGNORED_BUILDS)
  allowBuilds:                      # exact list fixed at M0 from `pnpm approve-builds`; default deny
    esbuild: true                   # example; only packages whose postinstall is reviewed
  ```
  The pinned pnpm makes unknown keys fatal, so a typo fails the install.
- **Exotic sources:** CI job `pnpm-policy` fails if `pnpm-lock.yaml` contains any `resolution` with `tarball:` from a non-registry host, `git`/`codeload`/`github:` sources, or `file:`/`link:` outside the workspace. A dedicated pnpm setting (`blockExoticSubdeps`, recalled from pnpm 10, not re-verified) **wasn't found** by a grep of the 12.14.0 `dist` or its CHANGELOG, so whether pnpm 12 has an equivalent is **UNVERIFIED**, and the lockfile scan is the control.
- **Commands:** `pnpm install --frozen-lockfile` in CI and in `scripts/build-release`; `pnpm audit --audit-level low` in CI (`pnpm-audit`).
- **CI:** every GitHub Action referenced by full commit SHA with a version comment; workflow-level `permissions: contents: read`; job-level escalation only where needed (for example the SBOM upload). CI job `actions-pinned` greps workflows for `uses: …@<40-hex>`.
- **Webview hardening that limits a bad dependency:** the strict CSP (no remote code, no `connect-src` beyond IPC), no plugin permissions, and Rust-side validation (ADR-014). The JS runtime dependency set is kept small: Svelte, SvelteKit, bits-ui, xterm and its addons, `@tauri-apps/api`.

## Alternatives considered

- **npm 12 or Bun.** No `allowBuilds`-style default-deny with a release-age gate in the same tool. pnpm is the fixed stack choice.
- **Vendoring all JS dependencies** (committed `node_modules` or a tarball mirror). Strongest reproducibility, but heavy churn and review burden for one engineer. The lockfile, release age and audit give most of the benefit.
- **Caret ranges with the lockfile as the pin.** The lockfile already pins, but exact manifest versions make intent explicit and stop accidental upgrades when the lockfile is regenerated.

## Consequences

- New package versions are usable only after 7 days, including security fixes. An urgent fix uses `minimumReleaseAgeExclude` with a dated reason in this ADR's log.
- `allowBuilds` must be reviewed whenever a dependency adds a postinstall script; CI fails until it's decided.
- Because setting `minimumReleaseAge` makes it strict, **every pinned JS version must be at least 7 days old on M0 day**; a pin newer than that fails `pnpm install`. The M0 pins are re-checked against their publish dates before the scaffold is committed, and a too-new pin is stepped back to the newest version that is old enough (recorded in this ADR's log).

## Verification

CI jobs `pnpm-policy` (lockfile scan, single TypeScript, exact versions), `pnpm-audit`, `actions-pinned`, `install` (`--frozen-lockfile`). The Stage 5 gate proves that a planted vulnerable dependency fails CI (SR-63), and that a planted dependency with a `postinstall` script (not in `allowBuilds`) fails install (AC-M0-17).

## Revisit when

pnpm 13 changes these settings, or a dedicated exotic-source block setting is confirmed in pnpm 12's docs.

## Exit path

All policy is configuration plus one CI script. Moving to another package manager means re-expressing the same four controls (pins, frozen lockfile, build allowlist, release age) in its terms.

## Log

**2026-10-20 · M0 scaffold (Stage 5).** `minimumReleaseAge: 10080` is strict, so every pin was checked against its npm publish time (cutoff 2026-10-13 12:15 UTC). Pins newer than 7 days were stepped back to the newest version that is old enough:

| Package | Planned (r7 / ADR-002) | Pinned at M0 | Planned version published | Notes |
|---|---|---|---|---|
| `@sveltejs/kit` | 3.1.0 | **3.0.1** | 2026-10-14 | Same peers (`vite ^8.0.12`, `svelte ^5.57.1`, `typescript ^6.0.0`); the ADR-002 config works unchanged |
| `@tauri-apps/api` | 2.12.4 | **2.12.3** | 2026-10-14 | Same 2.12 minor as the `tauri` crate (2.12.4) |
| `@tauri-apps/cli` | 2.12.3 | **2.12.2** | 2026-10-14 | |
| `vitest` · `@vitest/browser-playwright` | 5.0.5 | **5.0.4** | 2026-10-15 | |
| `playwright` · `@playwright/test` | 1.64.1 | **1.64.0** | 2026-10-14 | |
| `@axe-core/playwright` | 4.14.0 | **4.13.0** | 2026-10-15 | |
| `eslint` | 10.13.0 | **10.12.0** | 2026-10-16 | |
| `typescript-eslint` | 8.72.0 | **8.71.2** | 2026-10-19 | Peer `typescript >=4.8.4 <6.1.0`, so TS 6.0.3 still fits |
| `svelte-check` | 4.7.7 | **4.7.6** | 2026-10-16 | Peer `typescript ^5 \|\| ^6` |

Every other planned pin was already old enough and is used as planned. Re-pin to the planned versions once they are 7 days old (a one-line change each plus `pnpm install`).

- **`allowBuilds: {}`**: no package in the M0 tree has a build script (Vite 8 bundles with rolldown, so `esbuild` isn't in the tree). The example `esbuild: true` entry was not needed.
- **`trustPolicyExclude: [undici-types@6.21.0]`**: `trustPolicy: no-downgrade` rejected `undici-types@6.21.0` (published 2024-11-13, before that line had provenance). It is type declarations only, has no scripts, and is required by `@types/node` 22.x. Exact-version exclusion; re-check when `@types/node` moves to a newer `undici-types`.
- **Policy keys** live in `pnpm-workspace.yaml` (`saveExact`, `engineStrict` too); `.npmrc` keeps only the registry.
- **Proofs run on 2026-10-20** (`scripts/plants/run.sh`): a dependency with a `postinstall` script (`esbuild@0.25.0`) fails `pnpm add` with `ERR_PNPM_IGNORED_BUILDS`; a git-sourced package in the lockfile fails `scripts/check-lockfile-sources.mjs`; a known-vulnerable package (`minimist@1.2.5`) fails `pnpm audit`.

**2026-10-20 · Release-age exceptions (`minimumReleaseAgeExclude`).** Billy requires the latest SvelteKit and Tauri releases, and they are younger than the 7-day gate. `pnpm-workspace.yaml` therefore exempts these exact versions:

- `@sveltejs/kit@3.1.0` (published 2026-10-14 22:35 UTC; removable from 2026-10-21);
- `@tauri-apps/api@2.12.4` (published 2026-10-14 18:47 UTC; removable from 2026-10-21);
- `@tauri-apps/cli@2.12.3` (published 2026-10-14 19:50 UTC; removable from 2026-10-21), plus its 11 platform binaries `@tauri-apps/cli-<platform>@2.12.3`, published in the same release.

Scope: these four packages only, each a first-party release from the framework's own npm scope (`@sveltejs`, `@tauri-apps`), reviewed 2026-10-20. Every other package keeps the 7-day gate. The exemption relaxes SR-61's 7-day minimum for these entries only, and only until the expiry date.

Rule: pnpm 12.14.0 does not allow name patterns with versions in `minimumReleaseAgeExclude`, so the platform binaries are listed one by one. Verified 2026-10-20: `'@tauri-apps/cli-*@2.12.3'` fails with "Name patterns are not allowed with version unions".

Expiry: on or after 2026-10-21, confirm each version's age with `npm view <pkg> time --json`, delete the 14 entries, run `pnpm verify`, and add a log line here. The pinned versions stay; only the exemptions go.
