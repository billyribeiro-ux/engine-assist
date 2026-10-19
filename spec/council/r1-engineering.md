# Council Round 1 · Principal Engineer · Feasibility and Stack Verification

**Project:** Foreman · **Input:** Master Spec draft v0.9 (`docs/spec-input/01-master-spec.md`) · **Date:** 2026-10-19
**Role:** principal-engineer (feasibility for a solo principal engineer using Claude Code; evidence only, no claims from memory)

How the evidence was gathered:

- **Registries and docs, live:** versions come from the crates.io and npm registry APIs on 2026-10-19, read live. Doc content comes from code.claude.com, svelte.dev (through the official Svelte MCP docs server), v2.tauri.app, webdriver.io and docs.rs.
- **Hands-on probes** (in the scratchpad, not the repo):
  1. A Rust program parsed the real `claude-build-blueprint.md` with both `comrak 0.56.0` and `pulldown-cmark 0.13.4`.
  2. A second Rust program tested an HTML-comment marker.
  3. A minimal **SvelteKit 3.1.0 + adapter-static 4.0.0 + Vite 8.3.4 + TS 6.0.3** SPA was built and type-checked.
  4. The source of `tauri-plugin-pty`, `portable-pty 0.9.0`, `tauri 2.12.4`, `tauri-utils 2.10.2` and `tauri-plugin-shell 2.5.0` was read.
- **What could not be verified** is marked **UNVERIFIED**, with what was tried.

---

## Summary

### Recommendations at a glance

| # | Question / area | Recommendation | Confidence |
|---|---|---|---|
| A1 | Template source of truth | **Keep the Markdown as the source.** Add a small set of invisible `<!-- foreman: … -->` annotations, a strict validator, and a checked-in generated JSON snapshot for review diffs. No YAML/JSON source. | High |
| A2 | PTY | **`portable-pty` directly** (about 300 lines of our own Rust). Don't use `tauri-plugin-pty`: it's an experimental single-maintainer wrapper over the same crate, and it lets the frontend pick the program, which breaks SR-01. | High |
| A3 | E2E on macOS | **`tauri-driver` does not support macOS** (official docs). Use layers instead: Rust unit, proptest and integration tests with a fake `claude` binary; Vitest 5 component tests with `mockIPC`; Playwright flows against the web build with `mockIPC`; and a thin real-app smoke suite through `@wdio/tauri-service` with the embedded `tauri-plugin-wdio-webdriver`, compiled only under an `e2e` cargo feature. | High |
| A4 | SvelteKit 3 + adapter-static + Tauri 2 | **Changes are needed.** SvelteKit 3 (3.0.0 on 2026-10-01, now 3.1.0) **no longer supports `svelte.config.js`**, so the Tauri guide's config is invalid. Config moves into `sveltekit({ adapter })` in `vite.config.ts`, `$lib` becomes `#lib`, and TS must be **6.x**. Verified by a working build plus `svelte-check` with 0 errors and 0 warnings. | High (built and tested) |
| A5 | Headless vs interactive gate | **Headless by default for Gate A** on CLI stages, but with **`--permission-mode dontAsk`** (not `plan`) plus an exact allowlist, `--permission-prompts none`, and the JSON schema. Then resume the same session ID interactively for Gates B and C. Interactive is the fallback, and the path for human-judgement items. | Medium-high |
| A6 | Declared output files | **Not reliably parseable from prose.** Add a marker grammar: `<!-- foreman: id=… tag=… when=… outputs=… packet=… -->` under each `### Prompt` heading. Keep the prose heuristics as a lint cross-check only. | High |
| B | Stack | All rows are viable. Three changes: **pin TypeScript 6.0.x** (npm `latest` is 7.0.2, which SvelteKit 3.1, svelte-check and typescript-eslint all reject); **use Tauri's built-in `windowEffects`** (the window-vibrancy crate isn't needed); **don't add `tauri-plugin-shell`**. | High |
| C | Claude Code flags | Every flag in §6 exists and behaves as described. Six errors or gaps are listed in the next table. | High |
| D | Feasibility | v1 is buildable solo. Highest-risk features: **F-002, F-008, F-012** (high/med-high). Largest: F-002, F-004, F-008, F-012, F-013 (L). | Medium |

### Spec errors and gaps found

| # | Where | Problem | Fix |
|---|---|---|---|
| E1 | §11 Frontend | The Tauri SvelteKit guide (written for SvelteKit 2.20.4) puts the adapter in `svelte.config.js`, which **SvelteKit 3 no longer supports**. | Use the `vite.config.ts` config in §A4. |
| E2 | §11 Language | "TypeScript strict" has no version. npm `latest` = **7.0.2**, but `@sveltejs/kit@3.1.0` peers `typescript ^6.0.0`, `svelte-check@4.7.7` peers `^5 \|\| ^6`, and `typescript-eslint@8.72.0` peers `<6.1.0`. Blueprint Prompt 5.1 ("latest stable versions") would pick 7 and break the toolchain. | Pin `typescript@6.0.3`. Re-check when TS 7.1 ships its programmatic API. |
| E3 | §6.2 | `--permission-mode plan` in a `-p` run: when auto mode is available, the **classifier** reviews planning-time shell commands. So commands outside the allowlist could run. | Use `--permission-mode dontAsk`. It is the documented "exact allowlist" mode. |
| E4 | §6.3 | Version table incomplete. `--desktop` needs **v2.1.285**, `--permission-prompts` needs **v2.1.259**, and `-p` starts in Manual from **v2.1.302**. The npm `stable` tag is **2.1.292**, below Haiku 5.5's minimum of 2.1.293. | Doctor floor: **2.1.285**, or 2.1.293 when the template uses Haiku 5.5. Warn on the stable channel. |
| E5 | §6.1 | `--name` isn't unique. Re-running a step creates duplicate names, and `claude --resume <name>` then opens a picker instead of resuming. | Foreman generates a UUID and passes **`--session-id <uuid>`** with `--name`. Resume by ID. |
| E6 | SR-06 | "No inline script" can't hold literally. The SvelteKit SPA fallback `index.html` contains one inline bootstrap `<script>` (verified in the probe build). | Reword: "no inline script that isn't hash-pinned; `script-src` never contains `'unsafe-inline'`". Tauri hashes local scripts at compile time. |
| E7 | §11 Tests | `tauri-driver` e2e on macOS isn't supported. | See §A3. |
| E8 | F-013 | The blueprint's Crew Loop steps are **Desktop** surface. Desktop takes no model, effort or mode flags, and loop step 5 is a Desktop UI action. So "routed automatically" is impossible as written. | Decide: run loop steps through the **CLI** (Foreman can route the flags), or show Desktop steps with menu instructions only. |
| E9 | §6.2 / §7 | Verify commands "parsed from CLAUDE.md". CLAUDE.md is free-form prose, so parsing it is fragile and makes the allowlist depend on prose. | `.foreman/project.json` is the only allowlist source. Offer a one-time import suggestion from CLAUDE.md. |
| E10 | §5.3 vs blueprint | Tags (core, conditional, optional) and conditions exist only in the spec's table, **not in the blueprint text**. So F-002 can't produce tagged steps from the Markdown. | Carry them in the A6 markers. |
| E11 | §6.2 (latent) | `--bare` is "recommended for scripted calls", but bare mode **never reads OAuth credentials**, so it needs an API key. | Foreman must never pass `--bare`. Add this to §6. |

