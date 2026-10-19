# ADR-001: Desktop shell is Tauri 2.12.4

## Status
Proposed

## Date
2026-10-19

## Context

Foreman is a personal macOS app (Apple Silicon; macOS 26 and 27). It must:

- host an interactive terminal running the Claude Code TUI;
- spawn only allowlisted programs (SR-01, SR-02);
- confine every file write to a path policy (SR-30);
- run under a strict CSP with no network of its own (SR-20, SR-60);
- start in ≤ 1.5 s and stay ≤ 300 MB with 3 tabs (NFR-01, NFR-23);
- feel like a Mac app (Appendix A: vibrancy, transparent title bar).

The stack is fixed input from the Stage 2 packet; this ADR records the reasoning and the exit path.

## Decision

Use **Tauri 2.12.4** (`tauri` crate 2.12.4 and `tauri-build` 2.7.3, MSRV 1.90; `@tauri-apps/api` 2.12.4; `@tauri-apps/cli` 2.12.3), with the system WKWebView, built-in `windowEffects` (a Liquid Glass material plus a Visual Effect fallback) and `titleBarStyle: Overlay`.

- Only Rust spawns processes or touches files.
- The shell, fs and pty plugins are never added.
- Release builds are produced by `scripts/build-release` and ad-hoc signed (SR-64; no notarization, NFR-38).

Verified 2026-10-19:

- crates.io: `tauri` max stable 2.12.4 (2026-10-14); 3.0.0-alpha.5 exists;
- npm: `@tauri-apps/api` `latest` 2.12.4, `next` 3.0.0-alpha.3;
- tauri-utils 2.10.2 source: `windowEffects` materials, Liquid Glass on macOS 26+, `macOSPrivateApi` a no-op in 2.12.1+ (r1 §B).

## Alternatives considered

- **Electron 44.9.0.** Best terminal story (node-pty + xterm.js, as in VS Code). Lost on:
  - a bundle about 5.7× larger (323 vs 57 MB in the r7 macOS benchmark);
  - a new major every 8 weeks with three supported, so an upgrade about every 2 months;
  - a security model in which SR-01, SR-22 and SR-20 depend on the app applying a 20-item checklist correctly.
- **Native SwiftUI/AppKit + SwiftTerm.** Best native feel, no webview attack surface. Lost on solo build speed and on the web-shaped quality bar (Vitest, Playwright + axe, svelte-check). SwiftTerm's author notes it trails xterm.js on accessibility. It remains the **fallback if S-0 fails fundamentally**.
- **Tauri 3 alpha.** Not stable; no dated release plan found (UNVERIFIED: the GitHub releases page was blocked in r1).

## Consequences

- WebKit, not Chromium. Rendering and DevTools differ from Chrome, so Playwright's WebKit project is the closest automated proxy (ADR-015).
- `tauri-driver` doesn't support macOS, so real-app E2E uses `@wdio/tauri-service` (ADR-015).
- The Tauri-specific code is confined to `foreman-app` (ADR-003), which bounds any v3 migration.
- Memory is measured as the sum of the process tree (WebKit helpers included).

## Verification

- S-0 (Stage 5.3): three tabs with a real `claude` TUI for 30 min with total process-tree RSS ≤ 300 MB; cold start ≤ 1.5 s; Shift+Tab, a 5 KB paste, resize while streaming, ⌘K routing, ⌘C/⌘V, VoiceOver.
- `config::security_settings_snapshot` (SR-21).
- The CI release-build job.

## Revisit when

- S-0 fails fundamentally (TUI rendering or input unusable).
- Tauri 3 reaches stable and Tauri 2 security fixes stop.
- WKWebView regresses on a macOS major.

## Exit path

- **Tauri 3:** migrate `foreman-app` only; the core, the stores (behind `IpcClient`) and the UI components are unaffected.
- **SwiftUI + SwiftTerm:** `foreman-core` stays (Swift calls it over a C ABI or a small local helper binary). The UI and IPC client are rewritten. The file formats are unchanged, so projects carry over.
