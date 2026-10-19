=== HANDOFF PACKET · Stage 2 → Stage 3 ===
PROJECT: Foreman
APPROVED BY: Billy Ribeiro on 2026-10-19
SIGNED VIA: Claude Code, on Billy's explicit instruction and permission in chat (2026-10-19 19:02-19:04 ET)

POSITIONING: Foreman is the site office for building software with
  Claude: it holds the blueprint, knows where every project stands, and
  starts each step with exactly the right crew, model, effort, mode and
  inputs. It catches every handoff packet, keeps gates honest, and signs
  the record, so the process is followed every time without effort. It's
  the only tool that turns the blueprint into an executable, verified
  process while leaving Claude Code, Claude chat and the blueprint as
  they are.

V1 FEATURES: (P1 = thin end-to-end, ends at the kill check; P2 = rest of v1;
  full table with phases: docs/MASTER-SPEC.md §4 and §9)
  F-001 Template engine: annotations only, prose lint, item IDs + hashes
  F-002 Blueprint lock (with item hashes)
  F-003 TradeLog acceptance harness (P1) + Template health screen (P2)
  F-004 Rule engine: tags, tri-state conditions, applicable path
  F-005 Run configs + model policy
  F-006 Prompt resolver: six token classes incl. optional parts; fm:if parts
  F-007 Step kinds and states
  F-008 Launch pipeline: version, full config fingerprint, auth in cwd
  F-009 Terminal runner: portable-pty + xterm 6
  F-010 Session chains and re-arm (manual idle confirmation in P1)
  F-011 Session sensor (P2)
  F-012 Chat bridge: chips, links, paste-back, attachments
  F-013 Packet engine: watch, validate, normalize (repair, capture in P2)
  F-014 Signing and provenance (P2; current branch by default)
  F-015 Gates: attest with note or n/a, time, interactive Gate A import (P2)
  F-016 Lessons capture: F.2 per stage, Gate C (P2)
  F-017 Next step + Do next (P1); Today inbox (P2)
  F-018 Setup form (P1); wizard (P2)
  F-019 First run, version, auth (P1); Doctor, contract probes (P2)
  F-020 State, persistence, resilience
  F-021 Path policy (deny by component at any depth, symlink refusal)
  F-022 Outside-change review for decision files
  F-023 Knowledge manifest + build-package assembler (P2)
  F-024 Desktop handoff (P2)
  F-025 Add prompt / inspection item, this project only (P2)
  F-026 Run log + Export metrics (P1); metric panel, run log screen (P2)
  F-027 Run sheet export (P2)
  F-028 Core shortcuts (P1); command palette (P2)
  F-029 Settings (P1 paths and DRI; P2 rest)
  F-030 Stage workspace (P1); sheet strip, title block, inspector (P2)
  v1.1: headless Gate A with auto items and verify commands, Crew Loop,
  CR track, full overlay authoring, usage meter, live send, lessons
  view, metrics dashboard.

DIFFERENTIATORS:
  - vs running by hand: exact argv per step, re-arm for "same session"
    steps, zero manual prompt edits (tested by the TradeLog harness),
    validated packets, hash-bound signing, evidence-backed gates
  - vs Claude Desktop / Claude Code alone: they run sessions; Foreman
    knows the process, the next step, and what each step must produce
  - vs prompt managers: packets resolved, flags launched, gates enforced

