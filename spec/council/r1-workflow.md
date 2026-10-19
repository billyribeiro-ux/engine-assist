# Round 1 · User-advocate · Daily workflow walkthrough

Author: user-advocate (Stage 2 council) · Date: 2026-10-19
Subject: Foreman as written in `01-master-spec.md` v0.9 and `02-design-direction.md` v0.9, walked as Billy against `claude-build-blueprint.md` v2.1.
Scope rule: v1 stays tight. Every fix is tagged **v1** (essential, cheap or protects the "impossible to do wrong" promise) or **v1.x** (can wait).

---

## 0. How to read this

- Severity: **blocker** = the journey cannot be done as specified, or a gate/process rule can be violated silently. **major** = Billy loses minutes per use, or can make a costly mistake. **minor** = polish, small friction.
- IDs are `J<journey>-<n>`. Sections 12-17 roll everything up.
- Evidence base. I read the three spec inputs, then mined the blueprint for what the prompts actually contain (counts below), and checked every Claude Code claim I rely on against the official CLI reference, headless, hooks and agent-view docs (links in section 16). Where a claim could not be verified, it is listed as a **spike** (section 13), not asserted.

### What the blueprint actually looks like (measured, not assumed)

| Fact | Count / evidence | Why it matters |
|---|---|---|
| `RUN WITH` lines | 64 | The parser's whole input |
| Continue a previous conversation ("same session" 16, "same chat" 5) | 21 of 64 (33%) | One step in three is not a fresh launch. The spec only models fresh launches |
| Lines with a mode/model sequence (`→`, `then`, `switch to`, `/model`, `/effort`, Shift+Tab) | 12 arrows, many more in prose | The run config is a *sequence*, not a single triple |
| Lines naming Fable, mostly with an Opus alternative ("or", "if available") | 6 | Alternatives exist inside one RUN WITH; the spec models one value |
| Packets whose format literally says `APPROVED BY: <blank>` | 5 | Spec §8.4 says "blank"; the literal text `<blank>` will fail validation on first paste |
| Bracket tokens that are **not** Project Variables | `[N]`, `[ID]`, `[X]`, `[changes]`, `[paste numbers]`, `[list screens]`, `[list]`, `[jurisdictions]`, `[describe the voice]`, `[M#]`, `[NNN]`, `[what I want to change and why]`, `[your numbers]` | F-007 knows 7 variables and packet markers. Thirteen more kinds of token appear in prompts |
| Hard-coded `docs/handoff/stage-NN.md` reads | 14 prompts, e.g. 11.1 reads `stage-10.md` | If Stage 10 is not applicable (no payments), the file does not exist and the prompt points at nothing |
| Chat steps that output documents, not packets | 2.1, 4.1, 4.2 (10-15 ADRs), 4.3, 4.4 (schema + OpenAPI), 4.5, 4.6, 4.7, 15.1 | F-010 only handles packets |

---

## 1. Journey 1 · First launch and first-run setup

**Billy's steps**
1. Opens Foreman. Sees empty Projects screen: "Start your first project".
2. Realizes he needs the blueprint repo path, so goes to Settings (⌘,), picks the folder.
3. Goes to Doctor; reads the failures; fixes them in a terminal (`claude update`, `./install.sh`).
4. Types his name as DRI. Comes back; ⌘N.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J1-1 | major | No first-run sequence. Settings, Doctor and the wizard are three disconnected screens. The wizard can start with no blueprint and no `claude`, then fail late. | A first-run checklist that gates the wizard: (1) blueprint repo (auto-detect `~/claude-blueprint`, `~/Developer/claude-blueprint`, else Choose...), (2) parse it and show proof: "Blueprint 2.1 parsed: 19 stages, 64 run configs, 0 errors" (this is also the riskiest-assumption test made visible), (3) DRI name prefilled from `git config user.name`, (4) Doctor auto-runs, (5) "Start your first project". Fatal Doctor failures block; warnings do not. | v1 |
| J1-2 | major | Doctor checks model minimums (§6.3) but not flag minimums that Foreman itself needs. From the CLI reference: `--desktop` needs v2.1.285+, `--permission-prompts` (headless) v2.1.259+, `--name`/`--effort` exist, `--permission-mode manual` alias v2.1.200+. F-009 and §6.2 depend on these and the version table omits them. | Doctor shows a **capability table**: feature -> minimum version -> found -> which steps it blocks ("Desktop handoff: needs 2.1.285, found 2.1.280: blocks 14 steps"). Derive "blocks N steps" from the parsed template. | v1 |
| J1-3 | major | **API-key billing trap.** Brief: Max plan only, "never needs an API key". `claude auth status` JSON has `authMethod` (`claude.ai`, `oauth_token`, `api_key`, ...). Doctor only checks exit code 0 ("logged in"), which is also 0 for `api_key`. If Billy has `ANTHROPIC_API_KEY` in his shell for another product, a Foreman-launched session can bill per token. | Doctor fails (not warns) unless `authMethod == claude.ai`. Foreman builds the child environment from an **explicit allowlist** and strips `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN` and provider switches (Bedrock/Vertex/Foundry vars) from every spawn. Add as SR-10. Spike S-7 to confirm exact behavior. | v1 |
| J1-4 | major | Appendix A one-time setup (`install.sh`, global agents, `~/.claude/settings.json`, council agents) is only partly checked ("installed global agents"). Billy hits it at Stage 2 when the council cannot find its agents. | Doctor lists every Appendix A artifact and the per-project council folder. Each failure has **Copy fix command** (exact text, e.g. `cd ~/claude-blueprint && ./install.sh`). Foreman does not run shell scripts (SR-01); it copies, then Re-check. | v1 |
| J1-5 | minor | Spec inconsistency: Doctor must run `pnpm -v`, `node -v`, `cargo -V`, but SR-01 allows spawning only `claude` and `git`. A Tauri app launched from Finder also has a minimal PATH, so "pnpm not found" while it works in Terminal. | Decide explicitly: Doctor probes the three toolchain binaries from a fixed candidate list (`/opt/homebrew/bin`, `~/.cargo/bin`, `~/.local/share/pnpm`, `~/.nvm/...`) with a Settings override, and SR-01 is amended to "allowlist = resolved absolute paths of claude, git, and the Doctor probes (version flags only)". | v1 |
| J1-6 | minor | Doctor failures use the right voice (§9 writing) but nothing says how often it re-runs. | Run at launch (async, never blocks UI), cache 24 h, re-run on any launch failure, and add a status dot in the sidebar footer. | v1 |
| J1-7 | minor | "Claude Code 2.1.270 found. Opus 5.5 needs 2.1.280" is the right message, but the fix is a copy-paste. | **Run `claude update`** button opening a terminal tab running exactly that (it is the allowlisted `claude` binary). | v1 |

---

## 2. Journey 2 · New project (TradeLog), wizard, condition questions, tailored path

