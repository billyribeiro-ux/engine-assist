# Stage 4 · Findings carried into the independent review (4.7)

These came up while writing the threat model (4.3) and the contracts and domain rules (4.4). The 4.7 review checks them along with everything else, and the fix pass resolves them.

## From 4.3, threat model (§7)

F-1 to F-13 are already applied in `docs/requirements.md`:

| Finding | Applied as |
|---|---|
| F-1 | SR-24a, native confirmations |
| F-2 | SR-05, the removed environment variables now cover SR-07 |
| F-3 | SR-45, plumbing only (no status, diff, add or commit) |
| F-4 | SR-44, refuses to act on an enclosing repo |
| F-5 | SR-01, helper processes run with app data as the working directory |
| F-6 | SR-47, a signature written outside Foreman needs a native confirmation |
| F-7 | SR-26, default-ignorable characters are shown |
| F-8 | SR-31, an exact interest set for the watcher |
| F-9 | SR-09, an S-1′ case-variant probe |
| F-10 | SR-31, a nesting depth limit of 64 |
| F-11 | SR-30, path length limits |
| F-12 | SR-06, `config:changed` marks open runs |
| F-13 | SR-30, a write allowlist |

The review checks that the threat model's §7 and the ADRs reflect these.

## From 4.4, contracts and domain rules

1. `fm:if` must be block-level, but prompts sit inside code fences. Splitting the fences around the comments changes the rendered PDF slightly. The blueprint proposal (B-1) must say so.
2. The proposal sets `kind=followup` on 2.3, which conflicts with F-007's meaning of `followup` ("copy and focus the terminal"). Either define one `kind` for a continuing prompt step, or rename it.
3. Steps with no RUN WITH line still need annotations: Council setup, the 7-day wait after 1.3, the manual step in 2.4, and 5.1's second prompt. The grammar also needs a wait duration (`wait=<n>d`).
4. The proposal puts `in=X` on 1.1, but `[X]` appears in 1.2's prompt.
5. Chat steps have no effort value (`adaptive`). Claude Code's `effortLevel` setting rejects `max` and `ultracode`, which S-3's fail branch relies on.
6. 3.2 continues 3.1's chat, but 3.1 doesn't apply to Foreman's own build, so 3.2 would lose the Stage 2 packet. The rule: when a chain's first step doesn't apply, the next applicable step starts a fresh chain with context.
7. Packet formats sit inside prompt fences, not in separate fences as ADR-005 says. Formats don't mark which fields depend on conditional steps, so Foreman's own Stage 3 packet would fail validation. The Stage 1 packet signature has a trailing parenthetical; Billy re-signs it.
8. ADR-012 says the stage record holds its own commit OID, which is impossible because the record is inside that commit. ADR-012 also still lists Status and Diff operations. Proposal B-3 commits `docs/lessons.md`; F-014's path list doesn't include it.
9. The design doc's command table is missing about 20 commands that the acceptance criteria need. `probe_folder_open` is P2 in the design but M3 in scope.
10. `[BUDGET_TIME]`'s example value is a placeholder, so the harness fixture needs a real value. Item IDs must be unique across the whole template, because the lock keys items by ID alone.

## From 4.6, tickets

11. There are 160 tickets. By ticket count, 69% of the work comes before the kill check, while the Master Spec estimates about half by weeks. The review should check whether phase 1 can be cut further, or whether the estimate should be restated.
12. M4-28 (the build-package assembler) is NEEDS-DESIGN. It has no mockup or component entry for its panel, and the rules for prefilling late conditions from evidence cover only one case.
13. ADR-012 still lists Status and DiffNoExt; they need to be removed (see item 8).

## 4.7 review: resolution

Applied in the Stage 4.7 fix pass (2026-10-19). Files are in `stage/04-drawings` unless marked **s2** (`stage/02-design-studio`: `spec/blueprint-v2.2/PROPOSAL.md`, `docs/MASTER-SPEC.md`) or **s3** (`stage/03-permits`: `docs/requirements.md`). The carried items 1–13 above are covered by M-1, M-2, M-5, M-10 and m-2/m-10 below.

