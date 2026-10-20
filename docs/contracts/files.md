# Foreman · File contracts

**Status:** DRAFT for the Stage 4.7 review · 2026-10-19 · Blueprint stage 4.4
**Inputs:** `docs/design-doc.md` §3.6, §5.2, §5.6; ADR-009, ADR-010, ADR-017, ADR-018, ADR-021, ADR-022, ADR-023; MASTER-SPEC §7, §8; `docs/requirements.md` PR-02…PR-06, SR-09, SR-15, SR-52…SR-55; `spec/council/r6-feature-specs.md` F-002, F-014, F-020.

Every file Foreman owns is listed here with its schema, data class, mode, writer, retention and migration rule. A file Foreman writes that isn't listed here, or doesn't validate against its schema, fails `files::every_write_is_listed` (AC-XC-15). The schemas are JSON Schema **2020-12**; `crates/foreman-core/tests/schema/` holds them as files and validates every golden fixture and every file written by the integration tests against them (`files::schemas_validate_goldens`, `files::written_files_validate`).

---

## 1. Rules for every file

| ID | Rule | Test |
|---|---|---|
| FILE-R1 | **Format.** UTF-8 JSON, no BOM, object keys sorted (byte order of the key's UTF-8), arrays of records sorted by their ID field unless the schema says the order is meaningful, two-space indent, LF line endings, one trailing newline (ADR-010 §7, ADR-023). A parse–write round trip is byte-identical. | `files::canonical_roundtrip` (AC-F020-07) |
| FILE-R2 | **`schemaVersion`.** Every Foreman-format JSON file has a top-level integer `schemaVersion ≥ 1`. The exceptions are the two files in Claude Code's own settings format (§4.4, §4.11), which Foreman writes once and never reads back as data. | `files::schema_version_present` |
| FILE-R3 | **Migration** (ADR-023). Reading: `schemaVersion < CURRENT` → the pure chain `v<N>_to_v<N+1>` runs in memory; the migrated form is written only on Foreman's next legitimate write to that file. `schemaVersion > CURRENT` → the file and (for repo files) the project are read-only: "Created by a newer Foreman (schema N). Update Foreman to edit this project." Missing, unparsable or failing `deny_unknown_fields` after migration → the F-020 invalid-file path. Migrations only grow; each historical version has a golden fixture in `crates/foreman-core/tests/schema/<kind>/v<N>.json`. Repo decision-file migrations are Foreman's own change and go through the approval path (ADR-022). | `schema::all_versions_migrate`, `schema::newer_version_read_only`, `schema::read_does_not_rewrite`, `schema::deny_unknown_fields_after_migrate` |
| FILE-R4 | **Writes** go through `store::Root::write` with a `ReadStamp` (ADR-010): temp `.<name>.<16 hex>.foreman-tmp` created `O_CREAT\|O_EXCL\|O_NOFOLLOW` `0600`, `sync_all` (= `F_FULLFSYNC`), `renameatx_np` `RENAME_SWAP` (or `RENAME_EXCL` for a new file), compare the swapped-out bytes with the stamp, directory `F_FULLFSYNC`. A newer file is never overwritten. JSONL appends use `O_APPEND`, one `write` ≤ 4 KB, `sync_data`. | `atomic::*` (ADR-010) |
| FILE-R5 | **Modes.** App data: directories `0700`, files `0600`, from the first write (SR-53). Repo files: the existing file's mode, or `0644` for a new file (ADR-010). | `fs::app_data_permissions` |
| FILE-R6 | **Reads** of repo files go through `store::Root::read` (no symlink at any component, regular file, `nlink == 1`, size caps: decision files, packets and lessons 1 MB; Claude config 256 KB). | `fs_policy::read_*` |
| FILE-R7 | **Data classes** (PR-02): (a) project decisions and signed records, repo, versioned; (b) in-flight state, run records, prompt hashes; (c) terminal output tails, sensitive; (d) sensor events, sensitive, short retention; (e) redacted config snapshots; (f) settings incl. the captured login PATH; (g) claude.ai Project and chat URLs, repo `project.json`. | `files::class_table_matches` (this table is parsed by the test) |
| FILE-R9 | **Write allowlist** (SR-30). Inside a project, Foreman writes only the repo files in §2 rows 1–8: under `docs/` and `spec/`, `.foreman/` through the decision-file writers, the `council-setup` destinations under `spec/.claude/agents/`, `<git-path info/exclude>` through its own writer, and root `CLAUDE.md` through the assembler. Outside projects and app data, the only write is `WritePurpose::UserExport` (§2 row 28): a path Billy picks in the Rust save dialog, `.md` only, never inside a registered project's `.foreman/`, `.git` or `.claude`, written with the same atomic rules. Paths are ≤ 1,024 bytes, components ≤ 255 bytes. Parsers of these files (JSON, Markdown) stop at nesting depth 64 (SR-31). | `fs_policy::allowlist_only`, `fs_policy::rejects_overlong_path`, `files::json_depth_limit` |
| FILE-R8 | **No secrets, no content.** No file stores environment values, prompt text (only SHA-256), clipboard contents, transcript contents, or `~/.claude.json` keys other than `mcpServers`. Run inputs are stored only in `progress.json` (Billy's own typed values). | `runlog::no_env_values`, `runlog::argv_prompt_redacted`, `privacy::no_prompt_text_on_disk` |

## 2. Inventory

| # | File | Root | Class | Mode | Writer (module) · when | Retention | Schema |
|---|---|---|---|---|---|---|---|
| 1 | `.foreman/project.json` | repo | a, g | repo | decision-file writer (`core::decisions` via `store`, `WritePurpose::DecisionFile`) · setup, condition/variable/URL change, accepted migration | versioned in git | §3.1 v1 |
| 2 | `.foreman/overlay.json` | repo | a | repo | decision-file writer · skip, add step/item (P2) | git | §3.2 v1 |
| 3 | `.foreman/blueprint.lock.json` | repo | a | repo | decision-file writer · pin, re-pin (before Stage 1 signed) | git | §3.3 v1 |
| 4 | `.foreman/stages/stage-NN.json` | repo | a | repo | signing (`app` sign flow → `store`) · sign phase 1 (P2) | git; immutable | §3.4 v1 |
| 5 | `docs/handoff/stage-NN.md` | repo | a | repo | paste-back, normalize, signing · as named | git | §5 (text) |
| 6 | `docs/handoff/stage-NN-attachments.md` | repo | a | repo | paste-back · after the packet | git | free text |
| 7 | `docs/lessons.md` | repo | a | repo | setup (header), lessons append (P2) · CAS write, never append-mode | git | blueprint F.1 header |
| 8 | `<git-path info/exclude>` | repo `.git` | — | existing | `store::git_exclude` · setup, open (after `git::run` confirmed `rev-parse --show-toplevel` = the project root, SR-44) | — | one line `.*.foreman-tmp` |
| 9 | `settings.json` | app | f | 0600 | `app` settings commands · on change; contract result | until deleted | §4.1 v1 |
| 10 | `projects.json` | app | f | 0600 | setup, remove | until deleted | §4.2 v1 |
| 11 | `projects/<projectId>/progress.json` | app | b | 0600 | `app::state` · every state change | until project removed | §4.3 v1 |
| 12 | `approvals/<projectId>.json` | app | b | 0600 | `core::decisions` · accept, own change, item-kind confirm | until project removed | §4.5 v1 |
| 13 | `approvals/<projectId>/{project,overlay,blueprint.lock}.json` | app | a (copy) | 0600 | `core::decisions` · accept, own change | until project removed | same as §3.1–§3.3 |
| 14 | `runs/<runId>/run.json` | app | b | 0600 | `app::launch` · before spawn (`running`), at exit | Clear run history | §4.6 v1 |
| 15 | `runs/index-YYYY-MM.jsonl` | app | b | 0600 | `app::launch` · one line at run exit (append) | kept for metrics; Clear run history | §4.7 v1 (per line) |
| 16 | `runs/<runId>/settings.json` | app | b | 0600 | `core::launch::run_settings` → `store` · before spawn, never rewritten | with its run | §4.4 (Claude Code format) |
| 17 | `runs/<runId>/output.log` | app | c | 0600 | `sys::pty` ring flush · every 5 s and at exit | 90 days (SR-53) | text, last 2,000 lines |
| 18 | `events/<runId>/<seq>-<event>.json` | app | d | 0600 | `foreman-sensor` (P2) · per hook event; pruned by `app::sensor_ingest` | ≤ 20 per run; deleted at stage sign or 30 days after run end | §4.8 v1 |
| 19 | `cache/fingerprints/<cwdHash>.json` | app | e | 0600 | `app::launch` · `config_ack`, stat-cache and auth-cache updates | until project removed | §4.9 v1 |
| 20 | `templates/<sourceSha256>.md` | app | a (copy) | 0600 | lock pin / restore · once | 90 days after no project uses it | blueprint text, content-addressed |
| 21 | `templates/<sourceSha256>.p<parserVersion>.json` | app | b | 0600 | template cache · first parse | with its `.md` | §4.10 v1 |
| 22 | `probe/settings.json` (+ marker files) | app | b | 0600 (dir 0700, recreated per probe run) | `sys::claude` probes (P2) | recreated each probe run | §4.11 (Claude Code format) |
| 23 | `tmp/git-index-<uuid>` | app | b | 0600 | `sys::git` · during a plumbing commit | deleted after the commit; stale ones (> 1 h) at startup | git index format |
| 24 | `bin/foreman-sensor-<sha256-12>` | app | — | 0700 | startup copy from the bundle (P2) | replaced on hash change | binary |
| 25 | `logs/foreman.log[.1-.4]` | app | b | 0600 | `app::logging` | 10 MB × 5, rotated by size | redacted text |
| 26 | `projects/<projectId>/progress.json.corrupt-<timestamp>` | app | b | 0600 | `progress_rebuild_apply` · keeps the corrupt original (AC-F020-12) | until project removed | as found |
| 27 | `recovered/<timestamp>-<projectId>-<name>` | app | b | 0600 | `store` · a leftover `.foreman-tmp` that differs from its target (ADR-010) | until Billy deletes it (shown in a notice with Open and Reveal) | as found |
| 28 | Export files (metrics summary, run sheet) | Billy's chosen folder | b | 0644 | `metrics_export`, `run_sheet_export` · `WritePurpose::UserExport`, path from the Rust save dialog, `.md` only | Billy's | Markdown |

`<cwdHash>` is the first 32 hex characters of SHA-256 over the canonical absolute cwd path. `<seq>` is a 20-digit zero-padded nanosecond timestamp, `-`, and the zero-padded pid (ADR-009).

## 3. Repo files

Common definitions, referenced as `urn:foreman:common#/$defs/…`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:common",
  "$defs": {
    "uuid": { "type": "string", "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$" },
    "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
    "gitOid": { "type": "string", "pattern": "^(?:[0-9a-f]{40}|[0-9a-f]{64})$" },
    "timestamp": { "type": "string", "format": "date-time", "pattern": "Z$" },
    "isoDate": { "type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}$" },
    "stage": { "type": "integer", "minimum": 0, "maximum": 18 },
    "stepKey": { "type": "string", "pattern": "^(?:[0-9]{1,2}\\.[0-9]{1,2}[a-z]?|council-setup|[0-9]{1,2}:(?:gate\\.[ABC]|F\\.2)|x-[0-9]{1,2}-[0-9]{1,3})$" },
    "itemId": { "type": "string", "pattern": "^(?:[a-z][a-z0-9-]{0,47}|xi-[0-9]{1,2}-[0-9]{1,3})$" },
    "repoPath": { "type": "string", "minLength": 1, "maxLength": 1024, "pattern": "^(?!/)(?!.*(?:^|/)\\.\\.?(?:/|$))[^\\u0000-\\u001f\\u007f\\\\]+$" },
    "conditionId": { "enum": ["idea_known", "commercial", "payments", "multi_user", "public", "has_ui", "others_data", "server", "has_database", "has_uploads", "has_email", "has_integrations"] },
    "triState": { "enum": ["yes", "no", "undecided"] },
    "model": { "enum": ["fable", "opus", "sonnet", "haiku", "opusplan"] },
    "effort": { "enum": ["low", "medium", "high", "xhigh", "max", "ultracode", "adaptive"] },
    "mode": { "enum": ["plan", "default", "acceptEdits", "auto", "dontAsk"] },
    "start": {
      "type": "object", "additionalProperties": false, "required": ["model", "effort", "mode"],
      "properties": { "model": { "$ref": "#/$defs/model" }, "effort": { "$ref": "#/$defs/effort" },
                      "mode": { "oneOf": [{ "$ref": "#/$defs/mode" }, { "type": "null" }] } }
    },
    "itemKind": { "type": "string", "pattern": "^(?:attest|time:[1-9][0-9]{0,2}d(?:>attest)?|auto:cmd=[a-z][a-z0-9-]{0,47})$" },
    "claudeUrl": { "type": "string", "pattern": "^https://claude\\.ai/[^\\s@]*$", "maxLength": 2048 },
    "shortText": { "type": "string", "maxLength": 200 },
    "note": { "type": "string", "maxLength": 4000 }
  }
}
```

### 3.1 `.foreman/project.json` · v1

Decision fields (ADR-022): `name`, `slug`, `specFolder`, `conditions`, `conditionAudit`, `modelPolicy`. Variable fields (auto-accepted on outside change, toast "Variables reloaded"): `variables`, `chatProjectUrl`, `chatUrls`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:project.v1",
  "type": "object",
  "additionalProperties": false,
  "required": ["schemaVersion", "projectId", "name", "slug", "specFolder", "variables", "conditions", "conditionAudit", "modelPolicy", "chatProjectUrl", "chatUrls"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "projectId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "name": { "$ref": "urn:foreman:common#/$defs/shortText", "minLength": 1 },
    "slug": { "type": "string", "pattern": "^[a-z0-9-]{1,40}$" },
    "specFolder": { "const": "spec" },
    "variables": {
      "type": "object",
      "propertyNames": { "pattern": "^[A-Z][A-Z0-9_]{0,63}$" },
      "additionalProperties": { "type": "string", "maxLength": 4000 }
    },
    "conditions": {
      "type": "object", "additionalProperties": false,
      "required": ["idea_known", "commercial", "payments", "multi_user", "public", "has_ui", "others_data", "server", "has_database", "has_uploads", "has_email", "has_integrations"],
      "propertyNames": { "$ref": "urn:foreman:common#/$defs/conditionId" },
      "additionalProperties": { "$ref": "urn:foreman:common#/$defs/triState" }
    },
    "conditionAudit": {
      "type": "array",
      "items": {
        "type": "object", "additionalProperties": false, "required": ["condition", "from", "to", "at", "note"],
        "properties": {
          "condition": { "$ref": "urn:foreman:common#/$defs/conditionId" },
          "from": { "$ref": "urn:foreman:common#/$defs/triState" },
          "to": { "$ref": "urn:foreman:common#/$defs/triState" },
          "at": { "$ref": "urn:foreman:common#/$defs/timestamp" },
          "note": { "$ref": "urn:foreman:common#/$defs/note" }
        }
      }
    },
    "modelPolicy": {
      "type": "object", "additionalProperties": false, "required": ["plan", "substitutions"],
      "properties": {
        "plan": { "enum": ["max", "pro"] },
        "substitutions": {
          "type": "array",
          "items": {
            "type": "object", "additionalProperties": false, "required": ["from", "to", "effort"],
            "properties": {
              "from": { "$ref": "urn:foreman:common#/$defs/model" },
              "to": { "$ref": "urn:foreman:common#/$defs/model" },
              "effort": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/effort" }, { "type": "null" }] }
            }
          }
        }
      }
    },
    "chatProjectUrl": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/claudeUrl" }, { "type": "null" }] },
    "chatUrls": {
      "type": "object",
      "propertyNames": { "$ref": "urn:foreman:common#/$defs/stepKey" },
      "additionalProperties": { "$ref": "urn:foreman:common#/$defs/claudeUrl" }
    }
  }
}
```

- `conditionAudit` is append-only and in time order (order is meaningful; FILE-R1's sort exception).
- `chatUrls` is keyed by the chat chain's start step (`"1.1"`, `"3.1"`, `"2:F.2"` is never a chain start). Chain UUIDs live in app data; the step key is stable across machines (PR-02 (g)).
- Foreman's own project: `conditions` = `idea_known: yes, has_ui: yes`, all others `no`; `modelPolicy` = `{ "plan": "max", "substitutions": [{ "from": "fable", "to": "opus", "effort": "xhigh" }] }` (MASTER-SPEC §5).
- Verify commands (r6 F-020) are v1.1 (F-101) and enter by a v1 → v2 migration; v1 has no such key (`additionalProperties: false`).

### 3.2 `.foreman/overlay.json` · v1

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:overlay.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "skips", "addedSteps", "addedItems"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "skips": {
      "type": "object",
      "propertyNames": { "$ref": "urn:foreman:common#/$defs/stepKey" },
      "additionalProperties": {
        "type": "object", "additionalProperties": false, "required": ["reason", "at"],
        "properties": {
          "reason": { "$ref": "urn:foreman:common#/$defs/shortText", "minLength": 1 },
          "at": { "$ref": "urn:foreman:common#/$defs/timestamp" }
        }
      }
    },
    "addedSteps": {
      "type": "array",
      "items": {
        "type": "object", "additionalProperties": false,
        "required": ["id", "stage", "after", "copiedFrom", "title", "tag", "text", "surface", "cwd", "start", "out"],
        "properties": {
          "id": { "type": "string", "pattern": "^x-[0-9]{1,2}-[0-9]{1,3}$" },
          "stage": { "$ref": "urn:foreman:common#/$defs/stage" },
          "after": { "$ref": "urn:foreman:common#/$defs/stepKey" },
          "copiedFrom": { "$ref": "urn:foreman:common#/$defs/stepKey" },
          "title": { "$ref": "urn:foreman:common#/$defs/shortText" },
          "tag": { "enum": ["optional", "core"] },
          "text": { "type": "string", "maxLength": 65536 },
          "surface": { "enum": ["chat", "cli", "desktop"] },
          "cwd": { "oneOf": [{ "enum": ["project", "spec"] }, { "type": "null" }] },
          "start": { "$ref": "urn:foreman:common#/$defs/start" },
          "out": { "type": "array", "items": { "$ref": "urn:foreman:common#/$defs/repoPath" } }
        }
      }
    },
    "addedItems": {
      "type": "array",
      "items": {
        "type": "object", "additionalProperties": false, "required": ["id", "stage", "text", "kind"],
        "properties": {
          "id": { "type": "string", "pattern": "^xi-[0-9]{1,2}-[0-9]{1,3}$" },
          "stage": { "$ref": "urn:foreman:common#/$defs/stage" },
          "text": { "$ref": "urn:foreman:common#/$defs/shortText" },
          "kind": { "const": "attest" }
        }
      }
    }
  }
}
```

Every field is a decision field. Semantic rules beyond the schema (no skip of a core or conditional step, no `bypassPermissions`, `out` paths pass `check_write`, added steps always run as new sessions) are domain-rules DR-058…DR-062. P1 writes only `skips`; `addedSteps` and `addedItems` stay `[]` until F-025 (M4).

### 3.3 `.foreman/blueprint.lock.json` · v1

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:blueprint-lock.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "blueprintRepoCommit", "blueprintPath", "sourceSha256", "blueprintVersion", "grammarVersion", "pinnedAt", "items"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "blueprintRepoCommit": { "$ref": "urn:foreman:common#/$defs/gitOid" },
    "blueprintPath": { "$ref": "urn:foreman:common#/$defs/repoPath" },
    "sourceSha256": { "$ref": "urn:foreman:common#/$defs/sha256" },
    "blueprintVersion": { "type": "string", "pattern": "^[0-9]+\\.[0-9]+(?:\\.[0-9]+)?$" },
    "grammarVersion": { "type": "integer", "minimum": 1 },
    "pinnedAt": { "$ref": "urn:foreman:common#/$defs/timestamp" },
    "items": {
      "type": "object",
      "propertyNames": { "$ref": "urn:foreman:common#/$defs/itemId" },
      "additionalProperties": {
        "type": "object", "additionalProperties": false, "required": ["textSha256", "kind"],
        "properties": {
          "textSha256": { "$ref": "urn:foreman:common#/$defs/sha256" },
          "kind": { "$ref": "urn:foreman:common#/$defs/itemKind" }
        }
      }
    }
  }
}
```

