# ADR-016: No-network policy enforced by cargo-deny bans on the macOS graph, CSP, git protocol lock, and a blocked-network test

## Status
Proposed

## Date
2026-10-19

## Context

SR-60, NFR-39 and PR-01: Foreman makes no network requests of its own: no telemetry, update checks, crash upload, remote fonts or assets. "The Rust dependency tree contains no HTTP client; checked by `cargo deny`." SR-61 adds advisories, licenses and sources (crates.io only), `cargo audit` and `pnpm audit`. Opening a URL in the default browser isn't a Foreman request (SR-25).

Checked 2026-10-19:

- `tauri` 2.12.4 depends on `reqwest` 0.13, **but only** under `[target.'cfg(any(target_os = "android", all(target_vendor = "apple", not(target_os = "macos"))))'.dependencies]` (mobile dev-server proxying in `src/protocol/tauri.rs`). It isn't in the macOS graph.
- `tauri-plugin-wdio-webdriver` 1.5.0 depends on `axum` (an HTTP server on `127.0.0.1:4445`).
- `cargo-deny` 0.20.2 (2026-07-09) template: `[graph] targets = [...]`, `[bans] deny = [{ crate = "…", reason = "…" }]`.

## Decision

1. **`deny.toml`:**
   ```toml
   [graph]
   targets = ["aarch64-apple-darwin"]
   all-features = false
   no-default-features = false        # default features; `e2e` and `ts` are off
   [advisories]
   yanked = "deny"
   [licenses]
   allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
            "ISC", "Unicode-3.0", "Zlib", "MPL-2.0", "CC0-1.0"]   # final list fixed at M0 from the real graph
   [sources]
   unknown-registry = "deny"
   unknown-git = "deny"
   [bans]
   multiple-versions = "warn"
   deny = [
     { crate = "reqwest", reason = "SR-60: no HTTP client" },
     { crate = "hyper", reason = "SR-60" }, { crate = "hyper-util", reason = "SR-60" },
     { crate = "ureq", reason = "SR-60" }, { crate = "isahc", reason = "SR-60" },
     { crate = "curl", reason = "SR-60" }, { crate = "surf", reason = "SR-60" },
     { crate = "attohttpc", reason = "SR-60" }, { crate = "h2", reason = "SR-60" },
     { crate = "quinn", reason = "SR-60" }, { crate = "tungstenite", reason = "SR-60" },
     { crate = "axum", reason = "SR-60: only under the e2e feature" },
     { crate = "openssl", reason = "SR-60: no TLS stack needed" }, { crate = "rustls", reason = "SR-60" },
     { crate = "native-tls", reason = "SR-60" }, { crate = "git2", reason = "ADR-012" },
     { crate = "keyring", reason = "SR-51" }, { crate = "security-framework", reason = "SR-51: no keychain access" },
   ]
   ```

   **Verified on 2026-10-19:** a scratch workspace with the planned dependencies (tauri 2.12.4, tauri-build 2.7.3, portable-pty 0.9.0, notify 8.2.0, notify-debouncer-full 0.7.0, comrak 0.56.0, tokio 1.54.0, rustix 1.1.5, the five plugins, tracing-subscriber, jiff, sha2, uuid) was resolved with `cargo generate-lockfile`. `cargo tree --target aarch64-apple-darwin -e normal,build -i <crate>` reported nothing for **every** banned crate above, including `reqwest`, `hyper`, `hyper-util`, `rustls`, `openssl`, `security-framework` and `keyring`, across 383 crates. With `--target all`, `reqwest` appears (the mobile-only path), which is why the target filter is required.
