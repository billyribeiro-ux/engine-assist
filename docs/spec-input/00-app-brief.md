=== HANDOFF PACKET · Stage 1 → Stage 2 ===
PROJECT: Foreman
APPROVED BY: <sign here: your name and date>

APP: Foreman — a macOS desktop workspace that runs the Claude Build
     Blueprint for you, stage by stage, from first idea to Certificate of
     Occupancy, launching Claude Code with the right model, effort, mode,
     and agents at every step.

FOR: Billy Ribeiro (single user, personal tool). Principal engineer and
     solo founder running several products at once (TradeLog, swings
     platform, charting platform, options research, creator platform,
     Rampsmith).

PROBLEM: The blueprint works, but running it by hand is friction:
  - finding the right prompt, filling in variables, and remembering the
    exact model / effort / mode / agents for each step
  - copying handoff packets between steps without losing or mangling them
  - remembering which stages are done, signed, skipped, or not applicable
    across several projects at once
  - running gates and recording lessons consistently, every time
  Every manual step is a chance to skip a gate or pick the wrong model,
  which costs quality and Max-plan usage.

DOES:
  - Creates a project from a rough idea, asks qualifying questions
    (payments? other users? public?) and works out which stages apply
  - Shows the whole blueprint as a stage map (0–18) with live status per
    project: locked, ready, in progress, inspecting, signed, not applicable
  - For every step: shows the RUN WITH settings and the exact prompt with
    variables and the previous handoff packet already filled in
  - Launches Claude Code in an embedded terminal with the exact command
    (model, effort, permission mode, session name); or copies the prompt
    and opens Claude Desktop; or copies the prompt for Claude chat and
    takes the returned packet back in
  - Watches docs/handoff/ and shows each packet when it lands; validates
    it (all fields present, OPEN ITEMS empty) and lets me sign it
  - Runs stage gates: inspection checklist with evidence, optional
    headless inspection that returns structured PASS/FAIL results
  - Reads docs/tickets.md and runs the Crew Loop per ticket, routing the
    model, effort, and mode automatically from the ticket class
  - Skip: only for steps tagged conditional (when the qualifying answer
    makes them not applicable) or optional; core steps can't be skipped.
    Every skip records a reason in the stage's handoff packet.
  - Add: insert my own prompts, inspection items, or agents into any
    stage, marked "this project only" or "promote to blueprint"
  - Lessons: captures Gate C lessons, and turns "promote to blueprint"
    into a proposed, versioned edit in my claude-blueprint repo
  - Change Requests: records CRs; a CR can reactivate a not-applicable stage
  - Dashboard: progress per project, time per stage, runs per ticket,
    and the Certificate of Occupancy checklist

CORE LOOP: Open Foreman → pick a project → it shows the next ready step
  with everything prefilled → run it → packet lands → inspect → sign →
  next step. Several times a day, across several projects.

WOW MOMENT: Creating a project, answering five questions, and seeing the
  whole tailored path to production laid out, with the first prompt
  ready to run in one keystroke.

WITH (must integrate):
  - Claude Code CLI (installed locally; Max plan subscription; no API key)
  - Claude Desktop app (open on the project folder)
  - Claude chat (copy out, paste back)
  - git (status, commit of signed packets)
  - The claude-blueprint repo (template source, agents, templates)
  - Project repos on disk (docs/handoff, docs/lessons.md, docs/tickets.md)

WITH (constraints):
  - macOS only, personal use, built and run locally (no notarization)
  - Uses my Max plan through Claude Code; never calls the Claude API
    directly and never needs an API key
  - No Fable models anywhere in Foreman's own build
  - Offline-capable apart from what Claude Code itself needs
  - Quality bar: TypeScript strict with zero errors/warnings; clippy
    -D warnings clean; tests for every domain rule; WCAG 2.2 AA;
    production-grade, built to last

STACK PREFERENCE: Tauri (Rust core) + SvelteKit 3 / Svelte 5 frontend,
  TypeScript strict, pnpm. Stage 2 verifies every layer on evidence.

NOT DOING (v1): cloud sync, other users or teams, payments, Windows or
  Linux builds, an App Store release, calling the Claude API directly,
  editing code inside Foreman (Claude Code does that), replacing
  Claude Code's own UI.

MONETIZATION HYPOTHESIS: none. Personal tool.

SUCCESS METRICS:
  - TradeLog runs from Stage 1 to Stage 5 entirely through Foreman
  - Zero manual prompt edits needed to fill variables or packets
  - Zero gates skipped by accident
  - Starting any step takes under 10 seconds from opening the app

KILL CRITERION: if after running two stages of TradeLog through it,
  Foreman is slower than running the blueprint by hand, stop and simplify.

RISKIEST ASSUMPTION: that the blueprint's Markdown can be parsed into a
  reliable structured template. v1 tests this first (Stage 6, the
  template parser, with golden tests against the real blueprint).

OPEN ITEMS: none
NEXT: Stage 2 · Architect's Design Studio · Claude Code CLI council ·
      Opus 5.5 · xhigh · Plan