**Billy's steps**
1. ⌘N. Step 1: name "TradeLog", folder (new). Step 2: seven variables. Step 3: eight condition questions. Step 4: review tailored path. Create.
2. Lands on the Project screen; Stage 1.1 should be ready.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J2-1 | major | Seven variables typed from scratch for every project. `[MY_ADVANTAGES]`, `[PREFERRED_STACK]`, `[QUALITY_BAR]` are the same across Billy's six products. `[BUDGET_TIME]` ships in the blueprint as literal `[your numbers]`; if left, it passes as "filled". | **Global variable defaults** in Settings, prefilled in every wizard; only `[ROUGH_IDEA]`, `[TARGET_USERS]`, `[BUDGET_TIME]` usually need typing. Any value equal to a blueprint placeholder (`[your numbers]`) counts as unresolved. Multi-line fields for the long ones. | v1 |
| J2-2 | major | Conditions are asked on day 0 about things Stages 0-3 exist to decide (`payments`, `commercial`, `public`, `multi_user`), then "changeable only by a CR" (§5.2). But CRs only exist after the Stage 2 freeze. A wrong early guess costs ceremony; an honest "don't know yet" is impossible. | Three states: yes / no / **not decided yet**. "Not decided" keeps dependent stages visible as "decide before Stage N" and puts a **Confirm conditions** checkpoint in the Stage 2 sign dialog. Conditions are freely editable (with an audit line in `project.json`) **until the Stage 2 packet is signed**; after that, CR only. | v1 |
| J2-3 | major | Hidden conditions. §5.3 uses "the design doc declares a database", "uploads/email parts follow the design doc", but the wizard asks only 8 questions. Nobody ever answers the database question, so Stage 6 migrations have no truth value. | **Late conditions**: `has_database`, `has_uploads`, `has_email`, `has_integrations`. Asked at the Stage 4 sign dialog, prefilled from the 4.8 packet's REPO FILES list (e.g. `docs/migrations/` present). Same mechanism as J2-2. | v1 |
| J2-4 | major | The wizard creates `.foreman/`, `docs/handoff/`, `docs/lessons.md` (F-001) but not the Stage 2 council workspace (`[app]-spec/` or `spec/` with six agents) and nothing about the claude.ai Project. Billy will discover both at Stage 1.1 and Stage 2.2. | On Create, run a **setup actions** list, each shown and checkable: create `spec/` with the council agents copied from the blueprint kit (Rust file copy, no shell), create the docs skeleton, and a **Manual setup** row "Create the claude.ai Project named TradeLog" with a field to store its URL. Foreman then opens that URL directly from every chat step. | v1 |
| J2-5 | major | Pinning is underspecified. F-001 "blueprint version to pin" but §7 stores no template snapshot. If the blueprint repo is edited later (a retro, a promoted lesson), every open project silently changes meaning. Also two sources of truth: `project.json` and the `BLUEPRINT VERSION` field in the Stage 4 packet. | Pin = blueprint **git commit SHA + version string**, and store the parsed template as `.foreman/template.lock.json` (or read via `git show <sha>:claude-build-blueprint.md`). The project always opens against its pin. A dirty blueprint working tree cannot be pinned (wizard says why). | v1 |
| J2-6 | minor | For TradeLog every condition is yes, so the "tailored path" is all 19 stages. The advertised wow moment ("answer five questions, see the tailored path") is flat for Billy's actual products (all are servers with users and payments). | Keep the path review, but make the real wow the last wizard screen: **First action: Stage 1.1 · Claude chat · Opus 5.5 [Copy and open chat ⌘↵]**. After Create, focus lands on that primary button. | v1 |
| J2-7 | minor | Review step does not preview side effects. Existing folder with `docs/lessons.md` already there? | Show "Will create / will leave alone / will refuse to overwrite" list before Create. Never overwrite an existing `docs/lessons.md`. | v1 |
| J2-8 | major | Billy runs six products; several are already mid-flight (swings, options, charting). The wizard only starts at Stage 0/1. | **Adopt existing project**: choose "already through Stage N", mark stages <= N as `adopted` (reason, no packet), continue from N+1. Design the status enum now so it does not need a migration. | status enum v1, UI v1.x |
| J2-9 | minor | Implied-condition conflicts are not caught (payments yes with commercial no; payments with no server; multi_user with no auth). | A small consistency table with inline warnings ("Payments usually means commercial and others_data. Set them?"). | v1.x |

---

## 3. Journey 3 · Chat step (Stage 1.1): copy, Claude chat, iterate, paste the packet back