2. CI job `deny`: `cargo deny --workspace check` with the default config. A second job, `deny-e2e`, runs with `--features e2e` and an `e2e` exception file that allows `axum`/`hyper` only as dependencies of `tauri-plugin-wdio-webdriver` (`wrappers`), proving the server enters only through that plugin.
3. **Webview:** CSP `connect-src ipc: http://ipc.localhost` and `default-src 'self'` (ADR-014); no remote fonts or images.
4. **Git:** `-c protocol.allow=never` on every call (ADR-012).
5. **JS:** `pnpm audit` in CI (policy details in ADR-024). No runtime package may call `fetch`/`XMLHttpRequest`/`WebSocket` to a non-IPC origin (the CSP blocks it; a Playwright test asserts zero requests outside `ipc:` during the P1 flow).
6. **Behavioral test** (SR-60): `network::blocked_network_run`, a full harness run plus the WDIO smoke under a `sandbox-exec` profile with `(deny network*)` (allowing only local Unix sockets the WebKit helpers need), on Billy's Mac before the M5 security gate, with no errors and full function. `sandbox-exec` is deprecated but still shipped. **UNVERIFIED** on macOS 27 (not testable from this Linux session). The fallback is a `pf` anchor denying the Foreman process's outbound traffic.
7. **JS supply chain:** ADR-024.
8. A new dependency needs a line in the ADR log saying why (SR-61). `cargo deny` licenses are reviewed at each stage gate.

## Alternatives considered

- **A runtime egress check only** (Little Snitch or the firewall). Catches behavior, not dependencies; manual and late. Kept as the behavioral test, not the main control.
- **Banning by feature flags instead of crates** (for example denying `tauri/native-tls`). Brittle: a transitive crate can bring an HTTP client without any tauri feature. Crate bans fail closed.
- **cargo-deny over all targets.** Would ban `tauri` itself, because of the mobile-only `reqwest`. The target filter is correct because Foreman ships only `aarch64-apple-darwin`.

## Consequences

- An upgrade that adds an HTTP crate to the macOS graph fails CI with a named reason, which forces a decision.
- `openssl`/`rustls` bans assume no crate needs TLS for local work. If a legitimate need appears, it's an ADR amendment.

## Verification

CI jobs `deny`, `deny-e2e`, `audit` (`cargo audit`), `pnpm-audit`; the Playwright zero-external-requests assertion; `network::blocked_network_run` at the M5 gate.

## Revisit when

- Tauri moves an HTTP client into the macOS graph.
- Foreman v1.1+ ever needs a network feature (it would need a spec change first).

## Exit path

The policy is configuration. Relaxing it for a specific crate is a `[bans]` exception with a `wrappers` scope and a reason, reviewed like code.

## Log

**2026-10-20 · M0.** `deny.toml` is in the repo. The license list was fixed from the real `aarch64-apple-darwin` graph: `MIT`, `Apache-2.0`, `BSD-3-Clause`, `Unicode-3.0`, `Zlib`, `MPL-2.0`, `CC0-1.0`. `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause` and `ISC` were dropped because no crate uses them (cargo-deny warns on unused allowances); `deny-e2e.toml` keeps `Apache-2.0 WITH LLVM-exception`, which only the `e2e` graph needs. Known duplicate versions inside the tauri 2.12.4 graph are listed in `[bans] skip` so `cargo deny` reports zero warnings while any new duplicate still warns. `deny-e2e.toml` allows `axum` only under `tauri-plugin-wdio-webdriver`, and `hyper`/`hyper-util` only under `axum`. `cargo audit` scans the whole lockfile regardless of target, so `.cargo/audit.toml` ignores two warnings (RUSTSEC-2024-0370, RUSTSEC-2024-0429) whose crates are only in the Linux GTK graph (checked absent from `cargo tree --target aarch64-apple-darwin`). `scripts/sandbox/deny-network.sb` and `scripts/no-network-run.sh` exist; `network::blocked_network_run` runs on macOS only and is still UNVERIFIED until Billy runs it.


**2026-10-20 · M1.** foreman-core's pinned dependencies (design doc §3.2; each checked as the newest stable on the crates.io index) add two licenses to `deny.toml` and `deny-e2e.toml`: `BSD-2-Clause` (comrak 0.56.0) and `Unicode-DFS-2016` (finl_unicode 1.5.0, comrak's Unicode tables). Both are OSI-approved and permissive. New duplicate versions come from the core pinning `sha2` 0.11.1 while `tauri-codegen` 2.7.3 still uses `sha2` 0.10 (with `digest`, `block-buffer`, `crypto-common`, `cpufeatures`), and comrak's `phf` 0.14 next to the tauri stack's `phf` 0.13; the older version of each is in `[bans] skip` with a reason. Dev-only: `jsonschema` 0.59.0 (`default-features = false`, no HTTP retrieval) validates the golden fixtures against the files.md schemas.
