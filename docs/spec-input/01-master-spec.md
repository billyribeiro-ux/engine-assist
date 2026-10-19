# Foreman — Master Product Spec

Status: **DRAFT v0.9** — written in Stage 1 from the architect conversation. The Stage 2 council reviews, challenges, and completes it, then freezes it as v1.0. Nothing in here is final until the council's open questions are closed and you sign the Stage 2 packet.

---

## 1. Vision

Foreman is the site office for building software with Claude. It holds the blueprint, knows where every project stands, and hands each step to the right crew: Claude chat, Claude Code in the terminal, or Claude Desktop. It fills in every prompt, catches every handoff packet, and won't let a gate be skipped by accident.

It doesn't write code and doesn't replace Claude Code. It runs the process around it, so the process is followed every time without effort.

## 2. User and jobs

One user: you. You run several products in parallel, work in Svelte 5 / SvelteKit and Rust, and hold everything to a production standard.

| Job | Today, by hand | With Foreman |
|---|---|---|
| Start a project | Copy variables into prompts, decide which stages apply | A short wizard; the path is tailored automatically |
| Run the next step | Find the prompt, edit placeholders, paste the packet, type the command | One keystroke runs the exact command with everything filled in |
| Hand off between stages | Copy packets around; they get lost or edited by mistake | Packets are detected, validated, and signed in place |
| Keep the process honest | Remember gates and lessons | Core steps can't be skipped; gates and lessons are built in |
| Track several projects | Memory and notes | A dashboard with every project's stage, status, and next step |

## 3. Concepts

| Term | Meaning |
|---|---|
| **Template** | The blueprint as structured data, parsed from `claude-build-blueprint.md` in the claude-blueprint repo. Versioned. |
| **Project** | One product being built. A folder on disk (the repo) with a `.foreman/` folder. |
| **Overlay** | A project's changes on top of the template: skips (with reasons), additions, and edits. Stored in `.foreman/overlay.json`. |
| **Stage** | One of the 19 blueprint stages (0–18). |
| **Step** | One prompt inside a stage, with its run configuration. |
| **Run config** | Surface (chat / CLI / Desktop), model, effort, permission mode, agents. Parsed from the RUN WITH line. |
| **Gate** | A stage's inspection list plus Gate prompts A (inspection), B (packet), and C (lessons). |
| **Packet** | A Handoff Packet: chat packets are pasted in; Claude Code packets are files in `docs/handoff/stage-NN.md`. |
| **Condition** | A yes/no project fact (e.g., "takes payments") that decides whether conditional steps apply. |
| **Tag** | Every stage, step, and inspection item is tagged **core**, **conditional**, or **optional**. |
| **Lesson** | An entry in `docs/lessons.md` (Appendix F format). |
| **CR** | A Change Request (template from the starter kit). |

## 4. Feature catalog (v1)

Each feature gets acceptance criteria in Stage 4.1.