**Billy's steps**
1. Stage 1 -> step 1.1 shows spec line "Chat · Opus 5.5 · adaptive · web search ON" and the resolved prompt.
2. "Copy for chat" -> clipboard, browser opens claude.ai.
3. In claude.ai he creates the Project (first time), picks Opus 5.5, turns web search on, pastes, answers question rounds for an hour.
4. Back to Foreman for 1.2 ("same chat", needs `[X]` and `[changes]`), 1.3 (kit; then a 7-day real-world test), 1.4 (`[paste numbers]`, "approved").
5. Copies the packet code block, returns, pastes into the paste-back box; Foreman validates, saves `docs/handoff/stage-01.md`; he signs.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J3-1 | major | Step completion is undefined for chat steps. 1.1, 1.2, 1.3 produce no file. The stage map has stage statuses only. Foreman cannot tell "never started" from "chat in flight" from "done". | Step states: `not started / copied (awaiting you) / done / skipped / n/a / waiting-until <date>`. "Copied 14:02, waiting on you" shows in the Projects list. A **Mark done** action on chat steps. 1.3's 7-day test becomes `waiting-until` with a macOS reminder. | v1 |
| J3-2 | major | **Unresolved-token problem is much bigger than the 7 variables.** Prompts contain run-time inputs (`[X]`, `[changes]`, `[paste numbers]`, `[ID]`, `[list screens]`, `[jurisdictions]`, `[M#]`, `[describe the voice]`) and stray brackets that are not placeholders. F-007's "never send with unresolved required variables, offer to fill in place" has nothing to fill from for `[paste numbers]` (results arrive in 7 days). | The parser classifies every token as **project variable**, **packet marker**, **run input** (typed when the step runs, with label, type, multiline, and *remembered per project* so `[jurisdictions]` and `[list screens]` are asked once), or **literal** (not a placeholder). Run inputs are declared by a template annotation (section 12) and inferred by golden tests. Required run inputs appear as a small inline form on the step, not a modal. | v1 |
| J3-3 | major | Billy must remember the chat settings (model picker, web search ON, Research ON) and whether it is a **new Project / new conversation / same chat**. 4.x says "new conversation in the Project", 4.7 "brand-new conversation (fresh context)". Opening the wrong one pollutes context silently. | In the step header, a one-line **chat setup strip** with checkable chips: `New conversation in Project TradeLog` or `Same chat as 1.1`, `Opus 5.5`, `Web search ON`, `Research ON`. Bold the new/same distinction. Chips are memory aids only (Foreman cannot verify claude.ai). Do not rely on `claude.ai/new?q=` prefill: I found no official documentation for it, so the clipboard stays the contract. | v1 |
| J3-4 | major | Stage 4 chats depend on **Project knowledge** (scope.md, design doc, ADRs, threat model...). Billy must upload each output to the claude.ai Project; Foreman cannot see whether he did. Skipping one makes 4.3/4.6/4.7 hallucinate. After fixes in 4.7, files change and the uploaded copy is stale. | A **Project knowledge manifest** per project: the files that should be in the claude.ai Project right now (all signed packets + stage outputs), each with a checkbox, **Reveal in Finder** and **Copy contents**. Chat steps in Stage 4 show "Knowledge ready 4 of 6" before Copy. v1.x: record a content hash when ticked and flag "changed since uploaded, re-upload". | checklist v1, hash v1.x |
| J3-5 | major | Non-packet chat outputs (list in section 0) have no landing path. 4.2 emits 10-15 ADRs in one reply that must become `docs/adr/ADR-NNN.md`; 4.4 emits SQL migrations plus an OpenAPI file. Splitting by hand is slow and error-prone. | **Save output** paste-back: choose the target path (prefilled from the step's declared output), diff or overwrite-protect if it exists. v1.x: **Split by heading** for ADRs (`# ADR-NNN:` -> separate files) and a blueprint tweak so multi-file replies start each file with `FILE: <path>` which Foreman splits automatically. | single-file v1, split v1.x |
| J3-6 | major | The pasted text is "packet + press release/FAQ below it" for 1.4, and the blueprint also wants a copy at `spec/01-app-brief.md` (2.2 reads that name). F-010 handles one blob and one destination. | Parse the packet from the header line to the template's declared last field; the remainder is an **attachment** saved beside it (`docs/handoff/stage-01-attachments.md`). Declared outputs may list several destinations; Foreman writes `docs/handoff/stage-01.md` and the derived `spec/01-app-brief.md` and shows both. | v1 |
| J3-7 | major | `APPROVED BY: <blank>` is the literal text in 5 packet formats; Foreman's own brief uses `<sign here: ...>`. §8.4 says "blank". Every chat packet would be rejected on first paste. | Treat empty, whitespace, `<blank>`, `<leave blank...>`, `<sign here...>` as **unsigned**. Normalize case/trailing punctuation for `OPEN ITEMS: none` (`None`, `none.`) with a quiet note. Normalize smart quotes and NBSP. Add all five real formats to the golden tests. | v1 |
| J3-8 | major | An invalid pasted packet is a dead end (§8 "can't be signed"), but Billy is mid-chat and the fix belongs in that chat. | **Copy fix request**: generates a ready-to-paste message: "Your packet is missing: X, Y. OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block." | v1 |
| J3-9 | major | Chat stages 0-4 have no Gate A/B/C, but have an INSPECTION list (human) and the lessons prompt F.2 ("I'll paste the format"). Spec F-012 describes the gate only for Claude Code. | Chat-stage gate = **checklist of attestations** (no headless) + the F.2 lessons chat step with the lessons-entry format **auto-appended** to the copied prompt + a paste-back that appends entries to `docs/lessons.md` with auto-numbered `L-NNN`. | v1 |
| J3-10 | minor | Prompts are copied once; if the packet changes later (a re-run), nobody knows what text was actually sent. | Log a hash and the resolved prompt for every copy/launch in the run log (F-019). "Copy again" is always one key (⌘⇧C). | v1 |

---

## 4. Journey 4 · Stage 2 council in the embedded terminal, "go" between rounds

**Billy's steps**
1. Stage 2: 2.1 teardown (chat, conditional on `commercial`), then **Council setup (once)** (no prompt, a folder of six agents).
2. 2.2 "Convene the council": Chair Fable 5.1 high on Max, or Opus 5.5 xhigh; Plan mode first, then Accept edits; cwd `[app]-spec/`.
3. Presses ⌘↵. Terminal drawer opens, claude starts with the prompt. It prints the plan; he approves, chooses Accept edits.
4. Five rounds. After each: a 5-line summary; he types `go`.
5. 2.3-2.6 are all "same session": feature specs (ultrathink), stack decision (then he edits `r7-stack.md` himself to record his choice), NFRs, freeze in Manual mode (Shift+Tab), packet in a code block.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J4-1 | **blocker** | **"Same session" is not modeled.** 21 of 64 steps continue a prior conversation (2.3-2.6, 5.2-5.5, 6.2, 16.2, 18.2, Gate B and C, Crew Loop steps 2-4). F-008 says "one terminal tab per run" and launches `claude "prompt"`. Pressing Run on 2.3 starts a new, empty conversation, losing the chair's context. | Add `session` to the run config: `new | continue | fresh`. Steps form **session chains**; Foreman generates `--session-id <uuid>` at chain start and stores it (names are display-only and can collide on re-runs). Two ways to continue: (a) tab alive: **Send to session**, pasting the resolved prompt into the PTY as a bracketed paste (not auto-submitted; he presses Enter) and showing the switch cues; (b) tab gone: relaunch `claude --resume <session-id> --model X --effort Y --permission-mode Z "<prompt>"`. Flags on resume must be confirmed by spike S-3. | v1 |
| J4-2 | **blocker** | **Wrong working directory.** §6.1 spawns in the project folder. Stage 2 runs in `[app]-spec/` (agents live in `spec/.claude/agents/`, outputs go to `spec/council/`, the packet to `../docs/handoff/`). The retro (F.5) runs in the **blueprint repo**. Launch from the project root and the council agents are not found. | `cwd` in the run config: `project | project/spec | blueprint`, parsed from RUN WITH ("in `[app]-spec/`", "in your `claude-blueprint/` repo"). SR-04 already allows the paths. The watcher scope includes `spec/council/`. | v1 |
| J4-3 | major | A RUN WITH holds a **sequence**: "Plan first, then Accept edits", "Plan -> Manual", "Shift+Tab -> Plan", "/model opus, /effort high", "(HARD class: Manual)". The spec's run config is one model/effort/mode. The dangerous mistake (a money ticket left in Accept edits) is exactly a missed switch. | Parse `start` (flags for the launch) and ordered `switches` (what to change, and when: "after plan approval", "before step"). Show a **Now / Next strip** above the terminal: `Now: Plan · Next: after you approve the plan, press Shift+Tab to Manual`. | cue strip v1 |
| J4-4 | major | Live mode/effort is invisible. Hook payloads carry `permission_mode` and `effort.level` (hooks docs), so Foreman could show the *actual* state and warn on mismatch ("HARD ticket expects Manual, session is in Accept edits"). That is the "impossible to do wrong" feature. | **Session telemetry channel**: Foreman launches with an env var `FOREMAN_RUN_ID`; a tiny Foreman-owned hook (installed by Doctor into user settings, no-op when the var is absent) appends events (`Notification`, `Stop`, `PermissionRequest`, `SessionEnd`, plus `permission_mode`, `effort`) to the app-data `events/` dir. Hook docs do not establish per-session hook injection via `--settings`, hence user-level install + env gate. Spike S-1. | v1 if S-1 passes, else v1.x |
| J4-5 | major | No progress view for a 5-round, parallel-agent, 30-minute run. Billy cannot tell which round is running or whether it is waiting for his `go`. | Outputs are parseable in the council prompt (`-> council/r1-market.md`). Show a **round tracker** from the watcher: `Round 1 · 2 of 2 files · Round 2 · 1 of 3`. When the `Stop`/`Notification` event arrives (J4-4), badge the tab "Waiting for your go" and offer a **Send "go"** button. | tracker v1, "go" button with J4-4 |
| J4-6 | major | The embedded drawer is too small for what the council prints, and he needs to read `r3-synthesis.md` while the session waits. | Terminal **focus mode** (⌘⇧T: terminal fills the content area with a slim step HUD, same state). Output files named by the step get **Open in editor** (macOS default app); v1.x an in-app Markdown preview in the inspector rendered as escaped text (SR-05). | focus + open v1, preview v1.x |
| J4-7 | major | Manual steps exist in the blueprint with no prompt: "Council setup (once)", "Record your final choice at the bottom of `r7-stack.md`", "Fix every BLOCKER in the original chats", "Create the repo with exactly those files". Foreman has step types only for prompts. | Step kinds beyond prompt: `action` (Foreman does it, e.g. create the council folder), `manual` (checklist item with an optional **Open file** and change-detection: file mtime after step start satisfies it), `wait` (until a date). | v1 |
| J4-8 | major | **Packet emitted in the terminal, not as a file.** 2.6 says "output the MASTER SPEC PACKET in one code block". The watcher only sees `docs/handoff/`. Copying a code block out of xterm with wrapped lines breaks the format. (Foreman's own `03-build-prompts.md` already rewrote this to "write `../docs/handoff/stage-02.md`", which proves it.) | Blueprint change (via the proposals flow): every CLI-run packet is *written to the file*. Fallback **Capture last code block from the session transcript** (`transcript_path` is in every hook payload). | blueprint fix v1, capture v1.x |
| J4-9 | major | Chair model has alternatives ("Fable high on Max, or Opus xhigh on Pro / to save usage"). F-021 substitutes globally per project; there is no per-step choice, and Fable draws on the weekly limit (Appendix B #9). | The run config holds **alternatives** with a default picked by the Settings plan (Max/Pro). The spec line shows `Fable 5.1 · high (Max)  [or Opus 5.5 · xhigh]`; a chip switches it for this run and the choice is logged. | v1 |
| J4-10 | minor | SR-03 says Billy "sees the exact argv and prompt before any run". Read literally this is a confirm modal on every ⌘↵ (100 times a day), against the "under 10 seconds" metric. | Clarify: the visible step block (spec line + resolved prompt + argv under the button) *is* the preview. ⌘↵ runs. A confirm modal appears only for: an overlay-edited prompt/config (SR-07), a first run in a new project, or a mode other than the template's. | v1 |
| J4-11 | minor | Key routing between terminal and app is unspecified. In xterm, ⌘K is "clear" in macOS Terminal, Esc/Shift+Tab/Ctrl+keys belong to Claude Code (mode cycling, interrupt). J/K and ⌘G conflict with typing. | Rule: when the terminal is focused, only ⌘-chords and ⌃` reach the app; everything else goes to the PTY. ⌘K opens the palette from the terminal too. Add an explicit "return focus to workspace" chord. Document it in Settings. | v1 |
| J4-12 | minor | WCAG 2.2 AA claim vs a terminal. Claude Code ships `--ax-screen-reader` (v2.1.181+) for flat, non-animated output. | Settings toggle "Screen-reader friendly Claude output" that adds the flag. | v1.x |

---

## 5. Journey 5 · A packet lands from Claude Code; validate and sign

**Billy's steps**
1. Gate B runs in the session; Claude writes `docs/handoff/stage-06.md` and commits "handoff: stage 6".
2. Foreman detects it (<= 1 s), the sheet shows an attention mark, the inspector opens the Packet view.
3. He reads the fields, sees validation green, clicks Sign, confirms name/date. Foreman writes APPROVED BY, commits. The stamp animates; Stage 7 turns ready.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J5-1 | major | **Sign order contradicts the blueprint.** Blueprint: gate A -> B -> C (lessons) -> "you read it, add your name and date... **then** the next stage starts". Design direction Screen 4: "Packet -> Sign -> Lessons". Signing before lessons lets Stage N+1 start with no lesson captured, breaking "lessons built in". | Fixed order **A -> B -> C -> Sign**. Sign is disabled with the stated reason until a lessons entry exists for the stage (a "clean stage" entry counts). Update the design direction. | v1 |
| J5-2 | major | Double commit and sweep risk. Gate B tells Claude to commit "handoff: stage N"; Foreman then commits again with the same message. A plain `git commit` also sweeps whatever else Billy has staged. | Foreman commits with a **pathspec** (`git commit -m "handoff: stage N signed" -- docs/handoff/stage-NN.md`), never `-a`. Message differs from Claude's. Preferred: change Gate B to *not* commit; the signed commit is the only one. | v1 |
| J5-3 | major | **Time-of-check/time-of-use.** The packet was valid when displayed; Claude may still be editing (session running), or Billy edited it. Sign writes into a file that may have changed. | Sign is bound to a **content hash**: "Signing stage-06.md @ a3f9c1". If the file changed since display, Sign refuses and re-renders. If the stage's session is still running, show "Session still running" and require an extra confirmation. | v1 |
| J5-4 | major | Format-valid is not stage-done. Nothing ties Sign to a passed gate. A packet written early would be signable. | Sign requires gate state `PASS` recorded in `state.json` (every applicable item PASS or attested) *and* a lessons entry, else disabled with the reason. | v1 |
| J5-5 | major | **Skipped stages break packet headers and file reads.** Gate B hard-codes `Stage [N] -> Stage [N+1]`, and 14 prompts hard-code `docs/handoff/stage-NN.md` (11.1 reads `stage-10.md`). With payments = no, Stage 10 is n/a, so Stage 11 reads a file that never exists and the header should say `Stage 9 -> Stage 11`. | Resolver rules, with tests: `[N+1]` = **next applicable stage**; any `docs/handoff/stage-NN.md` reference is remapped to the **latest signed packet** (or flagged if none); validation rule 1 checks M against the applicable path, not N+1. | v1 |
| J5-6 | major | §8.5 / F-021 want `SKIPPED:` and the model substitution recorded in each packet, but the blueprint's Gate B format has no such fields, so Claude will not write them. | **Foreman writes `SKIPPED:` and `SUBSTITUTIONS:` itself at sign time** (deterministic, not LLM-authored). Validation then checks they match `overlay.json`. | v1 |
| J5-7 | major | Branch protection collides with signing. Stage 5.2 enables "no direct pushes to main". From Stage 5 on (14 stages), a `handoff: stage N` commit on `main` cannot be pushed. | Sign commits to the **current branch**, always showing the branch name in the dialog. Option **Sign on new branch `handoff/stage-NN`**. Foreman does not push or open PRs (no `gh` in the allowlist); it says so. | branch name v1, new-branch option v1.x |
| J5-8 | major | Git failure paths. Pre-commit hook rejects, index lock, detached HEAD. The signature is on disk but uncommitted, and the stamp has "happened". | Sign is a two-phase action with visible state: `signed (commit pending)` -> `signed`. On failure show git's stderr and a **Retry commit**; stamp animation plays only after the commit succeeds. | v1 |
| J5-9 | minor | A mistaken signature has no exit. | **Revoke signature** while the next stage is not started: writes a new commit blanking APPROVED BY with a revocation note; after that, only a CR. | v1.x |
| J5-10 | minor | The orange "needs you" is on the stamp area only. Billy scanning the strip should see *which sheet* is waiting. | An attention ring on the exact sheet in the strip and in the miniature strips on Projects home. | v1 |
| J5-11 | minor | "EVIDENCE lists every inspection item" is not a validation rule; inspection bullets have no IDs. | Foreman assigns stable item IDs (`6.i3`) and adds rule 6: every applicable item appears in EVIDENCE. | v1.x |

---

## 6. Journey 6 · Stage gate: A inspection (headless or interactive), B packet, C lessons

**Billy's steps**
1. ⌘⇧G opens the Gate: the stage's INSPECTION bullets as a checklist with evidence fields.
2. Chooses "Run inspection headless". Waits. The checklist fills with PASS/FAIL and evidence. Some FAIL.
3. Fixes them (Crew Loop), re-runs, all PASS. Runs B (packet), C (lessons). Reviews. Signs.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J6-1 | **blocker** | **A headless run can "PASS" items only a human can verify.** Real inspection items: "a rollback has actually been run once", "you personally reviewed every payments diff", "someone outside your team reaches the wow moment without help", "SLOs held 7 consecutive days", "you checked the hand derivations". The schema returns PASS/FAIL/NOT_APPLICABLE with free-text `evidence`; a model can write plausible evidence for any of them. That violates "zero gates skipped by accident". | Every inspection item has a **verification kind**: `auto` (a declared command or file check), `attest` (human only: checkbox plus a required one-line note, never settable by the headless run), `time` (earliest date, with reminder). Headless sees only `auto` items; the schema gains `CANNOT_VERIFY`, and anything not `auto` is excluded from the prompt. Default for an untagged item is `attest`, so the safe behavior needs no tagging work. | v1 |
| J6-2 | major | §6.2 runs the headless gate in `--permission-mode plan` with `--allowedTools "Bash(...)"`. Plan mode is "read-only exploration then a plan", which may refuse or distort the verify commands. The headless docs describe `dontAsk` as the locked-down unattended mode: it denies anything not explicitly allowed, and allow-listed tools still run; `--permission-prompts none` is the flag for "nobody can answer". | Use `--permission-mode dontAsk` + explicit allow-list + `--permission-prompts none`. Spike S-2 confirms that allow-listed `Bash(pnpm test *)` patterns run and everything else is denied. | v1 (after S-2) |
| J6-3 | major | **Headless A leaves no session for B and C.** The blueprint says A is a fresh session and B/C are "same session" (B writes EVIDENCE from A's results). The spec never links them. | Capture `session_id` from the JSON result; run B and C with `--resume <session_id>` (interactive tab by default; headless optional). Store the id in `state.json`. | v1 |
| J6-4 | major | Headless runs are long (audits 5-30 min) and `--output-format json` returns only at the end: no progress, no cancel, and a failed/timeouted/invalid-schema run has no state. | Use `--output-format stream-json --verbose` for live progress (current item, tool uses), **Cancel** (SIGINT per the headless docs), a timeout, and a `failed` run state with "Open transcript" / "Run interactively instead". Persist raw results as `.foreman/gates/stage-NN-run-K.json`. | v1 |
| J6-5 | major | After FAIL there is no path. Blueprint 14: "Fix every CRITICAL and HIGH with the Crew Loop, then re-run the audit". Billy copies failure text into a new Claude session by hand: exactly the friction Foreman exists to remove. | Per failed item: **Fix this** opens a CLI step (Opus 5.5 · high · Plan -> Accept edits, the BUG route) prefilled with the item, evidence and artifacts. **Re-run failed items only**. A third failure of the same item suggests the F.4 "happened twice" prompt. | copy-as-prompt + re-run v1, auto F.4 v1.x |
| J6-6 | major | Gate readiness for Crew Loop stages. Stages 6-13 end when *all their tickets are closed*. F-005 has "inspecting" but nothing says when the Gate unlocks. | Gate unlocks when every ticket whose `Stage` column matches is closed. Otherwise show "4 of 11 Stage 6 tickets open" with a link. | v1 |
| J6-7 | minor | Conditional inspection items "drop out of the gate" (F-003), but tags live on stages/steps; the bullets are untagged. | Default: all bullets apply. Per-bullet `fm:if` markers added only where needed (Stage 5, 11, 14, 16, 17). For TradeLog everything applies, so this can follow v1. | v1.x |
| J6-8 | minor | Evidence review burden: ten items of evidence text. | Show a diff-style summary first (FAIL on top, then CANNOT_VERIFY, then PASS collapsed); artifacts copyable; `Re-check this item`. | v1.x |
| J6-9 | minor | `/usage` snapshots per gate are a blueprint metric (F.6) that Foreman cannot read. | Optional "usage note" field in the sign dialog, stored in the run log. | v1.x |

---

## 7. Journey 7 · Working a ticket through the Crew Loop

**Billy's steps**
1. Tickets screen -> milestone M1 -> ticket M1-03 (HARD). Foreman shows routing: Opus 5.5 · xhigh · Plan -> Manual.
2. "Run next loop step": Plan (blueprint: Desktop, new session, worktree ON). Billy approves the plan.
3. Build (same session, Manual, tests first). Review (same session, switch to Opus high, Accept edits, `/code-review`, walk through diff). Commit (switch to Sonnet low). Close (PR merges, archive session).

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J7-1 | **blocker** | **The daily loop runs on Desktop, where Foreman cannot set anything.** Per the CLI reference, `claude --desktop` "takes no prompt and no other flags". So every loop step is: copy the prompt, open Desktop, set model (⌘⇧I), effort (⌘⇧E), mode (⌘⇧M), toggle worktree, paste. Five steps x N tickets x many tickets. This is the highest-volume activity in Stages 6-13 and the exact manual routing the app promises to remove ("Run next loop step", "routing automatically"). | Make **CLI the default loop surface** for Crew Loop steps, using documented flags: `claude -w <ticket-id> --model --effort --permission-mode --session-id "<prompt>"` (worktree at `<repo>/.claude/worktrees/<name>`). Desktop stays an opt-in per ticket (UI tickets that need the Browser pane; or the HARD diff review via `claude --desktop --resume <session-id>`, which the docs say opens that session in Desktop). A project/ticket setting "Loop surface: CLI | Desktop | Auto (UI tickets -> Desktop)". Record as a surface substitution like F-021. | v1 |
| J7-2 | major | Loop steps 2-4 switch **model, effort and mode** between steps in one session ("switch to Opus high Accept edits", then "Sonnet low"). Typing `/model` `/effort` and Shift+Tab by hand per step is slow and easy to skip. | "Run next loop step" = continue the session chain (J4-1): either relaunch `claude --resume <id> --model --effort --permission-mode "<prompt>"` (after the previous process exits), or Send-to-session plus Foreman typing `/model x` `/effort y` **only when idle** (idle from J4-4 events). Needs S-3/S-4. Always show the Now/Next strip. | v1 |
| J7-3 | major | **Ticket status has no home.** The 4.6 ticket format has no Status field; Foreman "reads docs/tickets.md" and "tracks each ticket's step and session" in `state.json`. Is `state.json` committed? If yes, worktrees each carry a copy and Foreman writes only the main checkout; if no, Billy loses progress on a new machine. The ticket text itself comes from a chat reply, so the parse is the second riskiest assumption after the blueprint. | Decisions for v1: (1) tickets.md stays read-only to Foreman; status lives in `.foreman/state.json`, **committed on the main checkout only, never written inside worktrees**; (2) tighten the 4.6 prompt to a strict per-ticket heading + labeled-field layout and add golden tests for it; (3) ticket parse errors show line numbers, not a blank list. | v1 |
| J7-4 | major | Loop shape differs by class, not "five steps": SPEC-COMPLETE replaces steps 1-2 with the `implementer` shortcut (which already runs reviewer); MECHANICAL says "batch several"; HARD adds Manual. F-013 says five loop steps. | Per-class loop shapes in the routing table; MECHANICAL supports **multi-select -> one batch prompt** listing the IDs. | shapes v1, batch v1.x |
| J7-5 | major | Worktree blind spot. Claude in `.claude/worktrees/<name>/` writes `docs/lessons.md` entries, ticket edits, handoff files *there*. The watcher on the main checkout will not see them until merge, and `.claude/worktrees` contains whole repo copies that a naive recursive watch will double-index. | Watcher ignores `.claude/worktrees/**` for packet/lesson/ticket truth but records `ticket -> worktree path -> branch` in state. Close-out suggestion: Foreman checks `git branch --merged` and proposes "M1-03 looks merged: close ticket?". | v1 |
| J7-6 | major | `[ID]` and "every ADR it lists" are inputs Foreman already has from the ticket. | The resolver binds `[ID]` (and `[M#]` in 12.1) from the selected ticket; zero typing. Also append nothing else: the prompt text stays the blueprint's. | v1 |
| J7-7 | minor | Dependencies: tickets list "Depends on"; Stage 7 and 12 ask for which can run in parallel worktrees. Spec lists status only. | Ticket rows show `Ready / Blocked by M1-02 / In loop (step 3 of 5, Opus high) / Closed`. "Next ticket" picks the first ready by milestone then class. | v1 |
| J7-8 | minor | Three tickets in flight means three tabs; which one needs Billy? | Handled by the Attention Inbox and tab dots (J8-1). | v1 |
| J7-9 | minor | Review step says `/code-review` and "Walk through in diff", a Desktop UI feature. | In the CLI route, step 3 uses the `code-reviewer` agent prompt (also in the blueprint). Spike S-5 confirms `/code-review` exists in the CLI. | v1 |

---

## 8. Journey 8 · Three projects in one morning; "what's waiting on me"

**Billy's steps**
1. Opens Foreman: Projects home with three miniature strips and "waiting" notes.
2. Opens TradeLog, signs a landed packet, starts the next step. Switches to Swings, a gate has failed. Switches to Foreman (the app), a council session is waiting for "go".
3. Wants one place that says what to do next across all three.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J8-1 | major | "Anything waiting on you" (Projects screen) has no defined sources. Packet-unsigned and gate-failed are file-based and easy. The ones that cost the most time are session-based: a Claude session blocked on a plan approval, permission prompt or `go`. Nothing in the spec can see them. | Define an **Attention model**: kinds (`session-blocked`, `packet-unsigned`, `gate-failed`, `step-ready`, `wait-elapsed`, `chat-awaiting-paste`), priority order, source, and one global **Inbox** (⌘I) across all projects with J/K/Enter, plus Dock badge and macOS notifications that jump to the item. `session-blocked` comes from J4-4 events (hook types include `permission_prompt`, `idle_prompt`, `agent_needs_input`). Comparables: Linear Inbox and Warp's notification mailbox with tab indicators. | file-based kinds v1, session-blocked v1 if S-1 |
| J8-2 | major | Home shows projects; Billy's real question each morning is "what's the next thing I should do, in what project". Core loop says "pick a project -> it shows the next ready step". | Home is a **Today** list: ordered attention items first, then each project's single *Do next* (one line, run config, ⌘↵). Global **Do next** (⌘⇧↵) executes the top item (sign, run step, open chat). The same idea as GitHub Desktop's single context-sensitive primary button (Fetch/Pull/Push). | v1 |
| J8-3 | major | Terminal tabs belong to a project, but sessions keep running when Billy switches. Where do they go? Closing the drawer should not stop them. | Sidebar project rows carry a state dot: running / blocked (attention) / idle, and a count. Per-project terminal tab sets persist across switches; last selected stage/step per project is restored. | v1 |
| J8-4 | minor | Fast switching. | ⌘1..⌘9 for pinned projects, ⌘K fuzzy ("trad" -> TradeLog), ⌃Tab MRU. | v1 |
| J8-5 | major | Cross-project concurrency and the Max plan: three Opus xhigh sessions and a Fable chair can hit the weekly limit unnoticed (Appendix B #9: Fable up to 50% of the limit). Foreman cannot read usage. | Show a live count of running sessions and a **Fable running** badge; warn when starting a second Fable session. v1.x: usage note per gate (J6-9). | v1.x |
| J8-6 | minor | "Since I last looked." | Activity feed (packet landed, session ended, gate result) in the Inbox footer, derived from the run log. | v1.x |
| J8-7 | minor | Success metric "start any step in under 10 seconds from opening the app" is not instrumented. | Log `ms from app focus/project open to run start` in the run log; show a median in Dashboard. Directly serves the kill criterion. | v1.x |

---

## 9. Journey 9 · Things going wrong

### 9a. Claude CLI outdated

Billy presses ⌘↵ on a step that needs Opus 5.5; installed Claude Code is older.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J9-1 | major | Doctor is a screen; the failure surfaces at runtime as a CLI error inside a terminal tab. | **Launch preflight** before every spawn: required version = max(model minimum, flag minimums) from the step; cached `claude -v` (refresh on failure). On mismatch, an inline banner on the step: "Needs 2.1.280, found 2.1.270. [Run claude update] [Re-check]" and Run is disabled. | v1 |
| J9-2 | minor | The CLI auto-updates and renames things (`default` -> `manual` alias exists in help since v2.1.200). | Use values the docs list as accepted; add a Doctor "flag probe" in CI-style tests against `claude --help` output so Foreman fails loudly in tests, not for Billy. | v1.x |

### 9b. A packet is invalid

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J9-3 | major | "Shown with each problem and can't be signed" is a dead end for Claude-Code packets too. | **Repair**: resumes the stage's session (`--resume <session-id>`) with a generated message listing exactly the problems ("Fix only these; do not change other content"). Alternative buttons: Open in editor; (for chat) Copy fix request (J3-8). If OPEN ITEMS is not `none`, show the items and say what is allowed: resolve them, or move to KNOWN LIMITS with a ticket ID. | v1 |

### 9c. A gate fails

Covered by J6-1..J6-5. Additional: after FAIL the stage status returns to `in progress`; the sheet shows a fail cross; the Inbox item is `gate-failed` (J8-1). **v1.**

### 9d. A terminal / Foreman crashes

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J9-4 | major | NFR says "never loses run state on crash", but a run's state is only defined in the run log, written at outcome time. A crash or quit loses the PTY child and the scrollback. §6.1 mentions `claude --resume <name>` but names are ambiguous across re-runs. | Write the run record **at launch** (`session-id`, cwd, argv, step, ticket, start time, status `running`) and update on exit. On next launch: **Interrupted sessions** list with one-click **Resume** (`claude --resume <session-id>`). Persist the last ~2,000 lines of each tab to `runs/<id>.log`. Tab states: running / exited(code) / errored / interrupted. | v1 |
| J9-5 | major | Quitting Foreman kills running sessions silently. | Quit guard: "3 sessions are running. They end when Foreman quits; you can resume them." Same for closing a project. | v1 |
| J9-6 | minor | Alternative substrate: `claude --bg` + `claude attach` sessions survive terminal/app closure, and `claude agents --json` gives `state` (working/blocked/done) and `waitingFor`. Attractive for "what's waiting". But agent view is a research preview, `--bg` sessions default to **their own git worktree** (opt out with `worktree.bgIsolation: "none"`), and permission-mode/effort flags are not documented for `--bg`. That would send council files into a worktree. | Not v1. Record as the leading v1.x substrate candidate; spike S-6. Direct PTY + J4-4 events is the v1 path. | v1.x |
| J9-7 | minor | Hung process: tab says running forever. | Tab menu: Interrupt (Ctrl-C), Terminate; plain-language state. | v1 |

### 9e. A stale overlay edit

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J9-8 | major | An overlay edit replaces a step's prompt/run config, but records no base. When the blueprint moves (retro applies a proposal; pin upgraded) the edit silently overrides the improved prompt, or is applied to text that no longer matches. SR-07 shows a diff against the template but cannot tell "your edit" from "template changed". | Each overlay edit stores `baseTemplateVersion` + `baseHash` of the text it edited. On mismatch: **Stale edit** banner on the step (not on launch), showing base / yours / new; choices Keep mine, Take new, Edit again. Never auto-merge. | hash + banner v1, 3-pane merge v1.x |
| J9-9 | major | `overlay.json` is git-tracked: a merge or hand edit can make it invalid, or `state.json` can be changed by a git pull while Foreman is open. | Invalid file -> Foreman opens the project **read-only** with a precise parse error and offers Open in editor; it never overwrites an invalid or newer file (consistent with §7). Reload on change with a visible "reloaded from disk" toast. Stable ordering of entries (sorted by id) so diffs are small. | v1 |
| J9-10 | major | There is no "upgrade this project to blueprint 2.2" flow. | **Upgrade pin** screen: shows changed steps, marks those in stages already signed as informational, those ahead as "will change", and lists overlay edits that become stale. | v1.x |

---

## 10. Journey 10 · Skip, not-applicable, custom step, promote a lesson

**Billy's steps**
1. Skips an optional step: reason field, recorded in the packet.
2. Views a not-applicable stage (hatched, one-line reason).
3. Clicks + between steps, adds a custom prompt, picks "This project only" or "Propose for the blueprint".
4. Opens Lessons, picks L-007, "Promote to blueprint": a proposal file lands in the blueprint repo.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J10-1 | major | **Prompts that bundle optional or conditional parts.** 1.4 is "press release + FAQ (optional) **and** the packet (core)" in one prompt; 14.2 mixes load test (server only), EXPLAIN ANALYZE and failure modes; 3.2 has a "legal part"; Stage 11 mixes SLOs (server) and analytics (multi_user). Skipping the step skips the core part; running it runs the n/a part. | Conditional blocks inside prompts: `<!-- fm:if server --> ... <!-- fm:end -->`, stripped by the resolver, with the skipped block listed in the packet's SKIPPED. For TradeLog every condition is yes, so this can follow v1, but until then 1.4 must be tagged **core** (no skip), and any step with mixed content is run whole. | v1.x (+ tag fix v1) |
| J10-2 | major | **Prompts that read outputs of skipped steps.** 2.2 reads `02-teardown.md` (2.1 conditional); 11.1 reads `stage-10.md` (J5-5). Claude hits a missing file and improvises. | **Input preflight** before Run: scan the resolved prompt for repo paths (`docs/...`, `spec/...`, `council/...`), check they exist; a missing signed packet is a hard stop, other missing paths a visible warning ("02-teardown.md not found: step 2.1 was skipped"). Cheap regex, high value, serves "impossible to do wrong". | v1 |
| J10-3 | major | Add-step UI is a two-choice menu (project only / blueprint). A usable prompt needs text, run config (surface, model, effort, mode, agents, cwd), position, tag (core/optional), declared output, run inputs, and an inspection item needs a verification kind. | One **Add step** sheet that reuses the step editor: fields above, live resolved-prompt preview, lint ("unknown variable [FOO]"), and "Duplicate from existing step" as a starting point. Inspection items: text, kind (auto/attest/time), optional command. | v1 |
| J10-4 | minor | Skip details: undo after skip, and skip after a packet is signed. | Undo until the stage packet is signed; afterwards only via CR. Skipped rows show "Skipped by you: <reason>" with Undo. | v1 |
| J10-5 | major | "Promote to blueprint" is defined for lessons (F-014) but the overlay's "promote" flag has no behavior; and lessons record the fix as free text, so Foreman cannot reliably construct old -> new text. §4 also says it "drafts a CHANGELOG entry", but CHANGELOG.md is a blueprint file Foreman must not edit. | One flow for both: **Create proposal**. The form shows the target step's *current* text from the pinned template, the lesson's FIX text, and the new text; Foreman verifies the old text exists verbatim in blueprint HEAD (else flags "stale"). Writes `proposals/YYYY-MM-DD-L-NNN.md` including the CHANGELOG draft *inside* the proposal file. Promotion status lives in Foreman's `state.json`, **not** by editing `docs/lessons.md` entries (avoids touching Claude-authored text). | v1 |
| J10-6 | minor | After the retro applies a proposal and Billy bumps the pin, his own overlay entry duplicates the blueprint. | Detect "overlay equals new template" and offer to remove it. | v1.x |
| J10-7 | minor | Lessons IDs: Claude numbers `L-NNN` itself in each session; collisions across sessions. | Foreman validates uniqueness on watch and shows a duplicate warning. | v1.x |
| J10-8 | minor | The F.5 retro is the payoff of the calibration loop, yet needs path-to-lessons and the F.6 metrics table pasted by hand. | **Start retro** at Certificate of Occupancy: cwd = blueprint repo, `[path]` filled, F.6 table generated from the run log and packet dates, promoted proposals listed. | v1.x |

---

## 11. Journey 11 · A Change Request that turns payments on

**Billy's steps**
1. Project is at Stage 9, created with `payments = no`. He decides to sell it.
2. Change requests -> New CR (template). Runs 18.3 in chat ("Change Request CR-NNN: ...") against the Master Spec; Claude recommends APPROVE and outputs a CR PACKET of deltas for scope, ADRs, threats, data, tickets.
3. He records APPROVE; applies `payments = yes` (and probably `commercial = yes`).
4. Foreman reactivates Stage 3.1 (business model), parts of 3.2, Stage 10, 15 legal, 17 live payments.

| ID | Sev | Friction | Fix | Scope |
|---|---|---|---|---|
| J11-1 | **blocker** | **No model for out-of-order reactivation.** Strict order says Stage N starts after N-1 is signed. Stage 3 and 4 are signed and behind him; the payments stages need a provider decision (3.1), a security/legal delta (3.2), ADR/threat/data/ticket deltas (4.x), and only then Stage 10 tickets. F-015 says only "apply condition changes and overlay deltas". Reopening signed stages would rewrite history; ignoring them leaves Stage 10 unbuildable. | Define a **CR track**: applying a CR creates a short, ordered checklist of *delta steps* on the Project screen (18.3 chat -> CR packet signed as `docs/handoff/cr-NNN.md` -> 3.1/3.2 delta -> 4.1/4.2/4.3/4.4/4.6 deltas -> new tickets appended -> Crew Loop). Earlier signed packets stay untouched and are marked "amended by CR-NNN". Reactivated later stages (10, 15 legal, 17) become `ready` only when the CR track is signed and any stage-ordered prerequisites exist. Gates already passed that gain items (e.g. Stage 8 for payment threats) show "re-inspection required for new items only". | minimal model v1 (list + gating + amended marker), polish v1.x |
| J11-2 | major | Billy must see impact before applying. Design direction says destructive actions confirm "exactly what will happen" but the contents are not defined. | Apply dialog = diff: conditions changed, steps/stages reactivated, signed stages that will be marked amended, gates gaining items, and the new CR track steps. | v1 |
| J11-3 | major | The blueprint has no prompts for "delta" application (18.3 says "Stage 4.1 / 4.2 / 4.6 deltas") so Foreman has no prompt text to resolve. | Ask blueprint v2.2 for a **18.3b CR delta** prompt (input: CR packet; output: amended docs). Until then Foreman offers it as an overlay-added step "Foreman addition" that Billy can promote. | blueprint v2.2 + overlay step v1 |
| J11-4 | minor | CR is a document with statuses (proposed / approved / rejected / applied) and numbering. | `docs/change-requests/CR-NNN.md` from the kit's template; Foreman auto-numbers, validates like a packet (OPEN ITEMS `none`), and Apply is disabled until the decision is APPROVE and signed. | v1 |
| J11-5 | minor | Payments implies other answers (commercial, others_data, server); Stage 17 has a test-mode variant ("Not taking payments yet?"). | Offer implied toggles in the Apply dialog (see J2-9). | v1.x |

---

## 12. Cross-cutting: what the template needs (answers Open Question 6 and more)

The parseable text covers headings, RUN WITH lines, fenced prompts, packet code blocks and inspection blockquotes. It does **not** reliably expose the facts below. Outputs are partly regex-parseable (`-> council/r1-market.md`, "Save the result to ...", "Write docs/handoff/stage-[N].md"), but not enough to drive the watcher. Recommend a small **annotation layer**: HTML comments in the blueprint, invisible in the PDF, checked by the golden tests (F-002 reports unknown/missing annotations with line numbers).

| Annotation | Carries | Needed by |
|---|---|---|
| `fm:cwd project\|spec\|blueprint` | Working directory | J4-2 |
| `fm:session new\|continue\|fresh` | Session chain behavior | J4-1, J7-2, J6-3 |
| `fm:start` / `fm:switch` | Launch flags vs ordered later changes | J4-3 |
| `fm:alt` | Model/plan alternatives with a default rule | J4-9 |
| `fm:input name type label` | Run-time inputs | J3-2 |
| `fm:out path` (repeatable) | Declared outputs (watcher, round tracker, save-output) | J3-5, J4-5 |
| `fm:kind prompt\|action\|manual\|wait\|repeatable` | Step types (council setup, "record your choice", 7-day test, ring reviews 17.2, weekly/monthly/quarterly Stage 18) | J4-7, J3-1 |
| `fm:if <cond>` ... `fm:end` | Conditional blocks inside prompts and inspection bullets | J10-1, J6-7 |
| `fm:verify auto\|attest\|time` | Inspection item kind | J6-1 |
| `fm:requires path` | Inputs that must exist | J10-2 |

Stage 17.2 (ring health review before *each* of 4 rings) and Stage 18 (weekly dependency updates, monthly ADR hygiene, quarterly restore drill) are **repeatable** steps with a history, not once-and-done statuses; model them as `repeatable` with a run counter and optional cadence reminder. v1 can ship `repeatable` with a counter; cadence reminders v1.x.

---

## 13. Verified facts and spikes

### Verified against official docs (links in section 16)

- `--permission-prompts none` exists (v2.1.259+); `--effort`, `--name`/`-n`, `--session-id` (UUID), `--resume` (id or name), `--worktree`/`-w`, `--output-format json|stream-json`, `--json-schema` (print mode only) all exist. `claude auth status` JSON includes `authMethod`. `--permission-mode` accepts `default|acceptEdits|plan|auto|dontAsk|bypassPermissions|manual`.
- `claude --desktop`: "Takes no prompt and no other flags except `--verbose` and the `--debug` flags"; needs v2.1.285+; `--continue` or `--resume <session-id>` opens that session in Desktop.
- Headless JSON output includes a session id usable with `--resume`; `structured_output` holds schema results; SIGINT ends a turn, SIGTERM does not record a result.
- Hooks: `Notification` types include `permission_prompt`, `idle_prompt`, `agent_needs_input`, `agent_completed`; payloads include `session_id`, `transcript_path`, `cwd`, `permission_mode`, `effort.level`. The hooks page does **not** establish that `--settings` can add hooks per session.
- Agent view: `claude agents --json` exposes `state` (`working|blocked|done|failed|stopped`) and `waitingFor`; `--bg` sessions default to a worktree; research preview.

### Spikes to run before the spec is frozen (Stage 2 council or early Stage 6)

| ID | Question | Pass condition | Decides |
|---|---|---|---|
| S-1 | Can a Foreman hook (user-level, env-gated) report `Notification`/`Stop` with `permission_mode`/`effort` for a PTY-launched interactive session? | Events arrive for permission prompt, idle, plan approval | J4-4, J8-1 (session-blocked), mode guard |
| S-2 | Does `claude -p --permission-mode dontAsk --allowedTools "Bash(<cmd> *)" --json-schema ...` run the verify commands and deny the rest? | Items verified; a non-listed command is denied and reported | §6.2 mode choice (J6-2) |
| S-3 | Does `claude --resume <id> --model --effort --permission-mode "<prompt>"` apply the new flags to the resumed session? | Model/mode/effort reflect flags | J4-1, J7-2 |
| S-4 | Bracketed paste of a 3-8 KB prompt into a live Claude Code TUI; sending `/model x` + Enter when idle | Prompt arrives intact, not auto-submitted | Send-to-session |
| S-5 | `claude -w <name> ... "<prompt>"` in a PTY; does `/code-review` exist in the CLI? | Worktree created; command available | J7-1, J7-9 |
| S-6 | `claude --bg` with `worktree.bgIsolation: none`: viable substrate (attach in embedded PTY, state via `claude agents --json`)? | Council writes into `spec/`; state accurate | v1.x substrate |
| S-7 | Env scrubbing: does an inherited `ANTHROPIC_API_KEY` change billing/auth for an interactive session? What is the minimum clean env? | Documented behavior confirmed | SR-10, J1-3 |

---

## 14. The 10 changes that most improve daily speed and confidence (ranked)

Ranking = how often Billy hits it x cost of the mistake it prevents.

| # | Change | Covers | Scope |
|---|---|---|---|
| 1 | **Today list + Attention Inbox + global "Do next"** (⌘⇧↵): one ordered list across all projects of what needs Billy now, each with a one-key action; Dock badge and notifications; project state dots. This is the core loop and the "under 10 seconds" metric. | J8-1, J8-2, J8-3, J5-10 | v1 (session-blocked kind needs S-1) |
| 2 | **Session chains**: "same session" steps continue the conversation (`--session-id` plus `--resume` with new flags, or Send-to-session), with a Now/Next strip for model, effort and mode switches. One in three steps depends on it. | J4-1, J4-3, J7-2 | v1 |
| 3 | **Run inputs and input preflight**: classify every token (variable, packet marker, run input, literal), remember run inputs per project, bind `[ID]` from the ticket, remap `stage-NN.md` to the last applicable signed packet, and check referenced files exist before Run. Delivers "zero manual prompt edits". | J3-2, J5-5, J7-6, J10-2 | v1 |
| 4 | **Crew Loop in the CLI with worktrees** (`claude -w <ticket>`), per-class loop shapes, "Run next loop step" in one keystroke; Desktop opt-in for UI tickets and diff review. | J7-1, J7-4, J7-5 | v1 |
| 5 | **Honest gates**: verification kinds (auto / attest / time), headless only for `auto`, `dontAsk` + allow-list, live progress and cancel, `session_id` carried into B and C, order A -> B -> C -> Sign. | J6-1..J6-4, J5-1, J5-4 | v1 |
| 6 | **Safe signing**: hash-bound sign, pathspec-only commit, two-phase commit state, Foreman-written SKIPPED/SUBSTITUTIONS, branch shown, revoke later. | J5-2, J5-3, J5-6, J5-7, J5-8 | v1 |
| 7 | **Chat bridge**: paste-back that normalizes `<blank>`, splits packet from attachments, writes multiple destinations, saves non-packet outputs to a target path, **Copy fix request**; chat-setup chips; Project knowledge manifest. | J3-3..J3-9 | v1 (split-by-heading, hashes v1.x) |
| 8 | **Wizard and first run that save typing and stop traps**: global variable defaults, tri-state conditions editable until the Stage 2 freeze, late conditions at Stage 4, setup actions (council folder, claude.ai Project URL), env scrub and `authMethod` check, first action as the wizard's last screen. | J1-1..J1-3, J2-1..J2-6 | v1 |
| 9 | **Resilience**: run record written at launch, Interrupted sessions + Resume, quit guard, launch preflight with version table and one-click `claude update`, invalid-file read-only mode. | J9-1, J9-3, J9-4, J9-5, J9-9 | v1 |
| 10 | **Template annotation layer** (cwd, session, outputs, kinds, conditional blocks, verification kinds): the data that makes 2-5 and 7 possible, checked by golden tests; plus a **live mode/effort guard** from hook events once S-1 passes. | section 12, J4-4 | annotations v1; guard v1 if S-1 |

---

## 15. Scope call

### Essential for v1 (the process can be broken, silently or expensively, without these)

- Run config with `cwd`, `session` chain, `start`/`switch` cues, alternatives (J4-1, J4-2, J4-3, J4-9)
- Token classification, run inputs, input preflight, packet/stage remapping (J3-2, J5-5, J10-2, J7-6)
- Gate verification kinds, headless on `dontAsk`, session carry-over, A -> B -> C -> Sign (J6-1..J6-4, J5-1, J5-4)
- Hash-bound sign, pathspec commit, Foreman-written SKIPPED/SUBSTITUTIONS (J5-2, J5-3, J5-6, J5-8)
- `<blank>` and normalization, packet/attachment split, Copy fix request, Repair (J3-6..J3-8, J9-3)
- Step kinds `action`/`manual`/`wait`/`repeatable` with states (J3-1, J4-7)
- Crew Loop on CLI worktrees, ticket status home, loop shapes, watcher ignoring worktrees (J7-1..J7-6)
- Attention model, Inbox, Do next, state dots (J8-1..J8-3)
- Wizard defaults, tri-state conditions, late conditions, setup actions, template lock, env scrub, authMethod check, first-run checklist (J1-1..J1-4, J2-1..J2-5)
- Run record at launch, interrupted-session resume, quit guard, launch version preflight, read-only on invalid files, stale-edit banner with base hash (J9-1, J9-4, J9-5, J9-8, J9-9)
- CR track minimal model and impact dialog (J11-1, J11-2, J11-4)
- Add-step sheet and unified Create proposal (J10-3, J10-5)
- Chat-stage gate as attestations + F.2 lessons step (J3-9)
- Blueprint edits (via proposals): CLI packets written as files (J4-8), 1.4 tagged core, annotation layer, strict ticket format (J7-3)

### Can wait for v1.x

Adopt-existing-project UI (J2-8); condition consistency warnings (J2-9); Project-knowledge hashes and stale-upload flags (J3-4); Split-by-heading and `FILE:` multi-file outputs (J3-5); Markdown output preview (J4-6); transcript capture of packets (J4-8 fallback); `--ax-screen-reader` toggle (J4-12); conditional blocks inside prompts and per-bullet gate conditions (J10-1, J6-7); item IDs and EVIDENCE rule (J5-11); revoke signature (J5-9); sign-on-new-branch option (J5-7); auto F.4 suggestion (J6-5); gate usage note (J6-9); Fable/usage warnings (J8-5); activity feed and launch-latency metric (J8-6, J8-7); `claude --bg` substrate (J9-6); upgrade-pin screen and 3-pane stale merge (J9-8, J9-10); retro starter (J10-8); duplicate-overlay detection and lessons ID checks (J10-6, J10-7); cadence reminders for Stage 18; CR polish (J11-5).

---

## 16. Comparable tools: what to borrow (brief)

- **Warp agent notifications**: toasts, a notification mailbox with All/Unread/Errors, tab indicators and click-to-jump to the blocked agent. Borrow for the Inbox and tab dots. https://docs.warp.dev/agent-platform/capabilities/agent-notifications
- **Claude Code agent view**: groups sessions into Needs input / Working / Completed, "peek and reply" without attaching, and prints state as JSON. Borrow the groupings and the peek idea; use as the v1.x substrate. https://code.claude.com/docs/en/agent-view
- **Linear Inbox and keyboard-first triage**: one list for "what needs me", J/K and one-key actions. Borrow for Today/Inbox. https://linear.app/docs/inbox
- **GitHub Desktop**: one primary button that changes meaning with repository state (Fetch / Pull / Push). Borrow for the single "Do next" button. https://docs.github.com/en/enterprise-server@3.22/desktop/working-with-your-remote-repository-on-github-or-github-enterprise/syncing-your-branch-in-github-desktop
- **Zellij session resurrection**: sessions restorable after crash. Borrow the principle for Interrupted sessions (our resurrection is `claude --resume`, not serialization). https://zellij.dev/documentation/session-resurrection
- **Raycast placeholders**: fill-in arguments at invocation time, remembered. Borrow for run inputs. https://manual.raycast.com/dynamic-placeholders

Primary sources for Claude Code behavior used above: CLI reference https://code.claude.com/docs/en/cli-reference · headless https://code.claude.com/docs/en/headless · hooks https://code.claude.com/docs/en/hooks

---

## 17. Questions for the council to settle in Round 3

1. Is the annotation layer (section 12) acceptable in the blueprint, or must Foreman infer everything from today's Markdown? (Recommend annotations; inference keeps as golden-test checks.)
2. Does `state.json` live in git (committed on main only) or app data? (Recommend git, main only, because the repo is the source of truth per §7.)
3. Is Crew Loop surface CLI-by-default acceptable given the blueprint says Desktop? (Recommend yes, as a documented surface substitution, with Desktop kept for UI tickets and HARD diff review.)
4. Hook-based session telemetry: install user-level env-gated hooks via Doctor, or accept PTY-only state in v1? Depends on S-1.
5. Should conditions be freely editable until the Stage 2 freeze? (Recommend yes.)