- Restore reads the blob with `git cat-file blob <commit>:<blueprintPath>` (or `show <rev>:<path>`, raw) through `git::run`; "dirty" at pin time is `hash-object --no-filters <file>` ≠ `rev-parse HEAD:<blueprintPath>`. Foreman never runs `git status` or `git diff` (SR-45).
- `sourceSha256` is over the **normalized** source (domain-rules DR-001), so CRLF, BOM and tab variants pin identically (AC-F001-08). `templates/<sourceSha256>.md` holds the exact blob from `git cat-file blob <commit>:<blueprintPath>`; restore re-normalizes and compares.
- `items` holds every inspection item of the template (all stages, overlay items excluded), keyed by item ID (template-unique, DR-034). Example entry for the Stage 5 gate's first bullet (DR-014):
  `"fresh-clone": { "kind": "attest", "textSha256": "6cfd6dda92a916f9c1c1b4f128dcd1839aacca3ed19508ec22e7d1cfb6b57640" }`.
- Every field is a decision field.

### 3.4 `.foreman/stages/stage-NN.json` · v1 (P2)

Written in signing phase 1, committed in the same plumbing commit, never rewritten (a re-sign of a stage isn't possible in v1).

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:stage-record.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "stage", "signedBy", "signedOn", "branch", "packet", "steps", "gate", "skipped", "substitutions", "git"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "stage": { "$ref": "urn:foreman:common#/$defs/stage" },
    "signedBy": { "$ref": "urn:foreman:common#/$defs/shortText" },
    "signedOn": { "$ref": "urn:foreman:common#/$defs/isoDate" },
    "branch": {
      "type": "object", "additionalProperties": false, "required": ["mode", "ref"],
      "properties": { "mode": { "enum": ["current", "other"] }, "ref": { "type": "string", "pattern": "^refs/heads/[^\\s~^:?*\\[\\\\]+$" } }
    },
    "packet": {
      "type": "object", "additionalProperties": false, "required": ["path", "sha256", "attachments"],
      "properties": {
        "path": { "$ref": "urn:foreman:common#/$defs/repoPath" },
        "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" },
        "attachments": { "oneOf": [{ "type": "object", "additionalProperties": false, "required": ["path", "sha256"],
          "properties": { "path": { "$ref": "urn:foreman:common#/$defs/repoPath" }, "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" } } }, { "type": "null" }] }
      }
    },
    "steps": {
      "type": "array",
      "items": {
        "type": "object", "additionalProperties": false,
        "required": ["step", "state", "runIds", "chainId", "intended", "observed", "promptSha256s", "activeMs"],
        "properties": {
          "step": { "$ref": "urn:foreman:common#/$defs/stepKey" },
          "state": { "enum": ["done", "skipped", "notApplicable"] },
          "reason": { "$ref": "urn:foreman:common#/$defs/shortText" },
          "runIds": { "type": "array", "items": { "$ref": "urn:foreman:common#/$defs/uuid" } },
          "chainId": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/uuid" }, { "type": "null" }] },
          "intended": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/start" }, { "type": "null" }] },
          "observed": { "oneOf": [{
            "type": "object", "additionalProperties": false, "required": ["model", "effort", "mode"],
            "properties": { "model": { "type": ["string", "null"], "maxLength": 100 },
                            "effort": { "type": ["string", "null"], "maxLength": 20 },
                            "mode": { "type": ["string", "null"], "maxLength": 20 } } }, { "type": "null" }] },
          "promptSha256s": { "type": "array", "items": { "$ref": "urn:foreman:common#/$defs/sha256" } },
          "activeMs": { "type": "integer", "minimum": 0 }
        }
      }
    },
    "gate": {
      "type": "array",
      "items": {
        "type": "object", "additionalProperties": false, "required": ["item", "kind", "outcome", "text"],
        "properties": {
          "item": { "$ref": "urn:foreman:common#/$defs/itemId" },
          "kind": { "$ref": "urn:foreman:common#/$defs/itemKind" },
          "outcome": { "enum": ["done", "notApplicable", "pass"] },
          "text": { "$ref": "urn:foreman:common#/$defs/note" },
          "link": { "type": "string", "maxLength": 2048 }
        }
      }
    },
    "skipped": { "type": "array", "items": { "type": "string", "maxLength": 400 } },
    "substitutions": { "type": "array", "items": { "type": "string", "maxLength": 400 } },
    "git": {
      "type": "object", "additionalProperties": false, "required": ["overrides", "signMode", "env"],
      "properties": {
        "overrides": { "type": "array", "items": { "type": "string" } },
        "signMode": { "const": "noGpgSign" },                       // v1 always commits with --no-gpg-sign (DR-144)
        "env": { "type": "array", "items": { "type": "string", "pattern": "^[A-Z_]+$" } }
      }
    }
  }
}
```

- `observed` is `null` without the sensor (P1 behavior, or S-1′ fail).
- **The commit OID is not in this file**: the record is part of the commit it would describe, so it can't contain that commit's OID. The OID is stored in app data (`progress.json` `signing[NN].commit`, §4.3), and the record at that commit is read with `cat-file blob <commit>:.foreman/stages/stage-NN.json` (ADR-012 decision 7; Foreman never runs `git log`).
- `skipped`, `substitutions` and `gate` carry exactly the lines Foreman writes into the packet's `SKIPPED:`, `SUBSTITUTIONS:` and `ATTESTED:` fields (§5.4).

## 4. App-data files

App data is `~/Library/Application Support/<bundle id>/` (`app_data_dir()`).

### 4.1 `settings.json` · v1 · class (f)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:settings.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "blueprintRepo", "driName", "tools", "editorBundleId", "theme", "terminal", "envPassthrough", "loginPath", "globalVariables", "authKeyAllowlist", "contract", "lastSeenClaudeVersion", "probeFolderTrusted"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "blueprintRepo": { "oneOf": [{ "type": "object", "additionalProperties": false, "required": ["path", "source"],
      "properties": { "path": { "type": "string", "pattern": "^/" }, "source": { "enum": ["autoDetect", "chosen"] } } }, { "type": "null" }] },
    "driName": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/shortText" }, { "type": "null" }] },
    "tools": {
      "type": "object", "additionalProperties": false, "required": ["claude", "git"],
      "properties": {
        "claude": { "$ref": "#/$defs/toolPath" },
        "git": { "$ref": "#/$defs/toolPath" }
      }
    },
    "editorBundleId": { "oneOf": [{ "type": "string", "pattern": "^[A-Za-z0-9-]+(\\.[A-Za-z0-9-]+){1,10}$" }, { "type": "null" }] },
    "theme": { "enum": ["system", "vellum", "night"] },
    "terminal": {
      "type": "object", "additionalProperties": false, "required": ["fontSize", "scrollback", "macOptionIsMeta"],
      "properties": { "fontSize": { "type": "integer", "minimum": 9, "maximum": 32 },
                      "scrollback": { "type": "integer", "minimum": 1000, "maximum": 100000 },
                      "macOptionIsMeta": { "type": "boolean" } }
    },
    "envPassthrough": { "type": "array", "uniqueItems": true, "items": { "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]{0,127}$" } },
    "loginPath": { "oneOf": [{ "type": "object", "additionalProperties": false, "required": ["entries", "shell", "capturedAt"],
      "properties": { "entries": { "type": "array", "items": { "type": "string", "pattern": "^/" } },
                      "shell": { "type": "string", "pattern": "^/" },
                      "capturedAt": { "$ref": "urn:foreman:common#/$defs/timestamp" } } }, { "type": "null" }] },
    "globalVariables": { "type": "object", "propertyNames": { "pattern": "^[A-Z][A-Z0-9_]{0,63}$" }, "additionalProperties": { "type": "string", "maxLength": 4000 } },
    "authKeyAllowlist": {
      "type": "array",
      "items": { "type": "object", "additionalProperties": false, "required": ["key", "sha256", "addedAt"],
                 "properties": { "key": { "type": "string", "maxLength": 200 },
                                 "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" },
                                 "addedAt": { "$ref": "urn:foreman:common#/$defs/timestamp" } } }
    },
    "contract": { "oneOf": [{ "type": "object", "additionalProperties": false, "required": ["version", "verifiedAt", "outcomes"],
      "properties": { "version": { "type": "string" }, "verifiedAt": { "$ref": "urn:foreman:common#/$defs/timestamp" },
                      "outcomes": { "type": "object", "additionalProperties": false, "required": ["probeA", "probeB"],
                                    "properties": { "probeA": { "$ref": "#/$defs/probeOutcome" }, "probeB": { "$ref": "#/$defs/probeOutcome" } } } } },
      { "type": "null" }] },
    "lastSeenClaudeVersion": { "oneOf": [{ "type": "string", "maxLength": 64 }, { "type": "null" }] },
    "probeFolderTrusted": { "type": "boolean" }
  },
  "$defs": {
    "toolPath": { "type": "object", "additionalProperties": false, "required": ["path", "source"],
      "properties": { "path": { "oneOf": [{ "type": "string", "pattern": "^/" }, { "type": "null" }] },
                      "source": { "enum": ["auto", "settings"] } } },
    "probeOutcome": { "type": "object", "additionalProperties": false, "required": ["result", "flag"],
      "properties": { "result": { "enum": ["pass", "flagFailure", "inconclusive"] },
                      "flag": { "type": ["string", "null"], "maxLength": 64 } } }
  }
}
```

