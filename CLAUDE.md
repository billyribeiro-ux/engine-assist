# engine-assist (Foreman)

Foreman: a personal macOS desktop app (Tauri + SvelteKit 3 / Svelte 5) that runs the Claude Build Blueprint stage by stage and launches Claude Code with the right model, effort, mode, and agents.

## Current phase: Pre-Construction

This is a bootstrap CLAUDE.md. Stage 4.8 replaces it with the full project CLAUDE.md.

- No application code is written before Stage 5. Stages 1–4 produce documents only.
- The process is `docs/spec-input/03-build-prompts.md`, following the blueprint in `docs/spec-input/claude-build-blueprint.md` (v2.1).
- Before planning any work, read the latest packet in `docs/handoff/` and `docs/lessons.md`.
- A stage starts only after the previous stage's packet is signed (APPROVED BY filled in by Billy). Never sign a packet.
- No Fable models in this project. Opus 5.5 at xhigh covers architecture, audits, and hard problems.
- Verify every technical claim (versions, APIs, flags) against current official docs; never from memory.
- The Stage 2 council runs from `spec/`, where its agents live (`spec/.claude/agents/`).
