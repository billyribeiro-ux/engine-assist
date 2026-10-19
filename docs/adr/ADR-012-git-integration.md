# ADR-012: Git through the git CLI with fixed safe overrides; signing by pathspec commit or plumbing, with read-back

## Status
Proposed

## Date
2026-10-19

## Context

Foreman uses git for:

- `git init` at setup;
- blueprint lock restore (`<commit>:<path>`, F-002);
- dirty-tree checks at pin;
- pathspec commits of decision changes before Stage 5;
- signing (P2, F-014): current-branch pathspec commit, or another branch through plumbing, then read-back;
- tamper detection.

MASTER-SPEC §8, SR-42 to SR-45 fix:

- every git call passes `-c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat -c diff.external= -c core.editor=true` and `GIT_TERMINAL_PROMPT=0`, with a 60 s timeout;
- current branch: `git add -- <paths>` then `git commit --only -m … -- <paths>`;
- another branch: `hash-object`, a temporary index, `commit-tree`, `update-ref`.

A repo's `.git/config` is untrusted input (SR-45).

Checked 2026-10-19:

- `git2` 0.21.0: default features are `[]`; `https` pulls `openssl-sys`, and `ssh` pulls libssh2.
- `gix` 0.89.0: MSRV 1.88; default features don't include network, while `blocking-network-client` adds `gix-transport`.

## Decision

1. **The git CLI** at an allowlisted absolute path (ADR-007), wrapped by `foreman-app::git::GitCli`. Argument lists are built by pure functions in `foreman-core::git`. Every invocation is:
   ```text
   git -C <root> -c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat
       -c diff.external= -c core.editor=true -c protocol.allow=never <subcommand> …
   ```
   - `protocol.allow=never` is an extra, additive guarantee that no git call Foreman makes can reach a remote (SR-60).
   - Environment: `env_clear()`, then `HOME`, `PATH` (built), `LANG=C`, `GIT_TERMINAL_PROMPT=0`; `GIT_OPTIONAL_LOCKS=0` on read-only commands, so `status` doesn't take `index.lock` while Claude works.
   - stdin is null, the timeout is 60 s, and output is capped. A `commit.gpgsign` GUI prompt ends at the timeout with a clear message.
2. **Git config preflight**, before any command that writes objects or commits (`add`, `commit`, `hash-object -w`, `commit-tree`):
   - run `git config --show-origin --show-scope --get-regexp '^(filter\..*|gpg\..*program|core\.sshcommand|commit\.gpgsign)$'`;
   - a `filter.<driver>.*` defined at **local or worktree scope** (or included from a file under the repo) whose driver applies to any signed path (`git check-attr filter -- <paths>`) **blocks** signing, because `git add` would execute it;
   - `gpg.program` or `gpg.<format>.program` at local scope blocks signing too;
   - global and system scope are Billy's own and trusted.

   This tightens SR-45's intent ("a repo's `.git/config` is untrusted"). The spec's flag list doesn't cover filter drivers or `gpg.program`, which run on `add`/`commit` even with hooks disabled.
3. **Read-back** (SR-42) reads every committed path with `git cat-file blob <commit>:<path>`, not `git show`, so no textconv or filter can alter what's compared. The SHA-256 is compared with what Foreman wrote. On a mismatch: "Your repo's commit hooks changed the packet" with the diff. The same plumbing reads the blueprint at a locked commit (F-002) and the committed packet for tamper checks.
4. **Signing, current branch:** `git add -- <paths>`, then `git commit --only -m "handoff: stage NN" -- <paths>`. Never `-a` or `add -A`. `--only` with pathspecs excludes Billy's unrelated staged changes.
5. **Signing, another branch** (SR-43):
   1. `hash-object -w --no-filters --stdin` per file (bytes from memory; nothing is written to the working tree);
   2. `GIT_INDEX_FILE=<app-data tmp>`, `read-tree <branch tip>`, `update-index --add --cacheinfo 100644,<oid>,<path>`, `write-tree`;
   3. `commit-tree <tree> -p <tip> -m …`;
   4. `update-ref refs/heads/<branch> <new> <tip>` (old value given, so a concurrent move fails).

   HEAD, the real index and the working tree are untouched.
6. Everything is recorded in the stage record (hooks disabled, overrides used, commit OID).

## Alternatives considered

- **`git2` 0.21.0 (libgit2).** In-process, no CLI parsing. Lost because:
  - it's C code linked into Foreman, with OpenSSL and libssh2 if its network features are enabled (and cargo-deny would have to police that);
  - its behavior on attributes, filters, `commit.gpgsign` and hooks differs from the git Billy uses in the same repos;
  - the spec's guarantees are phrased as git CLI flags that are auditable in argv logs.
- **`gix` 0.89.0** (gitoxide, pure Rust). Safe Rust and no subprocess, with `hash-object`/`commit-tree` equivalents. Lost because it's pre-1.0 with fast-moving APIs; its index/`add` with attributes, filters and signing is not at git parity; and mixing gix-written commits with CLI git in the same repo invites subtle differences (for example signing config ignored). A good candidate for read-only paths later (Exit path).
- **Running `git` through a shell.** Forbidden (SR-02).

## Consequences

- Each git call is a process spawn (~ms). Signing makes about 6–10 calls, which is fine for a click-driven action.
- Output parsing uses porcelain/plumbing formats only (`--porcelain=v2 -z`, `cat-file`, `rev-parse`), which are stable.
- If Billy's own global hooks or signing config are needed for signing commits, they still apply (global scope), except hooks, which never run (SR-44).

## Verification

Fixture repos:

- lint-staged + Prettier on Markdown: `git::no_hooks_on_any_command`, `sign::readback_detects_hook_rewrite` (with the hooks path forced through a local `core.hooksPath` to prove the override wins);
- a new untracked packet and stage record;
- a dirty tree with unrelated staged files: `sign::pathspec_only`;
- `sign::other_branch_leaves_worktree_head_and_index`;
- `sign::refuses_changed_file`;
- `git::invocation_snapshot`, `git::timeout`;
- `git::local_filter_blocks_signing`, `git::local_gpg_program_blocks_signing`, `git::protocol_never`;
- `lock::restore_from_git`.

## Revisit when

- gix reaches 1.0 with index/add/commit parity, including attributes and signing.
- git changes the semantics of `commit --only` or the `-c` keys used (git release notes are checked when Doctor sees a new major).

## Exit path

`GitCli` sits behind a `GitPort` trait (`read_blob`, `status`, `commit_paths`, `commit_on_branch`, `init`). A gix implementation can replace read-only operations first (`read_blob`, `status`), then the rest, test by test against the same fixtures.