---

## A. Open questions (§13)

### A1. Markdown as source of truth, or a structured YAML/JSON source?

**Recommendation: the Markdown stays the source of truth.** Foreman reads it with a strict parser plus a small marker vocabulary (A6). Don't move to YAML/JSON with generated Markdown.

**Evidence**

1. **Parse fidelity on the real file (probe, 2026-10-19).** Both parsers produced identical structure for the 100 KB blueprint, with exact line numbers:

   | Element | comrak 0.56.0 | pulldown-cmark 0.13.4 |
   |---|---|---|
   | `##` headings (stages and sections) | 31 | 31 |
   | `###` headings (prompts, gates, sections) | 91 | 91 |
   | Blockquotes | 92 (63 start with "RUN WITH", 17 with "INSPECTION") | 92 |
   | Fenced code blocks by info string | `text` 63 · `markdown` 16 · `json` 1 · `handoff` 1 | identical |
   | GFM tables | 26 | 26 |
   | First `handoff` block | lines 28–36 | lines 28–36 (from byte offsets) |
   | Parse time | 1.7 ms | 0.9 ms |

   The blueprint's structure is regular enough to parse reliably. The riskiest assumption in the App Brief is answered positively at the block level.

2. **The human and the retro edit the Markdown.** Appendix F.5's retro prompt edits `claude-build-blueprint.md` directly: "Show old text → new text … Apply the surviving edits … Re-render the PDF". F-014 also writes proposals as old → new text. A YAML source would mean:
   - every retro edit is made in YAML;
   - the PDF render pipeline is rebuilt from YAML;
   - prose-heavy prompts get wrapped in YAML block scalars, which are worse to review and diff than Markdown.

   That's a large process change for no gain in parse reliability.

3. **Where the Markdown is genuinely insufficient** (A6, E10), the gap is *metadata*, not structure:
   - step IDs, tags, conditions and declared outputs;
   - human-judgement gate items.

   An HTML comment carries metadata without changing the rendered document. The probe confirmed that comrak returns `<!-- foreman: output=docs/runbook-deploy.md tag=conditional when=server -->` as an `HtmlBlock` with its exact line number. The blueprint already contains 6 HTML-comment blocks (`<!-- -->` separators), so the renderer handles them today.

**What to build**

- The parser builds a typed `Template`. The validator **refuses** a template with any of the following, reporting each with file and line:
  - a `### Prompt` without a RUN WITH;
  - a marker with an unknown key;
  - a duplicate `id`;
  - a conditional step without `when`;
  - an output path the heuristics found but no marker declares.
- The claude-blueprint repo gets a generated `template.snapshot.json`, written by a small CLI mode of the parser, so every retro edit shows a reviewable structural diff. The snapshot is generated and never edited.
- Overlays reference steps by marker `id`, **never by heading text or position**. Retro edits renumber and reword prompts, and positional references would silently re-target.

**RUN WITH lines are a grammar with variants, not one shape.** The parser must model these variants found in the 63 RUN WITH lines:

- 22 "same session / same chat";
- 12 mode transitions (`**Plan** → **Manual**`);
- 7 with alternatives or switches (`(or **Opus 5.5** · **xhigh**)`, `Chair: … — or …`, "model and effort from the table above");
- 4 mid-session slash commands (`` `/model opus` ``);
- the surfaces: CLI 16, Desktop 10, chat 20.

Only 14 blocks carry a backticked `claude …` command. **Foreman must build argv from the parsed fields** and use the backticked command only as a consistency check (the validator warns on mismatch).

### A2. PTY: `portable-pty` directly, or `tauri-plugin-pty`?

**Recommendation: `portable-pty 0.9.0` directly, in about 300 lines of Foreman's own Rust.**

| | `portable-pty` | `tauri-plugin-pty` |
|---|---|---|
| Latest | 0.9.0, 2025-02-11 | 0.3.1, 2026-07-08 |
| Downloads (all time / recent) | 22.0 M / 12.8 M | 51 k / 10.9 k |
| Reverse dependencies | **662**, including atuin, skim, presenterm, bottom, wezterm-ssh, tui-term | n/a |
| Repository | wezterm/wezterm `pty/`, still maintained: commits 2026-06-07, 2026-08-25 (winapi → windows-sys), 2026-09-12 | Tnze/tauri-plugin-pty: 48 commits, 2 main authors. The crates.io `repository` field is empty. README: "Developing! Wellcome to contribute!" |
| macOS | Unix backend uses `libc::openpty` | Same, since it wraps `portable-pty = "0.9.0"` |
| Size | full crate | `src/lib.rs` is 237 lines |
| Security fit | Rust decides the program | **The JS `spawn(file, args, {cwd, env})` lets the webview choose any program.** That contradicts SR-01 (allowlisted absolute paths enforced in Rust) unless the plugin is wrapped or forked. |
| Data path | your choice | `read` is a request/response command returning ≤ 4096 bytes per IPC call, so the frontend polls |

The plugin adds a dependency, a permission surface and a polling design without adding capability. Its whole value is the 237 lines we'd write anyway, under our own security rules.

**Implementation notes for the ADR (verified in source)**

- **Spawn.** Use `CommandBuilder::new(<absolute path to claude>)` with the argv array and `cmd.cwd(project)`.
  - `CommandBuilder` inherits the parent environment (`get_base_env` copies `std::env::vars_os()`).
  - It does **not** set `TERM`. Set `TERM=xterm-256color` and `COLORTERM=truecolor` explicitly.
