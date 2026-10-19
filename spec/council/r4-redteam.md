# Round 4 · Red team · Objections

Author: red-team (Stage 2 council) · Date: 2026-10-19
Target: `r3-synthesis.md` (the chair's decisions), plus the r2 proposals it accepted. Evidence: r1-engineering, r1-workflow, the App Brief (`docs/handoff/stage-01.md`), Master Spec v0.9, blueprint v2.1.

**Method.** I attacked every decision in r3, then re-checked each Claude Code claim the design depends on against code.claude.com on 2026-10-19. Each objection gives the evidence and what would settle it. Anything I couldn't confirm in an official source is marked **UNVERIFIED**.

**Severity.**
- **Fatal:** as written, the decision breaks a hard requirement in the brief (Max plan only, "zero gates skipped by accident", SR-01 to SR-09) or can't work at all. It must change before the spec freezes.
- **Serious:** it will cost weeks, corrupt data or let a gate lie in realistic use. Fix it in Round 5.
- **Minor:** a real defect with a cheap fix. It can be a Stage 4 acceptance criterion.

---

## 1. Summary

| ID | Target | Sev | Objection |
|---|---|---|---|
| R-01 | P-14 headless Gate A argv; r1 A5 "read-only by construction"; r3 E3 | **Fatal** | A `claude -p` run doesn't stay inside the allowlist Foreman passes. It runs the repo's hooks, `env` and `apiKeyHelper`, and connects its `.mcp.json` servers, all without a trust prompt. It merges Billy's user allow rules (which include `Bash(pnpm exec *)`) and untracked local ones. Subagents run in their own `permissionMode`. Hooks can approve calls. So the run isn't locked down, and a repo can even switch billing. |
| R-02 | r3 E9, P-17: decision state in the repo's `.foreman/` | Serious | Claude can edit `.foreman/` files (they aren't a protected path), and so can git checkouts. "External edits always win" with a reload toast means the verify allowlist, conditions, skips, `fm:verify` kinds in `blueprint.lock.md`, and `action` destinations can all change without Billy approving the change. |
| R-03 | P-07 env scrub and auth guard (interactive launches) | Serious | Removing variables from the child environment doesn't stop a repo's settings `env` block or `apiKeyHelper` from changing auth. The auth check runs globally, not in the step's cwd with the launch environment. After Billy trusts a folder once, later changes to its Claude config apply silently. |
| R-04 | r3 §5 estimate and kill criterion | Serious | The kill criterion is checked after phase 3, when 16 of 18 proposals (about 84% of the sized work) are built, not "roughly halfway". There's also no measured by-hand baseline to compare against. |
| R-05 | P-01: inference plus a drift detector that treats disagreement as an error | Serious | Two sources for every field, with disagreement as a hard error, means a wrong inference can only be fixed with a Foreman code release. Retro edits can stop the template loading. Inference for Stages 6–18 is dead weight in v1. This layer, not the Markdown parse itself, is the template's real liability. |
| R-06 | P-02 positional item IDs; `fm:verify 1=auto 2=attest` | Serious | Item IDs and verification kinds are tied to bullet position. If a retro reorders two bullets, their kinds swap with no error (the item count is unchanged), so a human-only item can become `auto`. Attestations also point at a different item after a pin upgrade. |
| R-07 | P-14 `auto` items | Serious | Nothing ties a PASS to evidence. r2's own example tags "fresh clone … works" as `auto`, but that check can't be run under `dontAsk`. The blueprint's `test-runner` agent runs on Haiku at low effort, and Billy's `fallbackModel` can downgrade the main model too. False PASSes stay possible. |
| R-08 | P-05 re-arm | Serious | A `Stop` event doesn't mean nothing is running: background subagents and Bash can still be going, and the sensor doesn't watch for them. Typing `/exit` into a non-empty input box sends a prompt. Four behaviors re-arm relies on are undocumented. Opening the session in Desktop (v1 P-23) can make two writers interleave into one transcript. Repeated permission resets push Billy toward permanent allow rules, which makes R-01 worse. |
| R-09 | P-05, P-13, P-14, P-17: chains resumed later | Serious | Claude Code deletes transcripts after 30 days by default. A chain, Repair, Gate B/C or Resume Interrupted that comes back to an older session gets "No conversation found", and nothing in the design handles that. |
| R-10 | P-13 signing; P-17 commit timing | Serious | The repo's pre-commit hooks can reformat the packet during the signing commit, so the committed file no longer matches the signed hash. "Sign on new branch" needs a checkout under a live session. Before signing, `.foreman/` writes dirty the tree from Stage 5 on (P-17 says this can't happen). A stage record on an unmerged branch can't be seen from `main`. |
| R-11 | SR-04 versus repo-sourced paths (`out=`, `requires=`, REPO FILES, `write-blocks`, extra destinations) | Serious | No rules are set for paths that come from repo content: `..`, absolute paths, `.git/hooks`, `.claude/` and symlinked directories can all send Foreman's own writes outside the folders it's allowed to touch. |
| R-12 | Dependence on Claude Code versions (r3 §3.4, S-13) | Serious | The docs announce that `--bare` "will become the default for `-p` in a future release", and bare mode doesn't use the subscription login. Claude Code updates itself, Doctor caches the version for 24 h, and S-13 compares against `--help`, which the docs say doesn't list every flag. A breaking update would surface as a failure in a terminal tab. |
| R-13 | r3 §1 spike list and build order | Serious | r1's top-risk spike (PTY + xterm 6 + the real Claude TUI inside WKWebView; "do this spike first") was dropped from the first Stage 6 tickets. |
| R-14 | P-14 scope in v1 | Serious | In the whole metric run, headless Gate A covers about two items (Stage 5, items 1 and 4), and one of those can't be run under `dontAsk`. A size-L feature delivers almost nothing for v1. |
| R-15 | P-14 attest default | Minor | Attest volume is low (3–5 items per stage), so rubber-stamping is less likely than feared. But there's no recorded "not applicable" outcome (`fm:if` per item is v1.1), so a non-server project has to attest "Staging deploys automatically" falsely. |
| R-16 | P-06 sensor implementation | Minor | The hook `command` runs through `sh -c` with an "Application Support" path that contains a space. A repo can set `disableAllHooks: true`. The sensor's path breaks when Billy rebuilds Foreman. Events are unauthenticated and keep assistant text with no retention limit. Fallback 1 edits `~/.claude/settings.json`. |
| R-17 | S-3 fallback (type `/effort` after relaunch) | Minor | An interactive `/effort` saves the level per model into Billy's user settings, so the fallback quietly changes his defaults. |
| R-18 | P-13 provenance | Minor | Provenance records the argv, not what actually ran. Aliases move to newer models, `fallbackModel` applies (Appendix A.1), Fable falls back automatically, and agents use their own models. |
| R-19 | r3 §3.4 Doctor floor | Minor | The floor counts only the models steps use. The kit's `Explore` and `test-runner` agents use `haiku`, so the real floor is always 2.1.293, which is above npm `stable` (2.1.292). |
| R-20 | P-07 argv builder | Minor | The final positional prompt isn't checked for a leading `-`, `/` or `!`. Combining `--session-id` with `--resume` isn't documented. |
| R-21 | P-02 `.foreman/blueprint.lock.md` in the repo | Minor | The lock file is committed, so Grep finds it. Claude can pick up the whole blueprint as context or "evidence" during gates and sessions. |
| R-22 | P-17 atomic writes | Minor | Temp files in repo folders show up to the watcher, `git status` and Claude's `git add -A`. fsync and cross-volume rename (EXDEV) aren't specified. |
| R-23 | P-05 and P-08 re-arm in the same tab | Minor | A resumed TUI re-renders the history, so the scrollback grows with every re-arm (memory NFR). The 1–3 s restart estimate is optimistic. |
| R-24 | r3 §5 data shapes reserved for v1.1 | Minor | The reserved shapes are guesses that will be wrong. `schemaVersion` migrations are cheap for one user. v1.1 also has no kill criterion of its own, even though the daily-use value (the Crew Loop) is there. |
| R-25 | P-07 PATH and environment | Minor | The allowlist drops `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR`. Version managers' tools are missed. Claude Code sources `~/.zshrc` for aliases and functions only, not exports, so a missing `node` can lead Claude to try installing tools. |
| R-26 | r3 §3.1 grammar | Minor | Unknown keys are hard errors but the grammar has no version, so a v1.1-annotated blueprint can't load in v1. Nothing checks that every step in a chain uses the same cwd. |