- `authKeyAllowlist[].sha256` is `sha256("<key>=<canonical JSON value>")` for a key in **user** settings only (SR-07); the value itself is never stored.
- `lastSeenClaudeVersion` drives the "Claude Code updated X → Y" confirmation (SR-19); `contract.version` drives the probes (SR-17).
- **Proposal (v2, Stage 5):** add `"screenReaderMode": { "type": "boolean" }` (default `false`), the Settings toggle `settings_update` patches (ipc.md §2.4, AC-XC-22). Until the core's `SettingsFile` gains it with a v1→v2 migration (ADR-023), foreman-app keeps the toggle in memory only. Tracked in `docs/progress.md`.
- Missing file → defaults and auto-detection (AC-F029-03). Migration: ADR-023 chain; app data only, no approval path.

### 4.2 `projects.json` · v1 · class (f)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:projects.v1",
  "type": "object", "additionalProperties": false, "required": ["schemaVersion", "projects"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "projects": {
      "type": "array",
      "items": {
        "type": "object", "additionalProperties": false, "required": ["projectId", "name", "path", "addedAt", "pinned"],
        "properties": {
          "projectId": { "$ref": "urn:foreman:common#/$defs/uuid" },
          "name": { "$ref": "urn:foreman:common#/$defs/shortText" },
          "path": { "type": "string", "pattern": "^/", "maxLength": 4096 },
          "addedAt": { "$ref": "urn:foreman:common#/$defs/timestamp" },
          "pinned": { "oneOf": [{ "type": "integer", "minimum": 1, "maximum": 9 }, { "type": "null" }] }
        }
      }
    }
  }
}
```

`path` is the canonical absolute project root recorded at setup (ADR-011: the root's ancestors may be symlinks Billy chose; the canonical form is shown). `projectId` equals `.foreman/project.json` `projectId`; a mismatch at open → read-only with "This folder belongs to another Foreman project".

### 4.3 `projects/<projectId>/progress.json` · v1 · class (b)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:progress.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "projectId", "rev", "steps", "chains", "runInputs", "remembered", "optionalParts", "overrides", "gateRuns", "pendingOutside", "pendingOwn", "readStamps", "firstLaunchDone", "signing", "waits", "indexRefreshPending"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "projectId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "rev": { "type": "integer", "minimum": 0, "maximum": 4294967295 },
    "steps": {
      "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/stepKey" },
      "additionalProperties": {
        "type": "object", "additionalProperties": false, "required": ["state", "runCount", "lastRunId", "startedAt", "copiedAt"],
        "properties": {
          "state": { "enum": ["notStarted", "copied", "running", "done"] },
          "runCount": { "type": "integer", "minimum": 0 },
          "lastRunId": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/uuid" }, { "type": "null" }] },
          "startedAt": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/timestamp" }, { "type": "null" }] },
          "copiedAt": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/timestamp" }, { "type": "null" }] }
        }
      }
    },
    "chains": {
      "type": "array",
      "items": {
        "type": "object", "additionalProperties": false, "required": ["id", "surface", "cwd", "start", "steps", "status", "forkedFrom", "lastStart"],
        "properties": {
          "id": { "$ref": "urn:foreman:common#/$defs/uuid" },
          "surface": { "enum": ["chat", "cli"] },
          "cwd": { "oneOf": [{ "enum": ["project", "spec"] }, { "type": "null" }] },
          "start": { "$ref": "urn:foreman:common#/$defs/stepKey" },
          "steps": { "type": "array", "items": { "$ref": "urn:foreman:common#/$defs/stepKey" } },
          "status": { "enum": ["open", "closed", "superseded", "handedOff", "broken"] },
          "forkedFrom": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/uuid" }, { "type": "null" }] },
          "lastStart": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/start" }, { "type": "null" }] }
        }
      }
    },
    "runInputs": { "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/stepKey" },
                   "additionalProperties": { "type": "object", "additionalProperties": { "type": "string", "maxLength": 1048576 } } },
    "remembered": { "type": "object", "propertyNames": { "pattern": "^[A-Za-z][A-Za-z0-9_]{0,63}$" }, "additionalProperties": { "type": "string", "maxLength": 1048576 } },
    "optionalParts": { "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/stepKey" },
                       "additionalProperties": { "type": "object", "propertyNames": { "pattern": "^opt-[0-9]{1,2}$" }, "additionalProperties": { "type": "boolean" } } },
    "overrides": { "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/stepKey" },
                   "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["model", "effort", "mode"],
                     "properties": { "model": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/model" }, { "type": "null" }] },
                                     "effort": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/effort" }, { "type": "null" }] },
                                     "mode": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/mode" }, { "type": "null" }] } } } },
    "gateRuns": { "type": "object", "propertyNames": { "pattern": "^[0-9]{1,2}$" },
                  "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["state", "items", "failures"],
                    "properties": {
                      "state": { "enum": ["notReady", "a", "results", "b", "packetValid", "c", "lessonsFound", "signable"] },
                      "items": { "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/itemId" },
                                 "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["outcome", "text", "at"],
                                   "properties": { "outcome": { "enum": ["done", "notApplicable", "pass", "fail"] },
                                                   "text": { "$ref": "urn:foreman:common#/$defs/note" },
                                                   "link": { "type": "string", "maxLength": 2048 },
                                                   "at": { "$ref": "urn:foreman:common#/$defs/timestamp" } } } },
                      "failures": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0 } } } } },
    "pendingOutside": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["id", "file", "seenSha256", "detectedAt", "diff"],
      "properties": { "id": { "$ref": "urn:foreman:common#/$defs/uuid" }, "file": { "enum": ["project", "overlay", "blueprintLock"] },
                      "seenSha256": { "$ref": "urn:foreman:common#/$defs/sha256" }, "detectedAt": { "$ref": "urn:foreman:common#/$defs/timestamp" },
                      "diff": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["keyPath", "change"],
                        "properties": { "keyPath": { "type": "string" }, "change": { "enum": ["added", "removed", "valueChanged"] },
                                        "before": { "type": ["string", "null"] }, "after": { "type": ["string", "null"] } } } } } } },
    "pendingOwn": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["file", "sha256", "at"],
      "properties": { "file": { "enum": ["project", "overlay", "blueprintLock"] }, "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" },
                      "at": { "$ref": "urn:foreman:common#/$defs/timestamp" } } } },
    "readStamps": { "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/repoPath" },
                    "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["dev", "ino", "size", "mtimeNs", "sha256"],
                      "properties": { "dev": { "type": "integer", "minimum": 0 }, "ino": { "type": "integer", "minimum": 0 },
                                      "size": { "type": "integer", "minimum": 0 }, "mtimeNs": { "type": "integer" },
                                      "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" } } } },
    "firstLaunchDone": { "type": "boolean" },
    "signing": { "type": "object", "propertyNames": { "pattern": "^[0-9]{1,2}$" },
                 "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["state", "commit", "branch", "packetSha256", "at"],
                   "properties": { "state": { "enum": ["commitPending", "signed", "signedOtherBranch", "handSigned"] },
                                   "commit": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/gitOid" }, { "type": "null" }] },
                                   "branch": { "type": "string" }, "packetSha256": { "$ref": "urn:foreman:common#/$defs/sha256" },
                                   "at": { "$ref": "urn:foreman:common#/$defs/timestamp" } } } },
    "waits": { "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/stepKey" },
               "additionalProperties": { "$ref": "urn:foreman:common#/$defs/isoDate" } },
    "indexRefreshPending": { "oneOf": [{ "type": "null" }, { "type": "object", "additionalProperties": false, "required": ["commit", "paths", "since"],
                               "properties": { "commit": { "$ref": "urn:foreman:common#/$defs/gitOid" },
                                               "paths": { "type": "array", "items": { "$ref": "urn:foreman:common#/$defs/repoPath" } },
                                               "since": { "$ref": "urn:foreman:common#/$defs/timestamp" } } }] }
  }
}
```

