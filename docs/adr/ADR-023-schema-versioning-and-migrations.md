# ADR-023: Every file Foreman owns has an integer schemaVersion with pure, golden-tested forward migrations

## Status
Proposed

## Date
2026-10-19

## Context

Foreman should last 20 years (prompt 4.2). The relevant requirements:

- MASTER-SPEC §7: every JSON file has `schemaVersion` and a migration.
- PR-06: plain JSON and Markdown in documented schemas (Stage 4.4 `docs/contracts/files.md`).
- F-018: an existing `.foreman/` from a newer schema opens read-only.
- NFR-17: app data must be rebuildable from the repos (except the run log).

Repo files are shared across machines and possibly across Foreman versions; app data is local.

## Decision

1. Every JSON file (repo and app data) has a top-level integer `schemaVersion`, starting at 1, and is serialized with sorted keys, two-space indent and a trailing newline.
2. `foreman-core::schema`:
   - per file kind, `const CURRENT: u32` and a chain of pure functions `fn v1_to_v2(serde_json::Value) -> Result<Value, MigrationError>`;
   - `migrate::<T>()` applies the chain, then deserializes with `deny_unknown_fields`.
3. **Reading** a file:
   - `schemaVersion` < CURRENT → migrate in memory. The migrated form is written back only on Foreman's next legitimate write to that file (never just because it was read), so opening a project doesn't create repo diffs.
   - `schemaVersion` > CURRENT → **read-only** with "Created by a newer Foreman (schema N). Update Foreman to edit this project." Nothing is written.
   - Missing or invalid → the F-020 invalid-file path.
4. **Golden fixtures:** for every historical version of every file kind, a fixture in `crates/foreman-core/tests/schema/<kind>/v<N>.json` must migrate to an insta snapshot of the current form. Removing a migration is forbidden; the chain only grows.
5. Template caches (`templates/<sha256>.md`) are content-addressed and need no versioning. The parsed `Template` cache includes Foreman's parser version in its key, so a parser upgrade re-parses.
6. Migrations of repo decision files go through the decision-approval path (ADR-022): a migrated write is Foreman's own change.

## Alternatives considered

- **Semantic versions per file.** More expressive, but an integer per kind is enough for forward-only migrations.
- **Tolerant readers (ignore unknown fields), no versions.** Silent data loss when an old Foreman rewrites a newer file. Explicit versions plus read-only for newer files prevent that.
- **A database with migrations (SQLite).** Rejected for PR-06 (ADR-018, design doc §4).

## Consequences

- Every schema change carries a migration and a fixture in the same change. That's the cost of never losing data.
- Older Foreman builds can still read newer projects (read-only), so Billy can always see his state.

## Verification

`schema::all_versions_migrate` (golden), `schema::newer_version_read_only`, `schema::read_does_not_rewrite`, `schema::deny_unknown_fields_after_migrate`, and the Stage 4.4 contract tests listing each file's class (PR-02).

## Revisit when

A file kind needs a breaking redesign that can't be expressed as a migration. Then add a new file kind and migrate into it.

## Exit path

Files are plain JSON with an explicit version, so any future tool, or a script in another language, can read and convert them using `docs/contracts/files.md`.
