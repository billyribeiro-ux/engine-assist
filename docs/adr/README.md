# Architecture Decision Records · Foreman

Written in blueprint Stage 4.2 (2026-10-19). Every ADR is **Proposed** until the Stage 4.7 independent design review, after which all are marked Accepted.

Each ADR has these sections, in order: Status, Date, Context, Decision, Alternatives considered, Consequences, Verification, Revisit when, Exit path.

The design overview is in `docs/design-doc.md`.

| ID | Title | Status | Milestone |
|---|---|---|---|
| [ADR-001](ADR-001-desktop-shell.md) | Desktop shell is Tauri 2.12.4 | Proposed | M0 |
| [ADR-002](ADR-002-frontend-framework-and-router.md) | Frontend is SvelteKit 3 in SPA mode with file-based routing | Proposed | M0 (M2 screens) |
| [ADR-003](ADR-003-rust-workspace-split.md) | Rust workspace split: foreman-core, foreman-sys, foreman-app, foreman-sensor | Proposed | M0 (M1 core) |
| [ADR-004](ADR-004-typed-ipc-and-type-sync.md) | Typed IPC layer: ts-rs types, a hand-written client contract, a mock, and contract tests | Proposed | M2 |
| [ADR-005](ADR-005-markdown-parsing-and-fm-grammar.md) | Blueprint parsing with comrak =0.56.0 and an owned `fm:` grammar parser | Proposed | M1 |
| [ADR-006](ADR-006-pty-and-terminal.md) | PTY via an own wrapper over portable-pty 0.9.0; xterm 6 fed by a raw-bytes Channel, with sanitized input and Rust-routed links | Proposed | M2 (first spawn), M3 |
| [ADR-007](ADR-007-process-spawning-and-environment.md) | Process spawning: one spawn module, allowlisted absolute programs, fixed flags, an explicit child environment, and a defined child lifecycle | Proposed | M2 |
| [ADR-008](ADR-008-session-chains-and-rearm.md) | Session chains with Foreman-owned UUIDs; re-arm by graceful end and relaunch, user-initiated only | Proposed | M3 |
| [ADR-009](ADR-009-session-sensor-per-launch-hooks.md) | Session sensor through per-launch hooks in exec form, writing token-tagged event files | Proposed | M4 (S-1′ in M0) |
| [ADR-010](ADR-010-state-layout-and-durable-atomic-writes.md) | State layout and durable atomic writes: temp + F_FULLFSYNC + renameatx_np swap, never over a newer edit | Proposed | M1 |
| [ADR-011](ADR-011-path-policy-implementation.md) | Path policy: a lexical check with NFC and case folding, plus a component-wise fd walk with rustix | Proposed | M1 |
| [ADR-012](ADR-012-git-integration.md) | Git through one `git::run` entry point over the git CLI; all commits by plumbing, with read-back | Proposed | M1 (signing M4) |
| [ADR-013](ADR-013-file-watching.md) | File watching with notify 8.2.0 + notify-debouncer-full 0.7.0 as a hint; files are always re-read and validated | Proposed | M3 |
| [ADR-014](ADR-014-csp-and-tauri-capabilities.md) | Strict CSP, one minimal `main-window` capability, AppManifest-declared commands, Rust-built webview with navigation denied, plugins called only from Rust | Proposed | M0 (runtime M3) |
| [ADR-015](ADR-015-testing-strategy.md) | Layered testing; real-app E2E is a thin WDIO smoke because tauri-driver doesn't support macOS | Proposed | M0 |
| [ADR-016](ADR-016-no-network-policy.md) | No-network policy enforced by cargo-deny bans on the macOS graph, CSP, git protocol lock, and a blocked-network test | Proposed | M0 (test M5) |
| [ADR-017](ADR-017-claude-code-version-floor-and-contract-probes.md) | Claude Code compatibility: a computed version floor checked every launch, zero-token contract probes on version change, and fake-claude contract tests | Proposed | M2 (floor), M4 (probes) |
| [ADR-018](ADR-018-run-log-metrics-and-diagnostics.md) | Run log as per-run JSON plus monthly JSONL indexes; metrics exported as Markdown; local redacted diagnostics | Proposed | M2–M3 (panel M4) |
| [ADR-019](ADR-019-typescript-6-pin.md) | Pin TypeScript 6.0.3 (strict) until the Svelte and ESLint toolchain accepts TypeScript 7 | Proposed | M0 |
| [ADR-020](ADR-020-ui-primitives.md) | UI primitives: hand-built and keyboard-complete, styled with Foreman's own tokens (bits-ui 2.19.5 was proposed, not adopted; amended 2026-10-20) | Proposed | M2 (M5 design system) |
| [ADR-021](ADR-021-launch-preflight-config-fingerprint.md) | One launch pipeline: fingerprint every Claude Code input, block auth-changing keys, check auth fail-closed, write a fresh run settings file | Proposed | M3 (run settings M2) |
| [ADR-022](ADR-022-decision-file-approval.md) | Decision files take effect only from Billy-approved copies held in app data | Proposed | M1 (review UI M2, live detection M3) |
| [ADR-023](ADR-023-schema-versioning-and-migrations.md) | Every file Foreman owns has an integer schemaVersion with pure, golden-tested forward migrations | Proposed | M1 |
| [ADR-024](ADR-024-js-supply-chain-pnpm-policy.md) | JS supply chain under pnpm 12: exact pins, frozen lockfile, allowBuilds, a 7-day minimum release age, no exotic sources, SHA-pinned CI actions | Proposed | M0 |

