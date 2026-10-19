=== HANDOFF PACKET · Stage 3 → Stage 4 ===
PROJECT: Foreman
APPROVED BY: <blank>

COMPLETED:
  - docs/requirements.md: the complete security and privacy requirements
    (SR-01 to SR-64, PR-01 to PR-08, AR-01), each with the build stage,
    the feature that implements it, and the named test that proves it,
    plus a traceability summary by stage
  - Security review by the security-reviewer agent (findings resolved in
    docs/requirements.md; see EVIDENCE)

EVIDENCE:
  - Every requirement numbered and assigned to a stage and a test:
    attested, docs/requirements.md §1–§8 and the traceability summary
  - Security-reviewer check that nothing is missing: attested, review
    findings applied (see DECISIONS)

DECISIONS:
  - Threat posture: Billy is trusted; repo content, pasted chat output,
    the blueprint, Claude Code's behavior, each repo's Claude Code config
    and .git/config are untrusted; the webview is less trusted than Rust
  - The webview holds no plugin permissions; clipboard, opener, dialog,
    notification and window-state run only from Rust (SR-22)
  - v1 makes no `claude -p` calls (SR-16); verify commands and `auto`
    gate items are v1.1 (SR-11, SR-12)
  - Every git call disables repo hooks, fsmonitor, pagers, external diff
    and editors (SR-44, SR-45)

STATE: documents only; no code exists (Pre-Construction)

KNOWN LIMITS: CSP `style-src` strictness is decided by spike S-0 in
  Stage 5.3 (MASTER-SPEC §9); the path-policy crate and F_FULLFSYNC
  behavior are decided in the Stage 4.2 ADRs

SKIPPED:
  - 3.1 business model and pricing: not applicable (commercial = no,
    payments = no)
  - 3.2 legal and compliance part: not applicable (personal tool, no one
    else's data, no distribution)

OPEN ITEMS: none

NEXT: Stage 4 · Construction Drawings · Claude Code CLI · Opus 5.5 ·
  high (xhigh for 4.2, 4.3, 4.7) · Plan, then Accept edits

INPUTS FOR NEXT STAGE: docs/handoff/stage-02.md, docs/MASTER-SPEC.md,
  docs/requirements.md, spec/council/r6-feature-specs.md,
  spec/council/r7-stack.md, spec/council/r8-nfr.md
