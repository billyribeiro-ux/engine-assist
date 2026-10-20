# ADR-017: Claude Code compatibility: a computed version floor checked every launch, zero-token contract probes on version change, and fake-claude contract tests

## Status
Proposed

## Date
2026-10-19

## Context

Foreman's correctness depends on Claude Code's CLI contract: flags, aliases, hook events and fields, `auth status` JSON, and session and transcript behavior. Claude Code ships often. npm `@anthropic-ai/claude-code` dist-tags on 2026-10-19 when this ADR was written: `stable` 2.1.292, `latest` 2.1.303, `next` 2.1.303 (the Stage 4.7 review corrected `next`). Re-checked in the 4.7 fix pass the same day (registry, after 2.1.304 was published at 19:47 UTC): `stable`, `latest` and `next` all 2.1.304. The tags move daily; Doctor shows the installed version, not a tag. The CLI reference says "`claude --help` does not list every flag".

MASTER-SPEC §6 gives the floors:

| Feature | Minimum version |
|---|---|
| Fable 5.1 | 2.1.257 |
| `--permission-prompts` | 2.1.259 |
| Opus 5.5 | 2.1.280 |
| `--agents` file form | 2.1.281 |
| Sonnet 5.5 | 2.1.284 |
| `--desktop` | 2.1.285 |
| Haiku 5.5 | 2.1.293 |

The effective floor is the maximum over the steps' models, flags and the pinned kit's agent models (**2.1.293** today; above `stable` when this ADR was written, per R-19, and below it after `stable` moved to 2.1.304). Other requirements:

