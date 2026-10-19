# ADR-004: Typed IPC layer: ts-rs types, a hand-written client contract, a mock, and contract tests

## Status
Proposed

## Date
2026-10-19

## Context

Prompt 4.2 requires that the frontend talks to Rust **only** through a typed IPC layer, with a mock implementation for browser preview and component tests. Rust is the source of truth (MASTER-SPEC §10 State row). Every command must also be declared through `AppManifest::commands` (SR-23) and validate its input (SR-24). We need the TypeScript types to stay in sync with Rust without manual drift, and without tying Tauri upgrades to an unstable code generator.

Registry and source checks, 2026-10-19:

| Crate | Max stable | Max | Last release | Notes |
|---|---|---|---|---|
| `tauri-specta` | 1.0.2 | **2.0.0-rc.25** | 2026-05-08 | rc.25 depends on `tauri ^2` and `specta =2.0.0-rc.25` (exact); its README points Tauri v2 users to "Tauri Specta v2", which is RC only; it generates commands, events and `Channel` wrappers (`src/lang/js_ts.rs`) |
| `specta` | 1.0.5 | 2.0.0-rc.25 | 2026-05-07 | |
| `ts-rs` | **12.0.1** | 12.0.1 | 2026-01-31 | MIT, MSRV 1.78, 19 M downloads; `serde-compat` enabled by default; exports on `cargo test` or programmatically with `TS::export_all`; `TS_RS_EXPORT_DIR` sets the output |

## Decision

1. **Types:** every IPC DTO, event payload and error in `foreman-app::ipc` (and the core types they embed) derives `ts_rs::TS` with `#[ts(export)]`. `ts-rs` =12.0.1 with default `serde-compat` sits behind a `ts` cargo feature, so release builds don't compile it. Bindings go to `src/lib/ipc/bindings/` and are **checked in**. CI runs `cargo test -p foreman-app --features ts export_bindings` and then `git diff --exit-code src/lib/ipc/bindings`, so any drift fails the build.
2. **Commands:** one Rust constant `ipc::COMMANDS: &[&str]` lists every command. `build.rs` passes it to `tauri_build::AppManifest::commands`, which generates `allow-<cmd>`/`deny-<cmd>` (verified in tauri-build 2.7.3 `src/acl.rs`), and `generate_handler!` registers the same functions. On the TS side, `src/lib/ipc/contract.ts` declares `interface IpcClient` with one method per command, typed with the generated types.
3. **Contract tests:**
   - Rust: `capabilities::every_command_declared_and_granted` checks that `COMMANDS` equals the registered handler set and the capability grant list;
   - Rust exports `COMMANDS` to `bindings/commands.json`;
   - a Vitest test checks that `Object.keys` of the real client's command map equals `commands.json`, and that `MockIpcClient` implements `IpcClient` (enforced by `tsc`);
   - Rust: an `ipc::rejects_unknown_fields` test and a proptest fuzz per command (SR-24).
4. **Implementations:**
   - `TauriIpcClient` is the only module importing `@tauri-apps/api` (ESLint `no-restricted-imports`). It uses `invoke`, `Channel<ArrayBuffer>` for PTY output (Rust sends `tauri::ipc::Response::new(bytes)`, delivered as an `ArrayBuffer` per the `@tauri-apps/api` 2.12.4 docs), and `listen` for invalidation events.
   - `MockIpcClient` is an in-memory implementation over fixture projects (including the TradeLog fixture) with a scripted PTY. It's used by `pnpm dev:mock` (browser preview), Vitest component tests and Playwright flows.
   - `#lib/ipc/index.ts` picks one at startup (`import.meta.env.VITE_IPC === 'mock'`), never per call.
5. **Errors** are a tagged union `IpcError { code, message, detail? }` with a closed `code` set. The UI never parses messages.
6. **State:** Svelte 5 runes in `.svelte.ts` stores hold the last command result plus a revision. Rust events carry only `{id, rev}`, and a newer `rev` triggers a refetch, so Rust stays the source of truth.

## Alternatives considered

- **tauri-specta 2.0.0-rc.25.** It generates the whole client (commands, events, Channels) from Rust, so there's no hand-written contract. Lost because it's a release candidate after 25 RCs (latest May 2026), it pins `specta =2.0.0-rc.25` exactly, and it replaces `generate_handler!` with its own macros. Every Tauri or serde upgrade would wait on that RC chain, and a 20-year design shouldn't depend on an unreleased API. (`tauri` 2.12.4 even has a `specta` feature, so adopting it later is cheap; see Exit path.)
- **Hand-written TS types plus JSON contract tests only.** No generator dependency. Lost because every DTO change needs two edits, and drift is caught only by whatever fixtures happen to cover the field. `ts-rs` removes that class for one small, stable, MIT dependency that isn't in the release build.
- **JSON Schema (`schemars`) to TS through a Node generator.** Two toolchains and a schema layer we don't otherwise need; `ts-rs` is direct.

## Consequences

- Command names and argument shapes are written once in Rust and once in `contract.ts`. The contract tests make a mismatch a CI failure, not a runtime bug.
- The mock is a real product artifact: it must be updated with every command. `tsc` enforces that it implements the interface.
- Generated bindings are reviewed in diffs, which makes IPC changes visible in code review.

## Verification

- CI: `bindings-drift`, `capabilities::every_command_declared_and_granted`, `ipc::rejects_unknown_fields`, `ipc::fuzz_*`.
- Vitest `ipc/contract.test.ts` (command sets equal).
- Playwright flows run entirely on `MockIpcClient`.
- M2 walking skeleton: one command of each shape (query, mutation, Channel, event) through the real client in the WDIO smoke test.

## Revisit when

- specta/tauri-specta 2.0 ships a stable release, and its Tauri compatibility tracks Tauri minors within a week.
- The command count passes about 120 and the hand-written contract becomes a burden. (Re-baselined in the Stage 4.7 fix pass: the contract already lists 91 commands in `docs/contracts/ipc.md`, so the original 80 trigger had fired before code existed.)

## Exit path

- **To tauri-specta:** replace `ts-rs` derives with `specta::Type` (or keep both during migration), switch `generate_handler!` to `collect_commands!`, and generate `contract.ts`. The mock still implements the generated interface.
- **To hand-written types:** delete the derives and keep the checked-in bindings as source.
