# Runbook · End of phase 1 on the Mac: Stage 8 inspection, TradeLog Stages 1–2, kill check

Tickets M3-29, M3-30 and M3-31. Billy runs all three on his Mac with the real Claude Code; nothing here runs in the cloud sandbox. Do them in order: M3-30 depends on M3-29, and M3-31 on M3-30 (`docs/tickets.md`). Each section says what to run, what to record, where it goes, and the pass/fail rule, quoted from `docs/scope.md` (EX-M3-01, EX-M3-05, EX-M3-07, EX-M3-08, AC-KC-01 to AC-KC-05) and `docs/MASTER-SPEC.md` §9 (kill criterion).

Write everything down as you go. A result that isn't recorded in the named file doesn't count.

## 0. Before you start (10 minutes)

```bash
cd ~/path/to/engine-assist
git switch <the integrated stage branch> && git pull
pnpm install --frozen-lockfile
pnpm verify                      # all steps green; stop and fix anything red first
pnpm release                     # verify, build, then install /Applications/Foreman.app (answer y)
claude -v                        # must be ≥ 2.1.293
claude auth status               # authMethod must be claude.ai or oauth_token
```

Record at the top of `docs/security/stage-08-review.md` (create it): the date, the commit (`git rev-parse --short HEAD`), the macOS version (`sw_vers -productVersion`), the Mac model, the `claude -v` output and the Foreman version (Settings → About).

## 1. M3-29 · Stage 8 security inspection

**Pass rule.** All of these hold (EX-M3-01, EX-M3-05, AC-M0-10):

- `network::blocked_network_run` passes, and the full harness and the app's first-run path pass under the deny-network profile with Foreman making no request of its own (AC-M0-10, re-run);
- every M3 acceptance criterion and AC-XC-07, AC-XC-08, AC-XC-16, AC-XC-17, AC-XC-19 and AC-XC-29 passes (EX-M3-01);
- every threat in `docs/threat-model.md` links to a control and a passing test, and the `security-reviewer` agent reports **zero CRITICAL or HIGH** findings (EX-M3-05).

Any miss fails the inspection: fix it (a ticket, or a CR if it changes frozen behavior), then rerun this section.

**1.1 The required test (macOS only).**

```bash
cargo test -p foreman-app --test network          # network::blocked_network_run
```

Record: pass or fail, and the test output's last line.

**1.2 The harness and the test suite with networking denied.**

```bash
cargo fetch                                       # warm the cache; cargo can't reach crates.io inside the sandbox
scripts/no-network-run.sh cargo test --workspace  # the TradeLog harness and every Rust test
```

The script refuses to run if the profile lets a `curl` through. Record: pass or fail, and which mechanism you used (sandbox-exec, or the pf fallback in `docs/runbooks/release.md`) in that runbook's "No-network runs" table.

**1.3 The first-run path with networking denied.** First run needs empty app data, so set yours aside and put it back afterwards:

```bash
osascript -e 'quit app "Foreman"'
mv ~/Library/Application\ Support/dev.foreman ~/Library/Application\ Support/dev.foreman.p1-backup
scripts/no-network-run.sh /Applications/Foreman.app/Contents/MacOS/foreman-app
```

In the app: First run → pick the blueprint repo → Doctor runs → create a throwaway project with the setup form → open it. Watch for an error that names the network. Quit Foreman (⌘Q), then restore:

```bash
rm -rf ~/Library/Application\ Support/dev.foreman
mv ~/Library/Application\ Support/dev.foreman.p1-backup ~/Library/Application\ Support/dev.foreman
```

Record: pass or fail in the "No-network runs" table (`What ran`: "first-run path, app build <version>").

**1.4 The real-app checks (WKWebView).**

```bash
pnpm test:e2e      # launch, CSP (served policy, injected script refused, style-nonce carrier), navigation
```

