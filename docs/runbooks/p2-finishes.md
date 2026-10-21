# Runbook · The finishes on the Mac: Stages 14 to 17 (M5-08 to M5-14)

Tickets M5-08 to M5-14. Billy runs them on his Mac with a release build and the real Claude Code; nothing here runs in the cloud sandbox, and the build never signs a packet. The tickets form a chain (`docs/tickets.md`): each depends on the one before it, so do them in order. Each section says what to run, what to record, where it goes, and the pass or fail rule, quoted from `docs/scope.md` (EX-M5-03 to EX-M5-08, AC-SM-01 to AC-SM-04) and `docs/MASTER-SPEC.md`.

Write everything down as you go. A result that isn't recorded in the named file doesn't count. A finding that fails a pass rule is fixed first (a ticket, or a Change Request if it changes frozen behavior), then the section is run again.

Before M5-08, the earlier checks must be closed: Mac checks 1 to 24 in `docs/progress.md` ("Mac checks"), and the M5-05 to M5-07 results (axe, keyboard, mockup parity) recorded in the Stage 13 packet.

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

Record at the top of each review file you create below: the date, the commit (`git rev-parse --short HEAD`), the macOS version (`sw_vers -productVersion`), the Mac model, the `claude -v` output and the Foreman version (Settings → About).

## 1. M5-08 · Stage 14 security review

**Pass rule (EX-M5-03, security part).** All of these hold:

- `network::blocked_network_run` passes on the **release build**, and `release::no_webdriver` passes;
- every SR, PR and AR in `docs/requirements.md` is re-verified, with the ASVS 5.0 level 2 chapters that apply;
- the `security-reviewer` agent reports **zero open CRITICAL or HIGH** findings.

**1.1 The two required tests.**

```bash
cargo test -p foreman-app --test network           # network::blocked_network_run (macOS only)
cargo test -p foreman-app --test release           # release::no_webdriver
scripts/no-network-run.sh /Applications/Foreman.app/Contents/MacOS/foreman-app   # the release build, no network
```

For the last line: use the app (first run, a project, a run, Doctor) for ten minutes and watch for any error that names the network. Record each result, and the mechanism (sandbox-exec, or the pf fallback in `docs/runbooks/release.md`), in that runbook's "No-network runs" table.

**1.2 Every requirement, one by one.**

```bash
node scripts/check-required-tests.mjs               # lists any required test that doesn't exist
```

In `docs/security/stage-14-review.md` (create it) write a table: requirement ID · control · test or manual check · result. Cover every SR, PR and AR, and the ASVS 5.0 L2 chapters that apply to a local desktop app (record the ones you excluded and why).

**1.3 The security review.** In Claude Code at the repo root (Opus 5.5, effort xhigh; no Fable models):

```
Use the security-reviewer agent. Review the whole foreman repo at <commit> against
docs/threat-model.md and docs/requirements.md (every SR, PR and AR). Run the scanners
(cargo audit, cargo deny, pnpm audit, gitleaks). Rate every finding CRITICAL / HIGH / MEDIUM /
LOW with evidence and a fix. Report only.
```

Paste the report under "security-reviewer". Record the count per severity.

**1.4 Close.** Set M5-08 to `done` in `docs/progress.md` only if zero CRITICAL or HIGH are open. Threats this ticket covers: T-001, T-101, T-106.

## 2. M5-09 · Stage 14 performance and architecture audits

**Pass rule (EX-M5-03, performance and architecture part).** Every NFR is met in the **release build**, including NFR-01 (cold start p95 ≤ 1.5 s), NFR-02 (project switch p95 ≤ 300 ms), NFR-19 (50 projects), NFR-20 (2,000 tickets), NFR-22 (12 tabs), NFR-23 (≤ 300 MB with 3 tabs, ≤ 800 MB with 12) and NFR-25 (app data ≤ 50 MB per year). Zero open CRITICAL or HIGH findings in either audit.

**2.1 Performance** (write `docs/audits/stage-14-performance.md`, one row per NFR: target · measured · how · result).

```bash
cargo bench -p foreman-core                         # template, resolver, run index, core: record mean and interval
```

- NFR-01 and NFR-02 in the release app: launch, then read `perf: startup` and `perf: project switch` in `~/Library/Application Support/dev.foreman/logs/foreman.log` and the `fm:cold-start` and `fm:project-switch` marks (Mac check 22). Repeat the cold start 20 times for the p95.
- NFR-19: open the app with a 50-project registry fixture (the Stage 14.2 benchmark in `docs/MASTER-SPEC.md`); NFR-20: open a project whose `docs/tickets.md` has 200, then 2,000, tickets, and confirm each stays within NFR-02.
- NFR-22 and NFR-23: three, then twelve real terminal tabs for 30 minutes; read the app's resident memory in Activity Monitor.
- NFR-25: per-run size × runs per year from the run log, with 90-day pruning (AC-F026-10).
- NFR-07 (no frontend task over 50 ms): the long-task observer's report from the release build.

**2.2 Architecture** (write `docs/audits/stage-14-architecture.md`). In Claude Code (Opus 5.5, xhigh): compare every ADR in `docs/adr/` with the code (crate boundaries, `foreman-core` with no tauri, tokio or spawn: `bash scripts/check-core-deps.sh`; `foreman-sys` the only place that spawns). List each mismatch and fix the ADR or the code. Record the count per severity.

**2.3 Close.** Set M5-09 to `done` only if every NFR row passes and nothing CRITICAL or HIGH is open.

## 3. M5-10 · Stage 15 first run and copy review

**Pass rule (EX-M5-04).** A fresh install reaches a ready first project through first run alone, and every string passes the writing rules (`docs/MASTER-SPEC.md` A.9).

