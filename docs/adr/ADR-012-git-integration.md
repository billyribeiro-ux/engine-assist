# ADR-012: Git through one `git::run` entry point over the git CLI; all commits by plumbing, with read-back

## Status
Proposed

## Date
2026-10-19

## Context

Foreman uses git for:

- `git init` at setup;
- blueprint lock restore (F-002);
- the dirty-tree check at pin;
- `git config user.name` for the DRI default;
- P1 pathspec commits of decision changes before Stage 5;
- signing (P2, F-014): current branch or another branch, then read-back;
- tamper detection;
- locating `info/exclude` (SR-32).

A repo's `.git/config` is untrusted input: it can define filter drivers that `git add` executes, and a `gpg.program` that `git commit` executes, even with hooks disabled.

The revised `docs/requirements.md` (after the security review) fixes:

- **SR-44:** one `git::run` entry point, **built in Stage 6**, for every git use; no repo hook ever runs.
- **SR-45:**
  - environment `env_clear()` plus `HOME`, a fixed `PATH`, `LANG`, `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0` (`GIT_INDEX_FILE` only for Foreman's temporary index);
  - always `-c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat -c core.editor=true -c sequence.editor=true -c commit.gpgSign=false -c tag.gpgSign=false -c core.sshCommand=false -c credential.helper= -c protocol.allow=never`;
  - no `status`, `diff`, `add` or `commit` at all (plumbing only);
  - blobs only via `hash-object -w --no-filters`;
  - commits always `commit-tree --no-gpg-sign` in v1;
  - a 60 s timeout.
- **SR-42:** current-branch signing by **plumbing**: temporary index from HEAD; `hash-object -w --no-filters` per path; `update-index --cacheinfo`; `write-tree`; `commit-tree --no-gpg-sign`; `update-ref HEAD <new> <old>`; then `update-index --cacheinfo` on the real index for those paths only. An unborn HEAD gets a parentless first commit; a detached HEAD is refused. P1 decision-file commits follow the same rules.
- **SR-43:** another branch, by the same plumbing against that branch's tip, never touching HEAD, the index or the working tree.

Library check, 2026-10-19:

- `git2` 0.21.0: default features `[]`; `https` pulls `openssl-sys`; `ssh` pulls libssh2.
- `gix` 0.89.0: MSRV 1.88; network only behind `blocking-network-client`.

## Decision

1. **`foreman-sys::git::run(repo: &Root, args: GitArgs, opts: GitOpts) -> Result<GitOutput, GitError>`** is the only function that spawns git (clippy `disallowed-methods` plus `git::single_entry_point`). `GitArgs` is built by pure functions in `foreman-core::git` from a closed enum of operations that is exactly domain-rules DR-140's list:
   - `Init`;
   - `RevParse(ShowToplevel | GitPath(path) | GitCommonDir | VerifyHead | BlobAt{rev, path})` (`--show-toplevel`, `--git-path`, `--git-common-dir`, `--verify -q HEAD`, `<rev>:<path>`);
   - `SymbolicRefHead` (`symbolic-ref -q HEAD`);
   - `ConfigGet{key, show_origin_and_scope: bool}` (`config --get`, optionally `--show-origin --show-scope`);
   - `CatFileBlob{rev, path}`, `LsTree{rev, path}`, `ShowBlob{rev, path}` (raw);
   - `HashObjectNoFilters{write: bool}` (`hash-object [-w] --no-filters --stdin`);
   - `UpdateIndexCacheInfo{entries}` (`update-index --add --cacheinfo`);
   - `ReadTree(Tip(oid) | Empty)` (temporary index only: the builder refuses it without Foreman's `GIT_INDEX_FILE`);
   - `WriteTree`, `CommitTree{tree, parent: Option<oid>, msg}` (always `--no-gpg-sign`), `UpdateRef{ref, new, old}`.

   There is no `Status`, `Diff`, `Add`, `Commit` or `LsFiles` operation, and no free-form argv (SR-45). `git::status_and_diff_never_run_filters` checks the enum against DR-140's list.
2. **Invocation**, exactly SR-45:
   ```text
   <abs git> -C <root> -c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat
     -c core.editor=true -c sequence.editor=true -c commit.gpgSign=false -c tag.gpgSign=false
     -c core.sshCommand=false -c credential.helper= -c protocol.allow=never <op args>
   ```
   - environment: `env_clear()`, then `HOME`, a fixed `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin` plus the git binary's own directory), `LANG=C`, `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, and `GIT_INDEX_FILE` only for Foreman's temporary index (created in app data, `0600`);
   - stdin: a pipe used only for `hash-object --stdin`, otherwise null;
   - 60 s timeout; output capped at 8 MB;
   - plumbing output formats only.
3. **HEAD state first** (current branch only). `symbolic-ref -q HEAD` exits 1 on a detached HEAD: Foreman refuses with "Check out a branch first", and decision changes stay pending (nothing is lost). `rev-parse --verify -q HEAD` exits 1 on an unborn branch (a fresh `git init`): the commit is the repo's first one (step 4 below uses `read-tree --empty`, `commit-tree` without `-p`, and `update-ref HEAD <new> 0000000000000000000000000000000000000000`, so a concurrent first commit fails cleanly). Verified with git 2.43.0 on 2026-10-19: on an unborn branch `symbolic-ref -q HEAD` prints the branch and `update-ref HEAD <new> <zero oid>` creates it through the symref; with the branch already present it fails ("reference already exists").
4. **Every commit is plumbing** (current branch, another branch, and P1 decision-file commits):
   1. Re-read each path through `store` and compare with the reviewed hash (SR-42).
   2. Dirty check per path: `hash-object --no-filters` of the bytes ≠ `rev-parse HEAD:<path>` (missing at HEAD counts as changed, DR-142). Only changed paths are committed.
   3. `hash-object -w --no-filters --stdin` per file, with bytes from memory. No repo filter driver can run.
   4. `GIT_INDEX_FILE=<tmp> read-tree <tip>` (unborn: `read-tree --empty`), then `update-index --add --cacheinfo 100644,<oid>,<path>` per path, then `write-tree`.
   5. `commit-tree <tree> -p <tip> --no-gpg-sign -m "handoff: stage NN"` (unborn: no `-p`). **v1 never signs commits cryptographically**, whatever the global, system or repo configuration says; Doctor shows a notice when `commit.gpgSign` is `true` at any scope ("Foreman's commits aren't GPG-signed in v1").
   6. `update-ref <HEAD or refs/heads/branch> <new> <old>` (old value given, the zero OID for an unborn branch, so a concurrent move fails cleanly).
   7. Current branch only: `update-index --cacheinfo` on the **real** index for exactly those paths, so Billy's own `git status` shows them clean. Billy's other staged changes are untouched. If `index.lock` exists, Foreman retries 5 times, 200 ms apart. If it still fails, the commit stands (the ref already moved) and `progress.json` records `indexRefreshPending { commit, paths, since }` (`docs/contracts/files.md` §4.3); Foreman retries on project open and window focus, and shows the banner "Index not refreshed; don't commit until this clears" until a retry succeeds.
5. **Read-back:** `cat-file blob <commit>:<path>` for every path, with SHA-256 compared against what Foreman wrote. `cat-file` applies no textconv or filters. A mismatch shows the diff and keeps the stage in `signed (commit pending)`.
6. **Reads:** lock restore (`cat-file blob <commit>:<path>`), the blueprint dirty check (`hash-object --no-filters <file>` vs `rev-parse HEAD:<path>`, DR-142; never `git status`), `rev-parse --git-path info/exclude` and `--git-common-dir` (SR-32), `config --get user.name`. All go through `git::run`.
7. **Where the commit OID lives.** The stage record (`.foreman/stages/stage-NN.json`) is inside the commit it would describe, so it can't hold that commit's OID. The record notes the overrides and the sign mode (`none`); the OID is stored in app data, `progress.json` `signing[NN].commit` (`docs/contracts/files.md` §4.3), and the record is found later with `ls-tree`/`cat-file` at that commit.

## Alternatives considered

- **`git add -- <paths>` then `git commit --only -- <paths>`** (the frozen MASTER-SPEC §8 before the Stage 2 sign). Simple porcelain, but `git add` runs repo-configured clean filters, and `git commit` honors repo-local `gpg.program`. Hooks off doesn't stop either. The security review replaced it with plumbing (SR-42); MASTER-SPEC now says plumbing.
- **Honoring global/system signing config with `-S`.** Rejected for v1 (Stage 4.7 decision): scope checking every `gpg.*` key is fiddly to get right, a GUI pinentry can hang a click-driven action, and nothing in v1 needs signed commits. Revisit in v1.1 if Billy wants them.
- **`git2` 0.21.0 (libgit2).** In-process, but C code linked into Foreman (plus OpenSSL/libssh2 if network features creep in, and banned by ADR-016). Its attribute, filter and signing behavior differs from the git CLI used in the same repos, and the spec's guarantees are phrased as CLI flags auditable in argv.
- **`gix` 0.89.0** (pure Rust). Safe and promising, with plumbing equivalents. But it's pre-1.0 with fast-moving APIs, and mixing gix-written objects and refs with CLI git in one repo risks subtle divergence. A candidate for read-only operations later.

## Consequences

- Signing is about 8–12 short git processes, which is fine for a click-driven action.
- Because commits bypass `git add`, `.gitattributes` text conversion (for example `eol=crlf`) is **not** applied. Foreman writes LF and commits the exact bytes it wrote, which is what read-back verifies.
- Foreman's commits are never GPG-signed in v1, even if Billy signs commits globally; Doctor says so.
- A failed real-index refresh leaves the commit correct but `git status` stale for those paths until a retry clears it; the banner warns Billy not to commit meanwhile.

## Verification

Fixture repos and tests:

- `git::single_entry_point` (clippy `disallowed-methods`), `git::invocation_snapshot`, `git::env_cleared`, `git::timeout`, `git::no_hooks_on_any_command` (repo with local `core.hooksPath` and lint-staged);
- `git::filter_driver_never_runs` (a local `filter.x.clean` that would touch a marker file);
- `git::repo_gpg_program_never_runs` (local `gpg.program` + `commit.gpgSign=true`);
- `sign::refuses_changed_file`, `sign::pathspec_only` (unrelated staged changes survive and aren't committed), `sign::untracked_new_packet`, `sign::readback_detects_mismatch`, `sign::other_branch_leaves_worktree_head_and_index`, `sign::concurrent_ref_move_fails`;
- `sign::unborn_head_first_commit` (a fresh `git init`: parentless commit, branch created), `sign::detached_head_refused` (refused, decision changes still pending), `sign::index_locked_refresh_retried` (a held `index.lock`: 5 retries, then `indexRefreshPending` and the banner; a later focus clears it);
- `lock::restore_from_git`.

## Revisit when

- gix reaches 1.0 with plumbing parity.
- git changes plumbing semantics for `commit-tree`/`update-ref` (checked when Doctor sees a new git major).

## Exit path

`git::run` takes a closed operation enum, so a gix-backed implementation can replace operations one at a time behind the same enum, verified against the same fixtures.

## Amended 2026-10-20 (M1-08, M1-10): the real-index refresh can run a clean filter

Building `git::status_and_diff_never_run_filters` with a racily-clean index showed that step 4.7 (`update-index --cacheinfo` on the **real** index) also hashes racily-clean entries "as they would be added" when git writes the index, so a worktree `.gitattributes` filter driver runs (reproduced with git 2.43.0: the clean filter fired). This is T-040's mechanism reaching a plumbing call. Following T-040's proposed mitigation, that one call (and only it) now runs with attributes neutralized: `-c core.attributesFile=/dev/null` after the SR-45 overrides and `GIT_ATTR_SOURCE=4b825dc642cb6eb9a060e54bf8d69288fbee4904` (the empty tree) in its environment. Verified with git 2.43.0: the filter no longer runs. `$GIT_DIR/info/attributes` isn't covered by `GIT_ATTR_SOURCE`, so when that file is non-empty Foreman skips the refresh and records `indexRefreshPending` (the existing banner). **Residual, UNVERIFIED on Billy's Mac:** `GIT_ATTR_SOURCE` needs git ≥ 2.40; Apple's `/usr/bin/git` 2.39.x ignores it, so a racily-clean entry could still run a worktree filter during the refresh there. Doctor should recommend Homebrew git ≥ 2.40, or the refresh can be made opt-in. This adds one env name and one `-c` key to AC-XC-04's exact lists for that call only: **needs Billy's Change Request approval**.

Implementation note: `foreman-sys::spawn` uses `std::process::Command` with `process_group(0)`, a polling wait with the timeout and capped reader threads (synchronous), not `tokio::process` (ADR-007 §6); the app wraps calls in a blocking task. Behavior is the same (own process group, `env_clear`, null stdin, timeout, cap).