- **SR-17:** probes on version change, only in `<app-data>/probe/` (trusted once, recreated `0700` with only Foreman's files before each run), 20 s timeout, outcomes pass / flag failure / inconclusive.
- **SR-19:** a confirmation on the first launch after a version change.
- **NFR-44:** probes use no tokens.

## Decision

1. **Floor:** `core::compat::floor(template, kit_agents, policy) -> (Version, Vec<Requirement>)` computes the floor from a data table of `{feature, minVersion, source}` (the table above plus `--ax-screen-reader` 2.1.181). `semver` 1.0.28 parses the leading semver of `claude -v` output (`2.1.211 (Claude Code)`).
   - Checked **on every launch** (no cache).
   - Below the floor: Run is disabled, with the capability table (feature → minimum → found → steps blocked) and **Run `claude update`**. If the floor is above npm `stable`, Doctor says so and recommends the `latest` channel.
2. **Contract probes (P2, F-019):** run when the observed version differs from `settings.json` `contract.version` (the last verified version; `docs/contracts/files.md` §4.1), before the next launch, with no tokens. `settings.json` `lastSeenClaudeVersion` (the last version any launch saw) drives the SR-19 "Claude Code updated X → Y" confirmation; `probeFolderTrusted` records the first-run trust step.
   - **Probe A:** `claude --init-only --settings <probe settings> --session-id <fresh uuid> --model <m> --effort <e> --permission-mode <mode> --name fm-probe` with a `SessionStart` hook (exec form, the sensor binary) that writes a marker. Pass = the marker exists **and** exit 0. That proves the hooks fire and every interactive flag Foreman passes still parses.
   - **Probe B:** `claude --init-only --resume <random uuid>`. Pass = exit ≠ 0 with a "no conversation found"-class message (the flag parsed).
   - **Classification:**
     - stderr matching `unknown option|unrecognized|invalid value` → **flag failure**, mapped to the named flag and disabling only the dependent feature;
     - timeout, trust prompt or anything else → **inconclusive**, which keeps the last verified state and warns.
   - **Folder:** `<app-data>/probe/` is recreated before each run (`0700`, only Foreman's files: probe settings, no `.claude/`, no `CLAUDE.md`). Billy trusts it once during first run (Foreman opens `claude` there in a terminal tab).
   - The result is stored as `contract { version, verifiedAt, outcomes { probeA, probeB } }` in `settings.json` (field names exactly as `docs/contracts/files.md` §4.1).
   - The probe records one real hook payload per event type (redacted) as a fixture for the sensor's derived-state tests (ADR-009). That settles the SessionEnd reason field name (UNVERIFIED in ADR-009).
3. **Version-change confirmation** (SR-19): the first launch after a change shows "Claude Code updated X → Y" in the confirmation.
4. **Fake-claude contract tests** pin argv, environment and run settings for every harness step (ADR-015), so Foreman-side regressions are caught in CI. Probes catch Claude-side changes on Billy's Mac.
5. **Watch list** (manual, per Stage Gate): the CLI reference, hooks, permissions and settings pages, re-read for the flags, events and keys Foreman uses. Changes lead to a floor-table update and an ADR amendment.

## Alternatives considered

- **Parse `claude --help`.** Incomplete by the docs' own statement, and its format isn't a contract.
- **A 24 h version cache.** Cheaper, but an update between launches would go unchecked (R-12).
- **Probes with `claude -p`.** Costs tokens and needs a hardened argv; v1 makes no `-p` calls (SR-16).
- **Pinning Claude Code to an exact version.** Billy uses `claude` outside Foreman too, and auto-update is Claude Code's own behavior. A floor plus probes tolerates drift instead of fighting it.

## Consequences

- A Claude Code release that renames a flag disables only the dependent feature (for example re-arm effort or Desktop handoff). Everything else still launches (NFR-13).
- The probe folder is a trusted Claude Code workspace that Foreman owns. It contains nothing a session could misuse.

## Verification

- `compat::floor_from_kit_agents`, `compat::blocks_below_floor`, `compat::parses_version_output`.
- `probe::runs_on_version_change`, `probe::runs_only_in_probe_dir`, `probe::dir_reset_before_run`, `probe::inconclusive_keeps_state`, `probe::failure_disables_feature` (fake-claude scripted outcomes).
- `preflight::confirm_required_for_version_change`.
- S-1′ runs probe A for real in Stage 5.3.

## Revisit when

- Claude Code publishes a machine-readable capabilities command or a stable CLI contract.
- `--init-only` changes semantics.

## Exit path

The floor table and the probe set are data. If `--init-only` disappears, probes become "inconclusive" (state kept), and floors plus Billy's confirmation carry compatibility until a replacement probe is designed.

## Amended 2026-10-20 (M3 runners): probe B redesigned

**Evidence** (Claude Code 2.1.304 on Linux, `env -i HOME=<tmp>`, zero tokens; the S-1′ rehearsal note in `docs/spikes/S-1prime.md`):

| argv | exit | stderr |
|---|---|---|
| `--init-only --resume <random uuid>` | 0 | (none) |
| `--init-only --resume not-a-uuid` | 0 | (none) |
| `--init-only --resume <r> --fork-session --session-id <n>` | 0 | (none) |
| `--init-only --resume <r> --session-id <n>` | 1 | `Error: --session-id can only be used with --continue or --resume if --fork-session is also specified.` |
| `--init-only --bogus-flag` | 1 | `error: unknown option '--bogus-flag'` |
| `--init-only --permission-mode nope` | 1 | `error: option '--permission-mode <mode>' argument 'nope' is invalid. Allowed choices are …` |
| `--init-only --effort bogus` | **0** | `Warning: Unknown --effort value 'bogus' — ignoring it and using the default effort. …` |
| `--init-only --settings /nonexistent.json` | 1 | `Error: Settings file not found: …` |

So the original probe B criterion ("exit ≠ 0 with a no-conversation-found message") can never pass, and exit codes alone can't tell a parsed flag from an ignored one; a bad `--effort` exits 0.

**Decision.**

- **Probe B** is two runs: **B1** `--init-only --resume <r> --fork-session --session-id <n>` (Foreman's fork form) must exit 0 with no flag error; **B2** `--init-only --resume <r> --session-id <n>` (SR-14's forbidden form) must be **refused**: exit ≠ 0 with a message naming `--session-id` and `--resume`/`--fork-session`. B2's refusal proves the parser read `--resume` and still enforces the combination Foreman relies on. B1 refused → flag failure (`--fork-session`, or `--resume`); B2 accepted → inconclusive (state kept).
- **Flag-failure classification** reads stderr line by line for `unknown option`, `unrecognized`, `is invalid`, `invalid value` and `Unknown --<flag> value`, and names the first `--flag` on that line. Probe A is a flag failure on such a line **even with exit 0** (the `--effort` warning case).
- **Probe A's marker** is the `SessionStart` event file the hash-checked sensor writes to `events/<probeRunId>/` with the probe's token (files.md §4.11 amended); probe A passes only with exit 0, no flag error and the marker.
- Code: `foreman_core::probe` (`classify_a`, `classify_b`, `disabled_features`, `due`, `merge`), `foreman_core::launch::argv::{probe_a, probe_b}`, `foreman_sys::claude::run_probes`, `foreman_sys::launch::{probe_settings, reset_probe_dir, probe_marker}`. Tests: `probe::*` in `crates/foreman-core/tests/launch.rs` and `crates/foreman-sys/tests/probe.rs` (fake-claude mirrors the table above).
- Billy's Mac run of S-1′ still records the macOS behavior; if it differs, this table and the patterns are updated.
