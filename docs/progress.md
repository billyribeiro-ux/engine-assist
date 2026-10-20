# Progress

Ticket status for the build (`docs/tickets.md`). `pnpm verify` runs `scripts/check-required-tests.mjs`,
which lists every ticket's "Required tests" missing from `cargo test --workspace -- --list`; it
fails only for a milestone marked `done` below. Statuses: `todo`, `in-progress`, `done`,
`review-pending`.

Every M2–M4 ticket has one row in its milestone's table. "Req. tests" is what the checker finds
(present / named in the ticket) on `stage/05-integrate`, 2026-10-20. A ticket no branch recorded
is `todo`; one that a commit built but whose branch didn't record a status is `in-progress` with
the commit named, until someone checks it against its acceptance criteria.

## Milestones

| Milestone | Status | Notes |
| --------- | ------ | ----- |
| M1 | done | Domain core (Stage 6). M1-33's exit review (`code-reviewer`, Gate A–C, the Stage 6 packet) is Billy's gate run. |
| M2 | in-progress | 49/61 required tests present. |
| M3 | in-progress | 64/73 required tests present. |
| M4 | in-progress | 52/66 required tests present. |

## Waiting on Billy

Every open change request and every decision only Billy can make. CR-001 (latest framework
releases) is approved; there is no CR-005 in `docs/change-requests/`.