- Derived states (`skipped`, `notApplicable`, `undecided`, `needsAnnotation`, `needsNewerForeman`, `waitingUntil`) are **not stored**: they're computed from the template, conditions, overlay and `waits` on every read (DR-050), so they can't go stale. Stored `state` is only what Billy's actions produced.
- `readStamps` holds the stamp of every repo file Foreman may write next (ADR-010). `dev`, `ino`, `mtimeNs` are 64-bit; this file is never sent to the webview whole (IPC-R4 doesn't apply).
- `signing[NN].commit` is where the signing commit's OID lives (the stage record can't hold it, because it is inside that commit; ADR-012 decision 7, DR-143).
- `indexRefreshPending` is set when a commit moved the ref but the real-index refresh failed after 5 retries on `index.lock` (ADR-012). Foreman retries on project open and window focus and shows "Index not refreshed; don't commit until this clears" until it succeeds; then it goes back to `null`.
- `handSigned` marks a P1 packet whose `APPROVED BY` was filled in outside Foreman **and** confirmed by Billy in the native dialog (AC-F013-09, SR-47); it's derived from `approvals/<id>.json` `outsideSignatures` and `commit` is `null` for it. A packet with a name in it but no confirmation is "signature not confirmed", not signed.
- Rebuild (NFR-17, AC-F020-11): from `docs/handoff/` packets and `.foreman/stages/` records; chains, run inputs and overrides are lost.

