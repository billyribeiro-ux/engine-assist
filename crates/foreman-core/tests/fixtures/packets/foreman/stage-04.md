=== HANDOFF PACKET · Stage 4 → Stage 5 ===
PROJECT: Foreman
APPROVED BY: Billy Ribeiro on 2026-10-19
SIGNED VIA: Claude Code, on Billy's explicit instruction and permission in chat (2026-10-19 19:02-19:04 ET)

REPO FILES:
  CLAUDE.md                      (full project CLAUDE.md, replaces bootstrap)
  docs/MASTER-SPEC.md            (frozen v1.0, Appendix A design)
  docs/requirements.md           (SR-01..SR-64, PR-01..PR-08, AR-01)
  docs/scope.md                  (M0-M5, 380 ACs, 33 exit criteria, kill check)
  docs/design-doc.md
  docs/adr/README.md, ADR-001 .. ADR-024
  docs/threat-model.md           (107 threats, 17 abuse cases)
  docs/contracts/ipc.md          (91 commands, 3 channels, 9 events)
  docs/contracts/files.md        (18 JSON schemas + packet format)
  docs/domain-rules.md           (129 rules with real blueprint examples)
  docs/design/tokens.css, components.md, contrast.md, mockups/ (15),
    screenshots/, _build/
  docs/tickets.md                (160 tickets, coverage report)
  docs/build-prompts.md          (Stages 5-18, authoritative)
  docs/metrics-baseline.md       (by-hand log for the kill check)
  docs/review/stage4-carried-findings.md (findings and resolutions)
  docs/lessons.md

STACK + VERSIONS: Tauri 2.12.4 · SvelteKit 3.1.0 SPA (adapter-static
  4.0.0) · Svelte 5.57.2 · Vite 8.3.4 · TypeScript 6.0.3 · pnpm 12.14.0 ·
  Rust 1.99.1 (floor 1.90) · portable-pty 0.9.0 · xterm 6.0.0 · notify
  8.2.0 · comrak 0.56.0 · bits-ui 2.19.5 · ts-rs 12.0.1 · Claude Code
  >= 2.1.293 (latest is 2.1.304 today). Full list: design-doc §3.2.

MILESTONES:
  M0 Site prep (Stage 5): scaffold, verify, CI, release, 7 spikes
  M1 Domain core (Stage 6)
  M2 Walking skeleton (Stage 7)
  M3 Runners (Stages 8-9): ends with TradeLog Stages 1-2 through Foreman
     and the kill check
  M4 Complete v1 (Stages 11-12; Stage 10 not applicable)
  M5 Finishes (Stages 13-17): TradeLog Stages 3-5, success metrics, CO

EVIDENCE:
  - Independent review (design-reviewer, fresh context): READY AFTER
    FIXES; 1 blocker (Stages 1-3 unsigned; Billy's action), 12 major and
    16 minor findings, all fixed (docs/review/stage4-carried-findings.md)
  - Every SR/PR maps to a design element and a test: design-doc §5.1,
    scope §5, threat-model §6.1
  - Every threat maps to a control and a test: threat-model §6
  - Every domain rule has worked examples from the real blueprint and a
    named test: docs/domain-rules.md
  - Every ticket has a class, a stage and named tests; the coverage
    script finds no orphan AC, exit criterion, test, ADR or threat:
    docs/tickets.md "Coverage"
  - Mockups: 15 screens, contrast check 158/158, axe-core 0 violations
    in both themes

DECISIONS: ADR-001 .. ADR-024 (index in docs/adr/README.md), all
  Proposed; they become Accepted when this packet is signed. Chair
  decisions in the review fix pass: v1 never GPG-signs commits; kill
  check measured per step, chat clock ends at "prompt on clipboard and
  chat open", chat baseline needs >= 10 by-hand chat steps; a newer
  blueprint grammar makes affected steps "needs newer Foreman"; Foreman's
  own Stages 11-17 close out by hand in v1; screen reader mode is a
  Settings toggle.

KNOWN LIMITS:
  - Spike results (S-0, S-1′, S-3, S-7′, S-8, S-12, S-23) decide details
    in Stage 5.3; each has both branches specified (MASTER-SPEC §9)
  - High-contrast token values: ticket M5-04
  - Before the kill check: 110 tickets, about 110-440 hours at 1-4 h each

SKIPPED: none in Stage 4

BLUEPRINT VERSION: 2.1 (Foreman's own build); Foreman v1 targets 2.2

FIRST STAGE: Stage 5 · Site Prep · Claude Code CLI on Billy's Mac ·
  opusplan · medium · Plan (docs/build-prompts.md "Stage 5")

ACTION FOR BILLY: sign Stages 1 (re-sign), 2 and 3 first (the review's
  blocker), then this packet. Apply blueprint v2.2 in claude-blueprint
  before Stage 6 (docs/build-prompts.md "Before Stage 6").

OPEN ITEMS: none
