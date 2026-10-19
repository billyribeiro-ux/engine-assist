# ADR-011: Path policy as a lexical check plus a component-wise fd walk with rustix (O_NOFOLLOW, O_NOFOLLOW_ANY)

## Status
Proposed

## Date
2026-10-19

## Context

F-021, SR-30 and SR-31: one Rust function guards every write Foreman makes, whatever the source:

- repo-relative only;
- no `..`, absolute paths, NUL, control or bidi characters;
- deny by **component at any depth**: `.git`, `.claude` (except the hard-coded `council-setup` destinations), `.husky`, `.vscode`, `.idea`, `.github`, `.foreman` (except the decision-file writers);
- deny file names `.mcp.json`, `CLAUDE.local.md`, `.envrc`, `.gitmodules`, `.gitattributes`, `.npmrc`;
- `CLAUDE.md` only at the repo root, only through the assembler, with confirmation;
- **symlink refusal at every component through a directory-handle API**;
- final path inside the project root or app data.

Reads are limited to registered roots and exclude secrets (SR-31, SR-50). R-11 left the crate choice (`cap-std` vs `openat`-style calls with `O_NOFOLLOW`) to this ADR.

Checked 2026-10-19:

- `cap-std`, `cap-primitives` and `cap-fs-ext` 4.0.3 (2026-08-20). `cap-fs-ext::DirExt::open_dir_nofollow` is documented as "fails if the path names a symlink", and `cap_primitives::fs::FollowSymlinks` says "should symlinks be followed **in the last component** of a path". Intermediate symlinks that stay inside the sandbox are resolved.
- `rustix` 1.1.5 (2026-09-16; 1.27 B downloads): `openat`, `OFlags::NOFOLLOW`, `OFlags::DIRECTORY`, Apple-only `OFlags::NOFOLLOW_ANY` (`O_NOFOLLOW_ANY`), `renameat_with`, `unlinkat`, `statat` — all safe APIs.
- `openat` 0.1.21: last release 2021-04.

## Decision

Two layers, both in `foreman-core`:

1. **Lexical** (`pathpolicy::check_write(rel, purpose) -> Result<RepoPath, PolicyViolation>`, pure):
   - **Write paths are ASCII only**, from the set `[A-Za-z0-9 ._@+-]` plus `/` as the separator. This rejects NUL, control characters, bidi controls, backslashes, and every Unicode lookalike (for example `.gіt` with a Cyrillic і) without needing Unicode confusable tables. Every path Foreman writes comes from the blueprint, fixed constants, or a packet's REPO FILES list, all of which are ASCII in practice. A non-ASCII REPO FILES entry is refused with "Foreman writes only ASCII paths; create this file yourself."
   - Read paths may be any valid UTF-8 without NUL, control or bidi characters (Unicode bidi controls U+202A–U+202E and U+2066–U+2069).
   - The path is split on `/`, rejecting empty, `.` and `..` components.
   - Deny names are compared ASCII case-insensitively (APFS is case-insensitive by default), so `.GIT` and `Claude.md` outside the root are caught.
   - Exceptions are typed `WritePurpose` variants, never strings from content: `CouncilSetup(AgentName)` maps to the fixed set `spec/.claude/agents/<known>.md`; `DecisionFile(Kind)` maps to `.foreman/{project,overlay,blueprint.lock}.json` and `.foreman/stages/stage-NN.json`; `AssemblerClaudeMd` allows `CLAUDE.md` at the root only, after a UI confirmation token.
   - `check_read(rel)` applies the same syntax rules plus the secret patterns (`.env*`, `*.pem`, `*.key`, `id_*`, `*.p12`, `.npmrc`, `.netrc`, `.ssh/`, `.aws/`, `.config/gcloud/`).
2. **Resolution** (`store::Root`, with rustix):
   - `Root::open(abs, kind)` opens the root with `O_DIRECTORY|O_CLOEXEC`. The root itself may be reached through symlinks, because Billy chose it (for example `~/Developer` → another volume); its canonical path is recorded and shown.
   - For every intermediate component: `openat(dirfd, comp, O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC)`. A symlink yields `ELOOP`/`ENOTDIR` → `PolicyViolation::Symlink(component)`. `mkdirat` is used for missing directories only when the purpose allows creation.
   - The final open uses `O_NOFOLLOW` plus, on macOS, `O_NOFOLLOW_ANY` as defense in depth.
   - All later operations (temp create, rename, unlink, fsync) use the held parent fd (ADR-010), so the path is never re-resolved by name after checking. That closes the check-then-use race.
   - Hard links: before overwriting, `fstat` the target; `st_nlink > 1` → refuse (a hard link could alias a file outside policy).

## Alternatives considered

- **`cap-std` 4.0.3** (with `cap-fs-ext`). A mature capability model with a good `Dir` API. Lost because its nofollow semantics cover the last component, while in-sandbox intermediate symlinks are followed. F-021 requires refusal at **any** component, so we'd still walk components ourselves on top of it. Its sandbox also doesn't express our deny-by-name rules, and it adds several crates (`cap-primitives`, `io-lifetimes`, `fs-set-times`, …) for a policy that is about 200 lines with rustix.
- **`openat` 0.1.21.** Unmaintained since 2021.
- **Canonicalize then prefix-check (`std::fs::canonicalize`).** A classic TOCTOU: the path can change between canonicalize and open. It also follows symlinks by design.
- **`std::fs` with `symlink_metadata` checks per component.** Name-based again (TOCTOU) and no fd chaining.

## Consequences

- Foreman refuses to write through a symlinked `docs/handoff`. The error names the component and suggests replacing the symlink with a directory.
- The ASCII-only write rule may refuse a legitimate non-ASCII file name in a REPO FILES list; Billy creates such a file by hand. It's a deliberate tightening within SR-30's intent (which forbids control and bidi characters and requires lookalike tests). Stage 4.7 should confirm it.
- macOS-only flags (`NOFOLLOW_ANY`) are behind `cfg(target_os = "macos")`; Linux CI runs the per-component walk, which is the primary guarantee.

## Verification

- `fs_policy::*` golden suite: `..`, absolute, `.git/hooks/pre-commit`, `a/.git/x`, `spec/.claude/settings.json`, `spec/.mcp.json`, `spec/CLAUDE.md`, `CLAUDE.md` without the assembler purpose, NUL, bidi, a Cyrillic `.gіt` (non-ASCII refused), `.GIT` (case), a symlinked `docs/handoff`, a symlinked final file, a hard-linked target, and a REPO FILES fixture containing each.
- `fs_policy::read_scope`, `secrets::excluded_from_views_and_watcher`, `fs_policy::denies_claude_settings_writes`.
- A proptest: random paths with planted symlinks never write outside the root (checked by inode comparison).

## Revisit when

- macOS adds a `RESOLVE_BENEATH`-style `openat` flag: adopt it for the whole walk.
- cap-std adds an "any component nofollow" mode.

## Exit path

`check_write` is independent of the resolution layer. Replacing the rustix walk with cap-std (plus its own component checks) touches `store::Root` only.
