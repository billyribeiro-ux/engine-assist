# ADR-013: File watching with notify 8.2.0 + notify-debouncer-full 0.7.0 as a hint; files are always re-read and validated

## Status
Proposed

## Date
2026-10-19

## Context

Foreman must see:

- a packet within p95 ≤ 1 s of its write (NFR-06);
- declared `out` files (round tracker, P2);
- outside changes to decision files (F-022);
- tamper of signed packets (P2);
- sensor event files in app data (P2).

It must ignore `.git`, `node_modules`, `target`, `.claude/worktrees/**`, `*.foreman-tmp` and secrets (SR-50), and must not follow symlinks out of a project (SR-31). Editors and Claude Code save atomically (rename), which produces bursts of FSEvents.

Checked 2026-10-19:

- `notify` 8.2.0 is max stable; 9.0.0-rc.6 was released 2026-10-17. The default feature is `macos_fsevent` (r1).
- `notify-debouncer-full` 0.7.0 is stable (it depends on `notify ^8.2.0`); 0.8.0-rc.2 tracks notify 9.

## Decision

- **`notify` 8.2.0** (FSEvents backend) and **`notify-debouncer-full` 0.7.0** with a 250 ms timeout, in `foreman-app::watch`. One debouncer per open project root (recursive) and one for `app-data/events/` (P2).
- **Events are hints, never truth.** For each debounced path, the watcher:
  1. drops it unless it matches an interest set computed from the applicable path (`docs/handoff/stage-NN*.md`, declared `out` paths, `.foreman/*.json`, `docs/lessons.md`);
  2. drops ignored and secret patterns (shared with `pathpolicy::check_read`);
  3. re-reads the file through `store::Root::read` (`O_NOFOLLOW` walk; a symlinked target is refused and reported, never followed);
  4. compares its `ReadStamp` with the last known one and acts only on a real content change.
- **Watch roots** (SR-31): notify's FSEvents backend resolves the watch root itself, so configuration can't stop it following a symlinked root. Before watching, and on every event, the watch root and its ancestors up to the project root are `lstat`-checked; a symlink stops the watcher for that root, with a Doctor warning. Each event path is resolved again through `store` before it's read, and hard-linked or non-regular files are refused.
- **Missed events are tolerated by design.** On project open, window focus, and before every launch or sign, Foreman re-stats its interest set (cheap: about 30 paths). FSEvents coalescing or overflow therefore can't hide a change for longer than the next interaction (NFR-13).
- If the watcher fails to start (for example, a volume without FSEvents), Doctor shows "file watching unavailable: packets appear when you focus Foreman", and the re-stat path carries on.
- The watcher **never triggers an action** (SR-18). It updates state and emits `packet:changed`, `decision:pending` or `project:changed`.

## Alternatives considered

- **notify 9.0.0-rc.6 / debouncer-full 0.8.0-rc.2.** Release candidates; no need to take them before stable.
- **Polling only** (re-stat the interest set every 500 ms). Simple and deterministic, but constant wakeups for up to 50 projects (NFR-19). It stays as the fallback and the on-focus check.
- **`watchexec` 8.4.4.** A CLI-oriented layer on notify; more than needed.
- **Direct FSEvents bindings.** `unsafe` FFI, which is forbidden in our crates and duplicates notify.

## Consequences

- At most 250 ms + parse latency, within NFR-06.
- FSEvents reports at directory granularity in some cases. The interest-set re-stat absorbs that.
- Watching repo roots recursively includes large trees (`node_modules`). FSEvents handles them in the kernel; we filter early.

## Verification

- `watcher::packet_visible_within_1s` (fake writer, p95 over 100 writes, NFR-06), `watcher::atomic_save_detected` (rename-over), `watcher::no_symlink_follow`, `watcher::symlinked_root_refused`, `watcher::ignores_tmp_and_secrets`, `watcher::events_never_act` (dependency test), `watcher::refocus_rescan_catches_missed_event` (watcher disabled).

## Revisit when

- notify 9.0 ships stable and debouncer-full follows. Upgrade after the watcher tests pass.
- FSEvents latency on macOS 27 exceeds the budget.

## Exit path

The watcher only produces "path may have changed" hints. Replacing notify with polling or another crate changes `watch.rs` only.
