# Stage 5 spikes (M0-12 to M0-18)

Seven spikes decide branches that the frozen spec specifies both ways (MASTER-SPEC §9, `docs/scope.md` "Spike decision rules"). This folder is the kit: one report per spike (this folder) and one runner per spike (`scripts/spikes/`). Run them on Billy's Mac. Each report ends with the branch taken and the documents that branch changes (AC-M0-13); M0-19 needs all seven.

## Prerequisites

- macOS, Terminal (not the Foreman terminal), Node 22.17 or newer, `pnpm install --frozen-lockfile` done.
- Claude Code 2.1.293 or newer (the ADR-017 floor), logged in with the subscription: `claude auth status` shows `authMethod` `claude.ai` or `oauth_token`. `pnpm spikes check` verifies this and prints what it found.
- No `ANTHROPIC_API_KEY` needed or wanted. The runners start `claude` with an allowlisted environment like SR-05 (no `ANTHROPIC_*`, `AWS_*`, `*_PROXY` ...), so a key in your shell cannot change billing; `pnpm spikes check` lists the variables it drops.
- S-0 only: the S-0 harness built and running (see "Harness requirements" in [S-0.md](S-0.md)). S-23 only: Claude Desktop installed and signed in to the same account.
- Claude Code asks once to trust each new folder. The runners use sandboxes under `$TMPDIR/foreman-spikes/`; accept the trust dialog when it appears (S-7′ asks you to do it deliberately).

## Safety rules every runner follows

- argv arrays only, never a shell string; `claude` only ever runs inside a sandbox folder with a marker file, never in a real project; it refuses a temp location that sits inside a repo.
- Never `--bare`, `-p`/`--print`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions` or `bypassPermissions` (the runner throws if asked).
- Zero tokens unless the runner says `TOKENS:` and you answer `y` to a y/N question. Input that is not a terminal counts as "no".
- Nothing under `~/.claude` or `~/.claude.json` is ever written. S-7′ reads sizes and symlink targets only (and one trust boolean).
- Every runner ends with `PASS`, `FAIL` or `INCONCLUSIVE`, the decision-rule branch it implies, the documents that branch changes, and a Markdown block to paste into the Results section of the report. Exit codes: 0, 1, 2.
- Run on anything but macOS and the verdict is labelled NOT DECISIVE.

## Order and time

| Order | Spike | Command | Time | Tokens | Report |
| --- | --- | --- | --- | --- | --- |
| 1 | S-0 PTY, xterm 6, real TUI, strict CSP | `pnpm spikes s0 fixtures` (helper), then the manual checklist | 60 to 90 min (30 of them waiting) | the TUI sessions you open | [S-0.md](S-0.md) |
| 2 | S-12 PATH in a Finder-launched build | `pnpm spikes s12` | 3 min | none | [S-12.md](S-12.md) |
| 3 | S-7′ settings `env` / `apiKeyHelper` | `pnpm spikes s7` (twice: untrusted, then `--trusted`) | 7 min | none | [S-7prime.md](S-7prime.md) |
| 4 | S-1′ per-launch hooks | `pnpm spikes s1` | 5 min | none for the probe, one tiny session for the deny checks | [S-1prime.md](S-1prime.md) |
| 5 | S-3 effort on resume | `pnpm spikes s3` | 5 min | two one-word sessions | [S-3.md](S-3.md) |
| 6 | S-8 graceful end, signals, forks | `pnpm spikes s8 all` | 20 min | up to seven one-word sessions | [S-8.md](S-8.md) |
| 7 | S-23 `--desktop --resume` | `pnpm spikes s23 prepare`, Desktop steps, `pnpm spikes s23 check` | 10 min | one tiny session and one Desktop message | [S-23.md](S-23.md) |

S-0 goes first because its fail branch can stop M1 and reopen the shell choice. The rest are independent: do S-12, S-7′, S-1′ and S-3 in a second terminal while S-0's 30-minute memory wait runs (they use `claude` processes outside the app, so they do not disturb the app's memory samples).

`pnpm spikes list` prints this table; `pnpm spikes check` runs the prerequisites only.

## Closing a spike

1. Paste the runner's block into the report's Results section and fill in the manual items.
2. Tick the branch taken and apply it: edit exactly the documents the report lists (the ADR, the affected ACs in `docs/scope.md`, tickets), as a normal change on this branch. Do not sign any packet.
3. Commit the report. When all seven reports exist, M0-19 can close Stage 5 (`docs/handoff/stage-05.md`).

## Rehearsal notes (zero-token parts, Linux container, Claude Code 2.1.304, 2026-10-20; not decisive)

The runners were exercised without spending tokens before this kit was handed over. Things worth knowing before the real run (re-check them on the Mac, do not rely on them):

- `claude --init-only` fires `SessionStart` with exec-form `command` + `args` hooks, including when the settings file, hook script and cwd all contain a space. The payload carries `session_id`, `transcript_path`, `cwd`, `source` (`startup`) and `session_title`; it does not carry `effort` (the hooks docs say `effort.level` appears on tool-use-context events such as `Stop`, `PreToolUse`, `PostToolUse`).
- `--init-only --resume <random uuid>` exited 0 with no output, so ADR-017's probe B ("exit ≠ 0 with a no-conversation message") cannot tell a parsed flag from an ignored one. `--init-only` also writes no transcript.
- `--resume <x> --session-id <y>` without `--fork-session` is refused by Claude Code ("`--session-id` can only be used with `--continue` or `--resume` if `--fork-session` is also specified"); with `--fork-session` it parses. That matches SR-14 and the "single allowed combination" test.
- `--effort bogus` only warns on stderr and exits 0: probe A cannot detect a rejected effort value from the exit code.
- `claude auth status` reacts to project settings even in an untrusted folder: a project `apiKeyHelper` flipped `authMethod` to `api_key_helper`, `CLAUDE_CODE_USE_BEDROCK`/`VERTEX` env flipped it to `third_party`, and `forceLoginMethod: "console"` (a key SR-07 does not name) added `forcedLoginMethod`. S-7′ prints these diffs; the `forceLoginMethod` case is a candidate miss for the fail branch.