**Fatal: 1 · Serious: 13 · Minor: 12.**

**Answers to the chair's six questions (r3 §6), in short:**

1. Re-arm is acceptable only with the changes in R-08 and R-09.
2. Per-launch `--settings` hooks are fine (see "What survived"). The risk lies elsewhere: the repo's own settings (R-01, R-03).
3. Yes, a repo can make Foreman run things Billy didn't intend: R-01, R-02, R-11.
4. The Markdown parses reliably. The liability is the inference layer and the positional IDs (R-05, R-06).
5. The scope is realistic only if the build order changes so the kill check comes early (R-04, R-13, R-14).
6. Yes, the integration rests on undocumented behavior, and one breaking change is already announced (R-08, R-12).

---

## 2. Objections in detail

### R-01 · Headless Gate A isn't locked down · **Fatal**

**Target.** The P-14 argv as accepted in r3:
`claude -p --session-id … --permission-mode dontAsk --permission-prompts none --allowedTools Read Grep Glob "Bash(<verify>)" --output-format stream-json --verbose --json-schema … --settings <run settings> "<prompt>"`
It rests on r1 A5's claim that the run is "**read-only by construction**" and "bounded to the allowlist".

**The attack.** The run's cwd is the project repo, and Claude has been writing to that repo for five stages. Six documented mechanisms widen what a `-p` run in that repo can do:

1. **Repo hooks, `env` and helpers run without trust.** From the headless docs: *"Without `--bare`, a `-p` session runs the hooks in a project's `.claude/settings.json` and connects the servers in its `.mcp.json`, even in a folder you've never trusted. A `-p` session shows no workspace trust dialog and no per-server approval prompt."* The permissions page has a table of what applies under "`claude -p` or the SDK, folder never trusted". For "Hooks in settings files, the `env` block and helper commands such as `apiKeyHelper`" it says **Used**. For `.mcp.json` servers it says **"Connected without asking, approved or not"**. The settings index scopes `apiKeyHelper` to "Any file", which includes the project file. So a repo can run arbitrary commands at `SessionStart` and can swap the credential in a run Foreman calls "headless and read-only". That breaks the brief's "never needs an API key".
2. **Hooks can approve tool calls.** `dontAsk` runs *"actions matching your `permissions.allow` rules and calls approved by a PreToolUse hook"* (permission-modes). `--permission-prompts none` denies a request *"unless a `PermissionRequest` hook allows it"* (headless). Repo hooks are loaded (item 1), so a repo hook can approve anything.
3. **Allow lists merge across scopes.** *"When you set the same list key, such as `permissions.allow`, in more than one file, Claude Code combines the lists"* (settings). Billy's own `~/.claude/settings.json` (blueprint Appendix A.1, which Doctor checks for) allows `Bash(pnpm exec *)`, `Bash(pnpm run *)`, `Bash(pnpm install)` and `Bash(cargo build *)`. `pnpm exec <anything>` runs any program. So the "exact allowlist" already includes a general program runner.
4. **An untracked `.claude/settings.local.json` needs no trust.** Its allow rules apply once Claude Code has checked it's untracked, and a `-p` session *"counts as accepted"* for that check (permissions, "When your local settings file needs trust"). In interactive sessions, every "Yes, and don't ask again" Billy clicks writes a rule into that file. Re-arm resets session grants (R-08), so he'll click it often.
5. **Subagents use their own permission mode.** *"When the main conversation is in `default`, `dontAsk`, or `plan` mode, the subagent runs in the permission mode you set"* (sub-agents; the page words the exceptions ambiguously). Gate A tells Claude to "use the test-runner agent for every automated check". A project-scoped `test-runner.md` that sets `permissionMode: acceptEdits` would make that subagent edit files during a read-only gate. Project agents are loaded in `-p`; only their *frontmatter hooks* wait for trust.
6. **`--tools` is missing.** r1 A5 passed `--tools Read,Grep,Glob,Bash,Agent`; the accepted P-14 argv dropped it. Without it, Edit and Write are available, and any allow rule from items 3–4 can approve them.

