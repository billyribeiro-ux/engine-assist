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
