# CR-002 · Neutralize git attributes on the real-index refresh, and skip it when `info/attributes` is non-empty

Status: PROPOSED. Applied provisionally in M1 (`foreman-sys::git`), awaiting Billy's decision.
Requested by: the M1 build agent, found while building `git::status_and_diff_never_run_filters` (ADR-012 amendment 2026-10-20). Written 2026-10-20.
Decided by: Billy Ribeiro (pending). Never sign on his behalf.

## What changes

Two behaviors in the last step of a Foreman decision commit (the refresh of the repo's **real** index, after `update-ref`):

1. **Attributes are disabled for that one call.** `git update-index --cacheinfo …` runs with `-c core.attributesFile=/dev/null` (after the fixed SR-44/SR-45 `-c` overrides) and `GIT_ATTR_SOURCE=4b825dc642cb6eb9a060e54bf8d69288fbee4904` (the empty tree) in its environment (`GitOpts::neutralize_attributes`, `crates/foreman-sys/src/git/mod.rs`). No other git call gets either.
2. **The refresh is skipped when `$GIT_DIR/info/attributes` is non-empty.** Neither setting above covers that file. Foreman finds it with `rev-parse --git-path info/attributes`, checks its size, and if it has content it makes no refresh attempt at all. The commit stands, `progress.json` records `indexRefreshPending`, and the existing "index refresh pending" banner tells Billy. A later project open or focus retries, and it stays pending while the file is non-empty (`crates/foreman-sys/src/git/commit.rs`).

This adds one environment name and one `-c` key to AC-XC-04's exact lists, for that single call.

## Why (security)

T-040 (threat model): a repo's `.gitattributes` can name a clean-filter driver, and a hostile repo can define its command in config or ship one through `info/attributes`. SR-45's `--no-filters` covers `hash-object` only. Foreman never wants a repo-controlled command to run (SR-44/SR-45, SR-01).

Evidence (ADR-012 amendment): with a racily-clean index entry, `update-index --cacheinfo` on the real index hashes that entry "as it would be added" when git writes the index, which applies worktree filter drivers. Reproduced with git 2.43.0: the clean filter fired. The test `git::status_and_diff_never_run_filters` (marker files from a hostile repo fixture) covers both the neutralized path and the `info/attributes` skip. Foreman's own commits go through a temporary index with plumbing, so the real-index refresh is the only place this reaches.

The refresh exists so that `git status` in Billy's checkout doesn't show the committed decision files as changed after Foreman moved the ref behind the index's back.

## Behavior and residual risk

- **git ≥ 2.40:** `GIT_ATTR_SOURCE` takes effect; the filter no longer runs (verified on 2.43.0 only).
- **Apple's `/usr/bin/git` 2.39.x:** `GIT_ATTR_SOURCE` is ignored, so a racily-clean entry could still run a worktree filter during the refresh. **UNVERIFIED on Billy's Mac**: the 2.39 behavior is from the version history, not run here. Doctor should recommend Homebrew git ≥ 2.40, or the refresh should be opt-in.
- **`info/attributes` repos:** the index stays stale until Billy removes the file or refreshes it himself (for example `git status` in a shell, which is his choice and outside Foreman). The cost is a persistent banner.
- Both paths fail safe: the worst case is a stale index or a reported pending refresh, never a lost commit.

## Alternatives

1. **Refresh with no attribute neutralizing.** Simplest, and it runs repo-controlled filter commands. Rejected: T-040.
2. **Never refresh the real index.** No filter risk, but every Foreman commit leaves the checkout's `git status` out of step until Billy refreshes. Rejected as the default; it is the fallback if Billy prefers zero risk over a tidy status.
3. **Opt-in refresh** (setting, off by default, Doctor explains it). Removes the 2.39 residual for anyone who doesn't ask. Reasonable if Billy keeps Apple git.
4. **Avoid the racy window** by waiting until the clock passes the newest entry mtime before the index is written, so git has nothing to re-hash. Removes the cause instead of masking it, but depends on filesystem timestamp granularity and the other entries in the index. Not verified here; needs a macOS APFS test.
5. **Require git ≥ 2.40** and refuse to refresh below it, with Doctor showing the reason. Closes the 2.39 gap; Billy installs Homebrew git.

## Impact

- Scope / feature catalog: F-014 (signing) behavior unchanged on the happy path.
- ADRs: ADR-012 amended 2026-10-20 (this CR is referenced there). ADR-007 unaffected.
- Requirements and criteria: SR-44/SR-45 and AC-XC-04's exact `-c`/environment lists gain the two items above for this call only.
- Threat model: T-040 residual: Low on git ≥ 2.40, Medium on 2.39 (already in the T-040 row).
- Data model: none (`indexRefreshPending` exists in `progress.json`, files.md).
- Timeline and cost: none.

## Recommendation

APPROVE the current behavior as the default, and MODIFY with alternative 5 or 3 for the 2.39 gap (Doctor warns on git < 2.40; refresh only when git ≥ 2.40). If approved, `docs/scope.md` AC-XC-04 and the requirements text for SR-44/SR-45 get the two list entries.

## Deltas for Stage 4 (filled in when approved)

- docs/scope.md: AC-XC-04's exact `-c` and environment lists: add `core.attributesFile=/dev/null` and `GIT_ATTR_SOURCE=<empty tree>` for `update-index --cacheinfo` on the real index only.
- docs/requirements.md: SR-44, SR-45 (same entries).
- docs/adr/: ADR-012 (already amended; set Status wording to match the decision).
- docs/threat-model.md: T-040 row (cite the decision).
- docs/contracts/files.md: none.
