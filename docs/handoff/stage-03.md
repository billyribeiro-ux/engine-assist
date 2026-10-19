=== HANDOFF PACKET · Stage 3 → Stage 4 ===
PROJECT: Foreman
APPROVED BY: <blank>

COMPLETED:
  - docs/requirements.md: the complete security and privacy requirements
    (SR-01 to SR-64 incl. SR-25a and SR-31a, PR-01 to PR-08, AR-01), each
    with the build stage,
    the feature that implements it, and the named test that proves it,
    plus a traceability summary by stage
  - Security review by the security-reviewer agent (findings resolved in
    docs/requirements.md; see EVIDENCE)

EVIDENCE:
  - Every requirement numbered and assigned to a stage and a test:
    attested, docs/requirements.md §1–§8 and the traceability summary
  - Security-reviewer check that nothing is missing: attested. The
    reviewer read the published sources of tauri 2.12.4, tauri-utils,
    tauri-codegen, tauri-plugin-opener, portable-pty, notify, xterm 6 and
    the Claude Code 2.1.303 binary; 1 blocker (terminal paste/escape
    injection, now SR-28), 16 major and 12 minor findings, all applied

DECISIONS:
  - Threat posture: Billy is trusted; repo content, pasted chat output,
    the blueprint, Claude Code's behavior, each repo's Claude Code config
    and .git/config are untrusted; the webview is less trusted than Rust
  - The webview holds no plugin permissions; clipboard, opener, dialog,
    notification and window-state run only from Rust (SR-22)
  - v1 makes no `claude -p` calls (SR-16); verify commands and `auto`
    gate items are v1.1 (SR-11, SR-12)
  - One git entry point with a cleared environment and overrides that
    stop repo hooks, filter drivers, gpg programs, pagers and editors
    (SR-44, SR-45); signing uses plumbing (SR-42, SR-43)
  - Everything sent toward a Claude session is stripped of control and
    escape sequences (SR-28); terminal links and webview navigation are
    routed through Rust with a confirmation (SR-29)
  - Reads refuse symlinks and hard links; reads outside the project
    follow a fixed list (SR-31, SR-31a)
  - Claims resting on Claude Code docs the sandbox couldn't reach are
    marked "verify in 4.2" (SR-06, SR-09)

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