- **Stream output** with a reader thread from `try_clone_reader()` into a `tauri::ipc::Channel<Vec<u8>>`. Tauri 2.12.4's own docs describe `Channel` as "how you push an arbitrary number of messages to the JavaScript side … log lines, streamed responses". Keystrokes go through `take_writer()`, `resize()` maps to `MasterPty::resize`, plus kill and wait.
- **PATH: a macOS GUI app launched from Finder or the Dock doesn't inherit the login shell's PATH** ([flaviocopes](https://flaviocopes.com/macos-app-command-line-tools-path)). So:
  - Resolve `claude` to an absolute path. The official native installer puts the launcher at `~/.local/bin/claude`, a symlink into `~/.local/share/claude/versions/`. The Homebrew cask is the alternative.
  - Build an explicit child `PATH`: the resolved tool directories, then `/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin`. Claude Code's Bash tool needs `git`, `pnpm`, `cargo` and the rest.
  - Settings (F-020) can override it.
- **Strip `ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN` from the child environment by default.** The brief requires the Max plan and no API key. The docs say an `ANTHROPIC_API_KEY` that you approve skips the subscription login.

### A3. E2E: is `tauri-driver` usable on macOS?

**No.** The official Tauri WebDriver page says desktop support through `tauri-driver` is Windows and Linux only, because **"macOS has no WKWebView driver tool available"**. It recommends `@wdio/tauri-service`, whose embedded WebDriver server "is how macOS is supported". CrabNebula's `tauri-driver` fork supports macOS, but "a paid API key is required for macOS" (`CN_API_KEY`, plus `tauri-plugin-automation`).

**Recommended strategy (layered, cheapest first):**

| Layer | Tooling (verified versions) | Covers |
|---|---|---|
| 1. Rust unit + property | `cargo test`, `proptest 1.12.0` | Parser, validator, rule engine (§5), packet rules (§8), resolver, overlay merge, atomic writes |
| 2. Rust integration | `tauri 2.12.4` `test` feature: `tauri::test::mock_builder()`, `mock_app()`, `get_ipc_response()`, `assert_ipc_response()` (read in source) | Commands through IPC with no webview |
| 3. Process integration | A **fake `claude` binary** fixture: a small Rust bin that records argv, cwd and env, echoes to the PTY, honours resize, and exits with a scripted code | F-008, F-009, F-012, F-018: argv exactness, SR-01/02/03, PTY lifecycle, Doctor parsing. Real-CLI calls aren't needed in CI. |
| 4. Component | `vitest 5.0.5` (+ `@vitest/browser-playwright 5.0.5` + `vitest-browser-svelte 3.1.0`, or jsdom + `@testing-library/svelte 5.4.2`) with `@tauri-apps/api/mocks` `mockIPC`, `mockWindows`, `clearMocks` | Every component and state. `mockIPC(…, { shouldMockEvents: true })` since 2.7.0. |
| 5. Flow (web build) | `@playwright/test 1.64.1` against the Vite dev or preview build, with `mockIPC` injected | Keyboard-only flows, Cmd+K, focus order, axe checks for WCAG 2.2 AA |
| 6. Real-app smoke (local macOS) | `@wdio/tauri-service 1.5.0` + `tauri-plugin-wdio-webdriver 1.5.0` (MIT, 2026-10-08, repo webdriverio/desktop-mobile, 263 k downloads) | About 5 smoke paths in the real WKWebView: launch, open project, run fake-claude step, packet lands, sign |

Notes:

- The embedded plugin **opens an HTTP WebDriver server on `127.0.0.1:4445`**. Its docs say to "never include it in production builds", and gating is your own code's job. Foreman is always run locally in debug-ish builds, so gate it behind a **cargo feature `e2e`**, not just `cfg(debug_assertions)`, and add a CI check that the release binary doesn't link it.
- Playwright's WebKit project is the closest stand-in for WKWebView in layer 5. That's from the Playwright docs; **UNVERIFIED** this session (page not re-fetched).
- The Tauri testing overview page `https://v2.tauri.app/develop/tests/` was **blocked** (permission timeout). The WebDriver and mocking sub-pages were read.

### A4. SvelteKit 3 + `adapter-static` + Tauri 2

**Status (npm registry):**

- `@sveltejs/kit` **3.0.0 released 2026-10-01**, 3.0.1 on 2026-10-06, **3.1.0 on 2026-10-14**.
- `@sveltejs/adapter-static` **4.0.0** (2026-10-01) peers `@sveltejs/kit ^3.0.0-next.0`. adapter-static 3.x peers kit ^2.
- The last SvelteKit 2 release was 2.70.3 (2026-08-18).

**SvelteKit 3 minimums** (official migration guide, "Migrating to SvelteKit v3"): Node 22.17, **TypeScript 6**, Svelte 5.57.1, **Vite 8.0.12** ("the first Vite 8 release bundling stable rolldown v1"), `@sveltejs/vite-plugin-svelte` 7. These match the registry peer dependencies of kit 3.1.0: `vite ^8.0.12`, `svelte ^5.57.1`, `typescript ^6.0.0`, `vite-plugin-svelte ^7.0.0`, and engines `node >=22.17`.

**Breaking changes that affect Foreman:**

- **`svelte.config.js is no longer supported`.** Config goes to `sveltekit({...})` in `vite.config.*`. Former `kit.*` options become top-level plugin options.
- **`$lib` is now `#lib`**, declared in `package.json` `imports`, with file extensions in import paths.
- `$app/environment` is renamed `$app/env`. `$app/stores` is removed (use `$app/state`).
- `tsconfig.json` extends `$app/tsconfig` and must list `include` and `exclude`.
- `version.pollInterval` now **defaults to one hour**. `updated` is also checked on focus and visibility.
- `ssr = false` in the root layout is unchanged: "your entire app will only be rendered on the client". adapter-static's `fallback` option is unchanged.

**The Tauri SvelteKit guide** (`v2.tauri.app/start/frontend/sveltekit/`) says it is accurate as of SvelteKit 2.20.4. It configures `adapter-static` in `svelte.config.js` with `fallback: 'index.html'`, `+layout.ts` with `ssr = false`, and `frontendDist: "../build"`. **The `svelte.config.js` part is invalid on SvelteKit 3; the rest still applies.**

**Verified config.** Built 2026-10-19 with kit 3.1.0, adapter-static 4.0.0, vite 8.3.4 (rolldown 1.2.14), svelte 5.57.2, TS 6.0.3, and svelte-check 4.7.7 with `--fail-on-warnings`. Result: `build/index.html` + `build/_app/`, and svelte-check reported **0 errors, 0 warnings** on 171 files.

```ts
// vite.config.ts
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  clearScreen: false,                               // Tauri Vite guide: keep Rust errors visible
  server: { port: 5173, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  plugins: [
    sveltekit({
      adapter: adapter({ fallback: 'index.html' }), // SPA; Tauri loads index.html from frontendDist
      version: { pollInterval: 0 }                  // no deployments to poll in a desktop app
    })
  ]
});
```

```ts
// src/routes/+layout.ts
export const ssr = false;
```

```jsonc
// package.json (excerpt)
{ "type": "module",
  "imports": { "#lib": "./src/lib/index.ts", "#lib/*": "./src/lib/*" } }
// tsconfig.json
{ "extends": "$app/tsconfig", "compilerOptions": { "strict": true }, "include": ["src", "*.ts"], "exclude": [] }
// src-tauri/tauri.conf.json → build
{ "beforeDevCommand": "pnpm dev", "beforeBuildCommand": "pnpm build",
  "devUrl": "http://localhost:5173", "frontendDist": "../build" }
```

About the fallback name, the SvelteKit docs say "avoid `index.html` where possible to avoid conflicting with a prerendered homepage". Foreman prerenders nothing (`ssr = false` everywhere), so there's no conflict. Keep `index.html`, which matches Tauri's default entry.

### A5. Headless gate inspection: default or fallback?

**Recommendation: headless is the default for Gate A on Claude Code stages (5–17). Interactive is the fallback.** Change the §6.2 argv as follows:

```text
claude -p --model opus --effort high
  --permission-mode dontAsk
  --tools Read,Grep,Glob,Bash,Agent
  --allowedTools Read Grep Glob "Bash(<verify cmd 1>)" "Bash(<verify cmd 2>)"
  --permission-prompts none
  --max-turns <N>
  --output-format json --json-schema '<inspection schema>'
  "<Gate prompt A, resolved>"
```

**Why headless by default** (all from code.claude.com, 2026-10-19):

- **Structured, validated output.** "To get output conforming to a specific schema, use `--output-format json` with `--json-schema` … with the structured output in the `structured_output` field". An invalid schema exits with an error. That fills the checklist deterministically, which is the product's core promise ("zero gates skipped by accident").
- **`dontAsk` is the documented exact-allowlist mode.**
  - The permission-modes page lists "Run in CI with an exact allowlist: `claude -p "run the test suite" --permission-mode dontAsk --allowedTools "Bash(npm test)" "Read"`".
  - "Claude Code auto-denies every tool call that would otherwise prompt you", while still running reads in the working directories, read-only Bash commands and allow-rule matches.
  - So the run is **read-only by construction** (edits are denied), which matches the blueprint's "Do not fix anything; just report".
- **Why not `plan` (E3).** In `plan` mode, "Auto mode available and the `useAutoModeDuringPlan` setting on, which it is by default: the classifier reviews shell commands … Approved commands run". That means commands outside the verify list could run. `dontAsk` bounds them to the allowlist.
- **`--permission-prompts none`** (v2.1.259+): "Anything that would prompt is denied … Claude is told that nobody can approve the request and not to retry it". It also removes `AskUserQuestion`, so the run can't hang waiting for input.
- **Glob and Grep on macOS.** The default tool set on macOS "leaves Glob and Grep out" (searches go through Bash). "Naming either tool in `--allowedTools` restores both". Pass `--tools` explicitly too so the set is deterministic. Include **`Agent`**: Gate A tells Claude to use the `test-runner` agent, and the Agent tool needs no permission.
- **The session stays resumable.** `-p` sessions are excluded from the picker, but "you can still resume one by passing its session ID to `claude --resume <session-id>`". Foreman passes `--session-id <uuid>`, so it knows the ID up front. **Gates B and C then run interactively** as `claude --resume <uuid> --permission-mode acceptEdits "<Gate B prompt>"` in a terminal tab. That keeps the blueprint's "same session" semantics: B and C need edits, and you review them.

**Guardrails**

1. **No `--bare` (E11).** "In bare mode, Claude Code never reads OAuth credentials or the system keychain", so it would need an API key.
2. **Mind the variadic `--allowedTools`.** It's a variadic list. Issue #25978 ("Variadic --allowedTools option consumes positional prompt argument") was closed as fixed on 2026-02-15. Even so, Foreman orders argv so a single-value flag always comes between the list and the positional prompt, and a Stage 5 integration test against the real CLI proves the prompt isn't swallowed.
3. **Output parsing.** "When a failure happens inside the run, such as missing authentication, Claude Code prints the failure as the result on stdout". Branch on the exit code and the result object. Don't assume `structured_output` exists.
4. **Schema design.**
   - Each item has `id` (an **enum of the gate's marker IDs**), `status` ∈ {`PASS`, `FAIL`, `NOT_APPLICABLE`, **`NEEDS_HUMAN`**}, `evidence`, and `artifacts`.
   - Items such as Stage 4's "Your test: you'd hand this package to a contractor you've never met", or Stage 1's "the press release excites a real target user", are human judgements. They're tagged `judge=human` in the template and never sent to the headless run.
5. **When to fall back to interactive:**
   - the CLI version is below 2.1.259;
   - the result fails schema or ID validation;
   - the gate has only human items;
   - the user chooses it.
6. **Live progress.** `--output-format stream-json --verbose` streams events. Whether the final `result` message in stream-json also carries `structured_output` is **UNVERIFIED**: the docs show it only for `json`. Spike it in Stage 5; until then use `json` and a spinner.
7. **Subagents.** Whether subagents spawned inside a `dontAsk -p` run inherit the session's allow rules exactly is **UNVERIFIED**. The sub-agents page wasn't fetched. Cover it with the fake-claude integration test plus one real-CLI check in Stage 5.

### A6. Can a step's declared output file be parsed from the text?

**Not reliably. Add a marker.** Every output-file phrasing found in the blueprint:

| Line | Phrasing | Kind |
|---|---|---|
| 273 | `Write docs/handoff/stage-[N].md in exactly this format:` | Gate B packet (templated N) |
| 297 | `Append an entry to docs/lessons.md` | append, not create |
| 501 | `> **HANDOFF** → … copy them into a new folder `[app]-spec/01-app-brief.md`` | HANDOFF callout |
| 538 | `Save the result to `[app]-spec/02-teardown.md`.` | prose after the block |
| 643–657, 673, 701, 716 | `-> council/r1-market.md` … **8 outputs in one prompt (2.2)** | arrow |
| 1046 | `save the packet itself as `docs/handoff/stage-04.md`` | HANDOFF callout |
| 1139, 1707 | `Write docs/runbook-deploy.md.` · `Write docs/launch-plan.md.` | imperative inside prompt |
| 1283 | `Update docs/threat-model.md so …` | modify existing |
| 1657 | `record times and results in docs/drills.md` | prose |
| 1596 vs 1599 | 15.1: "Output it all as one structured COPY PACKET" · 15.2: "Read … docs/copy.md (the Copy Packet)" | **implied, never stated** |

Why heuristics fail:

- five different verbs;
- inputs look identical to outputs ("Read docs/threat-model.md" vs "Update docs/threat-model.md");
- one implied file (`docs/copy.md`);
- the outputs of a prompt with eight of them.

Any retro rewording (F.5) would silently break detection.

**Recommended marker grammar.** It goes on its own line directly under a `### Prompt` heading, or at the end of an inspection bullet:

```ebnf
marker   = "<!--" , ws , "foreman:" , { ws , attr } , ws , "-->" ;
attr     = key , "=" , value ;
key      = "id" | "tag" | "when" | "outputs" | "appends" | "packet" | "judge" ;
value    = bare | '"' , { char - '"' } , '"' ;
bare     = { char - ( ws | '"' ) }- ;
(* outputs / appends : comma-separated repo-relative paths; [N], [app] placeholders allowed  *)
(* tag  ∈ core | conditional | optional                                                     *)
(* when : condition expr over §5.2 ids, e.g.  commercial|payments   server   !idea_known   *)
(* judge ∈ claude | human   (inspection items only; default claude)                          *)
```

Examples:

```markdown
### Prompt 2.2: Convene the council
<!-- foreman: id=2.2 tag=core outputs="council/r1-market.md,council/r1-users.md,council/r2-features.md,council/r2-growth.md,council/r2-feasibility.md,council/r3-synthesis.md,council/r4-redteam.md,council/r5-resolution.md" -->

### Prompt 5.5: Staging pipeline
<!-- foreman: id=5.5 tag=conditional when=server outputs=docs/runbook-deploy.md -->

> - Your test: you'd hand this package to a contractor you've never met <!-- foreman: id=4.g5 tag=core judge=human -->
```

- **Validator rules:**
  - every marker key is known, and IDs are unique and stable;
  - `when` parses and references only §5.2 IDs;
  - `tag=conditional` requires `when`;
  - Gate B steps get `packet=docs/handoff/stage-[N].md` automatically.
- **Lint (warning, not error):** a path that the heuristics above detect in a step's prompt but no marker declares.
- **Verified:** block-level markers parse as `HtmlBlock` with line numbers (comrak probe). Inline markers at the end of a list item are CommonMark raw inline HTML (`HtmlInline`), but that form was **not probed**. Cover it in the F-002 golden tests.
- **Cost:** about 70 marker lines added to the blueprint. They're invisible in rendered Markdown and the PDF. This is a MINOR blueprint bump under F.7 (it adds no prompts).

---

## B. Stack verification (§11)

Versions are from the crates.io and npm registry APIs on 2026-10-19.

| Layer | Draft choice | Current stable (date) | Maintenance / evidence | Verdict |
|---|---|---|---|---|
| Shell | Tauri 2 | **tauri 2.12.4** (2026-10-14); `@tauri-apps/api` 2.12.4; `@tauri-apps/cli` 2.12.3 | 37.5 M downloads. **Tauri 3 is in alpha**: `3.0.0-alpha.0` (2026-09-13) → `alpha.5` (2026-10-14) on crates.io | **Keep Tauri 2.** Record a v3 migration watch in the ADR. v3 timeline and breaking changes **UNVERIFIED**: the GitHub releases page and Tauri blog search were blocked or returned nothing. |
| Frontend | SvelteKit 3 + Svelte 5, adapter-static SPA | **@sveltejs/kit 3.1.0** (2026-10-14); **svelte 5.57.2** (2026-10-06); **adapter-static 4.0.0** (2026-10-01); vite-plugin-svelte 7.3.1 | Official v3 migration guide; build verified (§A4) | **Keep**, with the §A4 config (E1). |
| Build | Vite 8 | **vite 8.3.4** (2026-10-08); 8.0.0 was 2026-03-12; rolldown 1.2.14 | Kit 3 needs ≥ 8.0.12 | **Keep.** |
| Language | TypeScript strict | npm `latest` = **7.0.2** (2026-07-08); **6.0.3** (2026-04-16) | InfoQ (2026-08-03): 7.0 "ships without a stable programmatic API, which the team expects to land in 7.1", so framework tooling can't use it yet. Peer ranges: kit `^6.0.0`, svelte-check `^5\|\|^6`, typescript-eslint `<6.1.0` | **Pin `typescript@6.0.3`** (E2). |
| Language | Rust stable | sandbox `rustc 1.97.0` (2026-07-07); `tauri-plugin-shell` README requires ≥ 1.90 | — | Keep. Latest stable Rust **UNVERIFIED** (rust-lang.org not fetched). Pin with `rust-toolchain.toml`. |
| Runtime | Node | Kit 3 engines `>=22.17`; vitest 5 `^22.12 \|\| ^24 \|\| >=26` | nodejs.org/dist/index.json returned **proxy 403** | Use Node ≥ 22.17 LTS. Current LTS line **UNVERIFIED**. |
| Package manager | pnpm | **pnpm 12.14.0** (2026-10-15); engines node ≥ 18 | The probe built fine with pnpm 10.28 | **Keep.** Pin via `packageManager`. |
| Terminal | `@xterm/xterm` | **6.0.0** (2025-12-22); 6.1.0-beta.315 active (registry modified 2026-10-13); addon-fit 0.11.0, addon-webgl 0.19.0, addon-unicode11 0.9.0 | **`@xterm/addon-canvas` 0.7.0 peers `@xterm/xterm ^5`**, so it's incompatible with 6. The 6.0 typings include `screenReaderMode` and `macOptionIsMeta` | **Keep xterm 6.** Use the WebGL or DOM renderer, not canvas. Turn on `screenReaderMode` for the VoiceOver requirement. Full 6.0 breaking-change list **UNVERIFIED** (GitHub release page blocked). |
| PTY | `portable-pty` or `tauri-plugin-pty` | portable-pty **0.9.0** (2025-02-11); tauri-plugin-pty **0.3.1** (2026-07-08) | See §A2 | **portable-pty directly.** |
| Process spawn | Rust std/tokio + PTY, no shell | tokio 1.54.0 (2026-10-19); tauri-plugin-shell 2.5.0 (2026-10-10) | `tauri-plugin-shell` exposes child-process spawning to the webview behind a capability scope. The shell plugin docs page was **blocked**; the crate README was read: "spawn child processes and manage files and URLs" | **Don't add `tauri-plugin-shell`.** Use `std::process::Command`/`tokio::process::Command` (argv arrays, no shell) for `claude -p`, `claude -v`, `claude auth status` and `git`, and portable-pty for interactive runs. Use **`tauri-plugin-opener` 2.7.2** to open claude.ai in the browser (an OS hand-off, not a Foreman network call). |
| File watching | `notify` | **notify 8.2.0** stable; 9.0.0-rc.6 (2026-10-17); **notify-debouncer-full 0.7.0** depends on `notify ^8.2.0` | 174 M downloads; default feature `macos_fsevent` | **Keep notify 8.2 + debouncer-full 0.7**, not the 9.x RC. Debounce about 250 ms to meet the ≤ 1 s NFR. |
| Markdown | comrak or pulldown-cmark | **comrak 0.56.0** (2026-10-06; 13 releases in 12 months; 315 reverse deps; BSD-2). **pulldown-cmark 0.13.4** (2026-05-20; 1,867 reverse deps; MIT) | Probe (§A1): identical structure on the real blueprint, both with tables, blockquotes and fenced info strings. **comrak gives an AST with `sourcepos` line and column on every node.** pulldown-cmark gives an event stream with byte ranges, so you build your own line index and tree. | **comrak** (pin `=0.56.0`, `default-features = false`). The AST matches the nesting the parser must walk (blockquote → paragraph → inline code; list item → HTML marker), and line numbers come free for parse errors. Pin exactly: comrak ships breaking 0.x releases roughly monthly. pulldown-cmark is a sound fallback. |
| UI primitives | Bits UI | **bits-ui 2.19.5** (2026-10-03); peer `svelte ^5.33.0` | `dist` has **no `@sveltejs/kit` import** (checked), so it's independent of the SvelteKit version. It ships `command`, `dialog`, `alert-dialog`, `context-menu`, `dropdown-menu`, `tabs`, `tooltip`, `select`, `toggle-group`, `scroll-area` and more. README: "Flexible, unstyled, and accessible primitives" (vendor claim) | **Keep.** bits-ui.com was blocked. Weekly downloads **UNVERIFIED** (npm downloads API 403). |
| Tests (TS) | Vitest | **vitest 5.0.5** (2026-10-15), peer vite `^8`; `@vitest/browser-playwright` 5.0.5; `vitest-browser-svelte` 3.1.0; `@testing-library/svelte` 5.4.2; `@playwright/test` 1.64.1 | Svelte docs recommend Vitest and Playwright | **Keep.** |
| Tests (Rust) | `cargo test` + proptest | **proptest 1.12.0** (2026-10-10) | 215 M downloads | **Keep.** Add the `tauri` `test` feature for `mock_builder`. |
| E2E | WebdriverIO + tauri-driver | `@wdio/tauri-service` 1.5.0, `tauri-plugin-wdio-webdriver` 1.5.0, webdriverio 10.1.0 | §A3 | **Replace** with the §A3 strategy (E7). |
| Lint / format | ESLint + Prettier + svelte-check; rustfmt + clippy | eslint **10.13.0**; eslint-plugin-svelte **3.23.1** (peer eslint ^10 OK); typescript-eslint **8.72.0** (TS < 6.1); svelte-check **4.7.7**; prettier **3.9.10**; prettier-plugin-svelte **4.1.1** | All compatible with TS 6.0.x, Svelte 5 and Vite 8 by their peer ranges | **Keep.** |
| macOS window look | vibrancy / transparent titlebar | Built into Tauri 2 config (read from `tauri-utils 2.10.2` source) | `titleBarStyle`: `Visible \| Transparent \| Overlay`. `hiddenTitle` and `trafficLightPosition` exist. **`windowEffects`** takes NSVisualEffect materials (`sidebar`, `underWindowBackground`, `hudWindow`, `windowBackground` and more, macOS 10.14+) and **Liquid Glass** (`liquidGlassRegular`/`Clear`, macOS 26+), and needs `transparent: true`. One Liquid Glass and one Visual Effect material can be listed together so macOS 15 and below fall back. **`macOSPrivateApi` is a no-op in 2.12.1+** ("APIs are always enabled now"). Overlay caveats (source doc comment): needs a custom drag region, and "you can't drag the window when it's not in focus" (tauri#4316) | **Use built-in `windowEffects` + `titleBarStyle`.** The `window-vibrancy` crate (0.8.1, 2026-09-22) isn't needed. Supply a Visual Effect fallback material, because the NFR covers the current and previous macOS majors. |
| Security config | Capabilities + CSP | Tauri 2.12 | **Capabilities:** JSON/TOML in `src-tauri/capabilities`, scoped by `windows` and `platforms`. **App commands are allowed for all windows by default** unless `build.rs` uses `AppManifest::commands`. **CSP:** "Tauri appends its nonces and hashes … automatically" at compile time, and is enabled only when `csp` is set. `freezePrototype` exists (default false) and freezes `Object.prototype` before frontend code. `pattern: isolation` is available. | Set an explicit `csp` (`default-src 'self'`; `script-src 'self'`; `connect-src ipc: http://ipc.localhost`; `img-src 'self' asset: http://asset.localhost data:`; `object-src 'none'`; `base-uri 'none'`) and a separate `devCsp` for Vite HMR. Set **`freezePrototype: true`**. Use `AppManifest::commands`. **Don't add `tauri-plugin-fs`**: all file I/O goes through Rust commands that check paths (SR-04). Capabilities can't validate *our own* command arguments; that enforcement is Rust code plus tests. Exact style CSP with Svelte and Bits UI inline styles needs a Stage 5 spike (**UNVERIFIED**). |
| App data path | `~/Library/Application Support/<bundle id>/` | `tauri 2.12.4` `app_data_dir()` = `data_dir()/<identifier>` (source) | — | Matches §7. |

---

## C. Claude Code CLI verification (§6)

Source: [CLI reference](https://code.claude.com/docs/en/cli-reference), [headless](https://code.claude.com/docs/en/headless), [model-config](https://code.claude.com/docs/en/model-config), [permission-modes](https://code.claude.com/docs/en/permission-modes), [sessions](https://code.claude.com/docs/en/sessions), [setup](https://code.claude.com/docs/en/setup), all fetched 2026-10-19. The npm `@anthropic-ai/claude-code` dist-tags: `latest` **2.1.303** (2026-10-18), `stable` **2.1.292**.

| Spec usage | Verified? | Doc facts and caveats |
|---|---|---|
| `claude "query"` (interactive with initial prompt) | ✅ | "Start interactive session with initial prompt". Interactive sessions start in **auto** mode by default from v2.1.283, so always pass `--permission-mode` (the spec and blueprint already do). |
| `--model` with `fable`, `opus`, `sonnet`, `haiku`, `opusplan` | ✅ | All are documented aliases. Also `best`, `default`, `sonnet[1m]`, `opus[1m]`. On the Anthropic API, `opus`/`sonnet`/`haiku` → Opus/Sonnet/Haiku 5.5, and `fable` → Fable 5.1. `opusplan` = "opus during plan mode, then switches to sonnet". |
| `--effort` with `low`, `medium`, `high`, `xhigh`, `max` | ✅ | Also `ultracode` (xhigh + ultracode, v2.1.203+). All five levels are supported on Opus, Sonnet and Haiku 5.5 and on Fable 5.1. `max` "applies to the current session only", which is fine since Foreman passes it per launch. |
| `--permission-mode` with `plan`, `default`, `acceptEdits`, `auto`, `dontAsk` | ✅ | Also `manual` (alias of `default`, v2.1.200+) and `bypassPermissions` (excluded by SR-02). `dontAsk` "never appears in the [Shift+Tab] cycle". |
| `--name` / `-n` | ✅ | "Set a display name … You can resume a named session with `claude --resume <name>`". **Caveat (E5):** an ambiguous name "opens the session picker". Add **`--session-id <uuid>`** ("must be a valid UUID") so Foreman owns the ID. |
| `--resume <name>` | ✅ | Takes an ID or name; name resolution covers the current repo and its worktrees. Resuming restores the model and, from a terminal, the permission mode unless `--permission-mode` is passed. If the session is a still-running background session, it **attaches** instead, and refuses (exit 1) when config flags are present. |
| `-p` | ✅ | "Query via SDK, then exit". From v2.1.302, `-p` starts in Manual when nothing sets a mode. `-p` sessions are hidden from the picker but resumable by ID. |
| `--output-format json` | ✅ | Print mode only. The result includes `result`, `session_id`, `total_cost_usd` (a client-side estimate) and `structured_output` with a schema. |
| `--json-schema` | ✅ | Print mode only. An invalid schema exits with "`--json-schema is not a valid JSON Schema`". The `format` keyword isn't enforced. |
| `--allowedTools` | ✅ | Permission-rule syntax, e.g. `Bash(git diff *)` (the space before `*` matters). It's variadic: see §A5 guardrail 2. Naming `Glob` or `Grep` restores both on macOS. |
| `--permission-prompts none` | ✅ | Print mode. **Requires v2.1.259**: "Earlier versions reject it with an unknown-option error". |
| `--desktop` | ✅ | Opens Desktop on the current directory and exits. "Takes no prompt and no other flags except `--verbose` and the `--debug` flags". macOS and x64 Windows, with a Claude subscription. **Requires v2.1.285** (E4). A bonus: `--desktop --resume <session-id>` opens a CLI session in Desktop. |
| `claude -v` | ✅ | `--version`, `-v`. Prints e.g. `2.1.211 (Claude Code)`; parse the leading semver. |
| `claude auth status` | ✅ | "Show authentication status as JSON. Use `--text` for human-readable output. Exits with code 0 if logged in, 1 if not." `authMethod` ∈ `none`, `claude.ai`, `oauth_token`, `api_key`, `api_key_helper`, `third_party`. `configDirectory` exists from v2.1.268. **Doctor should require `authMethod == "claude.ai"`** and flag `api_key` (it would bill the API, which the brief rules out). |
| §6.3 version minimums | ✅ (incomplete) | Opus 5.5 v2.1.280, Sonnet 5.5 v2.1.284 and Haiku 5.5 v2.1.293 match the model-config page exactly (Fable 5.1 is v2.1.257). Missing: `--desktop` v2.1.285 and `--permission-prompts` v2.1.259 (E4). |

**Useful but missing from §6:**

- `--session-id`;
- `--max-turns` (print-mode runaway guard);
- `--tools`;
- `--fork-session`;
- `claude doctor` (read-only install diagnostics that F-018 can surface);
- the stable vs latest channel (`autoUpdatesChannel`; Homebrew `claude-code` vs `claude-code@latest`).

---

## D. Feature feasibility (F-001…F-021)

Sizes assume one engineer working with Claude Code. **S** ≈ days, **M** ≈ 1–2 weeks, **L** ≈ 2–4 weeks, **XL** > 4 weeks. These are estimates.

| ID | Feature | Size | Risk | Reasoning |
|---|---|---|---|---|
| F-001 | Project wizard | M | Low | Forms plus atomic writes plus `git init` through the allowlisted git binary. Must mirror `new-repo-docs.sh` output exactly; golden-test against it. |
| F-002 | Template engine | **L** | **High** | Block structure parses cleanly (§A1 probe), but RUN WITH has several variant shapes (same-session, `→` transitions, alternatives, slash commands). Tags and outputs need markers (A6, E10). Placeholders are mixed: 22× `[APP_NAME]`, `[N]`, `[app]`, plus free-text ones like `[changes]`, `[X]`, `[describe the voice]`. Golden tests are mandatory. Risk drops to medium once markers land. |
| F-003 | Rule engine | M | Low | Pure functions over tags and conditions, plus a small `when` expression evaluator. Ideal for proptest (core is never skippable; a CR flip reactivates exactly the dependent items). |
| F-004 | Overlay engine | **L** | Med | Add, skip and edit are easy. The hard part is **rebasing overlays when the pinned blueprint version changes**, which needs stable marker IDs (A1). Also needs the SR-07 diff view. |
| F-005 | Stage map | S | Low | Derived view over state plus rules. |
| F-006 | Stage workspace | M | Low | Composition of the resolver, rules and actions. Add and skip affordances are gated by F-003. |
| F-007 | Prompt resolver | M | Med | Variable substitution is trivial. Classifying placeholders (project variable vs computed `[N]` vs packet paste vs free text that needs a prompt) and replacing `[paste the Stage N Handoff Packet]` variants (5 different wordings) is the risk. Must meet the ≤ 50 ms NFR (easily). |
| F-008 | Terminal runner | **L** | **High** | PTY plus xterm 6 hosting a full-screen TUI (alternate screen, bracketed paste, mouse, truecolor, resize, IME, VoiceOver `screenReaderMode`). GUI-app PATH resolution. Process lifecycle and crash-safe run state. Memory ≤ 300 MB with 3 tabs (cap scrollback; **UNVERIFIED** until a Stage 5 spike). Do this spike first. |
| F-009 | Desktop handoff | S | Low | Clipboard plugin plus spawning `claude --desktop` (v2.1.285+) with cwd. No prompt can be passed. Consider `--desktop --resume <id>` for continuing CLI sessions. |
| F-010 | Chat handoff | S | Low | Clipboard, opener and a paste box. Reuses the F-011 packet validator. |
| F-011 | Packet watcher and signing | M | Med | Rapid FSEvents notifications from editors' atomic saves (rename) need the debouncer and "newer file wins" rules. The signed commit goes through allowlisted git. The §8 rules are proptest-friendly. |
| F-012 | Gate runner | **L** | Med-high | Headless `dontAsk` plus schema plus resume for B and C (§A5). Builds the JSON schema from marker IDs, handles human-judgement items, and needs version gating and the fallback path. Depends on F-002 IDs and F-008 for B and C. |
| F-013 | Crew Loop | **L** | Med | Depends on the `docs/tickets.md` format, which Stage 4.6 fixes (a dependency). **Surface conflict (E8):** the blueprint's loop steps are Desktop, so automatic flag routing only works through the CLI. Step 5 can't be automated. |
| F-014 | Lessons | M | Low-med | Parse the Appendix F lessons format and write `proposals/…md` into the blueprint repo, which is within SR-04 scope. "Never edits the blueprint" is easy to enforce in Rust. |
| F-015 | Change Requests | M | Med | Applying a CR mutates conditions and the overlay with an audit trail. It must re-run F-003 and show reactivated items. |
| F-016 | Dashboard | M | Low | Aggregates the run log, state and packet dates. |
| F-017 | Command palette and keyboard | S–M | Low | Bits UI ships `command`. Mostly a shortcut registry plus focus management for WCAG 2.2 AA. |
| F-018 | Doctor | S–M | Low | `claude -v`, `claude auth status` (JSON plus exit code, `authMethod`), optionally `claude doctor`, plus git, pnpm, node and cargo checks. Version floor 2.1.285 (E4). PATH discovery is shared with F-008. |
| F-019 | Run log | S | Low | Monthly JSONL appends, plus search (a linear scan is fine at personal scale). |
| F-020 | Settings | S | Low | JSON plus `schemaVersion` migrations; tool path auto-detection shared with F-018. |
| F-021 | Model substitution | S | Low-med | A mapping applied when building argv. "Recorded in each packet" means injecting a line into the Gate B prompt, since Claude writes the packet. The display must show the original and the substitute. |

**Build-order advice (by risk):**

1. A Stage 5 spike for F-008 (PTY + xterm + real `claude` TUI) and F-012 (headless argv + schema + resume by ID).
2. F-002 with markers and golden tests.
3. F-003, F-007, F-011.
4. Then the UI features.

---

## Could not verify (attempts)

| Item | Tried | Result |
|---|---|---|
| Tauri testing overview page | WebFetch `https://v2.tauri.app/develop/tests/` | Permission timeout (blocked). The WebDriver and mocking sub-pages were read instead. |
| Tauri v3 timeline and breaking changes | WebFetch `github.com/tauri-apps/tauri/releases`; `gh api`; WebSearch | Blocked / "GitHub access not enabled for this session" / no announcement found. Only the crates.io alpha dates are confirmed. |
| `window-vibrancy` docs | WebFetch `docs.rs/window-vibrancy` | Blocked. Used the `tauri-utils` source instead (built-in effects). |
| Shell plugin docs page | WebFetch `v2.tauri.app/plugin/shell/` | Blocked. Used the crate README. |
| xterm.js 6.0.0 release notes | WebFetch GitHub release | Blocked. Used registry peer dependencies and typings. |
| bits-ui.com docs, npm weekly downloads | WebFetch; `api.npmjs.org` | Blocked / proxy 403. |
| Node.js current LTS line | `nodejs.org/dist/index.json` | Proxy 403. |
| Latest stable Rust | not fetched | Sandbox has 1.97.0 (2026-07-07), which isn't authoritative. |
| `structured_output` in a stream-json `result` | headless docs | Documented only for `json`. Spike in Stage 5. |
| Subagent permission inheritance in `dontAsk -p` | not fetched (sub-agents page) | Test in Stage 5. |
| Inline `HtmlInline` marker at the end of a list item | not probed | Cover in F-002 golden tests. |
| Playwright WebKit as a WKWebView proxy | playwright.dev not fetched | Low risk; confirm when setting up layer 5. |
| Memory with 3 terminal tabs (NFR) | — | Needs a Stage 5 measurement. |

---

## Sources

**Claude Code (official)**
- https://code.claude.com/docs/en/cli-reference
- https://code.claude.com/docs/en/headless
- https://code.claude.com/docs/en/model-config
- https://code.claude.com/docs/en/permission-modes
- https://code.claude.com/docs/en/sessions
- https://code.claude.com/docs/en/tools-reference
- https://code.claude.com/docs/en/setup
- https://registry.npmjs.org/@anthropic-ai/claude-code (dist-tags)
- https://claudeissues.com/issue/25978-variadic-allowedtools-option-consumes-positional-prompt-argument (mirror of anthropics/claude-code#25978)

**Svelte / SvelteKit / Vite (official)**
- https://svelte.dev/docs/kit/migrating-to-sveltekit-3 (through the official Svelte MCP docs server; also listed at https://next.svelte.dev/docs/kit/migrating-to-sveltekit-3)
- https://svelte.dev/docs/kit/single-page-apps
- https://svelte.dev/docs/kit/adapter-static
- https://svelte.dev/docs/kit/@sveltejs-kit-vite
- https://svelte.dev/docs/kit/page-options
- https://svelte.dev/docs/svelte/testing
- https://registry.npmjs.org/@sveltejs/kit · /@sveltejs/adapter-static · /@sveltejs/vite-plugin-svelte · /svelte · /svelte-check · /vite

**Tauri (official)**
- https://v2.tauri.app/start/frontend/sveltekit/
- https://v2.tauri.app/start/frontend/vite/
- https://v2.tauri.app/develop/tests/webdriver/
- https://v2.tauri.app/develop/tests/mocking/
- https://v2.tauri.app/security/csp/
- https://v2.tauri.app/security/capabilities/
- https://v2.tauri.app/learn/window-customization/
- https://v2.tauri.app/reference/config/
- https://crates.io/crates/tauri (API; source of `tauri-2.12.4.crate`: `src/test/mod.rs`, `src/ipc/channel.rs`, `src/path/desktop.rs`)
- https://crates.io/crates/tauri-utils (source of `tauri-utils-2.10.2.crate`: `src/config.rs`, `src/lib.rs`)
- https://crates.io/crates/tauri-plugin-shell (2.5.0 README) · https://crates.io/crates/tauri-plugin-opener · /tauri-plugin-dialog · /tauri-plugin-clipboard-manager · /tauri-plugin-window-state · /tauri-plugin-single-instance
- https://registry.npmjs.org/@tauri-apps/api · /@tauri-apps/cli

**E2E**
- https://webdriver.io/docs/desktop-testing/tauri/crabnebula-setup
- https://webdriver.io/docs/desktop-testing/tauri/platform-support
- https://docs.rs/crate/tauri-plugin-wdio-webdriver/1.3.0/source/README.md
- https://crates.io/crates/tauri-plugin-wdio-webdriver · https://registry.npmjs.org/@wdio/tauri-service · /webdriverio · /@playwright/test · /vitest · /@vitest/browser-playwright · /vitest-browser-svelte · /@testing-library/svelte

**PTY / terminal**
- https://crates.io/crates/portable-pty (API, reverse dependencies; source of `portable-pty-0.9.0.crate`)
- https://github.com/wezterm/wezterm (git history of `pty/`, read-only clone)
- https://crates.io/crates/tauri-plugin-pty · https://github.com/Tnze/tauri-plugin-pty (read-only clone) · https://registry.npmjs.org/tauri-pty
- https://registry.npmjs.org/@xterm/xterm · /@xterm/addon-fit · /@xterm/addon-webgl · /@xterm/addon-canvas · /@xterm/addon-unicode11
- https://flaviocopes.com/macos-app-command-line-tools-path

**Markdown, watching, Rust test**
- https://crates.io/crates/comrak · https://crates.io/crates/pulldown-cmark (API, reverse dependencies, version history)
- https://crates.io/crates/notify · https://crates.io/crates/notify-debouncer-full (API, dependencies)
- https://crates.io/crates/proptest · https://crates.io/crates/tokio · https://crates.io/crates/window-vibrancy · https://crates.io/crates/markdown

**TypeScript / lint / format / UI**
- https://registry.npmjs.org/typescript · /@typescript/typescript6
- https://infoq.com/news/2026/08/typescript-7-released/
- https://registry.npmjs.org/typescript-eslint · /eslint · /eslint-plugin-svelte · /prettier · /prettier-plugin-svelte
- https://registry.npmjs.org/bits-ui (2.19.5 tarball: README, `dist/bits/` listing)
- https://registry.npmjs.org/pnpm

**Project inputs**
- `docs/spec-input/01-master-spec.md`, `docs/spec-input/00-app-brief.md`, `docs/spec-input/claude-build-blueprint.md` (v2.1)
