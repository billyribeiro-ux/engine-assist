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
