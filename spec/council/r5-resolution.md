# Round 5 · Chair · Resolution

Chair: Opus 5.5 (xhigh) · Date: 2026-10-19
Input: `r4-redteam.md` (1 fatal, 13 serious, 12 minor), against `r3-synthesis.md`.

**Amended by `r5b-freeze-review.md`:** the phase-1 scope, estimate and kill-check timing (§3), `auto` items (moved to v1.1), the one remaining `claude -p` call (removed), the config fingerprint, path policy, signing defaults and probes. Where they differ, r5b and the frozen Master Spec win.

Rule from prompt 2.2: every fatal and serious objection is answered, by changing the plan or by citing evidence that defeats it. Every minor objection gets a decision too. Nothing is deferred as "open".

**Result: all 26 objections accepted.** None was defeated by counter-evidence. The red team's citations were spot-checked against the CLI reference on 2026-10-19: `--restricted` (v2.1.248+, "loads only managed settings and `--settings`"), `--init-only` ("Run Setup and SessionStart hooks, then exit without starting a conversation"), `--strict-mcp-config`, `--agents` file form with `-p` (v2.1.281+), `--fork-session`, `--setting-sources`, `--desktop --resume <session ID>`, and the sentence "`claude --help` does not list every flag". All confirmed ([cli-reference](https://code.claude.com/docs/en/cli-reference)).

---

## 1. Decisions

