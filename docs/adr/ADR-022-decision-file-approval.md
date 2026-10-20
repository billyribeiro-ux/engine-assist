# ADR-022: Decision files take effect only from Billy-approved copies held in app data

## Status
Proposed

## Date
2026-10-19

## Context

`.foreman/project.json`, `overlay.json` and `blueprint.lock.json` decide the process: conditions, skips, additions, model policy and the lock. The governed agent (Claude Code, working in the same repo) can edit them, and so can git checkouts and editors.

- **R-02 / SR-40:** a change from outside Foreman to a decision field opens a Pending outside change (redacted diff, Accept, Reject) and has **no effect until accepted**; app data stores the approved hashes.
- **F-022:** variables reload with a toast.
- **R-10:** from Stage 5 on, Foreman's own decision changes stay pending until the next signing commit.
- **SR-41:** the lock is verified.

## Decision

1. **Approved state lives in app data:** `approvals/<projectId>.json` (`{file: sha256}`, item-kind confirmations) plus `approvals/<projectId>/<file>.json`, a full approved copy of each decision file. **Foreman's effective configuration is always read from the approved copies**, never directly from the repo files.
2. **Detection:** on project open, on watcher hints (ADR-013), before every launch, and before every sign, Foreman re-reads each repo decision file through `store`. If its SHA-256 ≠ the approved hash:
   - **Unparsable** or a newer `schemaVersion` → project read-only, "Open in editor" (F-020).
   - Otherwise `core::decisions::classify_outside_change(approved, on_disk)`:
     - **variables only** → auto-accept; update the approved copy and hash; toast "Variables reloaded";
     - **decision fields** → `PendingOutsideChange { file, diff (redacted per SR-55), detectedAt }` in `progress.json`; Today shows it; the effective config stays the approved copy.
3. **Accept** (confirmed in a native dialog raised by Rust, SR-24a; the webview's button only requests it) sets the approved copy to the on-disk content (re-read and hash-checked at accept time; if it changed again, the review refreshes).
4. **Reject** writes the approved content back with `store.write` against the on-disk `ReadStamp` (ADR-010). A newer edit meanwhile → conflict, and the review repeats. Nothing is overwritten blindly.
5. **Foreman's own changes:** a write goes to the repo file and the approved copy together (the copy first, then the repo CAS write; on a repo conflict the copy is rolled back). It is refused (`OutsidePending`) while the repo file holds an outside edit that is neither the approved nor the last committed content, so Foreman never writes over an unreviewed edit (amended 2026-10-20, as built in M1).
   - Before Stage 5 is signed: committed at once by `git::run` plumbing (ADR-012).
   - From Stage 5 on: the change is held as pending in `progress.json` and written into the repo only inside the next signing commit (R-10). Until then the approved copy is ahead of the repo file, and the drift check treats the repo file as matching the **last committed** approved hash, so it doesn't flag Foreman's own pending change.
   - **Signing on another branch** (SR-43) commits the pending change on that branch only; the checked-out branch's file is untouched. `approvals/<id>.json` therefore keeps the committed hash **per branch**: `committedSha256` for the branch the change was committed on in the working tree, and `committedOnOtherBranches[<ref>]` for each other branch signed onto. The drift check uses the entry for the branch HEAD names (`symbolic-ref -q HEAD`), so checking out the signed branch (or merging it) doesn't show Foreman's own change as an outside edit. Test: `decisions::other_branch_sign_no_false_drift`.
6. **Lock:** `check_lock` verifies the lock against `templates/<sha256>.md` and, when the blueprint repo is available, against `cat-file blob <commit>:<path>` (SR-41). A mismatch → read-only.
7. Every launch's run settings deny Read/Edit of `//<repo>/.foreman/**` (ADR-021) as defense in depth; this doesn't bind Bash.

8. **Milestones:** the approved-copy store, classification and own-change commits in M1 (Stage 6); the review UI (`pending_changes_list`, `pending_change_accept` with its native dialog, `pending_change_reject`) in M2; live detection by the watcher in M3; per-branch committed hashes with signing in M4.

## Alternatives considered

- **Trust the repo files and show a notification on change.** The governed agent could change conditions or skips and have them take effect before Billy looks (R-02, serious).
- **Sign decision files cryptographically** (an HMAC with a key in app data). It detects tampering but doesn't define what takes effect. The approved-copy model gives both detection and a safe effective state, without key management.
- **Keep decision files only in app data.** It would lose versioning in the repo and the provenance the blueprint expects (MASTER-SPEC §7).

## Consequences

- If app data is lost, approvals are rebuilt by treating the committed versions at HEAD as approved, after a one-time review prompt (NFR-17).
- A git checkout to another branch with different decision files raises a pending change. That's intended: Billy confirms which rules apply.

## Verification

`decisions::outside_edit_has_no_effect_until_accepted` (one case per file), `decisions::variables_only_autoaccept`, `decisions::reject_restores_via_cas`, `decisions::pending_from_stage5_not_flagged`, `lock::hash_mismatch_read_only`, `lock::restore_from_git`, `preflight::diff_shows_redacted_change`.

## Revisit when

The v1.1 CR track (F-103) or full overlay authoring (F-104) adds new decision kinds.

## Exit path

Approved copies are plain JSON. Removing the mechanism means reading repo files directly; the classification code remains useful as a diff view.
