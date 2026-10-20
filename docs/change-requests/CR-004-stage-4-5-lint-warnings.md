# CR-004: Two lint warnings the annotated v2.2 blueprint carries (4.7 and 5.5)

STATUS: proposed
REQUESTED BY: the M4-04 build agent (`stage/05-annotate`) · 2026-10-20
DECIDED BY: (open)

## What changes

The annotated v2.2 fixture matches DR-038 exactly and has zero errors in Stages 0 and 3–5, but the prose lint (DR-040, DR-041) reports two warnings on it. Both are pinned in the snapshot `golden_v2_2_stages_0_5_lint`, so a third shows up in review. Template health (AC-F003-05, AC-F003-06) will list both for the real blueprint, and "no errors or warnings" will never show.

1. **5.5, `lint.out-disagrees`.** The 5.5 prompt says "Write docs/runbook-deploy.md" (DR-041 rule 6 reads that as an `out` entry), but DR-038's reference annotation for 5.5 has no `out=`. Option A (recommended): add `out=docs/runbook-deploy.md` to 5.5 in DR-038, the fixture (`crates/foreman-core/tests/fixtures/blueprint/claude-build-blueprint-v2.2.md`, `spec/blueprint-v2.2/claude-build-blueprint-v2.2.md`), `scripts/blueprint/annotate-v2.2.py` and the `DR038` list in `golden_v2_2.rs`. Option B: keep DR-038 and treat the warning as accepted.
2. **4.7, `lint.alt-missing`.** The RUN WITH says "**Fable 5.1** if available, otherwise **Opus 5.5**". DR-041 rule 5 reads "if available, otherwise" as an alternative, and DR-024 says that same phrase (`bp:979`) is *not* an alternative (a substitution rule or per-run override handles it, DR-102, DR-103). The two rules disagree. Option A (recommended): remove "if available, otherwise" from DR-041 rule 5 and from `lint.rs` (`has_alternative`), so the lint matches DR-024. Option B: keep the warning; the annotation stays without `alt=`.

## Why

Found while pinning the Stages 0–5 golden (M4-04). Neither is an error, so pinning and running are unaffected; the cost is a permanent warning count in Template health and a lint rule that contradicts DR-024. The fixture was not changed, because DR-038 is frozen and the golden compares against it.

## Impact
- Scope / feature catalog (F-IDs): F-003 (Template health display only).
- ADRs (new, amended, superseded): none.
- Threat model: none.
- Data model and migrations: none. A changed 5.5 annotation changes the blueprint's source hash, so a cached template is re-parsed (ADR-023 §5); no project is pinned yet.
- API contract and versioning: none.
- Pricing and entitlements: none.
- Timeline: under an hour.
- Cost (build + run): none.

## Recommendation
APPROVE option A for both. Billy applies the 5.5 change in the claude-blueprint repo; Foreman never edits the blueprint.

## Deltas for Stage 4 (filled in when approved)
- docs/domain-rules.md: DR-038 (5.5 row), DR-041 (rule 5).
- docs/tickets.md: none.
- Code: `crates/foreman-core/src/template/lint.rs` (`has_alternative`), the fixture and `golden_v2_2.rs` as above; refresh the snapshots `golden_v2_2_stage_5` and `golden_v2_2_stages_0_5_lint`.