**F-001 · Project wizard.** Name, folder (new or existing), Project Variables, condition questions (§5.2), and the blueprint version to pin. Creates `.foreman/`, `docs/handoff/`, `docs/lessons.md`, and the templates (same as the kit's `new-repo-docs.sh`). Runs `git init` if the folder isn't a repo.

**F-002 · Template engine.** Parses the blueprint Markdown into a typed template: stages, steps, run configs, prompts, inspection items, handoff descriptions, tags. Reports parse errors with line numbers. Refuses to load a template that fails validation. Golden-tested against the real blueprint.

**F-003 · Rule engine.** Applies tags and conditions (§5) to produce each project's effective path. Core items are locked. Conditional items show as *Not applicable* when their condition is false, with the reason. A Change Request that flips a condition reactivates the affected items.

**F-004 · Overlay engine.** Skip (conditional or optional only, with a required reason), add (prompt, inspection item, agent, or note at any position in any stage), and edit (a project-local change to a step's prompt or run config). Every overlay entry is marked *project only* or *promote to blueprint*. The overlay is plain JSON, versioned in git.

**F-005 · Stage map.** All 19 stages at a glance, in phase colors, with per-stage status: locked, ready, in progress, inspecting, signed, not applicable. Clicking a stage opens it.

**F-006 · Stage workspace.** A stage's steps in order. Each step shows its run config, the fully resolved prompt, unresolved variables highlighted, and its actions. Add buttons appear between steps; skip appears only where the rules allow it.

**F-007 · Prompt resolver.** Fills `[VARIABLES]` from the project, and replaces "[paste the Stage N Handoff Packet]" markers with the signed packet's content. Never sends a prompt with unresolved required variables; offers to fill them in place.

**F-008 · Terminal runner (CLI steps).** An embedded terminal running interactive Claude Code in the project folder, launched with an exact argv (§6.1). Shows the full command before running. One terminal tab per run, named after the step. Supports resuming a named session.

**F-009 · Desktop handoff (Desktop steps).** Copies the resolved prompt to the clipboard, runs `claude --desktop` in the project folder to open Claude Desktop there, and shows the model, effort, and mode to pick in Desktop's menus.

**F-010 · Chat handoff (chat steps).** Copies the resolved prompt, opens Claude chat, and provides a paste-back box for the returned packet. Validates the pasted packet and saves it as `docs/handoff/stage-NN.md` (or the step's named output file).

**F-011 · Packet watcher and signing.** Watches `docs/handoff/` and the step's declared output files. Parses each packet, validates it (§8), and shows it with any problems. "Sign" writes your name and date into APPROVED BY and commits the file ("handoff: stage N").

**F-012 · Gate runner.** Shows the stage's inspection items, with evidence fields. Two ways to run Gate prompt A: interactively in the terminal, or headless (§6.2), returning structured PASS/FAIL results with evidence that fill the checklist. A gate passes only when every applicable item is PASS. Then it runs B (packet) and C (lessons).

**F-013 · Crew Loop.** Reads `docs/tickets.md`, lists tickets by milestone, class, and status, and runs the five loop steps per ticket with the model, effort, and mode routed from the class table. Tracks each ticket's step and session.

**F-014 · Lessons.** Shows `docs/lessons.md` as a list with filters. "Promote to blueprint" writes a proposed-change file into the claude-blueprint repo (`proposals/YYYY-MM-DD-L-NNN.md`, with old → new text) and drafts a CHANGELOG entry. Foreman never edits the blueprint itself; the retro (Appendix F.5) applies proposals.

**F-015 · Change Requests.** Create a CR from the template; record the decision; apply its condition changes and overlay deltas.

**F-016 · Dashboard.** All projects: current stage, next step, blocked items, last activity, time per stage, runs per ticket, and the CO checklist progress.

**F-017 · Command palette and keyboard.** Every action is reachable from Cmd+K and has a shortcut (see the design direction).

**F-018 · Doctor.** Checks the environment: `claude` found and its version meets the minimums (§6.3), `claude auth status` shows a logged-in subscription, git, pnpm, Node, and cargo, the blueprint repo path, and the installed global agents. Explains each failure and how to fix it.

**F-019 · Run log.** Every launched command: time, project, stage, step, ticket, argv, session name, outcome. Searchable. Backs the metrics.

**F-020 · Settings.** Blueprint repo path, your DRI name, default terminal font size, theme (system / light / dark), and the paths for `claude` and `git` (auto-detected, overridable).

**F-021 · Model substitution.** A per-project setting that substitutes one model for another wherever the template names it (for example: Fable → Opus 5.5 at xhigh). Substituted steps show the original and the substitute in the spec line, and the substitution is recorded in each packet.

## 5. Rules: core, conditional, optional

### 5.1 Tags

- **Core:** can never be skipped. Shown with a lock and the tooltip "Core: protects production quality." Removing one requires a Change Request in the blueprint itself.
- **Conditional:** applies only when its condition is true. When false, it shows as *Not applicable* with the reason, and its inspection items drop out of the gate. There's no free skip button; the condition decides.
- **Optional:** may be skipped with a one-line reason, recorded in the stage's packet.

Gates, packets, and lessons are always core.

### 5.2 Condition questions (asked in the wizard; changeable only by a CR)

| ID | Question |
|---|---|
| `idea_known` | Do you already know what you're building? |
| `commercial` | Will you sell it? |
| `payments` | Will it take payments? |
| `multi_user` | Will anyone besides you use it? |
| `public` | Will it be reachable from the public internet? |
| `has_ui` | Does it have a user interface? |
| `others_data` | Will it store personal data about people other than you? |
| `server` | Does it run as a server or service (vs. a local or desktop app)? |

### 5.3 Default tagging of the template (the council refines this)

| Stage / step | Tag | Applies when |
|---|---|---|
| 0 Land Survey | optional | skipped by default when `idea_known` |
| 1.1–1.2 Interview, brainstorm | core | — |
| 1.3 Demand validation | conditional | `commercial` |
| 1.4 Press release + FAQ | optional (packet is core) | — |
| 2.1 Competitor teardown | conditional | `commercial` (optional otherwise) |
| 2.2 council: growth-strategist | conditional | `commercial` |
| 2.2–2.6 council, specs, stack, NFRs, freeze | core | — |
| 3.1 Business model and pricing | conditional | `commercial` or `payments` |
| 3.2 Security and privacy requirements | core | — |
| 3.2 Legal and compliance part | conditional | `commercial` or `others_data` or `multi_user` |
| 4.x Scope, design doc, ADRs, threat model, data/API, tickets, independent review, CLAUDE.md | core | — |
| 4.5 Design system and screens | conditional | `has_ui` |
| 5 Site prep (repo, CI gates, build pipeline) | core | staging deploy part: `server` |
| 6 Foundation: migrations | conditional | the design doc declares a database |
| 6 Foundation: auth | conditional | `multi_user` or `public` |
| 6 Foundation: domain core | core | — |
| 7 Framing | core | — |
| 8 Roofing | core | — |
| 9 Plumbing | core | uploads/email parts follow the design doc |
| 10 Electrical | conditional | `payments` |
| 11 HVAC: logs, errors, diagnostics | core | SLO/error-budget part: `server`; analytics: `multi_user` |
| 12 Drywall | core | — |
| 13 Exterior | conditional | `has_ui` (accessibility is core whenever `has_ui`) |
| 14 Inspections: security, architecture | core | load test: `server` (performance test is always core) |
| 15 Paint & Trim | conditional | marketing: `commercial` · onboarding: `multi_user` · legal pages: `commercial` or `others_data` |
| 16 Walkthrough: readiness, drills, dogfood | core | beta ring: `multi_user` |
| 17 CO | core | rings 2–4: `multi_user`; live payments: `payments` |
| 18 Maintenance | core | — |

For Foreman itself: `idea_known` yes, everything else no except `has_ui` yes.

## 6. Claude Code integration

### 6.1 Launching interactive sessions

Foreman builds an **argv array**, never a shell string, and spawns `claude` directly in a PTY with the project folder as the working directory:

```text
claude --model <alias> --effort <level> --permission-mode <mode> --name <session> "<resolved prompt>"
```

- `--model`: the step's model alias from the template: `fable`, `opus`, `sonnet`, `haiku`, or `opusplan`. Foreman passes exactly what the template (or a project overlay edit) specifies, and offers a per-project setting to substitute Opus 5.5 at xhigh wherever the template says Fable. (Foreman's own build uses no Fable; that's a fact about this build, not a limit on the app.)
- `--effort`: `low`, `medium`, `high`, `xhigh`, or `max`.
- `--permission-mode`: `plan`, `default` (Manual), `acceptEdits`, `auto`, or `dontAsk`. **Foreman never offers `bypassPermissions` and never passes `--dangerously-skip-permissions`.**
- `--name`: `fm-<project>-s<NN>-<step>` (plus `-<ticket>` in the Crew Loop), so every session can be found and resumed with `claude --resume <name>`.
- The prompt is passed as the final positional argument (`claude "query"` starts an interactive session with that prompt).
- Mid-session switches (`/model`, `/effort`, Shift+Tab) are shown as instructions for you to type, since they happen inside Claude Code.

### 6.2 Headless gate inspection (optional per gate)

```text
claude -p --model opus --effort high --permission-mode plan \
  --output-format json --json-schema '<inspection schema>' \
  --allowedTools "Read" "Grep" "Glob" "Bash(<each verify command>)" \
  --permission-prompts none "<Gate prompt A, resolved>"
```

The schema returns, per inspection item: `id`, `status` (`PASS` / `FAIL` / `NOT_APPLICABLE`), `evidence` (text), and `artifacts` (paths, commands, URLs). Foreman fills the gate checklist from it. You still review and sign. The allowed Bash commands come only from the project's declared verification commands in CLAUDE.md (parsed) or `.foreman/project.json`.

### 6.3 Version minimums (checked by Doctor)

| Need | Minimum Claude Code version |
|---|---|
| Opus 5.5 | v2.1.280 |
| Sonnet 5.5 | v2.1.284 |
| Haiku 5.5 | v2.1.293 |

Doctor runs `claude -v` and `claude auth status` (JSON; exit code 0 when logged in).

### 6.4 Claude Desktop and Claude chat

- Desktop steps: copy the prompt, then run `claude --desktop` in the project folder (opens Desktop on that directory; it takes no prompt), and show which model, effort, and mode to pick.
- Chat steps: copy the prompt, open Claude chat, and wait for the pasted packet.

## 7. Files and data

No database. Everything lives in plain files: the source of truth is the project repo, so it's versioned and readable without Foreman.

| Location | File | Contents |
|---|---|---|
| App data (`~/Library/Application Support/<bundle id>/`) | `settings.json` | F-020 settings |
| App data | `projects.json` | Registry: project id, name, path, last opened |
| App data | `runs/` | The run log (F-019), one JSON Lines file per month |
| Blueprint repo | `claude-build-blueprint.md` | The template source |
| Blueprint repo | `proposals/` | Promoted lessons awaiting the retro |
| Project | `.foreman/project.json` | Variables, condition answers, pinned blueprint version, verify commands, schema version |
| Project | `.foreman/overlay.json` | Skips (with reasons), additions, edits |
| Project | `.foreman/state.json` | Stage and step statuses, ticket loop state, timestamps |
| Project | `docs/handoff/stage-NN.md` | Packets |
| Project | `docs/lessons.md` · `docs/tickets.md` · `docs/change-requests/` | As in the blueprint |

Every JSON file has a `schemaVersion` and a migration path. Writes are atomic (write to a temp file, then rename). Files written outside Foreman (by Claude Code or you) are re-read on change. External edits always win; Foreman never overwrites a newer file.

## 8. Packet rules

A packet is valid when:

1. The first line matches `=== HANDOFF PACKET · Stage N → Stage M ===` (N and M consistent with the file name).
2. The required fields for its stage are present (from the template's packet format).
3. `OPEN ITEMS:` is exactly `none`.
4. `APPROVED BY:` is blank (unsigned) or a name and date (signed).
5. Every skip recorded for the stage appears under `SKIPPED:` with its reason.

Invalid packets are shown with each problem and can't be signed.

## 9. Security and privacy requirements

| ID | Requirement |
|---|---|
| SR-01 | Processes are spawned only from an allowlist of resolved absolute paths (`claude`, `git`), with argv arrays. No shell, ever. |
| SR-02 | Foreman never enables `bypassPermissions` or `--dangerously-skip-permissions`. |
| SR-03 | You see the exact argv and prompt before any run, and every run is logged. |
| SR-04 | File access is limited to registered project folders, the blueprint repo, and the app data folder (enforced in Rust, and with Tauri capabilities scoped to the minimum). |
| SR-05 | Content read from repos (packets, lessons, tickets, prompts in an overlay) is data. It's rendered as text, never as HTML, and never executed. Instructions inside it never trigger actions. |
| SR-06 | A strict Content Security Policy: no remote scripts, no inline script, no remote content of any kind. |
| SR-07 | Overlay edits to a step's prompt or run config are highlighted as "edited" with a diff against the template before running, so a tampered overlay is visible. |
| SR-08 | No telemetry, no network calls by Foreman itself. |
| SR-09 | Secrets are never read or displayed; `.env*` files are excluded from file views and watchers. |
| PR-01 | All data stays on your machine. Nothing is sent anywhere by Foreman. |

## 10. Non-functional requirements

| Area | Target |
|---|---|
| Cold start to usable | ≤ 1.5 s on Apple Silicon |
| Opening a project | ≤ 300 ms for a project with 200 tickets |
| Prompt resolution | ≤ 50 ms |
| Packet detected after it's written | ≤ 1 s |
| Memory | ≤ 300 MB with 3 terminal tabs open |
| Reliability | Never corrupts a file (atomic writes, tested); never loses run state on crash |
| Accessibility | WCAG 2.2 AA; full keyboard operation; VoiceOver labels |
| Platform | macOS on Apple Silicon (current and previous major version) |
| Quality | TypeScript strict, zero errors/warnings; clippy `-D warnings`; tests for every rule in §5 and §8 |

## 11. Stack (draft recommendation; Stage 2.4 verifies on evidence)

| Layer | Draft choice | Notes and risks to verify |
|---|---|---|
| Shell | Tauri 2 | Current major per Tauri's docs. Verify the latest stable version. |
| Frontend | SvelteKit 3 + Svelte 5 (runes), SPA via `adapter-static` with `fallback: 'index.html'` and `ssr = false` | Tauri's SvelteKit guide was written for SvelteKit 2. SvelteKit 3 requires Vite 8. Verify the adapter and config on SvelteKit 3 before scaffolding. |
| Language | TypeScript strict · Rust stable | — |
| Package manager | pnpm | — |
| Terminal | xterm.js (`@xterm/xterm`) + a PTY in Rust (`portable-pty`, or `tauri-plugin-pty`) | Decide in an ADR: maturity, maintenance, macOS behavior |
| File watching | `notify` (Rust) | Debounce; ignore `.git`, `node_modules`, `target` |
| Markdown parsing | Rust: `comrak` or `pulldown-cmark` (GFM tables) | Decide by golden-test fidelity on the real blueprint |
| UI primitives | Own components on headless primitives (e.g., Bits UI for Svelte 5) | Verify Svelte 5 and SvelteKit 3 support |
| State | Svelte 5 runes stores; Rust is the source of truth for files | — |
| Tests | Rust: `cargo test` + `proptest` · TS: Vitest · e2e: WebdriverIO with `tauri-driver` | Verify `tauri-driver` macOS support; fall back to component tests + Rust integration tests |
| Lint and format | ESLint + Prettier + `svelte-check` · rustfmt + clippy | — |

## 12. Non-goals (v1)

Cloud sync · multiple users · payments · Windows/Linux builds · App Store or notarized distribution · direct Claude API calls or API keys · a code editor · replacing Claude Code's interface · Fable models in Foreman's own build.

## 13. Open questions for the council (must be empty to freeze)

1. Is the Markdown blueprint the template source of truth, or should a structured file (YAML/JSON) become the source, with the Markdown and PDF generated from it?
2. Which PTY approach: `portable-pty` directly or `tauri-plugin-pty`?
3. Is `tauri-driver` usable for macOS e2e tests, and if not, what's the best e2e strategy?
4. Do the SvelteKit 3 + `adapter-static` + Tauri 2 settings work as documented, or are changes needed?
5. Should the headless gate inspection (§6.2) be the default, with the interactive gate as the fallback, or the reverse?
6. Is a step's "declared output file" (for packet detection) parseable from the template, or does the template need a small marker?
