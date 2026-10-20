# Progress

Ticket status for the build (`docs/tickets.md`). `pnpm verify` runs `scripts/check-required-tests.mjs`,
which lists every ticket's "Required tests" missing from `cargo test --workspace -- --list`; it
fails only for a milestone marked `done` below. Statuses: `todo`, `in-progress`, `done`,
`review-pending`.

## Milestones

| Milestone | Status | Notes |
| --------- | ------ | ----- |
| M1 | done | Domain core (Stage 6). M1-33's exit review (`code-reviewer`, Gate A–C, the Stage 6 packet) is Billy's gate run. |

## M1 tickets

| Ticket | Status | Title |
| ------ | ------ | ----- |
| M1-01 | done | Annotate blueprint v2.2: Stages 1–2, Stage Gate, F.2, Council setup |
| M1-02 | done | Core types, canonical JSON and schema versioning |
| M1-03 | done | Path policy: lexical write and read checks |
| M1-04 | done | Store: no-symlink walk and read policy |
| M1-05 | done | Atomic durable writes that never clobber a newer file |
| M1-06 | done | Reads outside projects and secret-file exclusion |
| M1-07 | done | Spawn module core |
| M1-08 | done | `git::run`: the single git entry point |
| M1-09 | done | `info/exclude` writer |
| M1-10 | done | Plumbing commit with read-back |
| M1-11 | done | Template parser: structure, items and bounds |
| M1-12 | done | `fm:` grammar: directives and values |
| M1-13 | done | Step binding, prompts, gate and F.2 instances, packet formats |
| M1-14 | done | Grammar validation and the diagnostic catalog |
| M1-15 | done | Prose lint and chat chips |
| M1-16 | done | Golden v2.2, round-trip property and template cache |
| M1-17 | done | Blueprint lock: pin, verify, restore, item drift |
| M1-18 | done | Rule engine: tags, tri-state conditions, applicable path |
| M1-19 | done | Run configs and model policy |
| M1-20 | done | Claude Code version floor |
| M1-21 | done | Token classification and the blueprint inventory |
| M1-22 | done | Prompt resolution |
| M1-23 | done | Sanitize: prompt hygiene, paste stripping, visible escapes |
| M1-24 | done | Step kinds and states |
| M1-25 | done | Action steps with fixed destinations |
| M1-26 | done | Packet validation |
| M1-27 | done | Decision files and approved copies |
| M1-28 | done | Foreman's own decision commits |
| M1-29 | done | App-data file contracts |
| M1-30 | done | TradeLog harness: Stages 1–2 and F.2 |
| M1-31 | done | Abuse-case suite |
| M1-32 | done | M1 benchmarks |
| M1-33 | review-pending | M1 exit review |

## M4 tickets (stage/05-annotate)

