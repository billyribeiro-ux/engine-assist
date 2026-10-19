# ADR-003: Rust workspace split: foreman-core, foreman-sys, foreman-app, foreman-sensor

## Status
Proposed

## Date
2026-10-19

## Context

Prompt 4.2 requires the domain core in Rust, unit- and property-tested. M1 (Stage 6) builds the core with **no Tauri dependency**, before any UI exists. The revised `docs/requirements.md` adds more:

- SR-44: all git use goes through one `git::run`, **built in Stage 6** (M1), for blueprint reads and P1 decision-file commits.
- The spawn controls (SR-01 to SR-05, SR-09, SR-57) arrive in Stage 7 (M2).
- SR-02 and SR-31a: clippy `disallowed-methods` confine `Command::new` and `portable_pty::CommandBuilder::new` to the `spawn` module, and `std::fs` to the fs module.

The sensor (F-011, P2) must be a tiny binary that execs in ≤ 10 ms. NFR-46: clippy pedantic on the core, no `unwrap`, `unsafe` forbidden.

## Decision

One Cargo workspace (resolver 3; shared `[workspace.lints]`; `clippy.toml` with `disallowed-methods`):

| Crate | Path | Contents | May depend on | Must not |
|---|---|---|---|---|
| `foreman-core` | `crates/foreman-core` | Pure domain (template/grammar, lock, rules, policy, resolver, step states, chains, the pure argv/env/git-args builders, fingerprint and redaction, packets, decisions, lexical path policy, schema migrations, run-log export, the F-003 harness) **plus** `store` (the fd-walk resolver and atomic writes: the only module allowed to touch `std::fs`) | std, `rustix` =1.1.5, `comrak` =0.56.0, serde, `sha2`, `jiff`, `uuid`, `semver`, `thiserror`, `icu_normalizer` =2.3.0, `icu_casemap` =2.3.0, optional `ts-rs` | `tauri*`, `tokio`, `portable-pty`, `notify`, any process spawning, network |
| `foreman-sys` | `crates/foreman-sys` | Host integration without Tauri: `spawn` (the **only** module that constructs a `Command` or `CommandBuilder`; the program allowlist; the environment application), `git::run` (M1), the `claude` CLI wrappers (`-v`, `auth status`, probes), the `PtySession` lifecycle with a byte-sink callback, and `open` (`/usr/bin/open` forms, SR-25a) | core, `tokio`, `portable-pty` 0.9.0, `rustix` | `tauri*`, network |
| `foreman-app` | `src-tauri` | Tauri shell, IPC commands and DTOs, Channel and event wiring, watcher, sensor ingest, Doctor UI logic, Rust-side plugin wrappers, logging | core, sys, the Tauri stack, `notify` | direct `Command`/`std::fs` use (clippy-denied) |
| `foreman-sensor` | `crates/foreman-sensor` | Hook helper (P2) | std, `serde_json`, `rustix` | everything else |
| `fake-claude` | `crates/fake-claude` | Test double for `claude`; `publish = false`, never bundled | std, `serde_json` | — |

- `#![forbid(unsafe_code)]` in every crate. `clippy::unwrap_used` and `clippy::expect_used` are denied outside `#[cfg(test)]`. `clippy::pedantic` applies to `foreman-core`. `-D warnings` everywhere (SR-62).
- `clippy.toml` `disallowed-methods`: `std::process::Command::new`, `tokio::process::Command::new` and `portable_pty::CommandBuilder::new` are allowed only in `foreman-sys::spawn` (module-level `#[allow]`), and `std::fs::*` only in the modules `clippy.toml` lists by path: `foreman-core::store` (including `store::outside` and `store::git_exclude`), `foreman-app::logging` (the size-rotating diagnostics writer, `logs/` in app data only), the `foreman-sensor` crate (its event-file writer), and test code (`#[cfg(test)]`, `tests/`). Every other module gets the lint error; the list is pinned by AC-XC-30.
- Core direct dependencies are exact-pinned (SR-61).
- `rust-toolchain.toml` pins channel **1.99.1**, the newest release tag in rust-lang/rust on 2026-10-19 (floors: 1.90 for `tauri-build` 2.7.3; 1.89 for `comrak` 0.56.0).
- The core is "functional core, imperative shell": facts needing processes or the clock are gathered by `foreman-sys`/`foreman-app` and passed in as values. `store` is the one I/O exception in the core, because atomicity, the path walk and crash injection must be tested together.

## Alternatives considered

- **Three crates (core, app, sensor), with spawning inside `foreman-app`.** This was this ADR's first draft. It lost when SR-44 moved `git::run` into Stage 6: M1 would need Tauri to exist just to run git, or git would leak into the core and break "the core never spawns".
- **A single crate (`src-tauri`) with modules.** The core couldn't be built and tested without the Tauri toolchain, nothing would stop a domain module from reaching for `AppHandle`, and M1 couldn't ship before M2.
- **Many small crates** (`foreman-template`, `foreman-rules`, `foreman-store`…). Stronger boundaries but more Cargo overhead and version bookkeeping for one engineer. Module visibility plus clippy `disallowed-methods` gives enough separation.
- **Core as a WASM module shared with the frontend.** It splits the source of truth and runs security decisions in the webview (trust boundary T1). Rejected.

## Consequences

- `cargo test -p foreman-core` runs on any Rust host in seconds (Linux CI included); goldens and the harness don't need macOS.
- `foreman-sys` integration tests (git fixtures, fake-claude, PTY) run without a webview, on macOS CI.
- Exactly one module can start a process and exactly one can touch the filesystem, which makes SR-02 and SR-31a reviewable by grep and enforceable by clippy.

## Verification

- CI `core-deps`: `cargo tree -p foreman-core -e normal` contains no `tauri`, `tokio`, `portable-pty`, `notify`, `reqwest` or `hyper`. CI `sys-deps`: `foreman-sys` contains no `tauri`.
- CI `lints`: `cargo clippy --workspace --all-targets -- -D warnings` with `disallowed-methods` (this is the SR-02 test and `git::single_entry_point`).
- A static test that `-c`/`sh` construction appears only in `doctor::capture_login_path` (`spawn::no_shell_in_any_code_path`).

## Revisit when

- The core passes about 30 k lines, or build time hurts iteration: split `store` and `template` into their own crates.
- A second frontend (for example a CLI runner) needs the core and sys crates.

## Exit path

The crates are separated at module boundaries already. Merging `foreman-sys` back into the app, or splitting the core further, is mechanical; no Tauri types cross the core or sys APIs.
