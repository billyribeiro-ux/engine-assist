# CR-003: A blueprint pin refuses annotation errors in Stages 1–2 and F.2

STATUS: approved (option B)
REQUESTED BY: the M1 build agent (Known deviations, item 3, `docs/progress.md`) · 2026-10-20
DECIDED BY: Billy Ribeiro · 2026-10-20
Approved by Billy Ribeiro on 2026-10-20 (in chat, 12:38 ET)

## What changes

`lock::pin` (`crates/foreman-core/src/lock.rs`) refuses to pin while any step in the **P1 scope** has an annotation error. The P1 scope is Stages 1–2 and F.2, defined in code as the steps of stages 1 and 2, including their `1:F.2` and `2:F.2` instances (`template::P1_STAGES`; DR-012).

- **What counts.** Every error diagnostic (never a warning or lint) that makes a P1 step not runnable under DR-037 scoping: an error in Stage 1 or 2; an error in the F.2 template or in the Stage Gate templates (DR-037 marks every gate and F.2 instance invalid, `1:F.2` and `2:F.2` among them); and an error that makes every step not runnable (no `fm:grammar` line, the depth limit, an error outside any stage or template section).
- **What it returns.** The new `LockError::AnnotationErrors { problems }`. Each problem carries the parser's diagnostic (rule, file, line, column, message), the stage it sits in and the annotation ID of the step that holds the line. The list is in line order. The UI shows the list; nothing is written (no lock file, no stored text), and an existing lock stays as it was.
- **The message** is plain and specific. One error: `Stage 2 step 2.3 has an annotation error on line 708: cwd is required for cli. Fix the blueprint, then pin again.` (the parser's own "2.3: " prefix is dropped when the step is named). Several errors: a count line, one `- Stage N step X, line L: …` line per error (the first eight, then "and N more"), then the same closing sentence.
- **Order of refusals** in `pin`: a closed re-pin window (AC-F002-03), then a file that differs from HEAD (DR-142), then annotation errors.
- **Unchanged.** Errors that sit only in other stages (0, 3–17) still pin. Those stages' steps, and their F.2 or gate instances, stay not runnable (DR-037, AC-F001-15), exactly as before.

## Why

Known deviation 3 (`docs/progress.md`): `lock::pin` pinned whatever parsed. A pinned blueprint with a broken P1 step cannot be repaired by editing the blueprint, because a re-pin is allowed only until Stage 1 is signed (AC-F002-03, "upgrading the pin is v1.1"). The Stage 1 exit criterion already expects Stages 1–2 to parse with zero errors, but nothing enforced it at the one moment the pin is still cheap to redo. No document required the refusal, and AC-F001-15 (errors in an unused stage are listed and the project opens) still holds for every other stage.

## Impact
- Scope / feature catalog (F-IDs): F-002 (pin). AC-F002-02 gains the second refusal. AC-F001-15 unchanged.
- ADRs (new, amended, superseded): none. ADR-022 (lock handling) is unaffected.
- Threat model: none. The refusal runs before any write, and the diagnostics are blueprint text shown as plain text (SR-26).
- Data model and migrations: none. `blueprint.lock.json` is unchanged.
- API contract and versioning: `docs/contracts/ipc.md` has no `blueprint_pin` command. The pin is reached through `setup_preview` (its pin preview) and `blueprint_repin`, so both Err lists gain the refusal, and §3's `IpcErrorDetail` gains `AnnotationErrors { errors: Vec<AnnotationProblem> }` under code `NotAllowed`. `src-tauri` has no pin command yet (M2), so the mapping and its ts-rs binding land with that command.
- Pricing and entitlements: none.
- Timeline: none. The change is in M1's lock module and tests.
- Cost (build + run): one extra pass over the diagnostics at pin time; no new dependency.

## Recommendation
APPROVE option B. Option A (keep the as-built rule and only document it) leaves the unrepairable-pin trap. Refusing every stage (option C) would contradict AC-F001-15 and block a pin over stages the first release never runs. Option B is limited to the P1 scope.

## Deltas for Stage 4 (applied 2026-10-20, each edit marked "(CR-003)")
- docs/scope.md: AC-F002-02 (second refusal, and what is not refused). Done.
- docs/adr/: none.
- docs/threat-model.md: none.
- docs/migrations/: none.
- docs/tickets.md: M1-17 (acceptance criteria note, likely files, Required tests). Done.
- docs/domain-rules.md: DR-155 (the as-built rule replaced by the refusal). Done.
- docs/contracts/ipc.md: §3 `IpcErrorDetail`, `setup_preview` and `blueprint_repin` Err lists. Done.
