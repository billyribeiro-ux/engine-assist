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