| Finding | Files changed | Resolution |
|---|---|---|
| M-1 Native confirmations | design-doc.md, contracts/ipc.md, adr/ADR-006, ADR-007, ADR-008, ADR-014, ADR-021, ADR-022, threat-model.md, scope.md, tickets.md, design/components.md, mockups gate and outside-change; requirements.md (s3) | No request carries an approval (IPC-R11); launch, re-arm idle, Terminate, accept, sign, item kind and `CLAUDE.md` are native dialogs; ADR-014 "Native confirmations"; RR-03 narrowed to `pty_write`; SR-24a rows in design §5.1, scope §5 and threat §6.1; AC-F008-18, -F021-06, -F022-02, -F001-17 are int (scripted dialog, Cancel changes nothing) plus comp. |
| M-2 Git plumbing | adr/ADR-012, domain-rules.md (DR-140, DR-144), scope.md (AC-XC-04, AC-F002-02, AC-F014-12), design-doc.md §3.5.5, contracts/files.md, threat-model.md; requirements.md SR-44, SR-45 (s3); MASTER-SPEC §8 (s2) | Operation enum = DR-140 exactly (adds `symbolic-ref -q HEAD`, `rev-parse --verify`, `read-tree --empty`); dirty check by hash; OID in `progress.json` `signing[NN].commit`; v1 always `commit-tree --no-gpg-sign`. MASTER-SPEC §8 still described `git add`/`commit --only` and was aligned. |
| M-3 Unborn / detached HEAD | ADR-012, DR-143, scope.md AC-F020-08, tickets M1-10, M1-28; requirements.md SR-42 (s3) | Unborn: `read-tree --empty`, parentless `commit-tree`, `update-ref HEAD <new> 0000…` (behavior checked with git 2.43.0); detached: refused, changes stay pending; tests `sign::unborn_head_first_commit`, `sign::detached_head_refused`. |
| M-4 Index refresh | ADR-012, DR-143, scope.md AC-F014-06, contracts/files.md (`indexRefreshPending`), design-doc.md, tickets M4-20; requirements.md SR-42 (s3) | 5 × 200 ms retry on `index.lock`, then `indexRefreshPending`, retry on open/focus, banner; test `sign::index_locked_refresh_retried`. |
| M-5 IPC gaps | contracts/ipc.md, design-doc.md, adr/ADR-008, scope.md AC-F026-09, tickets M2-02, M2-25, M3-15, M3-16, M4-03, M4-23, M4-25 | Added `launch_prepare.mode`, `chain_resume_prepare`, `packet_repair_prepare`, `paste_back_preview.source`, `metrics_view`, `project_pin_set` with types, errors, fixtures and AC links (91 commands); AC-F026-09 is the Run log metrics strip. |
| M-6 Kill check | scope.md AC-KC-01, -02, -04, AC-F026-02, -04, -05, metrics-baseline.md, adr/ADR-018, tickets M0-19, M3-26, M3-31; MASTER-SPEC §9 (s2) | Chat end point is "filled prompt on the clipboard and the target chat open" on both sides; active time per step (median per surface); chat baseline ≥ 10 by-hand chat steps between M0 and the kill check, named in M0-19. |
| M-7 Outside signatures | scope.md AC-F013-09, §5 SR-47 row, §7 decision 5, threat-model.md T-085/AB-17 | Counts as signed only after a native dialog binding the packet SHA-256; decision 5 deleted (stub keeps numbering); SR-47 row lists AC-F013-09 and `packet::outside_signature_needs_confirmation`. |
| M-8 Fingerprint caps and symlinks | adr/ADR-021, scope.md AC-F008-24, tickets M0-15, M3-01; requirements.md SR-31, SR-31a (s3) | `~/.claude.json` own 64 MB cap, streamed (`mcpServers` only); outside-repo symlinks followed to regular files outside projects (link and target recorded); repo content stays no-follow; S-7′ measures Billy's `~/.claude`. |
| M-9 pnpm build scripts | adr/ADR-024, scope.md AC-M0-17, tickets M0-06, M0-07; requirements.md SR-61 (s3) | `allowBuilds` default deny plus `strictDepBuilds: true` (exists in pnpm 12.14.0 CHANGELOG; default not readable, so set explicitly); postinstall plant must fail install; strict `minimumReleaseAge` means every pin ≥ 7 days old on M0 day. |
| M-10 PROPOSAL | PROPOSAL.md (s2), domain-rules.md §15 | DR-020…DR-038 named normative in B-1; PDF change from `fm:if` fence splits stated; `in=X` on 1.2; `paste_numbers:multiline`; gate.C `kind=followup`; one `<mode>` list; effort incl. `adaptive`/`ultracode`; unique item IDs; overlay `auto:cmd=` sentence removed; B-3 by-hand commit until Sign ships (P2); 2.3 `followup` explained; standalone steps (DR-007); wait date (DR-067); `BUDGET_TIME` fixture value (DR-083); `cwd` limited to project/spec. |
| M-11 Newer grammar | domain-rules.md DR-021, DR-035, DR-050, adr/ADR-005, scope.md AC-F001-13, contracts/ipc.md and files.md (`needsNewerForeman`), design-doc.md §5.7, tickets M1-12; PROPOSAL B-1 (s2) | Unknown key on a step under a newer grammar → `needsNewerForeman` (shown, not runnable); other unknowns warn; Template health says "Update Foreman". |
| M-12 Dogfooding | scope.md (EX-M4-06 deleted, close-out note), build-prompts.md, tickets M4-35 | Foreman's own Stages 11–17 close out by hand in v1; dogfooding via adoption is v1.1. |
| m-1 Stale departures | design-doc.md (header, §9), adr/README.md, scope.md header and decision 27 | Departure notes removed; §9 says none are open. |
| m-2 IPC tables, probe folder | design-doc.md §3.4, contracts/ipc.md, scope.md (F-019 row, AC-F019-08, decision 4), tickets M3-18 (moved to M4 section, Stage 12), M3-29, M2-16 | Design doc points to ipc.md; `probe_folder_open`, AC-F019-08 and M3-18 are M4. |
| m-3 ADR housekeeping | ADR-005, ADR-007, ADR-013, ADR-017, ADR-004, adr/README.md (ADR-022 row), ADR-022, threat-model.md §7 | Formats inside prompt fences; SR-05 removed list and helper cwd; exact interest-set regexes incl. `lessons.md`; ADR-022 milestones; ADR-017 field names and dist-tags (`next` was 2.1.303; re-checked: all tags 2.1.304 after 19:47 UTC); 80-command trigger re-baselined to 120; "Applied as" column. |
| m-4 process_group(0) | requirements.md SR-57 (s3), scope.md AC-F008-25, AC-XC-31, threat-model.md T-034/RR-16, ADR-007, design-doc.md | Accepted; RR-16 closed. |
| m-5 Notifications P1 | MASTER-SPEC §10 Tauri plugins row (s2; §12 has no notifications row), requirements.md SR-56 (s3), tickets M3-07 | P1 with `wait` steps; a missed wait notifies at next launch. |
| m-6 User exports | contracts/files.md FILE-R9 and inventory rows 27–28, adr/ADR-011, ADR-018, contracts/ipc.md; requirements.md SR-30 (s3) | `WritePurpose::UserExport`: Rust save dialog, `.md` only. |
| m-7 NFR-04 | design-doc.md §3.5.1 and §5.5, scope.md NFR-04 row and AC-F008-13, tickets M3-05 | Measured excluding `claude -v`, which runs in parallel and counts in NFR-05. |
| m-8 Accessibility settings | contracts/ipc.md (`screen_reader_mode`, `os_display`), adr/ADR-014, design-doc.md §5.4, scope.md AC-F008-20, AC-XC-22, AC-XC-23, design/components.md, tickets M3-11, M5-04; MASTER-SPEC argv comment (s2) | Settings toggle drives `--ax-screen-reader` and xterm; transparency and contrast via CSS media queries reported to Rust; AC-XC-23 requires ≥ 7:1 high-contrast tokens (the token values are built in M5-04; today's block is a placeholder). |
| m-9 Session names | design/mockups (6) and _build sources, components.md; MASTER-SPEC (s2) | Dashed per DR-121 (`fm-swings-s07-7-1`, gates `…-s05-5-gate-a`). |
| m-10 Tickets | tickets.md, scope.md M1 entry, design/components.md, design/_build (build.py s4 fixtures, screens/assembler.html), mockups/assembler.html, screenshots, domain-rules.md DR-063; MASTER-SPEC §9 and §17 row (s2) | M1 entry is M1-01's exit item; M2-25 duplicate removed; `Manual: yes` on M0-12…M0-19, M3-30, M3-31; M4-28 has component, mockup (axe clean, both themes) and DR-063 evidence rules (no evidence → undecided), class HARD; estimate restated: 110 tickets before the kill check (111 minus the moved M3-18) ≈ 110–440 h; 8–14 weeks dropped. |
| m-11 Leftover temps | adr/ADR-010, design-doc.md §3.6, contracts/files.md row 27, tickets M1-05; requirements.md SR-32 (s3) | A differing leftover temp moves to `app-data/recovered/` and is shown, never deleted. |
| m-12 `std::fs` modules | scope.md AC-XC-30, adr/ADR-003, design-doc.md, tickets M1-06; requirements.md SR-31a (s3) | Allowed: `store` (incl. `outside`, `git_exclude`), `app::logging`, `foreman-sensor`, test code. |
| m-13 Path with a space | adr/ADR-021, tickets M0-13 (S-1′), M2-09 | S-1′ case and `run_settings::deny_paths_absolute` case added. |
| m-14 `pnpm dev:mock` | scope.md AC-M0-12, build-prompts.md, tickets M0-04 | One script name. |
| m-15 Older builds | adr/ADR-023, tickets M1-02 | Read-only banner from a tolerant header read; test `schema::older_build_header_read_banner`. |
| m-16 Other-branch drift | adr/ADR-022, contracts/files.md (`committedOnOtherBranches`), tickets M4-21 | Committed hash kept per branch; drift uses HEAD's branch entry. |

Coverage after the fix pass (stdlib check over this branch): 160 tickets (M3 30, M4 36, no NEEDS-DESIGN), 380 ACs, 33 exit criteria, 273 named tests (276 with the s3 `requirements.md` merged), 24 ADRs, 106 threats: no orphans, no format or ordering problems.