### 4.4 `runs/<runId>/settings.json` · Claude Code settings format · class (b)

This file is **Claude Code's** settings format (passed as `--settings`), so it has **no `schemaVersion`** (FILE-R2 exception): an unknown top-level key could be rejected or warned about by Claude Code. Foreman writes it once, before spawn, records its SHA-256 in `run.json`, and never reads it back. Its shape is pinned by `run_settings::contents_snapshot` and parsed by the S-1′ probe.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:run-settings.v1",
  "type": "object", "additionalProperties": false,
  "required": ["disableAllHooks", "permissions"],
  "properties": {
    "disableAllHooks": { "const": false },
    "permissions": {
      "type": "object", "additionalProperties": false, "required": ["disableBypassPermissionsMode", "deny"],
      "properties": {
        "disableBypassPermissionsMode": { "const": "disable" },
        "deny": { "type": "array", "minItems": 4, "maxItems": 4, "prefixItems": [
          { "type": "string", "pattern": "^Read\\(//.+/\\.foreman/\\*\\*\\)$" },
          { "type": "string", "pattern": "^Edit\\(//.+/\\.foreman/\\*\\*\\)$" },
          { "type": "string", "pattern": "^Read\\(//.+/\\*\\*\\)$" },
          { "type": "string", "pattern": "^Edit\\(//.+/\\*\\*\\)$" } ] }
      }
    },
    "effortLevel": { "enum": ["low", "medium", "high", "xhigh"] },
    "hooks": {
      "type": "object", "additionalProperties": false,
      "propertyNames": { "enum": ["SessionStart", "Stop", "StopFailure", "Notification", "PermissionRequest", "SessionEnd", "SubagentStart", "SubagentStop", "TaskCreated", "TaskCompleted", "PostToolUse"] },
      "additionalProperties": {
        "type": "array", "minItems": 1, "maxItems": 1,
        "items": { "type": "object", "additionalProperties": false, "required": ["hooks"],
          "properties": {
            "matcher": { "const": "Bash" },
            "hooks": { "type": "array", "minItems": 1, "maxItems": 1, "items": {
              "type": "object", "additionalProperties": false, "required": ["type", "command", "args"],
              "properties": { "type": { "const": "command" },
                              "command": { "type": "string", "pattern": "^/.+/bin/foreman-sensor-[0-9a-f]{12}$" },
                              "args": { "type": "array", "minItems": 8, "maxItems": 8, "items": { "type": "string" } } } } } } }
      }
    }
  }
}
```

- Deny-rule paths are `//` + the canonical absolute repo or app-data path without its leading `/`, i.e. `Read(//Users/billy/dev/tradelog/.foreman/**)` (ADR-021). Glob metacharacters in a path are escaped or the launch blocks (`Blocker::RunSettingsPathUnsafe`).
- `effortLevel` appears only on the S-3 fail branch for a resumed step whose effort is `low`…`xhigh` (`max` and `ultracode` aren't accepted by `effortLevel`, per the Claude Code model-config docs, 2026-10-19; those fall to the shown `/effort` line, DR-117).
- `hooks` appears only in P2 with a hash-verified sensor; `args` = `["--run", <runId>, "--token", <64 hex>, "--event", <EventName>, "--v", "1"]`. `matcher: "Bash"` only on `PostToolUse` (ADR-009).
- Retention: deleted with its run (Clear run history). It contains the per-run token (SR-15), which is why it's `0600` in app data only.

### 4.5 `approvals/<projectId>.json` · v1 · class (b)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:approvals.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "projectId", "files", "itemKinds", "envApprovals", "outsideSignatures"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "projectId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "files": {
      "type": "object", "additionalProperties": false, "required": ["project", "overlay", "blueprintLock"],
      "properties": {
        "project": { "$ref": "#/$defs/approval" }, "overlay": { "$ref": "#/$defs/approval" }, "blueprintLock": { "$ref": "#/$defs/approval" }
      }
    },
    "itemKinds": { "type": "object", "propertyNames": { "$ref": "urn:foreman:common#/$defs/itemId" },
      "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["textSha256", "kind", "at"],
        "properties": { "textSha256": { "$ref": "urn:foreman:common#/$defs/sha256" }, "kind": { "$ref": "urn:foreman:common#/$defs/itemKind" },
                        "at": { "$ref": "urn:foreman:common#/$defs/timestamp" } } } },
    "envApprovals": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["file", "sha256", "at"],
      "properties": { "file": { "type": "string" }, "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" }, "at": { "$ref": "urn:foreman:common#/$defs/timestamp" } } } },
    "outsideSignatures": { "type": "object", "propertyNames": { "pattern": "^[0-9]{1,2}$" },
      "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["packetSha256", "signedBy", "signedOn", "at"],
        "properties": { "packetSha256": { "$ref": "urn:foreman:common#/$defs/sha256" }, "signedBy": { "$ref": "urn:foreman:common#/$defs/shortText" },
                        "signedOn": { "$ref": "urn:foreman:common#/$defs/isoDate" }, "at": { "$ref": "urn:foreman:common#/$defs/timestamp" } } } }
  },
  "$defs": {
    "approval": { "type": "object", "additionalProperties": false, "required": ["approvedSha256", "committedSha256", "committedOnOtherBranches"],
      "properties": { "approvedSha256": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/sha256" }, { "type": "null" }] },
                      "committedSha256": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/sha256" }, { "type": "null" }] },
                      "committedOnOtherBranches": { "type": "object", "propertyNames": { "pattern": "^refs/heads/[^\\s]{1,200}$" },
                                                    "additionalProperties": { "$ref": "urn:foreman:common#/$defs/sha256" } } } }
  }
}
```

- `approvedSha256` is the hash of the effective (approved) copy in `approvals/<projectId>/<file>.json`; `committedSha256` is the last version committed to the repo. Before Stage 5 is signed they're equal after every Foreman change; afterwards Foreman's own changes make `approved` run ahead of `committed` until the signing commit, and the drift check compares the repo file against `committedSha256` (ADR-022 §5). `committedOnOtherBranches` keeps, per branch ref, what a signing on another branch (SR-43) committed; while HEAD names one of those branches (`symbolic-ref -q HEAD`), the drift check compares against that branch's entry instead, so checking out the signed branch doesn't raise a false pending change (ADR-022 §5).
- `envApprovals` is used only on the S-7′ fail branch (AC-F008-07).
- `outsideSignatures` records Billy's native confirmation (`packet_signature_confirm`, SR-24a, SR-47) that a packet signed outside Foreman's signing flow is his. It binds the packet's SHA-256: if the file changes, the stage no longer counts as signed until he confirms again (DR-135).
- Lost app data: rebuilt by treating HEAD's committed decision files as approved after a one-time review prompt (ADR-022 Consequences).

### 4.6 `runs/<runId>/run.json` · v1 · class (b)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:run.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "runId", "projectId", "stage", "stepId", "surface", "kind", "chainId", "name", "argv", "envNames", "cwd", "promptSha256", "runSettingsSha256", "intended", "override", "observed", "entryPoint", "t", "timeToStartMs", "runInputInterventions", "copyPromptOnCli", "status", "exitCode", "claudeVersion", "pid", "sensorToken"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "runId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "projectId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "stage": { "$ref": "urn:foreman:common#/$defs/stage" },
    "stepId": { "$ref": "urn:foreman:common#/$defs/stepKey" },
    "surface": { "enum": ["chat", "cli", "desktop", "probe"] },
    "kind": { "enum": ["launch", "rearm", "followupCopy", "chatCopy", "desktop", "probe"] },
    "chainId": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/uuid" }, { "type": "null" }] },
    "name": { "oneOf": [{ "type": "string", "pattern": "^fm-[a-z0-9-]{1,80}$" }, { "type": "null" }] },
    "argv": { "type": "array", "items": { "type": "string", "not": { "pattern": "[\\u0000-\\u0008\\u000b-\\u001f]" } } },
    "envNames": { "type": "array", "uniqueItems": true, "items": { "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]*$" } },
    "cwd": { "oneOf": [{ "enum": ["project", "spec", "probe"] }, { "type": "null" }] },
    "promptSha256": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/sha256" }, { "type": "null" }] },
    "runSettingsSha256": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/sha256" }, { "type": "null" }] },
    "intended": { "oneOf": [{ "$ref": "urn:foreman:common#/$defs/start" }, { "type": "null" }] },
    "override": { "type": "boolean" },
    "observed": { "oneOf": [{ "type": "object", "additionalProperties": false, "required": ["model", "effort", "mode"],
      "properties": { "model": { "type": ["string", "null"] }, "effort": { "type": ["string", "null"] }, "mode": { "type": ["string", "null"] } } }, { "type": "null" }] },
    "entryPoint": { "enum": ["doNext", "palette", "stageView", "rearm", "repair", "probe"] },
    "t": {
      "type": "object", "additionalProperties": false, "required": ["wallStart", "lookStart", "prepared", "confirmed", "spawned", "exited"],
      "properties": {
        "wallStart": { "$ref": "urn:foreman:common#/$defs/timestamp" },
        "lookStart": { "type": ["integer", "null"], "minimum": 0 },
        "prepared": { "type": ["integer", "null"], "minimum": 0 },
        "confirmed": { "type": ["integer", "null"], "minimum": 0 },
        "spawned": { "type": ["integer", "null"], "minimum": 0 },
        "exited": { "type": ["integer", "null"], "minimum": 0 }
      }
    },
    "timeToStartMs": { "type": ["integer", "null"], "minimum": 0 },
    "runInputInterventions": { "type": "integer", "minimum": 0 },
    "copyPromptOnCli": { "type": "boolean" },
    "status": { "enum": ["running", "exited", "errored", "interrupted", "copied"] },
    "exitCode": { "type": ["integer", "null"] },
    "claudeVersion": { "type": ["string", "null"], "maxLength": 64 },
    "pid": { "type": ["integer", "null"], "minimum": 1 },
    "sensorToken": { "oneOf": [{ "type": "string", "pattern": "^[0-9a-f]{64}$" }, { "type": "null" }] }
  }
}
```