| Ticket | Status | Notes |
| ------ | ------ | ----- |
| M4-04 | done | Annotate blueprint v2.2: Stages 0 and 3–5; golden through Stage 5. The M1-01 generator (`scripts/blueprint/annotate-v2.2.py`) already wrote DR-038's annotations for Stages 0 and 3–5, so the fixture is unchanged; what M4-04 adds is the proof: snapshots `golden_v2_2_stage_0`, `_3`, `_4`, `_5` and `golden_v2_2_stages_0_5_lint` beside the Stages 1–2 and Gate ones, `template::golden_v2_2_stages_0_to_5` (zero errors, 40 runnable steps, 6–18 still needs-annotation) and `template::pin_clean_through_stage_5` (pin check clean; an error in Stage 0, 3, 4 or 5 is scoped and doesn't block the pin). Two lint warnings remain (4.7, 5.5): [CR-004](change-requests/CR-004-stage-4-5-lint-warnings.md), proposed. Template-health data for M4-06: the core already carries every field (steps' annotations, outs, lint diagnostics, per-stage status). The `TemplateHealth` DTO (`src-tauri`) has the per-stage rows and diagnostics but not the per-runnable-step annotations and outputs (AC-F003-05); adding them is M4-06's work, outside this core-only change. |

## Known deviations (M1, reviewed 2026-10-20)

Where the M1 build differs from the docs, the doc that should change (or the decision Billy owes), and what was done. "Doc changed" means this branch edited it; the rest wait for Billy.

| # | Deviation | Where | Doc that should change / decision | Status |
| - | --------- | ----- | --------------------------------- | ------ |
| 1 | The hostile-repo fixture and the 20,000-line blueprint are generated in code each run, not committed (`fixtures/hostile-repo/`, `fixtures/blueprint-20k.md`). Generating them keeps the repo small and the content deterministic; the 20k file is the v2.2 blueprint plus synthetic appendix notes, so it is not a real blueprint. | `crates/foreman-sys/tests/common/mod.rs` (`hostile_repo`), `crates/foreman-core/benches/template.rs` (`blueprint_20k`) | `docs/tickets.md` M1-08 and M1-32 "Likely files". | Doc changed. No CR (no behavior). |
| 2 | The file-system-reported name is read by inode from a `getdents` listing, not `F_GETPATH`, and Linux fakes it through `Root::with_name_reporter`. The real case-insensitive volume test (`fs_policy::case_and_normalization_variants`, an APFS disk image) runs on macOS only; Linux proves only the hook (`reported_name_checked`). The case-sensitive volume half of ADR-011's verification isn't built. | `crates/foreman-core/src/store/walk.rs`, `tests/fs_policy.rs` | ADR-011 (FS-reported name check, Verification). | ADR-011 changed. **Mac task:** run `cargo test -p foreman-core --test fs_policy` on macOS and record that the APFS test passed (Stage 5 packet). |
| 3 | Pinning didn't refuse annotation errors. `lock::pin` refused only a file that differed from HEAD or a re-pin after Stage 1 is signed, then pinned whatever parsed. No document required refusal (AC-F001-15 says errors in an unused stage are listed and the project opens), but the Stage 1 exit criterion expects the P1 stages to parse with zero errors, and a pinned P1 step in error can't be repaired once Stage 1 is signed. | `crates/foreman-core/src/lock.rs` (`pin`) | [CR-003](change-requests/CR-003-pin-refuses-p1-annotation-errors.md) (Approved, option B): DR-155, AC-F002-02, `docs/contracts/ipc.md` (§3, `setup_preview`, `blueprint_repin`) and M1-17 changed, each edit marked "(CR-003)". | **Resolved by CR-003**, approved by Billy on 2026-10-20 (in chat, 12:38 ET). `lock::pin` refuses while Stages 1–2 or F.2 have errors and returns the diagnostics (`LockError::AnnotationErrors`); errors in other stages still pin. Tests `lock::pin_refuses_p1_annotation_errors`, `lock::pin_allows_errors_outside_p1`, `lock::pin_clean_p1`. **Follow-up (M2): done** in `stage/05-app` (`IpcError{notAllowed, detail: annotationErrors}`, binding, mock, Setup list). |
| 4 | Foreman's own decision change won't overwrite an unreviewed outside edit: `own_change` returns `OutsidePending` when the repo file is neither the approved nor the last committed content. SR-33 and ADR-022 §4 imply it; no rule said it for Foreman's own writes. The IPC error for it isn't in `docs/contracts/ipc.md`. | `crates/foreman-core/src/decisions.rs` (`own_change`) | DR-153 and ADR-022 §5 (the refusal). `docs/contracts/ipc.md` needs an error case on the commands that change decision files; that file belongs to the src-tauri/IPC work, so it is not edited here. | DR-153 and ADR-022 changed. **Follow-up: done** (ipc.md §2.4: `Conflict` with the file's path). |
| 5 | From Stage 5 on, Foreman's own decision changes go to the approved copy and `progress.json` only; the repo file is left alone until the next signing commit. DR-153 read as if the repo file were written too, which contradicted its own drift rule (the repo file must still match `committedSha256`) and ADR-022 §5. | `decisions.rs` (`own_change`, `stage5_signed`) | **DR-153 now matches ADR-022 §5.** ADR-022's rule is the safer one: the working-tree file stays what Claude Code (and `git status`) sees as committed, nothing uncommitted can be swept into a commit that bypasses Foreman's signing, and drift needs no special case. | DR-153 changed. Billy confirms with the Stage 5 packet. |
| 6 | A CLI prompt step is done only on exit 0; a non-zero exit ends the run (code recorded) and returns the step to `notStarted`. DR-066 says a CLI step finishes "when the run exits (any code; the exit code is recorded)", and DR-065 says `running → done` on "exit 0" and `running → notStarted` when errored. The previous code made every exit code done. | `crates/foreman-core/src/steps/mod.rs` | Reading: DR-066's "any code" means the run is over and the code is recorded; DR-065 decides the step state. DR-066 should say "the run ends on any exit code (recorded); exit 0 finishes the step (DR-065)". | Code and tests fixed (`steps::finish_rules`, `steps::transitions_exhaustive` snapshot). DR-066 wording aligned with DR-065 (no behavior change); Billy confirms with the Stage 5 packet. |
| 7 | The real-index refresh runs with attributes neutralized and is skipped when `info/attributes` is non-empty. | `crates/foreman-sys/src/git/` | [CR-002](change-requests/CR-002-git-attributes-on-index-refresh.md) (Approved). | **CR-002 approved** by Billy on 2026-10-20 (in chat, 12:38 ET). Doc changed: SR-44, SR-45, AC-XC-04, ADR-012, T-040, DR-140, DR-143, design doc, files.md §4.3 (each edit marked "(CR-002)"). The git 2.39 residual stays open (UNVERIFIED on his Mac). |

Item 3: see [CR-003](change-requests/CR-003-pin-refuses-p1-annotation-errors.md).

## Performance measurements still owed

| Target | Measured here | Task |
| ------ | ------------- | ---- |
| NFR-03 / AC-F001-07: a cached load of the 20,000-line blueprint ≤ 5 ms (uncached p95 ≤ 50 ms) | **7.4 ms** cached in the M1 run; a re-run here on 2026-10-20 (`--warm-up-time 1 --measurement-time 3`) gave 6.4 ms (interval 6.19 to 6.70 ms) cached and 19.8 ms uncached for the 20k file. Both on a shared 2-core Xeon @ 2.8 GHz Linux container, not a Mac; criterion `template_load_20k_cached` | **Mac task (NFR-03):** run `cargo bench -p foreman-core --bench template` on Billy's Mac with nothing else running, and record the mean and the criterion interval for `template_load_v2_2_uncached`, `template_load_20k_uncached` and `template_load_20k_cached` in the Stage 5 packet. The target stays 5 ms. If the Mac misses it, profile `cache::load_or_parse` before changing the target or the cache format. Its cached path normalizes and SHA-256 hashes the whole 20k-line source, reads the cache file through the store's policy walk, and deserializes the template JSON; those three steps are the suspects. |

The bench itself (`crates/foreman-core/benches/template.rs`) carries the same note, so a reader running it sees that the Linux figure is not the verdict.

## M2/M3 runner tickets (branch `stage/05-runners`: `foreman-sys`, `fake-claude`, pure runner modules in `foreman-core`)

The runner halves below are built and tested; each ticket's IPC command, native dialog, `run.json` and UI parts belong to `foreman-app` (`stage/05-app`), so the tickets stay `in-progress` until those land. APIs: design doc §3.2.3.

| Ticket | Status | Runner part done here | Left for the app |
| ------ | ------ | --------------------- | ---------------- |
| M2-06 | in-progress | argv builder, fixed flags, prompt last, `--name`; `argv::*`, `spawn::no_shell_in_any_code_path`, abuse argv | `clipboard_copy { target: "command" }` done (`stage/05-chat`); preview UI left |
| M2-07 | in-progress | child env + PATH sanitizing; `env::*` | pass-through setting |
| M2-08 | in-progress | PTY/non-PTY lifecycle, no inherited fds, SIGHUP quit path | app: run table + startup scan done, `lifecycle::crash_leaves_interrupted` passes (`src-tauri/tests/launch.rs`); quit guard left (M3-13) |
| M2-09 | in-progress | run settings file (fresh, `0600`); `run_settings::*` | — |
| M2-11 | in-progress | `claude -v`, `auth status` readers (fail closed) | Doctor |
| M3-01 | in-progress | fingerprint over every source, stat cache, symlink rules, streamed `~/.claude.json`; `preflight::*_detected`, `fingerprint::*` | — |
| M3-02 | in-progress | redacted diff and `acknowledge` | app: `config_ack` (native dialog, stale fingerprint refused) done; banner UI left |
| M3-03 | in-progress | SR-07 blocks incl. `forceLoginMethod`; allowlist | app: `auth_key_allowlist_add` done; `project_env_approve` answers `Unsupported` until S-7′ is recorded |
| M3-04 | in-progress | auth in the step cwd, after the fingerprint, cached | — |
| M3-05 | in-progress | DR-120 order, version floor; `launch::progress_phase_order`, `compat::blocks_below_floor` | plan IDs, bench |
| M3-09 | in-progress | `PtySession` (env exact, no fds, back-pressure, live-only writes) | Channel adapter |
| M3-10 | in-progress | `start_session` | app: `launch_start`, `run.json` before spawn, index lines, `runlog::record_before_spawn`/`argv_prompt_redacted`/`no_env_values` done; tabs UI left |
| M3-12 | in-progress | SR-28 paste sanitizer; `pty::paste_strips_escape_sequences` | app: `pty_paste` done; `terminal_link_open` done (`stage/05-chat`: `foreman_core::urls::terminal_url`, native dialog, `foreman-sys` `open -u`, CR-006; `opener::terminal_link_scheme_policy`, `terminal::link_handler_routes_to_rust`); left: the `@xterm/addon-web-links` handler (the addon isn't installed; only OSC 8 links route today) and the S-0 real-TUI paste check |
| M3-13 | in-progress | terminate (SIGTERM → SIGKILL), hangup | app: `pty_terminate` native dialog done; quit guard left |
| M3-14 | in-progress | chain planning, S-3/S-8 branches, transcript path; `chain::*` | — |
| M3-15 | in-progress | re-arm machine + driver; `rearm::*` | app: `rearm_start`/`rearm_continue` with the native idle dialog done (`rearm::relaunches_after_idle_confirm`) |
| M3-19 | in-progress | watcher (interest set, lstat roots, re-read by stamp); `watcher::*` | app: `watcher::config_changed_marks_open_runs` done (`src-tauri/src/watch.rs`, a 5 s re-fingerprint tick while runs are live; the watcher has no config interest); `ProjectWatcher` → `packet:changed`/`decision:pending` event mapping left |
| M4-09/M4-12 (sys part) | in-progress | sensor ingest, contract probes (probe B redesigned, ADR-017) | `probe_run` (still `Unsupported`, M4-12: needs the P2 sensor hooks and `sensor::hash_checked_per_run`, M4-08), Doctor rows, retention |
## M2 tickets (stage/05-app: IPC, the Tauri app)

Statuses for the app side of each ticket. A command not built yet answers `Unsupported`
(`notYetAvailable:<ticket>`, ipc.md IPC-R12) after its request validates.

| Ticket | Status | Title |
| ------ | ------ | ----- |
| M2-01 | review-pending | IPC command framework: 91 commands registered, granted and validated; §2.4 types; ts-rs bindings; drift tests |
| M2-12 | in-progress | Settings backend and tool paths (`settings_get`, `settings_update`, `settings_pick_tool`; screen-reader mode in memory until settings v2) |
| M2-13 | in-progress | File opening and reveal (`file_open`, `file_reveal` through `foreman-sys::spawn`) |
| M2-15 | in-progress | Doctor P1 (`doctor_run`: claude path and version, git; auth row waits for M3-01/M3-04) |
| M2-16 | in-progress | First run (`blueprint_pick_repo`) |
| M2-17 | in-progress | Project setup backend (`setup_*`; `docs/templates/` content from the pinned kit not written yet) |
| M2-19 | in-progress | Project open, registry and read-only states (`project_open`, `projects_list`) |
| M2-20 | in-progress | Today (`today_get`, `do_next`; CLI steps wait for `launch_prepare`) |
| M2-21 | in-progress | Stage workspace backend (`step_view`, `step_skip`, `action_preview`, `action_apply`) |
| M2-22 | in-progress | Run inputs and prompt block (`run_input_set`, `optional_part_set`) |
| M2-24 | in-progress | Conditions, variables, outside-change review (`condition_set`, `variable_set`, `pending_*`) |
| M2-25 | in-progress | Launch preview (`launch_prepare`, `chain_resume_prepare`, `do_next` → `prepare_launch`; plans held 60 s, one use) |
| M2-26 | in-progress | `step_mark_done` |

### Commands not built yet (IPC-R12), by ticket

| Ticket | Commands |
| ------ | -------- |
| S-7′ | `project_env_approve` |
| M3-16 | `progress_rebuild_*` |
| M3-18, M4-12…M4-14 | `probe_folder_open`, `probe_run`, `doctor_capture_login_path`, `doctor_login_path_apply` |
| M3-21 | `packet_normalize_apply` (the DR-131 normalizer) |
| M4-16…M4-33 | gates, signing, `tamper_restore`, lessons, manifest, assembler, desktop, `add_step`, `add_item`, `run_sheet_export`, `item_kind_confirm`, `terminal_open_here` |

## M3 chat tickets (branch `stage/05-chat`)

| Ticket | Status | Title |
| ------ | ------ | ----- |
| M3-17 | done | Followup and manual steps, switches and overrides: followup copy returns the chain's live tab (`focusRun`), the UI copies-and-focuses a live chain and relaunches otherwise; Open file starts a `manual` step and a later change to its file finishes it (re-stat on open); a per-run override is consumed by the run that used it (DR-103, it wasn't before). Tests: core `steps::followup_copy_or_relaunch`, `steps::finish_rules`, `policy::switches_never_typed`, `policy::override_scope`, plus `src-tauri/tests/followup.rs` (same names, against fake-claude). |
| M3-23 | done | Chat chips, links and Copy and open: `chat_copy_and_open` (sanitized prompt, SR-25 recheck of the stored link, `chatCopy` run record with time to start, prompt hash only), `chat_link_set` (claude.ai only, stored on the chain start), chips from the RUN WITH prose (`ChatChips.svelte`), the step says which link to set (inline Project link form). Tests: `chain::chat_links`, `chain::stored_link_rechecked`, `opener::only_allowed_urls` (core + app), `src/lib/components/chat.svelte.test.ts`. |
| M3-24 | in-progress | Paste-back preview and apply: `foreman_core::pasteback` plans every target from declared paths only (packet, attachments, extra outs, outputs; wrong stage, two packets, P1 appends refused), apply re-checks every stamp before the first write. Tests: `pasteback::lists_targets_before_write`, `atomic::refuses_newer_file` (paste-back conflict), invalid packet → fix request, clipboard errors, policy refusal, AC-XC-16 `pasteback::no_chat_content_stored`. **Left:** field-level normalization (DR-131: unsigned forms, `none`, smart quotes, NBSP, header variants) is M3-20's `packet::normalize_packet`; paste-back calls the structural `pasteback::prepare` (BOM, CRLF, fence) through a normalizer seam until M3-20 lands, then the call in `chat::paste_back_preview` swaps to it. AC-F012-13's "explicit Open action" for pasted links has no command in ipc.md (links show as text). |
| M3-25 | done | Close-out checklist step (P1): `foreman_core::steps::closeout`, `StageView.closeoutChecklist` (ipc.md §2.4), `CloseOutChecklist.svelte` once the packet is valid and the stage unsigned. Test: `steps::closeout_sequences`, component tests. |

## Run metrics, retention and the run log (branch `stage/05-metrics`: M3-26, M4-02, M4-03, M3-28)

| Ticket | Status | Built | Left |
| ------ | ------ | ----- | ---- |
| M3-26 | in-progress | `metrics_export` (native save dialog; `.md` only; `WritePurpose::UserExport`), `metrics_view`; the engine is `foreman_core::runlog` (`parse_index`, `merge_latest`, `analyze`, `Baseline`, `export_metrics`); tests `metrics::runlog::*`, `metrics_view::*`, `export::*` (core and app); `bench_metrics_100k` 169 ms for 100k lines against the 300 ms NFR-02 budget (shared 2-core Linux container, not a Mac) | Chat clock call sites: `runs::record_chat_copy` (Copy and open) and `runs::note_step_done` (mark done, paste-back) are built and tested, but `chat.rs` and the paste-back/mark-done handlers belong to other branches and must call them at merge. Real-Mac performance run. |
| M4-02 | in-progress | `runs_clear`, full `project_remove` (`projects/<id>/`, approvals, fingerprints; the repo is untouched), 90-day tail pruning on the startup scan, `remove_file`/`remove_tree`/`tree_size` through symlink-safe `unlinkat` in `store::app_dirs`, a native confirmation for each, `settings.runHistory` size; tests `privacy::*`, `retention::*`, `app_data_delete::*` | Template/event pruning (events belong to M4-09). Project-removal UI (no screen calls it). APFS behaviors (Mac). |
| M4-03 | in-progress | `runs_search` (newest 3000 records scanned when a query is given; paged), the strip on `RunLog` (time to start, p95, active time, edits, baseline comparison); tests `search::*`, `RunLog.svelte.test.ts`, a11y flow | Cross-check of the mockup in both themes on a Mac. |
| M3-28 | in-progress | `tests/flows/p1-loop.spec.ts`: a keyboard-only mock flow for create, chat steps, paste back, config acknowledge, CLI run, re-arm of a continuing step, terminal focus out and back (3 tests, chromium). Fixes: `selectStep` also walks up with K; the mock gains `setRunIdle` (the P2 sensor's idle status) | The real-app keyboard script (`e2e/specs`, macOS only; the ticket's `tests/e2e/keyboard-p1.e2e.ts` path does not exist: the specs live in `e2e/specs`). WebKit flow project not installed in the sandbox. |

Definitions and assumptions (stated in the export too):
- The baseline is `docs/metrics-baseline.md` of the first registered project that has a filled log; the chat baseline needs at least 10 steps (kill check, per surface). Desktop runs are not in the kill check.
- Time to start leaves out `rearm`/`repair` entries. Active time per step is the sum of `timeToStartMs + durationMs - waitMs` per (surface, project, step), counted only if some run finished and none was interrupted. Median is the midpoint of an even pair; p95 is nearest-rank.
- Edits are run-input interventions (a run input set again to a different value; the first set is not one) plus Copy prompt on a CLI step. Marks live in memory and reset on restart; so does the chat step-done duration.
- `BaselineComparison` is the median time to start over both surfaces against the by-hand median ([CR-008b](change-requests/CR-008b-metrics-view-per-surface-baseline.md)). The latest index line per `runId` wins, and a chat step has a second line ([CR-008a](change-requests/CR-008a-run-index-latest-line-wins.md)).
- The export path policy is stricter than SR-30 asks: `.md` only, never inside `.git`, `.claude` or `.foreman` folders anywhere, never `CLAUDE.md`, never the app's data folder.
- A month index file is read up to 8 MB (`ReadClass::AppState`).
- `ipc.md` writes `MetricsView` without `p95_all_ms` in the §1052 Rust block but has `p95AllMs` in the TS block; the code has the field.
- The old `metrics_*`/`runs_*` stubs in `src-tauri/src/ipc/commands/packets.rs` are now unused; delete them at merge (that file belongs to the packets branch).

### Open follow-ups

- `settings.json` v2 with `screenReaderMode` (core schema + migration; files.md §4.1 proposal).
- `project_remove` is complete (M4-02, `stage/05-metrics`). No screen calls `projectRemove` yet; the removal UI is left (M4-02 asks for the native confirmation, which Rust raises).
- `foreman-core::store::dirs::ensure_dir` (new, for setup's empty folders) needs a core review.
- (CR-003) `foreman-core::pin_check::p1_annotation_errors` (new) lets `setup_preview` refuse before anything is written; it mirrors `lock`'s private `problem`/`step_at` (pinned by `pin_check::preview_check_matches_pin`). The core owner may fold it into `lock.rs`. Pin refusals map to `IpcError{notAllowed, detail: annotationErrors}`; Setup and the wizard list each error (`AnnotationProblems.svelte`). No re-pin screen exists yet; it should reuse that component.
- `PtySession::wait_exit` now returns only after the sink was told of the exit (`pty::write_requires_live_run` raced the sink: 3 of 60 runs under load failed; 0 of 60 after).
- The launch's own environment takes `HOME` from the app's home folder (`App::own_env`), so the transcript check and the child agree on `~/.claude` (tests no longer write to the real `~/.claude/projects`).
- `sensor::hash_checked_per_run` (M4-08) waits for the P2 sensor hooks; the app passes `sensor: None`.
- (stage/05-chat) URLs open through `foreman-sys` `/usr/bin/open -u` (CR-006, proposed); **Mac task:** confirm `open -u https://claude.ai/` opens the browser.
- (stage/05-chat) `pasteback::prepare` is the stand-in normalizer until M3-20; `foreman_core::pasteback::plan` takes the normalizer as a parameter.
- Item 4 above: the `OutsidePending` refusal maps to `Conflict` with the file's path (ipc.md §2.4).

## M4 gate, lessons and signing tickets (branch `stage/05-signing`)

| Ticket | Status | Notes |
| ------ | ------ | ----- |
| M4-15 | done | `foreman-core::gate` (state machine, attest rules, order, signable). Tests `gate::signable_requires_all` (property), `steps::attest_note_rules`, `steps::gate_item_order`, plus `gate::state_machine_rejects_out_of_order`, `gate::never_signable_with_unresolved_items`. |
| M4-16 | done | `gate_view`, `gate_import_results` (proposals only), `gate_import_confirm`, `gate_fix_start` (a detached new chain, `launch::prepare_custom`; F.4 on the third FAIL). `sensor::events_only_update_state` (app). Import results answers `Unsupported` for runs without a sensor token, which is every run until the P2 sensor hooks land (M4-08/M4-09). |
| M4-17 | done | `gate_item_attest`, `item_kind_confirm` (native dialog, `approvals.itemKinds`). `GateItemView` gained `textSha256` and `previousText` (ipc.md §2.4, bindings, mock, fixtures). Gate screen sends the real hash and shows the pinned and new text. `template::swapped_bullets_reconfirm` and `ipc::security_confirmations_are_native` (app, `src-tauri/tests/gates.rs`). |
| M4-18 | in-progress | `foreman-core::lessons` (entries, the stage requirement, next ID, the header's entry format, the CAS append with renumbering) and `lessons_status` (`commands/lessons.rs`). The F.2 prompt gets the header's format appended (`steps::resolve`). Tests `lessons::append_numbering`, `steps::lessons_entry_detected`, `lessons::f2_prompt_gets_header_format`, `lessons::status_through_ipc`. **Left:** `paste_back_preview/apply { kind: "lessons" }` (chat.rs, chat agent) must call `lessons::read` for the diff and `lessons::append(root, seen_stamp, text)` to write (AC-F016-02…-04 at IPC level). |
| M4-19 | done | `sign_preview` (`src-tauri/src/signing.rs`): gating from `closeout` + `gate::sign_blockers` (git missing names git, AC-F014-03; Stage 2 conditions), the added fields as a diff (`packet::sign_fields`), the 6-hex content hash, paths with notes, Stage 2 `confirmConditions`. Tests `packet::added_fields_golden`, `rules::skipped_queue`, `gate::signable_requires_all`, `sign::preview_shows_fields`, `sign::git_missing_disables_sign`. |
| M4-20 | done | `sign_apply`: re-read (Stale with a fresh preview), the native sign dialog, phase 1 (APPROVED BY in place, the added fields, `.foreman/stages/stage-NN.json`, pending decision files, all by CAS), `commitPending`, then `commit_paths` with read-back and the real-index refresh; the stamp only after read-back; sensor events of the stage deleted (SR-53). `indexRefreshPending` is retried on `project_open`. Tests in `src-tauri/tests/sign.rs`: `sign::refuses_changed_file`, `sign::pathspec_only`, `sign::readback_detects_mismatch`, `sign::untracked_new_packet`, `sign::concurrent_ref_move_fails`, `sign::index_locked_refresh_retried`, `ipc::security_confirmations_are_native`, `packet::outside_signature_needs_confirmation`, `decisions::pending_decisions_ride_in_signing_commit`. **Left:** the index-refresh retry on window focus (no focus hook exists yet; open only); `SignDialog.svelte` stays inline in the Gate screen. |
| M4-21 | done | Other-branch signing (`Target::Other`, CR-007 **Proposed**: `rev-parse --verify -q refs/heads/<b>` to find the tip), `sign_retry_commit`, commit pending after a kill between phases, `committedOnOtherBranches` with `Decisions::process_on` (no false drift on checkout), the 60 s timeout message. Tests `sign::other_branch_leaves_worktree_head_and_index` (HR-05/HR-06 fixture), `git::repo_gpg_program_never_runs` (HR-05/HR-06 from `scripts/make-hostile-repo.mjs`, with a positive control), `git::timeout`, `decisions::other_branch_sign_no_false_drift`, `sign::killed_between_phases_retry`. **Billy:** approve or reject CR-007. "Open terminal here" on commit pending waits for `terminal_open_here` (M4-33). |
| M4-22 | done | Tamper: on open, a `Signed` packet whose working hash differs from `signing.packetSha256` is read from its commit (the resolver uses it) and kept in `Opened::tampered`; `tamper:detected` on `project_open`; a Today inbox item; `tamper_restore` writes the committed blob back by CAS. Test `sign::tamper_detected`. **Left for the packets owner:** `packet_view` should set `tamper: crate::signing::tamper_diff(&p, n)` (packets.rs is not this branch's file; `tamper_restore` already returns it). The watcher's `packet:changed` mapping (M3-19) isn't built, so detection runs on open and Today. |
| M4-23 | in-progress | `packet_repair_prepare` (in `commands/signing.rs`; the handler routes there, `packets.rs` keeps its old stub untouched): problems from Rust's validation, a re-arm of the producer's chain via `launch::prepare_custom`, `not-cli-packet`, `chain-handed-off`; Billy continues with `rearm_start`. The Packet screen states OPEN ITEMS' two outcomes (AC-F013-13). Test `rearm::only_from_user_action` (`src-tauri/tests/packet_repair.rs`). **Left:** AC-F013-14 Capture from session is `paste_back_preview { source: "sensor" }` in chat.rs (chat agent) and needs the P2 sensor; `pasteback::lists_targets_before_write` for the capture path belongs with it. The packets owner can delete the `packets::packet_repair_prepare` stub. |
| M4-24 | in-progress | `foreman-core::packet::split` (ADR-set split on `^# ADR-\d{3}:`, every target listed with its current text before a write); the chat close-out order (packet → attest → F.2 lessons → Sign) is the gate view's sequence for chat stages. Tests `pasteback::lists_targets_before_write` (ADR targets, core), `steps::closeout_sequences`. **Left:** wiring `paste_back_preview/apply { kind: "adrSet" }` to `split::split_adrs` + `split::plan` is chat.rs (chat agent). No separate `CloseOut.svelte`: the Gate screen's sequence row is the close-out. |
