# Foreman (repo: engine-assist)

A personal macOS app that runs the Claude Build Blueprint stage by stage and launches Claude Code with exactly the right model, effort, mode, agents and inputs.

## Before planning any work
Read the latest `docs/handoff/stage-NN.md` and `docs/lessons.md`. A stage starts only after the previous packet is signed by Billy. **Never sign a packet** (never fill in APPROVED BY).

## Stack (exact; verify against registries before changing anything)
- Tauri 2.12.4 (CLI 2.12.3), Rust 1.99.1 via `rust-toolchain.toml` (floor 1.90)
- SvelteKit 3.1.0 in SPA mode (adapter-static 4.0.0, `fallback: 'index.html'`, `ssr = false`) on Svelte 5.57.2 and Vite 8.3.4. Config lives in `vite.config.ts` (`svelte.config.js` isn't supported). Use `#lib` imports.
- TypeScript **6.0.3** (pinned: TS 7 breaks svelte-check and typescript-eslint; ADR-019); pnpm 12.14.0; Node ≥ 22.17
- portable-pty 0.9.0, @xterm/xterm 6.0.0, notify 8.2.0, comrak =0.56.0, ts-rs 12.0.1
- UI primitives are hand-built and keyboard-complete in `#lib/components` (no UI library; bits-ui was not adopted, ADR-020 amendment 2026-10-20)
- Claude Code ≥ 2.1.293 on the machine; subscription login only (`authMethod` `claude.ai` or `oauth_token`)
- Full pinned list: `docs/design-doc.md` §3.2, `spec/council/r7-stack.md`
- Dev tools may lag the newest patch by up to 7 days (pnpm minimumReleaseAge); see docs/dev-notes.md.

## Layout
```
crates/foreman-core/    pure domain + store: template, rules, resolver, packets, path policy, atomic I/O (no tauri/tokio/spawn)
crates/foreman-sys/     the ONLY place that spawns: spawn, git::run, PTY, claude CLI, /usr/bin/open
crates/foreman-sensor/  hook helper binary (P2)
crates/fake-claude/     test double, never bundled
src-tauri/              foreman-app: Tauri shell, IPC commands, watcher, plugin wrappers, logging
src/routes/             SvelteKit routes (one per screen); src/lib/ (#lib): IPC client, mock IPC, stores (.svelte.ts), components
docs/                   spec, ADRs, contracts, threat model, tickets, design, handoff packets, lessons
spec/                   Stage 2 council work (read-only history)
```

## Commands (defined in M0)
- `pnpm install --frozen-lockfile`
- `pnpm tauri dev`: the app; `pnpm dev:mock`: frontend in the browser against the mock IPC
- `pnpm verify`: everything. svelte-check, ESLint, Prettier check, TS strict, Vitest, Playwright flows (axe, keyboard, layout, mockup parity), rustfmt check, clippy `-D warnings`, `cargo test --workspace`, cargo audit, cargo deny, pnpm audit, secret scan, license check. **Must be green before any commit.**
- `pnpm test` (Vitest, browser mode) · `cargo test --workspace` (unit, property, golden, fake-claude integration) · `pnpm test:e2e` (real-app smoke via the in-repo WebDriver client, `--features e2e`, macOS only)
- `pnpm build` · `pnpm release` (verify, then an ad-hoc signed `Foreman.app`; copies to /Applications only after confirmation)

## How work is done
- Work only from `docs/tickets.md`, one ticket per session, using the Crew Loop in `docs/build-prompts.md` (class → model, effort, mode). Tests come first, named exactly as in the ticket's "Required tests".
- Sources of truth, in order:
  1. `docs/requirements.md` (SR/PR)
  2. `docs/domain-rules.md` (DR)
  3. `docs/contracts/` (IPC, files)
  4. `docs/adr/`
  5. `docs/scope.md` (AC/EX)
  6. `docs/MASTER-SPEC.md`

  If they conflict, stop and ask.
- Any change to frozen behavior needs a Change Request (`docs/templates/change-request.md`), approved by Billy.
- One branch per stage (`stage/NN-name`); one PR per ticket into it; conventional commits referencing the ticket ID.

## Definition of done (per ticket)
- Every acceptance criterion has a passing test with the ticket's test name, and every listed threat's test passes.
- `pnpm verify` is green, with zero warnings anywhere.
- The `code-reviewer` agent finds zero CRITICAL or WARNING (`security-reviewer` too if the ticket lists a threat).
- UI matches its mockup in `docs/design/mockups/` in both themes, including empty, error, locked and not-applicable states. axe finds no violations; everything works by keyboard.
- Docs touched by the change are updated in the same PR (ADR amendments, contracts, DRs).

## Never
- Never spawn a shell. All processes go through `foreman-sys::spawn` with an argv array and a cleared, allowlisted environment (SR-01–05). The only exception is the consented login-PATH capture.
- Never pass `--bare`, `-p`/`--print` (v1), `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions` or `bypassPermissions`.
- Never run `git add`, `commit`, `status` or `diff`. All git goes through `git::run`, plumbing only, with the fixed `-c` overrides (SR-44, SR-45).
- Never write a file except through the core's atomic writer and path policy: allowlisted paths, no symlinks, never overwrite a newer external edit (SR-30–33).
- Never render repo, pasted or terminal content as HTML (no `{@html}`, `innerHTML`) (SR-26).
- Never send unsanitized text to a Claude session (SR-28).
- Never let the webview approve anything security-relevant. Confirmations are native Rust dialogs (SR-24a).
- Never give the webview a plugin permission, and never add a capability or remote URL (SR-22, SR-23).
- Never add a network client or make a network call (SR-60).
- Never use `unsafe`, `unwrap` or `expect` outside tests.
- Never edit `~/.claude/*` or any repo's `.claude/` settings (SR-10).
- Never edit the blueprint. Foreman only proposes changes (`spec/blueprint-v2.2/PROPOSAL.md` style).
- Never use Fable models in this project. Opus 5.5 at xhigh covers architecture, HARD tickets and stuck work.
- Never state a version, flag or API from memory. Verify it against official docs or registries and cite it.

## Key docs
`docs/MASTER-SPEC.md` · `docs/requirements.md` · `docs/scope.md` · `docs/design-doc.md` · `docs/adr/README.md` · `docs/threat-model.md` · `docs/contracts/ipc.md` · `docs/contracts/files.md` · `docs/domain-rules.md` · `docs/design/components.md` · `docs/design/tokens.css` · `docs/tickets.md` · `docs/build-prompts.md` · `docs/metrics-baseline.md` · `docs/lessons.md`
