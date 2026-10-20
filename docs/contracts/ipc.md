# Foreman · IPC contract

**Status:** DRAFT for the Stage 4.7 review · 2026-10-19 · Blueprint stage 4.4 · amended 2026-10-20 (M2-01: §2.4 settles the Stage 4 gaps, IPC-R12, §7.1 drift checks)
**Inputs:** `docs/design-doc.md` §3.5 (the design doc points here for the command list), ADR-004 (typed IPC), ADR-006 (PTY), ADR-008 (chains), ADR-014 (capabilities), ADR-021 (launch), ADR-022 (decisions), `docs/requirements.md`, `docs/scope.md`, `spec/council/r6-feature-specs.md`.

This file is the single list of everything that crosses the webview ↔ Rust boundary: **91 commands**, **3 Channel types** and **9 events**. A command, field, Channel or event that isn't in this file doesn't exist. Adding one is a change to this file, to `ipc::COMMANDS`, to `src/lib/ipc/contract.ts` and to `MockIpcClient`, in one commit (§7).

Library facts this contract relies on were checked on 2026-10-19 in the published crate sources:

- **ts-rs 12.0.1** (`src/lib.rs`, `ts-rs-macros 12.0.1`): `#[ts(export)]` generates a test named `export_bindings_<lowercased type name>`; `TS_RS_EXPORT_DIR` sets the output folder; `serde-compat` (default feature) honours `rename_all`, `tag`, `rename`; `Option<T>` becomes `T | null`; **`i64`/`u64`/`i128`/`u128` become `bigint` by default** (`TS_RS_LARGE_INT`), while `serde_json` sends them as JSON numbers; an empty named struct becomes `Record<symbol, never>`.
- **tauri 2.12.4** (`src/ipc/channel.rs`, `src/ipc/mod.rs`, `src/event/event_name.rs`): `Channel<TSend>` is a `CommandArg` (a separate command argument, not a field of a request struct); `impl<T: Serialize> IpcResponse for T` and `impl IpcResponse for Response`, so `Channel<LaunchProgress>` sends JSON and `Channel<tauri::ipc::Response>` sends raw bytes (an `ArrayBuffer` in JS); event names may contain only alphanumerics, `-`, `/`, `:` and `_`.

---

## 1. Rules for every command

| ID | Rule | Test |
|---|---|---|
| IPC-R1 | A command has exactly one request argument named `req`, a struct with `#[serde(deny_unknown_fields, rename_all = "camelCase")]`, no `#[serde(flatten)]`, no `#[serde(untagged)]`. Commands that stream also take **one** Channel argument (`output` or `progress`); Tauri injects `State`/`AppHandle`/`WebviewWindow`, which the webview never supplies. | `ipc::rejects_unknown_fields` (one case per command), `ipc::no_flatten_or_untagged` (source scan) |
| IPC-R2 | Rust validates every field before use: string lengths (the max is in the type table, §2), enum membership (serde), ID existence (`NotFound`), repo-relative paths through `pathpolicy::check_read`/`check_write`, URLs through `app::os` parsing. The webview never supplies a program, argv, env value or absolute path (SR-24). Paths Billy picks come from Rust-side dialogs (`blueprint_pick_repo`, `setup_pick_folder`, `settings_pick_tool`, save dialogs). | `ipc::fuzz_<command>` (one proptest per command: arbitrary JSON → typed error, never a panic), `ipc::no_absolute_paths_accepted` |
| IPC-R3 | Every command returns `Result<T, IpcError>` (§3). A handler panic is caught (`catch_unwind`) and becomes `Internal`, logged with the location only (SR-57). | `ipc::panic_returns_typed_error` |
| IPC-R4 | **No 64-bit integers in DTOs.** Counts, revisions and durations are `u32` (`number`); times are RFC 3339 strings (`jiff::Timestamp` serialized as a string, `#[ts(type = "string")]`). The default `TS_RS_LARGE_INT = bigint` is kept on purpose, so a stray `u64` shows up as `bigint` in the bindings and fails the test. | `ipc::no_bigint_in_bindings` (greps `src/lib/ipc/bindings/` for `bigint`) |
| IPC-R5 | Response `Option` fields serialize as `null` (no `skip_serializing_if`), so the generated `T \| null` is exact. Maps are `BTreeMap<String, _>` (sorted, string keys). | `ipc::response_snapshot_<command>` (insta JSON snapshot per command) |
| IPC-R6 | Commands that mutate state are user invokes only. No event, watcher hint, sensor event or terminal output calls a command handler or its internals (SR-18). | `rearm::only_from_user_action`, `watcher::events_never_act`, `sensor::events_only_update_state` (module-dependency tests) |
| IPC-R7 | Two-phase actions (`*_preview` → `*_apply`) store the preview under a one-time `PreviewId` (UUID v4) bound to the project revision and the `ReadStamp`s it read, valid 10 minutes. `launch_prepare` stores a one-time `PlanId` valid **60 s** and bound to the project revision and the config fingerprint (ADR-021). A consumed or expired ID returns `Expired`; a changed revision, stamp or fingerprint returns `Stale` with the fresh preview in `detail`. | `ipc::preview_single_use`, `ipc::plan_expires_60s`, `ipc::stale_plan_rejected` |
| IPC-R8 | Every command is listed in `ipc::COMMANDS`, passed to `AppManifest::commands` in `build.rs`, registered in `generate_handler!`, and granted to the `main-window` capability only (SR-23). | `capabilities::every_command_declared_and_granted` |
| IPC-R9 | Mutating commands on a project opened read-only (lock mismatch, invalid or newer-schema decision file, missing blueprint) return `ReadOnly` with the reason, except `pending_change_*`, `blueprint_repin`, `project_remove`, `progress_rebuild_*` and `file_open`/`file_reveal`. | `ipc::read_only_refuses_mutation` |
| IPC-R10 | A command that writes a file goes through `store::Root::write` (ADR-010). A compare-and-swap failure returns `Conflict` naming the file and, for multi-file applies, the list of files already written (§3.5.3 of the design doc). | `pasteback::conflict_reports_written_targets` |
| IPC-R11 | **Approvals are native, never IPC fields** (SR-24a). No request struct has a field that means approved, confirmed, consented, acknowledged or "idle confirmed". A command that needs Billy's confirmation raises a native macOS dialog from Rust (`tauri-plugin-dialog`, called only in `app::os`) and acts on its answer; declining returns `Cancelled`. The webview may show context (diffs, reasons, the preview) but its own buttons only *request* the action. Native dialogs: config acknowledgement (SR-06), launch and re-arm confirmations (SR-19, SR-13 idle assertion), Terminate, accepting an outside change (SR-40), writing root `CLAUDE.md` (SR-30), confirming a signature written outside Foreman (SR-47), signing (SR-42), terminal links (SR-29), security-relevant settings (pass-through list, tool paths, allowlisted key hashes, project `env` approval, login PATH), item-kind re-confirmation, project removal, Clear run history and the quit guard. | `ipc::security_confirmations_are_native` (a static check over `src/lib/ipc/bindings/*Req.ts`: no field named `confirm*`, `ack*`, `consent*`, `approved*`, `*Confirmed`; plus one integration test per dialog that a scripted "Cancel" leaves state unchanged), `settings::security_fields_require_native_confirm` |
| IPC-R12 | **Commands not built yet** (Stage 5 build-out). Every command in §4 is registered, granted and validated from M2-01 on. A command whose backend ticket hasn't landed returns `Unsupported { feature: "<command>", why: "notYetAvailable:<ticket>" }` after its request passes validation, and nothing changes. The ticket is named in `docs/progress.md`. A command reaches the UI's "not built yet" state only this way, never as `Internal` or an untyped error. | `ipc::rejects_unknown_fields` (a valid fixture never answers `Invalid` or `Internal`) |

Mutation column legend used below: **no** = reads only; **mem** = in-memory state only (plan or preview IDs); **app** = app-data files; **repo** = project repo files; **clip** = writes the clipboard; **spawn** = starts a process; **git** = makes a git commit.

## 2. Shared types

Every type below derives `Serialize`/`Deserialize` as needed, `ts_rs::TS` with `#[ts(export)]` (behind the `ts` feature, ADR-004), and is exported to `src/lib/ipc/bindings/`.

### 2.1 Scalars and identifiers

| Rust | Wire / TS | Validation (Rust) |
|---|---|---|
| `ProjectId(Uuid)`, `RunId(Uuid)`, `ChainId(Uuid)`, `PlanId(Uuid)`, `PreviewId(Uuid)`, `ChangeId(Uuid)`, `PickId(Uuid)` | `string` (`#[ts(type = "string")]`) | Lowercase hyphenated UUID; `NotFound` if unknown |
| `HitId` (§2.4) | `string` | 64 lowercase hex: the SHA-256 hit ID `fingerprint::auth_key_hits` gives an auth-key hit (stable across scans, never the value); `NotFound` if unknown |
| `StepKey(String)` | `string` | `^(?:[0-9]{1,2}\.[0-9]{1,2}[a-z]?\|council-setup\|[0-9]{1,2}:(?:gate\.[ABC]\|F\.2)\|x-[0-9]{1,2}-[0-9]{1,3})$` (domain-rules DR-005) |
| `ItemId(String)` | `string` | `^(?:[a-z][a-z0-9-]{0,47}\|xi-[0-9]{1,2}-[0-9]{1,3})$` |
| `Stage(u8)` | `number` | `0..=18` |
| `Rev(u32)` | `number` | Monotonic per project; wraps never in practice |
| `RepoPath(String)` | `string` | ≤ 1,024 bytes; `pathpolicy::check_read` (reads) or `check_write` (writes) |
| `Sha256(String)` | `string` | `^[0-9a-f]{64}$` |
| `Timestamp` | `string` | RFC 3339 UTC |
| `IsoDate(String)` | `string` | `^\d{4}-\d{2}-\d{2}$`, a real date |
| `ShortText(String)` | `string` | ≤ 200 chars, no C0/C1/DEL/bidi controls |
| `Note(String)` | `string` | ≤ 4,000 chars, no C0 (except LF) /C1/DEL |
| `InputValue(String)` | `string` | ≤ 1 MB; > 64 KB accepted with a warning (AC-F006-14) |
| `PtyData(String)` | `string` | ≤ 64 KB UTF-8 (client splits larger writes, §6.2) |
| `ClaudeUrl(String)` | `string` | Parsed; `https`, host `claude.ai`, no userinfo, no port (SR-25) |
| `BundleId(String)` | `string` | `^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+){1,10}$` |
| `EnvName(String)` | `string` | `^[A-Za-z_][A-Za-z0-9_]{0,127}$` |

### 2.2 Enums (serde `rename_all = "camelCase"` unless the wire form is shown)

```rust
enum TriState { Yes, No, Undecided }                       // "yes" | "no" | "undecided"
enum ConditionId { IdeaKnown, Commercial, Payments, MultiUser, Public, HasUi, OthersData, Server,
                   HasDatabase, HasUploads, HasEmail, HasIntegrations }   // wire: snake_case ("idea_known", …)
enum Surface { Chat, Cli, Desktop }
enum CwdKind { Project, Spec }                             // "blueprint"/"worktree" are template errors in v1
enum Model { Fable, Opus, Sonnet, Haiku, Opusplan }
enum Effort { Low, Medium, High, Xhigh, Max, Ultracode, Adaptive }   // "adaptive" only for chat
enum Mode { Plan, Default, AcceptEdits, Auto, DontAsk }    // "manual" in the template maps to Default
enum EntryPoint { DoNext, Palette, StageView }             // "doNext" | "palette" | "stageView"
enum StepState { NotStarted, Copied, Running, Done, Skipped, NotApplicable, WaitingUntil, Undecided, NeedsAnnotation,
                 NeedsNewerForeman }                       // DR-035: blueprint grammar newer than this Foreman
enum RunStatus { Starting, Running, Idle, Blocked, Exited, Errored, Interrupted }   // Idle/Blocked: P2 sensor
enum ReadOnlyReason { LockMismatch, BlueprintMissing, DecisionFileInvalid, NewerSchema, GitMissing, FolderMissing }
enum DoctorLevel { Ok, Warning, Failing, Unknown }
enum ToolKind { Claude, Git }
enum PasteKind { Auto, Packet, Output, AdrSet, Lessons }   // AdrSet, Lessons: P2
enum CopyTarget { Prompt, Command, SessionId, FileContents, FixRequest }
```

```ts
type TriState = "yes" | "no" | "undecided";
type ConditionId = "idea_known" | "commercial" | "payments" | "multi_user" | "public" | "has_ui"
  | "others_data" | "server" | "has_database" | "has_uploads" | "has_email" | "has_integrations";
type Surface = "chat" | "cli" | "desktop";
type CwdKind = "project" | "spec";
type Model = "fable" | "opus" | "sonnet" | "haiku" | "opusplan";
type Effort = "low" | "medium" | "high" | "xhigh" | "max" | "ultracode" | "adaptive";
type Mode = "plan" | "default" | "acceptEdits" | "auto" | "dontAsk";
type EntryPoint = "doNext" | "palette" | "stageView";
type StepState = "notStarted" | "copied" | "running" | "done" | "skipped" | "notApplicable"
  | "waitingUntil" | "undecided" | "needsAnnotation" | "needsNewerForeman";
type RunStatus = "starting" | "running" | "idle" | "blocked" | "exited" | "errored" | "interrupted";
```

### 2.3 Structured types (Rust shown; TS is the mechanical camelCase translation)

```rust
struct Start { model: Model, effort: Effort, mode: Option<Mode> }          // mode null for chat
struct Explained<T> { value: T, origin: ValueOrigin, from: Option<T> }      // F-005 explain on hover
enum ValueOrigin { Template, Alternative { rule: String }, Substitution { rule: String }, Override }
struct RunOverride { model: Option<Model>, effort: Option<Effort>, mode: Option<Mode> }

// Text crosses IPC only as typed nodes (SR-26). Visible escapes are pre-computed in Rust.
enum TextSpan { Plain { text: String }, Escape { codepoint: u32, label: String },   // bidi, zero-width, C0/C1
                Code { text: String }, Strong { children: Vec<TextSpan> }, Emph { children: Vec<TextSpan> },
                Link { text: String, url: String } }                              // url shown as text + Open action
enum DocNode { Heading { level: u8, spans: Vec<TextSpan> }, Paragraph { spans: Vec<TextSpan> },
               List { ordered: bool, items: Vec<Vec<DocNode>> }, Quote { children: Vec<DocNode> },
               CodeBlock { info: String, text: String }, Rule }

// Resolved prompt, one segment per resolver decision (domain-rules DR-080 ff.)
enum PromptSegment { Text { spans: Vec<TextSpan> },
                     Resolved { token: String, class: TokenClass, text: String, note: Option<String> },
                     Unresolved { token: String, class: TokenClass, input: Option<String> },   // attention pill
                     OptionalPart { part_id: String, included: bool, text: String },
                     RemovedPart { expr: String, reason: String } }                          // fm:if false
enum TokenClass { ProjectVariable, PacketMarker, PathAlias, RunInput, OptionalPart, Literal }

struct Diff { path: RepoPath, exists: bool, hunks: Vec<DiffHunk> }
struct DiffHunk { old_start: u32, new_start: u32, lines: Vec<DiffLine> }
struct DiffLine { op: DiffOp /* "ctx" | "add" | "del" */, spans: Vec<TextSpan> }
struct RedactedDiff { source: String, entries: Vec<RedactedEntry> }      // SR-55
struct RedactedEntry { key_path: String, change: ChangeKind /* "added"|"removed"|"valueChanged" */, before: Option<String>, after: Option<String> }

struct ReadOnly { reason: ReadOnlyReason, detail: String }
struct Problem { rule: String, line: Option<u32>, message: String, fix: Option<Vec<FixAction>> }   // every fix offered (§2.4)
enum FixAction { Repair, OpenInEditor, CopyFixRequest }     // Repair: P2
```

