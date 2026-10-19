# ADR-011: Path policy: a lexical check with NFC and case folding, plus a component-wise fd walk with rustix

## Status
Proposed

## Date
2026-10-19

## Context

Revised `docs/requirements.md`:

- **SR-30 (writes):**
  - repo-relative only; no `..`, absolute paths, NUL, control or bidi characters;
  - deny components at any depth: `.git`, `.claude` (except the hard-coded `council-setup` destinations), `.husky`, `.vscode`, `.idea`, `.github`, `.foreman` (except the decision-file writers);
  - deny the file names `.mcp.json`, `CLAUDE.local.md`, `.envrc`, `.gitmodules`, `.gitattributes`, `.npmrc`;
  - `CLAUDE.md` only at the root, through the assembler, with confirmation;
  - **NFC normalization and full case folding on each component, and a check of the name the file system reports for an existing entry** (APFS is case- and normalization-insensitive by default);
  - component-wise resolution through a directory handle with symlink refusal;
  - the final path inside the project root or app data.
- **SR-31 (reads):** the same component-wise resolution, refusing symlinks; refuse non-regular files and files with more than one hard link; size caps (packets, lessons, decision files 1 MB; Claude config 256 KB, where a larger file blocks the launch); `lstat` checks of the watch root and its ancestors.
- **SR-31a:** a fixed list of reads outside projects, in one module.
- **SR-50:** secret patterns are never opened or watched.

R-11 left the crate choice (`cap-std` vs `openat`-style with `O_NOFOLLOW`) to this ADR.

Checked 2026-10-19:

- **`cap-std`/`cap-primitives`/`cap-fs-ext` 4.0.3** (2026-08-20): `DirExt::open_dir_nofollow` "fails if the path names a symlink"; `FollowSymlinks` applies "in the last component of a path". Intermediate in-sandbox symlinks are followed.
- **`rustix` 1.1.5:** safe `openat`, `OFlags::{NOFOLLOW, DIRECTORY, CLOEXEC}`, Apple `OFlags::NOFOLLOW_ANY`, `fs::getpath` (`fcntl(F_GETPATH)`), `statat`/`fstat`, `renameat_with`, `unlinkat`, `mkdirat`.
- **`icu_normalizer` 2.3.0** (`ComposingNormalizerBorrowed::new_nfc().normalize(&str)`) is already in the macOS graph through `url` → `idna`.
- **`icu_casemap` 2.3.0:** `CaseMapperBorrowed::fold_string(&str)` (full case folding).
- `openat` 0.1.21: last release 2021.

## Decision

Both layers live in `foreman-core`.

**1. Lexical (`pathpolicy`, pure):**

- `check_write(rel, purpose) -> Result<RepoPath, PolicyViolation>`:
  - UTF-8; no NUL; no C0, C1 or DEL; no bidi controls (U+202A–U+202E, U+2066–U+2069, U+200E/F, U+061C); no zero-width characters (U+200B–U+200D, U+2060, U+FEFF); no backslash;
  - split on `/`; reject empty, `.` and `..`;
  - each component is compared to the deny names by `key(c) = fold_string(nfc(c))` (ICU4X full case folding after NFC), so `.GIT`, `Claude.MD`, a Kelvin-sign `.husKy` and NFD variants all match.

  Exceptions are typed `WritePurpose` variants, never strings from content:
  - `CouncilSetup(AgentName)` → the fixed set `spec/.claude/agents/<known>.md`;
  - `DecisionFile(Kind)` → `.foreman/{project,overlay,blueprint.lock}.json` and `.foreman/stages/stage-NN.json`;
  - `AssemblerClaudeMd` → root `CLAUDE.md`, only with a UI confirmation token;
  - `GitExclude` → handled by the dedicated exclude writer (SR-32, ADR-010), never by `check_write`.
- `check_read(rel)`: the same syntax rules, plus the SR-50 secret patterns (`.env*`, `*.pem`, `*.key`, `id_*`, `*.p12`, `.npmrc`, `.netrc`, and `.ssh/`, `.aws/`, `.config/gcloud/` components), compared on the folded key.

**2. Resolution (`store::Root`, rustix):**

