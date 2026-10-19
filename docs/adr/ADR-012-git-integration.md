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
  - diffs `--no-ext-diff --no-textconv`; status `--ignore-submodules=all`;
  - blobs only via `hash-object -w --no-filters`;
  - signing settings honored only from global/system scope;
  - a 60 s timeout.
- **SR-42:** current-branch signing by **plumbing**: temporary index from HEAD; `hash-object -w --no-filters` per path; `update-index --cacheinfo`; `write-tree`; `commit-tree --no-gpg-sign`; `update-ref HEAD <new> <old>`; then `update-index --cacheinfo` on the real index for those paths only. P1 decision-file commits follow the same rules.
- **SR-43:** another branch, by the same plumbing against that branch's tip, never touching HEAD, the index or the working tree.

Library check, 2026-10-19:

- `git2` 0.21.0: default features `[]`; `https` pulls `openssl-sys`; `ssh` pulls libssh2.
- `gix` 0.89.0: MSRV 1.88; network only behind `blocking-network-client`.

## Decision

1. **`foreman-sys::git::run(repo: &Root, args: GitArgs, opts: GitOpts) -> Result<GitOutput, GitError>`** is the only function that spawns git (clippy `disallowed-methods` plus `git::single_entry_point`). `GitArgs` is built by pure functions in `foreman-core::git` from a closed enum of operations:
   - `Init`, `ConfigGet{key, show_origin}`, `RevParseGitPath`, `Status`, `CatFileBlob{rev, path}`, `HashObjectNoFilters`, `ReadTree`, `UpdateIndexCacheInfo`, `WriteTree`, `CommitTree{tree, parent, msg, sign: SignMode}`, `UpdateRef{ref, new, old}`, `DiffNoExt`, `LsFiles`.

   No free-form argv.
2. **Invocation**, exactly SR-45:
   ```text
   <abs git> -C <root> -c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat
     -c core.editor=true -c sequence.editor=true -c commit.gpgSign=false -c tag.gpgSign=false
     -c core.sshCommand=false -c credential.helper= -c protocol.allow=never <op args>
   ```
   - environment: `env_clear()`, then `HOME`, a fixed `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin` plus the git binary's own directory), `LANG=C`, `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, and `GIT_INDEX_FILE` only for Foreman's temporary index (created in app data, `0600`);
   - stdin: a pipe used only for `hash-object --stdin`, otherwise null;
   - 60 s timeout; output capped at 8 MB;
   - `-z` porcelain or plumbing formats only.
3. **Every commit is plumbing** (current branch, another branch, and P1 decision-file commits):
   1. Re-read each path through `store` and compare with the reviewed hash (SR-42).
   2. `hash-object -w --no-filters --stdin` per file, with bytes from memory. No repo filter driver can run.
   3. `GIT_INDEX_FILE=<tmp> read-tree <tip>`, then `update-index --add --cacheinfo 100644,<oid>,<path>` per path, then `write-tree`.
   4. `commit-tree <tree> -p <tip> -m "handoff: stage NN"` with `SignMode`: `--no-gpg-sign` unless `git config --show-origin --show-scope commit.gpgSign` reports `true` from **global or system** scope **and** every `gpg.*` key that would apply also comes from global/system scope. Then `-S` is used, and the run has the 60 s timeout for a GUI pinentry. Local-scope signing settings are ignored, with a notice.
   5. `update-ref <HEAD or refs/heads/branch> <new> <old>` (old value given, so a concurrent move fails cleanly).
   6. Current branch only: `update-index --cacheinfo` on the **real** index for exactly those paths, so `git status` shows them clean. Billy's other staged changes are untouched.
4. **Read-back:** `cat-file blob <commit>:<path>` for every path, with SHA-256 compared against what Foreman wrote. `cat-file` applies no textconv or filters. A mismatch shows the diff and keeps the stage in `signed (commit pending)`.
5. **Reads:** lock restore (`cat-file blob <commit>:<path>`), dirty check (`status --porcelain=v2 -z --ignore-submodules=all -- <path>`), `rev-parse --git-path info/exclude` (SR-32), `config --get user.name`. All go through `git::run`.
6. Every commit's stage record notes the overrides, the sign mode and the commit OID.

## Alternatives considered

- **`git add -- <paths>` then `git commit --only -- <paths>`** (MASTER-SPEC §8 and r6 F-014 as frozen). Simple porcelain, but `git add` runs repo-configured clean filters, and `git commit` honors repo-local `gpg.program`. Hooks off doesn't stop either. The security review replaced it with plumbing (SR-42), and this ADR follows the requirement. **The frozen spec text must be aligned (Change Request).**
- **`git2` 0.21.0 (libgit2).** In-process, but C code linked into Foreman (plus OpenSSL/libssh2 if network features creep in, and banned by ADR-016). Its attribute, filter and signing behavior differs from the git CLI used in the same repos, and the spec's guarantees are phrased as CLI flags auditable in argv.
- **`gix` 0.89.0** (pure Rust). Safe and promising, with plumbing equivalents. But it's pre-1.0 with fast-moving APIs, and mixing gix-written objects and refs with CLI git in one repo risks subtle divergence. A candidate for read-only operations later.

## Consequences

- Signing is about 8–12 short git processes, which is fine for a click-driven action.
- Because commits bypass `git add`, `.gitattributes` text conversion (for example `eol=crlf`) is **not** applied. Foreman writes LF and commits the exact bytes it wrote, which is what read-back verifies.
- If Billy signs commits globally, Foreman's commits are signed too. Repo-local signing config is ignored by design.

## Verification

Fixture repos and tests:

- `git::single_entry_point` (clippy `disallowed-methods`), `git::invocation_snapshot`, `git::env_cleared`, `git::timeout`, `git::no_hooks_on_any_command` (repo with local `core.hooksPath` and lint-staged);
- `git::filter_driver_never_runs` (a local `filter.x.clean` that would touch a marker file);
- `git::repo_gpg_program_never_runs` (local `gpg.program` + `commit.gpgSign=true`);
- `sign::refuses_changed_file`, `sign::pathspec_only` (unrelated staged changes survive and aren't committed), `sign::untracked_new_packet`, `sign::readback_detects_mismatch`, `sign::other_branch_leaves_worktree_head_and_index`, `sign::concurrent_ref_move_fails`;
- `lock::restore_from_git`.

## Revisit when

- gix reaches 1.0 with plumbing parity.
- git changes plumbing semantics for `commit-tree`/`update-ref` (checked when Doctor sees a new git major).

## Exit path

`git::run` takes a closed operation enum, so a gix-backed implementation can replace operations one at a time behind the same enum, verified against the same fixtures.
