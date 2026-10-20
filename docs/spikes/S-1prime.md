# S-1′ · Per-launch hooks as a contract probe (M0-13)

Class HARD · manual (Billy on his Mac) · ADRs 009, 021 (and 017's probe A) · threats T-024, T-026 · ACs AC-M0-13 (S-1′), SR-09, SR-17, AC-F011-*, AC-F010-14 · required test `run_settings::deny_case_variant_probe` (manual, recorded here).

**Question.** Does `claude --init-only --settings <file with exec-form hooks>` fire `SessionStart` in a probe folder? Two side questions belong to the same spike: do the run settings' deny rules `Read(//<repo>/.foreman/**)` also deny the case variant `.FOREMAN/`, and do `//` deny rules still parse and deny when the repo path contains a space (for example `~/Developer/Trade Log`)?

## Run

```bash
pnpm spikes s1                 # zero-token probes, then a y/N for one short session (the deny checks)
pnpm spikes s1 --no-tokens     # probes only
```

What it does, in a sandbox under `$TMPDIR/foreman-spikes/s1`:

1. **Probe A** (zero tokens): `claude --init-only --settings <tmp settings> --session-id <uuid> --model opus --effort high --permission-mode plan --name fm-probe` with a `SessionStart` hook in exec form (`command` = node, `args` = [logger, log file, event]). Pass = exit 0 and the marker line exists.
2. **Negative control**: the same with `disableAllHooks: true`; the marker must be absent, otherwise the probe proves nothing.
3. **Spaced paths**: probe A again with the settings file, the hook script and the probe folder all under paths containing a space.
4. **Probe B** (ADR-017): `--init-only --resume <random uuid>`; recorded as information (see Notes).
5. **Deny checks** (one tiny plan-mode session, after your y/N): a throwaway repo at `…/Trade Log repo` with a canary in `.foreman/secret.txt`; run settings deny `Read(//<repo>/.foreman/**)` and `Edit(…)`; Claude is asked to Read `…/.foreman/secret.txt` and `…/.FOREMAN/secret.txt`. A read that came back with the canary means the rule did not apply. If the lower-case control is not denied in the spaced path, it repeats in a path without a space to isolate the cause. When Claude has answered, type `/exit`. Accept the trust dialog if asked.

The hook logger records every payload as JSON lines. Only `SessionStart` can fire under `--init-only`; the other sensor events (`Stop`, `SessionEnd`, `PermissionRequest`, ...) are logged by the real sessions in the deny check, S-3 and S-8: copy their event lists into Results.

## Pass and fail

| Check | Pass | Fail |
| --- | --- | --- |
| Hooks fire (probe A + negative control) | marker present, exit 0, session id honored; control has no marker | exit ≠ 0 with a flag error, or exit 0 and no marker |
| Case variant | `.FOREMAN/` read denied too | canary returned through `.FOREMAN/` (APFS is case-insensitive, so the file is the same) |
| Space in repo path | lower-case control denied in `Trade Log repo` | allowed there but denied without a space |

Manual deny check (if the runner could not do it): in a repo you do not mind, put `{"permissions":{"deny":["Read(//<abs repo>/.foreman/**)"]}}` in a file, create `.foreman/secret.txt`, start `claude --settings <file>`, ask it to read the file by its lower-case path and by `.FOREMAN/…`, and note which attempts say "denied". Remember the rule starts `//` followed by the absolute path **without** its leading slash, i.e. `/` + `/Users/…`.

## Decision rule

**Pass (hooks fire):** the sensor ships in P2 (M4-07 to M4-10 stay). Documents changed: `docs/adr/ADR-009-session-sensor-per-launch-hooks.md` (Status, S-1′ result, SessionEnd reason field name if a real payload was seen), `docs/adr/ADR-017-claude-code-version-floor-and-contract-probes.md` (probe A confirmed; probe B note below).

**Fail (hooks do not fire):** no sensor; the manual idle confirmation stays. Documents changed: `docs/scope.md` (remove AC-F011-* and the sensor part of AC-F010-14), `docs/tickets.md` (cancel M4-07 to M4-10 except the P1 fallbacks), ADR-009 (Status Rejected, exit path taken), `docs/requirements.md` SR-15/SR-17/SR-18 wording that mentions the sensor.

**Side results, independent of the branch above:**

- Case variant not denied → list the variants explicitly in the run settings (M2-09); `docs/adr/ADR-021-launch-preflight-config-fingerprint.md` and `docs/requirements.md` SR-09 (replace "if not, the variants are listed").
- Spaced path not denied → record the escaping rule in ADR-021 and encode it in M2-09 (`run_settings::deny_paths_absolute` already lists the spaced-path case).

## Results (fill in)

Run date: ____ · macOS: ____ · Claude Code: ____ · runner verdict: ____

(Paste the runner's block here.)

| Item | Result | Evidence |
| --- | --- | --- |
| Probe A, plain paths | | |
| Negative control | | |
| Probe A, paths with spaces | | |
| Probe B behavior | | |
| Deny: lower case, spaced path | | |
| Deny: `.FOREMAN/` | | |
| Events seen in real sessions | | |
| SessionEnd reason field name (from a real payload) | | |

Branch taken: [ ] pass (sensor ships in P2) · [ ] fail (no sensor)
Side results: case variant [ ] denied [ ] not denied · spaced path [ ] denied [ ] not denied

Documents changed (file and section): ____________

Notes:

- Probe B in the 2026-10-20 Linux rehearsal exited 0 with no output for an unknown UUID, so it cannot separate "parsed" from "ignored". If the Mac does the same, ADR-017 needs a probe-B criterion that works (for example only flag-parse errors on stderr), recorded here: ____________