| ID | Sev | Decision | What changes |
|---|---|---|---|
| R-01 | Fatal | **Accept.** Combined with R-14: **headless Gate A leaves v1.** | v1 has no `claude -p` gate. When headless Gate A ships in v1.1, it must use the hardened argv (§2.1) and pass S-2′ first. The phrase "read-only by construction" is struck from every document. Running approved verify commands executes repository code; that goes in the risk register. |
| R-02 | Serious | **Accept all five fixes.** | Approval hashes for decision files live in app data; any outside change opens a **Pending outside change** review (Accept / Reject, with a diff). The verify-command list is a security object with a linter (§2.3). `action` destinations are hard-coded in Rust. Every launch's `--settings` denies `Read(./.foreman/**)`. |
| R-03 | Serious | **Accept.** | Preflight fingerprints the step cwd's Claude config (`.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json`, `.claude/agents/**`, `CLAUDE.md`). A change needs one acknowledgement with the diff. Auth-changing keys (`apiKeyHelper`, `env` entries `ANTHROPIC_*` or `CLAUDE_CODE_USE_*`, `awsAuthRefresh`, `awsCredentialExport`, `gcpAuthRefresh`) **block** the launch. `claude auth status` runs in the step's cwd with the launch environment, cached against the fingerprint. |
| R-04 | Serious | **Accept.** | New build order with a thin end-to-end version and the kill check at about 7 weeks (§3). A by-hand baseline is recorded starting with Foreman's own Stage 5 (`docs/metrics-baseline.md`). Estimate restated (§3.3). |
| R-05 | Serious | **Accept.** | **Annotations are the only source** of every field Foreman acts on. Inference is a lint: it warns and never supplies values. A step without an annotation is "not runnable: needs annotation". P-01 drops from L to M. Blueprint proposal B-1 updated: every step in the v1 coverage list gets a full `fm:step`. |
| R-06 | Serious | **Accept.** | Inspection items get explicit IDs in `fm:verify`, bound to bullets by a normalized text hash in the lock. A changed hash means "re-confirm the kind". Step IDs come from the explicit `id=`. A golden test swaps two bullets and expects an error. |
| R-07 | Serious | **Accept the stronger fix.** | `auto` means **Foreman runs the command itself**: argv, no shell, cwd = repo, a timeout, exit code and output tail recorded as the evidence. `auto` requires `cmd=<verify-id>`. No LLM judges an `auto` item in v1. "Fresh clone works" is never `auto` in place; it may be a Foreman-run command in a temporary `git worktree` (a Stage 4 design decision), otherwise attest. |
| R-08 | Serious | **Accept.** | Idle = last event `Stop` and no outstanding subagent or background task (sensor adds `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, `PostToolUse` for background Bash). End a session with `Ctrl-C` then `/exit`, and wait for `SessionEnd` with reason `prompt_input_exit`. Never SIGTERM a session that isn't idle without explicit confirmation. `--session-id` only at chain start, never with `--resume`. "Open in Desktop" marks the chain `handed-off`; the next Foreman step forks (`--fork-session`) unless Billy confirms Desktop is closed. The permission-reset cost is shown after every re-arm. **Until the sensor exists (phase 2), re-arm asks Billy to confirm the session is idle.** |
| R-09 | Serious | **Accept.** | Preflight checks the transcript file exists. Doctor shows the effective `cleanupPeriodDays` and recommends a longer value (Foreman doesn't write it). New chain state `broken` with **Start a fresh chain with context** (the last signed packet plus the step's declared outputs). Session UUIDs in provenance are links, not evidence. |
| R-10 | Serious | **Accept.** | After every signing commit, read back with `git show HEAD:<path>` and compare hashes. "Skip repo hooks for signing commits" is on by default (these commits touch only packets, attachments and `.foreman/`); `--no-verify` is recorded in provenance. "Sign on new branch" uses plumbing (temporary index, `commit-tree`, `update-ref`) and never touches HEAD or the working tree. From Stage 5 on, decision changes stay pending in app data until the signing commit. Stage records are read ref-aware. A change to a signed packet after signing is a tamper event: the committed version wins and Today shows the diff. |
| R-11 | Serious | **Accept.** | One Rust path policy for every write: repo-relative only, no `..`, no absolute paths, a deny list (`.git/`, `.claude/` except the hard-coded `action` vocabulary, `.husky/`, `.github/workflows/`), and component-wise symlink refusal through a directory-handle API. The crate choice (`cap-std` or `openat`-style calls with `O_NOFOLLOW`) is an ADR in Stage 4.2, with its version verified then. |
| R-12 | Serious | **Accept.** | `claude -v` on every launch (no 24 h cache). On a version change, zero-token **contract probes** run: `claude --init-only --settings <sensor settings>` proves hooks still fire; an argv-parse probe per flag Foreman uses. A probe failure disables only the affected feature, with a precise banner. Every `-p` call Foreman makes (packet capture) checks for an auth failure in its own output and falls back to paste-back. S-13 is reworked as these probes and moves into v1. |
| R-13 | Serious | **Accept.** | **S-0, PTY and TUI fidelity, runs first**, in Stage 5.3 (the ADR reality check), before the template core. Its pass conditions are in §4. |
| R-14 | Serious | **Accept.** | v1 Gate A: interactive (as the blueprint says), **Import results**, attest, Foreman-run `auto` commands (R-07), and the A → B → C → Sign order. Headless Gate A moves to v1.1 with the Crew Loop. |
| R-15 | Minor | **Accept.** | Attest outcomes: `done (note)` or `not applicable (reason)`, recorded under ATTESTED with distinct labels. Notes ≥ 15 characters, not repeated within a gate, and can link an artifact. The item's "how to check" text sits next to the checkbox. |
| R-16 | Minor | **Accept all.** | Hook exec form with `args`; `disableAllHooks: false` in run settings; sensor copied to `<app-data>/bin/foreman-sensor-<hash>`; a per-run token checked on ingest; retention limit (last 20 events per run, deleted at stage sign); one file per event, atomic create; **fallback 1 dropped** (Foreman never edits `~/.claude/settings.json`). |
| R-17 | Minor | **Accept.** | S-3 tests `--effort` on resume first, then `--settings '{"effortLevel":"<x>"}'`. If both fail, Foreman shows the `/effort` line for Billy to type and warns that it saves as his default for that model. Foreman never types `/effort` itself. |
| R-18 | Minor | **Accept.** | Stage records list **intended** (argv) and **observed** (model from the sensor's `SessionStart` input or the `system/init` event; fallback notices; agent models from the pinned kit). |
| R-19 | Minor | **Accept.** | The Doctor floor is computed from steps and the pinned kit's agent frontmatter. With the kit as it stands, the floor is **2.1.293**, above npm `stable` (2.1.292); Doctor says so and recommends the `latest` channel. |
| R-20 | Minor | **Accept.** | The argv builder rejects a prompt starting with `-`, `/` or `!`. The fake-`claude` test asserts the positional argument for every step in the golden fixture. |
| R-21 | Minor | **Accept, with a simpler lock.** | The repo keeps only `.foreman/blueprint.lock.json`: blueprint repo commit, file SHA-256, grammar version, blueprint version. The full text lives in app data, keyed by hash, and can always be recovered from the blueprint repo at that commit. No 100 KB lock file in the repo. The `Read(./.foreman/**)` deny stays. |
| R-22 | Minor | **Accept.** | Temp files named `.<name>.foreman-tmp` in the same directory, added to `.git/info/exclude`, ignored by the watcher; `sync_all` on the file, fsync the directory around the rename; `tempfile::persist`; crash-injection proptest. Whether `sync_all` issues `F_FULLFSYNC` on macOS is checked in Stage 4.2 (ADR on durable writes) against the Rust std source. |
| R-23 | Minor | **Accept.** | On each re-arm the old xterm buffer is archived as a collapsed read-only section and a clean buffer starts. Re-arm latency is measured in S-8; a "Resuming 2.3…" state shows until the TUI is ready. |
| R-24 | Minor | **Accept.** | Reserve only the `adopted` enum value. No speculative structures. v1.1 gets its own metric and kill criterion (§3.4). |
| R-25 | Minor | **Accept.** | Pass `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` through when present. Doctor offers a one-time, consented **Capture my login shell PATH** (`$SHELL -l -c` printing PATH, shown to Billy, stored). S-12 runs with S-0 in Stage 5. |
| R-26 | Minor | **Accept.** | `<!-- fm:grammar v=1 -->` at the top of the blueprint. Unknown keys: errors within a version, warnings across versions. A `session=continue` step inherits its chain's cwd; declaring another cwd is an error. |

## 2. Revised designs

### 2.1 Hardened headless argv (v1.1; recorded now so the design is complete)

```text
claude -p --restricted
  --tools "Read,Grep,Glob,Agent"
  --strict-mcp-config --disallowedTools "mcp__*"
  --agents <app-data>/runs/<id>/agents.json
  --permission-mode dontAsk --permission-prompts none
  --allowedTools Read Grep Glob
  --max-turns <n> --settings <run settings>
  --model <m> --effort <e> --session-id <uuid>
  --output-format stream-json --verbose --json-schema <schema> "<prompt>"
```

Bash is **not** in the headless tool list, because `auto` items are run by Foreman itself (R-07). The LLM gate reads files and judges only items that need reading. That removes most of R-01's attack surface by construction. S-2′ (hostile-repo fixture) must pass before v1.1 ships it. If `--restricted` doesn't load `--agents`, the gate runs without subagents.

### 2.2 Interactive launch argv (v1)

```text
claude --session-id <uuid>            # chain start only
       | --resume <uuid> [--fork-session]   # continuing step
       --model <alias> --effort <level> --permission-mode <mode>
       --name fm-<project>-s<NN>-<step>
       --settings <app-data>/runs/<runId>/settings.json
       "<resolved prompt>"
```

Run settings contain: the sensor hooks (exec form, per-run token), `disableAllHooks: false`, and `permissions.deny: ["Read(./.foreman/**)"]`. Child environment: an explicit allowlist (HOME, USER, LOGNAME, SHELL, TMPDIR, LANG, LC_*, TERM=xterm-256color, COLORTERM=truecolor, PATH built per §2.4, `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` when present, `FOREMAN_RUN_ID`). Every `ANTHROPIC_*`, `CLAUDE_CODE_USE_*`, AWS, GCP and Azure variable is removed. Never `--bare`, never `bypassPermissions`, never `--dangerously-skip-permissions`.

### 2.3 Verify-command policy

A verify command is `{ id, argv: [program, ...args], cwd: "repo" | "spec", timeoutSec }`, stored in `.foreman/project.json`, approved in Foreman's UI, with the approval hash in app data. The linter rejects:
- shells and interpreters as the program (`sh`, `bash`, `zsh`, `fish`, `node`, `python*`, `ruby`, `perl`, `bun`, `deno`);
- generic runners (`pnpm exec`, `pnpm dlx`, `npx`, `npm exec`, `yarn dlx`);
- network tools (`curl`, `wget`, `ssh`, `scp`, `nc`);
- git write subcommands (`push`, `commit`, `reset`, `checkout`, `switch`, `rebase`, `merge`, `clean`, `stash`);
- any argument containing a glob or shell metacharacter.

Allowed: named package scripts (`pnpm run <script>` where `<script>` exists in `package.json` at approval time, and the script's text is shown to Billy and included in the approval hash), `cargo test|clippy|fmt --check|build`, `git status|diff|log|show`. A change to a referenced `package.json` script invalidates the approval.

### 2.4 PATH

Built from `/etc/paths`, `/etc/paths.d/*`, Homebrew (`/opt/homebrew/bin`, `/opt/homebrew/sbin`), `~/.cargo/bin`, the pnpm home, and the stored login-shell PATH if Billy captured it (R-25). Shown in Doctor. S-12 confirms `pnpm`, `cargo`, `node` and `claude` resolve from a Finder-launched build.

## 3. Build order and estimate

### 3.1 Phases

| Phase | Contents | Ends with |
|---|---|---|
| **0 · Prove the risky parts** (Stage 5) | Scaffold; S-0 (PTY + xterm 6 + real TUI, memory, VoiceOver, ⌘-routing); S-1′ as an `--init-only` probe; S-3; S-7′; S-8; S-12; S-23 | Spike reports in `docs/spikes/`; each picks a branch of its decision rule (§4) |
| **1 · Thin end-to-end** | Annotation-only parser for Stages 1–2, the Stage Gate and F.2; IDs and the lock; resolver for the tokens those steps use; minimal project setup; chat bridge (copy, paste-back, validate, save); CLI launch with the exact argv; re-arm with manual idle confirmation; path policy; approval hashes; pathspec signing with read-back; run log with P-18 timings | **TradeLog Stages 1–2 run through Foreman. Kill check** against the baseline. |
| **2 · Complete v1** (only if the kill check passes) | Sensor (P-06 as amended); Today and Do next; Doctor with contract probes; wizard and conditions; annotations for Stages 3–5; inference lint; Template health and the TradeLog harness; Foreman-run `auto` commands; interactive Gate A with Import results and attest outcomes; Desktop handoff; minimal Add; Knowledge manifest and assembler; metric panel UI | **TradeLog Stages 3–5 run through Foreman.** Success metrics read. |
| **v1.1** | Headless Gate A (hardened, after S-2′); Crew Loop; CR track; full overlay authoring; usage meter; live send; calibration bundle | v1.1 metric (§3.4) |

### 3.2 Baseline

Starting with Foreman's own Stage 5 on Billy's Mac, every step run by hand is logged in `docs/metrics-baseline.md`: the time from deciding to run the step to the prompt being sent, the number of manual edits to the prompt, and any mistake (wrong flag, wrong packet, missed step). The kill check compares Foreman's phase-1 numbers for the same measures against this.

### 3.3 Estimate

- Phase 0: about 1 week.
- Phase 1: 5–7 weeks. **Kill check at about 6–8 weeks.**
- Phase 2: 7–12 weeks.
- Total v1: 13–20 weeks of working time, plus Foreman's own Stage 6–17 ceremony, which is included in these figures because each phase is built through the blueprint's stages. Calendar time is longer: Billy runs several products.

### 3.4 v1.1 metric and kill criterion

- **Metric:** one TradeLog milestone runs through the CLI Crew Loop with zero manual routing (no hand-typed model, effort or mode changes) and zero lost ticket state.
- **Kill criterion:** if, over that milestone, a ticket takes longer to route through Foreman's loop than through the blueprint's Desktop loop by hand, simplify the loop to a launcher (prompt + argv per loop step) and drop the automation.

## 4. Spikes: decision rules

None of these can run in this cloud session (they need a logged-in `claude` on Billy's Mac). Each has both branches fully specified, so the spec freezes with no open questions.

| ID | Runs in | Pass → | Fail → |
|---|---|---|---|
| S-0 PTY + TUI fidelity, memory, VoiceOver | Stage 5.3 | xterm 6 + portable-pty as specified | Memory over 300 MB with 3 tabs: cap scrollback at 5,000 lines and archive on re-arm; if still over, raise the NFR to the measured value + 20% and record it. Shift+Tab or ⌘ routing broken: a key-routing layer that captures app chords before xterm. VoiceOver unusable: `--ax-screen-reader` launches when VoiceOver is on (v2.1.181+). A fundamental TUI failure (rendering or input unusable): stop and reopen the shell decision with SwiftUI + SwiftTerm, per r7. |
| S-1′ hooks via `--settings` | Stage 5.3 (zero-token `--init-only`) | Sensor as specified | No sensor; every attention state comes from files and manual idle confirmation (the phase-1 behavior stays permanent). |
| S-3 effort on resume | Stage 5.3 | `--effort` on resume | `--settings '{"effortLevel":…}'` on resume; else show the `/effort` line for Billy to type, with the warning (R-17). |
| S-7′ settings changing auth | Stage 5.3 | The R-03 controls are confirmed sufficient | Additionally refuse to launch in any cwd whose project settings set an `env` block at all, until Billy approves that block. |
| S-8 graceful end, background work, `--session-id` + `--resume`, latency | Stage 5.3 | Re-arm as specified | If `/exit` can't be made safe: re-arm asks Billy to end the session himself, then relaunches. |
| S-12 PATH | Stage 5.3 | PATH as specified | Login-shell PATH capture becomes mandatory at first run. |
| S-23 Desktop resume shares the transcript | Stage 5.3 | Fork rule as specified | Desktop handoff opens the session with `--fork-session` semantics only (new chain); Foreman never continues a chain after handing it to Desktop. |
| S-2′ hostile-repo headless gate | Before v1.1 | Headless Gate A ships | Headless Gate A doesn't ship; interactive Gate A stays. |

## 5. Stack: the DRI decision

Billy was asked to choose between SvelteKit 3 SPA and plain Svelte 5 + Vite (r7 PART 2) and answered **no preference**. His standing instruction is that the evidence-backed call should be made rather than asked. The chair therefore adopts the r7 recommendation:

**Frontend: Svelte 5.57.2 + Vite 8.3.4 + an in-house typed hash router.** Reasons: an SPA in Tauri uses none of SvelteKit's server features; the measured bundle is 2.6× smaller; there's no inline script, so the CSP is literally `script-src 'self'`; there are fewer config surfaces; and there's no dependency on an 18-day-old major whose Tauri guide is stale. What's given up (file routing, layouts, `load`) barely matters for six screens fed by Tauri commands.

All other layers follow r7 PART 3 with the pinned versions there. Billy can override this at the Stage 2 sign; the reason is recorded in `r7-stack.md` either way.

## 6. Effect on the r3 decisions

| r3 item | Now |
|---|---|
| P-01 | Annotations only; inference is a lint; size M |
| P-02 | Explicit step and item IDs; text hashes; lock in repo is metadata only |
| P-05 | As amended by R-08, R-09, R-23; manual idle confirmation until the sensor |
| P-06 | Phase 2; amended by R-16; fallback 1 dropped |
| P-07 | §2.2, §2.4; R-03 controls; R-20 check |
| P-13 | R-10 fixes |
| P-14 | v1: interactive Gate A + Foreman-run `auto` + attest outcomes. Headless → v1.1 |
| P-16 | Doctor gains contract probes (R-12), floor from agents (R-19), cleanup period (R-09), PATH capture (R-25) |
| P-17 | Approval hashes (R-02); pending decision writes from Stage 5 (R-10); atomic-write details (R-22) |
| P-23 | `handed-off` chain state and fork rule (R-08) |
| Reserved data shapes | Only `adopted` |
| Build order and estimate | §3 |