`Blocker` (returned inside `PrepareOutcome`, never thrown) and `ConfirmReason`:

```rust
enum Blocker {
  NotRunnable { reason: NotRunnableReason, text: String },      // §2.4: notApplicable, undecided, needsAnnotation, done, needsNewerForeman, waiting, running, notCli, chainNotStarted
  ReadOnly(ReadOnly),
  ClaudeMissing { path: Option<String>, why: ToolPathProblem }, // relative, missing, not executable, in project, symlink
  VersionBelowFloor { found: String, required: String, requirements: Vec<Requirement> },
  ContractFailed { feature: String, version: String },          // P2
  ConfigChanged { cwd: CwdKind, fingerprint: Sha256, diffs: Vec<RedactedDiff> },  // needs config_ack
  AuthKeyBlocked { file: String, key: String, hit: HitId, allowlistable: bool },
  ConfigTooLarge { file: String, bytes: u32 },
  ProjectEnvUnapproved { file: String, sha256: Sha256 },        // only on the S-7′ fail branch
  AuthFailed { why: AuthProblem },                              // wrongMethod{method} | unknownMethod | unparsable | exit{code} | timeout
  TranscriptMissing { chain: ChainId },                         // offers a fresh chain with context
  SignedPacketMissing { stage: Stage },
  PacketUnsigned { stage: Stage },                              // "Sign Stage N first"
  RunInputMissing { name: String },
  UnresolvedToken { token: String },
  PromptOptionLike { first: char },                             // SR-04
  PromptControlChar { offset: u32, codepoint: u32 },            // SR-28
  RunSettingsPathUnsafe { path: String },                       // glob metacharacters that can't be escaped
  CwdNotDirectory { cwd: CwdKind },
}
enum ConfirmReason { OverlayStep, ModeOrModelChanged { from: Start, to: Start }, FirstLaunchInProject,
                     AutoOrDontAsk { mode: Mode }, ClaudeVersionChanged { from: String, to: String } }
```

`Blocker` and `ConfirmReason` use `#[serde(tag = "kind", rename_all = "camelCase")]`, so TS gets discriminated unions (`{ kind: "versionBelowFloor"; found: string; … }`).

### 2.4 Types settled in Stage 5 (the UI's 67 `GAP:` notes)

Stage 4 left types that §2–§4 name but never define, and fields the screens need that no response had. They are settled here, for ticket M2-01, consistent with the domain rules. The Rust definitions are in `src-tauri/src/ipc/` (`types.rs`, `dto/*.rs`); the TypeScript below is what ts-rs generates from them into `src/lib/ipc/bindings/` (ADR-004), copied here for review. `src/lib/ipc/types.ts` only re-exports the bindings, so the webview can't use a shape Rust doesn't produce.