- `Root::open(abs, kind)` opens a registered root with `O_DIRECTORY|O_CLOEXEC`. The root's own ancestors may be symlinks, because Billy chose the path. They are `lstat`-checked and the canonical path is recorded and shown (SR-31's watch-root check uses the same function).
- **Intermediate components:** `openat(dirfd, comp, O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC)`. A symlink yields `ELOOP`/`ENOTDIR` → `PolicyViolation::Symlink(component)`. Missing directories are created with `mkdirat` only when the purpose allows.
- **FS-reported name check:** after each `openat`, `rustix::fs::getpath(fd)` (`F_GETPATH`) returns the name as stored on disk. Its last component's folded key is checked against the deny list again. That catches an existing `.Git` reached through a differently cased or normalized input on a case-insensitive volume.
- **Final file:** opened with `O_NOFOLLOW` plus `O_NOFOLLOW_ANY` (macOS) as defense in depth. Then `fstat`: it must be a regular file; `st_nlink > 1` → refuse (reads and writes); the size cap is applied per file class before reading.
- All later operations (temp create, rename, unlink, fsync) use the held parent fd (ADR-010). Paths are never re-resolved by name after the check, which closes check-then-use races.

**3. Outside reads (SR-31a):** `store::outside` holds the fixed list as constants and functions:

- user settings and managed settings (`/Library/Application Support/ClaudeCode/managed-settings.json` and `managed-settings.d/*.json`, from the managed-settings docs, 2026-10-19);
- `mcpServers` in `~/.claude.json`;
- `~/.claude/{CLAUDE.md,agents,skills,commands,rules,output-styles}`;
- ancestor `CLAUDE.md`/`CLAUDE.local.md` and their `@` imports;
- transcript existence (`statat`, never opened);
- `/etc/paths`, `/etc/paths.d/*`, `/etc/shells`;
- the blueprint auto-detect candidates.

Each read applies the same no-symlink and size rules. `std::fs` is clippy-denied everywhere else.

## Alternatives considered

- **`cap-std` 4.0.3.** A mature capability model, but its nofollow semantics cover the last component only, while F-021 requires refusal at **every** component. Its sandbox can't express deny-by-folded-name, and it adds several crates for a policy that's about 250 lines with rustix.
- **`openat` 0.1.21.** Unmaintained since 2021.
- **Canonicalize, then prefix-check.** A classic TOCTOU race; it also follows symlinks by design.
- **ASCII-only write paths** (an earlier draft of this ADR). Avoids Unicode tables, but SR-30 now asks for NFC and full case folding explicitly, and ICU4X normalization is already in the graph.

## Consequences

- One new crate (`icu_casemap` 2.3.0, from the Unicode Consortium's ICU4X, sharing `icu_provider` data infrastructure with the existing `icu_normalizer`). Folding cost is negligible at our path counts.
- Foreman refuses to read or write through any symlinked component inside a project (for example a symlinked `docs/handoff`). The error names the component.
- `F_GETPATH` and `O_NOFOLLOW_ANY` are macOS-only (`cfg(target_os = "macos")`). Linux CI exercises the walk and lexical rules; macOS CI exercises the rest.

## Verification

- `fs_policy::*` golden suite: `..`, absolute, `.git/hooks/pre-commit`, `.GIT/hooks/pre-commit`, `a/.git/x`, `spec/.claude/settings.json`, `spec/.mcp.json`, `.MCP.json`, `claude.md` in a subfolder, root `CLAUDE.md` without the assembler purpose, NUL, bidi, zero-width, Kelvin-sign and Cyrillic lookalikes, NFD forms, a symlinked `docs/handoff`, a symlinked final file, and a REPO FILES fixture containing each.
- `fs_policy::case_and_normalization_variants` on case-insensitive and case-sensitive APFS volumes (disk images created in the test), `fs_policy::reported_name_checked`.
- `fs_policy::read_refuses_symlink_at_any_component`, `fs_policy::read_refuses_hardlink`, `fs_policy::read_size_cap`, `fs_policy::read_scope`, `fs_policy::denies_claude_settings_writes`, `secrets::excluded_from_views_and_watcher`, `secrets::symlink_to_secret_refused`, `watcher::symlinked_root_refused`.
- A proptest: random paths with planted symlinks and hard links never touch anything outside the root (checked by inode).

## Revisit when

- macOS adds a `RESOLVE_BENEATH`-style `openat` flag: adopt it for the walk.
- cap-std adds an any-component nofollow mode.
- ICU4X 3.x changes the casemap API.

## Exit path

`check_write`/`check_read` are independent of the resolver. Replacing the rustix walk (for example with cap-std plus our own component checks) changes `store::Root` only. Replacing ICU4X folding changes one `key()` function covered by the golden suite.
