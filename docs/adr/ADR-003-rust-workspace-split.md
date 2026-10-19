# ADR-003: Rust workspace split: foreman-core, foreman-app, foreman-sensor

## Status
Proposed

## Date
2026-10-19

## Context

Prompt 4.2 requires the domain core in Rust, unit- and property-tested. M1 (Stage 6) builds the core with **no Tauri dependency**, before any UI exists. The sensor (F-011, P2) must be a tiny binary Claude Code can exec in ≤ 10 ms. NFR-46 asks for clippy pedantic on the core, no `unwrap`, and `unsafe` forbidden.

## Decision

One Cargo workspace (resolver 3; shared `[workspace.lints]`):

| Crate | Path | Contents | May depend on | Must not |
|---|---|---|---|---|
| `foreman-core` | `crates/foreman-core` | Template/grammar, lock, rules, policy, resolver, step states, chains, the pure launch builders, fingerprint, packets, decisions, path policy (lexical), store (atomic I/O + fd walk), schema migrations, run-log export, F-003 harness tests | std, `rustix` =1.1.5, `comrak` =0.56.0, serde, `sha2`, `jiff`, `uuid`, `semver`, `thiserror`, optional `ts-rs` | `tauri*`, `tokio`, `portable-pty`, `notify`, process spawning, network |
| `foreman-app` | `src-tauri` | Tauri shell, IPC, spawn, PTY, re-arm, git, watcher, Doctor, sensor ingest, OS plugin wrappers, logging | core + the Tauri stack | network crates (ADR-016) |
| `foreman-sensor` | `crates/foreman-sensor` | Hook helper (P2) | std, `serde_json`, `rustix` | everything else |
| `fake-claude` | `crates/fake-claude` | Test double for `claude`; `publish = false`, never bundled | std, `serde_json` | — |

- `#![forbid(unsafe_code)]` in every crate.
- `clippy::unwrap_used` and `clippy::expect_used` are denied outside `#[cfg(test)]`; `clippy::pedantic` applies to `foreman-core`; `-D warnings` everywhere (SR-62).
- Core direct dependencies are exact-pinned (SR-61).
- `rust-toolchain.toml` pins channel **1.99.1**, the newest release tag in rust-lang/rust on 2026-10-19 (the floor is 1.90 for `tauri-build` 2.7.3; `comrak` 0.56.0 needs 1.89).
- The core is "functional core, imperative shell": I/O facts are gathered in `foreman-app` and passed in as values. The one exception is `store`, which does file I/O because atomicity and the path walk must be tested together (crash injection).

## Alternatives considered

- **A single crate (`src-tauri`) with modules.** Simplest, but the core couldn't be built and tested without the Tauri toolchain (WebKit, system libraries in CI). Nothing would stop a domain module from reaching for `AppHandle`, and M1 couldn't ship before M2.
- **Many small crates** (`foreman-template`, `foreman-rules`, `foreman-store`…). Stronger boundaries but more Cargo overhead, slower builds and more version bookkeeping for one engineer. Module visibility (`pub(crate)`) inside one core crate gives enough separation.
- **Core as a WASM module shared with the frontend.** It would let the UI evaluate rules locally, but it splits the source of truth and runs security decisions in the webview (T1). Rejected.

## Consequences

- `cargo test -p foreman-core` runs on any Rust host in seconds (Linux CI included), and the harness and goldens don't need macOS.
- Process-spawning code (`spawn`, `pty`, `git`) lives in `foreman-app`, so its integration tests run on macOS with the fake binary.
- `rustix` in the core gives safe syscalls (`openat`, `renameatx_np`, `F_FULLFSYNC`) without `unsafe` in our crates.

## Verification

- CI `core-deps`: `cargo tree -p foreman-core -e normal` must not contain `tauri`, `tokio`, `portable-pty`, `notify`, `reqwest`, `hyper`.
- CI `lints`: `cargo clippy --workspace --all-targets -- -D warnings`, plus pedantic on the core.
- A static test that `std::process::Command` and `tokio::process::Command` appear only in `foreman-app/src/spawn.rs` and `pty.rs`.

## Revisit when

- The core passes about 30 k lines or its build time hurts iteration: split `store` and `template` into their own crates.
- A second frontend (for example a CLI runner) needs the core.

## Exit path

Splitting a module out of `foreman-core` into its own crate is mechanical: the public APIs are already at module boundaries and no Tauri types cross them.