**Rules for these fields.**
- Every text field below is Foreman-written (SR-56) or repo/terminal text already passed through visible escapes (SR-26): plain strings get `‹U+XXXX›` for bidi, zero-width, tag and control characters; `TextSpan[]` gets `escape` spans.
- `StepSummary.stateText`, `whyNotRunnable` and `conditionNote` come from the derived state (DR-050, DR-065): `Not applicable` with the reason, `Undecided` with `<condition> = undecided`, `Waiting until <date>`, `Skipped: <reason>`.
- `StepSummary.next` is true when another step of the same chain is running (Run re-arms it, DR-068).
- `StageView.status` (the sheet strip, `docs/design/components.md`): `signed` (Foreman's signing or a confirmed outside signature, DR-135); `notApplicable`, `needsAnnotation` and `undecided` from the applicable path (DR-054; `undecided` when no step of the stage is on the path and one waits on a condition); `locked` "until Stage N is signed" while the previous applicable stage isn't signed; `waiting` when a wait step is waiting; `inspecting` when every non-gate step on the path is done; `inProgress` when any is copied, running or done; else `ready`.
- `ProjectSummary.currentStage` is the lowest applicable stage that isn't signed. Before a project is opened in this run, `app_bootstrap` (which reads only `settings.json`, `projects.json` and each `progress.json`, NFR-01) estimates it from `progress.json` as the stage after the highest signed one.
- `SpecLine`: `model`/`effort`/`mode` are display names with `origin` and `from` set only when the effective value differs from the template's (DR-104); `note` is the `SUBSTITUTIONS:` line; `where` is `project root`, `spec/` or `claude.ai chat, in Project`.
- `PendingChange.file` uses the `progress.json` wire form (`project`, `overlay`, `blueprintLock`; files.md §4.3).
- `Problem.fix` lists every fix offered, in order (a packet problem offers `openInEditor` and `copyFixRequest`).
- `Blocker::NotRunnable` carries `text`, Foreman's sentence for the row. `NotRunnableReason` gains `waiting`, `running`, `notCli` (a `launch_prepare` on a step that isn't CLI) and `chainNotStarted` (a continuing step whose chain hasn't started).
- `SettingsView.screenReaderMode` is the Settings toggle `settings_update` patches. `settings.json` v1 has no field for it, so it is kept in memory until `settings.json` v2 adds `screenReaderMode` (files.md §4.1, proposal); `runHistory` counts `runs/` once run records exist (M2-10).
- `HitId` is the hit's SHA-256 ID from `fingerprint::AuthKeyHit` (not a UUID), so the same hit keeps its ID across scans; Rust maps it back to file, key and value hash.
- `Requirement { feature, min, source }`: the feature that needs the newer Claude Code, its minimum version and where the requirement comes from (`step 2.3`, `Foreman`), as `foreman-core::launch` reports it.
- `ToolPathProblem` gains `worldWritable` (the configured `claude` or a directory above it is writable by others, SR-03); `AuthProblem.exit.code` is `number | null` (killed by a signal).
- `LaunchProgress`: `Version.found` and `Exited.code` are `string | null` and `number | null`; `Contract.outcome` is `ContractOutcome` (`pass`, `flagFailure`, `inconclusive`, `skipped`), the probe result without its output.
- (CR-003) A pin refused for annotation errors is `code: notAllowed` with `detail: { kind: "annotationErrors", errors }` instead of the `notAllowed` detail, so the rule name `p1-annotation-errors` isn't on the wire; the UI branches on `detail.kind` and lists each error (stage, step ID, line, rule, message). `setup_preview` runs the same check before anything is written (`foreman_core::lock::p1_annotation_errors`, the check `lock::pin` runs), and `setup_apply` pins the same blob (its oid is compared).
- A decision-file write that meets an outside change still pending review returns `Conflict` with the file's path (the pending change is reviewed first; DR-148).
- (M3-25) `StageView.closeoutChecklist` is the P1 close-out checklist step (AC-F015-01): the stage type's close-out sequence (MASTER-SPEC §8, `foreman_core::steps::closeout`) while the stage's packet exists with no problems and the stage isn't signed; empty otherwise. P2 replaces it with `GateView.closeout`.
- `AppBootstrap.interrupted` lists runs whose `run.json` says `running` at startup (SR-57: listed, never relaunched); `capabilities` gates Desktop (`--desktop` ≥ 2.1.285), the P2 sensor and the verified probes.

**Nested request objects.** A tagged object inside a request (`EditorSetting`, `LinkSource`, `AttestOutcome`, `SignBranch`) is an enum whose variants hold a struct with `deny_unknown_fields`, so an extra field inside it is `Invalid` too (IPC-R1). The wire form is unchanged (`{ kind: "set", bundleId }`); ts-rs writes it as `{ "kind": "set" } & EditorSet`.

New types:

```ts
export type HitId = string;
export type ContractOutcome = "pass" | "flagFailure" | "inconclusive" | "skipped";
export type RunSummary = { runId: RunId, chain: ChainId, project: ProjectId, projectName: string, step: StepKey | null, stepTitle: string, tabName: string, ranWith: string, substituted: string | null, status: RunStatus, exitCode: number | null, error: string | null, startedAt: Timestamp, timeToStartMs: number | null, argv: Array<string>, resumable: boolean, whyNotResumable: string | null, };
export type Capabilities = { claudeVersion: string | null, desktop: boolean, sensor: boolean, probesVerified: boolean, };
export type ToolPathView = { path: string | null, version: string | null, autoDetected: boolean, problem: string | null, };
export type ToolPaths = { claude: ToolPathView, git: ToolPathView, };
export type TemplateHealthSummary = { runnable: string, needsAnnotation: string, errors: number, warnings: number, };
export type VariableSpec = { name: string, label: string, placeholder: string, multiline: boolean, fromSettings: boolean, };
export type ConditionSpec = { id: ConditionId, label: string, effect: string, late: boolean, };
export type LoginPathView = { capturedAt: Timestamp, entries: Array<string>, };
export type AllowlistEntry = { sha256: Sha256, file: string, key: string, addedAt: Timestamp, };
export type RunHistorySize = { runs: number, megabytes: number, };
export type InboxKind = "sessionBlocked" | "sessionYourTurn" | "packetLanded" | "packetInvalid" | "gateFailed" | "gateSignable" | "outsideChange" | "tamper" | "chatAwaitingPaste" | "waitElapsed" | "interruptedSession" | "stepReady";
export type SpecSurface = "chat" | "cli" | "desktop" | "action";
export type SpecLine = { surface: SpecSurface, where: string, model: Explained<string> | null, effort: Explained<string> | null, mode: Explained<string> | null, agents: Array<string>, droppedAgents: Array<string>, note: string | null, };
export type SheetStatus = "locked" | "ready" | "inProgress" | "inspecting" | "waiting" | "signed" | "notApplicable" | "needsAnnotation" | "undecided";
export type SignedInfo = { date: string, commit: string, branch: string, merged: boolean, };
export type PacketSummary = { path: RepoPath, exists: boolean, problems: number, signed: boolean, };
export type Applicability = { "kind": "applies" } | { "kind": "notApplicable", reason: string, } | { "kind": "undecided", condition: ConditionId, } | { "kind": "needsAnnotation" };
export type ConditionView = { value: TriState, locked: boolean, lockRule: string | null, label: string, effect: string, late: boolean, };
export type BlueprintPin = { version: string, commit: string, };
export type ConfigChangedNotice = { cwd: CwdKind, fingerprint: Sha256, blocking: boolean, title: string, detail: string, diffs: Array<RedactedDiff>, };
export type SchemaState = "current" | "older" | "newer";
export type PlannedAction = "create" | "leaveAlone" | "refuse";
export type PinPreview = { version: string, commit: string, clean: boolean, };
export type PathRowStatus = "applies" | "notApplicable" | "undecided" | "needsAnnotation";
export type ApplicablePathRow = { stages: string, title: string, status: PathRowStatus, why: string, };
export type StripCell = { stage: Stage, status: SheetStatus, why: string | null, };
export type ApplicablePathSummary = { applies: number, total: number, summary: string, rows: Array<ApplicablePathRow>, strip: Array<StripCell>, };
export type CheckGroup = "claude" | "toolchain";
export type CheckAction = "captureLoginPath" | "openProbeFolder";
export type DoctorCheck = { id: string, group: CheckGroup, name: string, level: DoctorLevel, summary: string | null, detail: Array<TextSpan>, found: string | null, foundMono: boolean, action: CheckAction | null, };
export type CapabilityRow = { feature: string, code: boolean, minimum: string, found: boolean, blocks: string | null, };
export type ProbeResult = "pass" | "fail" | "inconclusive";
export type ProbeOutcome = { probe: Array<TextSpan>, result: ProbeResult, text: string, detail: string | null, };
export type PathEntryCheck = { entry: string, ok: boolean, why: string | null, };
export type Counts = { stages: number, runnableSteps: number, items: number, errors: number, warnings: number, };
export type StageHealthStatus = "runnable" | "warning" | "partial" | "needsAnnotation";
export type StageHealth = { stages: string, title: string, status: StageHealthStatus, text: string, steps: string, };
export type Severity = "error" | "warning";
export type RunInputView = { name: string, label: string, token: string, multiline: boolean, remember: boolean, required: boolean, value: string, warning: string | null, };
export type SwitchCue = { text: string, };
export type ChatChip = { id: string, label: string, strong: boolean, };
export type ChatChips = { chips: Array<ChatChip>, link: string | null, linkLabel: string | null, };
export type OutView = { path: RepoPath, exists: boolean, };
export type StepPart = { partId: string, label: string, included: boolean, locked: boolean, };
export type MissingLink = "project" | "conversation";
export type PacketFields = { fromStage: Stage, toStage: Stage | null, };
export type ChangeEffect = "changesPath" | "noEffect" | "reloaded";
export type ConsequenceGlyph = "lock" | "info" | "check";
export type Consequence = { glyph: ConsequenceGlyph, text: string, };
export type ChangeContext = { changed: string, git: string, approved: string, };
export type PacketHeader = { line: string, fromStage: Stage, toStage: Stage | null, };
export type PacketFieldView = { key: string, label: string, value: Array<DocNode>, missing: boolean, problems: Array<Problem>, note: Array<TextSpan> | null, };
export type NormalizationNote = { field: string, message: string, };
export type Signature = { name: string, date: IsoDate, byForeman: boolean, confirmed: boolean, };
export type RawLine = { line: number, spans: Array<TextSpan>, bad: boolean, };
export type BaselineComparison = { baselineMedianMs: number, foremanMedianMs: number, };
export type GateState = "notStarted" | "inProgress" | "failed" | "passed" | "signed";
export type ArtifactKind = "path" | "command";
export type GateArtifact = { kind: ArtifactKind, text: string, };
export type ItemKind = "attest" | "time";
export type ItemStatus = "pass" | "fail" | "waiting" | "notApplicable";
export type Proposed = "pass" | "fail";
export type GateItemView = { id: ItemId, text: string, kind: ItemKind, how: string | null, status: ItemStatus, evidence: Array<TextSpan> | null, proposed: Proposed | null, note: string | null, waitsFor: string | null, artifacts: Array<GateArtifact>, failures: number, needsReconfirm: boolean, session: string | null, textSha256: Sha256, previousText: string | null, };
export type CloseoutKey = "gateA" | "results" | "gateB" | "gateC" | "sign";
export type CloseoutState = "done" | "now" | "failed" | "pending" | "locked";
export type CloseoutStep = { key: CloseoutKey, label: string, state: CloseoutState, note: string | null, };
export type LessonRef = { id: string, title: string, };
export type ManifestRow = { path: RepoPath, exists: boolean, inProject: boolean, producer: StepKey | null, };
export type AssemblerState = "exists" | "missing" | "empty" | "refused";
export type LateConditionView = { condition: ConditionId, value: TriState, proposed: TriState | null, evidence: string | null, decided: string | null, };
export type ImportResult = "pass" | "fail" | "unmatched";
export type Tag = "core" | "conditional" | "optional" | "added";
export type StepKind = "prompt" | "followup" | "action" | "manual" | "wait" | "repeatable" | "gate" | "lessons";
export type NotRunnableReason = "notApplicable" | "undecided" | "needsAnnotation" | "done" | "needsNewerForeman" | "waiting" | "running" | "notCli" | "chainNotStarted";
export type ToolPathProblem = "relative" | "missing" | "notExecutable" | "inProject" | "symlinkIntoProject" | "worldWritable";
export type AuthProblem = { "kind": "wrongMethod", method: string, } | { "kind": "unknownMethod" } | { "kind": "unparsable" } | { "kind": "exit", code: number | null, } | { "kind": "timeout" };
export type Requirement = { feature: string, min: string, source: string, };
export type DecisionFileKind = "project" | "overlay" | "blueprintLock";
export type FixAction = "repair" | "openInEditor" | "copyFixRequest";
export type Problem = { rule: string, line: number | null, message: string, fix: Array<FixAction> | null, };
export type Blocker = { "kind": "notRunnable", reason: NotRunnableReason, text: string, } | { "kind": "readOnly" } & ReadOnly | { "kind": "claudeMissing", path: string | null, why: ToolPathProblem, } | { "kind": "versionBelowFloor", found: string, required: string, requirements: Array<Requirement>, } | { "kind": "contractFailed", feature: string, version: string, } | { "kind": "configChanged", cwd: CwdKind, fingerprint: Sha256, diffs: Array<RedactedDiff>, } | { "kind": "authKeyBlocked", file: string, key: string, hit: HitId, allowlistable: boolean, } | { "kind": "configTooLarge", file: string, bytes: number, } | { "kind": "projectEnvUnapproved", file: string, sha256: Sha256, } | { "kind": "authFailed", why: AuthProblem, } | { "kind": "transcriptMissing", chain: ChainId, } | { "kind": "signedPacketMissing", stage: Stage, } | { "kind": "packetUnsigned", stage: Stage, } | { "kind": "runInputMissing", name: string, } | { "kind": "unresolvedToken", token: string, } | { "kind": "promptOptionLike", first: string, } | { "kind": "promptControlChar", offset: number, codepoint: number, } | { "kind": "runSettingsPathUnsafe", path: string, } | { "kind": "cwdNotDirectory", cwd: CwdKind, };
```

Types from §2–§4 with the fields this section adds:

```ts
export type AppBootstrap = { firstRun: boolean, settingsRev: Rev, projects: Array<ProjectSummary>, interrupted: Array<RunSummary>, doctor: DoctorLevel, capabilities: Capabilities, };
export type BlueprintRepoInfo = { displayPath: string, commit: string, version: string, grammarVersion: number, dirty: boolean, health: TemplateHealthSummary, variables: Array<VariableSpec>, conditions: Array<ConditionSpec>, };
export type SettingsView = { rev: Rev, driName: string | null, tools: ToolPaths, editorBundleId: BundleId | null, blueprintRepo: BlueprintRepoInfo | null, theme: Theme, terminal: TerminalPrefs, envPassthrough: Array<EnvName>, loginPath: LoginPathView | null, globalVariables: Record<string, string>, authKeyAllowlist: Array<AllowlistEntry>, screenReaderMode: boolean, runHistory: RunHistorySize, };
export type InboxItem = { kind: InboxKind, project: ProjectId, step: StepKey | null, stage: Stage | null, at: Timestamp, projectName: string, title: string, detail: string, chain: ChainId | null, run: RunId | null, };
export type StepSummary = { key: StepKey, number: string, title: string, state: StepState, tag: Tag, surface: Surface | null, specLine: SpecLine | null, whyNotRunnable: string | null, stateText: string, conditionNote: string | null, kind: StepKind, run: RunId | null, next: boolean, };
export type StageView = { stage: Stage, title: string, applicable: Applicability, signed: SignedInfo | null, steps: Array<StepSummary>, packet: PacketSummary | null, subtitle: string, status: SheetStatus, statusWhy: string | null, closeoutChecklist: Array<string>, };
export type ProjectView = { summary: ProjectSummary, conditions: { [key in ConditionId]?: ConditionView }, stages: Array<StageView>, pendingChanges: number, readOnly: ReadOnly | null, branch: string, blueprint: BlueprintPin, dri: string | null, chatProjectUrl: string | null, configChanged: ConfigChangedNotice | null, };
export type NextStepRow = { project: ProjectSummary, next: StepSummary | null, whyNone: string | null, strip: Array<SheetStatus>, stageTitle: string | null, };
export type DoctorReport = { level: DoctorLevel, checks: Array<DoctorCheck>, capabilities: Array<CapabilityRow>, probes: Array<ProbeOutcome>, probeNote: string | null, ranAt: Timestamp, };
export type Diagnostic = { severity: Severity, rule: string, file: string, line: number, column: number, message: Array<TextSpan>, stage: Stage, };
export type TemplateHealth = { version: string, grammarVersion: number, counts: Counts, stages: Array<StageHealth>, diagnostics: Array<Diagnostic>, commit: string, file: string, };
export type StepView = { summary: StepSummary, prompt: Array<PromptSegment>, inputs: Array<RunInputView>, start: Explained<Start> | null, switches: Array<SwitchCue>, chips: ChatChips | null, argvPreview: Array<string> | null, envNames: Array<EnvName>, blockers: Array<Blocker>, warnings: Array<string>, remaps: Array<string>, outs: Array<OutView>, override: RunOverride | null, parts: Array<StepPart>, copiedAt: Timestamp | null, };
export type PendingChange = { id: ChangeId, file: DecisionFileKind, detectedAt: Timestamp, seenSha256: Sha256, diff: RedactedDiff, effect: ChangeEffect, consequences: Array<Consequence>, context: ChangeContext, };
export type PacketView = { path: RepoPath, exists: boolean, sha256: Sha256 | null, header: PacketHeader | null, fields: Array<PacketFieldView>, problems: Array<Problem>, normalization: Array<NormalizationNote>, signed: Signature | null, tamper: Diff | null, raw: Array<RawLine>, writtenBy: string | null, summary: string, };
export type MetricsView = { perSurface: Array<SurfaceMetrics>, manualEditsByStage: Array<StageEdits>, stagesThroughForeman: number, baseline: BaselineComparison | null, runsCounted: number, p95AllMs: number, };
export type RunsPage = { runs: Array<RunSummary>, next: string | null, total: number, };
export type GateView = { stage: Stage, state: GateState, items: Array<GateItemView>, closeout: Array<CloseoutStep>, signable: boolean, whyNot: Array<string>, importedFrom: string | null, };
export type SignPreview = { previewId: PreviewId, packetSha256: Sha256, short: string, addedFields: Diff, paths: Array<RepoPath>, confirmConditions: Array<ConditionId> | null, chainBusy: boolean, blockers: Array<string>, branch: string, commitMessage: string, pathNotes: Record<string, string>, };
export type AssemblerRow = { path: string, state: AssemblerState, rule: string | null, note: string | null, };
```

Nested request payloads:

```ts
export type EditorSetting = { "kind": "set" } & EditorSet | { "kind": "clear" } & Empty;
export type EditorSet = { bundleId: BundleId, };
export type LinkSource = { "kind": "clipboard" } & Empty | { "kind": "text" } & LinkText;
export type LinkText = { url: ClaudeUrl, };
export type AttestOutcome = { "kind": "done" } & AttestDone | { "kind": "notApplicable" } & AttestNotApplicable | { "kind": "clear" } & Empty;
export type AttestDone = { note: Note, link: string | null, };
export type AttestNotApplicable = { reason: Note, };
export type SignBranch = { "kind": "current" } & Empty | { "kind": "other" } & SignOther;
export type SignOther = { branch: ShortText, };
export type Empty = Record<symbol, never>;
```

Event payloads (§6), generated from Rust so the webview and the emitter agree:

```ts
export type ProjectChanged = { projectId: ProjectId, rev: Rev, };
export type ProjectsChanged = { rev: Rev, };
export type PacketChanged = { projectId: ProjectId, stage: Stage, rev: Rev, };
export type DecisionPending = { projectId: ProjectId, count: number, };
export type TamperDetected = { projectId: ProjectId, stage: Stage, rev: Rev, };
export type RunStatusChanged = { runId: RunId, status: RunStatus, rev: Rev, };
export type ConfigChanged = { projectId: ProjectId, cwd: CwdKind, };
export type DoctorStatus = { level: DoctorLevel, };
export type SettingsChanged = { rev: Rev, };
```

## 3. Errors

```rust
#[serde(rename_all = "camelCase")]
struct IpcError { code: IpcErrorCode, message: String, detail: Option<IpcErrorDetail> }

enum IpcErrorCode { Invalid, NotFound, Expired, Stale, ReadOnly, NotAllowed, Conflict, PolicyViolation,
                    ToolMissing, ToolFailed, Clipboard, Cancelled, Unsupported, Io, Internal }

#[serde(tag = "kind", rename_all = "camelCase")]
enum IpcErrorDetail {
  Invalid { field: String, rule: String },
  Stale { what: StaleWhat /* "projectRev"|"readStamp"|"fingerprint"|"plan" */, fresh: Option<serde_json::Value> },
  ReadOnly(ReadOnly),
  NotAllowed { rule: String },                    // e.g. "core-step-skip", "condition-locked-after-stage-2", "pty-not-live"
  AnnotationErrors { errors: Vec<AnnotationProblem> },   // (CR-003) code NotAllowed, rule "p1-annotation-errors": a pin refused for errors in Stages 1–2 or F.2
  Conflict { path: RepoPath, written: Vec<RepoPath> },
  PolicyViolation { path: String, rule: String },  // the path is shown with visible escapes
  ToolMissing { tool: ToolKind, path: Option<String> },
  ToolFailed { tool: String, exit: Option<i32>, timed_out: bool, stderr_tail: String },   // ≤ 4 KB, redacted
  Clipboard { why: ClipboardProblem /* "empty"|"notText"|"tooLarge" */ },
  Unsupported { feature: String, why: String },
}
```

(CR-003) `AnnotationProblem { stage: Option<Stage>, step: Option<String>, diagnostic: Diagnostic }` (`Diagnostic` as in `template_health`); `step` is the annotation ID (`2.3`, `F.2`). The list is in line order and holds every error that stops the pin; `message` already reads "Stage 2 step 2.3 has an annotation error on line 214: … Fix the blueprint, then pin again." for one error and lists each for several.

`message` is Foreman-written text for display; the UI branches only on `code` and `detail.kind` (ADR-004 §5). `ToolFailed.stderr_tail` passes through the SR-54 redaction layer. In TS, `fresh` is `unknown` and is narrowed by the calling method's own type.

## 4. Commands

Each entry: request struct (Rust) and its generated TS shape, response, error codes beyond the universal `Invalid · Internal`, features and acceptance criteria (`docs/scope.md`), constraining requirements (`docs/requirements.md`), mutation, and phase/milestone.

### 4.1 App

**`app_bootstrap`** · P1 · M2 · mutates **no**
```text
Rust  AppBootstrapReq {}
TS    Record<symbol, never>
Res   AppBootstrap { first_run: bool, settings_rev: Rev, projects: Vec<ProjectSummary>, interrupted: Vec<RunSummary>,
                     doctor: DoctorLevel, capabilities: Capabilities }       // RunSummary, Capabilities: §2.4
Err   Io
```
F-017, F-019, F-020 · AC-F019-01, AC-F020-10, AC-F017-03 · SR-57 (interrupted runs never relaunched). Reads only `settings.json`, `projects.json` and each `progress.json` (NFR-01).

**`settings_get`** · P1 · M2 · **no**
```text
Rust  SettingsGetReq {}
TS    Record<symbol, never>
Res   SettingsView { rev: Rev, dri_name: Option<String>, tools: ToolPaths, editor_bundle_id: Option<BundleId>,
                     blueprint_repo: Option<BlueprintRepoInfo>, theme: Theme, terminal: TerminalPrefs,
                     env_passthrough: Vec<EnvName>, login_path: Option<LoginPathView>,
                     global_variables: BTreeMap<String, String>, auth_key_allowlist: Vec<AllowlistEntry> }
Err   Io
```
F-029 · AC-F029-01…04 · SR-05, SR-07.

**`settings_update`** · P1 (DRI, editor) + P2 (rest) · M2/M4 · **app**
```text
Rust  SettingsUpdateReq { expect_rev: Rev, patch: SettingsPatch }
      SettingsPatch { dri_name: Option<ShortText>, editor: Option<EditorSetting>, theme: Option<Theme>,
                      terminal: Option<TerminalPrefs>, env_passthrough: Option<Vec<EnvName>>,
                      global_variables: Option<BTreeMap<String, ShortText>>, reset_tool: Option<ToolKind>,
                      clear_login_path: Option<bool>, remove_allowlist_entry: Option<Sha256>,
                      screen_reader_mode: Option<bool>, os_display: Option<OsDisplay> }
      OsDisplay { reduce_transparency: bool, increase_contrast: bool }       // reported by the webview, not stored
      EditorSetting = Set { bundle_id: BundleId } | Clear                    // #[serde(tag = "kind")]
      Theme = System | Vellum | Night;  TerminalPrefs { font_size: u8, scrollback: u32, mac_option_is_meta: bool }
TS    { expectRev: number; patch: { driName: string | null;
        editor: { kind: "set"; bundleId: string } | { kind: "clear" } | null;
        theme: "system" | "vellum" | "night" | null;
        terminal: { fontSize: number; scrollback: number; macOptionIsMeta: boolean } | null;
        envPassthrough: string[] | null; globalVariables: Record<string, string> | null;
        resetTool: "claude" | "git" | null; clearLoginPath: boolean | null; removeAllowlistEntry: string | null;
        screenReaderMode: boolean | null; osDisplay: { reduceTransparency: boolean; increaseContrast: boolean } | null } }
Res   SettingsView
Err   Stale · Cancelled · Conflict · Io
```
F-029 · AC-F029-02, -03, -04, -05, -07 · SR-05 (`env_passthrough` can't re-add an always-removed name: it is stored, and `build_env` still removes it, AC-F029-05). In every patch, `null` means "unchanged". A patch that sets `env_passthrough`, `reset_tool` or `remove_allowlist_entry` is applied only after a native confirmation listing the change (SR-24a); declining returns `Cancelled` and nothing in the patch is saved. `scrollback` is 1,000–100,000 (default 10,000, or the S-0 branch value); `font_size` 9–32. `screen_reader_mode` is the Settings "Screen reader mode" toggle (stored, default off): when on, CLI launches add `--ax-screen-reader` and xterm gets `screenReaderMode: true` (AC-XC-22). `os_display` is what the webview's `prefers-reduced-transparency: reduce` and `prefers-contrast: more` media queries currently report; it isn't stored, and Rust turns the window's vibrancy off when either is true (ADR-014, AC-XC-23). As built (M5-04): `osDisplay` is sent by the webview at start and on every change of `prefers-reduced-transparency` or `prefers-contrast: more` (`src/lib/os-display.ts`); it is not stored. Rust passes `reduceTransparency || increaseContrast` to `Os::set_window_reduced_effects`, which records it (`window::reduced_effects`) and clears the main window's effects.

**`settings_pick_tool`** · P1 · M2 · **app**
```text
Rust  SettingsPickToolReq { tool: ToolKind }
TS    { tool: "claude" | "git" }
Res   ToolPaths
Err   Cancelled · ToolMissing (picked file not executable, relative, inside a project, symlink into a project) · Io
```
F-029 · AC-F029-01, AC-F008-03 · SR-01, SR-24, SR-24a (native file dialog, then a native confirmation showing the resolved path and `--version` output). Rust opens the file dialog; the path never comes from the webview.

**`today_get`** · P1 (Next step rows) + P2 (inbox) · M2/M4 · **no**
```text
Rust  TodayGetReq {}
TS    Record<symbol, never>
Res   TodayView { rows: Vec<NextStepRow>, inbox: Vec<InboxItem> }        // inbox empty in P1
      NextStepRow { project: ProjectSummary, next: Option<StepSummary>, why_none: Option<String> }
      InboxItem { kind: InboxKind, project: ProjectId, step: Option<StepKey>, stage: Option<Stage>, at: Timestamp }
Err   Io
```
F-017 · AC-F017-01, -03, -04, -05, -07, -08 · SR-56 (inbox notifications are Rust-side templates).
As built (M4-25): `inbox` is sorted by AC-F017-07's rank (`foreman_app::inbox::rank`; items of one rank keep the project order) and each item also carries `projectName`, `title`, `detail`, `chain` and `run` (Foreman-written text only, SR-56). Sources: live runs announced `blocked`/`idle` through `runs::emit_status` (session-blocked, session-your-turn), `closeout::gather` + `gates::view` (packet landed/invalid, gate failed/signable), pending outside changes, the tamper check, a chat step `copied` more than 10 minutes ago (`chat-awaiting-paste`), wait steps whose day has come (`wait-elapsed`) and interrupted runs. `step-ready` is never emitted: the Next step rows already show every project's next step. Time-derived items are not events, so the webview refetches once a minute (`bus.TODAY_TICK_MS`). When Foreman isn't focused, a new `session-blocked`/`packet-landed` item shows a macOS notification from a template in `src-tauri/src/notify.rs` (once per item per app run); the Dock badge counts the first four kinds. A notification click only activates Foreman (no action handler is registered). The webview and `tauri-plugin-notification` get no permission: both calls are made from Rust.

**`do_next`** · P1 · M2 · **mem** (one-time plan)
```text
Rust  DoNextReq {}
TS    Record<symbol, never>
Res   DoNextOutcome = Nothing { why: String } | Cli { project: ProjectId, step: StepKey, prepare: PrepareOutcome }
                    | Chat { project: ProjectId, step: StepKey, ready: bool, blockers: Vec<Blocker> }
Err   Io
```
F-017, F-008 · AC-F017-02, -06, AC-F028-01 · SR-19. Runs the same `launch::prepare` as `launch_prepare` with `entry = doNext`; the UI then calls `launch_start` / `rearm_start` (CLI) or `chat_copy_and_open` (chat) on the same keystroke. Nothing launches inside `do_next`.

**Quit guard** (no command). Rust intercepts `RunEvent::ExitRequested` while runs are live and shows a native dialog, "3 sessions are running; they end when Foreman quits; you can resume each later"; on confirm every child gets SIGHUP and the app exits (F-009 · AC-F009-09 · SR-24a, SR-57).

### 4.2 First run and Doctor

**`blueprint_pick_repo`** · P1 · M2 · **app**
```text
Rust  BlueprintPickRepoReq { mode: PickMode }        // "autoDetect" (fixed candidates only) | "choose" (Rust dialog)
TS    { mode: "autoDetect" | "choose" }
Res   BlueprintRepoInfo { display_path: String, commit: String, version: String, grammar_version: u32,
                          dirty: bool, health: TemplateHealthSummary }
Err   Cancelled · NotFound (no candidate) · NotAllowed ("not-repo-root": `rev-parse --show-toplevel` ≠ the folder) · ToolMissing (git) · Io
```
F-019, F-001 · AC-F019-01, -03, AC-F001-18 · SR-31a (only `~/claude-blueprint`, `~/Developer/claude-blueprint`), SR-44, SR-45. `dirty` compares `hash-object --no-filters` of the blueprint file with the blob at HEAD (`rev-parse HEAD:<path>`); `git status` is never run.

**`template_health`** · P1 (summary) + P2 (screen) · M2/M4 · **no**
```text
Rust  TemplateHealthReq { project_id: Option<ProjectId> }     // None = the blueprint in Settings
TS    { projectId: string | null }
Res   TemplateHealth { version: String, grammar_version: u32, counts: Counts, stages: Vec<StageHealth>,
                       diagnostics: Vec<Diagnostic> }
      Diagnostic { severity: "error" | "warning", rule: String, file: String, line: u32, column: u32, message: String }
Err   NotFound · ReadOnly · Io
```
F-001, F-003 · AC-F001-06, -15, AC-F003-05, -06, AC-F019-02 · SR-27.

**`doctor_run`** · P1 (version, auth, git) + P2 (full) · M2/M4 · **spawn** (`claude -v`, `claude auth status` in `<app-data>/probe/` after its fingerprint passes, `git --version`, P2 toolchain `--version`)
```text
Rust  DoctorRunReq { scope: DoctorScope }       // "p1" | "full"
TS    { scope: "p1" | "full" }
Res   DoctorReport { level: DoctorLevel, checks: Vec<DoctorCheck>, capabilities: Vec<CapabilityRow> }
Err   (none; failures are rows)
```
F-019 · AC-F019-04…07, -09…-11, -14 · SR-01, SR-08, SR-17. Emits `doctor:status`. As built (M4-13): row IDs `path`, `version`, `auth` (P1 and full), `sensor`, `probeFolder` (action `openProbeFolder`), `cleanup` (effective `cleanupPeriodDays`, managed over user, default 30, recommendation 90, never written), `hooks` (`disableAllHooks` in user or managed settings), `allow` (`Bash`, `Bash(*)`, `Bash(pnpm exec *)`, `Bash(npx *)`, `Bash(pnpm dlx *)`, `Bash(bunx *)` and interpreter rules), `appendixA` (blueprint v2.2 Appendix A: `~/.claude/settings.json`, `CLAUDE.md`, the 8 global agents) in group `claude`; `git` (P1 and full), `node`, `pnpm`, `cargo` (fixed candidates, `--version`, cwd app data), `launchPath` (the PATH a launch gets), `loginPath` (action `captureLoginPath`) in group `toolchain`. `capabilities[].blocks` names the registered projects' steps that need the missing feature. `probes` and `probeNote` come from `settings.json` `contract` (full scope only).

**`doctor_capture_login_path`** · P2 (M2 if S-12 fails) · M4 · **spawn** (the single consented shell), **mem**
```text
Rust  DoctorCaptureLoginPathReq {}                      // Rust asks for consent in a native dialog first
TS    Record<symbol, never>
Res   LoginPathPreview { preview_id: PreviewId, shell: String, entries: Vec<PathEntryCheck> }
Err   NotAllowed ($SHELL not in /etc/shells) · Cancelled · ToolFailed
```
F-019 · AC-F019-12, -13 · SR-02. As built (M4-14): `$SHELL` from Foreman's own environment must be an absolute path listed in `/etc/shells` (`NotAllowed("shell-not-listed")`, before any dialog); the native consent dialog names the exact command; `foreman_sys::spawn` runs `SpawnRequest::LoginShellPath` with the fixed argv `-l -c 'printf %s "$PATH"'`, cwd app data, a 10 s timeout, 64 KB output cap and an environment of `HOME`, `USER`, `LOGNAME`, `TMPDIR`, `LANG`, `LC_*`, `SHELL` and `PATH=/usr/bin:/bin:/usr/sbin:/sbin`. The output is split on `:` (≤ 200 entries) and checked as SR-05 PATH entries; each entry comes back with `ok` and `why`. Timeout or a non-zero exit is `ToolFailed`.

**`doctor_login_path_apply`** · P2 · M4 · **app**
```text
Rust  DoctorLoginPathApplyReq { preview_id: PreviewId }
TS    { previewId: string }
Res   SettingsView
Err   Expired · Cancelled (native dialog listing the entries) · Conflict
```
F-019 · AC-F019-12 · SR-02, SR-05, SR-24a (entries sanitized again when PATH is built). As built: the dialog lists the kept entries and the number dropped; OK stores `loginPath { entries, shell, capturedAt }` by CAS.

**`probe_folder_open`** · P2 · M4 · **spawn** (interactive `claude` in a PTY tab, cwd `<app-data>/probe/`, no prompt)
```text
Rust  ProbeFolderOpenReq {}
TS    Record<symbol, never>
Res   RunStarted
Err   ToolMissing · Io
```
`output: Channel<tauri::ipc::Response>` · F-019 · AC-F019-08 · SR-17. P2, M4 (moved from M3 in the Stage 4.7 fix pass, with AC-F019-08 and ticket M3-18). As built (M3-18): the probe folder is reset first (`0700`, Foreman's settings file only), then `claude --name fm-probe` runs in it as an ordinary terminal tab (`pty_*` work on its `runId`; no `run.json`, no index line). Opening it records `settings.json` `probeFolderTrusted: true` (the trust step was done; Foreman can't see Billy's answer without acting on a session event, SR-18). A probe that then meets the trust prompt is inconclusive and Doctor says so.

**`probe_run`** · P2 · M4 · **spawn** (`claude --init-only` probes A and B), **app** (`settings.json` `contract`)
```text
Rust  ProbeRunReq {}
TS    Record<symbol, never>
Res   ContractResult { version: String, verified_at: Timestamp, outcomes: Vec<ProbeOutcome> }
Err   ToolMissing
```
`progress: Channel<LaunchProgress>` · F-019 · AC-F019-15…17 · SR-17, NFR-44. `launch_prepare` runs the same probes automatically before the first launch on a new version (AC-F008-21). As built (M4-12): `contract.version` is the semver of `claude -v`. `outcomes` are probe A then probe B of this run (§2.4 rows; `fail` = flag failure). Probe A's marker needs the hash-checked sensor; without it A is inconclusive. Before a launch the probes run only when `probeFolderTrusted` is set and this build has the sensor (otherwise `Contract { outcome: skipped }` and Doctor's `probeNote` says why). The stored contract turns into launch blockers only for the features a launch needs: `launch` and `chains` (`--session-id`; every launch passes it) always, `re-arm` (`--resume`) for a continuing step; an `--effort` failure is a warning (Claude Code ignores an unknown effort and runs), `desktop-handoff` is Desktop's.

### 4.3 Projects

**`projects_list`** · P1 · M2 · **no**
```text
Rust  ProjectsListReq {}
TS    Record<symbol, never>
Res   Vec<ProjectSummary>
      ProjectSummary { id: ProjectId, name: String, display_path: String, rev: Rev, read_only: Option<ReadOnly>,
                       current_stage: Option<Stage>, pinned: Option<u8> }
Err   Io
```
F-017, F-018 · AC-F017-05, -10.

**`setup_pick_folder`** · P1 · M2 · **mem**
```text
Rust  SetupPickFolderReq {}
TS    Record<symbol, never>
Res   FolderPick { pick_id: PickId, display_path: String, is_git_repo: bool, has_foreman: Option<SchemaState>,
                   refusal: Option<String> }    // inside another project, the blueprint repo, not writable
Err   Cancelled · Io
```
F-018 · AC-F018-06, -07 · SR-24.

**`setup_preview`** · P1 · M2 · **mem**
```text
Rust  SetupPreviewReq { pick_id: PickId, name: ShortText, slug: ShortText, variables: BTreeMap<String, ShortText>,
                        conditions: BTreeMap<ConditionId, TriState>, chat_project_url: Option<ClaudeUrl> }
TS    { pickId: string; name: string; slug: string; variables: Record<string, string>;
        conditions: Partial<Record<ConditionId, TriState>>; chatProjectUrl: string | null }
Res   SetupPreview { preview_id: PreviewId, writes: Vec<PlannedWrite>, git_init: bool, pin: PinPreview,
                     path: ApplicablePathSummary }
      PlannedWrite { path: RepoPath, action: "create" | "leaveAlone" | "refuse", reason: Option<String> }
Err   Expired (pick) · NotAllowed (pin refused: blueprint file differs from HEAD, AC-F002-02, shown as both hashes instead of `git status` output; or, (CR-003) a step in Stages 1–2 or F.2 has an annotation error, `detail: AnnotationErrors` listing each, AC-F002-02) · ToolMissing (git) · PolicyViolation
```
F-018, F-002, F-004 · AC-F018-01, -02, -04, -08, AC-F002-01, -02 · SR-30, SR-34, SR-44. A folder inside an enclosing repo but without its own (`rev-parse --show-toplevel` ≠ the folder) gets `gitInit: true`; Foreman never acts on the enclosing repo. Variable keys must be blueprint variable names (`^[A-Z][A-Z0-9_]*$`); `slug` must match `^[a-z0-9-]{1,40}$` (it becomes part of `--name`, DR-121).

**`setup_apply`** · P1 · M2 · **repo** (`.foreman/*`, `docs/…`, `spec/`, `git init`), **app**
```text
Rust  SetupApplyReq { preview_id: PreviewId }
TS    { previewId: string }
Res   ProjectSummary
Err   Expired · Stale · Conflict · PolicyViolation · ToolFailed (git init) · Io
```
F-018, F-002, F-020 · AC-F018-02, -03, -05, AC-F002-01 · SR-30, SR-32, SR-33, SR-44. No commit (AC-F018-03).

**`project_open`** · P1 · M2 · **app** (`progress.json` read stamps; approvals bootstrap), **repo** (idempotent `info/exclude` append only)
```text
Rust  ProjectOpenReq { project_id: ProjectId }
TS    { projectId: string }
Res   ProjectView { summary: ProjectSummary, conditions: BTreeMap<ConditionId, ConditionView>,
                    stages: Vec<StageView>, pending_changes: u32, read_only: Option<ReadOnly> }
      StageView { stage: Stage, title: String, applicable: Applicability, signed: Option<SignedInfo>,
                  steps: Vec<StepSummary>, packet: Option<PacketSummary> }
      StepSummary { key: StepKey, number: String, title: String, state: StepState, tag: Tag,
                    surface: Option<Surface>, spec_line: Option<SpecLine>, why_not_runnable: Option<String> }
Err   NotFound · Io  (read-only states are returned in the view, not as errors)
```
F-002, F-004, F-020, F-022, F-030 · AC-F002-04…07, AC-F020-05, -06, AC-F030-02, -03 · SR-31, SR-32 (exclude writer), SR-40, SR-41.

**`project_url_set`** · P1 · M3 · **repo** (`project.json`, a variable field: auto-accepted, ADR-022)
```text
Rust  ProjectUrlSetReq { project_id: ProjectId, url: Option<ClaudeUrl> }
TS    { projectId: string; url: string | null }
Res   Rev
Err   ReadOnly · Conflict
```
F-012 · AC-F012-02 · SR-25, PR-02 (g).

**`condition_set`** · P1 · M2 · **repo** (`project.json` + audit; commit by plumbing before Stage 5 is signed), **git**
```text
Rust  ConditionSetReq { project_id: ProjectId, condition: ConditionId, to: TriState, note: Note }
TS    { projectId: string; condition: ConditionId; to: TriState; note: string }
Res   ProjectView
Err   NotAllowed ("condition-locked-after-stage-2" for early conditions, "late-condition-locked-after-stage-4") · ReadOnly · Conflict · ToolFailed (git)
```
F-004, F-020 · AC-F004-10, -11, AC-F020-08, -09, AC-F023-06 · SR-40, SR-42, SR-44, SR-45.

**`variable_set`** · P1 · M2 · **repo** (`project.json` variables), **git** (before Stage 5)
```text
Rust  VariableSetReq { project_id: ProjectId, name: String, value: ShortText }
TS    { projectId: string; name: string; value: string }
Res   ProjectView
Err   NotFound (unknown variable) · ReadOnly · Conflict
```
F-018, F-006 · AC-F006-03 · SR-40.

**`blueprint_repin`** · P1 · M2 · **repo** (`blueprint.lock.json`), **app** (`templates/`), **git**
```text
Rust  BlueprintRepinReq { project_id: ProjectId }
TS    { projectId: string }
Res   ProjectView
Err   NotAllowed ("repin-after-stage-1-signed", "blueprint-dirty", "p1-annotation-errors" with `detail: AnnotationErrors`, CR-003) · ToolMissing · Conflict
```
F-002 · AC-F002-02, -03 · SR-41, SR-44. (CR-003) A re-pin runs the same checks as the first pin; on refusal the existing lock and stored text are untouched.

**`project_remove`** · P1 (registry) + P2 (full deletion UI) · M4 · **app** (deletes `projects/<id>/`, `approvals/<id>*`, its fingerprint snapshots)
```text
Rust  ProjectRemoveReq { project_id: ProjectId }        // native confirmation names what is deleted
TS    { projectId: string }
Res   Vec<ProjectSummary>
Err   Cancelled · NotAllowed ("runs-live") · Io
```
F-029, F-020 · AC-F029-06 · PR-05, SR-55. Never touches the repo.

**`project_pin_set`** · P2 · M4 · **app** (`projects.json` `pinned`)
```text
Rust  ProjectPinSetReq { project_id: ProjectId, slot: Option<u8> }   // 1..=9; None unpins
TS    { projectId: string; slot: number | null }
Res   Vec<ProjectSummary>
Err   NotFound · Invalid (slot outside 1–9)
```
F-017 · AC-F017-10. A slot already held by another project moves to this one (the other is unpinned). ⌘1–⌘9 open the project in that slot. Mock fixtures: `requests/project_pin_set.json` (`{ "projectId": <TradeLog>, "slot": 1 }`), `responses/project_pin_set.json`.

### 4.4 Steps

**`step_view`** · P1 · M2 · **no**
```text
Rust  StepViewReq { project_id: ProjectId, step: StepKey }
TS    { projectId: string; step: string }
Res   StepView { summary: StepSummary, prompt: Vec<PromptSegment>, inputs: Vec<RunInputView>,
                 start: Option<Explained<Start>>, switches: Vec<SwitchCue>, chips: Option<ChatChips>,
                 argv_preview: Option<Vec<String>>, env_names: Vec<EnvName>, blockers: Vec<Blocker>,
                 warnings: Vec<String>, remaps: Vec<String>, outs: Vec<OutView>, override_: Option<RunOverride> }
Err   NotFound · Io
```
F-005, F-006, F-030 · AC-F005-03, -04, AC-F006-12, -13, AC-F008-17, AC-F030-01, -04, AC-XC-32 · SR-19, SR-26. `argv_preview` is the argv the next launch would use (prompt shown in full here; the run log stores only its hash, SR-52).

**`run_input_set`** · P1 · M2 · **app** (`progress.json`; `remember` inputs per project)
```text
Rust  RunInputSetReq { project_id: ProjectId, step: StepKey, name: String, value: InputValue }
TS    { projectId: string; step: string; name: string; value: string }
Res   StepView
Err   NotFound (input not declared) · ReadOnly
```
F-006 · AC-F006-12, -14, AC-F008-11 · SR-28 (checked again in the resolved prompt), SR-27.

**`optional_part_set`** · P1 · M2 · **app**
```text
Rust  OptionalPartSetReq { project_id: ProjectId, step: StepKey, part_id: String, include: bool }
TS    { projectId: string; step: string; partId: string; include: boolean }
Res   StepView
Err   NotFound
```
F-006 · AC-F006-07.

**`run_override_set`** · P1 · M3 · **app**
```text
Rust  RunOverrideSetReq { project_id: ProjectId, step: StepKey, over: Option<RunOverride> }    // None clears
TS    { projectId: string; step: string; over: { model: Model | null; effort: Effort | null; mode: Mode | null } | null }
Res   StepView
Err   NotFound · NotAllowed (a value outside the step's surface vocabulary, e.g. a mode on a chat step)
```
F-005 · AC-F005-01, -05 · SR-03, SR-19 (an override that changes mode or model triggers `ModeOrModelChanged`). F-005's per-run chip needs it.

**`step_mark_done`** · P1 · M2 · **app**
```text
Rust  StepMarkDoneReq { project_id: ProjectId, step: StepKey }
TS    { projectId: string; step: string }
Res   StepView
Err   NotAllowed (CLI prompt steps finish by exit, not by mark; `wait` before its date) · ReadOnly
```
F-007 · AC-F007-04, -08 · DR-066. As built (integration, M3-26): a chat step marked done ends its run's clock (`runs::note_step_done`: a second index line for the Copy and open run, with its duration).

**`step_skip`** · P1 · M2 · **repo** (`overlay.json`, a decision file: commit by plumbing before Stage 5, pending after), **git**
```text
Rust  StepSkipReq { project_id: ProjectId, step: StepKey, reason: ShortText }
TS    { projectId: string; step: string; reason: string }
Res   ProjectView
Err   NotAllowed ("core-step-skip", "reason-required", "conditional-step-skip") · ReadOnly · Conflict
```
F-004 · AC-F004-05, -07 · SR-40.

**`action_preview`** · P1 · M2 · **mem**
```text
Rust  ActionPreviewReq { project_id: ProjectId, step: StepKey }
TS    { projectId: string; step: string }
Res   ActionPreview { preview_id: PreviewId, action: ActionKind, writes: Vec<Diff> }   // "writeBlocks" | "copyFile" | "mkdir"
Err   NotFound · NotAllowed (step isn't an action) · PolicyViolation · ReadOnly
```
F-007 · AC-F007-02, -03 · SR-34. `action` steps (Council setup, the Stage 1 brief copy) need it.

**`action_apply`** · P1 · M2 · **repo**
```text
Rust  ActionApplyReq { preview_id: PreviewId }
TS    { previewId: string }
Res   StepView
Err   Expired · Stale · Conflict · PolicyViolation
```
F-007 · AC-F007-02, -03 · SR-30, SR-33, SR-34.

**`wait_start`** · P1 · M3 · **app** (`progress.json`), schedules a Rust-side notification
```text
Rust  WaitStartReq { project_id: ProjectId, step: StepKey, until: IsoDate }
TS    { projectId: string; step: string; until: string }
Res   StepView
Err   NotAllowed (not a wait step; date not after today)
```
F-007 · AC-F007-05, -09 · SR-56. The grammar has no duration key, so Billy enters the date (domain-rules DR-067).

**`clipboard_copy`** · P1 · M2 · **clip**, **app** (run log `copyPromptOnCli`, step state `copied` for followups)
```text
Rust  ClipboardCopyReq { project_id: ProjectId, target: CopyTarget, step: Option<StepKey>, chain: Option<ChainId>,
                         path: Option<RepoPath>, stage: Option<Stage> }
TS    { projectId: string; target: "prompt" | "command" | "sessionId" | "fileContents" | "fixRequest";
        step: string | null; chain: string | null; path: string | null; stage: number | null }
Res   CopyResult { removed_controls: u32, focus_run: Option<RunId> }   // focus_run: followup → focus its chain's tab
Err   NotFound · Invalid (the target needs a field that is null) · NotAllowed ("unresolved-tokens", "prompt-control-char")
```
F-005, F-007, F-009, F-023, F-026 · AC-F007-07, AC-F009-05, -16, AC-F026-01, AC-F023-01, AC-F013-12 · SR-22, SR-28 (text sanitized before it reaches the clipboard), SR-56. Covers Copy prompt (⌘⇧C), Copy command (⌘⌥C), Copy session ID, `followup` Copy-and-focus, manifest Copy contents and the fix request. One typed command covers them all; the fix request is `clipboard_copy { target: "fixRequest" }`. As built (M2-06, M3-17): `command` runs the same preparation as `launch_prepare` (the UI never builds argv) and copies the program and argv, POSIX shell-quoted; a blocked step answers `NotAllowed("not-runnable")`. A `followup`'s `prompt` copy returns `focusRun`, the live run of its chain, or `null` when the chain's process has exited (the UI then relaunches it, DR-068).

### 4.5 Chat

**`chat_copy_and_open`** · P1 · M3 · **clip**, **app** (run record with time-to-start, step `copied`), opens a URL
```text
Rust  ChatCopyAndOpenReq { project_id: ProjectId, step: StepKey, entry: EntryPoint }
TS    { projectId: string; step: string; entry: EntryPoint }
Res   ChatCopyResult { run_id: RunId, opened: bool, missing_link: Option<"project" | "conversation">,
                       removed_controls: u32 }
Err   NotFound · ReadOnly · NotAllowed ("unresolved-tokens", "prompt-control-char") · Io
```
F-012, F-026 · AC-F012-03, -04, AC-F026-02, AC-F017-06 · SR-25, SR-28, SR-52. As built (M3-23): `NotAllowed` also carries `not-a-chat-step` and `not-runnable` (the step refuses a copy: not applicable, undecided, waiting, with the reason). The URL opens through `foreman-sys` (`/usr/bin/open -u <url>`, CR-006) only after the stored link passes the SR-25 allowlist again; a stored link that fails it counts as missing (`missingLink`). If `open` itself fails, the prompt stays copied and the result is `opened: false, missingLink: null`. The run record is `kind: chatCopy`, `status: copied`, prompt hash only; a per-run override is consumed by the copy (DR-103). As built (integration, M3-26): the record and its one index line are written by `runs::record_chat_copy` (the run log's single path): the hash is of the sanitized prompt that went onto the clipboard, the step's run-input interventions are carried, and `override` is set when one was used.

**`chat_link_set`** · P1 · M3 · **repo** (`project.json` `chatUrls`, variable field), reads the clipboard when asked
```text
Rust  ChatLinkSetReq { project_id: ProjectId, chain_start: StepKey, source: LinkSource }
      LinkSource = Clipboard | Text { url: ClaudeUrl }            // #[serde(tag = "kind")]
TS    { projectId: string; chainStart: string; source: { kind: "clipboard" } | { kind: "text"; url: string } }
Res   Rev
Err   Clipboard · Invalid (not https://claude.ai/…) · Conflict
```
F-012 · AC-F012-02 · SR-25, SR-56, PR-02 (g). As built (M3-23): `chainStart` may name any chat step of the chain; the link is stored under the chain's start step. `Invalid { field: "chainStart", rule: "chatChainStart" }` when it isn't a chat chain; the clipboard text is trimmed before the check.

**`paste_back_preview`** · P1 (packet, output) + P2 (ADR set, lessons) · M3/M4 · reads the clipboard, **mem**
```text
Rust  PasteBackPreviewReq { project_id: ProjectId, step: StepKey, kind: PasteKind, source: PasteSource }
      PasteSource = Clipboard | Sensor            // Sensor: P2, the run's `last_assistant_message` (Capture from session)
TS    { projectId: string; step: string; kind: "auto" | "packet" | "output" | "adrSet" | "lessons"; source: "clipboard" | "sensor" }
Res   PastePreview { preview_id: PreviewId, kind: PasteKind, targets: Vec<Diff>, problems: Vec<Problem>,
                     notes: Vec<String>, packet_fields: Option<PacketFields> }
Err   Clipboard · Unsupported (`sensor` with no sensor, or no `last_assistant_message` for the step's run) · NotAllowed ("wrong-stage-packet", "two-packets", "appends-in-p1") · PolicyViolation · ReadOnly
```
F-012, F-013, F-016 · AC-F012-05…-11, -13, -14, -15, AC-F013-14, AC-F016-02 · SR-18, SR-26, SR-35, SR-56. Diffs show bidi, zero-width and control characters as visible escapes. `source: "sensor"` (P2, M4) reads the text the sensor recorded for the step's last run; it goes through the same normalize, split, `check_write` and preview as a clipboard paste, and only Billy's Apply writes it (SR-18). Mock fixtures: one request per source (`requests/paste_back_preview.json`, `requests/paste_back_preview.sensor.json`). As built (M3-24): `NotAllowed` also carries `no-packet` (a packet step's paste has no header), `not-a-packet-step` (`kind: packet` on a step without a packet `out`) and `no-output` (the step declares nowhere to write); Each other declared `out` of a packet step receives the whole paste (packet, then attachments). Problems carry `fix: ["openInEditor", "copyFixRequest"]`. As built (Stage 5 integration, M4-18, M4-23, M4-24): `kind: "lessons"` works only on a step whose `appends` is `docs/lessons.md` (else `NotAllowed("not-a-lessons-step")`); the file is read through `foreman_core::lessons::read` and the one target is `docs/lessons.md` with the append as its diff (each entry renumbered to the next free `L-NNN`, with a note; an entry for another stage is noted); a missing file is `NotAllowed("lessons-missing")` and nothing is written (AC-F016-04); a paste without an entry heading is `NotAllowed("no-lessons-entries")`. `kind: "auto"` on an appending step still answers `appends-in-p1` (AC-F012-14); the UI sends `lessons` for F.2 steps. `kind: "adrSet"` splits through `foreman_core::packet::adr_split` (`split_adrs`, `plan`): every `docs/adr/ADR-NNN.md` is listed with its diff against the current file before any write; `NotAllowed("no-adrs" | "duplicate-adr")`; a target the policy refuses is `PolicyViolation` for the whole paste. An ADR set makes the step done only when the step declares no `out` and no `appends`. `source: "sensor"` reads the step's last run's `last_assistant_message` through `foreman_sys::sensor::ingest` (the run's own `sensorToken`) and then takes the clipboard path; without a run, a sensor token or a `Stop` reply it answers `Unsupported { feature: "sensor", why: "noSensor" }` (every run until the P2 sensor hooks give runs a token, M4-08/M4-09).

**`paste_back_apply`** · P1 · M3 · **repo**
```text
Rust  PasteBackApplyReq { preview_id: PreviewId }
TS    { previewId: string }
Res   PasteApplyResult { written: Vec<RepoPath>, step_done: bool }
Err   Expired · Conflict (lists `written`) · PolicyViolation
```
F-012, F-016 · AC-F012-06…-08, -12, AC-F016-02, -03, -04 · SR-33, SR-35. As built (M3-24): every target's stamp is re-checked before the first write, so a file changed since the preview writes nothing (`Conflict` with `written: []`); a change racing the writes reports what was written. A valid packet or output makes the step `done` (a step never copied counts the paste-back as its copy) and emits `packet:changed` for a packet. As built (integration): a lessons append is written by `foreman_core::lessons::append` against the stamp read at preview (a changed file is `Conflict` on `docs/lessons.md`, nothing written; the next preview diffs the new text, AC-F016-03); ADR files are written by compare-and-swap like every other target. A step made `done` here ends its chat run's clock (`runs::note_step_done`: a second index line with the run's duration).

### 4.6 Launch

**`launch_prepare`** · P1 · M2 (preflight subset) / M3 (fingerprint, auth) · **spawn** (`claude -v`; `claude auth status` only after the fingerprint and block checks pass, and only on a cache miss; P2 probes on a version change), **mem**
```text
Rust  LaunchPrepareReq { project_id: ProjectId, step: StepKey, entry: EntryPoint, mode: PrepareMode }
      PrepareMode = Normal | FreshWithContext     // FreshWithContext: a new chain whose prompt starts with the last
                                                  // signed packet and the step's declared inputs (AC-F010-09, DR-113)
TS    { projectId: string; step: string; entry: EntryPoint; mode: "normal" | "freshWithContext" }
Res   PrepareOutcome = Ready(LaunchPreview) | Blocked { blockers: Vec<Blocker>, warnings: Vec<String> }
      LaunchPreview { plan_id: PlanId, expires_in_ms: u32, prompt: Vec<PromptSegment>, argv: Vec<String>,
                      env_names: Vec<EnvName>, run_settings_sha256: Sha256, confirm: Vec<ConfirmReason>,
                      rearm: Option<RearmNeed>, warnings: Vec<String>, remaps: Vec<String> }
      RearmNeed = IdleConfirmation { chain: ChainId } | SensorBusy { tasks: u32 } | Exited { chain: ChainId }
Err   NotFound · ReadOnly · NotAllowed ("fresh-not-continuing": `freshWithContext` on a step that doesn't continue a chain) · Io
```
`progress: Channel<LaunchProgress>` · F-008, F-010, F-017 · AC-F008-01…-21, -24, AC-F010-01, -02, -03, -09, -10, -11, AC-F017-02 · SR-01, SR-03…SR-09, SR-17, SR-19, SR-24, SR-28. The order of checks is fixed (ADR-021, DR-120). The UI never sends argv. `mode: "freshWithContext"` is what the broken-chain banner's "Start a fresh chain with context" button sends, and what S-8's ambiguous-fork branch uses; it never reads the old transcript. Mock fixtures: `requests/launch_prepare.json` (normal) and `requests/launch_prepare.fresh.json`; the `brokenChain` scenario answers the normal request with `TranscriptMissing` and the fresh one with `Ready`.

**`chain_resume_prepare`** · P1 · M3 · **spawn** (as `launch_prepare`), **mem**
```text
Rust  ChainResumePrepareReq { chain: ChainId }
TS    { chain: string }
Res   PrepareOutcome                                   // plan for the chain's last step, `--resume <uuid>`, no new prompt
Err   NotFound · ReadOnly · NotAllowed ("chain-live", "chain-handed-off") · Io
```
`progress: Channel<LaunchProgress>` · F-009, F-010, F-020, F-026 · AC-F009-05, AC-F020-10, AC-F026-07 · SR-13, SR-14, SR-18, SR-19. Resume from a tab, from Interrupted sessions or from the Run log. Runs the full preflight (DR-120); Billy then calls `launch_start` with the plan, so the same native confirmation applies. A missing transcript returns `Blocked { TranscriptMissing }`, which offers `launch_prepare { mode: "freshWithContext" }`. Mock fixtures: `requests/chain_resume_prepare.json`, `responses/chain_resume_prepare.json` (Ready) and the `brokenChain` scenario.

**`launch_start`** · P1 · M2 (fake binary) / M3 (PTY tab) · **app** (`runs/<runId>/settings.json`, `run.json` before spawn), **spawn** (PTY)
```text
Rust  LaunchStartReq { plan_id: PlanId }      // if the plan has `confirm` reasons, Rust shows them in a native dialog
TS    { planId: string }
Res   RunStarted { run_id: RunId, chain: ChainId, tab_name: String }
Err   Expired · Stale (revision or fingerprint changed: fresh preview in detail) · Cancelled · NotAllowed ("needs-rearm") · ToolFailed · Io
```
`output: Channel<tauri::ipc::Response>` · F-008, F-009, F-026 · AC-F008-14…-16, -19, -20, -25, AC-F009-01, AC-F026-01, -03 · SR-01…SR-05, SR-09, SR-19, SR-52, SR-57. Emits `run:status`.

**`config_ack`** · P1 · M3 · **app** (`cache/fingerprints/<cwdHash>.json`, redacted, `0600`)
```text
Rust  ConfigAckReq { project_id: ProjectId, cwd: CwdKind, fingerprint: Sha256 }
TS    { projectId: string; cwd: CwdKind; fingerprint: string }
Res   ()                                       // TS: null
Err   Cancelled · Stale (fingerprint moved since the banner was shown) · Io
```
F-008 · AC-F008-04, -23 · SR-06, SR-24a, SR-55. Rust shows the native acknowledgement (the changed sources by name); the webview's banner only shows the redacted diff.

**`auth_key_allowlist_add`** · P1 · M3 · **app** (`settings.json`)
```text
Rust  AuthKeyAllowlistAddReq { hit: HitId }      // hit from Blocker::AuthKeyBlocked; Rust holds file/key/value hash
TS    { hit: string }
Res   SettingsView
Err   Expired · Cancelled · NotAllowed ("not-user-settings")
```
F-008, F-029 · AC-F008-22 · SR-07, SR-24a (native dialog names the file and key).

**`project_env_approve`** · P1 · M3, **only on the S-7′ fail branch** · **app** (`approvals/<id>.json`)
```text
Rust  ProjectEnvApproveReq { project_id: ProjectId, cwd: CwdKind, sha256: Sha256 }
TS    { projectId: string; cwd: CwdKind; sha256: string }
Res   ()
Err   Cancelled · Stale · Unsupported (S-7′ passed)
```
F-008 · AC-F008-07 · SR-06, SR-07, SR-24a.

### 4.7 Terminal

**`pty_attach`** · P1 · M3 · **no**
```text
Rust  PtyAttachReq { run_id: RunId }
TS    { runId: string }
Res   AttachInfo { status: RunStatus, replayed_lines: u32, tab_name: String }
Err   NotFound
```
`output: Channel<tauri::ipc::Response>` (replays the last 2,000 lines, then live) · F-009 · AC-F009-01, -07 · SR-22.

**`pty_write`** · P1 · M3 · **spawned process input**
```text
Rust  PtyWriteReq { run_id: RunId, data: PtyData }
TS    { runId: string; data: string }
Res   ()
Err   NotAllowed ("pty-not-live") · NotFound
```
F-009 · AC-F009-19 · SR-24 (residual risk recorded in the threat model).

**`pty_paste`** · P1 · M3 · reads the clipboard
```text
Rust  PtyPasteReq { run_id: RunId }
TS    { runId: string }
Res   PasteText { text: String, removed_controls: u32 }    // the webview calls term.paste(text)
Err   Clipboard · NotAllowed ("pty-not-live")
```
F-009 · AC-F009-15 · SR-28, SR-56.

**`pty_resize`** · P1 · M3 · **no**
```text
Rust  PtyResizeReq { run_id: RunId, cols: u16, rows: u16 }      // 2..=1000 cols, 2..=500 rows
TS    { runId: string; cols: number; rows: number }
Res   ()
Err   NotFound · NotAllowed ("pty-not-live")
```
F-009 · AC-F009-01.

**`pty_flow`** · P1 · M3 · **no**
```text
Rust  PtyFlowReq { run_id: RunId, paused: bool }
TS    { runId: string; paused: boolean }
Res   ()
Err   NotFound
```
F-009 · AC-F009-10 · NFR-08.

**`pty_interrupt`** · P1 · M3 · process input (`\x03`)
```text
Rust  PtyInterruptReq { run_id: RunId }
TS    { runId: string }
Res   ()
Err   NotAllowed ("pty-not-live")
```
F-009 · AC-F009-04 · SR-18.

**`pty_terminate`** · P1 · M3 · signals (SIGTERM to the process group, SIGKILL after 5 s)
```text
Rust  PtyTerminateReq { run_id: RunId }       // native confirmation (always in P1; when not idle in P2)
TS    { runId: string }
Res   ()
Err   Cancelled · NotFound
```
F-009, F-010 · AC-F009-04, -09, AC-F010-05 · SR-13.

**`rearm_start`** · P1 · M3 · process input (`\x03`, `/exit\r`), **spawn** (relaunch), **app**
```text
Rust  RearmStartReq { plan_id: PlanId }    // P1: native "Claude is idle and nothing is running in the background" dialog
TS    { planId: string }
Res   RearmOutcome = Relaunched(RunStarted) | EndYourself { chain: ChainId }    // "Claude didn't exit…"
Err   Expired · Stale · Cancelled · NotAllowed ("no-rearm-needed", P2: "background-tasks-running" until Billy picks End anyway in the native dialog)
```
`output: Channel<tauri::ipc::Response>`, `progress: Channel<LaunchProgress>` — the one command with two Channels; IPC-R1 allows one Channel **per role**. F-010 · AC-F010-03…-06, -10, -11, -13, -14, AC-F013-11 · SR-13, SR-14, SR-18. After the old process exits, Rust prepares again; if the new argv or prompt hash differs from the confirmed plan, it stops with `Stale` and the new preview (DR-116). (ADR-008 names the argument `stepId`; this contract takes the prepared `planId` so a re-arm always goes through the same preview and confirmation.) As built (M4-10): when Billy presses Run, Rust reads the old session's sensor state: idle with nothing outstanding → no dialog (AC-F010-13); outstanding subagents, tasks or background shells → the native "N background tasks still running" dialog, where Wait answers `NotAllowed("background-tasks-running")` and End anyway re-arms (AC-F010-14); no sensor, no events, or a session working or blocked with nothing outstanding → the P1 idle dialog (AC-F011-09). The preview's `rearm` stays `idleConfirmation` (the answer that counts is read at Run); `sensorBusy` isn't sent.

**`rearm_continue`** · P1 · M3 · **spawn**
```text
Rust  RearmContinueReq { chain: ChainId }      // after "End it yourself, then press Continue"
TS    { chain: string }
Res   RearmOutcome
Err   NotAllowed ("process-still-alive") · Expired
```
`output`, `progress` Channels as above · F-010 · AC-F010-05, -11 · SR-13, SR-18.

**`terminal_link_open`** · P1 · M3 · opens a URL after a native confirmation
```text
Rust  TerminalLinkOpenReq { run_id: RunId, url: String }     // ≤ 8 KB; parsed in Rust
TS    { runId: string; url: string }
Res   LinkOutcome     // "opened" | "refused" | "cancelled"
Err   NotFound
```
F-009 · AC-F009-17 · SR-29, SR-26 (the dialog shows the URL with visible escapes). As built (M3-12): a refused URL answers `refused` without a dialog; `Invalid { field: "url", rule: "tooLong" }` over 8 KB; `ToolFailed` when `open` fails. The URL opens through `foreman-sys` (`/usr/bin/open -u`, CR-006), never in the webview.

### 4.8 Files

**`file_open`** · P1 · M2 · **spawn** (`/usr/bin/open -b <editor> <abs>`)
```text
Rust  FileOpenReq { project_id: ProjectId, path: RepoPath }
TS    { projectId: string; path: string }
Res   ()
Err   PolicyViolation (read policy, extension not .md/.json/.txt, symlink) · NotAllowed ("no-editor-set") · NotFound
```
F-013, F-020, F-029 · AC-F029-07, AC-XC-28, AC-F020-05 · SR-25a, SR-31, SR-50.

**`file_reveal`** · P1 · M3 · **spawn** (`open -R`)
```text
Rust  FileRevealReq { project_id: ProjectId, path: RepoPath }
TS    { projectId: string; path: string }
Res   ()
Err   PolicyViolation · NotFound
```
F-023, F-013 · AC-XC-28 · SR-25a.

**`terminal_open_here`** · P2 · M4 · **spawn** (Terminal by bundle ID, at the project root)
```text
Rust  TerminalOpenHereReq { project_id: ProjectId }
TS    { projectId: string }
Res   ()
Err   NotFound · ToolFailed
```
F-014 · AC-F014-11, AC-XC-28 · SR-25a.
As built (M4-25): `/usr/bin/open -b com.apple.Terminal <project root>` through `foreman-sys::spawn` (`SpawnRequest::OpenWithBundle`) behind the `Os::open_terminal` seam; no shell and no command is typed into Terminal. A project whose folder is missing answers `ReadOnly`/`NotFound`; a non-zero `open` is `ToolFailed`.

### 4.9 Decisions and recovery

**`pending_changes_list`** · P1 · M2 · **no**
```text
Rust  PendingChangesListReq { project_id: ProjectId }
TS    { projectId: string }
Res   Vec<PendingChange>
      PendingChange { id: ChangeId, file: DecisionFileKind, detected_at: Timestamp, seen_sha256: Sha256,
                      diff: RedactedDiff }
Err   NotFound
```
F-022 · AC-F022-01, -02 · SR-40, SR-55.

**`pending_change_accept`** · P1 · M2 · **app** (approved copy + hash), **git** (before Stage 5)
```text
Rust  PendingChangeAcceptReq { project_id: ProjectId, change: ChangeId, seen_sha256: Sha256 }
TS    { projectId: string; change: string; seenSha256: string }
Res   ProjectView
Err   Cancelled · Stale (file changed again: new diff in detail) · NotFound
```
F-022 · AC-F022-02, -05 · SR-24a, SR-40, SR-42. Native dialog: "Accept this change to <file>? It changes: <decision keys>".

**`pending_change_reject`** · P1 · M2 · **repo** (approved content written back by CAS)
```text
Rust  PendingChangeRejectReq { project_id: ProjectId, change: ChangeId, seen_sha256: Sha256 }
TS    { projectId: string; change: string; seenSha256: string }
Res   ProjectView
Err   Stale · Conflict · NotFound
```
F-022 · AC-F022-02, -05 · SR-33, SR-40.

**`item_kind_confirm`** · P2 · M4 · **app** (`approvals/<id>.json`)
```text
Rust  ItemKindConfirmReq { project_id: ProjectId, item: ItemId, text_sha256: Sha256 }
TS    { projectId: string; item: string; textSha256: string }
Res   GateView
Err   Cancelled · Stale (bullet changed again) · NotFound
```
F-001, F-015 · AC-F001-16, -17 · SR-24a, SR-40. `textSha256` is the item's `GateItemView.textSha256`; the gate view also gives `previousText` (the pinned bullet's text when another item now has it, e.g. swapped bullets) so the webview can show both texts as context. The approval itself is the native dialog only.

**`progress_rebuild_preview`** · P1 · M3 · **mem**
```text
Rust  ProgressRebuildPreviewReq { project_id: ProjectId }
TS    { projectId: string }
Res   RebuildPreview { preview_id: PreviewId, stages: Vec<StageView>, lost: Vec<String> }
Err   NotFound · Io
```
F-020 · AC-F020-11, -12 · NFR-17. Opens the project even when `progress.json` is unreadable (the file is left as it is) and writes nothing. The rebuilt state: every step on the path of a signed stage is `done`, the step whose `out` is a stage's packet is `done` when the packet exists, Foreman's own pending decision changes are recomputed from the approvals, and pending outside changes are kept; chains, run inputs, overrides, optional parts, waits and gate runs are lost (`lost` says so). From M4, stage records add the signing entries. A missing `progress.json` (app data deleted) is rebuilt the same way on `project_open` without a preview, since nothing can be lost (M3-16).

**`progress_rebuild_apply`** · P1 · M3 · **app** (the corrupt file is kept as `progress.json.corrupt-<timestamp>`)
```text
Rust  ProgressRebuildApplyReq { preview_id: PreviewId }
TS    { previewId: string }
Res   ProjectView
Err   Expired · Conflict
```
F-020 · AC-F020-12. `Conflict` when `progress.json` changed after the preview (compare-and-swap against the stamp the preview read). No native dialog (not in IPC-R11's list): Billy's press after reviewing the preview is the confirmation. An unreadable `project_open` is `Io` naming the file; the UI offers Rebuild from repo on that error (M3-16).

### 4.10 Packets and run log

**`packet_view`** · P1 · M3 · **no**
```text
Rust  PacketViewReq { project_id: ProjectId, stage: Stage }
TS    { projectId: string; stage: number }
Res   PacketView { path: RepoPath, exists: bool, sha256: Option<Sha256>, header: Option<PacketHeader>,
                   fields: Vec<PacketFieldView>, problems: Vec<Problem>, normalization: Vec<NormalizationNote>,
                   signed: Option<Signature>, tamper: Option<Diff> }
Err   NotFound
```
F-013, F-014 · AC-F013-01…-09, -12, -13, AC-F014-15 · SR-26, SR-27. `normalization` lists the DR-131 notes for the file as it is on disk (one per change kind and field; `field` is a format key, `header`, or `""` for the whole file, e.g. a removed fence); empty means normalizing would change nothing that §5.2 notes. `problems` are DR-133's, plus (AC-F004-12) `packet.undecided-early-condition` "Decide <condition> before signing Stage 2" on a hand-signed, unconfirmed Stage 2 packet while an early condition is `undecided` (M3-21). `tamper` (M4-22) is the diff from the committed text to the working copy of a signed packet whose working hash differs from `signing.packetSha256`; the other fields show the committed text (`crate::signing::tamper_diff`).

**`packet_normalize_apply`** · P1 · M3 · **repo**
```text
Rust  PacketNormalizeApplyReq { project_id: ProjectId, stage: Stage, seen_sha256: Sha256 }
TS    { projectId: string; stage: number; seenSha256: string }
Res   PacketView
Err   Stale · Conflict
```
F-013 · AC-F013-08 · SR-33. Writes `packet::normalize_packet`'s output by compare-and-swap against the stamp read with `seenSha256` (no native dialog: Billy's press after reviewing the notes is the confirmation, and normalization isn't security-relevant under IPC-R11); a file that needs nothing returns the view unchanged; emits `packet:changed` (M3-21).

**`packet_signature_confirm`** · P1 · M3 · **app** (`approvals/<id>.json` `outsideSignatures`)
```text
Rust  PacketSignatureConfirmReq { project_id: ProjectId, stage: Stage, seen_sha256: Sha256 }
TS    { projectId: string; stage: number; seenSha256: string }
Res   PacketView
Err   Cancelled · Stale (packet changed) · NotAllowed ("not-signed-form", "undecided-early-condition" for Stage 2)
```
F-013, F-004 · AC-F013-09, AC-F004-12 · SR-24a, SR-47. A packet whose `APPROVED BY` is `<name> on <date>` but wasn't written by Foreman's signing counts as signed only after this native confirmation ("Did you sign Stage N yourself? Claude writing a name never signs a packet."); the confirmation records the packet hash, so any later change voids it (DR-135). 

**`packet_repair_prepare`** · P2 · M4 · **spawn** (as `launch_prepare`), **mem**
```text
Rust  PacketRepairPrepareReq { project_id: ProjectId, stage: Stage }
TS    { projectId: string; stage: number }
Res   PrepareOutcome          // rearm of the stage's chain; prompt "Fix only these problems in docs/handoff/stage-NN.md: … Do not change anything else."
Err   NotFound · ReadOnly · NotAllowed ("no-problems", "not-cli-packet", "chain-handed-off")
```
`progress: Channel<LaunchProgress>` · F-013, F-010 · AC-F013-11 · SR-13, SR-18, SR-28. The problem list comes from Rust's own validation of the current packet, never from the webview; the prompt passes `check_prompt`. Billy continues with `rearm_start` (its native idle dialog applies). Mock fixtures: `requests/packet_repair_prepare.json`, `responses/packet_repair_prepare.json`.

**`metrics_export`** · P1 · M3 · writes to a path chosen in the Rust save dialog
```text
Rust  MetricsExportReq {}
TS    Record<symbol, never>
Res   ExportResult    // "saved" with display path | "cancelled"
Err   Io
```
F-026 · AC-F026-04…-06 · SR-52, SR-30 (`WritePurpose::UserExport`: `.md` only, path from the save dialog).

**`metrics_view`** · P2 · M4 · **no**
```text
Rust  MetricsViewReq { project_id: Option<ProjectId> }      // None = every project
TS    { projectId: string | null }
Res   MetricsView { per_surface: Vec<SurfaceMetrics>, manual_edits_by_stage: Vec<StageEdits>,
                    stages_through_foreman: u32, baseline: Option<BaselineComparison>, runs_counted: u32 }
      SurfaceMetrics { surface: Surface, median_time_to_start_ms: u32, p95_time_to_start_ms: u32,
                       median_active_ms_per_step: u32 }
Err   NotFound · Io
```
F-026 · AC-F026-08, AC-F026-09, AC-SM-04 · SR-52, NFR-09, NFR-21. The metrics strip on the Run log screen (`docs/design/mockups/run-log.html`). Numbers come from the same functions as `metrics_export`, so the two always agree. Mock fixtures: `requests/metrics_view.json` (`projectId: null`) and `requests/metrics_view.project.json`; the empty-registry scenario returns zero runs.

**`runs_clear`** · P2 (Settings) · M4 · **app** (deletes `runs/`, `events/`)
```text
Rust  RunsClearReq {}
TS    Record<symbol, never>
Res   ()
Err   Cancelled · NotAllowed ("runs-live")
```
F-026, F-029 · AC-F026-10 · PR-05, SR-53.

**`runs_search`** · P2 · M4 · **no**
```text
Rust  RunsSearchReq { query: ShortText, project_id: Option<ProjectId>, cursor: Option<String>, limit: u16 }  // limit ≤ 200
TS    { query: string; projectId: string | null; cursor: string | null; limit: number }
Res   RunsPage { runs: Vec<RunSummary>, next: Option<String> }
Err   Io
```
F-026 · AC-F026-07, -08 · SR-52 (argv shows `<prompt sha256:…>`).

### 4.11 Gates (P2)

**`gate_view`** · M4 · **no**
```text
Rust  GateViewReq { project_id: ProjectId, stage: Stage }
TS    { projectId: string; stage: number }
Res   GateView { stage: Stage, state: GateState, items: Vec<GateItemView>, closeout: Vec<CloseoutStep>,
                 signable: bool, why_not: Vec<String> }
Err   NotFound
```
F-015, F-016 · AC-F015-01, -06, -08…-10, AC-F014-01 · SR-47.

**`gate_item_attest`** · M4 · **app** (`progress.json` gate run)
```text
Rust  GateItemAttestReq { project_id: ProjectId, stage: Stage, item: ItemId, outcome: AttestOutcome }
      AttestOutcome = Done { note: Note, link: Option<String> } | NotApplicable { reason: Note } | Clear
TS    { projectId: string; stage: number; item: string;
        outcome: { kind: "done"; note: string; link: string | null } | { kind: "notApplicable"; reason: string } | { kind: "clear" } }
Res   GateView
Err   NotAllowed ("note-too-short", "note-repeated", "time-not-elapsed", "kind-needs-reconfirm") · NotFound
```
F-015 · AC-F015-02, -03, AC-F001-17 · SR-47.

**`gate_import_results`** · M4 · **no** (reads the sensor's `last_assistant_message` for the gate.A run)
```text
Rust  GateImportResultsReq { project_id: ProjectId, stage: Stage }
TS    { projectId: string; stage: number }
Res   Vec<ImportProposal>     // { item, proposed: "pass" | "fail" | "unmatched", evidence: TextSpan[] }
Err   Unsupported (no sensor) · NotFound
```
F-015, F-011 · AC-F015-05 · SR-18, SR-27.

**`gate_import_confirm`** · M4 · **app**
```text
Rust  GateImportConfirmReq { project_id: ProjectId, stage: Stage, item: ItemId, result: PassFail, evidence: Note }
TS    { projectId: string; stage: number; item: string; result: "pass" | "fail"; evidence: string }
Res   GateView
Err   NotFound
```
F-015 · AC-F015-05.

**`gate_fix_start`** · M4 · **mem** (a prepared plan for a new chain)
```text
Rust  GateFixStartReq { project_id: ProjectId, stage: Stage, item: ItemId }
TS    { projectId: string; stage: number; item: string }
Res   PrepareOutcome          // or the F.4 prompt on the third failure
Err   NotAllowed ("item-not-failed")
```
`progress: Channel<LaunchProgress>` · F-015 · AC-F015-07.

### 4.12 Signing (P2)

**`sign_preview`** · M4 · **mem**
```text
Rust  SignPreviewReq { project_id: ProjectId, stage: Stage, branch: SignBranch, name: ShortText, date: IsoDate }
      SignBranch = Current | Other { branch: ShortText }         // ref name validated by `git check-ref-format` rules in Rust
TS    { projectId: string; stage: number; branch: { kind: "current" } | { kind: "other"; branch: string }; name: string; date: string }
Res   SignPreview { preview_id: PreviewId, packet_sha256: Sha256, short: String, added_fields: Diff,
                    paths: Vec<RepoPath>, confirm_conditions: Option<Vec<ConditionId>>, chain_busy: bool,
                    blockers: Vec<String> }
Err   NotAllowed ("not-signable") · ToolMissing (git) · ReadOnly
```
F-014 · AC-F014-01…-03, AC-F004-13 · SR-42, SR-47.

**`sign_apply`** · M4 · **repo**, **git** (plumbing commit + read-back)
```text
Rust  SignApplyReq { preview_id: PreviewId }   // native dialog: name, date, "Signing stage-05.md @ a3f9c1", conditions, chain state
TS    { previewId: string }
Res   SignResult = Signed { commit: String, stage: Stage } | CommitPending { stderr_tail: String }
                 | ReadBackMismatch { diffs: Vec<Diff> }
Err   Expired · Cancelled · Stale ("packet changed since you reviewed it") · Conflict · ToolFailed
```
`progress: Channel<SignProgress>` · F-014, F-016 · AC-F014-04…-14, AC-F013-15 · SR-42…SR-47, SR-53 (sensor events of the stage deleted). Built (M4-20, M4-21): the native dialog names the DRI, the date, "Signing stage-NN.md @ <6 hex>", the branch, Stage 2's conditions and a running session. Read-back runs before the real-index refresh, so `ReadBack` messages come before `RefreshIndex` (a mismatch leaves Billy's index alone). On another branch nothing is written to the working tree, so a failed commit there answers `ToolFailed` and records nothing (sign again); `CommitPending` is the current branch's state after phase 1.

**`sign_retry_commit`** · M4 · **git**
```text
Rust  SignRetryCommitReq { project_id: ProjectId, stage: Stage }
TS    { projectId: string; stage: number }
Res   SignResult
Err   NotAllowed ("not-commit-pending") · ToolFailed
```
`progress: Channel<SignProgress>` · F-014 · AC-F014-11, -13.

**`tamper_restore`** · M4 · **repo** (committed blob written back by CAS)
```text
Rust  TamperRestoreReq { project_id: ProjectId, stage: Stage, seen_sha256: Sha256 }
TS    { projectId: string; stage: number; seenSha256: string }
Res   PacketView
Err   Stale · Conflict
```
F-014 · AC-F014-15 · SR-46.

### 4.13 Lessons, manifest, assembler (P2)

**`lessons_status`** · M4 · **no**
```text
Rust  LessonsStatusReq { project_id: ProjectId, stage: Stage }
TS    { projectId: string; stage: number }
Res   LessonsStatus { met: bool, entries: Vec<LessonRef>, next_id: String, file_present: bool }
Err   NotFound
```
F-016 · AC-F016-05, -06 · SR-47. Appending goes through `paste_back_preview { kind: "lessons" }` / `paste_back_apply`.

**`manifest_view`** · M4 · **no**
```text
Rust  ManifestViewReq { project_id: ProjectId }
TS    { projectId: string }
Res   Manifest { files: Vec<ManifestRow>, ready: u32, total: u32, first_producer: Option<StepKey> }
Err   NotFound
```
F-023 · AC-F023-01, -02.

**`manifest_mark`** · M4 · **app**
```text
Rust  ManifestMarkReq { project_id: ProjectId, path: RepoPath, in_project: bool }
TS    { projectId: string; path: string; inProject: boolean }
Res   Manifest
Err   NotFound
```
F-023 · AC-F023-01.

**`assembler_view`** · M4 · **no**
```text
Rust  AssemblerViewReq { project_id: ProjectId }
TS    { projectId: string }
Res   Assembler { rows: Vec<AssemblerRow>, complete: bool, late_conditions: Vec<LateConditionView> }
      AssemblerRow { path: String, state: "exists" | "missing" | "empty" | "refused", rule: Option<String> }
Err   NotFound · NotAllowed ("no-stage-4-packet")
```
F-023 · AC-F023-03…-06 · SR-30.

**`assembler_paste_preview`** · M4 · reads the clipboard, **mem**
```text
Rust  AssemblerPastePreviewReq { project_id: ProjectId, path: RepoPath }   // must be a REPO FILES row
TS    { projectId: string; path: string }
Res   PastePreview
Err   Clipboard · PolicyViolation · NotFound (not a REPO FILES row)
```
F-023 · AC-F023-03, -04, AC-F021-06 · SR-30, SR-35, SR-56.

**`assembler_paste_apply`** · M4 · **repo**
```text
Rust  AssemblerPasteApplyReq { preview_id: PreviewId }   // root CLAUDE.md: native "Claude Code loads this file in every session"
TS    { previewId: string }
Res   Assembler
Err   Expired · Cancelled · Conflict
```
F-023, F-021 · AC-F021-06 · SR-30 (`WritePurpose::AssemblerClaudeMd`).

### 4.14 Desktop, overlay additions, run sheet (P2)

**`desktop_open`** · M4 · **spawn** (`claude --desktop`, step cwd, no other flags), **clip**
```text
Rust  DesktopOpenReq { project_id: ProjectId, step: StepKey }
TS    { projectId: string; step: string }
Res   DesktopOpened { chips: ChatChips }
Err   ToolMissing · NotAllowed ("not-desktop-step") · ToolFailed
```
`progress: Channel<LaunchProgress>` · F-024 · AC-F024-01, -04 · SR-01, SR-05, SR-08.

**`desktop_handoff`** · M4 · **spawn** (`claude --desktop --resume <uuid>`), **app** (chain `handed-off`)
```text
Rust  DesktopHandoffReq { chain: ChainId }
TS    { chain: string }
Res   ()
Err   NotFound · ToolMissing · NotAllowed ("chain-live": end it first)
```
F-024, F-010 · AC-F024-02, -03 · SR-14.

**`add_step`** · M4 · **repo** (`overlay.json`)
```text
Rust  AddStepReq { project_id: ProjectId, stage: Stage, after: StepKey, copy_config_from: StepKey,
                   title: ShortText, text: Note, config: AddedStepConfig, tag: AddedTag }   // "optional" | "core"
TS    { projectId: string; stage: number; after: string; copyConfigFrom: string; title: string; text: string;
        config: { surface: Surface; cwd: CwdKind | null; start: Start; out: string[] }; tag: "optional" | "core" }
Res   ProjectView
Err   NotAllowed ("bypass-permissions", "invalid-start") · PolicyViolation (an `out` path) · Conflict
```
F-025 · AC-F025-01, -03, -04, -05 · SR-03, SR-19, SR-30. As built (M4-30, M4-31): `NotAllowed` also carries `stage-not-applicable`, `wrong-stage` (`after` is not in `stage`), `bad-title` (empty, over 200 characters or a line break), `bad-text` (empty or over 4000), `bad-out` (more than 8 outputs) and `too-many` (999 added steps in a stage); `invalid-start` means the start config is not the one the surface needs (a chat step has no mode or folder and adaptive effort; a CLI step needs a folder, a mode and a non-adaptive effort). Nothing is written when a check fails. Foreman numbers the step `x-<stage>-<n>` (highest `n` in the overlay plus one) and writes it to `.foreman/overlay.json` through `Opened::own_change`. The project's effective template is the project-local template with each added step spliced in after `after`, so the path, stage view, resolver and launch treat an added step like any other; stage state counts template steps only (DR-054), and a launch of an added step always asks first (`ConfirmReason::OverlayStep`, SR-19).

**`add_item`** · M4 · **repo** (`overlay.json`)
```text
Rust  AddItemReq { project_id: ProjectId, stage: Stage, text: ShortText }
TS    { projectId: string; stage: number; text: string }
Res   GateView
Err   Conflict
```
F-025 · AC-F025-02. As built (M4-30, M4-31): the item is an `attest` item `xi-<stage>-<n>`; it shows in the gate and in the close-out checklist after the template's items. `NotAllowed` carries `bad-text` and `wrong-stage` like `add_step`.

**`run_sheet_export`** · M4 · writes to the Rust save dialog's path
```text
Rust  RunSheetExportReq { project_id: ProjectId, stage: Stage }
TS    { projectId: string; stage: number }
Res   ExportResult
Err   NotFound · Io
```
F-027 · AC-F027-01…-03 · NFR-14, SR-30 (`WritePurpose::UserExport`).

### 4.15 Command count by group

| Group | Commands | Count |
|---|---|---|
| App | `app_bootstrap`, `settings_get`, `settings_update`, `settings_pick_tool`, `today_get`, `do_next` | 6 |
| First run / Doctor | `blueprint_pick_repo`, `template_health`, `doctor_run`, `doctor_capture_login_path`, `doctor_login_path_apply`, `probe_folder_open`, `probe_run` | 7 |
| Projects | `projects_list`, `setup_pick_folder`, `setup_preview`, `setup_apply`, `project_open`, `project_url_set`, `condition_set`, `variable_set`, `blueprint_repin`, `project_remove`, `project_pin_set` | 11 |
| Steps | `step_view`, `run_input_set`, `optional_part_set`, `run_override_set`, `step_mark_done`, `step_skip`, `action_preview`, `action_apply`, `wait_start`, `clipboard_copy` | 10 |
| Chat | `chat_copy_and_open`, `chat_link_set`, `paste_back_preview`, `paste_back_apply` | 4 |
| Launch | `launch_prepare`, `chain_resume_prepare`, `launch_start`, `config_ack`, `auth_key_allowlist_add`, `project_env_approve` | 6 |
| Terminal | `pty_attach`, `pty_write`, `pty_paste`, `pty_resize`, `pty_flow`, `pty_interrupt`, `pty_terminate`, `rearm_start`, `rearm_continue`, `terminal_link_open` | 10 |
| Files | `file_open`, `file_reveal`, `terminal_open_here` | 3 |
| Decisions / recovery | `pending_changes_list`, `pending_change_accept`, `pending_change_reject`, `item_kind_confirm`, `progress_rebuild_preview`, `progress_rebuild_apply` | 6 |
| Packets / run log | `packet_view`, `packet_normalize_apply`, `packet_signature_confirm`, `packet_repair_prepare`, `metrics_export`, `metrics_view`, `runs_clear`, `runs_search` | 8 |
| Gates (P2) | `gate_view`, `gate_item_attest`, `gate_import_results`, `gate_import_confirm`, `gate_fix_start` | 5 |
| Signing (P2) | `sign_preview`, `sign_apply`, `sign_retry_commit`, `tamper_restore` | 4 |
| Lessons, manifest, assembler (P2) | `lessons_status`, `manifest_view`, `manifest_mark`, `assembler_view`, `assembler_paste_preview`, `assembler_paste_apply` | 6 |
| Desktop, overlay, run sheet (P2) | `desktop_open`, `desktop_handoff`, `add_step`, `add_item`, `run_sheet_export` | 5 |
| **Total** | P1 (incl. P1 + P2 commands): 60 · P2-only: 31 | **91** |

This file is the only command list; the design doc points here (Stage 4.7 fix pass). The Stage 4.7 review added `chain_resume_prepare`, `packet_repair_prepare`, `metrics_view` and `project_pin_set`, the `mode` field of `launch_prepare` and the `source` field of `paste_back_preview`, each needed by an acceptance criterion.

## 5. Channels

| Channel type | Wire | Created by | Payload | Rules |
|---|---|---|---|---|
| **PTY output** `Channel<tauri::ipc::Response>` | `InvokeResponseBody::Raw` → `ArrayBuffer` | `launch_start`, `rearm_start`, `rearm_continue`, `pty_attach`, `probe_folder_open` | Raw PTY bytes, chunks ≤ 64 KB (`Response::new(Vec<u8>)`) | Never JSON (a `Channel<Vec<u8>>` would send a number array). The webview writes with `term.write(new Uint8Array(buf), cb)` and calls `pty_flow(paused=true)` above 1 MB pending (NFR-08). Ordered per channel. One channel per run; a re-arm's relaunch gets a new run and a new channel; the old buffer is archived client-side. Tests: `pty::channel_sends_raw_bytes`, `pty::burst_1mb_backpressure`. |
| **Launch progress** `Channel<LaunchProgress>` | JSON | `launch_prepare`, `chain_resume_prepare`, `packet_repair_prepare`, `rearm_start`, `rearm_continue`, `gate_fix_start`, `desktop_open`, `probe_run` | `#[serde(tag = "phase")]` `Version { found: Option<String> }` · `Contract { outcome: ContractOutcome }` · `Fingerprint { firstScan: bool }` · `Auth { cached: bool }` · `Transcript` · `Inputs` · `Ending { step: "ctrlC" \| "exit" \| "waiting" }` · `Exited { code: Option<i32> }` · `Spawning` · `Done` | Phases arrive in DR-120's order; a phase that is skipped (cache hit) is still sent with `cached: true`. Contains no prompt text, env values or config values. Test: `launch::progress_phase_order`. |
| **Sign progress** `Channel<SignProgress>` (P2) | JSON | `sign_apply`, `sign_retry_commit` | `ReRead` · `Write { path }` · `HashObject { path }` · `WriteTree` · `CommitTree { signed: bool }` · `UpdateRef` · `RefreshIndex` · `ReadBack { path, ok }` · `Done { commit }` | One message per plumbing step (ADR-012). Test: `sign::progress_matches_plumbing_order`. |

Channels need no capability permission (SR-22). The TS client wraps each in `#lib/ipc/tauri.ts`; components receive callbacks, never `Channel` objects.

## 6. Events (Rust → webview, invalidation only)

Events carry identifiers and a revision, never content (design doc §3.3). A newer `rev` makes the store refetch through a command. The capability grants only `core:event:allow-listen`/`allow-unlisten`; there are **no** webview → Rust events.

| Event | Payload (Rust → TS) | Emitted when | Source | F / AC | SR |
|---|---|---|---|---|---|
| `project:changed` | `{ projectId: string; rev: number }` | Any project state change: step state, chain, condition, overlay, wait elapsed, stage signed | commands, `app::watch` (decision files, `out` paths), timers | F-017, F-030 · AC-F022-04, AC-F007-09 | SR-18 |
| `projects:changed` | `{ rev: number }` | Registry change (setup, remove) | commands | F-018 | — |
| `packet:changed` | `{ projectId: string; stage: number; rev: number }` | A packet file (exactly `docs/handoff/stage-\d{2}\.md` or `stage-\d{2}-attachments\.md`; lookalike names are ignored, SR-31) changed on disk and was re-read and re-validated | `app::watch`, re-stat on focus/open/launch | F-013 · AC-F013-06, -07, -10 | SR-31, SR-18 |
| `decision:pending` | `{ projectId: string; count: number }` | An outside change to a decision file was classified as pending | `app::watch`, open, launch, sign | F-022 · AC-F022-04 | SR-40 |
| `tamper:detected` (P2) | `{ projectId: string; stage: number; rev: number }` | A signed packet differs from its committed blob | `app::watch` | F-014 · AC-F014-15 | SR-46 |
| `run:status` | `{ runId: string; status: RunStatus; rev: number }` | Run state transitions (spawned, exited, errored, interrupted; P2 sensor idle/blocked) | `sys::pty` exit, `app::sensor_ingest` (P2), exit handler | F-009, F-011 · AC-F009-02, -03, AC-F011-05 | SR-18 |
| `config:changed` | `{ projectId: string; cwd: CwdKind }` | A fingerprinted file changed while a run in that cwd is live (warning only) | `app::watch` | F-008 · ADR-021 §7 | SR-06, SR-18 |
| `doctor:status` | `{ level: DoctorLevel }` | Doctor finished | `doctor_run`, startup | F-019 · AC-F019-07 | — |
| `settings:changed` | `{ rev: number }` | `settings.json` changed on disk (reloaded) or through a command | `app::watch` (app data), commands | F-029 · AC-F029-03 | SR-33 |

Rules: `events::payload_snapshot` (insta, every event), `events::no_content_fields` (every payload type has only IDs, numbers, enums and booleans), `watcher::events_never_act`.

Mapping in `app::watch` (M3-21, M3-22): the project watcher starts at `project_open`; a packet or attachments change → `packet:changed` (its `rev` is an app-wide packet counter, not the project revision); a decision-file change → re-classified as on open → `decision:pending` (new pending only) and `project:changed` (when the pending list or a variables-only reload changed state); `docs/lessons.md` and `out` paths emit nothing in P1 (their readers are P2); a refused watch root stops the watcher and emits `doctor:status` (`warning`). `config:changed` comes from a per-run hint watcher over the run's fingerprint sources, with a 60 s re-check for what hints can't see (5 s for a run whose hint watcher couldn't start). Window focus re-stats every watched project and re-checks live runs.

### 6.2 Client-side rules

- `TauriIpcClient` is the only importer of `@tauri-apps/api` (ESLint `no-restricted-imports`, ADR-004).
- `pty_write` data over 64 KB (for example from `term.paste` of a 1 MB clipboard) is split by the client into ≤ 64 KB writes at UTF-8 code-point boundaries, in order, awaiting each. Test: `terminal::large_paste_chunked` (Vitest).
- Stores keep `{ value, rev }`; an event with `rev ≤ stored rev` is ignored.

## 7. Contract tests and the mock

### 7.1 Keeping TypeScript and Rust in sync (ADR-004)

| Check | What it proves | Where |
|---|---|---|
| `bindings-drift` | `cargo test -p foreman-app --features ts export_bindings` (ts-rs runs one `export_bindings_<type>` test per exported type, writing to `TS_RS_EXPORT_DIR=src/lib/ipc/bindings`), then `git diff --exit-code src/lib/ipc/bindings`. Any Rust DTO change without regenerated bindings fails. | CI |
| `ipc::commands_json` | Rust writes `ipc::COMMANDS` to `src/lib/ipc/bindings/commands.json` (sorted). | cargo test |
| `ipc/contract.test.ts` | `Object.keys(COMMAND_MAP)` of `TauriIpcClient` equals `commands.json` exactly (all 91; drift either way fails); every `Requests[cmd]` is the generated `<Name>Req` binding; `MockIpcClient` is declared `implements IpcClient` (tsc enforces every method and signature); `commands.json` equals the command list in this document (the test parses §4.15's table from `docs/contracts/ipc.md`). | Vitest |
| `capabilities::every_command_declared_and_granted` | `COMMANDS` = `generate_handler!` set = `allow-*` grants in `capabilities/main-window.json`. | cargo test |
| `ipc::fixture_requests_deserialize` | For every command, `src/lib/ipc/fixtures/requests/<command>.json` (type-checked in TS with `satisfies <Req>`) deserializes in Rust into the request struct. Catches serde/ts-rs divergences (renames, tags, `Option` handling). | cargo test + tsc |
| `ipc::fixture_responses_roundtrip` | The mock's fixture responses (`src/lib/ipc/fixtures/responses/<command>.json`) deserialize in Rust into the response type and re-serialize byte-identically. | cargo test |
| `ipc::rejects_unknown_fields` | One case per command: an extra field → `Invalid`. | cargo test |
| `ipc::no_bigint_in_bindings` | IPC-R4. | Vitest |
| `ipc::error_codes_closed` | The TS `IpcErrorCode` union equals the Rust enum; the UI has a message for each. | Vitest |

### 7.2 Mock IPC rules

`MockIpcClient` (`src/lib/ipc/mock.ts`) serves `pnpm dev:mock` (browser preview), Vitest component tests and Playwright flows (ADR-004, ADR-015). Rules:

1. **Every command is implemented** with fixture data; no method throws "not implemented". `ipc/mock.coverage.test.ts` calls every command in `commands.json` with its request fixture and asserts a typed result or a typed `IpcError`.
2. **Fixtures** live in `src/lib/ipc/fixtures/`: the TradeLog project (all conditions `yes`, Stages 1–4 signed), Foreman's own project (conditions per Master Spec §5), an empty registry, a read-only project (each `ReadOnlyReason`), and the Foreman `docs/handoff/stage-0[1-3].md` packets as paste and packet-view fixtures. Response fixtures are the same JSON files Rust round-trips (§7.1), so the mock can't drift from Rust's shapes.
3. **State is in memory and deterministic**: a fixed clock (`2026-10-19T09:00:00Z`), UUIDs from a seeded counter, revisions increment exactly as Rust's do. Mutating commands change the in-memory state and emit the same events (`project:changed`, …) with the next `rev`.
4. **Scenarios** are selected by `VITE_MOCK_SCENARIO` or `mock.setScenario()`: `happy`, `firstRun`, `blockedVersion`, `blockedAuth`, `configChanged`, `conflict`, `readOnly`, `brokenChain`, `outsideChange`, `tamper` (P2). Each error scenario returns the same `IpcError` code and detail Rust would.
5. **PTY**: the mock delivers a scripted byte stream (the fake-claude transcript fixture) as `ArrayBuffer` chunks through the same callback interface as the real Channel, honours `pty_flow`, echoes `pty_write` input, and responds to `\x03` and `/exit\r` like fake-claude.
6. **Native dialogs are scripted**: every command that raises a native confirmation in Rust (IPC-R11) takes its answer from the scenario (`mock.setDialogAnswer('cancel' | 'ok')`), so component tests cover both outcomes without any webview "approve" path.
7. **Security behaviors are mirrored**, not skipped: unknown fields → `Invalid`; `pty_write` to a non-live run → `NotAllowed`; one-time preview and plan IDs expire; clipboard reads return the fixture clipboard (≤ 1 MB, text only).
8. The mock is a product artifact: a command change without a mock change fails `tsc`; a fixture change without the Rust round-trip passing fails CI.
9. The mock is never bundled in the release build: `#lib/ipc/index.ts` picks it only when `import.meta.env.VITE_IPC === 'mock'`, and a build check greps the release bundle for the mock module's marker string (`ipc::mock_not_in_release`).