**Evidence.**
- [headless](https://code.claude.com/docs/en/headless), "Start faster with bare mode", "Auto-approve tools", "Turn off permission prompts".
- [permissions](https://code.claude.com/docs/en/permissions), "What runs before you trust a folder" and "When your local settings file needs trust".
- [permission-modes](https://code.claude.com/docs/en/permission-modes), "Allow only pre-approved tools with dontAsk mode".
- [settings](https://code.claude.com/docs/en/settings), "Lists merge instead of overriding".
- [settings-reference](https://code.claude.com/docs/en/settings-reference) index (`apiKeyHelper` scope "Any file").
- [sub-agents](https://code.claude.com/docs/en/sub-agents), permission modes.
- `claude-build-blueprint.md` Appendix A.1, lines 1821–1847.

**What settles it.** A revised S-2, run with a real `claude` before the freeze, in a throwaway repo seeded with:
- a `SessionStart` hook that touches a marker file;
- a `PreToolUse` hook that allows everything;
- an untracked `settings.local.json` with `Bash(*)`;
- a project `test-runner.md` with `permissionMode: acceptEdits`;
- a `.mcp.json` server;
- an `apiKeyHelper`.

Run the r3 argv, then the hardened argv below. Pass condition for the hardened argv: no marker file, no MCP connection, `apiKeySource`/auth unchanged in the `system/init` event (field name **UNVERIFIED**), every non-listed command denied, and `test-runner` unable to edit.

**Fix.** Rebuild the headless argv so Claude Code loads only what Foreman passes:

```text
claude -p --restricted
  --tools "Read,Grep,Glob,Bash,Agent"
  --strict-mcp-config --disallowedTools "mcp__*"
  --agents <app-data>/runs/<id>/agents.json      # test-runner etc., from the pinned kit
  --permission-mode dontAsk --permission-prompts none
  --allowedTools Read Grep Glob "Bash(<approved verify 1>)" …
  --max-turns <n> --settings <run settings>
  --model … --effort … --session-id <uuid>
  --output-format stream-json --verbose --json-schema <schema> "<prompt>"
```

`--restricted` (v2.1.248+, under the floor) *"loads only managed settings and `--settings`"*, *"removes the built-in tools that run commands or code … unless you name them individually in `--tools`"*, and *"confines the built-in file tools to the working directories"* ([cli-reference](https://code.claude.com/docs/en/cli-reference)). It is the documented mode for exactly this case.
- **UNVERIFIED:** whether `--restricted` still loads user agents and project `.claude/agents/`. Passing `--agents` from the pinned kit (file form, v2.1.281+) removes the dependency either way.
- **Fallback if `--restricted` misbehaves:** `--setting-sources user`, plus `--settings '{"disableAllHooks": true}'` (the stream already gives progress, so the sensor isn't needed headless), plus a Doctor check that refuses headless gates while Billy's user allow list contains broad runner rules (`pnpm exec *`, `pnpm dlx *`, `npx *`, `Bash(*)`, interpreters).

Record the hardened argv in the ADR and correct the wording "read-only by construction". With approved verify commands such as `pnpm test`, the run executes repository code; that's inherent in running tests and goes in the risk register.

---

### R-02 · Repo-resident decision state can be changed silently · Serious

**Target.** r3 E9 ("`.foreman/project.json` is the only allowlist source") and P-17 ("Changes are reloaded with a 'Reloaded from disk' toast"; Master Spec §7, "External edits always win").

**The attack.** These files decide which gates run and what the headless run may execute:
- `.foreman/project.json`: verify commands and conditions;
- `.foreman/overlay.json`: skips and added items;
- `.foreman/blueprint.lock.md`: every `fm:verify` kind, `cwd`, `start` mode, and `action` destination.

Claude can edit all of them. `.foreman` isn't in Claude Code's protected-path list (`.git`, `.claude`, `.husky`, `.vscode` and others; [permission-modes, Protected paths](https://code.claude.com/docs/en/permission-modes)), so `acceptEdits` and `auto` sessions edit them without asking. Plausible routes, none malicious:
- Claude "helpfully" adds `Bash(pnpm exec *)` to the verify commands so a check can run;
- a prompt-injected dependency README tells Claude to;
- `git checkout` or `git stash pop` brings in an older `.foreman/`.

Foreman then reloads and **accepts** the change. A changed `fm:verify 3=auto` in `blueprint.lock.md` turns "a rollback has actually been run once" into something the headless run can PASS. A changed `action` destination makes Foreman's own Rust write a council "agent" into `.claude/settings.json`; Claude's protected-path rules don't apply to Foreman's writes. SR-07 only highlights overlay *edits to prompts or run config*, not skips, conditions, verify commands or verify kinds.

**Evidence.**
- Protected-path list (above).
- P-17 reload rule.
- Master Spec §7.
- `blueprint.lock.json` stores the SHA-256 of `blueprint.lock.md` *in the same repo*, so editing both defeats it.

**What settles it.** A test: edit each file from a Claude `acceptEdits` session, then check whether Foreman's effective template, conditions, skips or allowlist changed without a confirmation step.

**Fix.**
1. **Pin the approved state in app data.** Foreman keeps a hash of the last state Billy approved for `project.json`, `overlay.json` and `blueprint.lock.*`. Any outside change to a decision field opens a **Pending outside change** review: a diff with Accept or Reject. It is never reloaded silently. Read-only fields (variables) can still reload with a toast.
2. **Verify the lock against its source.** At open, check `blueprint.lock.md` against `git show <sha>:claude-build-blueprint.md` in the blueprint repo when it's available, and always against a hash stored in app data.
3. **Treat the verify-command list as a security object.** Approve it in Foreman's UI, store the approval hash in app data, and lint it. Reject a bare `Bash`, a `*` before the subcommand, interpreters and shells (`sh`, `bash`, `zsh`, `node`, `python`, `bun`, `deno`), generic runners (`pnpm exec`, `pnpm dlx`, `npx`, `npm exec`), network tools, and git write subcommands. r1 E9's choice of source stands; the trust model for it changes.
4. **Hard-code `action` destinations in Rust** (`spec/.claude/agents/<known names>.md`, `spec/01-app-brief.md`). Never take them from template text.
5. **Defense in depth.** Every Foreman launch's `--settings` adds `permissions.deny: ["Read(./.foreman/**)"]`. A Read deny *"also blocks the Edit and Write tools on the same path"* (v2.1.208+ for edits, v2.1.228+ for writes; [tools-reference](https://code.claude.com/docs/en/tools-reference)). It isn't a boundary against Bash, so item 1 remains the real control. This also fixes R-21.

---

### R-03 · Env scrub and auth guard can be bypassed through settings · Serious

**Target.** P-07 steps 2–3: "cached `claude auth status` must show `claude.ai` or `oauth_token`", plus the env allowlist and denylist.

**The attack.** The denylist removes `ANTHROPIC_API_KEY` from the child *process* environment. Settings files can put it back:
- a settings `env` block *"is an ordinary key and follows the levels"*;
- after trust, *"most `env` values"* from the project file apply ([settings](https://code.claude.com/docs/en/settings));
- `apiKeyHelper` can be set in "Any file".

Doctor runs `claude auth status` once, globally, and caches it for 24 hours. It never sees the effective auth of a launch in `/path/to/tradelog/spec` with that folder's settings. After Billy accepts the trust dialog once per repo, later changes to that repo's `.claude/settings.json` hooks, `env`, `.mcp.json` or helpers apply in every future session with no prompt. Blueprint step 5.4 itself has Claude write `.claude/settings.json` hooks, so these files do change.

Which variables Claude Code ignores in a project `env` block is **UNVERIFIED**: that section of settings-reference was cut off when fetched.

**What settles it.** Spike S-7′: set `ANTHROPIC_BASE_URL` and `apiKeyHelper` in a trusted repo's `.claude/settings.json`, launch through Foreman's pipeline, and check `claude auth status` (run in that cwd) and the `system/init` metadata.

**Fix.**
- **Config fingerprint at preflight.** Hash the step cwd's `.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json`, `.claude/agents/**` and `CLAUDE.md`. If anything changed since Billy last launched in that cwd, show the diff as a banner that needs one acknowledgement.
- **Hard-block keys that change auth.** Block when those files set `apiKeyHelper`, `ANTHROPIC_*` or `CLAUDE_CODE_USE_*` in `env`, `awsAuthRefresh`, `awsCredentialExport` or `gcpAuthRefresh`.
- **Check auth where the step runs.** Run `claude auth status` in the step's cwd, with the exact launch environment, cached against the fingerprint rather than for 24 hours.

---

### R-04 · The kill criterion comes at about 84% of the build, with no baseline · Serious

**Target.** r3 §5: "The kill criterion is checked after Stage 2 of the TradeLog run, roughly halfway, so the investment is bounded."

**The attack.** Phase sizes from r2 §8.2, at r2's own upper bounds (S ≤ 2 days, M ≤ 1 week, L ≤ 3 weeks):

| Phase | Proposals | Upper bound |
|---|---|---|
| 1 Template core | P-01 L, P-02 M, P-03 S, P-09 M | 5.4 wk |
| 2 Chat path | P-16 L, P-10 M, P-11 L, P-13 M | 8.0 wk |
| 3 CLI path, then **kill check** | P-04, P-05, P-06, P-07, P-08, P-15, P-17 (M each), P-18 S | 7.4 wk |
| 4 Finish | P-12 M, P-14 L | 4.0 wk |

The kill check comes after 20.8 of 24.8 weeks: **84%**, with 16 of 18 proposals built. That doesn't bound the investment.

The criterion also compares against "running the blueprint by hand", but no by-hand time is recorded anywhere. P-18 measures only Foreman's side. On top of that:
- Billy runs six products, so calendar time will be longer than working time;
- the estimate leaves out the blueprint ceremony for Foreman's own Stages 6–17 (gates, audits, CO).

**What settles it.** The arithmetic above, plus a decision on what the minimum is that lets TradeLog Stages 1–2 run through Foreman.

**Fix.**
1. **Record a baseline now.** Billy is running Foreman's own Stages 2–5 by hand, which is the perfect baseline. Log each step's time from "decide to run the step" to "prompt sent", and each stage's total elapsed time, in `docs/metrics-baseline.md` (or from git timestamps).
2. **Build a thin end-to-end version for the kill check (about 5–7 weeks):**
   - a parser that reads annotations only (no inference; see R-05), covering Stages 1–2, the Stage Gate and F.2;
   - resolver token classes only for the tokens those steps use;
   - chat copy, paste-back and packet validation;
   - CLI launch with the exact argv;
   - re-arm with a **manual idle confirmation** (no sensor yet);
   - pathspec signing, with R-10's fixes;
   - P-18 logging.

   Run TradeLog Stages 1–2 and read the kill criterion. Only then build the sensor, Today, Doctor polish, the wizard's extras, inference lint, Gate A and the assembler.
3. **Restate the estimate** as about 6 weeks to the kill check plus 10–18 weeks after it, so "bounded" is true.

---

### R-05 · Inference as a primary source, with drift as an error · Serious (the real template liability)

**Target.** P-01 as unified in r3 §3.1: "Inference fills every field it can; annotations are required only where inference can't decide. The drift detector makes any disagreement a parse error."

**The attack.**
1. **Deadlock.** When inference is *confidently wrong*, the fix is to write an annotation with the correct value. But that annotation disagrees with inference, and disagreement is a parse error. The only remaining fixes are rewording the blueprint's prose or shipping a new Foreman build. The real RUN WITH lines are irregular, for example:
   - "same session · same model · **Accept edits** (HARD class: **Manual**)";
   - "**Plan** (switch to **xhigh** + **Manual** for upload tickets)";
   - "agents: `test-runner`, then `sre-reviewer`";
   - "effort unchanged";
   - "**Opus 5.5** (switch to **Fable 5.1** if the system is platform-scale or genuinely ambiguous)".

   These are exactly where inference will be wrong.
2. **Coupling.** A retro (F.5) that rewords a line can make the template fail validation, and Master Spec F-002 "refuses to load a template that fails validation". Projects pinned to the lock keep working, but every upgrade waits on a Foreman release.
3. **Dead weight.** r3 says Stages 6–18 "parse on inference alone", yet v1 runs only Stages 1–5. Inference for 6–18 is v1 work with no v1 user.
4. **It reverses the engineer's advice.** r1 A6 recommended "Keep the prose heuristics as a lint cross-check only." The synthesis made them a source of values without saying why.

Is the Markdown itself the liability? **No.** r1's probe parsed the real 100 KB blueprint identically with comrak and pulldown-cmark, with exact line numbers. The block structure (92 blockquotes, 63 RUN WITH, 17 INSPECTION) is regular. The liability is giving the same field two sources.

**What settles it.** Run the inference rules over all 63 RUN WITH lines and count the lines where inference produces a value that differs from a careful human reading. If that count is above zero, deadlock case 1 is real.

**Fix.**
- **Annotations are the only source** of every field a machine acts on (`session`, `cwd`, `start`, `switch`, `alt`, `out`, `kind`, `tag`, `when`, `verify`). Inference becomes a **lint**: it warns, never errors, and never supplies values. For example: "prose says 'same session' but `fm:step session=new`".
- Steps without annotations are shown as "not runnable: needs annotation". They are not run on guessed values.
- P-01 drops from L to M. The annotation budget for v1 is Stages 1–5, the Gate, F.2 and Council setup: about 40 comment lines.

---

### R-06 · Positional IDs and positional verify kinds · Serious

**Target.** P-02: "Inspection item IDs are positional, and the drift detector checks the item count." r3 §3.1: `<!-- fm:verify <n>=auto|attest … -->`.

**The attack.** Suppose a retro swaps bullets 3 and 4 of Stage 5's inspection list and leaves `fm:verify 1=auto 2=attest 3=attest 4=auto` alone. The count is unchanged, so nothing fires, and "Staging deploys automatically, and a rollback has actually been run once" is now `auto`, which the headless run can PASS. That breaks the r1 J6-1 blocker fix without a trace.

The same goes for stored attestations and gate results keyed `s05.i3`: after a pin upgrade they refer to a different sentence. P-02 claims IDs "survive edits to the blueprint", but positional IDs don't.

**What settles it.** A golden test that swaps two bullets and expects an error. It fails under the current design.

**Fix.**
- Give each inspection bullet a short explicit ID in `fm:verify`, for example `fm:verify fresh-clone=auto:cmd=dev-smoke ci-gates=attest staging=attest adr-check=auto`, matched to bullets by a normalized text hash stored in the lock.
- A changed hash means the item changed: show "re-confirm the kind".
- A retro has to keep IDs stable on purpose.

---

### R-07 · `auto` items can still PASS falsely · Serious

**Target.** P-14: "Restricting headless runs to `auto` items with a `CANNOT_VERIFY` escape removes the incentive to invent evidence."

**The attack.**
- The schema accepts any `evidence` text. Nothing checks that a command ran.
- r2's own example tags Stage 5 item 1, "A fresh clone, then `install` and `dev`, works with zero manual steps", as `1=auto`. Under `dontAsk`, the run can't `git clone`, can't write outside the working directory, and can't reach the network for `install`. An honest model returns CANNOT_VERIFY; an over-eager one PASSes after reading the README.
- Gate A routes "every automated check" to `test-runner`, which the kit defines as `model: haiku`, `effort: low` (blueprint line 1875). The evidence comes from Haiku at low effort, while provenance says "Opus · high".
- Appendix A.1 sets `fallbackModel: ["sonnet", "haiku"]`, so an overloaded Opus is replaced.

**What settles it.** Run S-2′ (R-01) with Stage 5's items and count PASSes that have no matching successful tool call in the stream.

**Fix.**
- `auto` **requires** `cmd=<verify-id>`. A PASS is accepted only if the stream-json log shows a Bash tool call matching that command with a successful result. The headless docs say subagent `tool_use` and `tool_result` blocks are in the stream by default. Otherwise Foreman downgrades the result to **PROPOSED**, which Billy confirms.
- Better and cheaper: Foreman runs each `cmd` itself (argv, no shell, cwd = repo or a temporary `git worktree`) and records the exit code and the tail of the output. The LLM run then judges only items that genuinely need reading.
- Never tag environment-level checks like "fresh clone works" as headless `auto`. Make them a Foreman-run command or attest.

---

### R-08 · Re-arm: idle detection, termination, and undocumented behavior · Serious

**Target.** P-05 (re-arm) and P-23 (minimal Desktop handoff, v1), as accepted in r3.

**The attack.**

1. **A `Stop` event doesn't mean idle.** `Stop` fires *"when Claude finishes responding"* ([hooks](https://code.claude.com/docs/en/hooks)). Background subagents and background Bash keep running past the turn: *"A background subagent's results reach Claude as a completion notification in a later turn"* ([sub-agents](https://code.claude.com/docs/en/sub-agents)). The council in 2.2 runs agents "in parallel wherever a round allows". P-06's hook set (`SessionStart`, `Stop`, `StopFailure`, `Notification`, `PermissionRequest`, `SessionEnd`) can't see outstanding work. Killing the process then loses it: *"Background Bash and monitor tasks aren't [restored]"*, and an unfinished background subagent *"shows up in the resumed transcript as a note that it didn't finish"* ([sessions](https://code.claude.com/docs/en/sessions)). r2's "Background subagents must have finished, which the idle check enforces" isn't true as designed.
2. **Typing `/exit` (S-8 option a) into a non-empty input box** sends Billy's unsent draft with "/exit" appended as a prompt, which starts a turn.
3. **Termination is undocumented for interactive sessions.** The SIGTERM behavior is documented only for `-p` ("leaves the turn that was in progress unfinished and records no result", [headless](https://code.claude.com/docs/en/headless)). The transcript *"is written asynchronously and may lag"* ([hooks](https://code.claude.com/docs/en/hooks)).
4. **Four behaviors re-arm relies on are undocumented:**
   - `--effort` on resume (E-3);
   - graceful exit for interactive sessions;
   - combining `--session-id <uuid>` with `--resume <uuid>` (r3 E5 says "every launch passes `--session-id`"; the CLI reference documents `--session-id` only alone and `--fork-session` "when resuming"; **UNVERIFIED** whether the combination errors);
   - whether `claude --desktop --resume <uuid>` continues the same transcript or copies it (**UNVERIFIED**: the desktop page's "Coming from the CLI" section couldn't be fetched).
5. **Two writers.** *"If you resume the same session in two terminals without forking, messages from both interleave into one transcript"* ([sessions](https://code.claude.com/docs/en/sessions)). P-23 (v1) opens the chain's session in Desktop. If Billy keeps working there and Foreman then re-arms, the chain forks silently inside one file.
6. **A feedback loop with R-01.** Each re-arm resets "allow for this session" grants (r2 states the cost). Billy's easiest way out is "Yes, and don't ask again", which writes permanent rules into `.claude/settings.local.json`. Those widen every later session, including headless runs that aren't hardened.

**What settles it.** S-8, extended to these cases:
- (a) a non-empty input box;
- (b) a background subagent running;
- (c) a background Bash command running;
- (d) `Ctrl-C` versus `/exit` versus SIGTERM versus SIGHUP (closing the PTY);
- (e) `--session-id` together with `--resume`.

Plus S-3 extended with `--settings '{"effortLevel": "<x>"}'` (see R-17), and an S-23 checking whether Desktop resumes the same transcript.

**Fix.**
- **Sensor:** add `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, and `PostToolUse` for Bash with `run_in_background`. "Idle" means the last event is `Stop` **and** there's no outstanding subagent or background task. Otherwise re-arm shows "2 background tasks still running: wait / end them anyway".
- **Termination:** send `Ctrl-C` first (clears the input), then `/exit`. Confirm `SessionEnd` with reason `prompt_input_exit` before relaunching. Never SIGTERM a session that isn't idle without Billy's explicit confirmation.
- **argv:** pass `--session-id` only when starting a chain, never together with `--resume`.
- **Desktop:** "Open in Desktop" marks the chain `handed-off`. The next Foreman step re-arms with `--fork-session` into a new chain, unless Billy says Desktop is closed.
- **Stay honest about the cost:** put "permission grants reset at each step" in Settings, and show it in the Now/Next strip after a re-arm.

---

### R-09 · Chains outlive their transcripts · Serious

**Target.** P-05 chains, P-13 Repair, P-14 Gates B and C, P-17 Interrupted sessions.

**The attack.** Transcripts are deleted after 30 days by default: the sessions page lists *"Change the 30-day retention"* with `cleanupPeriodDays` ([sessions](https://code.claude.com/docs/en/sessions)). Billy runs six products. Stage 1 alone contains a 7-day wait. A project left for a month and then resumed hits:
- **Repair**, which re-arms "the stage's chain";
- **Gate B and C**, which continue `gate.A`;
- **Resume** from the Interrupted sessions list;
- **packet capture** (`claude -p --resume`).

Each fails with `No conversation found with session ID`. The design has no state for this. Provenance records session UUIDs as if they were lasting evidence.

**What settles it.** The documented default, plus a test that deletes a transcript and runs each of those actions.

**Fix.**
- **Preflight:** check that `~/.claude/projects/<derived>/<uuid>.jsonl` exists. The path rule is documented; the format inside the file isn't, so only check existence.
- **Doctor:** show the effective `cleanupPeriodDays` and recommend a longer value in Billy's user settings (Foreman doesn't write it).
- **A `broken` chain state** with **Start a fresh chain with context**: a new session whose prompt includes the last signed packet and the step's declared outputs.
- **Provenance:** treat session UUIDs as convenience links, not evidence.

---

### R-10 · Signing integrity · Serious

**Target.** P-13 (two-phase sign, sign on a new branch) and P-17 (commit timing).

**The attack.**
1. **Repo hooks rewrite the packet.** Foreman's `git commit -- <paths>` runs the repo's pre-commit hooks. A lint-staged plus Prettier setup on `*.md`, which is likely after Stage 5, reformats `stage-05.md` *during* the commit. The committed content then differs from the hash Billy signed, and nothing notices. Commit signing (`commit.gpgsign`) with a GUI passphrase prompt can also hang a commit started from a GUI app.
2. **"Sign on new branch `handoff/stage-NN`"** needs `git switch -c` in the checkout where a Claude session may be running, which changes HEAD under it.
3. **Stage 5 on, `.foreman/` writes dirty the tree.** P-17 writes `project.json` and `overlay.json` immediately but commits them "folded into the next signing commit (from Stage 5 on)". Between signings they are dirty in the working tree, and Claude's habitual `git add -A` sweeps them into feature commits and PRs. That contradicts P-17's own claim: "Nothing Foreman writes between signings dirties the working tree that Claude commits from."
4. **Records on an unmerged branch.** P-13 says the next stage's preflight "reads the signed packet from the stage record". If signing happened on an unmerged `handoff/stage-05`, the record isn't in the `main` checkout.
5. **A running session can overwrite a signed packet.** Signing while a session runs is allowed (with a confirmation). The session can then rewrite the packet, and the watcher only sees a "changed packet".

**What settles it.** A fixture repo with lint-staged plus Prettier on Markdown, then sign, then compare `git show HEAD:docs/handoff/stage-05.md | shasum` with the signed hash.

**Fix.**
- **After every commit, read the file back with `git show HEAD:<path>`** and compare hashes. On a mismatch, show "Your repo's commit hooks changed the packet" with the diff.
- Make **"skip hooks for signing commits"** a Settings option, on by default. These commits touch only packets, attachments and `.foreman/`. Use `--no-verify`, and record it in provenance.
- **Commit to a new branch without a checkout:** use plumbing (a temporary `GIT_INDEX_FILE`, `git commit-tree`, `git update-ref refs/heads/handoff/stage-NN`). It never touches HEAD, the index or the working tree.
- **From Stage 5 on, keep decision changes pending in app data** and write them to the repo only inside the signing commit.
- **Read stage records ref-aware** (`git show <ref>:.foreman/stages/…`), and show "signed on unmerged branch" explicitly.
- **After signing, a change to a signed packet is a tamper event:** the committed version wins, and Today shows the diff.

---

### R-11 · Paths that come from repo content · Serious

**Target.** SR-04 ("File access is limited to registered project folders…"), applied to `out=`, `requires=`, P-12 "Paste into" over `REPO FILES` parsed from the 4.8 packet, P-11 "declared extra destinations", and P-10 `write-blocks`.

**The attack.**
- The 4.8 packet is chat output that Billy pastes. A `REPO FILES:` line of `.git/hooks/pre-commit`, `.claude/settings.json` or `../../.zshrc` turns "Paste into" into a write that runs code later. Git runs `.git/hooks/*` on Foreman's own signing commit (R-10).
- A symlinked `docs/handoff`, made by anyone in the repo, sends Foreman's atomic writes outside the repo.
- "Registered project folder" checks on strings don't catch symlinks.

**What settles it.** Unit tests with `..`, absolute paths, `.git/…`, a symlinked parent directory, and Unicode tricks.

**Fix.** One path policy in Rust, applied to every write:
- repo-relative only, with no `..` and no absolute paths;
- a deny prefix list (`.git/`, `.claude/` except through the hard-coded `action` vocabulary, `.husky/`, `.github/workflows/` unless declared by an `fm:` annotation in the verified lock);
- resolve the final path with `O_NOFOLLOW` on each component, or a directory-handle API (for example the `cap-std` crate; version **UNVERIFIED** this session), so a symlink can't escape.

Then golden-test the policy.

---

### R-12 · Claude Code updates can break Foreman, and one break is announced · Serious

**Target.** r3 §3.4 (Doctor floor and capability table), the 24 h Doctor cache, and S-13 (flag probe "against `claude --help`", deferred to v1.x).

**The attack.**
1. **An announced breaking change.** *"`--bare` is the recommended mode for scripted and SDK calls, and **will become the default for `-p` in a future release**"* ([headless](https://code.claude.com/docs/en/headless)). And: *"In bare mode, Claude Code never reads OAuth credentials or the system keychain."* When that ships, every Foreman `-p` call loses Billy's subscription login: headless Gate A, packet capture with `-p --resume`, and Gate B/C after a headless A. r1 E11 ("never pass `--bare`") doesn't help once bare is the default. Whether an opt-out flag will exist is **UNVERIFIED**.
2. **S-13's method doesn't work.** *"`claude --help` does not list every flag, so a flag's absence from `--help` does not mean it is unavailable"* ([cli-reference](https://code.claude.com/docs/en/cli-reference)).
3. **Silent updates.** Claude Code updates itself, and Doctor caches `claude -v` for 24 h. Behavior can change between two steps of one chain, and Foreman finds out from an error inside a terminal tab.
4. **Behavior already changes by version.** Interactive sessions default to auto mode from v2.1.283; `-p` starts in Manual from v2.1.302 ([cli-reference](https://code.claude.com/docs/en/cli-reference), [headless](https://code.claude.com/docs/en/headless)).

**What settles it.** Settled already by the docs quote. What's left is designing how Foreman detects these changes.

**Fix.**
- **Record the version each contract was last checked on.** Store "contract last verified on Claude Code X.Y.Z" in app data. On each launch, read `claude -v` again; it's cheap, and it replaces the 24 h cache. When the version changes, run **contract probes that cost no usage**:
  - `claude --init-only --settings <sensor settings>` (it *"Run[s] Setup and SessionStart hooks, then exit[s] without starting a conversation"*) proves the per-launch hooks still fire, which is S-1′ and costs no tokens;
  - an argv-parse probe for each flag Foreman uses, where an unknown option fails before the run starts.
- **Every `-p` call checks auth in its own output.** Read the `system/init` event and the result for an authentication failure, and on failure show a precise banner: "Claude Code changed headless auth; headless gates are disabled".
- **Fall back to interactive.** Gate A interactive and paste-back packet capture already exist as fallbacks. Make the fallback switch automatic.
- Move S-13 (reworked as these probes) into **v1**.

---

### R-13 · The highest-risk spike was dropped · Serious

**Target.** r3 §1, "Spikes S-1′, S-2, S-3, S-7, S-8: run as the first Stage 6 tickets", and the build order "template core first".

**The attack.** r1 rated F-008 (PTY + xterm 6 + the real Claude TUI) **high** risk and wrote: "Do this spike first." The list includes:
- the alternate screen, bracketed paste, mouse, truecolor, resize and IME;
- VoiceOver `screenReaderMode`;
- Shift+Tab and ⌘-chord routing inside WKWebView;
- the GUI-app PATH;
- the ≤ 300 MB memory NFR with 3 tabs.

That spike appears nowhere in r3's spike list. Because the build order puts the PTY in phase 3, a failure there would show up at about week 13.

**What settles it.** The spike itself: one day or two.

**Fix.** Make **S-0: PTY and TUI fidelity** Stage 6 ticket #1, before the template core. Real `claude` in portable-pty 0.9.0 + xterm 6 inside the Tauri window, checking:
- Shift+Tab mode cycling;
- a 5 KB paste;
- resize while streaming;
- ⌘K reaching the app;
- `screenReaderMode` with VoiceOver;
- memory with 3 tabs after a 30-minute council run.

---

### R-14 · Headless Gate A is a size-L feature with almost no v1 use · Serious

**Target.** P-14 headless Gate A in v1 (r3: "Accept").

**The attack.** The metric run covers Stages 1–5. Stages 1–4 are chat stages, so all their items are attest (P-11). Stage 5 has four items. By r2's own tags only items 1 and 4 are `auto`, and item 1 can't be run under `dontAsk` (R-07). So the headless pipeline is used for about one meaningful item in the v1 run:
- schema built from IDs;
- stream-json progress;
- Cancel and timeout;
- the `failed` state;
- re-running failed items;
- resuming into B and C.

It's also the most security-sensitive part of Foreman (R-01) and depends on an announced breaking change (R-12).

**What settles it.** Count the `auto` items across Stages 1–5 once B-1 is annotated.

**Fix.**
- **v1 ships:** Gate A interactive (as the blueprint already says), **Import results**, attest, Foreman-run verify commands (R-07), and the A → B → C → Sign order.
- **Headless Gate A moves to v1.1**, together with the Crew Loop, whose Stage 6–13 gates are where `auto` items are common.
- This also takes R-01 off v1's critical path, as long as the hardened argv is still specified for v1.1.

---

### R-15 · Attestation: the risk is a missing outcome, not volume · Minor

**Target.** "Untagged items default to `attest`" (r3 §3.1) and the worry that Billy will rubber-stamp.

**Evidence.** Counted from the blueprint: Stages 0–17 have 3–5 inspection items each, about 62 in total. Volume is low, so rubber-stamping is a modest risk. The real gap: per-item conditions (`fm:if`) are v1.1 (P-25), and the schema dropped `NOT_APPLICABLE` (r3 decision on P-14). Stage 5 item 3, "Staging deploys automatically…", has no n/a outcome for a `server = no` project. Foreman's own build is one. Billy would have to attest something false or block the gate.

**Fix.** Attest outcomes are `done (note)` or `not applicable (reason)`, and both are recorded under ATTESTED with distinct labels. The note must be at least 15 characters, must differ from earlier notes in the same gate, and can link an artifact (file or URL) from a picker. The "how to check" text is shown next to the checkbox.

---

### R-16 · Sensor implementation details · Minor

**Target.** P-06.

**The attack and fixes.**
- **Shell parsing.** A hook `command` is *"passed to a shell: `sh -c` on macOS"*; *"Shell form runs when `args` is absent"* ([hooks](https://code.claude.com/docs/en/hooks)). The app-data path contains "Application Support" (a space). **Fix:** use the exec form with `args`, never a shell string.
- **A repo can switch hooks off.** It can set `disableAllHooks: true`, and Doctor checks only the user file. **Fix:** the run's `--settings` sets `"disableAllHooks": false` explicitly; command-line settings outrank project settings.
- **Unstable binary path.** The sensor binary sits in the app bundle, and Billy builds Foreman locally, so a rebuild or `cargo clean` deletes it under running sessions. Every event would then show a hook error in the TUI. **Fix:** at startup, copy the sensor to `<app-data>/bin/foreman-sensor-<hash>` and reference that path.
- **Events are unauthenticated.** Any process, including Claude's Bash, can append a fake `Stop`. `FOREMAN_RUN_ID` is inherited by every child process. **Fix:** a per-run random token in the run settings file (`args`), checked on ingest. Write the run settings file fresh at each launch and check its hash.
- **Event files keep assistant text forever.** `last_assistant_message` is stored with no retention limit (SR-09). **Fix:** keep only the last N per run, and delete them when the run's stage is signed.
- **Concurrent appends.** Hooks for one event can run in parallel. **Fix:** write one file per event, using atomic create plus rename.
- **Fallback 1 contradicts P-06.** The user-level hook edits `~/.claude/settings.json`, which P-06 says is "never touched", and it runs for every Claude session Billy starts anywhere. **Fix:** drop fallback 1. With the zero-token probe from R-12, S-1′ is decided on day one.

---

### R-17 · The S-3 fallback changes Billy's defaults · Minor

**Evidence.** *"Claude Code saves the level per model, under the `modelSettings` key in your user settings"*; only `/effort` in a `-p` run *"doesn't save it as your default"* ([model-config](https://code.claude.com/docs/en/model-config)). If re-arm types `/effort xhigh`, Billy's own sessions on that model default to xhigh from then on.

**Fix.** S-3 also tests passing effort through `--settings '{"effortLevel": "<x>"}'` on resume; that key exists per settings-reference. If both the flag and the setting fail on resume, type `/effort` and then restore the previous `modelSettings` value. That needs a write to user settings, so show it to Billy and get his consent first.

---

### R-18 · Provenance records intent, not what ran · Minor

**Evidence.**
- *"Aliases point to the recommended version for your provider and update over time"* ([model-config](https://code.claude.com/docs/en/model-config)).
- Appendix A.1 sets `fallbackModel: ["sonnet", "haiku"]`.
- Fable falls back automatically when its classifiers flag a request, and *"After a fallback, the session continues on the fallback model"*.
- Agents run their own models (`test-runner`: haiku).

**Fix.** Record the resolved model from hook input or the stream-json `system/init` event at each step. Record fallback notices when the sensor sees them. Record agent models from the pinned kit. The stage record lists "intended" and "observed".

---

### R-19 · The Doctor floor ignores agent models · Minor

**Evidence.** The kit's `Explore` (blueprint line 1860) and `test-runner` (line 1875) agents use `model: haiku`. Haiku 5.5 needs v2.1.293 (r1). npm `stable` is 2.1.292 (r1 E4). r3's floor is "2.1.293 when any *step* uses Haiku 5.5", but nearly every step uses those agents.

**Fix.** Compute the floor from steps **and** the pinned kit's agent frontmatter. In practice that's 2.1.293, so warn on the stable channel.

---

### R-20 · Hygiene of the prompt argument · Minor

**The attack.** The prompt is the final positional argument. A resolved prompt that starts with `-` could be read as an option. One that starts with `/` or `!` means a slash command or bash mode in the interactive TUI. Blueprint prompts don't start that way today, but overlay "Add prompt" (v1 P-21) does let them. The `--session-id` plus `--resume` combination is covered in R-08.

**Fix.** The argv builder rejects a prompt whose first character is `-`, `/` or `!`, or prefixes it with a fixed phrase. The fake-`claude` test asserts the position of the positional argument for every step in the golden fixture.

---

### R-21 · `blueprint.lock.md` pollutes Claude's view of the repo · Minor

**Evidence.** *"Grep respects `.gitignore`"* ([tools-reference](https://code.claude.com/docs/en/tools-reference)). The lock is committed, so it isn't ignored. A 100 KB file full of "Write docs/runbook-deploy.md", "rollback" and so on will match Gate A's searches, and could be cited as evidence.

**Fix.** The `Read(./.foreman/**)` deny in every launch's `--settings` (R-02 item 5). Alternatively, move the lock to app data and keep only its hash in the repo.

---

### R-22 · Atomic writes · Minor

**The attack.** Write-to-temp-then-rename in a repo folder makes temp files that the watcher sees, that appear in `git status`, and that Claude's `git add -A` can pick up. The spec doesn't mention fsync, or EXDEV when the temp file sits on another volume.

**Fix.**
- Name temp files `.<name>.foreman-tmp`, in the same directory, and add that pattern to `.git/info/exclude` (local and never committed).
- The watcher ignores the pattern.
- Call `sync_all` on the file and fsync the directory before and after the rename.
- Use a vetted crate (for example `tempfile`'s `persist`), with a crash-injection proptest. Whether Rust's `sync_all` uses `F_FULLFSYNC` on macOS is **UNVERIFIED**.

---

### R-23 · Re-arm in the same tab · Minor

**The attack.** A resumed session re-renders its history, so re-arming steps 2.2 → 2.6 puts the council transcript into the scrollback up to five times. That pushes against the ≤ 300 MB NFR and makes the tab hard to read. Startup with a large resumed transcript, MCP servers and `SessionStart` hooks will often take longer than 1–3 s (**UNVERIFIED**; measure it in S-8).

**Fix.** On each re-arm, archive the old xterm buffer as a collapsed, read-only log section and start a clean buffer. Measure re-arm latency in S-8 and show a spinner with "resuming 2.3…".

---

### R-24 · Reserved v1.1 data shapes; v1.1 has no kill criterion · Minor

**The attack.** `adopted`, `amendments/`, `revisions[]` and the ticket-loop state are designed before the features that use them, so they'll be wrong in some detail. For a single-user app with `schemaVersion` migrations, a v1.1 migration costs a day; reserving the shapes now adds tests and ambiguity to v1.

The bigger point is that Foreman's daily, several-times-a-day value lives in the Crew Loop (r1 J7-1: "the highest-volume activity in Stages 6–13"), and v1.1 has no stated success metric or kill criterion.

**Fix.**
- Reserve only enum values that cost nothing (`adopted`). Drop the speculative structures.
- Give v1.1 its own metric, for example "one TradeLog milestone through the CLI Crew Loop with zero manual routing", and its own kill criterion.
- Deferring the Crew Loop and CR track itself stands (see "What survived").

---

### R-25 · PATH and environment allowlist · Minor

**Evidence.** *"At session start, Claude Code sources `~/.zshrc`, `~/.bashrc`, or `~/.profile` …, captures the resulting aliases, functions, and shell options"*, and *"Environment variables don't persist"* ([tools-reference](https://code.claude.com/docs/en/tools-reference)). So `nvm`'s function arrives, but the `node` on PATH that nvm sets up doesn't. P-07's candidate list will miss tools installed by nvm, fnm, mise or volta. When `pnpm` is missing, Claude in `acceptEdits` or `auto` mode tends to "fix" it by installing tools.

The allowlist also drops `CLAUDE_CODE_OAUTH_TOKEN`, which breaks the `oauth_token` auth P-07 accepts. It drops `CLAUDE_CONFIG_DIR` too, which moves auth and transcripts.

**Fix.**
- Pass `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` through whenever they're present in Foreman's own environment.
- Offer a one-time, consented **"Capture my login shell PATH"** in Doctor (`$SHELL -l -c` printing PATH once, shown to Billy and stored). It is the standard way GUI editors handle this. SR-01 forbids Foreman *spawning shells for runs*; this is a single setup probe, and Claude Code sources the same rc files anyway.
- Run S-12 before the freeze, not in Stage 6.

---

### R-26 · Grammar versioning and chain cwd · Minor

**The attack.** "Unknown keys are hard errors" with no grammar version means a blueprint annotated for v1.1 keys can't be read by v1, even for projects that don't use those steps. A continuing step can also declare a different `cwd` from the chain it joins. `--resume` by ID works "from any directory" ([sessions](https://code.claude.com/docs/en/sessions)), so the conversation would silently move to a new working directory.

**Fix.**
- Add `fm:grammar v=1` at the top of the blueprint. Unknown keys are errors within a version and warnings across versions.
- Validator rule: a step with `session=continue` inherits its chain's cwd, and declaring a different one is an error.

---

## 3. What survived

I attacked these and couldn't break them. Some survive with the amendment noted.

| Decision | Why it survived |
|---|---|
| **Markdown stays the source**, with block-level `fm:` HTML-comment annotations (r3 §3.1, spec Q1) | The parse is proven (r1's two-parser probe, identical structure, line numbers). Comments don't show in the rendered page or the PDF. The alternatives (YAML source, sidecar file) are worse for the retro loop. *Amended by R-05 and R-06:* annotations are the only source, and IDs are explicit. |
| **`portable-pty` directly; no `tauri-plugin-pty`, `tauri-plugin-shell` or `tauri-plugin-fs`** | The plugin lets the webview choose the program (r1 read its source). Keeping spawning in Rust is the right boundary. |
| **`--session-id <uuid>` at chain start; the UUID is authoritative, `--name` is for display** (E5) | Documented: *"Use a specific session ID for the conversation (must be a valid UUID)"*; ambiguous names open the picker. *Amended by R-08:* don't combine it with `--resume`. |
| **Model and permission-mode overrides on resume** (P-05's base) | Documented: *"A model you pick for the new launch with `--model` … still takes precedence over the restored model"* ([model-config](https://code.claude.com/docs/en/model-config)) and *"Pass `--permission-mode` … to override the restored mode"* ([sessions](https://code.claude.com/docs/en/sessions)). Re-arm as a *mechanism* survives; its idle detection, termination and lifetime do not (R-08, R-09). |
| **Per-launch `--settings` for the sensor hooks** (P-06) | *"Values you set here override the same keys in your `settings.json` files for this session"* ([cli-reference](https://code.claude.com/docs/en/cli-reference)). The hooks page itself uses `--settings` to set `disableAllHooks` per run. Lists merge, so Billy's hooks still run. S-1′ remains, but it's low risk and can be settled with a zero-token `--init-only` probe (R-12). |
| **Never pass `--bare`** (E11) | Correct today: bare mode never reads OAuth. *Amended by R-12:* bare becoming the default needs detection. |
| **`dontAsk` rather than `plan` for unattended runs** (E3) | The classifier-in-plan finding is documented. The *mode* choice survives; the rest of the argv doesn't (R-01). |
| **Untagged items default to `attest`** | It's safe. Volume is low (3–5 items per stage), so it doesn't produce a rubber-stamping workload. *Amended by R-15:* add an n/a outcome. |
| **Pathspec-only commits, hash-bound signing, Foreman-written SKIPPED / SUBSTITUTIONS / ATTESTED, A → B → C → Sign** (P-13) | Each closes a real silent-corruption path. *Amended by R-10:* verify after commit, hooks, ref-aware records. |
| **Splitting decisions (repo) from in-flight state (app data)** (P-17) | The principle is right, and it avoids branch-protection collisions before Stage 5. *Amended by R-02 and R-10:* approval hashes in app data, and pending writes after Stage 5. |
| **Deferring the Crew Loop (P-19) and the CR track (P-20) to v1.1** | No Stage 1–5 step uses tickets. Conditions are free until the Stage 2 sign, so the metric run never needs a CR. B-4 (strict ticket format) staying in v1 is the right exception. *Amended by R-24:* a v1.1 metric and kill criterion. |
| **Chat bridge with the clipboard as the contract** (P-11) | It doesn't depend on undocumented claude.ai URL prefill. Normalizing `<blank>` and splitting packets from attachments fix real first-paste failures. |
| **Conditions tri-state and freely editable until the Stage 2 sign** | It removes premature CRs. The Confirm-conditions checkpoint at the Stage 2 sign is cheap and right. |
| **Today inbox with file-based attention kinds** (P-15) | They work without the sensor, and the sensor-based kinds degrade cleanly. |
| **Tauri hardening** (explicit CSP with compile-time hashes, `freezePrototype`, `AppManifest::commands`, a separate `devCsp`) and **TS 6.0.3 / SvelteKit 3 config** (E1, E2) | r1 verified these from source and a working build. I found nothing contradicting them. |

---

## 4. Spike list after this round

| ID | Change | Before freeze? |
|---|---|---|
| S-0 | **New.** PTY + xterm 6 + real `claude` TUI fidelity and memory (R-13) | Stage 6 ticket #1 |
| S-1′ | Run as a zero-token `claude --init-only --settings` probe (R-12) | Yes |
| S-2′ | **Rewritten.** Hostile-repo fixture against the r3 argv and the hardened argv (R-01) | Yes |
| S-3 | Add the `--settings effortLevel` variant (R-17) | Yes |
| S-7′ | Settings `env` and `apiKeyHelper` changing auth in a trusted repo (R-03) | Yes |
| S-8 | Extend: non-empty input, background subagent or Bash, Ctrl-C vs `/exit` vs SIGTERM vs SIGHUP, `--session-id` with `--resume`, re-arm latency (R-08, R-23) | Yes |
| S-12 | Move before the freeze (R-25) | Yes |
| S-13 | Replace `--help` diffing with contract probes; v1 (R-12) | Stage 6 |
| S-23 | **New.** Does `--desktop --resume` share the transcript with the CLI? (R-08) | Stage 6 |

---

## Sources

All fetched 2026-10-19.

- Claude Code:
  - https://code.claude.com/docs/en/permissions
  - https://code.claude.com/docs/en/permission-modes
  - https://code.claude.com/docs/en/settings
  - https://code.claude.com/docs/en/settings-reference (index only; detailed entries were cut off when fetched)
  - https://code.claude.com/docs/en/cli-reference
  - https://code.claude.com/docs/en/headless
  - https://code.claude.com/docs/en/sessions
  - https://code.claude.com/docs/en/hooks (first ~110k characters; the Stop and Notification sections weren't reachable, so Stop-with-background-work is **UNVERIFIED**)
  - https://code.claude.com/docs/en/model-config
  - https://code.claude.com/docs/en/sub-agents
  - https://code.claude.com/docs/en/tools-reference
  - https://code.claude.com/docs/en/desktop (the "Coming from the CLI" section wasn't reachable)
- Tauri: https://v2.tauri.app/develop/calling-frontend/ (Channels are "designed to be fast and deliver ordered data" and are used for "child process output"; no objection raised).
- Project inputs: `claude-build-blueprint.md` v2.1 (Appendix A.1 lines 1821–1847; agent frontmatter lines 1860–1988; inspection lists), `r3-synthesis.md`, `r2-proposals.md`, `r1-engineering.md`, `r1-workflow.md`, `docs/handoff/stage-01.md`, `01-master-spec.md` v0.9.