**3.1 Fresh install.** Move your app data aside (`mv ~/Library/Application\ Support/dev.foreman{,.p2-backup}`), install the release build, launch it. Without touching a terminal or a file: pick the blueprint repo, let Doctor run, create a project with the setup form, open it. Time it, and record every moment you were stuck. Restore your app data afterwards.

**3.2 Copy review.** Go through every screen in both themes and every error, empty, locked and not-applicable state, with the rules open:

- names say what they do ("Run in terminal", not "Execute"), and the same verb follows through ("Sign packet" → "Packet signed");
- every error says what happened and how to fix it;
- every empty state invites the next action;
- every not-applicable and locked state explains why in one line.

Record in `docs/handoff/stage-15.md` (create it): the first-run time, the list of strings you changed (file and before/after, in `src/lib/copy/` or where the string lives), and "no strings left unreviewed".

**3.3 Close.** Set M5-10 to `done` only if first run reached a ready project unaided and every string passed.

## 4. M5-11 · Stage 16 readiness and drills

**Pass rule (EX-M5-05).** Every readiness item is DONE, including the degraded modes, and the three drills are recorded in `docs/drills.md`.

**4.1 Readiness** (`docs/readiness.md`, create it: item · status · evidence). Degraded modes, each run on the release build and recorded with the message shown:

| Mode | How to provoke it | Expect |
| ---- | ----------------- | ------ |
| `claude` missing | Put `claude` off the PATH Foreman captures (rename it for the test, then put it back) | Doctor says what is missing and how to fix it; no step launches |
| Logged out | `claude auth logout` | Doctor's sign-in row fails with the fix; launches are refused. Sign in again afterwards |
| `claude` too old | A copy earlier than the floor (2.1.293) first on the PATH | Doctor and the launch preview name the floor |
| `git` missing | Run with `PATH` without git | Gate and sign refuse with the reason; nothing is written |
| Blueprint repo moved | Move the blueprint folder | First run and Template health say where it was and how to point to the new place |
| Project folder deleted | Delete a test project's folder | The project shows as missing; removal still works; nothing else is affected |
| Disk full | A small disk image filled to the brim, app data on it | A clear error on the failed write; nothing half-written |

**4.2 Drills** (`docs/drills.md`, create it; each: date, steps, what happened, result):

1. Kill Foreman mid-write: during a packet write or a sign, `kill -9` the app. Relaunch: no corrupted file, interrupted runs shown (AC-XC-18, AC-F020-10).
2. Corrupt an app-data state file (truncate one JSON in `~/Library/Application Support/dev.foreman/`): a clear error and a way to recover.
3. Install a broken release and roll back to `Foreman-previous.app` by `docs/runbooks/release.md` ("Rollback"), and record the version that opens.

**4.3 Close.** Set M5-11 to `done` only if every readiness item is DONE and all three drills are recorded. Threat: T-059.

## 5. M5-12 · TradeLog Stages 3 to 5 through Foreman

**Pass rule (EX-M5-06).** TradeLog Stages 3, 4 and 5 run through Foreman, so Stages 1 to 5 have run entirely through Foreman.

Same loop as section 2 of `docs/runbooks/p1-kill-check.md`: start every step from Foreman (Do next ⌘⇧↵, then Run or Copy and open), never open the blueprint file to find a step, and never edit a prompt by hand (set the declared run input instead, and write down why). Close each stage by hand: attest, lessons, APPROVED BY, commit. Stage 5 ends at its gate, with the rollback check (Mac check in `docs/runbooks/release.md`, "Rollback check (AC-M0-11)").

Record in the TradeLog repo: `docs/handoff/stage-03.md`, `stage-04.md` and `stage-05.md`, each signed by you; and in `docs/metrics-baseline.md` the run notes. Set M5-12 to `done` when the three packets are signed.

## 6. M5-13 · Success metrics

**Pass rule (AC-SM-01 to AC-SM-04, EX-M5-07).** Read after TradeLog Stages 1 to 5 and a week of real use:

| ID | Rule | Where to read it |
| -- | ---- | ---------------- |
| AC-SM-01 | Every step of TradeLog Stages 1 to 5 was started through Foreman | Run log and the stage records |
| AC-SM-02 | Zero manual prompt edits (as AC-KC-03) | Metric panel and your attestation |
| AC-SM-03 | Zero gates skipped by accident: every applicable gate item has a result, a note or a not-applicable reason | Stage records (AC-F015-10) |
| AC-SM-04 | Time to start a step p95 ≤ 10 s, excluding time typing run inputs (NFR-09) | Metric panel (AC-F026-09) |

Export the metrics (Run log → Export metrics) and record the four results in `docs/handoff/stage-17.md` (create it), with the export's file name and date. Set M5-13 to `done` only if all four hold.

## 7. M5-14 · Certificate of Occupancy

**Pass rule (EX-M5-08).** Foreman v1.0.0 is installed from a release build with rollback proven; every ADR matches the code; packets 1 to 16 are signed; lessons are recorded at every gate.

```bash
git tag -l 'v1.0.0'                                  # after you decide to tag
pnpm release                                         # answer y; the old app becomes Foreman-previous.app
cargo test -p foreman-app --test release             # release::no_webdriver on the v1.0.0 build
```

Then: roll back and forward once by `docs/runbooks/release.md` and record the versions that opened; confirm `docs/handoff/stage-01.md` to `stage-16.md` each have APPROVED BY filled in by you; check `docs/lessons.md` has an entry for every gate. Record the results in `docs/handoff/stage-17.md`, and sign it yourself. Update `docs/adr/` where Stage 14's architecture audit found drift. Set M5-14 to `done` when the pass rule holds. Threat: T-106. Out of scope: v1.1.