STACK (FINAL):
  Shell: Tauri 2.12.4
  Frontend: SvelteKit 3.1.0 SPA (adapter-static 4.0.0, fallback
    index.html, ssr = false, #lib) + Svelte 5.57.2 + Vite 8.3.4
    (Billy's decision; config verified in r1 §A4)
  Language: TypeScript 6.0.3 strict · Rust stable >= 1.90
  Package manager / runtime: pnpm 12.14.0 · Node >= 22.17
  Terminal: @xterm/xterm 6.0.0 · PTY: portable-pty 0.9.0 (own wrapper)
  Spawning: std/tokio process, argv only (no shell, fs or pty plugins)
  File watching: notify 8.2.0 + notify-debouncer-full 0.7.0
  Markdown: comrak =0.56.0
  UI primitives: bits-ui 2.19.5 · State: Svelte 5 runes mirroring Rust
  Tests: cargo test + proptest + insta · Vitest 5.0.5 · Playwright 1.64.1
    + axe (mocked IPC) · @wdio/tauri-service smoke behind `e2e` feature
  Lint: eslint 10 + svelte-check 4.7.7 + prettier 3.9 · clippy -D warnings
  Full pinned list: spec/council/r7-stack.md PART 3

NFRs: cold start p95 <= 1.5 s · project switch p95 <= 300 ms · template
  load p95 <= 50 ms · keystroke to spawn p95 <= 250 ms · packet visible
  p95 <= 1 s · time to start a step p95 <= 10 s (excl. typing inputs) · <= 300 MB with 3
  terminal tabs (verified in S-0) · no data loss, crash-safe run state ·
  ASVS 5.0 L2 (applicable chapters) · WCAG 2.2 AA · macOS 27 and 26 on
  Apple Silicon · no network calls by Foreman · $0 infrastructure.
  Full list: spec/council/r8-nfr.md (NFR-01 to NFR-48)

PRICING SIGNALS: none (personal tool; commercial = no)

DATA WE HANDLE: no personal data about others, no payment data. Local
  only: project repos (packets, lessons, decision files), app data (run
  log, terminal output tails, sensor events), and the Claude Code
  subscription login, which Foreman never reads (it checks authMethod
  only). Sensitive by nature: terminal output may contain secrets that
  appear in a session; kept only in app data, 90-day retention.

CONDITIONS (confirmed for Foreman): idea_known yes · has_ui yes ·
  commercial no · payments no · multi_user no · public no ·
  others_data no · server no

SKIPPED:
  - 2.1 Competitor teardown: not applicable (commercial = no)
  - Council market-analyst and growth-strategist roles: not applicable
    (commercial = no)

SUBSTITUTIONS:
  - Chair: Opus 5.5 · xhigh instead of Fable 5.1 · high (Billy's rule:
    no Fable in Foreman's build)
  - Council run in a Claude Code cloud session with subagents, not an
    interactive CLI session with a "go" after each round (Billy:
    "Get everything that can be done done here"); every round is a file
    in spec/council/ for review
  - Round 1 used principal-engineer (engineering evidence) and
    user-advocate (workflow review) in place of market-analyst (n/a);
    feasibility (2.2 Round 2) is covered by r1-engineering and r7-stack
  - The council's spikes need a logged-in Claude Code on Billy's Mac;
    each has a decision rule and runs in Stage 5.3 (MASTER-SPEC §9)

BLUEPRINT DEPENDENCY: Foreman v1 targets blueprint v2.2. Signing this
  packet approves applying spec/blueprint-v2.2/PROPOSAL.md (B-1 to B-6a,
  B-8 now; B-6b and B-7 with v1.1) in the claude-blueprint repo. Note any
  item you reject next to your signature.

CHANGES FROM THE APP BRIEF: docs/MASTER-SPEC.md §17 lists every App
  Brief item moved to v1.1 or redefined (headless inspection, Crew Loop,
  add agents / promote to blueprint, CRs, dashboard, the 10-second
  metric as p95, kill check at about 8-14 weeks, signing by hand in P1).
  Signing accepts them; note any you reject.

ACTION FOR BILLY BEFORE SIGNING: the Stage 1 packet's APPROVED BY line was
  filled in by Claude from your chat approval, which breaks the "never
  sign a packet" rule and isn't in the standard form. Please re-sign
  docs/handoff/stage-01.md yourself as "Billy Ribeiro on 2026-10-19".

OPEN ITEMS: none

NEXT: Stage 3 · Permits & Zoning · Claude Code CLI · Opus 5.5 · high ·
  Plan, then Accept edits · agents: security-reviewer
  (per docs/spec-input/03-build-prompts.md: business model and legal
  parts not applicable; security and privacy requirements are core)

FILES: docs/MASTER-SPEC.md (with design Appendix A), spec/council/r1-engineering.md,
  spec/council/r1-workflow.md, spec/council/r2-proposals.md,
  spec/council/r3-synthesis.md, spec/council/r4-redteam.md,
  spec/council/r5-resolution.md, spec/council/r5b-freeze-review.md,
  spec/council/r6-feature-specs.md,
  spec/council/r7-stack.md, spec/council/r8-nfr.md,
  spec/blueprint-v2.2/PROPOSAL.md (for Billy to apply in the blueprint
  repo), docs/metrics-baseline.md (start logging at Stage 5)

INSPECTION (2.6):
  - Feature matrix covers every serious alternative, with sources:
    attested, docs/MASTER-SPEC.md §2 (no market; alternatives are by hand,
    Claude Desktop / Claude Code, prompt managers)
  - Every v1 feature beats a named alternative or is table stakes:
    attested, docs/MASTER-SPEC.md §4 and r6 "Beats" lines
  - Every fatal or serious red-team objection resolved in writing:
    attested, r5-resolution §1 (R-01 to R-14, all accepted); an
    independent freeze review (r5b) found 3 blockers, 17 major and 20
    minor issues, all resolved in r5b and applied to the frozen files
  - Evidence-first stack recommendation for every layer plus the
    recorded final choice: attested, r7-stack PART 1–3 and DRI section
  - Open questions: zero; Status: FROZEN v1.0: attested,
    docs/MASTER-SPEC.md §15 and header