## Dependency log (SR-61: every new dependency has a line saying why)

| Dependency | Version | Added by | Why |
|---|---|---|---|
| `rustix` | 1.1.5 | ADR-010, ADR-011, ADR-006 | Safe `openat`/`O_NOFOLLOW(_ANY)`, `renameatx_np`, `F_GETPATH`, `kill`, `umask` without `unsafe`. The `event` feature (no new crate) is enabled in `foreman-sensor` only, for a 1 s `poll` deadline on the bounded stdin read (2026-10-20) |
| `icu_casemap` | 2.3.0 | ADR-011 | Full case folding for SR-30 |
| `icu_normalizer` | 2.3.0 | ADR-011 | NFC for SR-30 (already in the graph through `url`) |
| `ts-rs` | 12.0.1 | ADR-004 | Generated TS types (behind the `ts` feature; not in release) |
| `sha2` | 0.11.1 | ADR-010, ADR-005 | SHA-256 for hashes and stamps |
| `jiff` | 0.2.38 | ADR-018 | Timestamps and local dates |
| `tracing-subscriber` | 0.3.23 | ADR-018 | Local diagnostics with a redaction layer |
| `tauri-plugin-notification` | 2.7.0 | ADR-014 | `wait` steps and P2 notifications (Rust side only) |
| `@sveltejs/kit` / `@sveltejs/adapter-static` | 3.1.0 / 4.0.0 | ADR-002 | Frontend framework (Billy's decision) |
| `@types/node` | 22.20.5 | M0 (ADR-019) | Node types for tooling scripts, configs and Node-side tests only; webview code doesn't reference them |
| `tempfile` | 3.27.0 (dev) | M0 (ADR-015) | Temp dirs for `fake-claude` integration tests (already in r7's list for the store) |
| `proptest` · `insta` · `criterion` | 1.12.0 · 1.49.0 · 0.8.2 (dev) | M0 (ADR-015) | Layer-1 tooling wired in `foreman-core` (`criterion` with `default-features = false`) |
| `tauri-plugin-wdio-webdriver` | 1.5.0 (optional) | M0 (ADR-015) | WDIO smoke; only under the `e2e` feature, never in release (`release::no_webdriver`) |
| `@playwright/test` · `@axe-core/playwright` | 1.64.0 · 4.13.0 (dev) | M0 (ADR-015) | Layer-4 flows and axe (stepped back for the 7-day release age; ADR-024 log) |
| CI-only tools | cargo-deny 0.20.2 · cargo-audit 0.22.2 · cargo-cyclonedx 0.5.9 · gitleaks 8.30.1 · semgrep 1.180.0 | M0 (ADR-015, ADR-016) | Gates in `pnpm verify` and CI; installed with exact versions (`--locked`, checksum-verified for gitleaks), never linked into Foreman |
