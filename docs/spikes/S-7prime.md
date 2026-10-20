# S-7′ · Settings `env` and `apiKeyHelper` in a trusted repo (M0-15)

Class HARD · manual (Billy on his Mac) · ADR-021 · threat T-020 · ACs AC-M0-13 (S-7′), AC-F008-07 · rules SR-06, SR-07, SR-08 · encoded later by `preflight::blocks_<key>` (M3-03).

**Question.** A repo Billy has trusted can set `env` and `apiKeyHelper` in `.claude/settings.json`. When the fingerprint (SR-06) and block rules (SR-07) are applied by hand to it, do they catch **every** input that changes how the session is billed or authenticated? Second part: how big are Billy's real `~/.claude` and `~/.claude.json` (caps 256 KB and 64 MB) and which fingerprint sources are symlinks (ADR-021's symlink rule)?

## Run (zero tokens)

```bash
pnpm spikes s7                    # 1) case matrix in an UNtrusted fixture, 2) inventory of ~/.claude
cd "$TMPDIR/foreman-spikes/s7/fixture-repo" && claude     # accept the trust dialog, then /exit (zero tokens)
pnpm spikes s7 --trusted          # the real run: same matrix, fixture now trusted
pnpm spikes s7 --prepare c03      # optional: write one case into the fixture to inspect by hand
pnpm spikes s7 --inventory-only   # only the ~/.claude inventory
```

The fixture is a git repo at `$TMPDIR/foreman-spikes/s7/fixture-repo` (the path is stable on purpose: Claude Code records trust by path). For each case the runner writes `.claude/settings.json` (or `settings.local.json`), runs `claude auth status` in that folder with an SR-05 style environment, compares the JSON with the no-settings baseline, and applies SR-07 as written in `docs/requirements.md` by hand (its own small scanner). Fake secrets are placeholders and the base URL and proxy point at a closed local port, so nothing leaves the machine. Nothing under `~/.claude` is written.

Cases: c00 baseline · c01/c02 `apiKeyHelper` (settings, settings.local) · c03 `ANTHROPIC_API_KEY` · c04 `ANTHROPIC_AUTH_TOKEN` · c05 `ANTHROPIC_BASE_URL` · c06/c07 `CLAUDE_CODE_USE_BEDROCK`/`VERTEX` · c08 `CLAUDE_CODE_OAUTH_TOKEN` · c09 `CLAUDE_CONFIG_DIR` · c10 `HTTPS_PROXY` · c11 `AWS_PROFILE` · c12 `awsAuthRefresh` · candidates SR-07 does not name: c13 `forceLoginMethod: "console"`, c14 `env.HOME`, c15 `env.XDG_CONFIG_HOME`, c16 a benign `env` key.

## Manual check (2 minutes, because `auth status` may not load all project settings)

For c03 (`--prepare c03`) and c13 (`--prepare c13`): `cd` into the fixture, run `claude`, type `/status` and read the auth and account lines, then `/exit`. Record whether the TUI shows an API key / Console login instead of the subscription. This is zero-token as long as you send no prompt. A case whose TUI line changes but is not blocked by SR-07 is a miss.

## Pass and fail

- **Pass:** every billing-relevant input shown by `auth status` or `/status` is blocked by SR-07 as written (and SR-08's `authMethod` check would catch the rest): the fingerprint and block rules suffice.
- **Fail:** any input that changes billing or auth is not named by SR-07 (the runner prints the field-level diff, for example `forcedLoginMethod: undefined -> "console"`).
- Inventory (reported, not part of the branch): `~/.claude.json` against 64 MB; any fingerprint file against 256 KB; every symlink with its target. Under ADR-021 a symlink is followed only when its target is a regular file outside every registered project; a symlink to a directory, a loop or a non-regular file blocks the launch.

## Decision rule

| Branch | Condition | Documents changed |
| --- | --- | --- |
| Pass | rules catch everything | ADR-021 (S-7′ result; caps and symlink rule checked against this machine); `docs/scope.md` AC-F008-07 stays inactive |
| Fail | an input is missed | AC-F008-07 is kept and built in M3-03: any project `env` block also blocks the launch until Billy approves it (`docs/scope.md` AC-F008-07, `docs/adr/ADR-021` Exit path, `docs/requirements.md` SR-07 for any missed non-env key such as `forceLoginMethod`, `preflight::blocks_<key>` tests in M3-03) |

If the inventory shows a cap exceeded or a directory symlink: ADR-021 caps or symlink rule change (for example resolve directory symlinks that stay outside projects); record it in ADR-021 and `docs/contracts/files.md` if the fingerprint cache format changes.

## Results (fill in)

Run date: ____ · macOS: ____ · Claude Code: ____ · fixture trusted: [ ] yes [ ] no · runner verdict: ____

(Paste the runner's block here.)

| Case | `auth status` changed? (diff) | Blocked by SR-07? | `/status` in the TUI (manual) |
| --- | --- | --- | --- |
| c01 apiKeyHelper | | | |
| c03 ANTHROPIC_API_KEY | | | |
| c06 CLAUDE_CODE_USE_BEDROCK | | | |
| c09 CLAUDE_CONFIG_DIR | | | |
| c13 forceLoginMethod | | | |
| c14 env.HOME | | | |

Project `apiKeyHelper` executed by `auth status` in a trusted folder? ____

Inventory: `~/.claude.json` ____ MB (cap 64) · largest fingerprint file ____ KB (cap 256) · files over cap: ____
Symlinks (link -> target):
- ____

Branch taken: [ ] pass · [ ] fail

Documents changed (file and section): ____________

Notes: ____________