| # | What | Where | State |
| - | ---- | ----- | ----- |
| 1 | [CR-002](change-requests/CR-002-git-attributes-on-index-refresh.md): the real-index refresh runs with attributes neutralized and is skipped when `info/attributes` is non-empty. | `foreman-sys::git` | **Approved** 2026-10-20 (in chat, 12:38 ET) and applied (Known deviations, item 7). Only its git 2.39 residual is open, a Mac check (Mac checks, item 3). |
| 2 | [CR-003](change-requests/CR-003-pin-refuses-p1-annotation-errors.md): pinning refuses P1 annotation errors (option B). | `lock::pin` | **Approved** 2026-10-20 (in chat, 12:38 ET) and applied (Known deviations, item 3). Nothing open. |
| 3 | [CR-004](change-requests/CR-004-stage-4-5-lint-warnings.md): the two remaining blueprint lint warnings (4.7, 5.5). | M4-04 | **Proposed.** Approve or reject. |
| 4 | [CR-006](change-requests/CR-006-open-urls-through-foreman-sys.md): URLs open through `foreman-sys` `/usr/bin/open -u`. | M3-12, M3-23 | **Proposed.** Approve or reject; the Mac check is Mac checks, item 9. |
| 5 | [CR-007](change-requests/CR-007-rev-parse-branch-tip.md): other-branch signing finds the tip with `rev-parse --verify -q refs/heads/<b>`. | M4-21 | **Proposed.** Approve or reject. |
| 6 | [CR-008a](change-requests/CR-008a-run-index-latest-line-wins.md): the latest index line per `runId` wins; a chat step has a second line. | M3-26 | **Proposed.** Approve or reject. |
| 7 | [CR-008b](change-requests/CR-008b-metrics-view-per-surface-baseline.md): `BaselineComparison` is the median time to start over both surfaces against the by-hand median. | M3-26, M4-03 | **Proposed.** Approve or reject. |
| 8 | DR-153 now matches ADR-022 §5 (from Stage 5 on, Foreman's own decision changes go to the approved copy and `progress.json` only). | Known deviations, item 5 | Confirm with the Stage 5 packet. |
| 9 | DR-066 wording aligned with DR-065 (a CLI step is done only on exit 0). | Known deviations, item 6 | Confirm with the Stage 5 packet. |
| 10 | M1-33 exit review (`code-reviewer`, Gate A–C, the Stage 6 packet). | M1 | Billy's gate run (`review-pending`). |
| 11 | AC-F012-13's "explicit Open action" for links in pasted content has no command in `ipc.md`; links show as text only. | M3-24 | Decide: a command (CR) or text only. |
| 12 | The P1 close-out checklist (M3-25) still says "add the entries to docs/lessons.md yourself", and `paste_back_preview { kind: "auto" }` on an F.2 step still answers `appends-in-p1` (AC-F012-14, "in P1"). F-016 now writes lessons (`kind: "lessons"`, which the step row's Paste back sends for F.2). | M3-25, M4-18 | Decide whether the P1 wording and the `auto` answer change now that P2 lessons capture is built (core text, snapshot and `ipc.md`). |

## Mac checks

Everything that only Billy's Mac can settle, to record in the Stage 5 packet.

| # | Check | From |
| - | ----- | ---- |
| 1 | Run `cargo test -p foreman-core --test fs_policy` on macOS and record that the APFS case-insensitive volume test (`fs_policy::case_and_normalization_variants`, a disk image) passed. Linux proves only the hook (`reported_name_checked`); the case-sensitive volume half of ADR-011's verification isn't built. | Known deviations, item 2 (ADR-011) |
| 2 | **NFR-03 / AC-F001-07** (cached load of the 20,000-line blueprint ≤ 5 ms, uncached p95 ≤ 50 ms): run `cargo bench -p foreman-core --bench template` with nothing else running, and record the mean and the criterion interval for `template_load_v2_2_uncached`, `template_load_20k_uncached` and `template_load_20k_cached`. Measured here: **7.4 ms** cached in the M1 run; a re-run on 2026-10-20 (`--warm-up-time 1 --measurement-time 3`) gave 6.4 ms (interval 6.19 to 6.70 ms) cached and 19.8 ms uncached for the 20k file, both on a shared 2-core Xeon @ 2.8 GHz Linux container, not a Mac (criterion `template_load_20k_cached`). The target stays 5 ms. If the Mac misses it, profile `cache::load_or_parse` before changing the target or the cache format: its cached path normalizes and SHA-256 hashes the whole 20k-line source, reads the cache file through the store's policy walk, and deserializes the template JSON; those three steps are the suspects. The bench (`crates/foreman-core/benches/template.rs`) carries the same note, so a reader running it sees that the Linux figure is not the verdict. | Performance measurements still owed (M1-32) |
| 3 | CR-002's git 2.39 residual stays open (UNVERIFIED on his Mac). | Known deviations, item 7 |
| 4 | FSEvents latency: run `cargo test -p foreman-app --test packet_landing watcher` on macOS. The latency test uses 20 rename-over writes, not ADR-013's 100. | M3-21 |
| 5 | NFR-02: `bench_metrics_100k` took 169 ms for 100k lines against the 300 ms budget on a shared 2-core Linux container; run it on the Mac. | M3-26 |
| 6 | APFS behaviors of the symlink-safe app-data deletes (`remove_file`/`remove_tree`/`tree_size` through `unlinkat`). | M4-02 |
| 7 | Cross-check the Run log mockup in both themes. | M4-03 |
| 8 | The real-app keyboard script (`e2e/specs`, macOS only; the ticket's `tests/e2e/keyboard-p1.e2e.ts` path does not exist: the specs live in `e2e/specs`). The WebKit flow project isn't installed in the sandbox. | M3-28 |
| 9 | Confirm `open -u https://claude.ai/` opens the browser. | CR-006 (M3-12, M3-23) |
| 10 | The S-0 real-TUI paste check (Claude Code's TUI receiving `pty_paste`). | M3-12 |
| 11 | AC-F017-09: with Foreman unfocused, a session-blocked or packet-landed item shows a macOS notification (Foreman text only), clicking it only focuses Foreman, and the Dock badge counts the first four kinds. Also `open -b com.apple.Terminal <project folder>` from "Open terminal here". | M3-07, M4-25 |

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

## M2 tickets

Runner halves (`foreman-sys`, `fake-claude`, pure runner modules in `foreman-core`) were built in
`stage/05-runners`; the IPC commands, native dialogs, `run.json` and UI in `stage/05-app` and later
branches. APIs: design doc §3.2.3. A command not built yet answers `Unsupported`
(`notYetAvailable:<ticket>`, ipc.md IPC-R12) after its request validates.

| Ticket | Status | Title | Req. tests | Built, and what's left |
| ------ | ------ | ----- | ---------- | ---------------------- |
| M2-01 | review-pending | IPC command framework | 2/3 | 91 commands registered, granted and validated; §2.4 types; ts-rs bindings; drift tests. The checker doesn't find `ipc::fuzz_*`. |
| M2-02 | in-progress | Mock IPC and contract tests | 1/1 | Built in `77a4db4` (typed IPC contract for all 91 commands and 9 events, stateful mock with TradeLog fixtures); no status recorded. |
| M2-03 | todo | Rust-built main window and startup hardening | 1/3 | No branch recorded this ticket. Missing `webview::navigation_denied_to_remote`, `webview::window_open_denied`. |
| M2-04 | in-progress | App shell, routes and UI primitives | none named | Built in `5e8d9c5` (app shell, tokens, safe rendering, shortcuts and the palette); no status recorded. |
| M2-05 | in-progress | Safe rendering: node trees and visible escapes | 1/5 | Built in `5e8d9c5`, tests in `ab50a98`; no status recorded. Missing `render::escapes_html_in_packet`, `render::no_raw_html_passthrough`, `render::bidi_visible_in_diff`, `render::hostile_filename_escaped`. |
| M2-06 | in-progress | Argv builder | 8/8 | Runner: argv builder, fixed flags, prompt last, `--name`; `argv::*`, `spawn::no_shell_in_any_code_path`, abuse argv. App: `clipboard_copy { target: "command" }` done (`stage/05-chat`). Left: the preview UI. |
| M2-07 | in-progress | Child environment and PATH | 5/5 | Runner: child env and PATH sanitizing; `env::*`. Left: the pass-through setting. |
| M2-08 | in-progress | Child process lifecycle | 2/2 | Runner: PTY/non-PTY lifecycle, no inherited fds, SIGHUP quit path. App: run table and startup scan done, `lifecycle::crash_leaves_interrupted` passes (`src-tauri/tests/launch.rs`). Left: the quit guard (M3-13). |
| M2-09 | in-progress | Run settings file | 4/4 | Runner: run settings file (fresh, `0600`); `run_settings::*`. Nothing listed as left for the app. |
| M2-10 | in-progress | Run record and redacted run log | 4/4 | No row of its own; its tests landed with M3-10's app part (`run.json` before spawn, index lines, `runlog::record_before_spawn`/`argv_prompt_redacted`/`no_env_values`). No status recorded. |
| M2-11 | in-progress | `claude -v` and `claude auth status` readers | 3/3 | Runner: the readers (fail closed). Left: Doctor. |
| M2-12 | in-progress | Settings backend and tool paths | 2/3 | `settings_get`, `settings_update`, `settings_pick_tool`; screen-reader mode in memory until settings v2. Missing `settings::security_fields_require_native_confirm`. |
| M2-13 | in-progress | File opening and reveal | 0/2 | `file_open`, `file_reveal` through `foreman-sys::spawn`. Missing `opener::open_file_refuses_executable_types`, `opener::uses_editor_bundle_id`. |
| M2-14 | in-progress | Settings screen (P1) | none named | Built in `6637582`; no status recorded. |
| M2-15 | in-progress | Doctor (P1) | none named | `doctor_run`: claude path and version, git; the auth row waits for M3-01/M3-04. |
| M2-16 | in-progress | First run | 1/1 | `blueprint_pick_repo`. |
| M2-17 | in-progress | Project setup backend | 2/2 | `setup_*`; `docs/templates/` content from the pinned kit not written yet. |
| M2-18 | in-progress | Setup form | none named | Built in `6637582`; no status recorded. |
| M2-19 | in-progress | Project open, registry and read-only states | 2/2 | `project_open`, `projects_list`. |
| M2-20 | in-progress | Today (P1) | none named | `today_get`, `do_next`; CLI steps wait for `launch_prepare`. |
| M2-21 | in-progress | Stage workspace: step rows and spec line | 1/1 | `step_view`, `step_skip`, `action_preview`, `action_apply`. |
| M2-22 | in-progress | Run inputs and prompt block | 1/1 | `run_input_set`, `optional_part_set`. |
| M2-23 | todo | Redacted config and decision snapshots | 2/2 | No branch recorded this ticket; the checker finds its two required test names. |
| M2-24 | in-progress | Conditions, variables and the outside-change review | 1/2 | `condition_set`, `variable_set`, `pending_*`. Missing `decisions::reject_restores_via_cas`. |
| M2-25 | in-progress | Launch preview and native confirmations | 6/7 | `launch_prepare`, `chain_resume_prepare`, `do_next` → `prepare_launch`; plans held 60 s, one use. Missing `preflight::confirm_required_for_*`. |
| M2-26 | in-progress | Core shortcuts and the terminal drawer shell | none named | The app table listed `step_mark_done` under this ticket. Shortcuts built in `5e8d9c5`, tests in `ab50a98`. |
| M2-27 | in-progress | Screen component tests, e2e smoke and performance marks | none named | Built in `ab50a98` (screen component tests, P1 keyboard flow, axe in both themes, token lint); no status recorded. |
| M2-28 | todo | M2 exit | none named | |

## M3 tickets

| Ticket | Status | Title | Req. tests | Built, and what's left |
| ------ | ------ | ----- | ---------- | ---------------------- |
| M3-01 | in-progress | Fingerprint sources and stat cache | 6/6 | Runner: fingerprint over every source, stat cache, symlink rules, streamed `~/.claude.json`; `preflight::*_detected`, `fingerprint::*`. Nothing listed as left for the app. |
| M3-02 | in-progress | Config-change banner and acknowledgement | 2/2 | Runner: redacted diff and `acknowledge`. App: `config_ack` (native dialog, stale fingerprint refused) done. Left: the banner UI. |
| M3-03 | in-progress | Auth-changing key blocks | 1/4 | Runner: SR-07 blocks incl. `forceLoginMethod`; allowlist. App: `auth_key_allowlist_add` done; `project_env_approve` answers `Unsupported { why: "s7-passed" }` (the S-7′ fail branch isn't taken: Claude Code doesn't apply a project's `env` before the trust prompt). Missing `preflight::blocks_*`, `preflight::blocks_api`, `settings::security_fields_require_native_confirm`. |
| M3-04 | in-progress | Auth check, fail closed | 4/4 | Runner: auth in the step cwd, after the fingerprint, cached. Nothing listed as left for the app. |
| M3-05 | in-progress | Launch pipeline order and the version floor | 2/2 | Runner: DR-120 order, version floor; `launch::progress_phase_order`, `compat::blocks_below_floor`. Left: plan IDs, bench. |
| M3-06 | todo | URL opener and clipboard discipline | 1/3 | No branch recorded this ticket (the opener itself is CR-006's, M3-12/M3-23). Missing `clipboard::read_only_on_user_action`, `clipboard::size_cap`. |
| M3-07 | done | Wait-step notifications | 1/1 | `tauri-plugin-notification =2.7.0` (Rust only, no webview permission), `Os::notify/set_badge/app_focused` (`ScriptedOs` records), templates in `src-tauri/src/notify.rs`; `wait-elapsed` notifies once per item per app run. `notify::body_template_snapshot` (+ `project_name_is_flattened_and_cut`, `badge_counts_the_first_four_kinds`). Mac only: the real notification (Mac checks, item 11). |
| M3-08 | todo | CSP on the built app | 1/1 | No branch recorded this ticket. |
| M3-09 | in-progress | PTY session | 3/3 | Runner: `PtySession` (env exact, no fds, back-pressure, live-only writes). Left: the Channel adapter. `PtySession::wait_exit` returns only after the sink was told of the exit (`pty::write_requires_live_run` raced the sink: 3 of 60 runs under load failed; 0 of 60 after). |
| M3-10 | in-progress | Launch start and terminal tabs | 2/2 | Runner: `start_session`. App: `launch_start`, `run.json` before spawn, index lines, `runlog::record_before_spawn`/`argv_prompt_redacted`/`no_env_values` done. Left: the tabs UI. |
| M3-11 | todo | Terminal host and key routing | 0/2 | Missing `terminal::options_snapshot`, `terminal::no_answerback_injection`. |
| M3-12 | in-progress | Terminal paste and links | 3/3 | Runner: SR-28 paste sanitizer; `pty::paste_strips_escape_sequences`. App: `pty_paste` done; `terminal_link_open` done (`stage/05-chat`: `foreman_core::urls::terminal_url`, native dialog, `foreman-sys` `open -u`, CR-006; `opener::terminal_link_scheme_policy`, `terminal::link_handler_routes_to_rust`). Left: the `@xterm/addon-web-links` handler (the addon isn't installed; only OSC 8 links route today) and the S-0 real-TUI paste check (Mac checks, item 10). |
| M3-13 | in-progress | Tab menu, terminate and the quit guard | 3/3 | Runner: terminate (SIGTERM → SIGKILL), hangup. App: `pty_terminate` native dialog done. Left: the quit guard. |
| M3-14 | in-progress | Session chains: planning and session flags | 7/7 | Runner: chain planning, S-3/S-8 branches, transcript path; `chain::*`. Nothing listed as left for the app. |
| M3-15 | in-progress | Re-arm | 5/5 | Runner: re-arm machine and driver; `rearm::*`. App: `rearm_start`/`rearm_continue` with the native idle dialog done (`rearm::relaunches_after_idle_confirm`). |
| M3-16 | done | Restart, interrupted sessions and rebuild from the repo | 1/1 | `progress_rebuild_preview`/`_apply` (unreadable file kept as `progress.json.corrupt-<ts>`, CAS, single use); a missing `progress.json` is rebuilt from packets on open; `read_progress` error names the file; Rebuild from repo on the project error screen; Interrupted sessions with Resume on Today (`chain_resume_prepare` → `launch_start`); `src-tauri/tests/restart.rs` (`lifecycle::crash_leaves_interrupted` restart case, `rebuild::*`). A hand signature confirmed before app data was deleted must be confirmed again (it lived in approvals; DR-135). Stage records (M4) will add signing entries to the rebuild. |
| M3-17 | done | Followup and manual steps, switches and overrides | 4/4 | Followup copy returns the chain's live tab (`focusRun`), the UI copies-and-focuses a live chain and relaunches otherwise; Open file starts a `manual` step and a later change to its file finishes it (re-stat on open); a per-run override is consumed by the run that used it (DR-103, it wasn't before). Tests: core `steps::followup_copy_or_relaunch`, `steps::finish_rules`, `policy::switches_never_typed`, `policy::override_scope`, plus `src-tauri/tests/followup.rs` (same names, against fake-claude). |
| M3-18 | done | Probe-folder trust at first run (moved from M3) | 1/1 | `probe_folder_open` (`src-tauri/src/ipc/commands/doctor.rs`): the probe folder reset, then `claude --name fm-probe` in it as a terminal tab; records `probeFolderTrusted` (ipc.md as-built note). Doctor's `probeFolder` row offers Open probe folder. Test `probe::runs_only_in_probe_dir` (trust-tab case, `src-tauri/tests/probe.rs`). Mac check: Claude Code's trust prompt in `~/Library/Application Support/dev.foreman/probe`. |
| M3-19 | in-progress | Watcher | 5/5 | Runner: watcher (interest set, lstat roots, re-read by stamp); `watcher::*`. App: `ProjectWatcher` → `packet:changed`/`decision:pending` mapping and config hint watchers done in `stage/05-packets` (M3-21, M3-22). Integration: `watcher::refocus_rescan_catches_missed_event` (ADR-013, watcher disabled) passes; the interest set is re-stat'ed on focus (also for a project whose watcher isn't running) and before every launch and sign (`watch::restat_before_acting`). Left: the runner branch's own close-out of the ticket. |
| M3-20 | done | Packet normalization, splitting and fix requests | 3/3 | `packet::normalize_packet` (DR-131 notes per kind with lines), `packet::split_paste` (DR-132: preamble kept apart, two packets / wrong stage refused), `packet::fix_request_text` (DR-137; `Problem.field` added); `crates/foreman-core/tests/packet_paste.rs` (`packet::normalization_golden` over Foreman's own packets, `split_paste`, `fix_request_text`, variants, idempotence proptest). `paste_back_preview` passes `packet::normalize_paste` to `pasteback::plan` (test `packet::pasteback_uses_full_normalizer`); the planner keeps its own DR-132 split, so `split_paste` is the core's reference implementation; `clipboard_copy { fixRequest }` uses the core text. |
| M3-21 | done | Packet landing and hand-signed packets | 1/1 | `packet_view` normalization notes and the AC-F004-12 problem; `packet_normalize_apply` (CAS, `packet:changed`); `packet_signature_confirm` tested (Cancel, stale, void on change); watcher started by `project_open`, mapped in `src-tauri/src/watch.rs`; Doctor warning on a stopped watcher; focus re-stat; Packet screen "Formatting Foreman can fix"; `src-tauri/tests/packet_landing.rs` (`packet::outside_signature_needs_confirmation`, `watcher::packet_visible_within_1s` …). FSEvents latency is Mac checks, item 4. |
| M3-22 | done | Live outside-change detection and config-changed runs | 2/2 | `project::refresh_decisions` from watcher hints (`decision:pending` live); per-run `HintWatcher` over the fingerprint sources replaces the 5 s tick (now a 60 s fallback, 5 s only without a watcher); `src-tauri/tests/outside_change_live.rs` (`decisions::outside_edit_has_no_effect_until_accepted` live, `watcher::config_changed_by_watcher_event`, `watcher::config_targets_relevance`). `watcher::config_changed_marks_open_runs` stays in `tests/launch.rs`. |
| M3-23 | done | Chat chips, links and Copy and open | 2/2 | `chat_copy_and_open` (sanitized prompt, SR-25 recheck of the stored link, `chatCopy` run record with time to start, prompt hash only), `chat_link_set` (claude.ai only, stored on the chain start), chips from the RUN WITH prose (`ChatChips.svelte`), the step says which link to set (inline Project link form). Tests: `chain::chat_links`, `chain::stored_link_rechecked`, `opener::only_allowed_urls` (core and app), `src/lib/components/chat.svelte.test.ts`. Integration: the run record now goes through `runs::record_chat_copy` (M3-26). |
| M3-24 | done | Paste-back preview and apply | 2/2 | `foreman_core::pasteback` plans every target from declared paths only (packet, attachments, extra outs, outputs; wrong stage, two packets, P1 appends refused); apply re-checks every stamp before the first write. Field-level normalization is M3-20's `packet::normalize_paste` (`pasteback::plan` takes the normalizer as a parameter; swapped in `stage/05-packets`; `pasteback::prepare`, the stand-in until M3-20, stays as the planner tests' structural baseline). Integration: the P2 kinds and the sensor source (M4-18, M4-23, M4-24) are wired; a valid paste-back ends the step's chat clock. Tests: `pasteback::lists_targets_before_write` (packet; plus `lessons::`, `adr_set::`, `capture::pasteback::lists_targets_before_write`), `atomic::refuses_newer_file` (paste-back conflict), invalid packet → fix request, clipboard errors, policy refusal, AC-XC-16 `pasteback::no_chat_content_stored`. AC-F012-13's Open action is Waiting on Billy, item 11. |
| M3-25 | done | Close-out checklist step (P1) | 1/1 | `foreman_core::steps::closeout`, `StageView.closeoutChecklist` (ipc.md §2.4), `CloseOutChecklist.svelte` once the packet is valid and the stage unsigned. Test: `steps::closeout_sequences`, component tests. Its F.2 wording is Waiting on Billy, item 12. |
| M3-26 | in-progress | Run metrics and Export metrics | 1/1 | `metrics_export` (native save dialog; `.md` only; `WritePurpose::UserExport`), `metrics_view`; the engine is `foreman_core::runlog` (`parse_index`, `merge_latest`, `analyze`, `Baseline`, `export_metrics`); tests `metrics::runlog::*`, `metrics_view::*`, `export::*` (core and app). Integration: `chat_copy_and_open` records through `runs::record_chat_copy` (one path, one index line; the sanitized prompt's hash; interventions; override), and `step_mark_done` and `paste_back_apply` call `runs::note_step_done`; `kill_check::chat_step_end_to_end` checks the kill-check numbers through the real commands. Left: the real-Mac performance run (Mac checks, item 5). Definitions below. |
| M3-27 | todo | TradeLog harness argv snapshots | 1/1 | No branch recorded this ticket; the checker finds `argv::positional_prompt_snapshot`. |
| M3-28 | in-progress | Keyboard-only P1 loop | none named | `tests/flows/p1-loop.spec.ts`: a keyboard-only mock flow for create, chat steps, paste back, config acknowledge, CLI run, re-arm of a continuing step, terminal focus out and back (3 tests, chromium). Fixes: `selectStep` also walks up with K; the mock gains `setRunIdle` (the P2 sensor's idle status). Left: the real-app keyboard script (Mac checks, item 8). |
| M3-29 | todo | Stage 8 security inspection | 0/1 | Missing `network::blocked_network_run`. |
| M3-30 | todo | TradeLog Stages 1–2 through Foreman | none named | |
| M3-31 | todo | Kill check | none named | |

## M4 tickets

| Ticket | Status | Title | Req. tests | Built, and what's left |
| ------ | ------ | ----- | ---------- | ---------------------- |
| M4-01 | todo | Diagnostic logging and crash reports | 1/2 | Missing `logging::redaction`. |
| M4-02 | in-progress | Retention, Clear run history and project removal | 3/3 | `runs_clear`, full `project_remove` (`projects/<id>/`, approvals, fingerprints; the repo is untouched), 90-day tail pruning on the startup scan, `remove_file`/`remove_tree`/`tree_size` through symlink-safe `unlinkat` in `store::app_dirs`, a native confirmation for each, `settings.runHistory` size; tests `privacy::*`, `retention::*`, `app_data_delete::*`. Left: template/event pruning (events belong to M4-09); the project-removal UI (no screen calls `projectRemove`; Rust raises the native confirmation); APFS behaviors (Mac checks, item 6). |
| M4-03 | in-progress | Run log screen and metric panel | none named | `runs_search` (newest 3000 records scanned when a query is given; paged), the strip on `RunLog` (time to start, p95, active time, edits, baseline comparison); tests `search::*`, `RunLog.svelte.test.ts`, a11y flow. Left: the mockup cross-check (Mac checks, item 7). |
| M4-04 | done | Annotate blueprint v2.2: Stages 0 and 3–5; golden through Stage 5 | 1/1 | The M1-01 generator (`scripts/blueprint/annotate-v2.2.py`) already wrote DR-038's annotations for Stages 0 and 3–5, so the fixture is unchanged; what M4-04 adds is the proof: snapshots `golden_v2_2_stage_0`, `_3`, `_4`, `_5` and `golden_v2_2_stages_0_5_lint` beside the Stages 1–2 and Gate ones, `template::golden_v2_2_stages_0_to_5` (zero errors, 40 runnable steps, 6–18 still needs-annotation) and `template::pin_clean_through_stage_5` (pin check clean; an error in Stage 0, 3, 4 or 5 is scoped and doesn't block the pin). Two lint warnings remain (4.7, 5.5): CR-004 (Waiting on Billy, item 3). |
| M4-05 | todo | Resolver and harness through Stage 5 | 1/1 | No branch recorded this ticket. |
| M4-06 | in-progress | Template health screen | none named | Screen built in `6637582`; no status recorded. Template-health data (from M4-04): the core already carries every field (steps' annotations, outs, lint diagnostics, per-stage status). The `TemplateHealth` DTO (`src-tauri`) has the per-stage rows and diagnostics but not the per-runnable-step annotations and outputs (AC-F003-05); adding them is this ticket's work. |
| M4-07 | done | Sensor binary | 3/3 | `foreman-sensor` (tests `sensor::rejects_wrong_token`, `sensor::stdin_cap`, `sensor::exits_under_10ms` in `crates/foreman-sensor`). Install: the release build embeds it (`FOREMAN_SENSOR_BIN`, `build.rs`, `scripts/build-release`); `foreman-app::sensor` installs it `0700` at `bin/foreman-sensor-<sha12>` (`core::store::app_dirs::install_executable`); `sensor::installed_from_bundle`. ADR-009 amended. Mac checks: `pnpm release` embeds and the installed copy runs (its ad-hoc linker signature survives the copy); `cargo bench -p foreman-sensor` ≤ 10 ms. |
| M4-08 | done | Sensor hooks in the run settings | 3/3 | `launch::prepare` asks `sensor::hooks_for_run` (hash checked per run, re-install once, else no hooks + warning); token into `run.json` `sensorToken`; `events/<runId>/` created before spawn. Tests (`src-tauri/tests/sensor_ingest.rs`): `sensor::exec_form_only`, `sensor::hash_checked_per_run`, `fs_policy::denies_claude_settings_writes` (sensor setup paths). |
| M4-09 | done | Sensor ingest and derived session state | 3/3 | `foreman-app::sensor_ingest`: per-run ingest thread, derived state, `run:status` idle/blocked, 20 files per run, `observed` in `run.json`, the mode guard on the step view (AC-F011-06), read API `sensor_ingest::session_state` / `last_assistant_message` (Capture from session: `capture::reply_captured_end_to_end`). Retention: `sensor::prune_events` at the startup scan. Tests `sensor::events_only_update_state` (+ module-dependency scan), `sensor::rejects_wrong_token`, `sensor::mode_guard_warns`, `sensor::keeps_last_20_events`, `retention::events_max_age`, `retention::events_deleted_on_sign` (`sensor_sign.rs`). |
| M4-10 | done | Sensor idle, background tasks and tab states | 1/1 | `rearm_start` reads `sensor_ingest::idle_source` at Run: idle → no dialog; tasks → "N background tasks still running" (Wait → `NotAllowed(background-tasks-running)`); otherwise the P1 dialog; `pty_attach` status from the sensor. Mock: `setRunSensor` (status + tasks; `setRunIdle` kept as a wrapper), `rearmStart` in step. Tests `rearm::only_from_user_action`, `rearm_manual::manual_idle_without_events` (fake-claude `tui.noHooks`), `src/lib/ipc/mock.sensor.test.ts`, `TerminalDrawer.svelte.test.ts` (AC-F009-14). |
| M4-11 | todo | Now/Next strip, round tracker and focus mode | none named | |
| M4-12 | done | Contract probes | 6/6 | `foreman-app::probe` (run, merge, store; launch blockers/warnings by feature) and `probe_run`; `launch::prepare` runs the probes first on a version change once the probe folder is trusted and the sensor is built in (AC-F008-21). App tests (`src-tauri/tests/probe.rs`): `probe::runs_on_version_change`, `probe::runs_only_in_probe_dir`, `probe::dir_reset_before_run`, `probe::inconclusive_keeps_state`, `probe::failure_disables_feature`, `preflight::confirm_required_for_version_change` (plus the core and sys tests of the same names). Doctor: Run probes now. Mac check: S-1′ on the real Claude Code (probe A marker through the installed sensor). |
| M4-13 | done | Full Doctor | 1/1 | Every row (ipc.md as-built note): sensor, probe folder, `cleanupPeriodDays` with a recommendation, `disableAllHooks`, broad allow rules, Appendix A artifacts, git/node/pnpm/cargo at fixed paths with `--version` in app data, the launch PATH, login PATH, capability table naming blocked steps, probe rows. Tests `spawn::helper_cwd_is_app_data` (toolchain, `src-tauri/tests/doctor.rs`), `doctor_rows::full_doctor_rows`, `doctor_rows::capability_names_blocked_steps`; Doctor component tests (Run probes now, Open probe folder). Mac check: both themes against `doctor.html`. |
| M4-14 | done | Login-shell PATH capture | 1/1 | `doctor_capture_login_path` / `doctor_login_path_apply`: `/etc/shells` check, native consent, `foreman_sys::spawn::SpawnRequest::LoginShellPath` (fixed argv, app data, 10 s), data-only parse, second native dialog to store. Tests `doctor::login_path_requires_consent` (real `/bin/bash`, `~/.bash_profile` marker), `doctor::login_path_output_is_data`, sys `login_shell::fixed_argv_in_app_data`, `spawn::no_shell_in_any_code_path` extended. AC-F019-13 (S-12 fail branch: capture required at first run) isn't built; the pass branch applies until S-12 is recorded. AC-F019-14 is a Mac check (a Finder-launched build resolves claude, git, node, pnpm, cargo). |
| M4-15 | done | Gate model | 3/3 | `foreman-core::gate` (state machine, attest rules, order, signable). Tests `gate::signable_requires_all` (property), `steps::attest_note_rules`, `steps::gate_item_order`, plus `gate::state_machine_rejects_out_of_order`, `gate::never_signable_with_unresolved_items`. |
| M4-16 | done | Gate runs, Import results and Fix this | 1/1 | `gate_view`, `gate_import_results` (proposals only), `gate_import_confirm`, `gate_fix_start` (a detached new chain, `launch::prepare_custom`; F.4 on the third FAIL). `sensor::events_only_update_state` (app). Import results answers `Unsupported` for runs without a sensor token, which is every run until the P2 sensor hooks land (M4-08/M4-09). |
| M4-17 | done | Gate screen and item re-confirmation | 2/2 | `gate_item_attest`, `item_kind_confirm` (native dialog, `approvals.itemKinds`). `GateItemView` gained `textSha256` and `previousText` (ipc.md §2.4, bindings, mock, fixtures). The Gate screen sends the real hash and shows the pinned and new text. `template::swapped_bullets_reconfirm` and `ipc::security_confirmations_are_native` (app, `src-tauri/tests/gates.rs`). |
| M4-18 | done | Lessons capture | 2/2 | `foreman-core::lessons` (entries, the stage requirement, next ID, the header's entry format, the CAS append with renumbering) and `lessons_status` (`commands/lessons.rs`). The F.2 prompt gets the header's format appended (`steps::resolve`). Integration: `paste_back_preview/apply { kind: "lessons" }` reads through `lessons::read` (the diff; missing file refused, nothing written) and writes through `lessons::append(root, seen_stamp, entries)` (a change since the preview is `Conflict`, the next preview re-diffs); the step row's Paste back sends `lessons` on F.2 steps. Tests `lessons::append_numbering`, `steps::lessons_entry_detected`, `lessons::f2_prompt_gets_header_format`, `lessons::status_through_ipc`, `lessons::pasteback::lists_targets_before_write`, `lessons::changed_on_disk_is_refused_and_rediffed`, `lessons::missing_file_writes_nothing` (`src-tauri/tests/pasteback.rs`). |
| M4-19 | done | Sign preview and Sign gating | 3/3 | `sign_preview` (`src-tauri/src/signing.rs`): gating from `closeout` + `gate::sign_blockers` (git missing names git, AC-F014-03; Stage 2 conditions), the added fields as a diff (`packet::sign_fields`), the 6-hex content hash, paths with notes, Stage 2 `confirmConditions`. Tests `packet::added_fields_golden`, `rules::skipped_queue`, `gate::signable_requires_all`, `sign::preview_shows_fields`, `sign::git_missing_disables_sign`. |
| M4-20 | done | Sign apply on the current branch | 8/8 | `sign_apply`: re-read (Stale with a fresh preview), the native sign dialog, phase 1 (APPROVED BY in place, the added fields, `.foreman/stages/stage-NN.json`, pending decision files, all by CAS), `commitPending`, then `commit_paths` with read-back and the real-index refresh; the stamp only after read-back; sensor events of the stage deleted (SR-53). `indexRefreshPending` is retried on `project_open`. Tests in `src-tauri/tests/sign.rs`: `sign::refuses_changed_file`, `sign::pathspec_only`, `sign::readback_detects_mismatch`, `sign::untracked_new_packet`, `sign::concurrent_ref_move_fails`, `sign::index_locked_refresh_retried`, `ipc::security_confirmations_are_native`, `packet::outside_signature_needs_confirmation`, `decisions::pending_decisions_ride_in_signing_commit`. Left: the index-refresh retry on window focus (the focus hook, `watch::on_focus`, doesn't retry it yet; open only); `SignDialog.svelte` stays inline in the Gate screen. |
| M4-21 | done | Sign on another branch, commit pending and git edge cases | 4/4 | Other-branch signing (`Target::Other`, CR-007: `rev-parse --verify -q refs/heads/<b>` to find the tip), `sign_retry_commit`, commit pending after a kill between phases, `committedOnOtherBranches` with `Decisions::process_on` (no false drift on checkout), the 60 s timeout message. Tests `sign::other_branch_leaves_worktree_head_and_index` (HR-05/HR-06 fixture), `git::repo_gpg_program_never_runs` (HR-05/HR-06 from `scripts/make-hostile-repo.mjs`, with a positive control), `git::timeout`, `decisions::other_branch_sign_no_false_drift`, `sign::killed_between_phases_retry`. CR-007 is Waiting on Billy, item 5. "Open terminal here" on commit pending waits for `terminal_open_here` (M4-33). |
| M4-22 | done | Tamper detection and restore | 1/1 | On open, a `Signed` packet whose working hash differs from `signing.packetSha256` is read from its commit (the resolver uses it) and kept in `Opened::tampered`; `tamper:detected` on `project_open`; a Today inbox item; `tamper_restore` writes the committed blob back by CAS. Integration: `packet_view` sets `tamper: signing::tamper_diff(&p, n)` (`tamper_restore` returns the same view). Tests `sign::tamper_detected`, `sign::packet_view_shows_the_tamper_diff`. Left: the watcher's `packet:changed` for a signed packet doesn't re-run detection; it runs on open and Today (and the re-stat before launch and sign emits the event). |
| M4-23 | done | Packet repair, capture and open items | 2/2 | `packet_repair_prepare` (in `commands/signing.rs`): problems from Rust's validation, a re-arm of the producer's chain via `launch::prepare_custom`, `not-cli-packet`, `chain-handed-off`; Billy continues with `rearm_start`. The Packet screen states OPEN ITEMS' two outcomes (AC-F013-13). Integration: Capture from session is `paste_back_preview { source: "sensor" }`: the step's last run's `last_assistant_message` through `foreman_sys::sensor::ingest`, then the clipboard path (normalize, split, `check_write`, preview; only apply writes). Without a reply it answers `Unsupported { feature: "sensor", why: "noSensor" }`, which is every real run until M4-08/M4-09 give runs a sensor token (no `notYetAvailable:M4-09` needed: the ingest exists). The old `packets::packet_repair_prepare` stub is deleted. Tests `rearm::only_from_user_action` (`src-tauri/tests/packet_repair.rs`), `capture::pasteback::lists_targets_before_write`, `capture::without_the_sensor_is_unsupported` (`src-tauri/tests/pasteback.rs`). |
| M4-24 | done | ADR-set split and chat close-out | 2/2 | `foreman-core::packet::adr_split` (ADR-set split on `^# ADR-\d{3}:`, every target listed with its current text before a write); the chat close-out order (packet → attest → F.2 lessons → Sign) is the gate view's sequence for chat stages. Integration: `paste_back_preview/apply { kind: "adrSet" }` uses `split_adrs` + `plan`, lists every ADR file with its diff, and applies by CAS (every stamp checked before the first write). Tests `pasteback::lists_targets_before_write` (ADR targets, core), `adr_set::pasteback::lists_targets_before_write`, `adr_set::changed_target_writes_nothing` (app), `steps::closeout_sequences`. No separate `CloseOut.svelte`: the Gate screen's sequence row is the close-out. |
| M4-25 | done | Today inbox and notifications | 1/1 | `src-tauri/src/inbox.rs` (sources, AC-F017-07 rank, notifications, Dock badge; `inbox::*`, 8 tests), `today_get` sorted, 60 s refetch, sidebar status dot (shape + words) and pin number, title-bar `ProjectMenu` and palette entries (pin, Remove through the native dialog, Open terminal here), `terminal_open_here` (`Os::open_terminal`; `terminal_here::*`), Gate "Open terminal here" on commit pending. Vitest: `inbox`, `Today`, `Sidebar`, `ProjectMenu`. Left: nothing; `step-ready` is never emitted (Next step rows show it). |
| M4-26 | done | Setup wizard | none named | Component tests for the four steps, Settings defaults, one line per condition, the reviewed path and writes, CR-003 errors and Back; Create lands on the first action with focus (`ui.landOnAction`, `StageWorkspace` test). |
| M4-27 | todo | Knowledge manifest | none named | `manifest_view`, `manifest_mark` answer `notYetAvailable:M4-27`. |
| M4-28 | in-progress | Build-package assembler and root `CLAUDE.md` | 1/3 | Screen built in `6637582`; its commands answer `notYetAvailable:M4-28`. Missing `assembler::rejects_policy_violations`, `rules::late_condition_evidence`. |
| M4-29 | todo | Desktop handoff | 1/1 | `desktop_open`, `desktop_handoff` answer `notYetAvailable:M4-29`. |
| M4-30 | done | Overlay additions: rules and validation | 6/6 | Built in the UI wave because M4-31 depends on it (nothing else had picked it up). `foreman-core::rules::overlay` (`add_step`, `add_item`, `effective_template`, `validate_step`; `rules.rs` became `rules/mod.rs`). Tests: `rules::overlay_add_only`, `added_step_rules`, `added_item_attest_only`, `added_step_validated`, `overlay_roundtrip` (`crates/foreman-core/tests/overlay.rs`) and the overlay case of `template::rejects_bypass_permissions`. |
| M4-31 | done | Add a prompt or inspection item | none named | `add_step`, `add_item` are real (`src-tauri/src/ipc/commands/overlay.rs`; `project.rs` splices the effective template; tests `overlay::add_step_is_stored_and_listed`, `added_step_always_confirms`, `add_item_lands_in_the_gate`, `refusals_write_nothing`). `InsertPoint.svelte` replaces the old insert dialog in the stage workspace: menu (Add prompt, Add inspection item), copy-config picker, surface, folder, mode, model, effort, outputs, optional/core, the Rust refusal sentence in the dialog; component tests in `InsertPoint.svelte.test.ts` (9, with the workspace's "Added for this project" badge and the overlay-step confirm). |
| M4-32 | todo | Run sheet export | none named | `run_sheet_export` answers `notYetAvailable:M4-32`. |
| M4-33 | done | Command palette and remaining shortcuts | none named | `Palette.svelte` lists every A.7 action beside steps, projects and stages (each runs the shortcut's own handler, so a refusal gives the same one-line reason), caps each group so none is pushed out, and shows the open stage's steps when the query is empty; `terminal_open_here` is built (M4-25). Component tests: `Palette.svelte.test.ts` (11: groups, every action and key, filtering, ↑↓, ⌘↵ run, empty state, Esc focus return, inert background, refusal reason) and `dispatch.svelte.test.ts` (13: ⌘K from the terminal, ⌘N, ⌘,, ⌘1/⌘9, ⌃`, ⌘⌥I, ⌘⇧T, ⌘⇧E, ⌘⇧↵, ⌘] ⌘[ ⌘G ⌘⇧G). Also fixed a late xterm write callback reading a closed tab in `TerminalView`. |
| M4-34 | todo | Settings (P2) | 1/2 | Missing `settings::security_fields_require_native_confirm`. |
| M4-35 | todo | Stage 11–12 inspections and M4 exit | none named | |

### Commands not built yet (IPC-R12), by ticket

Each answers `Unsupported { why: "notYetAvailable:<ticket>" }` after its request validates.
`project_env_approve` is built: it answers `Unsupported { why: "s7-passed" }` because the S-7′ fail
branch isn't taken.

| Ticket | Commands |
| ------ | -------- |
| M4-27 | `manifest_view`, `manifest_mark` |
| M4-28 | `assembler_view`, `assembler_paste_preview`, `assembler_paste_apply` |
| M4-29 | `desktop_open`, `desktop_handoff` |
| M4-32 | `run_sheet_export` |

## Run metrics: definitions and assumptions (M3-26, M4-02, M4-03)

Stated in the export too.
- The baseline is `docs/metrics-baseline.md` of the first registered project that has a filled log; the chat baseline needs at least 10 steps (kill check, per surface). Desktop runs are not in the kill check.
- Time to start leaves out `rearm`/`repair` entries. Active time per step is the sum of `timeToStartMs + durationMs - waitMs` per (surface, project, step), counted only if some run finished and none was interrupted. Median is the midpoint of an even pair; p95 is nearest-rank.
- Edits are run-input interventions (a run input set again to a different value; the first set is not one) plus Copy prompt on a CLI step. Marks live in memory and reset on restart; so does the chat step-done duration.
- `BaselineComparison` is the median time to start over both surfaces against the by-hand median ([CR-008b](change-requests/CR-008b-metrics-view-per-surface-baseline.md)). The latest index line per `runId` wins, and a chat step has a second line ([CR-008a](change-requests/CR-008a-run-index-latest-line-wins.md)).
- The export path policy is stricter than SR-30 asks: `.md` only, never inside `.git`, `.claude` or `.foreman` folders anywhere, never `CLAUDE.md`, never the app's data folder.
- A month index file is read up to 8 MB (`ReadClass::AppState`).
- `ipc.md` writes `MetricsView` without `p95_all_ms` in the §1052 Rust block but has `p95AllMs` in the TS block; the code has the field.
- The old `metrics_*`/`runs_*` stubs in `src-tauri/src/ipc/commands/packets.rs` are deleted (integration).

## Known deviations (M1, reviewed 2026-10-20)

Where the M1 build differs from the docs, the doc that should change (or the decision Billy owes), and what was done. "Doc changed" means this branch edited it; the rest wait for Billy.

| # | Deviation | Where | Doc that should change / decision | Status |
| - | --------- | ----- | --------------------------------- | ------ |
| 1 | The hostile-repo fixture and the 20,000-line blueprint are generated in code each run, not committed (`fixtures/hostile-repo/`, `fixtures/blueprint-20k.md`). Generating them keeps the repo small and the content deterministic; the 20k file is the v2.2 blueprint plus synthetic appendix notes, so it is not a real blueprint. | `crates/foreman-sys/tests/common/mod.rs` (`hostile_repo`), `crates/foreman-core/benches/template.rs` (`blueprint_20k`) | `docs/tickets.md` M1-08 and M1-32 "Likely files". | Doc changed. No CR (no behavior). |
| 2 | The file-system-reported name is read by inode from a `getdents` listing, not `F_GETPATH`, and Linux fakes it through `Root::with_name_reporter`. The real case-insensitive volume test (`fs_policy::case_and_normalization_variants`, an APFS disk image) runs on macOS only; Linux proves only the hook (`reported_name_checked`). The case-sensitive volume half of ADR-011's verification isn't built. | `crates/foreman-core/src/store/walk.rs`, `tests/fs_policy.rs` | ADR-011 (FS-reported name check, Verification). | ADR-011 changed. Mac checks, item 1. |
| 3 | Pinning didn't refuse annotation errors. `lock::pin` refused only a file that differed from HEAD or a re-pin after Stage 1 is signed, then pinned whatever parsed. No document required refusal (AC-F001-15 says errors in an unused stage are listed and the project opens), but the Stage 1 exit criterion expects the P1 stages to parse with zero errors, and a pinned P1 step in error can't be repaired once Stage 1 is signed. | `crates/foreman-core/src/lock.rs` (`pin`) | [CR-003](change-requests/CR-003-pin-refuses-p1-annotation-errors.md) (Approved, option B): DR-155, AC-F002-02, `docs/contracts/ipc.md` (§3, `setup_preview`, `blueprint_repin`) and M1-17 changed, each edit marked "(CR-003)". | **Resolved by CR-003**, approved by Billy on 2026-10-20 (in chat, 12:38 ET). `lock::pin` refuses while Stages 1–2 or F.2 have errors and returns the diagnostics (`LockError::AnnotationErrors`); errors in other stages still pin. Tests `lock::pin_refuses_p1_annotation_errors`, `lock::pin_allows_errors_outside_p1`, `lock::pin_clean_p1`. **Follow-up (M2): done** in `stage/05-app` (`IpcError{notAllowed, detail: annotationErrors}`, binding, mock, Setup list). |
| 4 | Foreman's own decision change won't overwrite an unreviewed outside edit: `own_change` returns `OutsidePending` when the repo file is neither the approved nor the last committed content. SR-33 and ADR-022 §4 imply it; no rule said it for Foreman's own writes. The IPC error for it isn't in `docs/contracts/ipc.md`. | `crates/foreman-core/src/decisions.rs` (`own_change`) | DR-153 and ADR-022 §5 (the refusal). `docs/contracts/ipc.md` needs an error case on the commands that change decision files; that file belongs to the src-tauri/IPC work, so it is not edited here. | DR-153 and ADR-022 changed. **Follow-up: done** (ipc.md §2.4: `Conflict` with the file's path). |
| 5 | From Stage 5 on, Foreman's own decision changes go to the approved copy and `progress.json` only; the repo file is left alone until the next signing commit. DR-153 read as if the repo file were written too, which contradicted its own drift rule (the repo file must still match `committedSha256`) and ADR-022 §5. | `decisions.rs` (`own_change`, `stage5_signed`) | **DR-153 now matches ADR-022 §5.** ADR-022's rule is the safer one: the working-tree file stays what Claude Code (and `git status`) sees as committed, nothing uncommitted can be swept into a commit that bypasses Foreman's signing, and drift needs no special case. | DR-153 changed. Billy confirms with the Stage 5 packet (Waiting on Billy, item 8). |
| 6 | A CLI prompt step is done only on exit 0; a non-zero exit ends the run (code recorded) and returns the step to `notStarted`. DR-066 says a CLI step finishes "when the run exits (any code; the exit code is recorded)", and DR-065 says `running → done` on "exit 0" and `running → notStarted` when errored. The previous code made every exit code done. | `crates/foreman-core/src/steps/mod.rs` | Reading: DR-066's "any code" means the run is over and the code is recorded; DR-065 decides the step state. DR-066 should say "the run ends on any exit code (recorded); exit 0 finishes the step (DR-065)". | Code and tests fixed (`steps::finish_rules`, `steps::transitions_exhaustive` snapshot). DR-066 wording aligned with DR-065 (no behavior change); Billy confirms with the Stage 5 packet (Waiting on Billy, item 9). |
| 7 | The real-index refresh runs with attributes neutralized and is skipped when `info/attributes` is non-empty. | `crates/foreman-sys/src/git/` | [CR-002](change-requests/CR-002-git-attributes-on-index-refresh.md) (Approved). | **CR-002 approved** by Billy on 2026-10-20 (in chat, 12:38 ET). Doc changed: SR-44, SR-45, AC-XC-04, ADR-012, T-040, DR-140, DR-143, design doc, files.md §4.3 (each edit marked "(CR-002)"). The git 2.39 residual stays open (Mac checks, item 3). |

Item 3: see [CR-003](change-requests/CR-003-pin-refuses-p1-annotation-errors.md).

## Open follow-ups

- `settings.json` v2 with `screenReaderMode` (core schema + migration; files.md §4.1 proposal).
- `foreman-core::store::dirs::ensure_dir` (new, for setup's empty folders) needs a core review.
- (CR-003) The setup preview's pin check is `foreman_core::lock::p1_annotation_errors`, the same check `lock::pin` runs (the separate `pin_check` module and its copies of `problem`/`step_at` were folded into `lock.rs` at integration; `pin_check::preview_check_matches_pin` still pins the two together). Pin refusals map to `IpcError{notAllowed, detail: annotationErrors}`; Setup and the wizard list each error (`AnnotationProblems.svelte`). No re-pin screen exists yet; it should reuse that component.
- The launch's own environment takes `HOME` from the app's home folder (`App::own_env`), so the transcript check and the child agree on `~/.claude` (tests no longer write to the real `~/.claude/projects`).
- (stage/05-w3-sensor) Runs launched with the sensor carry a `sensorToken`; Capture from session and Import results read it (`capture::reply_captured_end_to_end`). A build without `FOREMAN_SENSOR_BIN` still answers `noSensor`.
- Item 4 of Known deviations: the `OutsidePending` refusal maps to `Conflict` with the file's path (ipc.md §2.4).
- `cargo deny`: `thiserror` 1.0.69 / `thiserror-impl` 1.0.69 are skipped in `deny.toml`. `portable-pty` 0.9.0 (newest) needs `filedescriptor` 0.8.3 (newest, crates.io, published 2025-02-11), which requires `thiserror ^1.0`; no release aligns it with the workspace's `thiserror` 2. Drop the skip when `filedescriptor` moves to `thiserror` 2.
- `.vitest/attachments/` (Vitest browser-mode screenshots) is untracked and in `.gitignore`.