Then the open Mac checks from `docs/progress.md` ("Mac checks") that belong to Stage 8 and the terminal: 8 (keyboard script), 9 (`open -u https://claude.ai/`), 10 (S-0 paste into the real TUI), 11 (notifications), and the S-0 rows in `docs/spikes/S-0.md` (B1, B2, B5 especially: zero CSP violations at boot and with a streaming terminal; CR-013 is the change that keeps `style-src` strict). Record each check's result in the review file, and tick it off in `docs/progress.md`.

**1.5 The threat model, control by control.** For every row `T-001` … in `docs/threat-model.md`, confirm the "Tests" column names a test that exists and passed in 1.2 or 1.4 (or a manual check you just did). The quickest way:

```bash
node scripts/check-required-tests.mjs            # lists any required test that doesn't exist
```

Write a table in the review file: threat ID · control · test(s) · result. A threat whose test is missing or failing is a finding (severity from the threat model's likelihood × impact).

**1.6 The security review.** In Claude Code at the repo root (Opus 5.5, effort xhigh; no Fable models):

```
Use the security-reviewer agent. Review the whole foreman repo at <commit> against
docs/threat-model.md and docs/requirements.md (every SR and PR item), with M3's code in focus:
the launch pipeline, PTY and terminal, clipboard, URL opening, the watcher, paste-back, the run
log and the diagnostic log. Run the scanners (cargo audit, cargo deny, pnpm audit, gitleaks).
Rate every finding CRITICAL / HIGH / MEDIUM / LOW with evidence and a fix. Report only.
```

Paste the agent's report into the review file under "security-reviewer". Record the count per severity.

**1.7 Close.** In `docs/progress.md`, set M3-29 to `done` (with the review file's date) only if the pass rule holds; otherwise leave it `billy` and list what failed.

## 2. M3-30 · TradeLog Stages 1–2 through Foreman

**Pass rule (EX-M3-07).** Every step of TradeLog Stages 1–2 has been **started through Foreman**: chat steps through Copy and open (⌘↵), CLI steps through Run in terminal (⌘↵). Close-out (attest, F.2 lessons, APPROVED BY and the commit) is by hand, guided by the close-out checklist step (AC-F015-01).

This run is also the kill check's data (section 3), so how you start each step matters:

- Start every step from Foreman: focus Foreman or press **Do next (⌘⇧↵)**, then Run or Copy and open. Don't open the blueprint or prompts file to find a step.
- Don't edit a prompt by hand. If a step needs different text, set its declared run input (that is logged as an intervention) and write down why. Avoid **Copy prompt on a CLI step**: each one is a proxy for a manual edit and needs an explanation (AC-KC-03).
- Keep waits honest: if you walk away while Claude works, that's a wait, and it is excluded automatically for CLI runs.

**2.1 Set up.** In Foreman: ⌘N → TradeLog, the folder of the TradeLog repo, the variables and conditions from your notes → Create. Pin the blueprint when asked.

**2.2 Stage 1 (chat).** For each step, in order: select it (J/K) → ⌘↵ (Copy and open) → paste and send in claude.ai → when the reply is in, copy it → ⌘⇧V (Paste back) → Apply, or ⌘D (Mark done) for a step with no output. When the packet is saved and valid, follow the close-out checklist: attest, F.2 lessons (paste into `docs/lessons.md` by hand), fill APPROVED BY, commit by hand.

**2.3 Stage 2 (CLI and chat).** Same loop. For a CLI step: ⌘↵ → read the launch preview (the argv on screen must be the argv spawned, EX-M3-02) → confirm in the native dialog → work in the terminal tab. For a continuing step, Run re-arms the session after you confirm "Claude is idle" (the old buffer becomes the archive above the new one). Close out Stage 2 by hand as in 2.2.

**2.4 Record.**

- In the TradeLog repo: `docs/handoff/stage-01.md` and `docs/handoff/stage-02.md`, signed by you (APPROVED BY), committed.
- In Foreman's Run log (sidebar → Run log): every Stage 1–2 step key appears with a run started through Foreman (chat steps as `copied`, CLI steps as runs with an exit code). Write the list of step keys and run IDs in `docs/handoff/stage-09.md` under "TradeLog Stages 1–2", with every intervention or Copy prompt on CLI and its reason (section 3 needs them).
- Anything that went wrong (a refusal you didn't expect, a crash, a wrong flag): a line each, with the run ID. Foreman's own log is `~/Library/Application Support/dev.foreman/logs/foreman.log` (no prompt or terminal text in it; safe to attach).

**2.5 Close.** Set M3-30 to `done` in `docs/progress.md` when both packets are signed and every step key is in the run log.

## 3. M3-31 · The kill check

**Pass rule (AC-KC-01 to AC-KC-05; Master Spec §9).** For **each surface, chat and CLI**, all of:

1. **Baseline present (AC-KC-04):** `docs/metrics-baseline.md` has by-hand entries for the surface: CLI from Foreman's own Stage 5 onward; chat from **at least 10 chat steps** you ran by hand between M0 and now. A surface with fewer entries can't pass.
2. **Time to start (AC-KC-01):** Foreman's median seconds to start over TradeLog Stages 1–2 is **lower** than the by-hand median for the same surface (end point: the spawn for CLI; the filled prompt on the clipboard and the chat open for chat).
3. **Active time (AC-KC-02):** Foreman's median active minutes per step is **lower** than the by-hand median for the same surface.

And across all of TradeLog Stages 1–2:

4. **Manual prompt edits (AC-KC-03):** none. Every proxy event in the export (a run-input intervention, a Copy prompt on a CLI step) is explained in your Stage 9 attestation as not a manual prompt edit. An event you can't explain that way is a manual edit, and the check fails.

**Decision (AC-KC-05):** all hold → M4 starts. Any fails → work stops, no M4 ticket starts, and you draft a Change Request (`docs/templates/change-request.md`) that proposes the simplification (Master Spec §16).

**3.1 Make Foreman read your baseline.** Export metrics reads `docs/metrics-baseline.md` from the first registered project whose Log table has entries (`runs::read_baseline`). Your by-hand log is in this repo, which isn't registered in Foreman, so copy it into the TradeLog repo (it's a doc there, commit it with the Stage 2 close-out or on its own):

```bash
cp ~/path/to/engine-assist/docs/metrics-baseline.md ~/path/to/tradelog/docs/metrics-baseline.md
```

Check the Log table first: every row has a Surface (`chat` or `CLI`), Seconds to start and Active minutes; count the chat rows (≥ 10) and the CLI rows (≥ 1).

**3.2 Export.** Foreman → Run log → **Export metrics** → save as `kill-check-<date>.md` in a folder outside any repo's `.git`, `.claude` or `.foreman` (Desktop is fine; the save is refused inside those).

**3.3 Read the export.** Its **Summary** table has both columns side by side; its **Kill check** table has one row per surface: "Baseline present", "Median to start lower", "Median active lower", "Manual-edit proxies". The check passes only when every row says yes and no step needed a manual prompt edit (the export says so in the same words). Cross-check the "Steps logged / By hand" counts against your Log table; if they differ, Foreman read another project's file (3.1).

**3.4 Record.**

- `docs/metrics-baseline.md` (this repo), **Summary** table: fill every row, "By hand" from your log and "Foreman" from the export's Summary (median and p95 seconds to start per surface, median active minutes per surface, manual edits per step, steps logged). Write the decision and the date under the table.
- `docs/handoff/stage-09.md` (Foreman's Stage 9 packet): the export's Kill check table, the result per criterion (AC-KC-01 to AC-KC-04), the decision (AC-KC-05), and your attestation explaining each proxy event (AC-KC-03). Attach the export file. You sign it (APPROVED BY); nobody else does.
- If it fails: the CR draft in `docs/change-requests/`, status Proposed, linked from the Stage 9 packet.

**3.5 Close.** Set M3-31 to `done` in `docs/progress.md` when the Stage 9 packet records a pass (EX-M3-08). On a fail, set it to `done` with "failed: CR-NNN" and stop M4 work.
