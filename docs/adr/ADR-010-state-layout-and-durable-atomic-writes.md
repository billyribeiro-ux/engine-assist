# ADR-010: State layout and durable atomic writes: temp + F_FULLFSYNC + renameatx_np swap, never over a newer edit

## Status
Proposed

## Date
2026-10-19

## Context

- NFR-10: no data loss or corruption, ever.
- NFR-11: crash-safe run state.
- NFR-12 and SR-33: external edits win; never overwrite a file that changed since Foreman read it.
- SR-32: temp `.<name>.foreman-tmp` in the same directory, `sync_all`, rename, fsync of the directory; temp pattern in `.git/info/exclude`.
- R-22: whether `sync_all` issues `F_FULLFSYNC` on macOS must be checked in the Rust std source.

Repos are edited concurrently by Claude Code, editors and git.

**Source check (Rust std, tags 1.97.0 and 1.99.1, `library/std/src/sys/fs/unix.rs`):** `File::sync_all` → `fsync()` → on `target_vendor = "apple"`, `os_fsync` is `libc::fcntl(fd, libc::F_FULLFSYNC)`. `sync_data` → `datasync()` → on Apple also `fcntl(fd, F_FULLFSYNC)`. **So `sync_all` already issues `F_FULLFSYNC` on macOS; no extra call is needed for files.**

**Other checks:**

- `tempfile` 3.27.0: `persist` renames without any fsync; `persist_noclobber` uses `rustix::fs::renameat_with(CWD, …, RenameFlags::NOREPLACE)`.
- `rustix` 1.1.5 on Apple: `renameat_with` calls `renameatx_np`, with `RenameFlags::EXCHANGE = RENAME_SWAP` and `NOREPLACE = RENAME_EXCL` (`src/backend/libc/fs/types.rs` 543–548, `syscalls.rs` 585–620). `fcntl_fullfsync` exists (`src/fs/fcntl_apple.rs`); `OFlags::NOFOLLOW_ANY` exists for Apple.

## Decision

All persistent writes go through `foreman-core::store::Root` (ADR-011 for the path walk):

1. **Read** returns `(bytes, ReadStamp { dev, ino, size, mtime_ns, sha256 })` or `ReadStamp::Absent`. Callers keep the stamp with the parsed value (`progress.json` keeps stamps for repo files Foreman may write).
2. **Write**(`path`, `bytes`, `expect: ReadStamp`):
   1. Walk to the parent directory fd (`O_DIRECTORY|O_NOFOLLOW` per component).
   2. `openat(parent, ".<name>.<16 hex>.foreman-tmp", O_CREAT|O_EXCL|O_WRONLY|O_NOFOLLOW|O_CLOEXEC, 0o600)` (SR-32). The 64 random bits come from a `uuid` v4. `O_EXCL` plus a random name means a planted file or symlink can never be opened as our temp. In repos, `fchmod` to the existing file's mode (or `0644`) before the rename; in app data, files stay `0600`. Stale temps from a crash (`.*.foreman-tmp` older than 1 hour, regular files only) are handled when the directory is next written: a temp whose bytes equal the current target is removed; a temp that **differs** from its target (it may hold a newer external edit swapped out just before a crash) is never discarded: it is copied durably to `<app-data>/recovered/<timestamp>-<projectId>-<name>` (`0600`, same write rules), then removed from the folder, and Foreman shows "Recovered a file Foreman was writing when it stopped" with Open and Reveal. Test: `atomic::leftover_temp_differs_moved_to_recovered`.
   3. Write all bytes, then `File::sync_all()` (= `F_FULLFSYNC`).
   4. If `expect` is `Absent`: `renameat_with(parent, tmp, parent, name, NOREPLACE)`. `EEXIST` → **Conflict** (someone created it meanwhile).
   5. Otherwise: `renameat_with(..., EXCHANGE)`. The temp name now holds the previous file. Hash it (opened `O_NOFOLLOW`): if it equals `expect.sha256`, unlink the temp; otherwise **swap back** (EXCHANGE again) and return **Conflict** with the newer stamp. The newer external content is never lost, even if it arrived between Foreman's check and the rename.
   6. Sync the parent directory: the directory fd converted to `File` and `sync_all()` (= `F_FULLFSYNC`). On `ENOTSUP`/`EINVAL`, fall back to `rustix::fs::fsync` and log once.
   7. If `renameatx_np` returns `ENOTSUP`/`NOSYS` for `EXCHANGE` (a volume without swap support), fall back to: re-stat and re-hash the target, compare with `expect`, plain `renameat`. The remaining window is microseconds, and it's logged as `degraded-cas`.