- `argv` replaces the final positional prompt with the literal string `<prompt sha256:<64 hex>>` (SR-52). `envNames` holds names only.
- `t.*` are milliseconds since `wallStart` (monotonic clock). `lookStart` is when Foreman was focused or Do next was pressed (MASTER-SPEC §9); for chat runs `spawned` is the Copy-and-open moment.
- `pid` plus `t.wallStart` identify the process at restart: a `running` record whose pid is gone or belongs to a later-started process becomes `interrupted` (ADR-010 §6).
- `sensorToken` (ADR-009) is `null` in P1 and when the sensor isn't used.

### 4.7 `runs/index-YYYY-MM.jsonl` · v1 per line · class (b)

One JSON object per line, appended at run exit (or at Copy-and-open for chat). `YYYY-MM` is the UTC month of `wallStart`. A torn last line is skipped on read.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:run-index-line.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "runId", "projectId", "stage", "stepId", "surface", "entryPoint", "wallStart", "timeToStartMs", "durationMs", "waitMs", "status", "runInputInterventions", "copyPromptOnCli", "override"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "runId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "projectId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "stage": { "$ref": "urn:foreman:common#/$defs/stage" },
    "stepId": { "$ref": "urn:foreman:common#/$defs/stepKey" },
    "surface": { "enum": ["chat", "cli", "desktop"] },
    "entryPoint": { "enum": ["doNext", "palette", "stageView", "rearm", "repair"] },
    "wallStart": { "$ref": "urn:foreman:common#/$defs/timestamp" },
    "timeToStartMs": { "type": ["integer", "null"], "minimum": 0 },
    "durationMs": { "type": ["integer", "null"], "minimum": 0 },
    "waitMs": { "type": "integer", "minimum": 0 },
    "status": { "enum": ["exited", "errored", "interrupted", "copied"] },
    "runInputInterventions": { "type": "integer", "minimum": 0 },
    "copyPromptOnCli": { "type": "boolean" },
    "override": { "type": "boolean" }
  }
}
```

Each line is ≤ 4 KB (one `write`, FILE-R4); a typical line is about 300 bytes. Migration: a reader accepts lines of every historical `schemaVersion` and migrates each line in memory; lines are never rewritten (append-only).

### 4.8 `events/<runId>/<seq>-<event>.json` · v1 · class (d) · P2

Written by `foreman-sensor` with `O_CREAT|O_EXCL|O_NOFOLLOW`, `0600` (SR-15). ADR-009's shape plus `schemaVersion` (ADR-023 applies to every file; ADR-009 omits it).

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:sensor-event.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "token", "event", "receivedAt", "truncated", "payload"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "token": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
    "event": { "enum": ["SessionStart", "Stop", "StopFailure", "Notification", "PermissionRequest", "SessionEnd", "SubagentStart", "SubagentStop", "TaskCreated", "TaskCompleted", "PostToolUse"] },
    "receivedAt": { "$ref": "urn:foreman:common#/$defs/timestamp" },
    "truncated": { "type": "boolean" },
    "payload": { "type": ["object", "null"] }
  }
}
```

