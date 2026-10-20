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

## Known deviations (M1, reviewed 2026-10-20)

Where the M1 build differs from the docs, the doc that should change (or the decision Billy owes), and what was done. "Doc changed" means this branch edited it; the rest wait for Billy.

| # | Deviation | Where | Doc that should change / decision | Status |
| - | --------- | ----- | --------------------------------- | ------ |
| 1 | The hostile-repo fixture and the 20,000-line blueprint are generated in code each run, not committed (`fixtures/hostile-repo/`, `fixtures/blueprint-20k.md`). Generating them keeps the repo small and the content deterministic; the 20k file is the v2.2 blueprint plus synthetic appendix notes, so it is not a real blueprint. | `crates/foreman-sys/tests/common/mod.rs` (`hostile_repo`), `crates/foreman-core/benches/template.rs` (`blueprint_20k`) | `docs/tickets.md` M1-08 and M1-32 "Likely files". | Doc changed. No CR (no behavior). |
| 2 | The file-system-reported name is read by inode from a `getdents` listing, not `F_GETPATH`, and Linux fakes it through `Root::with_name_reporter`. The real case-insensitive volume test (`fs_policy::case_and_normalization_variants`, an APFS disk image) runs on macOS only; Linux proves only the hook (`reported_name_checked`). The case-sensitive volume half of ADR-011's verification isn't built. | `crates/foreman-core/src/store/walk.rs`, `tests/fs_policy.rs` | ADR-011 (FS-reported name check, Verification). | ADR-011 changed. **Mac task:** run `cargo test -p foreman-core --test fs_policy` on macOS and record that the APFS test passed (Stage 5 packet). |
| 3 | Pinning doesn't refuse annotation errors. `lock::pin` refuses only a file that differs from HEAD or a re-pin after Stage 1 is signed, then pins whatever parses. No document requires refusal (AC-F001-15 says errors in an unused stage are listed and the project opens), but the Stage 1 exit criterion expects the P1 stages to parse with zero errors. | `crates/foreman-core/src/lock.rs` (`pin`) | DR-155 (states the as-built rule). **Decision for Billy:** should a pin also refuse errors in Stages 1–2 and F.2? CR stub below. | DR-155 changed. Open question. |
| 4 | Foreman's own decision change won't overwrite an unreviewed outside edit: `own_change` returns `OutsidePending` when the repo file is neither the approved nor the last committed content. SR-33 and ADR-022 §4 imply it; no rule said it for Foreman's own writes. The IPC error for it isn't in `docs/contracts/ipc.md`. | `crates/foreman-core/src/decisions.rs` (`own_change`) | DR-153 and ADR-022 §5 (the refusal). `docs/contracts/ipc.md` needs an error case on the commands that change decision files; that file belongs to the src-tauri/IPC work, so it is not edited here. | DR-153 and ADR-022 changed. **Follow-up:** add the ipc.md error mapping (M2-01). |
| 5 | From Stage 5 on, Foreman's own decision changes go to the approved copy and `progress.json` only; the repo file is left alone until the next signing commit. DR-153 read as if the repo file were written too, which contradicted its own drift rule (the repo file must still match `committedSha256`) and ADR-022 §5. | `decisions.rs` (`own_change`, `stage5_signed`) | **DR-153 now matches ADR-022 §5.** ADR-022's rule is the safer one: the working-tree file stays what Claude Code (and `git status`) sees as committed, nothing uncommitted can be swept into a commit that bypasses Foreman's signing, and drift needs no special case. | DR-153 changed. Billy confirms with the Stage 5 packet. |
| 6 | A CLI prompt step is done only on exit 0; a non-zero exit ends the run (code recorded) and returns the step to `notStarted`. DR-066 says a CLI step finishes "when the run exits (any code; the exit code is recorded)", and DR-065 says `running → done` on "exit 0" and `running → notStarted` when errored. The previous code made every exit code done. | `crates/foreman-core/src/steps/mod.rs` | Reading: DR-066's "any code" means the run is over and the code is recorded; DR-065 decides the step state. DR-066 should say "the run ends on any exit code (recorded); exit 0 finishes the step (DR-065)". | Code and tests fixed (`steps::finish_rules`, `steps::transitions_exhaustive` snapshot). DR-066 wording aligned with DR-065 (no behavior change); Billy confirms with the Stage 5 packet. |
| 7 | The real-index refresh runs with attributes neutralized and is skipped when `info/attributes` is non-empty. | `crates/foreman-sys/src/git/` | [CR-002](change-requests/CR-002-git-attributes-on-index-refresh.md) (Proposed, applied provisionally). | CR written. |

**CR stub (item 3), `CR-003 · Should a blueprint pin refuse annotation errors?`** Proposed, not yet written up. Today a pin succeeds with errors in any stage, and the steps in error are simply not runnable. Option A (keep): no code change; DR-155 says so. Option B: refuse while Stages 1–2 or F.2 (the P1 scope) have errors, showing the diagnostics, because a pinned blueprint with a broken P1 step can't be fixed without the v1.1 re-pin (re-pin closes when Stage 1 is signed). Recommendation: B for the P1 stages only. Touches DR-155, AC-F002-02, `blueprint_pin` Err list in ipc.md, ticket M1-17.

## Performance measurements still owed

