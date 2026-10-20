# CR-007: `rev-parse --verify -q refs/heads/<branch>` for signing on another branch

STATUS: proposed
REQUESTED BY: Claude (M4-21 build, stage/05-signing) · 2026-10-20
DECIDED BY: <Billy> · <date>

## What changes
DR-140 and ADR-012 decision 1 list the `rev-parse` forms `git::run` may run: `--show-toplevel`, `--git-path`, `--git-common-dir`, `--verify -q HEAD` and `<rev>:<path>`. This CR adds one form:

```text
rev-parse --verify -q refs/heads/<branch>^{commit}
```

as `RevParse::VerifyBranch(ref)` in `foreman-core::git`. The branch name is checked in Rust first (`git::branch_ref`, which follows `git check-ref-format --branch` and allows only ASCII letters, digits and `/ . _ -`). No other operation changes. The `-c` overrides, the environment and the 60 s timeout stay as they are.

## Why
AC-F014-09 and SR-43 require "the same plumbing against that branch's tip": `read-tree <tip>`, `commit-tree -p <tip>` and `update-ref refs/heads/<branch> <new> <old>`. All three need the branch's current commit OID, and DR-140's list has no form that resolves a branch name to it. `rev-parse --verify -q HEAD` only resolves HEAD. `<rev>:<path>` returns a blob or tree, not the commit. Reading `.git/refs` or `packed-refs` directly would skip git (reftable repos, packed refs), so that was not used.

## Impact
- Scope / feature catalog: F-014 (other-branch signing). No AC changes.
- ADRs: ADR-012 decision 1 gains `VerifyBranch(ref)` in its `RevParse` list. DR-140 gains the form.
- Threat model: none new. The form reads a ref and never writes. It's built from a validated name with no option-like value, and it carries the SR-45 overrides.
- Data model, API contract: none.
- Timeline: none; it's built behind this CR in `stage/05-signing` (M4-21).

## Recommendation
APPROVE. Without it, signing on another branch can't find the tip it must commit onto.

## Deltas (filled in when approved)
- docs/domain-rules.md DR-140: add `--verify -q refs/heads/<branch>` to the `rev-parse` forms.
- docs/adr/ADR-012-git-integration.md decision 1: add `VerifyBranch(ref)`.
- `git::invocation_snapshot` already shows the new argv line.
