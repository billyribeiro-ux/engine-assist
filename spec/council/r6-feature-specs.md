# Round 6 · Feature specifications (prompt 2.3)

Chair: Opus 5.5 (xhigh) · Date: 2026-10-19
Inputs: App Brief (`docs/handoff/stage-01.md`), `r1-engineering.md`, `r1-workflow.md`, `r2-proposals.md`, `r3-synthesis.md`, `r4-redteam.md`, `r5-resolution.md`, `r7-stack.md`, `r8-nfr.md`, design direction v0.9.

This file is the build specification for every v1 feature, after the red team and the independent freeze review. Where it conflicts with an earlier round, this file wins; `r5-resolution.md` and `r5b-freeze-review.md` explain why each change was made. Phases match Master Spec §9.

**Phases** (from r5 §3): **P1** = the thin end-to-end version that ends with the kill check. **P2** = the rest of v1, built only if the kill check passes. A feature marked "P1 + P2" ships a stated subset in P1.

**"Beats".** Foreman has no market (personal tool; Stage 2.1 teardown doesn't apply). The competitor every feature must beat is **running the blueprint by hand** (the PDF or Markdown, copy and paste, terminal flags typed from memory), measured by the baseline in `docs/metrics-baseline.md`. Where an existing tool covers part of the job, it is named. Claims about other tools are limited to their documented behavior.

**Definitions used throughout.**
- *Manual prompt edit*: any change Billy makes to resolved prompt text outside the step's declared run inputs (r2 §0).
- *Applicable path*: the ordered list of stages and steps that apply to a project after tags and conditions.
- *Decision files*: `.foreman/project.json`, `.foreman/overlay.json`, `.foreman/blueprint.lock.json`.
- *App data*: `~/Library/Application Support/<bundle id>/`.

**Ambiguities.** Prompt 2.3 asks for ambiguous points as numbered questions. **There are none left.** Where a choice had to be made, it is stated in the spec as a decision, and the decisions Billy might want to revisit are collected in §5.

---

## 1. Catalog

| ID | Feature | Phase | Size | Replaces (v0.9) | From |
|---|---|---|---|---|---|
| F-001 | Template engine | P1 (Stages 1–2, F.2) + P2 | M | F-002 | P-01, R-05, R-06, R-26 |
| F-002 | Blueprint lock | P1 | S | — | P-02, R-21 |
| F-003 | Template health and the TradeLog harness | P1 (harness) + P2 (screen) | S | — | P-03 |
| F-004 | Rule engine: tags, conditions, applicable path | P1 | M | F-003 | §3.2 of r3 |
| F-005 | Run configs and model policy | P1 | M | F-021 | P-04 |
| F-006 | Prompt resolver | P1 + P2 | M | F-007 | P-09 |
| F-007 | Step kinds and states | P1 | S | — | P-10 |
| F-008 | Launch pipeline | P1 | M | — | P-07, R-03, R-20, R-25 |
| F-009 | Terminal runner | P1 + P2 | M | F-008 | P-08, R-23 |
| F-010 | Session chains and re-arm | P1 | M | — | P-05, R-08, R-09 |
| F-011 | Session sensor | P2 | M | — | P-06, R-16 |
| F-012 | Chat bridge | P1 + P2 (ADR split) | L | F-010 | P-11 |
| F-013 | Packet engine | P1 + P2 (repair, capture) | M | F-011 (part) | P-13, R-10 |
| F-014 | Signing and provenance | P2 | M | F-011 (part) | P-13, R-10, R-18 |
| F-015 | Gates | P2 | M | F-012 | P-14, R-07, R-14, R-15 |
| F-016 | Lessons capture | P2 | S | F-014 (part) | P-11, F.2 |
| F-017 | Today and Do next | P1 + P2 | M | F-016 | P-15 |
| F-018 | Projects and the wizard | P1 + P2 | M | F-001 | P-16 |
| F-019 | First run, Doctor, contract probes | P1 + P2 | M | F-018 | P-16, R-09, R-12, R-19, R-25 |
| F-020 | State, persistence, resilience | P1 | M | — | P-17, R-22 |
| F-021 | Path policy | P1 | S | — | R-11 |
| F-022 | Outside-change review | P1 | S | — | R-02 |
| F-023 | Knowledge manifest and build-package assembler | P2 | M | — | P-12 |
| F-024 | Desktop handoff | P2 | S | F-009 | P-23 (minimal), R-08 |
| F-025 | Add to a stage (this project only) | P2 | S | F-004 (part) | P-21 (minimal) |
| F-026 | Run log and metric panel | P1 + P2 | S | F-019 | P-18 |
| F-027 | Run sheet export | P2 | S | — | NFR-14 (pulled from P-25) |
| F-028 | Command palette and keyboard | P1 + P2 | S | F-017 | design §7 |
| F-029 | Settings | P1 + P2 | S | F-020 | — |
| F-030 | Sheet strip and stage workspace | P1 (workspace) + P2 (strip) | M | F-005, F-006 | design §4–6 |

**v1.1** (scheduled after Foreman's Certificate of Occupancy, each designed in r2 and amended in r5): F-101 headless Gate A (hardened argv, after S-2′), `auto` items and approved verify commands (r5 §2.3) · F-102 Crew Loop on CLI worktrees (P-19) · F-103 Change Request track (P-20) · F-104 full overlay authoring and promote-to-blueprint (P-21) · F-105 usage meter (P-22) · F-106 live send and "go" (P-24) · F-107 lessons view, retro starter (P-25 remainder) · F-108 project metrics dashboard (v0.9 F-016).

---

## 2. Template and rules

### F-001 · Template engine

**Starting situation.** The blueprint is a 2,235-line Markdown file in Billy's `claude-blueprint` repo. In v2.2 it carries `fm:` annotations on every step Foreman runs (blueprint proposal B-1).

**Behavior.**
1. Parse the Markdown with comrak (`=0.56.0`, sourcepos on). Walk the AST once and collect stages (`## Stage N:`), steps (`### Prompt N.M:`, `### Gate prompt X:`, Council setup, F.2), RUN WITH blockquotes, fenced `text` prompt blocks, INSPECTION blockquotes, HANDOFF blockquotes, packet formats (fenced blocks starting `=== HANDOFF PACKET`), and `fm:` comments.
2. Read `<!-- fm:grammar v=1 -->`. Grammar v1 is defined in `spec/blueprint-v2.2/PROPOSAL.md` §B-1. An unknown key is an error within v1, a warning for a higher version.
3. **Annotations are the only source** of `id`, `kind`, `tag`, `when`, `surface`, `cwd`, `session` (`new` or `continue`), `chain` (a step ID, or `@stage` for the chain of the stage's packet-producing step, used by F.2), `start`, `switch`, `alt`, `in`, `out`, `appends`, `requires`, and every `fm:verify` entry. All paths are **repo-relative**.
3a. **`fm:if <expr>` … `fm:end`** wraps a conditional part of a prompt (v1). The part is removed when the expression is false and recorded under SKIPPED. Nested blocks are an error.
4. **Prose lint.** Inference rules (r2 P-01 behavior 1) run over the RUN WITH text and produce **warnings only** when they disagree with the annotation. They never supply values.
5. A step with no `fm:step` is `needs-annotation`: shown, never runnable.
6. `fm:verify` must have one entry per bullet. Each item's ID, kind and the SHA-256 of its normalized text (lowercase, whitespace collapsed, Markdown emphasis removed) are written to the project's lock at pin (F-002). On every open, each bullet's current hash is compared with the lock; a difference shows "re-confirm kind" and the item can't be attested until Billy confirms (stored in app data `approvals`).
7. Validation rules (each an error with file, line and column):
   - duplicate step or item IDs;
   - a `session=continue` step with no earlier step in the same stage to join, or a `chain=` naming an unknown or later step;
   - a continuing step declaring a cwd different from its chain;
   - `start` values outside the known model aliases (`fable`, `opus`, `sonnet`, `haiku`, `opusplan`), effort levels (`low`…`max`, `ultracode`) and modes (`plan`, `default`, `manual`, `acceptEdits`, `auto`, `dontAsk`). `bypassPermissions` is always an error;
   - a condition ID in `when` or `fm:if` that isn't one of the twelve defined conditions;
   - an `auto` kind (v1.1 only: v1 reports "auto items need Foreman v1.1" and treats the item as attest);
   - verify entries ≠ bullets;
   - a path that fails the path policy (F-021) in `out`, `appends` or `requires`.
8. Output: a typed `Template { grammarVersion, blueprintVersion, sourceSha256, stages[], steps[], items[], packetFormats[], diagnostics[] }`, cached in app data by source hash.

**Inputs / outputs.** Input: blueprint text. Output: `Template` + diagnostics. No side effects.

**Edge cases and failures.**
- Blueprint not found or unreadable: First run (F-019) shows "Choose your blueprint repo"; existing projects open read-only from their cached lock (F-002).
- Errors in a stage a project doesn't use: the project still opens; Template health lists them; steps in that stage are not runnable.
- An `fm:` comment inside a list item or inline: error "annotations must be block-level", with the line.
- CRLF line endings, BOM, tabs: normalized before hashing.
- A 20,000-line blueprint: still meets NFR-03 (≤ 50 ms).

**Done when.**
- Golden test: the real v2.2 blueprint parses with zero errors; the snapshot of all steps in Stages 1–5, the Gate and F.2 matches (insta).
- Property tests: random valid annotations round-trip; random key orders parse identically.
- Swapping two inspection bullets without moving their IDs produces "re-confirm kind" on both (R-06).
- Changing "same session" to "fresh session" in prose produces a lint warning and no change in behavior.
- P1 coverage: Stages 1–2 and F.2. P2: Stages 0–5 and the Stage Gate.

**Beats.** By hand, the RUN WITH line is read and retyped every time; nothing catches "same session" vs "new conversation" mistakes. Foreman makes every run setting explicit, checked, and identical every run.

### F-002 · Blueprint lock

**Behavior.**
- Pinning a project records in `.foreman/blueprint.lock.json`: `{ schemaVersion, blueprintRepoCommit, blueprintPath, sourceSha256, blueprintVersion, grammarVersion, pinnedAt, items: { <itemId>: { textSha256, kind } } }`.
- The full source text is stored in app data at `templates/<sourceSha256>.md`. The repo carries no copy (R-21).
- On open, the project loads its template from app data by hash. If missing (new machine, cleared app data), Foreman runs `git -C <blueprint repo> show <commit>:<path>`, checks the hash, and restores it. If the blueprint repo is absent or the commit is gone, the project opens read-only with "Blueprint 2.2 at <commit> not found. Point Foreman at your blueprint repo or re-pin."
- A dirty blueprint working tree can't be pinned; the dialog shows `git status` for that file.
- The 4.8 packet's `BLUEPRINT VERSION` must equal the lock's `blueprintVersion`.
- Upgrading the pin is v1.1 (F-104). In v1, re-pinning is allowed only before Stage 1 is signed.

**Done when.** Unit tests for restore-from-git, hash mismatch (refuse), missing repo (read-only), dirty tree (refuse).

**Beats.** By hand there's no record of which blueprint version produced a project's packets.

### F-003 · Template health and the TradeLog harness

**Behavior.**
- **Template health** (screen, and step 2 of first run): blueprint version; stage, step and item counts; errors and warnings grouped by stage; for each runnable step its annotations, lint warnings and declared outputs; a row per stage marked runnable / partly / needs annotation.
- **TradeLog harness** (test suite, not UI). Fixture: the real blueprint; a TradeLog `project.json` from the blueprint's example variables (all conditions yes); synthetic signed packets for Stages 1–4. It resolves every runnable step through Stage 5, plus the Gate prompts, and asserts: zero unresolved tokens; every run input declared; every CLI step has cwd, session and start; the argv snapshot for each step matches.
- P1 runs the harness for Stages 1–2. P2 extends it to Stage 5 and adds the screen.

**Done when.** The harness is part of `cargo test` and blocks merge.

**Beats.** By hand, gaps show up mid-run. This proves the success metric "zero manual prompt edits" before Billy runs a step.

### F-004 · Rule engine: tags, conditions, applicable path

**Behavior.**
- Tags: `core` (never skippable; lock icon), `conditional` (applies when its `when` expression is true; shown as not applicable with the reason otherwise; no skip button), `optional` (Skip with a required one-line reason). Gates, packets and lessons are core.
- Conditions are tri-state: `yes`, `no`, `undecided`. A `when` expression that references an undecided condition makes the step `undecided`: shown, not runnable, with "Decide <condition> to continue".
- **Early conditions** (asked at project setup): `idea_known`, `commercial`, `payments`, `multi_user`, `public`, `has_ui`, `others_data`, `server`. **Late conditions** (asked at the Stage 4 sign, F-023): `has_database`, `has_uploads`, `has_email`, `has_integrations`.
- Conditions are editable, each change appending `{condition, from, to, at, note}` to `project.json`'s audit list, **until the Stage 2 packet is signed**. The Stage 2 sign dialog has a Confirm conditions checkpoint; any `undecided` early condition blocks it. After that, a condition shows "Change requires a Change Request (v1.1)" with the CR template path.
- `tag=optional` with `when`: not applicable when the expression is false; skippable with a reason when true.
- Applicable path = stages and steps in blueprint order, filtered by tags, conditions and skips. Recomputed on every change (pure function).
- The default tagging table is Master Spec §5, carried into the blueprint annotations by B-1 for Stages 0–5.

**Edge cases.** A skip on a step that becomes core after a pin change: the skip is ignored and flagged. A stage with every step not applicable: the stage shows as not applicable (hatched), its gate isn't run, the next stage's packet remap (F-006) points at the last signed packet, and its reason is written into the next signed packet's SKIPPED.

**Done when.** Unit tests for every tag × condition combination; a property test that the applicable path never drops a core item.

**Beats.** By hand, "does this step apply?" is decided from memory each time, and skips are invisible later.

---

## 3. Running steps

### F-005 · Run configs and model policy

**Behavior.**
- `RunConfig = { surface: chat | cli | desktop, cwd: project | spec, session: new | continue, chain?, start: {model, effort, mode?}, switches[], alts[], agents[] }`. (`blueprint` and `worktree` cwd values exist in the grammar; v1 rejects them as "v1.1".)
- **Model policy** per project: `plan: max | pro` selects among `alt=` entries; substitution rules map a model to another (for Foreman's own build: `fable → opus@xhigh`). The spec line shows the original and the chosen value, for example `Fable 5.1 · high (Max) → Opus 5.5 · xhigh (substituted)`. A per-run chip overrides it; every override is logged and written under SUBSTITUTIONS at signing.
- **Explain on hover**: each spec-line value says where it came from (template, alternative rule, substitution, override).
- **Switches** are in-step cues shown in the Now/Next strip (F-009), never automated keystrokes.

**Done when.** Unit tests for alternative selection and substitution precedence (override > substitution > alternative > template).

**Beats.** By hand, the Max/Pro alternative and the no-Fable rule are applied from memory, and nothing records which model actually ran.

### F-006 · Prompt resolver

**Behavior.** Every bracket token in a prompt is classified at parse time into one of six classes, and resolution is a pure function of (template, project, signed packets, run inputs, context):

| Class | Examples | Resolution |
|---|---|---|
| Project variable | `[APP_NAME]`, `[QUALITY_BAR]`, `[PREFERRED_STACK]` | `project.json`. A value equal to a blueprint placeholder counts as unresolved. |
| Packet marker | `[paste the Stage 1 Handoff Packet]`, `[paste the Stage 2 and Stage 3 Handoff Packets]`, `[paste the stage's inspection list]` | Signed packet text, joined in order. The inspection list is filtered to applicable items. |
| Path alias | `[app]-spec/`, `stage-[N-1].md`, `[N+1]` | `[app]-spec/` → the project's spec folder (`spec/`). Packet paths → the latest signed packet on the applicable path; `[N+1]` → the next applicable stage. Each remap is shown as a note on the step. |
| Run input | `[X]`, `[changes]`, `[paste numbers]`, `[jurisdictions]`, `[list screens]` | An inline field, from the step's `in=` annotations (label, type text/multiline, `remember` stores the value per project). Context inputs (`[N]` from the stage) fill themselves. |
| Optional part | `[optional: write a press release …]` | Shown with an **Include** toggle (default on). Off removes the text and records it under SKIPPED. |
| Literal | Brackets in code samples, `[1m]`, Markdown links | Left alone. |

- `fm:if` parts are removed when their expression is false. Repo-relative paths from annotations are rendered relative to the step's cwd (for a `cwd=spec` step, `docs/handoff/stage-02.md` becomes `../docs/handoff/stage-02.md`).
- Unresolved required tokens render as attention pills; clicking one focuses its field. Run is disabled until all are resolved.
- **P1 subset:** the tokens used in Stages 1–2, the Gate and F.2. P2: through Stage 5. Any token the classifier doesn't know is a template error, not a guess.

**Edge cases.** A packet marker whose stage was not applicable: resolved to the latest signed packet before it, with the remap note. A packet marker whose packet is unsigned: Run blocked, "Sign Stage N first". A run input longer than 64 KB: accepted, with a warning that chat paste limits may apply.

**Done when.** Resolution of every step in the harness (F-003) with zero unresolved tokens; p95 ≤ 50 ms (NFR-04); golden tests pin classification of every bracket token in the blueprint.

**Beats.** By hand, variables and packets are pasted into each prompt, which is exactly where the "manual prompt edit" metric is lost.

### F-007 · Step kinds and states

**Behavior.**
- Kinds: `prompt`; `followup` (an extra prompt in the same session; v1: **Copy and focus terminal**, Billy pastes and presses Enter); `action` (Foreman does it, from a fixed Rust vocabulary: `write-blocks` for Council setup into `spec/.claude/agents/<known names>.md`, `copy-file` for the Stage 1 brief into `spec/01-app-brief.md`, `mkdir`; destinations are hard-coded per action ID, never taken from template text, R-02); `manual` (a checklist item with optional **Open file**; a change to the named file after the step started satisfies it); `wait` (until a date, with a macOS notification); `repeatable` (run counter).
- States: `not-started`, `copied`, `running`, `done`, `skipped(reason)`, `n/a(reason)`, `waiting-until(date)`, `undecided`, `needs-annotation`. `adopted` is reserved in the enum only (R-24).
- Chat steps: **Mark done** (⌘D), implied by a valid paste-back.

**Done when.** State-machine tests: every transition legal or rejected with a reason.

**Beats.** By hand, "Council setup", "record your final choice" and the 7-day wait live in Billy's head.

### F-008 · Launch pipeline

**Behavior.** Every launch goes through one Rust function, `prepare_launch(step) -> Result<LaunchPlan, Vec<Blocker>>`, in ≤ 50 ms with cached data. Checks, in order:
1. **Version.** `claude -v` is read on every launch (cheap; no 24 h cache). Required version = max of model minimums, flag minimums and the pinned kit's agent models (floor 2.1.293 with the current kit, R-19). Mismatch: Run disabled, banner with **Run `claude update`**.
2. **Contract** (P2). If the version differs from the last verified version, the contract probes run first (F-019). A failed probe disables only the dependent feature; an inconclusive one keeps the last verified state and warns.
3. **Config fingerprint (R-03, r5b M8).** SHA-256 over: user settings (`~/.claude/settings.json`, or `$CLAUDE_CONFIG_DIR/settings.json`); managed settings at their documented macOS location; every `CLAUDE.md` from the step cwd up to the repo root; `CLAUDE.local.md`; the cwd's `.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json`, `.claude/agents/**`, `.claude/skills/**`, `.claude/commands/**`, `.claude/rules/**`. Changed since the last launch in that cwd: a banner with the diff and **Acknowledge** (one click). Auth-changing keys (`apiKeyHelper`; `env` keys `ANTHROPIC_*` or `CLAUDE_CODE_USE_*`; `awsAuthRefresh`, `awsCredentialExport`, `gcpAuthRefresh`) **block** wherever they appear, user settings included, with the file and key.
4. **Auth.** `claude auth status` run in the step cwd with the launch environment, cached against the full fingerprint. `authMethod` must be `claude.ai` or `oauth_token`; anything else blocks.
5. **Transcript (continuing steps).** The chain's transcript file must exist (R-09); otherwise the chain is `broken` (F-010).
6. **Inputs.** Every repo path the prompt references must exist; a missing signed packet blocks; other missing paths warn ("02-teardown.md not found: 2.1 was not applicable").
7. **Run inputs.** All required inputs filled.
8. **Prompt hygiene.** A resolved prompt starting with `-`, `/` or `!` blocks (R-20).
9. **Build argv** (r5 §2.2) and the **environment** (explicit allowlist; `ANTHROPIC_*`, `CLAUDE_CODE_USE_*`, AWS, GCP and Azure variables always removed; `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` passed when present; PATH per r5 §2.4).
10. **Run settings file** written fresh to `<app-data>/runs/<runId>/settings.json`: sensor hooks (P2), `disableAllHooks: false`, `permissions.deny: ["Read(./.foreman/**)"]`; its hash is recorded.
11. **Preview.** The step block always shows the resolved prompt and the exact argv (collapsed, one click to expand). That is the SR-03 preview. A confirmation modal appears only for: an overlay-added step, a mode different from the template's, the first launch in a project, or `auto`/`dontAsk` in an interactive session.
12. **Run record** written (status `running`) before spawn (F-020).

**Never:** a shell; `--bare`; `bypassPermissions`; `--dangerously-skip-permissions`; `--allow-dangerously-skip-permissions`.

**Done when.** A fake `claude` binary records argv and environment; tests assert every step in the harness produces the expected argv, an allowlisted environment, and the run record before spawn. Each blocker has a test.

**Beats.** By hand, flags are typed from memory; an inherited `ANTHROPIC_API_KEY` or a repo `apiKeyHelper` can silently move billing off the Max plan.

### F-009 · Terminal runner

**Behavior.**
- xterm 6 (WebGL renderer, DOM fallback) bound to a portable-pty child per run, in the terminal drawer; one tab per chain, named `fm-<project>-s<NN>-<step>`. Output flows over a Tauri Channel; input and resize go back as commands.
- Tab states: `running`, `idle` (P2, sensor), `blocked` (P2), `exited(code)`, `errored`, `interrupted`. Tab menu: Interrupt (Ctrl-C), Terminate (SIGTERM, then SIGKILL after 5 s, with a confirmation when not idle), Copy session ID, Resume.
- Scrollback capped at 10,000 lines per tab. On re-arm, the old buffer is archived as a collapsed read-only section and a fresh buffer starts (R-23). The last 2,000 lines of each run are kept in `runs/<runId>/output.log` (pruned after 90 days).
- **Key routing.** With the terminal focused, ⌘-chords and ⌃` reach the app and everything else goes to the PTY, except ⌘C (copy selection), ⌘V (paste into the PTY), ⌘A (select all) and ⌘F (find), which the terminal handles. ⌘K opens the palette from the terminal; ⌘⇧E returns focus to the workspace (P1, so the terminal is never a keyboard trap).
- **P2 additions:** the **Now/Next strip** (`Now: Plan · Opus 5.5 · xhigh │ Next: after you approve the plan, choose Accept edits`, with live mode from the sensor and an `--attention` state on mismatch); the **round tracker** for steps with several `out=` files (driven by the watcher); **focus mode** (⌘⇧T); **Open output** on each written `out` file.
- Accessibility: when VoiceOver is on, CLI launches add `--ax-screen-reader` (CLI v2.1.181+), and xterm's `screenReaderMode` is on.

**Edge cases.** The child exits during spawn: tab `errored` with stderr. PTY write fails: tab `errored`, run record updated. Foreman quits with running tabs: quit guard ("3 sessions are running; they end when Foreman quits; you can resume each later").

**Done when.** S-0 passes (r5 §4). Integration tests with the fake binary: spawn, output, input, resize, exit codes, terminate.

**Beats.** Terminal.app gives a session but no idea which step it belongs to or what comes next.

### F-010 · Session chains and re-arm

**Behavior.**
- A chain is one Claude conversation: `{ chainId (session UUID), projectId, cwd, steps[], status: open | closed | superseded | handed-off | broken }`, stored in `progress.json`.
- `session=new`: Foreman generates a UUID and launches with `--session-id <uuid>` (only here; never with `--resume`, R-08).
- `session=continue`: joins the previous step's chain (or `chain=`). **Re-arm:**
  1. **Idle check.** P1: Billy confirms "Claude is idle and nothing is running in the background" (one click; the button says what it asserts). P2: the sensor decides: last event `Stop` and no outstanding subagent or background task; otherwise "2 background tasks still running: Wait / End anyway".
  2. **End.** Send Ctrl-C (clears the input box), then `/exit` + Enter. Wait up to 10 s for process exit (P1) or `SessionEnd` with reason `prompt_input_exit` (P2). If it doesn't end: "Claude didn't exit. End it yourself, then press Continue." Foreman never SIGTERMs a non-idle session without explicit confirmation.
  3. **Relaunch** in the same tab: `claude --resume <uuid>` + the step's full flags, `--settings`, and the resolved prompt. A divider is written: `── 2.3 Feature specifications · Opus 5.5 · xhigh · Accept edits · permission grants reset ──`.
- Re-arm always starts from Billy's click on Run; no sensor event starts it.
- If the chain's process already exited, re-arm is just the relaunch.
- Re-running a chain-start step creates a new chain; the old one is `superseded`.
- **Desktop handoff** (F-024) marks the chain `handed-off`. The next continuing step relaunches with `--fork-session` (a new chain recorded as forked from the old one), unless Billy confirms Desktop is closed. The forked chain's UUID: if S-8 shows `--resume <uuid> --fork-session --session-id <new>` works, Foreman chooses it; otherwise it's the only transcript created in the project's transcript folder after launch, and if that's ambiguous, Foreman starts a fresh chain with context.
- **Broken chain** (transcript missing, R-09): the step shows "This conversation no longer exists (Claude Code removes transcripts after `cleanupPeriodDays`)" and **Start a fresh chain with context**: a new session whose prompt is the step's prompt preceded by the last signed packet and the paths of the step's declared inputs.
- Effort on resume follows S-3's result (r5 §4). If neither the flag nor `effortLevel` applies on resume, Foreman shows the `/effort <level>` line for Billy to type, with the note that it changes his saved default for that model.

**Done when.** Fake-binary tests for: new chain argv; continuing argv (no `--session-id`); Ctrl-C then `/exit` written in that order; superseded and forked chains; broken-chain detection. S-8 passes on Billy's Mac.

**Beats.** By hand, every "same session" step needs `/model`, `/effort` and Shift+Tab typed in the right order inside the TUI. Foreman launches each step with exactly the template's settings.

### F-011 · Session sensor (P2)

**Behavior.**
- The run settings file adds hooks for `SessionStart`, `Stop`, `StopFailure`, `Notification`, `PermissionRequest`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, and `PostToolUse` (matcher `Bash`). Each uses the **exec form** (`command` + `args`), never a shell string, pointing at `<app-data>/bin/foreman-sensor-<hash>` (copied there at startup from the bundle), with the per-run token as an argument.
- `foreman-sensor` reads stdin JSON, checks the token, and writes one file per event into `<app-data>/events/<runId>/` (atomic create). It prints nothing and exits within 10 ms.
- Foreman derives: `working`, `idle`, `blocked:permission`, `blocked:input`, `blocked:plan-approval`, `ended(reason)`; live `permission_mode` and `effort.level` (the **mode guard**: a visible warning when they differ from the step's, never enforced); outstanding subagents and background tasks; `last_assistant_message` (for packet capture and gate import); the observed model at `SessionStart` (provenance, F-014).
- Retention: the last 20 events per run; all events deleted when the run's stage is signed.
- The token stops cross-run mix-ups and processes of other users; it does not stop a forger running as Billy (for example, a command in Claude's Bash can read the settings file). That's within the trust boundary because no event triggers an action by itself: events only update displayed state and enable buttons Billy clicks.
- No fallback that touches `~/.claude/settings.json` (R-16). If S-1′ fails, the sensor isn't shipped and the P1 manual behaviors stay.

**Done when.** S-1′ passes; unit tests for every derived state; a forged event with a wrong token is ignored.

**Beats.** Claude Code's agent view lists background sessions; it isn't wired to blueprint steps. The sensor tells Billy which step is waiting on him.

### F-012 · Chat bridge

**Behavior.**
1. **Chat setup strip** in the step header, as checkable chips: **New conversation in Project <name>** or **Same chat as 1.1** (bold); model; Web search ON; Research ON. They're memory aids and say so.
2. **Chat links.** The project stores its claude.ai Project URL (set in project setup). Each chat chain stores its conversation URL after **Link this chat** (paste, or From clipboard on an explicit click). **Copy and open** (⌘↵) copies the resolved prompt and opens the right URL in the default browser through the opener plugin: the Project for a new conversation, the stored conversation for a continuing step. Foreman never depends on URL prefill.
3. **Paste-back** (⌘⇧V on the step) accepts:
   - a **packet**: normalized (`<blank>`, `<leave blank…>`, `<sign here…>` → unsigned; `None`/`none.` → `none` with a note; smart quotes and NBSP fixed). Text from the header line to the format's last field is the packet; anything after it goes to `docs/handoff/stage-NN-attachments.md`. Declared extra destinations (from annotations) are written too. Every write is listed in a confirmation line before it happens.
   - an **output**: saved to the step's `out` path; an existing file shows a diff first.
   - an **ADR set** (P2): split on `^# ADR-\d{3}:` into `docs/adr/ADR-NNN.md`.
4. **Copy fix request** for an invalid packet: "Your packet is missing: X, Y. OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block."
5. **Chat-stage close-out** (P2; by hand in P1): attest items (F-015), then the F.2 lessons step (F-016), then Sign (F-014).

**Edge cases.** Clipboard empty or not text: inline error. Paste of the wrong stage's packet: "This is the Stage 3 packet; this step expects Stage 2." Paste with two packets: rejected with "paste one packet at a time". All writes pass the path policy (F-021).

**Done when.** Golden tests with real packets from Foreman's own Stage 1–2 (in `docs/handoff/`), including the attachment split and every normalization.

**Beats.** By hand, chat output is copied into files manually, `<blank>` packets fail later checks, and the 1.4 press release ends up inside the packet.

---

## 4. Packets, signing, gates, lessons

### F-013 · Packet engine

**Behavior.**
- The watcher (notify 8.2 + debouncer-full, ~250 ms) covers `docs/handoff/` and each runnable step's declared `out` paths. It ignores `.git`, `node_modules`, `target`, `.claude/worktrees/**` and `*.foreman-tmp`.
- **Validation:**
  1. The header matches `=== HANDOFF PACKET · Stage N → Stage M ===`, N matches the file name, M is the next **applicable** stage.
  2. Required fields from the stage's packet format are present.
  3. `OPEN ITEMS:` is exactly `none`.
  4. `APPROVED BY:` is unsigned (normalized) or `<name> on <YYYY-MM-DD>`.
  5. `BLUEPRINT VERSION` (4.8) equals the lock.
  6. Every recorded skip and not-applicable item for the stage will appear under `SKIPPED:` (Foreman writes it at signing).
- P1: watch, validate, normalize, show problems. P2: fix actions. Each problem has a fix action: **Repair** (CLI packets: re-arms the stage's chain with "Fix only these problems in docs/handoff/stage-NN.md: … Do not change anything else."), **Open in editor**, or **Copy fix request** (chat packets). When OPEN ITEMS isn't `none`, the view lists them and states the two allowed outcomes: resolve them, or move them to KNOWN LIMITS with a ticket ID.
- **Packet capture** (P2) for a step that printed its packet instead of writing it: **Capture from session** uses the sensor's `last_assistant_message`; otherwise paste-back (P1). Blueprint v2.2 B-2 makes this rare. **v1 makes no `claude -p` calls** (r5b B3).

**Done when.** Unit tests per rule; fixtures with every failure.

**Beats.** By hand, a packet with a missing field or `OPEN ITEMS: TBD` is found by the next stage, too late.

### F-014 · Signing and provenance

**Behavior.**
- **Close-out order enforced** (Master Spec §8): chat stages: attest → F.2 → Sign; Stage 2: attest → F.2 followup in the 2.2 chain → Sign; Stages 5–17: Gate A → B → C → Sign. Sign is disabled, with the reason, until: the gate is passed (F-015); a lessons entry for the stage exists ("clean stage" counts); the stage's chain is idle or ended (or Billy confirms otherwise).
- **Sign dialog:** name and date (DRI name from Settings; Billy types or confirms); branch (**current branch, the default**; or **Sign on another branch**); **Fields Foreman will add**, as a diff: `SKIPPED:`, `SUBSTITUTIONS:` (model policy, overrides, mode substitutions), `ATTESTED:` (each attest item with its note or n/a reason); the content hash ("Signing stage-05.md @ a3f9c1"); Stage 2 only: **Confirm conditions**.
- **Phase 1:** re-read the file and compare hashes (refuse and re-render on mismatch); write APPROVED BY and the added fields atomically; write `.foreman/stages/stage-NN.json` (step states, run IDs, intended and observed models/efforts/modes, prompt SHA-256s, gate results, attestations, durations, packet hash after signing; session UUIDs as links).
- **Phase 2:** commit only the named paths (packet, attachments, stage record, decision files with pending changes). Every git call Foreman makes passes `-c core.hooksPath=/dev/null -c core.fsmonitor=false -c core.pager=cat -c diff.external= -c core.editor=true`, with `GIT_TERMINAL_PROMPT=0` and a 60 s timeout, so no repo hook runs (pre-commit, prepare-commit-msg, post-commit included); this is recorded in the stage record. **Current branch:** `git add -- <paths>`, then `git commit --only -m "handoff: stage NN" -- <paths>`. **Another branch:** plumbing only: `hash-object -w` for each signed file, a temporary `GIT_INDEX_FILE` read from that branch's tip, `update-index --cacheinfo`, `write-tree`, `commit-tree`, `update-ref`. In this mode Phase 1 writes nothing to the working tree; the stage shows "signed on <branch> (not merged)" and the resolver reads the packet from that ref.
- **Read-back:** `git show <commit>:<path>` for every path; hash compare; mismatch shows "Your repo's commit hooks changed the packet" with the diff.
- State `signed (commit pending)` until git succeeds. On failure: stderr, **Retry commit**, **Open terminal here**. The stamp animation runs only after read-back passes.
- After signing, a change to the signed packet on disk is a **tamper event**: Today shows the diff; the committed version is authoritative; **Restore committed version** is one click.
- Stage records are read ref-aware; a record on an unmerged branch shows "signed on handoff/stage-05 (not merged)".
- `commit.gpgsign` with a GUI prompt: the 60 s timeout ends the wait with a clear message.

**Done when.** Fixture repos: lint-staged + Prettier on Markdown (no hook runs; read-back passes); a new untracked packet and stage record (committed correctly); another-branch signing (working tree and HEAD unchanged); a dirty working tree with unrelated staged files (not included in the commit); signing while the file changes (refused).

**Beats.** By hand, `git add -A` sweeps unrelated work into the signing commit, and nothing records what was skipped or substituted.

### F-015 · Gates

**Behavior.**
- Each inspection item has a kind from `fm:verify`. **v1 kinds are `attest` and `time`;** `auto` arrives in v1.1 with F-101 (r5b M3).
  - `attest`: Billy checks **Done** with a note (≥ 15 characters, not repeated within the gate, optional linked file or URL) or **Not applicable** with a reason. The item's "how to check" text sits next to it.
  - `time:<n>d`: available after the date, then as declared.
- **Gate A, interactive** (P2 for CLI stages): runs in the terminal as the blueprint says (its own chain). **Import results** proposes PASS/FAIL per item from the sensor's last message by matching item text; Billy confirms each. Without the sensor, Billy marks each item.
- **Gates B and C** continue the gate chain (`chain=gate.A`).
- Review order: FAIL, then waiting attest items, then PASS (collapsed).
- **After a FAIL** (from Import results or marked by Billy): **Fix this** opens a new CLI chain (Opus 5.5 · high · Plan, then Accept edits) prefilled with the item and evidence; a third failure of the same item shows the F.4 prompt.
- Gate state: `not-ready → A → results → B → packet-valid → C → lessons-found → signable`.
- All of F-015 ships in P2. In P1, Billy runs the stage's close-out by hand, guided by a `manual` checklist step listing the sequence for that stage type.

**Done when.** A gate can't reach `signable` with any applicable item unresolved (property test).

**Beats.** By hand, gate items are ticked from memory and nothing ties a PASS to evidence.

### F-016 · Lessons capture

**Behavior.**
- Chat stages: the F.2 step's prompt gets the lessons-entry format from `docs/lessons.md`'s header appended automatically. Its paste-back appends entries to `docs/lessons.md` with the next free `L-NNN`, after showing the diff.
- Stage 2: F.2 runs as a followup in the 2.2 chain (`chain=@stage`) and Claude writes the entries to `docs/lessons.md`; Foreman detects them.
- Stages 5–17: Gate C writes the entry in its session; Foreman detects a new entry for the stage.
- A stage's lessons requirement is met by an entry headed `## L-NNN · Stage N · …` for that stage, including a "clean stage" entry in that form.

**Done when.** Append tests: numbering, concurrent external edits (refuse and re-read), header preserved.

**Beats.** By hand, lessons are skipped when a stage feels done.

---

## 5. The daily surface

### F-017 · Today and Do next

**Behavior.**
- **P1:** each project row shows its **Next step** (the first runnable step on the applicable path) with its spec line and primary action. **⌘⇧↵** runs the top project's next step from any screen. Opening the app focuses that row, so starting the next step is two keystrokes.
- **P2: Today inbox** across projects. Item kinds in priority order: `session-blocked`, `session-your-turn`, `packet-landed` / `packet-invalid`, `gate-failed` / `gate-signable`, `outside-change` (F-022), `tamper` (F-014), `chat-awaiting-paste` (copied > 10 min ago), `wait-elapsed`, `interrupted-session`, `step-ready`. J/K to move, Enter for the primary action. Dock badge for the first four kinds; macOS notifications for `session-blocked` and `packet-landed` only when Foreman isn't focused. Sidebar dots per project. ⌘1–⌘9 for pinned projects.

**Done when.** Time-to-start is logged for every run (F-026); p95 ≤ 10 s, excluding time typing run inputs (NFR-09).

**Beats.** By hand, "what's next and in which project?" means opening each repo's latest packet.

### F-018 · Projects and the wizard

**Behavior.**
- **P1 setup** (one form): name; folder (new or existing; `git init` if not a repo); variables (global defaults from Settings prefill `[MY_ADVANTAGES]`, `[PREFERRED_STACK]`, `[QUALITY_BAR]`; placeholder values count as empty); the eight early conditions (tri-state); the claude.ai Project URL; pin the blueprint. **Review** lists every file Foreman will create, leave alone, or refuse to overwrite (an existing `docs/lessons.md` is never overwritten). Creates `.foreman/`, `docs/handoff/`, `docs/lessons.md` (template header), `docs/templates/` (handoff packet, change request and postmortem templates, copied from the pinned kit), `docs/change-requests/`, `docs/postmortems/`, `spec/`.
- **P2 wizard**: the same, as four steps (name and folder → variables → conditions, each with one line on what it changes → review the tailored path: which stages apply, which don't, and why). **Create** lands on the first action with focus.
- Council agents are written by the Stage 2 `council-setup` action, not by setup, so conditions decided by then can drop commercial roles.
- **Adopt existing project** is v1.1 (the `adopted` state is reserved).

**Edge cases.** Folder inside another project; folder is the blueprint repo; folder not writable; existing `.foreman/` from a newer schema (open read-only).

**Done when.** Setup tests on empty folder, existing repo, existing docs; review list matches the actual writes exactly.

**Beats.** `new-repo-docs.sh` creates the files but doesn't tailor the path or pin the blueprint.

### F-019 · First run, Doctor, contract probes

**Behavior.**
- **First run** (gates project creation): blueprint repo (auto-detect `~/claude-blueprint`, `~/Developer/claude-blueprint`, else Choose…) → Template health → DRI name (prefilled from `git config user.name`) → Doctor → Start your first project.
- **Doctor checks** (P1: version, auth, git, and the probe-folder trust step; P2: all):
  - `claude` found at an allowlisted path; version vs the computed floor, with a **capability table** (feature → minimum → found → steps it blocks);
  - `claude auth status` `authMethod` (`claude.ai` or `oauth_token`);
  - effective `cleanupPeriodDays`, with a recommendation to raise it if chains may sit longer than that (Foreman doesn't write it);
  - `disableAllHooks` not set in user settings;
  - Appendix A artifacts (global agents, settings keys);
  - broad allow rules in the user settings that matter for v1.1 headless gates (`Bash(*)`, `pnpm exec *`, `npx *`, interpreters), reported as warnings;
  - toolchain probes (`git`, `node`, `pnpm`, `cargo`) from fixed candidate paths, `--version` only;
  - **Capture my login shell PATH** (one-time, consented: runs `$SHELL -l -c 'printf %s "$PATH"'`, shows the result, stores it). This is the only shell Foreman ever spawns, and only on this explicit click.
- **Contract probes** (R-12, P2), run when `claude -v` changes, cost no tokens. They run in a fixed `<app-data>/probe/` folder that Billy trusts once in first run (Foreman opens `claude` there in a terminal tab so he can accept the trust dialog). Each has a 20 s timeout and a classified outcome: pass, flag failure (an unknown-option style error), or inconclusive (timeout, trust prompt, anything else; keeps the last verified state). Probes: `claude --init-only --settings <probe settings> --session-id <fresh uuid> --model <m> --effort <e> --permission-mode <mode> --name <probe>` with a `SessionStart` hook that writes a marker. `--init-only` runs Setup and `SessionStart` hooks and exits without a conversation ([cli-reference](https://code.claude.com/docs/en/cli-reference)), so one call proves the per-launch hooks fire **and** that every interactive flag Foreman passes still parses (an unknown option fails the call). A second probe, `claude --init-only --resume <random uuid>`, passes when the error is "no conversation found" (the flag parsed). Probes never run in a project. `--desktop` isn't probed (it opens the app); it's covered by the version floor. The result is stored as "contract verified on X.Y.Z". A failure disables the dependent feature and names it.
- Doctor runs at launch (async) and after any launch failure; a footer dot shows status.

**Done when.** Tests with fake binaries for each probe outcome and each auth method.

**Beats.** `claude doctor` checks Claude Code's installation; it doesn't know which flags and versions a blueprint run needs.

### F-020 · State, persistence, resilience

**Behavior.**
- **Repo:** `.foreman/project.json` (variables, conditions + audit, model policy, spec folder, verify commands, schemaVersion), `.foreman/overlay.json` (skips with reasons, project-only additions), `.foreman/blueprint.lock.json`, `.foreman/stages/stage-NN.json`. Before Stage 5 is signed, decision changes are committed by Foreman with a pathspec at the moment of change. From Stage 5 on, they stay **pending in app data** and are written into the repo only inside the next signing commit (R-10).
- **App data:** `settings.json`, `projects.json`, `projects/<id>/progress.json` (step states, chains, gate runs, run inputs, pending decision changes), `approvals/<projectId>.json` (F-022), `runs/<runId>/{run.json, output.log, settings.json}`, `runs/index-YYYY-MM.jsonl`, `events/<runId>/`, `probe/`, `templates/<hash>.md`, `bin/`.
- **Atomic writes:** temp file `.<name>.foreman-tmp` in the same directory (pattern added to `.git/info/exclude`), `sync_all`, rename, fsync the directory; `tempfile::persist`. A file changed on disk since Foreman read it is never overwritten: the write is refused and the conflict shown.
- **Run record at launch** (status `running`), updated at exit. **Interrupted sessions** on next start, with Resume (re-arm).
- **Invalid files:** a decision file that fails to parse opens the project read-only with the error and **Open in editor**.
- All JSON is sorted by key and ID so diffs stay small; every file has `schemaVersion` and a migration.

**Done when.** Crash-injection proptest at every write point (NFR-10); kill-with-live-child test (NFR-11).

**Beats.** By hand, nothing survives a closed terminal except the transcript.

### F-021 · Path policy

**Behavior.** One Rust function guards every write Foreman makes, whatever the source:
- repo-relative only; no `..`, no absolute paths, no NUL, no Unicode bidi or control characters;
- deny by **path component at any depth**: `.git`, `.claude` (except the hard-coded `council-setup` destinations under `spec/.claude/agents/`), `.husky`, `.vscode`, `.idea`, `.github`, `.foreman` (except through the decision-file writers); deny file names `.mcp.json`, `CLAUDE.local.md`, `.envrc`, `.gitmodules`, `.gitattributes`, `.npmrc`;
- `CLAUDE.md` is writable only at the repo root, only through the build-package assembler, with a confirmation that says "Claude Code loads this file in every session";
- component-wise resolution through a directory-handle API with symlink refusal (crate chosen in the Stage 4.2 ADR, version verified then);
- the final path must be inside the project root or app data.

**Done when.** Golden tests for `..`, absolute, `.git/hooks/pre-commit`, `spec/.claude/settings.json`, `spec/.mcp.json`, `spec/CLAUDE.md`, a symlinked `docs/handoff`, Unicode lookalikes, and a REPO FILES list containing each.

**Beats.** By hand, nothing; this protects Foreman's own writes from repo content.

### F-022 · Outside-change review

**Behavior.**
- App data stores the SHA-256 of each decision file as Billy last approved it, plus the approved verify-command set.
- On a change from outside Foreman (Claude, git checkout, an editor): decision fields (conditions, skips, additions, verify commands, model policy, lock) go to a **Pending outside change** review with a diff and **Accept** / **Reject** (Reject restores the approved content through an atomic write). Variables reload with a toast.
- The lock is verified against app data's template hash and, when available, the blueprint repo at the locked commit.
- Every launch's settings deny `Read(./.foreman/**)` to Claude (defense in depth; not a boundary against Bash).

**Done when.** A test edits each decision file from outside; Foreman's effective config is unchanged until Accept.

**Beats.** Nothing else checks that the files deciding the process weren't changed by the agent being governed.

### F-023 · Knowledge manifest and build-package assembler (P2)

**Behavior.**
- **Knowledge manifest:** the files that should be in the claude.ai Project now (signed packets, declared chat outputs), each with a checkbox, Reveal in Finder, Copy contents. Stage 4 steps show "Knowledge ready 4 of 6" by the Copy button.
- **Assembler** (`manual` step at the end of Stage 4): parses `REPO FILES` from the 4.8 packet; each path shows exists / missing / empty, with **Paste into** (clipboard to file through the path policy, with an overwrite diff) and **Reveal**. The Stage 4 attest item "the package is complete" is offered only when every row exists. The same panel asks the late conditions, prefilled from evidence (`docs/migrations/` present → `has_database = yes`).

**Done when.** Tests with a 4.8 packet fixture; a malicious REPO FILES entry is refused by the path policy.

**Beats.** By hand, "create the repo with exactly those files" is a long manual job with no checklist.

### F-024 · Desktop handoff (P2)

**Behavior.** For a step with `surface=desktop`: **Open in Desktop** copies the prompt and runs `claude --desktop` in the step's cwd (v2.1.285+; no other flags); the step shows chips for the model, effort and mode to pick in Desktop. For a CLI chain: **Open this session in Desktop** runs `claude --desktop --resume <uuid>` and marks the chain `handed-off` (F-010).

**Done when.** Fake-binary argv tests; S-23 result applied.

**Beats.** By hand, Desktop sessions start with whatever model was last used.

### F-025 · Add to a stage, this project only (P2)

**Behavior.** The **+** insert point between steps offers **Add prompt** and **Add inspection item**. An added prompt copies the run config of a step Billy picks, then lets him edit text and config; it's stored in `overlay.json` with a generated ID (`x-<stage>-<n>`), tag `optional` by default (Billy may make it `core` for this project). Added items are `attest`. Every added step shows an "added for this project" badge and always confirms before running (SR-07). Promote to blueprint, Add agent and Add note are v1.1.

**Done when.** Overlay round-trip tests; added steps appear in the applicable path and packets list them.

**Beats.** By hand, extra steps live in notes and get forgotten.

### F-026 · Run log and metric panel

**Behavior.**
- **Measures** (Master Spec §9, the same definitions as the by-hand baseline): time to start a step (from focusing Foreman or pressing Do next to the prompt being sent: spawn for CLI, Copy and open for chat) and active time per stage (first step started to packet signed, excluding waits).
- **P1:** every run's record includes time-to-start, the entry point (Do next, palette, stage view), the number of run-input interventions, and any **Copy prompt** on a CLI step (a proxy for running by hand). Stored in `runs/<runId>/run.json` with a monthly index `runs/index-YYYY-MM.jsonl`. **Export metrics** writes a Markdown summary of both measures per surface and per stage, in the same table layout as `docs/metrics-baseline.md`.
- **P2:** a searchable **Run log** screen (argv, session name, outcome, Resume where the transcript exists), and Settings → Metrics showing median and p95 time-to-start, manual-edit count per stage, stages run through Foreman, and the baseline comparison from `docs/metrics-baseline.md`.

**Done when.** At the end of P1, Export metrics produces the kill-check table; in P2 the panel shows it live.

**Beats.** By hand, the kill criterion would be a feeling.

### F-027 · Run sheet export

**Behavior.** Any stage exports as one Markdown file: for each applicable step, the resolved prompt, the exact argv (or chat chips), run inputs as filled, and the gate items with their kinds. Saved through a save dialog.

**Done when.** Golden test of the Stage 2 sheet.

**Beats.** It's the escape hatch: the process never depends on Foreman running (NFR-14).

### F-028 · Command palette and keyboard

**Behavior.** ⌘K palette (Bits UI Command) over projects, stages, steps, and every action. Shortcuts per design direction §7, plus: ⌘⇧↵ Do next, ⌘⇧V paste-back, ⌘D mark done, ⌘⇧T focus mode (P2), ⌘⇧E return from terminal. P1 ships ⌘↵, ⌘⇧↵, ⌘⇧V, ⌘D, ⌃`, ⌘⇧E; the palette (⌘K) and the rest in P2.

**Done when.** Keyboard-only test script covers the full P1 loop.

### F-029 · Settings

**Behavior.** Blueprint repo path; DRI name; global variable defaults; theme (system / Vellum / Night); terminal font size; scrollback cap; paths for `claude` and `git` (auto-detected, overridable, must be absolute); environment pass-through list; captured login PATH (view, re-capture, clear); Clear run history. P1 ships the `claude` and `git` paths and the DRI name; the rest is P2.

**Done when.** Every setting persists across restarts through an atomic write; invalid paths (relative, missing, not executable) are refused with the reason; the pass-through list can never re-add a variable on the always-removed list (`env::denylist_beats_passthrough`).

### F-030 · Sheet strip and stage workspace

**Behavior.** Per design direction §4–6: the sheet strip (19 sheets, phase edges, status marks: locked, ready, in progress, inspecting, signed, not applicable, needs annotation); the stage workspace (steps as callouts with step number, spec line, resolved prompt, attention pills, primary action matching the surface); the title block; the contextual inspector (packet, gate, lessons); the terminal drawer. Not-applicable rows are hatched with the reason. Core items show the lock.

**P1:** the stage workspace (steps, spec lines, resolved prompts, run-input forms, primary actions, not-applicable and locked rows) and the terminal drawer. **P2:** the sheet strip, title block, inspector and stamp motion.

**Done when.** Selecting a step shows exactly the resolver's output for it; a not-applicable or locked row can't be run and states why; a core item never shows Skip; axe-core finds zero violations; a keyboard-only walkthrough completes the P1 loop; both themes pass the computed contrast table (NFR-31–35).

---

## 6. Rejected options (with reasons)

| Option | Why rejected |
|---|---|
| A YAML/JSON template source with generated Markdown | The blueprint is edited as prose in retros; a generator adds a second place for mistakes (r2 P-01). |
| Inference as a source of values | Deadlocks when inference is wrong; couples retros to Foreman releases (R-05). |
| Positional inspection-item IDs | Reordering silently swaps verification kinds (R-06). |
| Headless LLM Gate A in v1 | Little v1 use, the largest attack surface, and an announced breaking change (R-01, R-12, R-14). v1.1 with the hardened argv. |
| A user-level hook in `~/.claude/settings.json` | Runs in every Claude session Billy starts; contradicts "never touched" (R-16). |
| Typing `/model`, `/effort`, Shift+Tab into the live TUI | Fragile; fails silently (r2 P-05). |
| `claude --bg` + attach as the session substrate | Research preview; worktree default; undocumented flag support (r2 P-05). |
| The Agent SDK or the API | The brief: Max plan only, never an API key. |
| Browser automation of claude.ai | Brittle, outbound integration, against SR-08 (r2 P-11). |
| `tauri-plugin-pty`, `tauri-plugin-shell`, `tauri-plugin-fs` | The webview could choose programs or paths (r1). |
| Plain Svelte 5 + Vite with an in-house router | Smaller bundle and no inline script (r7), but Billy chose SvelteKit 3 for one stack and file routing across his projects. |
| Electron; SwiftUI + SwiftTerm | r7 PART 1. SwiftUI + SwiftTerm stays the fallback if S-0 fails fundamentally. |
| A separate dashboard in v1 | Today covers the job; the metrics view is F-108 (v1.1). |

## 7. Decisions made in place of questions

Billy may want to revisit these at the Stage 2 sign; each has a default that ships if he doesn't.

1. **Frontend:** SvelteKit 3 SPA (Billy's decision).
2. **Repo hooks never run on Foreman's git calls**, signing included (r5b minor 11).
3. **Signing on the current branch** is the default (r5b M11); another branch is an option.
4. **Kill check** after TradeLog Stages 1–2 at the end of P1 (about 8–14 weeks); gates and signing by hand until P2.
5. **Headless Gate A** in v1.1, not v1.
6. **Run sheet export** in v1 (P2, pulled forward from P-25) because it's the escape hatch NFR-14 requires.
7. **Foreman's own build conditions:** `idea_known` yes, `has_ui` yes, all others no (v0.9 §5.3).
