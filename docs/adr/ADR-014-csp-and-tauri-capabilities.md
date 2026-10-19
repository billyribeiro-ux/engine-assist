# ADR-014: Strict CSP, a minimal main-window capability, AppManifest-declared commands, plugins called only from Rust

## Status
Proposed

## Date
2026-10-19

## Context

Requirements:

- SR-20: strict production CSP, never `'unsafe-inline'`/`'unsafe-eval'`; a separate `devCsp`; `dangerousDisableAssetCspModification` off.
- SR-21: `freezePrototype: true`, `withGlobalTauri: false`, `assetProtocol.enable: false`.
- SR-22: the webview holds **no plugin permissions**; the main window's capability grants only event listen/unlisten, `start-dragging`, and Foreman's commands.
- SR-23: commands declared with `AppManifest::commands`.
- SR-25: URL allowlist.
- NFR-27: Tauri's security guidance in full.

The frontend is SvelteKit 3 SPA (ADR-002), whose `index.html` contains one inline bootstrap script (r1 E6).

Checked in source, 2026-10-19:

- `tauri-utils` 2.10.2 `config.rs` has `csp`, `dev_csp`, `freeze_prototype` (default false), `dangerous_disable_asset_csp_modification` (default `Flag(false)`), `with_global_tauri` (default false) and `AssetProtocolConfig`.
- `tauri-build` 2.7.3 `AppManifest::commands(&'static [&'static str])` generates `allow-$command`/`deny-$command`.
- Tauri appends hashes of local inline scripts and styles to the CSP at compile time when `csp` is set (r1 §B, Tauri CSP docs).
- Plugin Rust APIs exist for all five plugins, so the webview never needs them:
  - `tauri-plugin-opener` 2.7.2: `open_url`, `reveal_item_in_dir`;
  - `-clipboard-manager` 2.5.0: `write_text`, `read_text`;
  - `-dialog` 2.8.3: `pick_folder`;
  - `-notification` 2.7.0: `builder().show()`;
  - `-window-state` 2.5.0: `AppHandleExt::save_window_state`, `WindowExt::restore_state`.

## Decision

**`tauri.conf.json` `app.security`:**

```json
{
  "csp": {
    "default-src": "'self'",
    "script-src": "'self'",
    "style-src": "'self'",
    "img-src": "'self' data:",
    "font-src": "'self'",
    "connect-src": "ipc: http://ipc.localhost",
    "object-src": "'none'",
    "base-uri": "'none'",
    "form-action": "'none'",
    "frame-ancestors": "'none'"
  },
  "devCsp": { "...same...": "", "script-src": "'self'", "connect-src": "ipc: http://ipc.localhost http://localhost:5173 ws://localhost:5173" },
  "freezePrototype": true,
  "dangerousDisableAssetCspModification": false,
  "assetProtocol": { "enable": false },
  "capabilities": ["main"]
}
```

Also set `app.withGlobalTauri: false`.

- **Hashes:** Tauri injects the hash of SvelteKit's inline bootstrap into `script-src` at build time. No nonce or `'unsafe-inline'` is ever added by hand.
- **Fonts:** IBM Plex Sans Condensed is bundled locally (OFL). No remote origins anywhere.
- **Capability `src-tauri/capabilities/main.json`:**

  ```json
  { "identifier": "main", "windows": ["main"], "platforms": ["macOS"],
    "permissions": ["core:event:allow-listen", "core:event:allow-unlisten",
                    "core:window:allow-start-dragging", "allow-<each command in ipc::COMMANDS>"] }
  ```

  No `core:default`, no plugin permissions, no remote URLs.
- **Plugins** are initialized in Rust and called only from Foreman's own commands in `foreman-app::os`:
  - `open_url` accepts only `https://claude.ai/…` and `https://code.claude.com/docs/…`, after URL parsing (scheme, host exact match, no credentials or port);
  - reveal-in-Finder accepts only policy-checked paths;
  - the dialog runs in Rust and returns an opaque handle the UI can't forge.

  The JS plugin packages (`@tauri-apps/plugin-*`) are **not installed** (nothing in the webview could use them).
- **Single window** `main`. No other window or webview exists in v1. The isolation pattern isn't used: there are no third-party scripts, and the webview has no plugin permissions to protect.

**S-0 decision rule (MASTER-SPEC §9):**

- If bits-ui, Svelte transitions or xterm need inline styles that Tauri can't hash (runtime `style=` attributes set through the CSSOM are allowed by CSP; inline `<style>` elements inserted at runtime aren't), the fallback is `style-src 'self' 'unsafe-inline'` **only**. Scripts stay strict. This ADR is then amended to record the accepted risk: CSS injection without script execution.
- If SvelteKit's `version.json` check still fetches with `pollInterval: 0` (ADR-002, UNVERIFIED), `connect-src` gains `'self'`.

## Alternatives considered

- **The default capability (`core:default`) plus plugin permissions with scopes.** Convenient: the UI could call clipboard and opener directly. But SR-22 forbids it, and scopes can't express our checks (path policy, URL parsing, previews).
- **The isolation pattern.** Protects against compromised frontend dependencies tampering with IPC. Costs an extra iframe and encryption per call, and buys little when every command validates input in Rust and the webview has no plugin permissions. Reconsider if third-party UI code grows.
- **No CSP (Tauri's default when `csp` is unset).** Unacceptable (SR-20).

## Consequences

- Any new plugin use means a Rust wrapper command, which keeps the attack surface reviewable in one place.
- `style-src` strictness depends on S-0. The fallback is defined and bounded.
- A missing hash shows up as a broken page in the release build. A CI smoke test of the built `index.html` under the CSP catches it.

## Verification

- `csp::production_config_snapshot`, `config::security_settings_snapshot`, `capabilities::main_window_snapshot` (fails on any added permission), `capabilities::every_command_declared_and_granted`.
- `opener::only_allowed_urls` (including lookalike hosts, `http:`, userinfo, ports), `opener::reveal_requires_path_policy`.
- Playwright: an injected inline `<script>` doesn't execute. S-0 CSP check in the real WKWebView. The release-build smoke loads `index.html` with zero CSP violations.

## Revisit when

- S-0 forces the `style-src` fallback (amend this ADR).
- Tauri changes CSP hashing or the capability format (Tauri 3).
- Foreman adds a second window or any remote content.

## Exit path

All grants live in one capability file and one command list. Tightening or loosening is a reviewed diff with snapshot tests. On Tauri 3, the same model migrates file by file.
