# ADR-014: Strict CSP, one minimal `main-window` capability, AppManifest-declared commands, Rust-built webview with navigation denied, plugins called only from Rust

## Status
Proposed

## Date
2026-10-19

## Context

Revised `docs/requirements.md`:

- **SR-20:** the production CSP (directives below). Tauri adds compile-time **hashes for inline scripts** (SvelteKit's bootstrap) and **nonces for `<style>` tags** already in the HTML. `script-src` never gets `'unsafe-inline'`/`'unsafe-eval'`. `style-src` alone may add `'unsafe-inline'` if S-0 shows runtime styles need it. SvelteKit's `kit.csp` stays unset. A separate `devCsp`. `dangerousDisableAssetCspModification` off.
- **SR-21:**
  - `freezePrototype: true`;
  - `withGlobalTauri: false` (hygiene only);
  - `assetProtocol.enable: false`;
  - `capabilities: ["main-window"]`;
  - `dragDropEnabled: false`;
  - the devtools feature off in release;
  - no remote URLs;
  - IPC pattern `brownfield`, with the reason given here.
- **SR-22:** no plugin permissions in the webview. `main-window` grants only `core:event:allow-listen`, `core:event:allow-unlisten`, `core:window:allow-start-dragging`, `core:window:allow-internal-toggle-maximize`, and Foreman's commands. Channels need no permission.
- **SR-23:** `AppManifest::commands`; every command granted to `main-window` only.
- **SR-25 / SR-25a:** URL allowlist; files opened only via `/usr/bin/open -b <editor bundle id>` after the read policy, `.md`/`.json`/`.txt` only; reveal via `open -R`; opener plugin with `open_js_links_on_click(false)`.
- **SR-29:** a Rust-built webview with `on_navigation` allowing only the app origin (plus the dev server in debug) and `on_new_window` → Deny.
- **SR-56:** clipboard in Rust only; notifications carry only Foreman-generated text.

Checked in source, 2026-10-19:

- tauri-utils 2.10.2 `config.rs`: `csp`, `dev_csp`, `freeze_prototype`, `dangerous_disable_asset_csp_modification`, `with_global_tauri`, `AssetProtocolConfig`, window `drag_drop_enabled` (default `true`), `PatternKind::{Brownfield, Isolation}`.
- tauri 2.12.4:
  - `src/manager/mod.rs` collects inline-script hashes and style hashes/nonces into `script-src`/`style-src`;
  - `WebviewWindowBuilder::on_navigation(Fn(&Url) -> bool)` and `on_new_window(Fn(Url, NewWindowFeatures) -> NewWindowResponse)`;
  - permission `core:window:allow-internal-toggle-maximize` exists;
  - the `devtools` cargo feature exists.
- tauri-build 2.7.3: `AppManifest::commands`.
- tauri-plugin-opener 2.7.2: `Builder::open_js_links_on_click(bool)` (default `true`), `open_url`, `reveal_item_in_dir`.
- Clipboard 2.5.0 `read_text`/`write_text`; dialog 2.8.3 `pick_folder`; notification 2.7.0 `builder().show()`; window-state 2.5.0 `save_window_state`/`restore_state`.

## Decision

**`tauri.conf.json`:**

```json
{
  "app": {
    "withGlobalTauri": false,
    "windows": [],
    "security": {
      "csp": {
        "default-src": "'self'", "script-src": "'self'", "style-src": "'self'",
        "img-src": "'self' data:", "font-src": "'self'",
        "connect-src": "ipc: http://ipc.localhost",
        "object-src": "'none'", "base-uri": "'none'", "form-action": "'none'", "frame-ancestors": "'none'"
      },
      "devCsp": { "...": "same, plus http://localhost:5173 ws://localhost:5173 in connect-src" },
      "freezePrototype": true,
      "dangerousDisableAssetCspModification": false,
      "assetProtocol": { "enable": false },
      "pattern": { "use": "brownfield" },
      "capabilities": ["main-window"]
    }
  }
}
```

- **The window is built in Rust** (`app.windows` is empty) in `setup`: `WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))` with:
  - `.on_navigation(|u| is_app_origin(u) || (cfg!(debug_assertions) && is_dev_server(u)))`;
  - `.on_new_window(|_, _| NewWindowResponse::Deny)`;
  - `.disable_drag_drop_handler()` (the same effect as `dragDropEnabled: false`);
  - `titleBarStyle(Overlay)` and `windowEffects`.

  `is_app_origin` accepts only `tauri://localhost` / `http://tauri.localhost` and `ipc:`.
- **Release builds** don't enable tauri's `devtools` feature (CI checks `cargo tree -e features`).
- **Hashes and nonces:** Tauri injects the SvelteKit bootstrap's hash into `script-src` and nonces for existing `<style>` tags. `kit.csp` stays unset, so two systems don't fight.
- **Capability** `src-tauri/capabilities/main-window.json`:
  ```json
  { "identifier": "main-window", "windows": ["main"], "platforms": ["macOS"],
    "permissions": ["core:event:allow-listen", "core:event:allow-unlisten",
                    "core:window:allow-start-dragging", "core:window:allow-internal-toggle-maximize",
                    "allow-<each command in ipc::COMMANDS>"] }
  ```
  No `core:default`, no plugin permissions, no remote URLs.
- **Commands:** `build.rs` → `tauri_build::Attributes::new().app_manifest(AppManifest::new().commands(ipc::COMMANDS))` (ADR-004).
- **Plugins** are initialized in Rust and reached only through Foreman commands in `foreman-app::os`:
  - opener with `open_js_links_on_click(false)`;
  - `open_url` allows only `https://claude.ai/…` and `https://code.claude.com/docs/…` after URL parsing (scheme, exact host, no userinfo or port); terminal links follow ADR-006's confirmed policy;
  - files open via `foreman-sys::spawn` `Open(OpenForm::Editor{bundle_id, path})` → `/usr/bin/open -b <id> <abs path>` only for `.md`/`.json`/`.txt` that passed `check_read`; `Reveal` → `open -R`; "Open terminal here" → `open -b com.apple.Terminal <dir>`;
  - clipboard is read only on explicit commands (`paste_back_preview`, `pty_paste`), ≤ 1 MB, never logged;
  - notifications use fixed templates (`notify::body_template_snapshot`) and only focus the app when clicked.

  The JS plugin packages (`@tauri-apps/plugin-*`) are **not installed**.
- **Native confirmations** (SR-24a, `docs/contracts/ipc.md` IPC-R11). Every decision that grants authority is confirmed in a native macOS dialog raised by Rust through `tauri-plugin-dialog` in `foreman-app::os`, never by a webview button or a request field: launch confirmations (SR-19), config acknowledgement (SR-06), the P1 re-arm idle assertion (SR-13), Terminate, accepting an outside decision change (SR-40), writing root `CLAUDE.md` (SR-30), confirming an outside signature (SR-47), signing (SR-42), terminal links (SR-29), security-relevant Settings (tool paths, pass-through list, allowlisted key hashes, project `env` approval, login PATH), item-kind re-confirmation, project removal, Clear run history and the quit guard. The webview shows context (diffs, reasons, previews) and its buttons only *request* the action; Rust raises the dialog and acts on its answer, and Cancel returns `Cancelled` with no state change. A compromised webview can therefore request an action but can't approve it. The one residual is `pty_write` into a live session (SR-24, threat model RR-03). Tests: `ipc::security_confirmations_are_native` (no request type has a `confirm*`/`ack*`/`consent*`/`approved*`/`*Confirmed` field, plus one scripted-Cancel integration test per dialog), `settings::security_fields_require_native_confirm`.
- **Window effects and accessibility settings.** The webview detects Reduce Transparency and Increase Contrast with CSS media queries (`prefers-reduced-transparency: reduce`, `prefers-contrast: more`) and reports changes through the `settings_update` patch's display fields; Rust then turns the window's vibrancy (`windowEffects`) off. Rust never reads accessibility preferences directly.
- **Pattern: `brownfield`.** The isolation pattern guards IPC against compromised frontend dependencies by passing every message through a sandboxed iframe hook. Here every command already validates its input in Rust (SR-24), the webview holds no plugin permissions, the CSP forbids remote and inline code, and JS dependencies are policed by ADR-024. Isolation adds an iframe, per-message encryption, and a second HTML entry the CSP must cover, for little residual gain. The residual risk (a malicious dependency can call Foreman's commands and so *request* actions, but every approval is a native dialog it can't answer; it can type into a live session through `pty_write`) is recorded in the threat model.

**S-0 decision rule (MASTER-SPEC §9, SR-20):**

- If runtime styles (SvelteKit's `app.html` style attribute, Svelte transitions, xterm; bits-ui is not used, ADR-020) fail under `style-src 'self'` plus nonces, `style-src` alone gets `'unsafe-inline'`, and this ADR is amended to record the accepted risk (CSS injection without script execution).
- If SvelteKit's `version.json` focus check still fetches with `pollInterval: 0` (ADR-002, UNVERIFIED), `connect-src` gains `'self'`.

## Alternatives considered

- **The default capability (`core:default`) plus scoped plugin permissions.** Convenient, but forbidden by SR-22, and scopes can't express our checks (path policy, URL parsing, previews).
- **The isolation pattern.** See the reasoning above; reconsider if third-party UI code grows.
- **Windows declared in `tauri.conf.json`.** Can't attach `on_navigation`/`on_new_window` handlers, so the Rust builder is required for SR-29.
- **No CSP** (Tauri's default when `csp` is unset). Unacceptable (SR-20).

## Consequences

- Any new plugin use means a Rust wrapper command, which keeps the attack surface in one reviewed module.
- `style-src` strictness depends on S-0, with a bounded fallback.
- A missing hash breaks the page in release. The WDIO CSP test on the built app catches it.

## Verification

- `csp::production_config_snapshot`, `config::security_settings_snapshot`, `capabilities::main_window_snapshot`, `capabilities::every_command_declared_and_granted`.
- `webview::navigation_denied_to_remote`, `webview::window_open_denied`.
- `opener::only_allowed_urls`, `opener::open_file_refuses_executable_types` (including a `.md` symlink to an `.app`), `opener::uses_editor_bundle_id`, `clipboard::read_only_on_user_action`, `notify::body_template_snapshot`.
- A `@wdio/tauri-service` test on the built app reads the served CSP header and checks that an injected inline script raises `securitypolicyviolation` and doesn't run. S-0 CSP check.

## Revisit when

- S-0 forces the `style-src` fallback (amend this ADR).
- Tauri changes CSP injection or the capability format (Tauri 3).
- Foreman adds a second window, remote content, or substantial third-party UI code (reconsider isolation).

## Exit path

All grants live in one capability file and one command list, and the window is built in one function. Tightening or loosening is a reviewed diff with snapshot tests. On Tauri 3, the same model migrates file by file.