| Target | Measured here | Task |
| ------ | ------------- | ---- |
| NFR-03 / AC-F001-07: a cached load of the 20,000-line blueprint ≤ 5 ms (uncached p95 ≤ 50 ms) | **7.4 ms** cached in the M1 run; a re-run here on 2026-10-20 (`--warm-up-time 1 --measurement-time 3`) gave 6.4 ms (interval 6.19 to 6.70 ms) cached and 19.8 ms uncached for the 20k file. Both on a shared 2-core Xeon @ 2.8 GHz Linux container, not a Mac; criterion `template_load_20k_cached` | **Mac task (NFR-03):** run `cargo bench -p foreman-core --bench template` on Billy's Mac with nothing else running, and record the mean and the criterion interval for `template_load_v2_2_uncached`, `template_load_20k_uncached` and `template_load_20k_cached` in the Stage 5 packet. The target stays 5 ms. If the Mac misses it, profile `cache::load_or_parse` before changing the target or the cache format. Its cached path normalizes and SHA-256 hashes the whole 20k-line source, reads the cache file through the store's policy walk, and deserializes the template JSON; those three steps are the suspects. |

The bench itself (`crates/foreman-core/benches/template.rs`) carries the same note, so a reader running it sees that the Linux figure is not the verdict.

## M2/M3 runner tickets (branch `stage/05-runners`: `foreman-sys`, `fake-claude`, pure runner modules in `foreman-core`)

The runner halves below are built and tested; each ticket's IPC command, native dialog, `run.json` and UI parts belong to `foreman-app` (`stage/05-app`), so the tickets stay `in-progress` until those land. APIs: design doc §3.2.3.

| Ticket | Status | Runner part done here | Left for the app |
| ------ | ------ | --------------------- | ---------------- |
| M2-06 | in-progress | argv builder, fixed flags, prompt last, `--name`; `argv::*`, `spawn::no_shell_in_any_code_path`, abuse argv | preview UI |
| M2-07 | in-progress | child env + PATH sanitizing; `env::*` | pass-through setting |
| M2-08 | in-progress | PTY/non-PTY lifecycle, no inherited fds, SIGHUP quit path | `lifecycle::crash_leaves_interrupted` (run table) |
| M2-09 | in-progress | run settings file (fresh, `0600`); `run_settings::*` | — |
| M2-11 | in-progress | `claude -v`, `auth status` readers (fail closed) | Doctor |
| M3-01 | in-progress | fingerprint over every source, stat cache, symlink rules, streamed `~/.claude.json`; `preflight::*_detected`, `fingerprint::*` | — |
| M3-02 | in-progress | redacted diff and `acknowledge` | banner, `config_ack` native dialog |
| M3-03 | in-progress | SR-07 blocks incl. `forceLoginMethod`; allowlist | `auth_key_allowlist_add`, `project_env_approve` |
| M3-04 | in-progress | auth in the step cwd, after the fingerprint, cached | — |
| M3-05 | in-progress | DR-120 order, version floor; `launch::progress_phase_order`, `compat::blocks_below_floor` | plan IDs, bench |
| M3-09 | in-progress | `PtySession` (env exact, no fds, back-pressure, live-only writes) | Channel adapter |
| M3-10 | in-progress | `start_session` | `launch_start`, tabs, `runlog::record_before_spawn` (PTY path) |
| M3-12 | in-progress | SR-28 paste sanitizer; `pty::paste_strips_escape_sequences` | `pty_paste`, links |
| M3-13 | in-progress | terminate (SIGTERM → SIGKILL), hangup | native dialogs, quit guard |
| M3-14 | in-progress | chain planning, S-3/S-8 branches, transcript path; `chain::*` | — |
| M3-15 | in-progress | re-arm machine + driver; `rearm::*` | `rearm_start`/`rearm_continue`, native idle dialog |
| M3-19 | in-progress | watcher (interest set, lstat roots, re-read by stamp); `watcher::*` | event mapping, `config_changed_marks_open_runs` |
| M4-09/M4-12 (sys part) | in-progress | sensor ingest, contract probes (probe B redesigned, ADR-017) | `probe_run`, Doctor rows, retention |
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
| M2-25 | todo | Launch preview (`launch_prepare` returns prompt-side blockers; the plan needs the argv builder and run settings) |
| M2-26 | in-progress | `step_mark_done` |

### Commands not built yet (IPC-R12), by ticket

| Ticket | Commands |
| ------ | -------- |
| M2-25, M3-01…M3-05 | `launch_prepare` (plan), `config_ack`, `auth_key_allowlist_add`, `project_env_approve` |
| M2-06 | `clipboard_copy { target: "command" }` |
| M3-10…M3-16 | `launch_start`, `chain_resume_prepare`, `pty_*`, `rearm_*`, `terminal_link_open`, `progress_rebuild_*` |
| M3-18, M4-12…M4-14 | `probe_folder_open`, `probe_run`, `doctor_capture_login_path`, `doctor_login_path_apply` |
| M3-21 | `packet_normalize_apply` (the DR-131 normalizer) |
| M3-23, M3-24 | `chat_copy_and_open`, `chat_link_set`, `paste_back_*` |
| M3-26, M4-02, M4-03 | `metrics_export`, `metrics_view`, `runs_clear`, `runs_search` (run records first) |
| M4-16…M4-33 | gates, signing, `tamper_restore`, lessons, manifest, assembler, desktop, `add_step`, `add_item`, `run_sheet_export`, `item_kind_confirm`, `terminal_open_here` |

### Open follow-ups

- `settings.json` v2 with `screenReaderMode` (core schema + migration; files.md §4.1 proposal).
- `project_remove` removes the registry entry only; deleting `projects/<id>/`, approvals and fingerprints needs a store delete (M4-02).
- `foreman-core::store::dirs::ensure_dir` (new, for setup's empty folders) needs a core review.
