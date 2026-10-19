=== HANDOFF PACKET · Stage 2 → Stage 3 ===
PROJECT: Foreman
APPROVED BY: <blank>

POSITIONING: Foreman is the site office for building software with
  Claude: it holds the blueprint, knows where every project stands, and
  starts each step with exactly the right crew, model, effort, mode and
  inputs. It catches every handoff packet, keeps gates honest, and signs
  the record, so the process is followed every time without effort. It's
  the only tool that turns the blueprint into an executable, verified
  process while leaving Claude Code, Claude chat and the blueprint as
  they are.

V1 FEATURES: (P1 = thin end-to-end, ends at the kill check; P2 = rest of v1)
  F-001 Template engine: annotations only, prose lint, explicit item IDs (P1+P2)
  F-002 Blueprint lock (P1)
  F-003 Template health + TradeLog acceptance harness (P1+P2)
  F-004 Rule engine: tags, tri-state conditions, applicable path (P1)
  F-005 Run configs + model policy (P1)
  F-006 Prompt resolver, five token classes (P1+P2)
  F-007 Step kinds and states (P1)
  F-008 Launch pipeline: version, contract, config fingerprint, auth in cwd (P1)
  F-009 Terminal runner: portable-pty + xterm 6 (P1+P2)
  F-010 Session chains and re-arm (P1, manual idle confirmation)
  F-011 Session sensor: per-launch hooks, exec form, tokened events (P2)
  F-012 Chat bridge: chips, links, paste-back, attachments, ADR split (P1)
  F-013 Packet engine: watch, validate, repair, capture (P1)
  F-014 Signing and provenance: hash-bound, pathspec, read-back (P1)
  F-015 Gates: attest with note or n/a; auto = Foreman runs the command (P1+P2)
  F-016 Lessons capture (P1)
  F-017 Today and Do next (P1 Next step; P2 inbox)
  F-018 Projects and setup (P1 form; P2 wizard)
  F-019 First run, Doctor, zero-token contract probes (P1+P2)
  F-020 State, persistence, resilience (P1)
  F-021 Path policy for every Foreman write (P1)
  F-022 Outside-change review for decision files (P1)
  F-023 Knowledge manifest + build-package assembler (P2)
  F-024 Desktop handoff (P2)
  F-025 Add prompt / inspection item, this project only (P2)
  F-026 Run log (P1) + metric panel (P2)
  F-027 Run sheet export (P1)
  F-028 Command palette and keyboard (P1+P2)
  F-029 Settings (P1)
  F-030 Sheet strip and stage workspace (P1)
  v1.1: headless Gate A (hardened), Crew Loop, CR track, full overlay
  authoring, usage meter, live send, lessons view, metrics dashboard.

DIFFERENTIATORS:
  - vs running by hand: exact argv per step, re-arm for "same session"
    steps, zero manual prompt edits (tested by the TradeLog harness),
    validated packets, hash-bound signing, evidence-backed gates
  - vs Claude Desktop / Claude Code alone: they run sessions; Foreman
    knows the process, the next step, and what each step must produce
  - vs prompt managers: packets resolved, flags launched, gates enforced

STACK (FINAL):
  Shell: Tauri 2.12.4
  Frontend: Svelte 5.57.2 + Vite 8.3.4 + in-house typed hash router
    (Billy: "no preference"; evidence-backed recommendation adopted)
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
  p95 <= 1 s · time to start a step median <= 10 s · <= 300 MB with 3
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

OPEN ITEMS: none

NEXT: Stage 3 · Permits & Zoning · Claude chat · Opus 5.5 · adaptive
  (3.1 business model: not applicable; 3.2 security, privacy and
  compliance requirements: core, legal part not applicable)

FILES: spec/MASTER-SPEC.md, spec/council/r1-engineering.md,
  spec/council/r1-workflow.md, spec/council/r2-proposals.md,
  spec/council/r3-synthesis.md, spec/council/r4-redteam.md,
  spec/council/r5-resolution.md, spec/council/r6-feature-specs.md,
  spec/council/r7-stack.md, spec/council/r8-nfr.md,
  spec/blueprint-v2.2/PROPOSAL.md (for Billy to apply in the blueprint
  repo), docs/metrics-baseline.md (start logging at Stage 5)

INSPECTION (2.6):
  - Feature matrix covers every serious alternative, with sources:
    attested, MASTER-SPEC §2 (no market; alternatives are by hand,
    Claude Desktop / Claude Code, prompt managers)
  - Every v1 feature beats a named alternative or is table stakes:
    attested, MASTER-SPEC §4 and r6 "Beats" lines
  - Every fatal or serious red-team objection resolved in writing:
    attested, r5-resolution §1 (R-01 to R-14, all accepted) and an
    independent freeze review
  - Evidence-first stack recommendation for every layer plus the
    recorded final choice: attested, r7-stack PART 1–3 and DRI section
  - Open questions: zero; Status: FROZEN v1.0: attested, MASTER-SPEC
    §15 and header
