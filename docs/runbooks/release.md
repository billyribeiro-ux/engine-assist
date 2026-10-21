# Runbook · Release build, install and rollback

Ticket M0-10 (AC-M0-11, SR-64, NFR-38) and M0-11 (AC-M0-10). Applies to Billy's Mac (Apple Silicon, macOS 26/27). Nothing here runs in CI.

## Build and install (`pnpm release`)

```bash
pnpm install --frozen-lockfile   # only needed on a fresh clone
pnpm release                      # = scripts/build-release
```

`scripts/build-release`:

1. Refuses to run anywhere but Apple Silicon macOS.
2. Uses the toolchain from `rust-toolchain.toml` (1.99.1) and the lockfiles (`pnpm install --frozen-lockfile`, `cargo … --locked`).
3. Runs `pnpm verify`; any failure stops the release.
4. Builds with `pnpm tauri build --target aarch64-apple-darwin --bundles app -- --locked`. Ad-hoc signing comes from `bundle.macOS.signingIdentity: "-"` in `src-tauri/tauri.conf.json` (Tauri docs, "macOS Code Signing", ad-hoc section).
5. Checks the bundle: `codesign --verify --deep --strict`, `Signature=adhoc`, and that neither `tauri-plugin-wdio-webdriver` nor `axum` is in the release graph (`release::no_webdriver`).
6. Prints the binary's SHA-256 and the commit.
7. Asks before installing. On **y**, the current `/Applications/Foreman.app` is moved to `/Applications/Foreman-previous.app` (replacing any older one), then the new build is copied with `ditto`. On anything else, nothing is installed.

`scripts/build-release --no-install` stops after step 6.

The first launch of an ad-hoc signed app may need approval in System Settings → Privacy & Security (Tauri docs: ad-hoc signing doesn't remove that prompt).

## Rollback to `Foreman-previous.app`

1. Quit Foreman (⌘Q; the quit guard ends any live sessions after a native confirmation).
2. In Terminal:
   ```bash
   mv /Applications/Foreman.app /Applications/Foreman-broken.app
   mv /Applications/Foreman-previous.app /Applications/Foreman.app
   open /Applications/Foreman.app
   ```
3. Check the version (Settings → About, or `defaults read /Applications/Foreman.app/Contents/Info.plist CFBundleShortVersionString`).
4. App data in `~/Library/Application Support/dev.foreman/` is shared by both builds. Files use forward-only schema migrations (ADR-023): if the newer build migrated a file to a schema the older build doesn't know, the older build opens that project read-only (`NewerSchema`) instead of corrupting it. Keep `Foreman-broken.app` until the cause is understood, then delete it.

**Rollback check (AC-M0-11):** run it once at the Stage 5 gate. Install build A, release build B (A becomes `Foreman-previous.app`), roll back, and confirm the app that opens reports A's version. Record the result in `docs/metrics-baseline.md`'s Stage 5 notes.

## Reproducible build

Inputs fixed by the script: `rust-toolchain.toml`, `Cargo.lock --locked`, `pnpm-lock.yaml --frozen-lockfile`, `SOURCE_DATE_EPOCH` = the commit time, `CARGO_INCREMENTAL=0`, `--remap-path-prefix` for the checkout and `~/.cargo`, `codegen-units = 1` and `lto = true` in the release profile.

**Check (M0-10 AC):** build twice from the same commit (`scripts/build-release --no-install`, moving the first `Foreman.app` aside) and compare the printed binary SHA-256 values. **Not measured yet** (it needs the Mac). If they differ, record the cause here (expected candidates: the ad-hoc code signature, which embeds a code-directory hash and can't carry a timestamp, and the `Info.plist` build metadata), and compare `shasum` of the binary after `codesign --remove-signature` on copies.

| Date | Commit | Build 1 SHA-256 | Build 2 SHA-256 | Same? | Documented difference |
|---|---|---|---|---|---|
| | | | | | |

## No-network runs (M0-11, AC-M0-10, ADR-016 decision 6)

```bash
scripts/no-network-run.sh cargo test --workspace
scripts/no-network-run.sh /Applications/Foreman.app/Contents/MacOS/foreman-app
```

`scripts/no-network-run.sh` first proves the profile works (an outbound `curl` must fail under it) and refuses to continue if it doesn't. The profile is `scripts/sandbox/deny-network.sb`: everything allowed except IP networking; local Unix sockets stay open for WebKit's helper processes. `cargo test` under the profile needs a warm cargo cache (`cargo fetch` first) because cargo itself can't reach crates.io inside the sandbox.

**Fallback if `sandbox-exec` is missing or ineffective on macOS 27** (pf can filter by user, not by process): create a throwaway local user `foremantest`, then

```bash
echo 'block drop out quick proto { tcp udp } user foremantest' | sudo tee /etc/pf.anchors/foreman-deny
sudo pfctl -a foreman-deny -f /etc/pf.anchors/foreman-deny
sudo pfctl -E
sudo -u foremantest /Applications/Foreman.app/Contents/MacOS/foreman-app
# afterwards
sudo pfctl -a foreman-deny -F all
```

The main `/etc/pf.conf` must reference the anchor (`anchor "foreman-deny"`) for the rule to load. Record which mechanism was used:

| Date | macOS | Mechanism (sandbox-exec / pf) | What ran | Result |
|---|---|---|---|---|
| | | | | |