`payload` is Claude Code's hook input (≤ 256 KB of stdin; `null` with `truncated: true` if it was cut and no longer parses). It's untrusted (T5): ingest reads only the fields named in ADR-009 and ignores the rest. Retention: ingest prunes to the last 20 files per run; all events of a stage's runs are deleted at that stage's sign or 30 days after the run ends (SR-53). Migration: none needed (short-lived); a newer `schemaVersion` is ignored with a log line.

### 4.9 `cache/fingerprints/<cwdHash>.json` · v1 · class (e)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:foreman:fingerprint-cache.v1",
  "type": "object", "additionalProperties": false,
  "required": ["schemaVersion", "projectId", "cwd", "acked", "stat", "auth"],
  "properties": {
    "schemaVersion": { "const": 1 },
    "projectId": { "$ref": "urn:foreman:common#/$defs/uuid" },
    "cwd": { "type": "string", "pattern": "^/" },
    "acked": { "oneOf": [{ "type": "object", "additionalProperties": false, "required": ["fingerprint", "at", "snapshot"],
      "properties": {
        "fingerprint": { "$ref": "urn:foreman:common#/$defs/sha256" },
        "at": { "$ref": "urn:foreman:common#/$defs/timestamp" },
        "snapshot": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["source", "path", "sha256", "redacted"],
          "properties": { "source": { "enum": ["userSettings", "managedSettings", "managedDropIn", "claudeJsonMcp", "userMemory", "userDir", "claudeMd", "claudeLocalMd", "import", "projectClaudeDir", "mcpJson", "plugin", "program"] },
                          "path": { "type": "string" }, "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" },
                          "redacted": {} } } } } }, { "type": "null" }] },
    "stat": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["path", "dev", "ino", "size", "mtimeNs", "sha256"],
      "properties": { "path": { "type": "string" }, "dev": { "type": "integer" }, "ino": { "type": "integer" }, "size": { "type": "integer" },
                      "mtimeNs": { "type": "integer" }, "sha256": { "$ref": "urn:foreman:common#/$defs/sha256" } } } },
    "auth": { "oneOf": [{ "type": "object", "additionalProperties": false, "required": ["fingerprint", "claudeVersion", "envKey", "authMethod", "exit", "at"],
      "properties": { "fingerprint": { "$ref": "urn:foreman:common#/$defs/sha256" }, "claudeVersion": { "type": "string" },
                      "envKey": { "$ref": "urn:foreman:common#/$defs/sha256" }, "authMethod": { "type": ["string", "null"], "maxLength": 64 },
                      "exit": { "type": "integer" }, "at": { "$ref": "urn:foreman:common#/$defs/timestamp" } } }, { "type": "null" }] }
  }
}
```

- `redacted` is the canonical parsed JSON of a settings-like source after `fingerprint::redact` (values under `env`/`headers`, `apiKey*` keys, keys containing `token`/`secret`/`password`, and secret-pattern strings become `‹redacted sha256:<8 hex>›`), or `null` for non-JSON sources (Markdown, programs), which are diffed by hash only (SR-55).
- From `~/.claude.json`, only `mcpServers` (user scope and `projects[<repo path>]`) is ever stored (SR-31a).
- `envKey` = SHA-256 over the sorted env **names** and the SHA-256 of their values (never the values themselves), leaving out `FOREMAN_RUN_ID`, which is new every run (amended 2026-10-20, ADR-021).
- Retention: until the project is removed (PR-04).

### 4.10 `templates/<sourceSha256>.p<parserVersion>.json` · v1 · class (b)

The parsed `Template` (design doc §3.2.1) serialized with serde, keyed by source hash and Foreman's parser version (ADR-023 §5). Its schema is generated from the Rust types (`schemars` isn't a dependency, so the test `template::cache_roundtrip` is the contract: deserialize(serialize(t)) == t and the file has `schemaVersion: 1`). A cache that fails to load is deleted and re-parsed; it's never migrated.

### 4.11 `probe/settings.json` · Claude Code settings format · P2

Same schema as §4.4 with exactly one hook (`SessionStart`, the hash-checked sensor run with a fresh probe run ID and token), no `effortLevel`; its deny rules name the probe folder and app data. The marker is the `SessionStart` event file the sensor writes to `events/<probeRunId>/` with that token (amended 2026-10-20, ADR-017: it reuses the sensor's checked writer instead of a separate marker file). Recreated with the folder before each probe run (SR-17).

## 5. Handoff packet text format

Packets are Markdown files Foreman parses (`core::packet`), never JSON. Path: `docs/handoff/stage-NN.md`, `NN` two-digit zero-padded (`stage-02.md`); the paste-back remainder goes to `docs/handoff/stage-NN-attachments.md`. Read cap 1 MB (SR-31).

### 5.1 Grammar (after normalization, §5.2)

```text
packet      = header LF field+ [trailing]
header      = "=== HANDOFF PACKET · Stage " N " → Stage " M " ==="     N, M decimal, no padding
field       = KEY ":" [ " " first-line ] LF continuation*
KEY         = one of the stage format's keys (DR-017), or an EXTRA key
EXTRA       = [A-Z][A-Z0-9 ()+/&.-]{0,59}  at column 1, followed by ":"   (warning: "extra field")
continuation= ( SP+ text | "" ) LF                    indented lines and blank lines inside a field
trailing    = text after the last field of the format  (only in a paste; goes to attachments)
```

- A field's value is its first line plus continuation lines up to the next line that starts at column 1 with a known or EXTRA key, or the end of the file. Trailing blank lines are trimmed from each value. This matches Foreman's own packets (`docs/handoff/stage-01.md`…`03`), which indent continuations by 2 or 5 spaces and put blank lines between fields.
- Keys are matched exactly after normalization (case-sensitive: the Stage 2 format's key is `NFRs`, with a lowercase `s`).
- A duplicate key is a problem (`packet.duplicate-field`).

### 5.2 Normalization (`packet::normalize_packet`, DR-131)

Applied to pasted text and offered (shown, then written on confirm, AC-F013-08) for files Claude wrote:

| Input | Normalized | Note recorded |
|---|---|---|
| BOM; CRLF or CR line endings | removed; LF | — |
| Wrapping code fence (```` ``` ```` or ```` ```text ````) around a pasted packet | removed | "removed code fence" |
| NBSP (U+00A0), narrow NBSP (U+202F) | space | "fixed non-breaking spaces" |
| `“ ” ‘ ’` | `" " ' '` | "fixed smart quotes" |
| Trailing whitespace on a line | removed | — |
| Header with `->` for `→`, `-`/`•`/`\|` for `·`, or extra spaces | canonical header | "normalized header" |
| `APPROVED BY:` empty, `<blank>`, `<leave blank…>`, `<leave blank for me to sign>`, `<sign here…>`, `[blank]` (any case) | `APPROVED BY: <blank>` | "unsigned" |
| `OPEN ITEMS:` `None`, `NONE`, `none.`, `None.` | `OPEN ITEMS: none` | "OPEN ITEMS normalized to none" |

Normalization never changes any other field's content. Bidi and zero-width characters are **not** removed; they're shown as visible escapes and flagged (SR-26).

### 5.3 Signature forms

- Unsigned: `APPROVED BY: <blank>` (the only stored unsigned form, blueprint v2.2 B-6a).
- Signed: `APPROVED BY: <name> on <YYYY-MM-DD>`, where `<name>` is 1–80 characters without `<`, `>` or line breaks. An optional trailing parenthetical (`(…)`, ≤ 200 characters) is accepted with a warning, because Foreman's own `docs/handoff/stage-01.md` was signed as `Billy Ribeiro on 2026-10-19 (approved in chat: "…")` (DR-134).
- The signed **form** is not a signature. A packet counts as signed only when Foreman's signing wrote it (P2) or Billy confirmed an outside signature in the native dialog, recorded with the packet hash in `approvals/<id>.json` `outsideSignatures` (SR-24a, SR-47, DR-135).

### 5.4 Fields Foreman adds at signing (P2)

Written by `sign_apply` phase 1, deterministically, as the last fields before `OPEN ITEMS:` (or replacing fields of the same name, whose old content is shown in the sign diff). Each list is sorted by stage-step order; an empty list is written as `none`.

```text
SKIPPED:
  - 2.1 Competitor teardown: not applicable (commercial = no)
  - 1.4 part "optional 1": skipped: Include turned off
  - 3.2 part "fm:if commercial|others_data|multi_user": not applicable (all false)
SUBSTITUTIONS:
  - 2.2: fable@high/plan → opus@xhigh/plan (substitution fable → opus@xhigh)
  - 2.4: opus@xhigh/acceptEdits → opus@max/acceptEdits (override)
ATTESTED:
  - feature-matrix: done: matrix in spec/council/r1-engineering.md §3, 7 tools, sources linked
  - beats-competitor: not applicable: personal tool, no competitors (commercial = no)
APPROVED BY: Billy Ribeiro on 2026-10-20
```

- `SKIPPED:` lists every skipped or not-applicable step and part on the stage's applicable path, plus not-applicable whole stages queued from earlier stages (DR-055).
- `SUBSTITUTIONS:` lists every step whose effective start differs from the template start, with the origin (alternative rule, substitution, override) (DR-104).
- `ATTESTED:` lists every `attest` item with its outcome and note or reason; `time` items as `waited until <date>`.
- `APPROVED BY:` is replaced in place (it stays on line 3 of the packet). The packet SHA-256 after these writes is stored in the stage record and in `progress.json`.