3. **Appends** (`runs/index-YYYY-MM.jsonl`, `docs/lessons.md` is **not** an append target; it's a full CAS write): open `O_APPEND|O_NOFOLLOW`, one `write` of a line ≤ 4 KB, `sync_data` (= `F_FULLFSYNC`). A torn last line is skipped on read.
4. **Temp exclusion** (SR-32): on project setup or open, `store::git_exclude::ensure(root)`:
   1. asks `git::run` for `rev-parse --git-path info/exclude` (so linked worktrees and `.git` files work);
   2. refuses a result that is a symlink, or lies outside the project root or the repo's common git directory (`rev-parse --git-common-dir`);
   3. appends `.*.foreman-tmp` idempotently with the same CAS write.

   This is the only write under `.git`, exempt from SR-30's `.git` rule by its typed purpose. The watcher ignores the pattern.
5. **Layout** is MASTER-SPEC §7, plus `approvals/<projectId>/` (approved copies, ADR-022), `cache/fingerprints/`, `logs/`. App data directories `0700`, files `0600` (SR-53).
6. **Run state:** `run.json` is written with status `running` before spawn and updated at exit. On startup, any `running` record whose pid is gone (or belongs to a different process start time) becomes `interrupted` and is listed with Resume.
7. JSON is serialized with sorted keys, two-space indent and a trailing newline. Every file has `schemaVersion` (ADR-023).

## Alternatives considered

- **`tempfile::NamedTempFile::persist`.** Simple, but no fsync of the file or directory, and no compare-and-swap; it creates temps by path rather than through our held directory fd. It would need wrapping anyway, so `tempfile` stays a dev-dependency (test fixtures) only.
- **Explicit `fcntl(F_FULLFSYNC)` via `rustix::fs::fcntl_fullfsync` instead of `sync_all`.** Redundant: std already does it on Apple. We keep `sync_all` (portable and safe) and keep `fcntl_fullfsync` only as a documented equivalent.
- **`F_BARRIERFSYNC`** (cheaper ordering barrier). Weaker durability than NFR-10 asks for; write volume is tiny, so the full flush cost (milliseconds) is acceptable.
- **Advisory locks (`fs4` 1.1.0, `fd-lock` 4.0.4) on repo files.** Claude Code, editors and git don't honor them, so they don't prevent the conflict that matters. The swap check does.
- **SQLite with WAL for app state.** Durable and transactional, but opaque, not plain JSON (PR-06), and still no help for repo files.

## Consequences

- Each durable write costs two full flushes (file + directory) and one extra hash. Volumes are small (decision files, progress, run records), within NFR-04 because flushes happen after the preview, not during resolution.
- In-place writers holding an fd open across our swap (rare; most editors save atomically) write to the inode that ends up at the temp name. Foreman detects the mismatch and swaps back, so their edit survives.
- A conflict is a normal, user-visible outcome ("This file changed on disk since Foreman read it. Review the newer version"), never a silent overwrite.

## Verification

- `atomic::crash_injection` (proptest): a fault-injecting `Root` aborts at every step (after the temp create, after the write, before and after the sync, after the swap, before the unlink, before the directory sync). After "restart", the target is either the old or the new content, never torn, and no temp is left as the target.
- `atomic::refuses_newer_file` (property, interleaved external writes, including writes between the stamp check and the swap).
- `atomic::noreplace_conflict`, `atomic::swap_unsupported_fallback`, `atomic::dir_sync_fallback`, `atomic::temp_excluded_from_git`, `atomic::refuses_planted_tmp_symlink`, `atomic::exclude_writer_scope` (normal repo, linked worktree, a `.git` file pointing outside), `fs::app_data_permissions`.
- NFR-11: `kill_with_live_child` integration test.
- **UNVERIFIED:** that `F_FULLFSYNC` on a **directory** fd succeeds on APFS. Tried: the std source (shows the call, not the filesystem's behavior); no Apple documentation fetched. `atomic::dir_sync_on_apfs` runs on Billy's Mac in M0, and the fallback is defined.

## Revisit when

- Rust std changes Apple `fsync` semantics (watch `library/std/src/sys/fs/unix.rs`).
- Write volume grows enough (for example, run-log streaming) that full flushes show up in NFR-05.

## Exit path

`store::Root` is the single seam. Swapping the strategy (for example to `F_BARRIERFSYNC` for the run-log index only, or to a different CAS) changes one module and its crash tests.
