# Foreman · Domain rules

**Status:** DRAFT for the Stage 4.7 review · 2026-10-19 · Blueprint stage 4.4
**Inputs:** `docs/spec-input/claude-build-blueprint.md` (v2.1, 2,235 lines; quoted below with its line numbers as `bp:NNN`), `spec/blueprint-v2.2/PROPOSAL.md` (grammar v1, B-1…B-8), `docs/MASTER-SPEC.md` §3–§9, `spec/council/r6-feature-specs.md`, `spec/council/r2-proposals.md` P-01, `docs/design-doc.md`, ADR-005, ADR-008, ADR-010, ADR-012, ADR-021, ADR-022, `docs/requirements.md` (revised after the threat model), `docs/scope.md`.

Every rule has an ID (`DR-NNN`), exactly one owning module (named as in design doc §3.2), at least one worked example from the real blueprint text, and the test that proves it. A rule's test lives in its module's test suite. Where a v2.2 annotation is shown, it's the **reference annotation** the Stage 6 annotation ticket writes into the blueprint (PROPOSAL B-1 says only Foreman's parser can prove each annotation loads cleanly); the golden test `template::golden_v2_2` (AC-F001-01, -02) pins the parse of the annotated file.

Modules used: `template`, `template::grammar`, `lock`, `rules`, `policy`, `resolver`, `sanitize`, `steps`, `chain`, `launch`, `packet`, `git`, `decisions`, `pathpolicy` (all in `foreman-core`), `app::rearm`, `app::watch` (in `foreman-app`).

**Counts:** 129 rules (§1–§14); bracket-token inventory of Stages 0–5, the Stage Gate and F.2: **22 distinct tokens** in v2.1 (51 occurrences) plus the `[optional: …]` part B-5 adds in v2.2 (§6, DR-082).

---

## 1. Blueprint structure (`template`)

**DR-001 · `template` · Source normalization and the source hash.** Before parsing, the text loses a leading BOM and every CRLF or lone CR becomes LF. `sourceSha256` is SHA-256 over that text with each TAB also expanded to spaces up to the next multiple of 4 columns (CommonMark's tab stop), so CRLF, BOM and tab variants pin identically. Parsing uses the BOM/CRLF-normalized text with tabs intact.
*Example:* the v2.1 file saved with CRLF endings and a BOM has the same `sourceSha256` as the original. *Test:* `template::normalized_hash_crlf_bom_tabs` (AC-F001-08).

**DR-002 · `template` · Stages.** A stage is a level-2 heading `## Stage N: <title>` with `N` in 0–18; stages appear once each, in ascending order. `# PART …` headings and all other level-2 headings (`## How this blueprint works`, `## The Stage Gate: …`, `## Appendix …`) are not stages.
*Example:* `bp:505 ## Stage 2: Architect's Design Studio. Design the entire product` → stage 2, title "Architect's Design Studio. Design the entire product". *Test:* `template::stage_headings`.

**DR-003 · `template` · Step headings.** Inside a stage, `### Prompt N.M: <title>` opens a step whose `N` must equal the stage number; `### Council setup (once): …` (Stage 2 only) opens the Council setup step; `### Stage N gate` marks the stage's gate instance (DR-011). In `## The Stage Gate`, `### Gate prompt A|B|C: <title>` open the three gate templates; `### F.2 Lessons in the chat stages (0–4)` in Appendix F opens the F.2 template. Every other level-3 heading (`### Dogfood, then beta`) is prose.
*Example:* `bp:663 ### Prompt 2.3: Feature specifications` → number `2.3`; `bp:252 ### Gate prompt A: Inspection` → template `gate.A`. A `### Prompt 3.1:` heading under Stage 2 is error `structure.step-stage-mismatch`. *Test:* `template::step_headings`.

**DR-004 · `template` · RUN WITH blockquotes.** A RUN WITH is a blockquote whose first paragraph begins with the strong text `RUN WITH`. It belongs to the closest preceding step heading. Its text feeds only the prose lint (§3) and the chat chips (DR-043); no value Foreman acts on comes from it (R-05).
*Example:* `bp:630 > **RUN WITH** · Claude Code CLI in `[app]-spec/` · Chair: **Fable 5.1** · effort **high** (Max plan) — or **Opus 5.5** · effort **xhigh** (Pro plan / to save usage) · **Plan** mode first, then **Accept edits** · agents: all six council agents`, followed by `bp:632`–`bp:633` (the two `claude …` command lines inside the same blockquote). *Test:* `template::run_with_attached_to_step`.

**DR-005 · `template` · Step IDs and step keys.** A step's identity is its annotation `id`. IDs match `^(?:[0-9]{1,2}\.[0-9]{1,2}[a-z]?|council-setup|gate\.[ABC]|F\.2)$` and are unique in the template (`grammar.duplicate-id`). Steps without a RUN WITH that the blueprint describes in prose take the letter-suffixed ID of the prompt they follow (`1.3b`, `2.4b`, `5.1b`; B-7's `18.3b` is the precedent). Gate and F.2 templates are instantiated per stage (DR-011, DR-012); an instance's **step key** is `<stage>:<id>` (`5:gate.A`, `2:F.2`). Overlay steps use `x-<stage>-<n>` (DR-059). Step keys are what app data and IPC use (`docs/contracts/ipc.md` §2.1).
*Example:* `bp:437 ### Prompt 1.3: Demand validation kit` → `1.3`; the paragraph `bp:451 Run the test yourself, then paste the numbers into the same chat.` → wait step `1.3b`. *Test:* `template::step_keys`, `grammar::duplicate_id_error`.

**DR-006 · `template` · `fm:step` placement under a RUN WITH.** For a step with a RUN WITH, its `fm:step` is the `HtmlBlock` node that is the RUN WITH blockquote's next sibling and starts on the line after the blockquote's last line (no blank line, PROPOSAL B-1). CommonMark ends the blockquote there because an HTML-comment block (type 2) interrupts a paragraph, so comrak emits a separate `HtmlBlock` (comrak 0.56.0 `NodeValue::HtmlBlock`, `Ast::sourcepos`). A blank line between them, or an `fm:step` anywhere else under that heading, is `grammar.misplaced-step`.
*Example (v2.2):*
```markdown
> **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits**
<!-- fm:step id=2.3 kind=followup tag=core surface=cli session=continue chain=2.2
     start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r6-feature-specs.md -->
```
*Test:* `template::fm_step_must_follow_run_with`.

**DR-007 · `template` · Steps without a RUN WITH.** `kind=action`, `kind=manual`, `kind=wait`, and a `kind=followup` that the blueprint writes as a bare second prompt, carry a standalone block-level `fm:step` placed directly after the prose paragraph (or HANDOFF blockquote) that describes them. Their order in the stage is source order.
*Examples (v2.2):* after `bp:542 Create these six files. Copy the folder for every future project.` → `<!-- fm:step id=council-setup kind=action tag=core -->`; after `bp:704 Record your final choice and reason at the bottom of `council/r7-stack.md`. …` → `<!-- fm:step id=2.4b kind=manual tag=core out=spec/council/r7-stack.md -->`; after `bp:1081 Approve the plan: Opus hands off and Sonnet executes. Then:` → `<!-- fm:step id=5.1b kind=followup tag=core session=continue chain=5.1 -->`. *Test:* `template::standalone_steps_in_source_order`.

**DR-008 · `template` · Prompt binding.** A `prompt` or `followup` step's prompt is the first fenced code block with info string `text` after its `fm:step`, before the next `fm:step` or step heading. If `fm:if`/`fm:end` blocks (DR-032) directly follow that fence, the prompt continues with the next `text` fence, and so on: a prompt is a maximal run of `text` fences separated only by `fm:if`/`fm:end` blocks, joined by concatenating their literals (each literal ends with LF). Fences with other info strings (`markdown` in Council setup, `handoff` in the relay section) are never prompts. `action`, `manual` and `wait` steps have no prompt; a prompt kind with no fence is `grammar.missing-prompt`.
*Example:* 2.3's prompt is the fence `bp:667`–`bp:674` starting `ultrathink. For each v1 feature, write a specification a senior engineer`. 5.1 has two fences (`bp:1070` and `bp:1083`); the first binds to 5.1, the second to 5.1b. *Test:* `template::prompt_binding`, `template::prompt_parts_joined`.

**DR-009 · `template` · Inspection lists.** An INSPECTION is a blockquote whose first paragraph begins with strong `INSPECTION:`; its items are the top-level list items of that blockquote, in order. Item text is the item's inline content (DR-014). The `fm:verify` follows it like DR-006.
*Example:* `bp:758 > **INSPECTION: pass before Stage 3**` with five bullets, the first `bp:760 > - The feature matrix covers every serious competitor, with sources`. *Test:* `template::inspection_items`.

**DR-010 · `template` · HANDOFF blockquotes.** A blockquote beginning with strong `HANDOFF` is recorded as the stage's handoff note (shown in the stage view and the knowledge manifest); Foreman takes no value from it.
*Example:* `bp:768 > **HANDOFF** → the Master Spec Packet + `MASTER-SPEC.md``. *Test:* `template::handoff_note_is_display_only`.

**DR-011 · `template` · Gate instances.** A stage with a `### Stage N gate` heading (Stages 5–17) instantiates `N:gate.A`, `N:gate.B`, `N:gate.C` after its last prompt step, with the INSPECTION under that heading as the stage's inspection list. Stages 0–4 have no gate instances; their INSPECTION lists are attested directly.
*Example:* `bp:1143 ### Stage 5 gate`, `bp:1145 Run the **Stage Gate** (prompts A, B, and C) with these items:`, then `bp:1147 > **INSPECTION: pass before Stage 6**` → steps `5:gate.A`, `5:gate.B`, `5:gate.C` and items `fresh-clone`, `ci-gates`, `staging`, `adr-check`. *Test:* `template::gate_instances_stages_5_to_17`.

**DR-012 · `template` · F.2 instances.** The F.2 template is instantiated as `N:F.2` for Stages 0, 1, 2, 3 and 4, after the stage's packet-producing step (DR-053). Stages 5–17 capture lessons with `N:gate.C` instead (`bp:2151 Run **Gate prompt C** at the end of every stage.`).
*Example:* Stage 2 gets `2:F.2`, which continues the 2.2 chain (MASTER-SPEC §8: "F.2 as a followup in the 2.2 chain"). *Test:* `template::f2_instances_stages_0_to_4`.

**DR-013 · `template` · Needs annotation.** A step heading with no `fm:step` is `needs-annotation`: listed, never runnable, never on Today. A stage whose prompt steps are all `needs-annotation` shows as "needs annotation" (Stages 6–18 in v2.2).
*Example:* `bp:1168 ### Prompt 6.1: Migrations` has no `fm:step` in v2.2 → `needs-annotation`. *Test:* `template::missing_annotation_not_runnable` (AC-F001-04).

**DR-014 · `template` · Item text and its hash.** An item's text is the concatenation of its inline nodes' literal text (code spans without backticks, link text without the URL, emphasis and strong markers dropped, soft and hard breaks as one space), then Unicode lowercase, whitespace runs collapsed to one space, trimmed. `textSha256` is SHA-256 of that string as UTF-8.
*Example:* `bp:1149 > - A fresh clone, then `install` and `dev`, works with zero manual steps` → `a fresh clone, then install and dev, works with zero manual steps` → `6cfd6dda92a916f9c1c1b4f128dcd1839aacca3ed19508ec22e7d1cfb6b57640`. *Test:* `template::item_text_hash_golden` (the value above is the golden).

**DR-015 · `template` · Resource bounds.** The parser enforces a nesting depth of 64 for Markdown containers and for annotation expressions (SR-31), a 20,000-line soft limit (warning above it; NFR-24), and never panics: every problem is a `Diagnostic { file, line, column, rule, message }` with comrak's 1-based line and byte column.
*Example:* 70 nested `>` blockquote markers → `structure.depth-limit` at line 1. *Test:* `template::deep_nesting_bounded`, `template::never_panics` (AC-F001-06).

**DR-016 · `template` · `bypassPermissions` is banned everywhere.** The string `bypassPermissions` (ASCII case-insensitive) anywhere in the blueprint source, or in an overlay step, is error `grammar.bypass-permissions` (SR-03). Prose like `bp:123 | **Bypass** | No prompts | Containers or VMs only, never your machine |` doesn't contain the token and is fine.
*Example:* `start=fable@high/bypassPermissions` → error at its column. *Test:* `template::rejects_bypass_permissions` (AC-F001-10, AC-F025-05).

**DR-017 · `template` · Packet formats.** A packet format starts at a line `=== HANDOFF PACKET · Stage <a> → Stage <b> ===` **inside a `text` fence that belongs to a step** and runs to the first blank line or the end of the fence. Each line `KEY: <placeholder>` at column 1 defines a required key, in order; indented lines continue the previous key's placeholder. The format belongs to the stage of its step; the Gate B format is the generic format for every stage 5–17. The relay example in `bp:28` (a `handoff` fence outside any stage) is ignored. Keys are kept exactly, including case.
*Examples:* Stage 0's format is `bp:358`–`bp:371` (keys `PROJECT`, `APPROVED BY`, `IDEA`, …, `NEXT`), inside 0.2's prompt. Stage 2's is `bp:741`–`bp:753` with keys including `NFRs` and `FILES`. Stage 4's (`bp:1011`–`bp:1033`) has no `NEXT` key; `REPO FILES (create exactly these)` continues over the indented lines `bp:1015`–`bp:1028`. Gate B's is `bp:275`–`bp:285`, ending before the blank line at `bp:286`. *Test:* `template::packet_formats_stages_0_to_4_and_gate`.

## 2. The `fm:` grammar v1 (`template::grammar`)

**DR-020 · `template::grammar` · Lexical form.** A directive is an `HtmlBlock` whose literal starts with `<!-- fm:` and ends with `-->`; it may span lines. After `fm:<name>` come whitespace-separated tokens. `fm:step`, `fm:verify` and `fm:grammar` tokens are `key=value` (key `[a-z-]+`, value without whitespace); `fm:if` takes one expression; `fm:end` takes none. An `fm:` comment inside a list item, a blockquote, a paragraph (inline HTML) or a fence is `grammar.not-block-level` ("annotations must be block-level").
*Example:* the two-line 2.2 comment in PROPOSAL B-1 parses into one directive. `> - item <!-- fm:verify a=attest -->` → `grammar.not-block-level` at its line. *Test:* `grammar::block_level_only` (AC-F001-12).

**DR-021 · `template::grammar` · Grammar version.** The first `HtmlBlock` after the title must be `<!-- fm:grammar v=<n> -->` with `n ≥ 1`. Missing → `grammar.missing-version` (the whole template is not runnable). `v=1` is grammar v1 (this section). For `v>1` (newer than this Foreman supports), a step whose `fm:step` has a key v1 doesn't know becomes `needsNewerForeman` (shown, never runnable); unknown directives and keys elsewhere are warnings; known ones are read as v1 (DR-035). Template health then says "This blueprint uses grammar v<n>; update Foreman to run every step".
*Example (v2.2):* line 2 of the annotated file, after `bp:1 # The Claude Build Blueprint`: `<!-- fm:grammar v=1 -->`. *Test:* `grammar::version_required`.

**DR-022 · `template::grammar` · `fm:step` keys.** Keys, value grammar and defaults:

| Key | Values | Default | Repeatable | Allowed on kinds |
|---|---|---|---|---|
| `id` | DR-005 | required | no | all |
| `kind` | `prompt` `followup` `action` `manual` `wait` `repeatable` | `prompt` | no | — |
| `tag` | `core` `conditional` `optional` | `core` | no | all |
| `when` | expression (DR-027) | none | no | all; required with `conditional`, forbidden with `core` |
| `surface` | `chat` `cli` `desktop` | required | no | prompt, followup, repeatable (inherited from the chain when `session=continue`, DR-029) |
| `cwd` | `project` `spec` (`blueprint`, `worktree` → `grammar.v1-1-cwd`) | required for cli/desktop | no | cli, desktop only |
| `session` | `new` `continue` | `new` | no | prompt kinds |
| `chain` | a step ID, `gate.A` inside a gate, or `@stage` | nearest earlier prompt step of the stage, with lint `lint.implicit-chain` | no | `session=continue` only |
| `start` | `<model>@<effort>[/<mode>]` (DR-023) | required, except inherited by `chain=@stage` | no | prompt kinds |
| `switch` | `<trigger>:<model>@<effort>/<mode>` | none | yes | cli |
| `alt` | `<rule>:<model>@<effort>` (DR-024) | none | yes | prompt kinds |
| `in` | `<name>:<type>[:remember]` (DR-025) | none | yes | prompt kinds |
| `out` | comma-separated repo paths (DR-026) | none | no | prompt kinds, manual |
| `appends` | one repo path | none | no | prompt kinds |
| `requires` | comma-separated repo paths | none | no | all |

A key on a kind it isn't allowed on is `grammar.key-not-allowed`; a repeated non-repeatable key is `grammar.duplicate-key`.
*Example:* `<!-- fm:step id=council-setup kind=action tag=core start=opus@high/plan -->` → `grammar.key-not-allowed` (`start` on `action`). *Test:* `grammar::key_table`.

**DR-023 · `template::grammar` · Run-config values.** `<model>` ∈ `fable opus sonnet haiku opusplan`. `<effort>` ∈ `low medium high xhigh max ultracode` for cli/desktop (the Claude Code model-config docs list `low`…`max` for Fable 5.1 and the 5.5 models, and `ultracode` for `--effort` from v2.1.203; checked 2026-10-19) and `adaptive` only for chat. `<mode>` ∈ `plan default manual acceptEdits auto dontAsk`, where `manual` is stored as `default`; chat steps have no mode (`grammar.mode-on-chat`); cli/desktop steps must have one (`grammar.mode-required`). Anything else is `grammar.unknown-value`.
*Examples:* `bp:1066 > **RUN WITH** · CLI · **opusplan** (Opus plans, Sonnet executes) · effort **medium** · **Plan** mode` → `start=opusplan@medium/plan`. `bp:394 > **RUN WITH** · Claude chat, new Project `[APP_NAME]` · **Opus 5.5** · effort adaptive · web search ON` → `start=opus@adaptive`. `bp:721 … **Manual** mode (Shift+Tab)` → `start=fable@high/manual`, stored as mode `default`. *Test:* `grammar::start_vocabulary` (AC-F001-09).

**DR-024 · `template::grammar` · Alternative rules.** `alt=<rule>:<model>@<effort>` with `<rule>` ∈ `pro-plan` `max-plan` (selected by the project's `modelPolicy.plan`, DR-101). Other rule names are `grammar.unknown-alt-rule`. Judgment alternatives in prose ("switch to Fable 5.1 if the system is platform-scale", `bp:874`; "Fable 5.1 if available, otherwise Opus 5.5", `bp:979`) are not alternatives: they're handled by a substitution rule or a per-run override (DR-102, DR-103).
*Example:* `bp:630 … Chair: **Fable 5.1** · effort **high** (Max plan) — or **Opus 5.5** · effort **xhigh** (Pro plan / to save usage)` → `start=fable@high/plan alt=pro-plan:opus@xhigh`. *Test:* `grammar::alt_rules_closed_set`.

**DR-025 · `template::grammar` · Run inputs.** `in=<name>:<type>[:remember]`, `<name>` matching `^[A-Za-z][A-Za-z0-9_]{0,63}$`, `<type>` ∈ `text multiline`. A run input binds to the prompt token whose inner text, with each space replaced by `_`, equals `<name>` exactly. A declared input with no matching token, or a run-input token (DR-082) with no declaration on its step, is `grammar.input-binding`.
*Examples:* `bp:426 We're going with concept [X] (with these changes: [changes]). Now` → on 1.2: `in=X:text in=changes:text`. `bp:458 Here are the validation results: [paste numbers]. If they missed the` → on 1.4: `in=paste_numbers:multiline`. `bp:810 3. Legal and compliance for selling to [TARGET_USERS] in [jurisdictions]:` → on 3.2: `in=jurisdictions:text:remember`. (PROPOSAL B-1 now puts `in=X:text` on 1.2, where `[X]` appears.) *Test:* `grammar::input_binding`.

**DR-026 · `template::grammar` · Annotation paths.** `out`, `appends` and `requires` values are repo-relative paths (never relative to `cwd`), comma-separated, no spaces, checked with `pathpolicy::check_write(path, TemplateOut)` for `out`/`appends` and `check_read` for `requires` (DR-160). In the gate and F.2 templates only, the literal `NN` in a `stage-NN` file name is replaced at instantiation by the two-digit stage number.
*Examples:* 2.2's `out=spec/council/r1-market.md,…` (PROPOSAL B-1) while its prompt says `council/r1-market.md` relative to `spec/`; Gate B (v2.2): `out=docs/handoff/stage-NN.md` → for `5:gate.B`, `docs/handoff/stage-05.md`. `out=../docs/x.md` → `grammar.path-policy` naming the rule. *Test:* `grammar::paths_repo_relative`, `grammar::nn_instantiation`.

**DR-027 · `template::grammar` · Expressions.** `when` and `fm:if` use:
```text
expr = or ;  or = and { "|" and } ;  and = not { "&" not } ;  not = "!" not | atom ;  atom = ID | "(" expr ")"
ID   = idea_known | commercial | payments | multi_user | public | has_ui | others_data | server
     | has_database | has_uploads | has_email | has_integrations
```
Precedence `!` > `&` > `|`; no spaces inside a `when=` value; nesting depth ≤ 64. Any other identifier is `grammar.unknown-condition`; a syntax error is `grammar.bad-expression` with the column. (`commercial\|payments` in PROPOSAL B-1's table is Markdown table escaping; the annotation is `when=commercial|payments`.)
*Example:* 3.1 (v2.2): `when=commercial|payments`; 3.2's legal part: `<!-- fm:if commercial|others_data|multi_user -->`; Stage 0: `when=!idea_known`. *Test:* `grammar::expression_parser` (property: print/parse round trip).

**DR-028 · `template::grammar` · Sessions and chains.** `session=continue` needs a chain: `chain=<id>` names an earlier step of the same stage (or `gate.A` inside a gate instance), or `chain=@stage` (F.2 only) names the chain of the stage's packet-producing step (DR-053). An unknown or later step is `grammar.bad-chain`; `chain` with `session=new` is `grammar.chain-without-continue`; `chain=@stage` outside F.2 is `grammar.stage-chain-outside-f2`.
*Examples:* 2.3 `session=continue chain=2.2`; Gate C `session=continue chain=gate.A`; F.2 `chain=@stage`. A `chain=2.4` on 2.3 → `grammar.bad-chain` (later step). *Test:* `grammar::chain_validation` (AC-F001-09).

**DR-029 · `template::grammar` · Inheritance along a chain.** A continuing step inherits its chain's `surface` and `cwd`; declaring a different one is `grammar.chain-cwd-mismatch` / `grammar.chain-surface-mismatch`. `chain=@stage` steps (F.2) must omit `surface`, `cwd` and `start`; they take the chain's surface and cwd and its last effective start (in Stage 2 that's the CLI council chain; in chat stages a chat chain).
*Example:* `bp:665 > **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits**` → 2.3 omits `cwd` and gets `spec` from 2.2. `<!-- fm:step id=2.3 … cwd=project -->` → `grammar.chain-cwd-mismatch`. *Test:* `grammar::continuing_inherits_cwd_surface`.

**DR-030 · `template::grammar` · `fm:verify`.** Directly after an INSPECTION blockquote (DR-006 placement). One `<itemId>=<kind>` entry per bullet, in bullet order; the count must equal the bullet count (`grammar.verify-count`).
*Example (v2.2, Stage 5, PROPOSAL B-1):* `<!-- fm:verify fresh-clone=attest ci-gates=attest staging=attest adr-check=attest -->` under the four bullets `bp:1149`–`bp:1152`. Three entries under four bullets → `grammar.verify-count`. *Test:* `grammar::verify_count` (AC-F001-09).

**DR-031 · `template::grammar` · Item kinds.** `attest`; `time:<n>d` with `1 ≤ n ≤ 365`, optionally `time:<n>d>attest`; `auto:cmd=<id>` is reserved for v1.1: v1 reports warning `grammar.auto-needs-v1-1` ("auto items need Foreman v1.1") and treats the item as `attest`.
*Example:* `adr-check=auto:cmd=adr-lint` → warning, kind `attest`. *Test:* `grammar::auto_treated_as_attest` (AC-F001-11).

**DR-032 · `template::grammar` · Conditional parts.** `<!-- fm:if <expr> -->` and `<!-- fm:end -->` are block-level directives between the `text` fences of one prompt (DR-008). Each `fm:if` needs a matching `fm:end` before the prompt ends; `fm:if` inside an open `fm:if` is `grammar.nested-if`; an unmatched one is `grammar.unmatched-if`. The fences between them form the part.
*Example (v2.2, 3.2):* the fence `bp:802`–`bp:816` becomes three fences: lines 803–809 (items 1–2), then `<!-- fm:if commercial|others_data|multi_user -->`, a fence with `bp:810`–`bp:813` ("3. Legal and compliance for selling to [TARGET_USERS] in [jurisdictions]: …"), `<!-- fm:end -->`, and a fence with `bp:814`–`bp:831`. *Test:* `grammar::fm_if_blocks` (AC-F001-14).

**DR-033 · `template::grammar` · Optional parts.** Inside a prompt, `[optional: …]` (the text after `[optional:` up to the matching `]`, which may span lines) is an optional part. Parts are numbered `opt-1`, `opt-2`, … in order within the step. Nesting `[optional:` inside one is `grammar.nested-optional`; an unclosed one is `grammar.unclosed-optional`.
*Example (v2.2, 1.4, PROPOSAL B-5):* `Otherwise, [optional: write a press release for launch day as if the` … `ask, with answers.]` → `opt-1`. *Test:* `grammar::optional_parts`.

**DR-034 · `template::grammar` · Item IDs.** Item IDs match `^[a-z][a-z0-9-]{0,47}$` and are unique across the whole template (`grammar.duplicate-id`), because the lock keys items by ID alone (`docs/contracts/files.md` §3.3). Reordering bullets with their IDs keeps each ID with its text.
*Example:* Stage 5's `staging` and a later stage's `staging` → duplicate; the later one must be named, e.g., `staging-load`. *Test:* `grammar::item_ids_unique`.

**DR-035 · `template::grammar` · Unknown directives and keys.** Within the declared grammar version, an unknown directive (`fm:foo`) or key is an error. When the declared version is higher than the parser's: an `fm:step` with an unknown key makes that step `needsNewerForeman` (listed and shown, never runnable, never on Today; Foreman can't know what the key changes); an unknown key on `fm:verify` or an unknown directive is warning `grammar.newer-grammar` and the rest is read as v1 understands it. Template health shows "Update Foreman" with the count of affected steps.
*Example:* `<!-- fm:step id=1.1 surfaces=chat -->` → `grammar.unknown-key` under `v=1`; under `v=2`, step 1.1 is `needsNewerForeman` and Template health says "Update Foreman: 1 step needs a newer version". `<!-- fm:note text=x -->` under `v=2` → warning only. *Test:* `grammar::unknown_key_by_version` (AC-F001-13).

**DR-036 · `template::grammar` · Action vocabulary.** An `action` step's behavior comes from a Rust table keyed by step ID (SR-34), never from text: `council-setup` → `write-blocks` (the six `markdown` fences under `### Council setup` → `spec/.claude/agents/<name>.md`, names from each fence's `name:` line, which must be one of `market-analyst user-advocate product-visionary principal-engineer growth-strategist red-team`); `1.4b` → `copy-file` (`docs/handoff/stage-01.md` plus its attachments → `spec/01-app-brief.md`, PROPOSAL B-8). An action step whose ID isn't in the table is `grammar.unknown-action`.
*Example:* `bp:546 name: market-analyst` in the first fence → `spec/.claude/agents/market-analyst.md`. A fence with `name: ../../evil` → `grammar.unknown-action-target`. *Test:* `actions::destinations_fixed` (AC-F007-02).

**DR-037 · `template::grammar` · Diagnostic catalog.** Every error and warning has a stable rule code (the codes used in this section and §3), a file, line and column. Errors in a stage make only that stage's steps not runnable; the project still opens (AC-F001-15).
*Example:* a `grammar.bad-chain` in Stage 4 leaves Stages 1–2 runnable. *Test:* `template::errors_scoped_to_stage`.

**DR-038 · `template::grammar` · Reference annotations (Stages 0–5, Gate, F.2, Council setup).** The annotated v2.2 text must produce exactly these steps (prompt-step lines are `fm:step` under the RUN WITH quoted; standalone ones per DR-007). This table is the expected half of `template::golden_v2_2`.

| Key | Source (`bp:`) | Annotation (v2.2) |
|---|---|---|
| 0.1 | 318 `Claude chat, new Project "Opportunity Research" · **Opus 5.5** · effort adaptive · **Research ON**` | `id=0.1 tag=optional when=!idea_known surface=chat session=new start=opus@adaptive` |
| 0.2 | 348 `same chat · **Opus 5.5** · effort adaptive` | `id=0.2 tag=optional when=!idea_known session=continue chain=0.1 start=opus@adaptive out=docs/handoff/stage-00.md` |
| 1.1 | 394 | `id=1.1 tag=core surface=chat session=new start=opus@adaptive` |
| 1.2 | 423 `same chat` | `id=1.2 tag=core session=continue chain=1.1 start=opus@adaptive in=X:text in=changes:text` |
| 1.3 | 439 | `id=1.3 tag=conditional when=commercial session=continue chain=1.1 start=opus@adaptive` |
| 1.3b | 451 (prose) | `id=1.3b kind=wait tag=conditional when=commercial` |
| 1.4 | 455 | `id=1.4 tag=core session=continue chain=1.1 start=opus@adaptive in=paste_numbers:multiline out=docs/handoff/stage-01.md` |
| 1.4b | 501 (HANDOFF, B-8 text) | `id=1.4b kind=action tag=core` |
| 2.1 | 516 | `id=2.1 tag=conditional when=commercial surface=chat session=new start=opus@adaptive out=spec/02-teardown.md` |
| council-setup | 542 (prose) | `id=council-setup kind=action tag=core` |
| 2.2 | 630 | PROPOSAL B-1 worked example |
| 2.3 | 665 | PROPOSAL B-1 worked example |
| 2.4 | 678 | `id=2.4 kind=followup tag=core session=continue chain=2.2 start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r7-stack.md` |
| 2.4b | 704 (prose) | `id=2.4b kind=manual tag=core out=spec/council/r7-stack.md` |
| 2.5 | 708 | `id=2.5 kind=followup tag=core session=continue chain=2.2 start=fable@high/acceptEdits alt=pro-plan:opus@xhigh out=spec/council/r8-nfr.md` |
| 2.6 | 721 `**Manual** mode (Shift+Tab)` | `id=2.6 kind=prompt tag=core session=continue chain=2.2 start=fable@high/default alt=pro-plan:opus@xhigh out=spec/MASTER-SPEC.md,docs/handoff/stage-02.md` |
| 3.1 | 778 | `id=3.1 tag=conditional when=commercial\|payments surface=chat session=new start=opus@adaptive` |
| 3.2 | 800 `same chat` | `id=3.2 tag=core session=continue chain=3.1 start=opus@adaptive in=jurisdictions:text:remember out=docs/handoff/stage-03.md` |
| 4.1–4.7 | 855, 874, 911, 927, 946, 963, 979 | `id=4.N tag=core surface=chat session=new start=opus@adaptive` (4.5 adds `tag=conditional when=has_ui`; 4.7 `start=fable@adaptive`; `in=list_screens:text` on 4.5) |
| 4.8 | 996 | `id=4.8 tag=core surface=chat session=new start=opus@adaptive out=docs/handoff/stage-04.md` |
| 5.1 | 1066 | `id=5.1 tag=core surface=cli cwd=project session=new start=opusplan@medium/plan switch=plan-approved:opusplan@medium/acceptEdits` |
| 5.1b | 1081 (prose) | `id=5.1b kind=followup tag=core session=continue chain=5.1` |
| 5.2 | 1090 ``/model opus` · `/effort high` · Shift+Tab → **Plan**`` | `id=5.2 tag=core session=continue chain=5.1 start=opus@high/plan switch=plan-approved:opus@high/acceptEdits` |
| 5.3 | 1105 | `id=5.3 tag=core session=continue chain=5.1 start=opus@high/plan` |
| 5.4 | 1117 | `id=5.4 tag=core session=continue chain=5.1 start=sonnet@low/acceptEdits` |
| 5.5 | 1131 | `id=5.5 tag=conditional when=server session=continue chain=5.1 start=opus@high/plan switch=plan-approved:opus@high/acceptEdits` |
| gate.A | 254 | `id=gate.A tag=core surface=cli cwd=project session=new start=opus@high/plan` |
| gate.B | 270 `same session · **Opus 5.5** · effort **medium** · Shift+Tab → **Accept edits**` | `id=gate.B tag=core session=continue chain=gate.A start=opus@medium/acceptEdits out=docs/handoff/stage-NN.md` |
| gate.C | 292 | `id=gate.C kind=followup tag=core session=continue chain=gate.A start=opus@medium/acceptEdits appends=docs/lessons.md` |
| F.2 | 2136 | `id=F.2 kind=followup tag=core session=continue chain=@stage appends=docs/lessons.md` |

`fm:verify` (v2.2): Stage 0 `candidates=attest pick-on-evidence=attest experiment-threshold=attest`; Stage 1 `validation-threshold=attest press-release-tested=attest brief-open-items=attest`; Stage 2 `feature-matrix=attest beats-competitor=attest redteam-resolved=attest stack-choice=attest frozen=attest`; Stage 3 `margin-positive=attest payment-terms=attest requirements-numbered=attest`; Stage 4 `review-clean=attest requirements-mapped=attest formulas=attest tickets-complete=attest contractor-test=attest`; Stage 5 as in DR-030. Stage 2's 2.2 prompt gets `fm:if commercial` parts around `bp:643` (market-analyst) and `bp:647` (growth-strategist) per MASTER-SPEC §5.
*Test:* `template::golden_v2_2` (AC-F001-01, -02).

## 3. Prose lint (`template`)

**DR-040 · `template` · Lint is warnings only.** The r2 P-01 inference rules run over each RUN WITH and produce `lint.*` warnings when they disagree with the annotation. They never supply or change a value (R-05).
*Example:* if a retro changes `bp:665` to "fresh session" and leaves `session=continue`, the step still continues its chain and Template health shows `lint.session-disagrees` at that line. *Test:* `template::prose_change_lints_only` (AC-F001-03).

**DR-041 · `template` · The inference rules.** (1) `same session`/`same chat` ⇒ `session=continue`; `fresh session`, `new session`, `new conversation`, `brand-new conversation` ⇒ `session=new`. (2) ``in `[app]-spec/` `` or ``in `spec/` `` ⇒ `cwd=spec`. (3) `X first, then Y`, `X → Y`, `Shift+Tab → Y` ⇒ `start` mode X and a `switch` to Y. (4) `/model x`, `/effort y`, bold model and `effort **level**` ⇒ `start` model and effort. (5) `— or`, `(or …)`, `if available, otherwise` ⇒ an alternative exists. (6) `-> path`, `Save the result to path`, `Write path` ⇒ an `out` entry. Each mismatch is one warning naming both values.
*Examples:* `bp:1090 > **RUN WITH** · same session · `/model opus` · `/effort high` · Shift+Tab → **Plan**` agrees with `start=opus@high/plan`. `bp:673 -> council/r6-feature-specs.md` agrees with `out=spec/council/r6-feature-specs.md` (lint compares after resolving the prompt path against `cwd`). *Test:* `template::inference_rules` (one case per rule).

**DR-042 · `template` · Followups can't change the session.** A `kind=followup` step is copied into the live session (DR-068), so its `start` should equal its chain's expected live start (the previous step's start after its switches). Otherwise: `lint.followup-changes-start` ("a followup can't change model, effort or mode; use kind=prompt to re-arm").
*Example:* 2.6 (`bp:721`, Manual mode after 2.5's Accept edits) must be `kind=prompt`; annotating it `kind=followup start=fable@high/default` triggers the warning. *Test:* `template::followup_start_lint`.

**DR-043 · `template` · Chat chips come from prose.** The chat setup chips (F-012) are memory aids, so they may come from RUN WITH text: "New conversation in Project <name>" (session new), "Same chat as <chain start>" (bold), the model, and `web search ON` / `Research ON` when those words appear. They never change any value Foreman acts on.
*Example:* `bp:516 > **RUN WITH** · Claude chat, `[APP_NAME]` Project · **Opus 5.5** · effort adaptive · **Research ON**` → chips "New conversation in Project TradeLog", "Opus 5.5", "Research ON". *Test:* `template::chat_chips`.

## 4. Tags, conditions, applicable path, overlay (`rules`)

**DR-050 · `rules` · Step applicability.** For each step, `applicable_path` computes one of `applicable`, `skippable`, `skipped(reason)`, `notApplicable(reason)`, `undecided(condition)`, `needsAnnotation`, `needsNewerForeman` (DR-035), from tag, `when` (three-valued, DR-051) and the overlay:

| Tag | `when` | `when` value | State | Skip control |
|---|---|---|---|---|
| core | — (forbidden) | — | applicable | none (lock: "Core: protects production quality. Change it in the blueprint with a Change Request.") |
| conditional | required | yes | applicable | none |
| conditional | required | no | notApplicable ("Not applicable: … (<cond> = no)") | none |
| conditional | required | undecided | undecided ("Decide <condition> to continue") | none |
| optional | none | — | skippable | Skip with a reason |
| optional | present | yes | skippable | Skip with a reason |
| optional | present | no | notApplicable | none |
| optional | present | undecided | undecided | none |

Gate instances, F.2 instances and packets are core. These states are derived on every read, never stored (`docs/contracts/files.md` §4.3).
*Examples (MASTER-SPEC §5 defaults, Foreman's own conditions: `idea_known` yes, `has_ui` yes, all others no):* 0.1 `optional when=!idea_known` → notApplicable; 1.3 `conditional when=commercial` → notApplicable ("this app isn't commercial (commercial = no)"); 4.5 `conditional when=has_ui` → applicable; 5.5 `conditional when=server` → notApplicable. *Test:* `rules::tag_condition_table` (every row, AC-F004-01), `rules::core_never_dropped` (property, AC-F004-02).

**DR-051 · `rules` · Three-valued evaluation.** `when` and `fm:if` use Kleene logic: `!U = U`; `Y & U = U`, `N & U = N`; `Y | U = Y`, `N | U = U`.
*Example:* 3.1 `when=commercial|payments` with `commercial = yes`, `payments = undecided` → yes (applies). With `commercial = no`, `payments = undecided` → undecided. *Test:* `rules::kleene_truth_table` (exhaustive over 3×3 per operator).

**DR-052 · `rules` · Condition edits.** Early conditions (`idea_known commercial payments multi_user public has_ui others_data server`) can change, each appending `{condition, from, to, at, note}`, until Stage 2 is signed; late conditions (`has_database has_uploads has_email has_integrations`) until Stage 4 is signed. Afterwards: refused with "Change requires a Change Request (v1.1)" and the path `docs/templates/change-request.md`.
*Example:* changing `payments` after Stage 2's packet is signed → refused; changing `has_database` during Stage 4 (the assembler's late-conditions panel) → allowed. *Test:* `rules::condition_edit_windows` (AC-F004-10, -11).

**DR-053 · `rules` · The packet-producing step.** A stage's packet-producing step is the applicable step whose `out` contains `docs/handoff/stage-NN.md` for that stage (for 5–17, `N:gate.B`). A stage must have exactly one (`grammar.packet-producer`).
*Example:* Stage 2 → 2.6 (`out=spec/MASTER-SPEC.md,docs/handoff/stage-02.md`); Stage 0 → 0.2. *Test:* `rules::packet_producer`.

**DR-054 · `rules` · Stage applicability and the next stage.** A stage is not applicable when none of its prompt, followup, action, manual or wait steps is applicable or skippable (all are notApplicable or skipped). It then has no gate, no F.2 and no packet. The "next applicable stage" after N is the lowest applicable stage above N.
*Example:* with `idea_known = yes`, Stage 0 is not applicable (0.1, 0.2 both notApplicable), so the first packet is Stage 1's, and Stage 1's header must say `Stage 1 → Stage 2`. *Test:* `rules::stage_not_applicable` (AC-F004-09).

**DR-055 · `rules` · SKIPPED queue.** Every skipped or not-applicable step and part on a stage's path, and every not-applicable stage since the last signed packet, is queued for the next signed packet's `SKIPPED:` field, with its reason (DR-139).
*Example:* for Foreman, Stage 2's packet lists `2.1 Competitor teardown: not applicable (commercial = no)`; a not-applicable Stage 0 is queued into Stage 1's packet. *Test:* `rules::skipped_queue` (AC-F013-15).

**DR-056 · `rules` · Skips.** Only `skippable` steps can be skipped, with a reason of 1–200 characters stored in `overlay.json` `skips`. A recorded skip on a step that is no longer optional (template change) is ignored and flagged `rules.stale-skip`.
*Example:* skipping 0.1 when `idea_known = no` → allowed with a reason; skipping 2.2 (core) → refused "core-step-skip"; skipping 1.3 (conditional) → refused "conditional-step-skip". *Test:* `rules::skip_rules` (AC-F004-05, -07, -08).

**DR-057 · `rules` · Gate and lessons are core.** Gate instances, F.2 instances, inspection items of an applicable stage and the packet can't be skipped. An inspection item can be attested "not applicable" with a reason (DR-073), which isn't a skip.
*Example:* `staging` (Stage 5) for a project with `server = no` is attested "not applicable: no server, so no staging" instead of skipped. *Test:* `rules::gate_items_unskippable`.

**DR-058 · `rules` · Overlay is project-only add.** The overlay can only (a) skip `skippable` steps, (b) add steps, (c) add inspection items. It can't remove, reorder or edit template steps or items, change tags or conditions, or change any annotation value (F-104 is v1.1).
*Example:* an `overlay.json` entry that tries to set `skips["2.2"]` is ignored and flagged; an unknown top-level key fails the schema. *Test:* `rules::overlay_add_only`.

**DR-059 · `rules` · Added steps.** An added step has ID `x-<stage>-<n>` (`n` = 1 + the highest existing `n` in that stage), sits after the step named in `after`, copies the run config of `copiedFrom`, is tag `optional` unless Billy makes it `core`, always starts a new session (never joins a chain), always confirms before running (`ConfirmReason::OverlayStep`, SR-19) and shows "added for this project".
*Example:* adding a prompt after 2.6 copying 2.6's config → `x-2-1`, `cli`, `cwd=spec`, `start=fable@high/default`, session new. *Test:* `rules::added_step_rules` (AC-F025-01, -03).

**DR-060 · `rules` · Added items.** Added inspection items get ID `xi-<stage>-<n>`, kind `attest` only, appear after the template items, and are part of `[paste the stage's inspection list]` (DR-085) and of `ATTESTED:`.
*Example:* `xi-5-1 "Billing sandbox keys rotated"` on Stage 5. *Test:* `rules::added_item_attest_only` (AC-F025-02).

**DR-061 · `rules` · Added-step validation.** An added step goes through the same grammar checks as a template step: DR-016 (`bypassPermissions`), DR-023 (vocabulary), DR-026 (paths, through the write allowlist).
*Example:* an added step with `out=.github/workflows/x.yml` → refused (allowlist, DR-160). *Test:* `rules::added_step_validated` (AC-F025-05).

**DR-062 · `rules` · Overlay round trip.** Overlay JSON round-trips unchanged; added steps appear in the applicable path in position; packets list them in `SKIPPED:` when skipped.
*Test:* `rules::overlay_roundtrip` (property, AC-F025-04).

**DR-063 · `rules` · Late-condition evidence (assembler prefill, P2).** The build-package assembler (F-023) proposes a value for each late condition that is still `undecided`, from **paths only**: the Stage 4 packet's REPO FILES entries and the paths that exist in the project (never file contents). Matching is on the repo-relative path, lowercased:

| Condition | Evidence that proposes `yes` |
|---|---|
| `has_database` | a path under `docs/migrations/`, or a file ending in `.sql`, or named `schema.prisma` |
| `has_uploads` | a path component containing `upload` or `attachments-storage` |
| `has_email` | a path component containing `email` or `mail` (e.g. `docs/email-templates/`) |
| `has_integrations` | a path under `docs/integrations/`, or a component containing `webhook` or `integration` |

With no evidence the condition stays **`undecided`**; the assembler never proposes `no`. A proposal is shown with its evidence ("from: `docs/migrations/0001.sql`") and takes effect only when Billy confirms it, which records `condition_set` with the note "from evidence: <path>" (DR-052's window applies). A condition Billy already decided is never changed.
*Example:* Foreman's own Stage 4 REPO FILES has no migrations, uploads, email or integration paths → all four stay `undecided` until Billy answers them; TradeLog's fixture lists `docs/migrations/0001_init.sql` → `has_database` proposed `yes`. *Test:* `rules::late_condition_evidence` (AC-F023-06).

## 5. Step kinds, states and close-out (`steps`)

**DR-065 · `steps` · States and transitions.** Stored states: `notStarted → copied` (chat Copy and open; followup copy), `notStarted|copied → running` (CLI spawn), `running → done` (exit 0 of a prompt step, or Mark done for chat), `running → notStarted` (errored or interrupted: run again), `done → running` (re-run; for a chain start it supersedes the chain, DR-112). Derived states (DR-050) override display. Every other (state, event) pair is rejected with a reason; `adopted` is never reachable.
*Example:* Mark done on a `needsAnnotation` step → rejected "needs annotation". *Test:* `steps::transitions_exhaustive` (property, AC-F007-01).

**DR-066 · `steps` · Finishing steps.** Chat `prompt` steps finish by Mark done (⌘D) or a valid paste-back of their `out`; a CLI step's run ends when the process exits with any code, and the code is recorded; only exit 0 makes the step done, and any other code returns it to not started (DR-065); `manual` steps finish when a declared `out` file changes after the step started, or by Mark done; `action` steps finish on a successful apply. A CLI step can't be marked done by hand.
*Example:* 2.4b (`out=spec/council/r7-stack.md`): Claude or Billy edits the file after 2.4b started → done. *Test:* `steps::finish_rules` (AC-F007-04, -08, AC-F012-12).

**DR-067 · `steps` · Wait steps.** A `wait` step becomes `waitingUntil(date)` when Billy starts it with a date (the grammar has no duration); it's runnable again on that date, when a notification fires (Foreman-written text). The step after it is blocked until then.
*Example:* 1.3b after 1.3 (`bp:451 Run the test yourself, then paste the numbers into the same chat.`): Billy enters 2026-10-26 → 1.4 is "Waiting until 2026-10-26". *Test:* `steps::wait_until_date` (AC-F007-05, -09).

**DR-068 · `steps` · Followups.** A `followup` step copies its resolved prompt (sanitized, SR-28) and focuses its chain's terminal tab when the chain's process is live; nothing is typed into the PTY. If the process has exited, the followup runs as a continuing launch (re-arm relaunch, DR-113) with its `start`, or the chain's last start if it has none.
*Example:* 5.1b (`bp:1084 Use the test-runner agent to run the full verification suite and fix`) while 5.1's session runs → copied, tab focused. *Test:* `steps::followup_copy_or_relaunch` (AC-F007-07).

**DR-069 · `steps` · Repeatable steps.** A `repeatable` step never becomes `done` on its own; each run increments `runCount`.
*Example:* a repeatable 18.x step run three times shows "3 runs". *Test:* `steps::repeatable_counter` (AC-F007-06).

**DR-070 · `steps` · Close-out per stage type.** Sign becomes possible only after the stage type's sequence (MASTER-SPEC §8):

| Stage type | Sequence (gate states) |
|---|---|
| Chat stages 0, 1, 3, 4 | packet valid → every applicable item resolved → `N:F.2` pasted back (lessons entry for N) → signable |
| Stage 2 (CLI council) | 2.6 wrote a valid packet → items resolved → `2:F.2` followup in the 2.2 chain wrote a lessons entry → signable |
| Stages 5–17 (CLI) | `notReady → a` (gate.A run) `→ results` (items PASS/attested) `→ b` (gate.B run) `→ packetValid → c` (gate.C run) `→ lessonsFound → signable` |

Out-of-order transitions are rejected. In P1 the same sequence is a `manual` checklist step, and Billy signs by hand (DR-135).
*Example:* Stage 5: running `5:gate.B` before `5:gate.A`'s results are confirmed → rejected "Run Gate A first". *Test:* `steps::closeout_sequences` (AC-F015-01, -08, AC-F012-16).

**DR-071 · `steps` · Signable.** A stage is signable only when: the close-out is complete; every applicable item is resolved (DR-073); a lessons entry for the stage exists (DR-074); the stage's chain is idle or ended, or Billy confirms otherwise in the native dialog; for Stage 2, no early condition is `undecided`; for Stage 4, no late condition is `undecided`; the packet is valid (DR-133).
*Example:* Stage 2 with `server = undecided` → "Decide server before signing Stage 2". *Test:* `gate::signable_requires_all` (property, AC-F014-01, AC-F015-10, AC-F004-13).

**DR-072 · `steps` · Gate item order.** In the gate view: FAIL, then items waiting for attestation, then PASS/done (collapsed). Not-applicable items are excluded from the gate count.
*Test:* `steps::gate_item_order` (AC-F015-06, -09).

**DR-073 · `steps` · Attestation evidence.** `attest`: Done needs a note ≥ 15 characters (after trimming) that, compared lowercased and whitespace-collapsed, equals no other note in the same gate; or Not applicable with a reason (≥ 1 character). `time:<n>d`: available only `n` days after its start (the stage's first run), then as declared (`>attest`). A gate.A FAIL can be retried; the third FAIL of one item shows the F.4 prompt (`bp:2160 Lesson [L-NNN] has now happened twice. …`).
*Example:* two items with the note "checked it, all good" → second refused "note-repeated". *Test:* `steps::attest_note_rules` (AC-F015-02, -03, -07).

**DR-074 · `steps` · Lessons requirement.** A stage's lessons requirement is met by an entry in `docs/lessons.md` whose heading matches `^## L-(\d{3,}) · Stage (\d{1,2}) · ` with that stage's number; a "clean stage" entry in the same heading form counts. The next free ID is `L-` + (highest number + 1), zero-padded to 3.
*Example:* Foreman's `docs/lessons.md` has `## L-006 · Stage 2 · 2026-10-19 · The chair's resolution didn't fully reach the frozen documents` → Stage 2 is met; the next ID is `L-007`. *Test:* `steps::lessons_entry_detected` (AC-F016-05, -06).

## 6. Prompt resolution (`resolver`, `sanitize`)

**DR-080 · `resolver` · Six classes, closed table.** At parse time every bracket token in prompt text is classified into exactly one class: project variable, packet marker, path alias, run input, optional part, literal. Classification uses a closed table of exact token texts (DR-082) plus two syntactic rules (DR-081). A bracket token in a prompt that's neither in the table nor matched by DR-081 is template error `resolver.unknown-token`, never a guess.
*Example:* a retro adding `[paste the threat model]` to a prompt → `resolver.unknown-token` until Foreman's table learns it. *Test:* `resolver::unknown_token_is_error` (AC-F006-02).

**DR-081 · `resolver` · Token syntax.** A token is `[` … `]` with no line break inside (except `[optional: …]`, DR-033) and no nested `[`. It's a **literal** by syntax when (a) it is immediately followed by `(` (a Markdown link), or (b) it is immediately preceded by a letter or digit (`sonnet[1m]`). Tokens are only scanned in prompt text (DR-008), never in RUN WITH, prose or `markdown` fences.
*Example:* `bp:39`'s `docs/handoff/stage-NN.md` has no brackets; `[1m]` in `sonnet[1m]` → literal. *Test:* `resolver::token_syntax`.

**DR-082 · `resolver` · Inventory of the real blueprint (Stages 0–5, Stage Gate, F.2).** Every distinct bracket token, with its class and resolution. Lines are `bp:` lines of v2.1 where the token appears in prompt text (P) or other text (o).

| # | Token | Class | Lines | Resolution |
|---|---|---|---|---|
| 1 | `[APP_NAME]` | project variable | P 259, 276, 469, 636, 681, 742, 820, 859, 950, 1012; o 382, 394, 516, 756, 778, 851 | `variables.APP_NAME` |
| 2 | `[MY_ADVANTAGES]` | project variable | P 326, 405 | `variables.MY_ADVANTAGES` (Settings default prefill) |
| 3 | `[BUDGET_TIME]` | project variable | P 327, 406 | `variables.BUDGET_TIME` |
| 4 | `[PREFERRED_STACK]` | project variable | P 328, 482, 694 | `variables.PREFERRED_STACK` |
| 5 | `[ROUGH_IDEA]` | project variable | P 402 | `variables.ROUGH_IDEA` |
| 6 | `[TARGET_USERS]` | project variable | P 404, 810 | `variables.TARGET_USERS` |
| 7 | `[QUALITY_BAR]` | project variable | P 869, 1075 | `variables.QUALITY_BAR` |
| 8 | `[paste the Stage 1 Handoff Packet]` | packet marker | P 520 | signed Stage 1 packet |
| 9 | `[paste the Stage 2 Handoff Packet]` | packet marker | P 782 | signed Stage 2 packet |
| 10 | `[paste the Stage 2 and Stage 3 Handoff Packets]` | packet marker | P 858 | Stage 2 then Stage 3 packet |
| 11 | `[paste the Stage 2 and 3 packets]` | packet marker | P 877 | Stage 2 then Stage 3 packet (same as #10) |
| 12 | `[paste the stage's inspection list]` | packet marker | P 265 | the instantiating stage's applicable items (DR-085) |
| 13 | `[app]` (in `[app]-spec/`) | path alias | o 501, 538, 540, 630 (removed in v2.2 by B-8) | the project's spec folder `spec` |
| 14 | `[N-1]` (in `stage-[N-1].md`) | path alias | P 259 | latest signed packet stage before N, two digits (DR-087) |
| 15 | `[N+1]` | path alias | P 275, 284 | next applicable stage after N |
| 16 | `[N]` | run input (context) | P 259, 273, 275, 287, 295 | the instantiating stage number; two digits inside `stage-[N].md` |
| 17 | `[X]` | run input | P 426 | `in=X:text` on 1.2 |
| 18 | `[changes]` | run input | P 426 | `in=changes:text` on 1.2 |
| 19 | `[paste numbers]` | run input | P 458 | `in=paste_numbers:multiline` on 1.4 |
| 20 | `[jurisdictions]` | run input | P 810 | `in=jurisdictions:text:remember` on 3.2 |
| 21 | `[list screens]` | run input | P 950 | `in=list_screens:text` on 4.5 |
| 22 | `[M#-NN]` | literal | P 969 (v2.2: `### [M#-NN] <Title>`, B-4) | left as written (format instruction) |
| + | `[optional: …]` | optional part | v2.2 only: 1.4 (B-5) | Include toggle (DR-090) |

22 distinct tokens, 51 occurrences in these sections (41 in `text` prompt fences, 10 in RUN WITH, HANDOFF and prose lines). The F.2 prompt (`bp:2139`–`bp:2144`) and the Council setup fences contain none.
*Test:* `resolver::blueprint_token_inventory` (golden of this table, AC-F006-01, -15).

**DR-083 · `resolver` · Project variables.** Resolve from the approved `project.json` `variables`. Empty, whitespace-only, equal to the token itself (`[APP_NAME]`), or equal to a placeholder in the blueprint's Project Variables example column that is itself bracketed (`bp:84 | `[BUDGET_TIME]` | Money and hours available | [your numbers] |`) counts as unresolved.
*Example:* `BUDGET_TIME = "[your numbers]"` → unresolved pill (the TradeLog harness fixture therefore sets a concrete value, e.g. "10 h/week, $0"). *Test:* `resolver::placeholder_is_unresolved` (AC-F006-03).

**DR-084 · `resolver` · Packet markers.** A marker resolves to the text of each named stage's **signed** packet (header line through the last field; attachments excluded), joined in stage order with one blank line between. An unsigned named packet blocks Run with "Sign Stage N first"; a missing file blocks with `SignedPacketMissing`.
*Example:* `bp:858 Inputs: [paste the Stage 2 and Stage 3 Handoff Packets]` → `docs/handoff/stage-02.md`'s packet, a blank line, `stage-03.md`'s packet. *Test:* `resolver::packet_markers` (AC-F006-04, -11).

**DR-085 · `resolver` · The inspection-list marker.** `[paste the stage's inspection list]` (Gate A) resolves to the instantiating stage's template items plus overlay items, minus items already attested Not applicable, one `- <original bullet text>` line each, in order.
*Example:* for `5:gate.A` with `staging` marked not applicable: three lines starting `- A fresh clone, then `install` and `dev`, …`. *Test:* `resolver::inspection_list_filtered` (AC-F006-04).

**DR-086 · `resolver` · `[app]-spec/`.** Resolves to `spec/` (`project.json` `specFolder`) rendered for the step's cwd (DR-092); a remap note "`[app]-spec/` → `spec/`" is shown. B-8 removes the alias from v2.2's text; the table keeps it so a v2.1-style edit still resolves.
*Example:* `bp:538 Save the result to `[app]-spec/02-teardown.md`.` (prose, v2.1) → `spec/02-teardown.md`. *Test:* `resolver::app_spec_alias` (AC-F006-05).

**DR-087 · `resolver` · Stage-number tokens.** In a gate or F.2 instance for stage N: `[N]` → `N` (no padding), except inside a `stage-[N]` file name, where it's two digits; `stage-[N-1].md` → `stage-<P>.md` where P is the latest stage before N with a signed packet on the applicable path (two digits; a remap note when P ≠ N−1); `[N+1]` → the next applicable stage after N (DR-054).
*Examples:* `bp:259 Run the Stage [N] inspection for [APP_NAME]. Read docs/handoff/stage-[N-1].md` for N=5 → `Run the Stage 5 inspection for TradeLog. Read docs/handoff/stage-04.md`; `bp:275 === HANDOFF PACKET · Stage [N] → Stage [N+1] ===` → `=== HANDOFF PACKET · Stage 5 → Stage 6 ===`; `bp:273 … Write docs/handoff/stage-[N].md` → `stage-05.md`. *Test:* `resolver::stage_tokens_padding`.

**DR-088 · `resolver` · Packet remapping.** A marker or `stage-[N-1]` alias naming a not-applicable stage resolves to the latest signed packet before it, with a note ("Stage 3 was not applicable; using Stage 2's packet"). When a remap makes a marker name a packet already included in the same marker, it's included once, with a note.
*Example:* `bp:877 Inputs: [paste the Stage 2 and 3 packets]` in a project where Stage 3 is not applicable → Stage 2's packet once, note "Stage 3 not applicable". *Test:* `resolver::remap_not_applicable` (AC-F006-10, AC-F004-09).

**DR-089 · `resolver` · Run inputs.** A run input token resolves to the value Billy typed for that step (or, for `remember` inputs, the project's last value as a prefill). It is required only if the token survives part removal (DR-090, DR-091). Values longer than 64 KB are accepted with a warning.
*Example:* 3.2 with `commercial = others_data = multi_user = no`: the legal part holding `[jurisdictions]` is removed, so the input isn't asked for. *Test:* `resolver::inputs_required_only_if_present` (AC-F006-12, -14, AC-F008-11).

**DR-090 · `resolver` · Optional parts.** Include defaults to on; off removes the part's text (including its brackets) and queues "<step> part \"opt-N\": skipped: Include turned off" for `SKIPPED:`; on removes only the `[optional: ` prefix and the closing `]`.
*Example (v2.2, 1.4):* off → `Otherwise, ` is followed directly by `Then, when I say "approved", output the APP BRIEF …`. *Test:* `resolver::optional_part_toggle` (AC-F006-07).

**DR-091 · `resolver` · `fm:if` parts.** A part whose expression is false is removed and queued for `SKIPPED:`; true keeps it; undecided makes the step `undecided` ("Decide <condition> to continue").
*Example:* Foreman (all of `commercial others_data multi_user` = no) → 3.2's legal item 3 is removed; `SKIPPED:` gets `3.2 part "fm:if commercial|others_data|multi_user": not applicable (all false)`. *Test:* `resolver::fm_if_removal` (AC-F006-07).

**DR-092 · `resolver` · Paths render relative to cwd.** Repo-relative paths that the resolver inserts (path aliases, annotation paths in notes) are rendered relative to the step's cwd. Literal paths already in prompt text are never rewritten.
*Example:* a `cwd=spec` step that references `docs/handoff/stage-02.md` → `../docs/handoff/stage-02.md` (PROPOSAL B-2's 2.6 text already writes `../docs/handoff/stage-02.md` literally and is left alone). *Test:* `resolver::cwd_relative_render` (AC-F006-06).

**DR-093 · `resolver` · Purity.** `resolve(ctx)` is a pure function of (template, approved project, overlay, signed packets, run inputs, optional-part toggles, stage context). Text inside a packet or input that looks like flags, slash commands or instructions is inserted as text and changes no flag, mode, model, cwd or destination.
*Example:* a signed packet containing `--dangerously-skip-permissions` and `/model haiku` produces the same argv as one without, apart from the prompt hash. *Test:* `resolver::deterministic` (property, AC-F006-08), `abuse::packet_text_cannot_change_flags` (AC-F006-09).

**DR-094 · `sanitize` · Prompt hygiene.** A resolved prompt is rejected with the offset of the first offending character if it contains C0 controls other than TAB, LF, CR, DEL, or C1 controls (U+0080–U+009F); and, after trimming leading whitespace, if it starts with `-`, `/` or `!` (SR-04, SR-28).
*Example:* a run input pasted with `\x1b[31m` → `PromptControlChar { offset, 0x1b }`. No v2.1 prompt in Stages 0–5 starts with those characters (2.3 starts `ultrathink.`). *Test:* `resolver::rejects_control_chars_in_prompt` (AC-F006-17), `argv::rejects_option_like_prompt` (AC-F008-12).

**DR-095 · `sanitize` · Text for terminals.** Text Foreman places on the clipboard for a terminal step, and every terminal paste, has ESC, C0 (except TAB, LF, CR), DEL and C1 removed, with a visible count of removed characters.
*Example:* paste `\x1b[201~\x03!touch /tmp/pwn\r` → `[201~!touch /tmp/pwn\r` with "removed 2 control characters", sent as one bracketed paste. *Test:* `pty::paste_strips_escape_sequences` (AC-F009-15, -16).

## 7. Run configs and model policy (`policy`)

**DR-100 · `policy` · Run config.** A step's run config is `{ surface, cwd, session, chain, start, switches, alts, agents }` from its annotation and inheritance (DR-029). `agents` is display-only, from the RUN WITH text (no flag carries it in v1).
*Example:* 2.2 → `{cli, spec, new, —, fable@high/plan, [plan-approved → fable@high/acceptEdits], [pro-plan → opus@xhigh], "all six council agents"}`. *Test:* `policy::run_config_from_annotation`.

**DR-101 · `policy` · Alternatives.** If the project's `modelPolicy.plan` matches an `alt` rule (`pro` ↔ `pro-plan`, `max` ↔ `max-plan`), that alternative's model and effort replace `start`'s and every switch's model and effort; the mode stays.
*Example:* 2.2 with `plan: pro` → `opus@xhigh/plan`, switch `opus@xhigh/acceptEdits`. With `plan: max` → unchanged. *Test:* `policy::alternative_selection` (AC-F005-02).

**DR-102 · `policy` · Substitutions.** After the alternative, each `modelPolicy.substitutions` entry `{from, to, effort}` replaces model `from` with `to` (and effort with `effort` when non-null), in start and switches. Substitutions apply once, in list order, not transitively.
*Example (Foreman):* `fable → opus@xhigh`: 2.2 `fable@high/plan` → `opus@xhigh/plan`; spec line "Fable 5.1 · high (Max) → Opus 5.5 · xhigh (substituted)"; 4.7 `fable@adaptive` → `opus@adaptive` (effort `xhigh` doesn't apply to chat; chat keeps `adaptive`). *Test:* `policy::substitution_rules` (AC-F005-03).

**DR-103 · `policy` · Overrides.** A per-run override (model, effort, mode, each optional) applies last, for the next run only, and is logged in the run record. Override values must fit the surface (no mode on chat, no `adaptive` on CLI).
*Example:* 2.4 overridden to `opus@max` → that run uses `--effort max`; `run.json` `override: true`. *Test:* `policy::override_scope` (AC-F005-05).

**DR-104 · `policy` · Precedence and SUBSTITUTIONS.** Effective start = override > substitution > alternative > template, each value with its origin (`Explained<Start>`). Every step whose effective start differs from its template start gets one `SUBSTITUTIONS:` line at signing: `<step>: <template start> → <effective start> (<origin>)`.
*Example:* `2.2: fable@high/plan → opus@xhigh/plan (substitution fable → opus@xhigh)`. *Test:* `policy::precedence` (AC-F005-01, -04).

**DR-105 · `policy` · Switches are cues.** Switches are shown in the step and (P2) the Now/Next strip as "Next: after you approve the plan, choose Accept edits"; Foreman never sends them as keystrokes.
*Example:* 5.1's `switch=plan-approved:opusplan@medium/acceptEdits`. *Test:* `policy::switches_never_typed` (AC-F005-07).

**DR-106 · `policy` · Confirmation baseline.** "A mode or model different from the template's" (SR-19) compares the effective start with the policy start (template + alternative + substitution): only overrides trigger `ModeOrModelChanged`. `auto` or `dontAsk` in an interactive session always confirms.
*Example:* Foreman's 2.2 (substituted to Opus) needs no confirmation; an override to `sonnet` does. *Test:* `preflight::confirm_required_for_mode_change`.

## 8. Chains and re-arm (`chain`, `app::rearm`)

**DR-110 · `chain` · Session plans.** `plan_session(step, chains, new_id)`: `session=new` → `New { id }` (fresh UUID v4 for CLI; an internal ID for chat); `session=continue` → `Continue { chain }` with the chain of `chain=`; chain `broken` → `Broken`; chain `handed-off` → `Fork { from }` unless Billy confirms Desktop is closed.
*Example:* 2.2 → `New`; 2.3 → `Continue(2.2's chain)`. *Test:* `chain::plan_session_table`.

**DR-111 · `chain` · Argv per plan.** `New` → `--session-id <id>`, never `--resume`; `Continue` → `--resume <id>`, never `--session-id`; `Fork` → `--resume <id> --fork-session` plus `--session-id <new>` only on S-8's pass branch (the single allowed combination).
*Example:* 2.6 → `claude --resume 6f1c… --model opus --effort xhigh --permission-mode default --name fm-foreman-s02-2-6 --settings …/runs/<runId>/settings.json "<prompt>"`. *Test:* `argv::session_id_only_on_chain_start_or_fork` (property, AC-F010-01, -02, -15).

**DR-112 · `chain` · Superseded and closed.** Re-running a chain-start step creates a new chain and marks the old one `superseded`; later continuing steps join the new chain. A chain becomes `closed` when its stage is signed.
*Example:* re-running 2.2 after 2.3 ran → new chain; 2.3 must run again in it. *Test:* `chain::superseded_on_rerun` (AC-F010-08).

**DR-113 · `chain` · Broken chains and chain starts that don't apply.** A continuing CLI step whose transcript file is missing (`stat` only; path per ADR-008, encoding verified in M3) is `broken` and offers "Start a fresh chain with context": a new session whose prompt is the last signed packet and the step's declared input paths, followed by the step's prompt. The same applies when the chain-start step is not applicable or skipped.
*Example:* Foreman's 3.2 (`same chat` as 3.1) with 3.1 not applicable (`commercial = payments = no`) → 3.2 starts a new chat whose prompt is Stage 2's signed packet followed by 3.2's prompt (otherwise the Stage 2 packet pasted in 3.1, `bp:782`, would be missing). *Test:* `chain::broken_when_transcript_missing`, `chain::start_not_applicable_becomes_fresh` (AC-F010-09).

**DR-114 · `app::rearm` · When re-arm runs.** Re-arm runs for a continuing `prompt` step (and a followup whose chain process exited, DR-068) only from `rearm_start`/`rearm_continue`, i.e. Billy's click or keystroke; no event, watcher hint, sensor event or output starts it (SR-18).
*Test:* `rearm::only_from_user_action` (AC-F010-07).

**DR-115 · `app::rearm` · The ending sequence.** If the chain's process is alive: P1, the native dialog "Claude is idle and nothing is running in the background" must be confirmed; P2, the sensor must report idle (last event `Stop`, no outstanding subagents or tasks), otherwise "N background tasks still running: Wait / End anyway". Then write `\x03`, wait 150 ms, write `/exit\r`, wait ≤ 10 s for exit (P2: `SessionEnd` `prompt_input_exit`). On timeout: "Claude didn't exit. End it yourself, then press Continue."; no signal without Terminate's confirmation. Then archive the buffer and write `── <step> · <model> · <effort> · <mode> · permission grants reset ──`.
*Example:* 2.6 after 2.5: `── 2.6 Freeze the Master Spec (the handoff) · Opus 5.5 · xhigh · Manual · permission grants reset ──`. *Test:* `rearm::ctrl_c_then_exit_order`, `rearm::no_kill_without_idle_or_confirm` (AC-F010-03…-06, -13, -14).

**DR-116 · `app::rearm` · Re-check before relaunch.** After the old process exits, Rust runs `launch::prepare` again; if the new argv (prompt hashed) or the prompt hash differs from the plan Billy confirmed, the relaunch stops with `Stale` and the new preview.
*Example:* Billy changes a run input in another window during the 10 s wait → `Stale`, nothing spawned. *Test:* `rearm::stale_plan_not_spawned`.

**DR-117 · `chain` · Effort on resume (S-3).** Pass → `--effort <level>`. Fail → `effortLevel` in the run settings when the level is `low`…`xhigh` (`effortLevel` doesn't accept `max` or `ultracode`); otherwise, and when neither works, the step shows `/effort <level>` for Billy to type, with the note that it changes his saved default.
*Example:* S-3 fail, 2.6 at `xhigh` → `"effortLevel": "xhigh"` in `runs/<runId>/settings.json`; at `max` → the `/effort max` line. *Test:* `chain::effort_on_resume_branches` (AC-F010-10).

**DR-118 · `chain` · Desktop handoff and forks.** `claude --desktop --resume <id>` marks the chain `handed-off`; the next continuing step forks (DR-110) into a new chain with `forkedFrom`. The fork's ID: S-8 pass → Foreman's `--session-id`; fail → the only new transcript after launch; ambiguous → fresh chain with context. S-23's copied branch: no fork.
*Test:* `chain::fork_argv` (AC-F010-15, AC-F024-02, -03).

**DR-119 · `chain` · Chat chains.** A chat chain has no Claude UUID; continuing chat steps open the stored conversation URL (`project.json` `chatUrls[<chain start>]`), new ones the Project URL. Without the needed link, the prompt is copied, nothing opens, and the step says which link to set.
*Example:* 1.2 continues 1.1's chat → opens `chatUrls["1.1"]`. *Test:* `chain::chat_links` (AC-F012-03, -04).

## 9. Launch (`launch`)

**DR-120 · `launch` · Preflight order.** Fixed: version (`claude -v`, every launch; floor) → contract (P2) → fingerprint + auth-key scan → acknowledgement → auth (`claude auth status`, cached; only after the previous two pass) → transcript (continuing) → inputs (referenced repo paths; signed packets block, others warn) → run inputs → prompt hygiene (DR-094) → argv and env → run settings → confirmation reasons. The first blocker stops the checks that depend on it.
*Example:* a repo `.claude/settings.local.json` with `"apiKeyHelper"` → blocked at step 3; `auth status` never runs. *Test:* `preflight::auth_after_fingerprint`, `launch::progress_phase_order` (AC-F008-08).

**DR-121 · `launch` · Session name.** `--name fm-<slug>-s<NN>-<step>`, where `<step>` is the step key lowercased with `.` and `:` replaced by `-`, total matching `^fm-[a-z0-9-]{1,80}$` (truncate the slug if needed).
*Examples:* `fm-tradelog-s02-2-3`, `fm-foreman-s05-5-gate-a`, `fm-foreman-s02-2-f-2`. *Test:* `argv::name_format` (AC-F008-14).

**DR-122 · `launch` · Argv from the effective start.** CLI: `--model <model> --effort <effort> --permission-mode <mode>` (`manual` already stored as `default`); `opusplan` passes as is; `--ax-screen-reader` when VoiceOver is on; `--settings <app-data>/runs/<runId>/settings.json`; the prompt last. Chat and desktop steps build no `claude` argv (Desktop: `claude --desktop` only).
*Example (Foreman 5.1):* `claude --session-id <uuid> --model opusplan --effort medium --permission-mode plan --name fm-foreman-s05-5-1 --settings … "<prompt>"`. *Test:* `argv::positional_prompt_snapshot` (harness, AC-F003-02, AC-F008-14).

**DR-123 · `launch` · Child environment.** `env_clear()` plus the allowlist (SR-05); the always-removed families beat Billy's pass-through list and cover every SR-07 `env` pattern: `ANTHROPIC_*`, `CLAUDE_CODE_*` (except `CLAUDE_CODE_OAUTH_TOKEN` from Foreman's own environment), `AWS_*`, `GOOGLE_*`, `GCLOUD_*`, `CLOUDSDK_*`, `AZURE_*`, `*_PROXY`/`*_proxy`, `NODE_EXTRA_CA_CERTS`, `NODE_TLS_REJECT_UNAUTHORIZED`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `DYLD_*`, `NODE_OPTIONS`, `BASH_ENV`, `ENV`, `GIT_*`.
*Example:* pass-through `HTTPS_PROXY` → still removed. *Test:* `env::denylist_beats_passthrough`, `env::removed_list_covers_blocked_patterns`.

**DR-124 · `launch` · Confirmation reasons.** Exactly: overlay-added step; mode or model changed (DR-106); first launch in the project (`firstLaunchDone` false); `auto`/`dontAsk`; first launch after `claude -v` changed ("Claude Code updated X → Y"). The confirmation is a native dialog (SR-24a).
*Test:* `preflight::confirm_required_for_{overlay_step,mode_change,auto_and_dontask,first_launch,version_change}` (AC-F008-18).

## 10. Packets (`packet`)

**DR-130 · `packet` · Files.** Stage N's packet is `docs/handoff/stage-NN.md` (two digits), attachments `docs/handoff/stage-NN-attachments.md`. The watcher considers exactly those names (`stage-\d{2}\.md`, `stage-\d{2}-attachments\.md`); `stage-2.md`, `Stage-02.md`, `stage-02.md.bak` are ignored.
*Test:* `watcher::interest_set_exact_names`.

**DR-131 · `packet` · Normalization.** As `docs/contracts/files.md` §5.2: BOM/CRLF, wrapping fence, NBSP, smart quotes, trailing spaces, header variants, unsigned `APPROVED BY` variants → `<blank>`, `None`/`none.` → `none`. Each change is a recorded note; nothing else is altered. Pasted packets are normalized before validation; files Claude wrote are shown normalized and written only after Billy confirms.
*Example:* the v2.1 Gate B line `bp:277 APPROVED BY: <leave blank for me to sign>` → `APPROVED BY: <blank>`, note "unsigned". *Test:* `packet::normalization_golden` (Foreman's `docs/handoff/stage-0[1-3].md`, AC-F012-05, AC-F013-08).

**DR-132 · `packet` · Splitting a paste.** The packet is the header line through the end of the format's last key's value (its value ends at the first blank line); everything after it goes to the attachments file. Two header lines → "paste one packet at a time"; a header naming another stage → "This is the Stage 3 packet; this step expects Stage 2.". Nothing is written in either case.
*Example:* a 1.4 paste whose `NEXT: Stage 2 · Architect's Design Studio · chat + Claude Code council` (`bp:490`) line is followed by a blank line and the press release → the press release goes to `docs/handoff/stage-01-attachments.md`. *Test:* `packet::split_paste` (AC-F012-06, -10).

**DR-133 · `packet` · Validity.** A packet is valid when: (1) the header is `=== HANDOFF PACKET · Stage N → Stage M ===`, N matches the file name and M is the next applicable stage (DR-054); (2) every key of the stage's format is present with a value; (3) `OPEN ITEMS:` is exactly `none`; (4) `APPROVED BY:` is `<blank>` or a signed form (DR-134); (5) for Stage 4, `BLUEPRINT VERSION` equals the lock's `blueprintVersion`; (6) no value still equals the format's placeholder text (`packet.placeholder`). Extra keys are allowed with a warning; a duplicate key is a problem. Every problem has a line.
*Example:* Stage 0 format's `bp:362 FOR: <who pays, specifically>` left as is → `packet.placeholder` at that line. A Stage 2 packet whose header says `Stage 2 → Stage 4` while Stage 3 applies → rule 1. *Test:* `packet::validation_rules` (one passing and one failing fixture per rule, AC-F013-01, -02, AC-F002-08), `packet::never_panics` (AC-F013-03).

**DR-134 · `packet` · Signature forms.** Unsigned: `<blank>`. Signed form: `<name> on <YYYY-MM-DD>` (name 1–80 characters without `<`, `>`), optionally followed by a parenthetical of ≤ 200 characters, which is accepted with a warning.
*Example:* Foreman's `docs/handoff/stage-01.md` line 3, `APPROVED BY: Billy Ribeiro on 2026-10-19 (approved in chat: "Get everything that can be done done here")` → signed form, warning `packet.signature-note`. *Test:* `packet::signature_forms`.

**DR-135 · `packet` · Outside signatures need Billy's native confirmation.** A packet in the signed form that Foreman's signing didn't write counts as signed only after Billy confirms it in a native dialog; the confirmation stores the packet's SHA-256 in `approvals/<id>.json` `outsideSignatures`, and any later change to the file voids it. Claude writing a name never signs a packet (SR-47). In P1 this is how a hand-signed packet counts; for Stage 2 the confirmation is refused while an early condition is `undecided`.
*Example:* Claude writes `APPROVED BY: Billy Ribeiro on 2026-10-19` into `stage-02.md` → shown as "signature not confirmed"; the Next step stays in Stage 2 until Billy confirms. *Test:* `packet::outside_signature_needs_confirmation` (AC-F013-09, AC-F004-12).

**DR-136 · `packet` · Not-applicable field values.** A required key whose content depends on skipped or not-applicable work may hold `not applicable: <reason>` or `skipped: <reason>`; this is accepted only when the stage's `SKIPPED:` queue (DR-055) is non-empty. (The blueprint's formats don't say which keys depend on which steps.)
*Example:* Stage 3 with 3.1 not applicable: `PRICING: not applicable: personal tool (commercial = no)` is accepted; Stage 1's `bp:488 PRESS RELEASE + FAQ: <attached below the packet>` becomes `skipped: <reason>` when `opt-1` is off (B-5). *Test:* `packet::na_values_need_skips`.

**DR-137 · `packet` · Fix request text.** "Your packet is missing: X, Y. OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block." with the actual problems (the OPEN ITEMS sentence only when rule 3 failed).
*Test:* `packet::fix_request_text` (AC-F012-11).

**DR-138 · `packet` · Lessons append (P2).** F.2 paste-back appends entries to `docs/lessons.md`, renumbering each `## L-NNN` heading to the next free IDs (DR-074), after a diff, by a full compare-and-swap write; the header is kept; a missing file refuses the append.
*Test:* `lessons::append_numbering` (AC-F016-02…-04).

**DR-139 · `packet` · Fields Foreman adds at signing.** `SKIPPED:`, `SUBSTITUTIONS:`, `ATTESTED:` rendered deterministically (`docs/contracts/files.md` §5.4) as the last fields before `OPEN ITEMS:`, replacing same-named fields; `APPROVED BY:` replaced in place with `<DRI name> on <date>`.
*Test:* `packet::added_fields_golden` (AC-F013-15, AC-F014-02).

## 11. Signing and git (`git`)

**DR-140 · `git` · Allowed operations.** `git::run` takes only: `init`, `rev-parse` (`--show-toplevel`, `--git-path`, `--git-common-dir`, `--verify -q HEAD`, `<rev>:<path>`), `symbolic-ref -q HEAD`, `config --get`/`--show-origin --show-scope`, `cat-file blob`, `ls-tree`, `hash-object [-w] --no-filters`, `update-index --add --cacheinfo`, `read-tree <tip>`/`read-tree --empty` (temporary index only), `write-tree`, `commit-tree --no-gpg-sign`, `update-ref`, `show <rev>:<path>` (raw blob). ADR-012's operation enum is exactly this list. Never `status`, `diff`, `add`, `commit` (SR-45). Every call carries the SR-45 `-c` overrides and environment and a 60 s timeout. (CR-002) One call also carries attribute neutralizing: `update-index --cacheinfo` on the real index gets `-c core.attributesFile=/dev/null` and `GIT_ATTR_SOURCE=4b825dc642cb6eb9a060e54bf8d69288fbee4904` (SR-45).
*Test:* `git::status_and_diff_never_run_filters` (static allowlist check), `git::invocation_snapshot`.

**DR-141 · `git` · Own repo only.** Before any operation, `rev-parse --show-toplevel` (resolved) must equal the project root (or the blueprint repo root for blueprint reads); a folder inside an enclosing repo is refused, and setup offers `git init` (SR-44).
*Example:* a project folder `~/dev/monorepo/apps/tradelog` without its own `.git` → refused; setup shows "Initialize a repository here". *Test:* `git::refuses_enclosing_repo`.

**DR-142 · `git` · Change detection by hash.** "Changed since HEAD" is `hash-object --no-filters <file>` ≠ `rev-parse HEAD:<path>` (missing at HEAD counts as changed). The blueprint pin refuses a changed file (showing both hashes); signing commits a path only if it changed.
*Example:* pinning with an edited blueprint → "claude-build-blueprint.md differs from HEAD (working a3f9c1… vs HEAD 7d02e4…)". *Test:* `lock::dirty_blueprint_refused` (AC-F002-02).

**DR-143 · `git` · The commit.** For a set of paths: re-read each through `store` and compare with the reviewed hash; `hash-object -w --no-filters --stdin` per path; `GIT_INDEX_FILE=<app-data>/tmp/git-index-<uuid> read-tree <tip>`; `update-index --add --cacheinfo 100644,<oid>,<path>` per path; `write-tree`; `commit-tree <tree> -p <tip> --no-gpg-sign -m "handoff: stage NN"` (decision commits: `-m "foreman: <file>"`); `update-ref <ref> <new> <old>`; current branch only: `update-index --cacheinfo` on the real index for those paths. Paths: the packet, its attachments, the stage record, decision files with pending changes, and `docs/lessons.md` if changed (PROPOSAL B-3 commits it with the packet). The commit OID is stored in `progress.json` `signing[NN].commit`, never in the stage record (which the commit contains).
- **HEAD state** (current branch). Unborn (`rev-parse --verify -q HEAD` fails, e.g. right after setup's `git init`): `read-tree --empty`, `commit-tree` without `-p`, `update-ref HEAD <new> 0000000000000000000000000000000000000000`. Detached (`symbolic-ref -q HEAD` exits 1): refused with "Check out a branch first"; decision changes stay pending and signing stays possible once a branch is checked out.
- **Real-index refresh.** The last step retries on `index.lock` 5 times, 200 ms apart. On failure the commit stands; `progress.json` records `indexRefreshPending`, Foreman retries on open and focus, and the banner "Index not refreshed; don't commit until this clears" shows until a retry succeeds.
- **Attributes during the refresh (CR-002).** The refresh runs with `-c core.attributesFile=/dev/null` and `GIT_ATTR_SOURCE=<empty tree>` so a racily-clean entry can't run a repo filter driver (T-040). Neither covers `$GIT_DIR/info/attributes` (found with `rev-parse --git-path info/attributes`), so while that file is non-empty Foreman makes no refresh attempt: the commit stands, `indexRefreshPending` is recorded, and the same banner shows until Billy clears the file or refreshes the index himself. `GIT_ATTR_SOURCE` needs git ≥ 2.40; on Apple's 2.39.x the residual stays (UNVERIFIED on his Mac; T-040).
*Example:* Foreman's own project right after setup (`git init`, no commits): the first `condition_set` before Stage 5 makes the repo's first commit, `foreman: project.json`, with no parent. *Test:* `sign::pathspec_only`, `sign::untracked_new_packet`, `sign::other_branch_leaves_worktree_head_and_index`, `sign::concurrent_ref_move_fails` (AC-F014-06…-09), `sign::unborn_head_first_commit`, `sign::detached_head_refused` (AC-F020-08), `sign::index_locked_refresh_retried` (AC-F014-06).

**DR-144 · `git` · Sign mode.** v1 always commits with `commit-tree --no-gpg-sign`, at every scope of signing configuration (Stage 4.7 decision); `-S` is never passed and `gpg.program` never runs. Doctor shows a notice when `config --show-origin --show-scope commit.gpgSign` is `true` anywhere ("Foreman's commits aren't GPG-signed in v1").
*Test:* `git::repo_gpg_program_never_runs` (AC-F014-12).

**DR-145 · `git` · Read-back.** After `update-ref`, `cat-file blob <commit>:<path>` for every path must hash to what Foreman wrote; otherwise the stage stays `commitPending` with the diff ("Your repo's commit hooks changed the packet"). The stamp runs only after read-back passes.
*Test:* `sign::readback_detects_mismatch` (AC-F014-10, -11).

**DR-146 · `git` · Tamper.** After signing, a packet whose working-file hash differs from its committed blob is a tamper event; the resolver uses the committed blob; Restore writes the blob back by compare-and-swap.
*Test:* `sign::tamper_detected` (AC-F014-15).

## 12. Decision files (`decisions`)

**DR-150 · `decisions` · Effective config is the approved copy.** Foreman reads conditions, model policy, skips, additions and the lock from `approvals/<projectId>/<file>.json`, never directly from the repo file.
*Example:* Claude edits `.foreman/project.json` to set `commercial: yes` → the path doesn't change until Billy accepts. *Test:* `decisions::outside_edit_has_no_effect_until_accepted` (one case per file, AC-F022-01).

**DR-151 · `decisions` · Classification.** An outside change whose only differences are in `variables`, `chatProjectUrl` or `chatUrls` is accepted automatically ("Variables reloaded"); any other difference (including `name`, `slug`, every overlay and lock field) is a pending change with a redacted diff; an unparsable or newer-schema file makes the project read-only.
*Test:* `decisions::variables_only_autoaccept` (AC-F022-03).

**DR-152 · `decisions` · Accept and reject.** Accept (native dialog) re-reads and re-hashes; if the file changed again the review refreshes. Reject writes the approved content back by compare-and-swap; a newer edit is a conflict and the review repeats.
*Test:* `decisions::reject_restores_via_cas` (AC-F022-02, -05).

**DR-153 · `decisions` · Foreman's own changes.** Before Stage 5 is signed: written to the approved copy first, then to the repo file by compare-and-swap (the copy is rolled back on a conflict), and committed immediately (DR-143). From Stage 5 on: written to the approved copy only and held as pending in `progress.json`; the repo file is not touched until the next signing commit writes it, so it keeps the last committed version and drift is checked against `committedSha256` (ADR-022 §5). In both cases an own change is refused (`OutsidePending`) while the repo file holds an outside edit that Billy hasn't accepted or rejected (SR-33, ADR-022 §4); he resolves the pending change first.
*Reconciled 2026-10-20: the earlier text ("approved copy first, then the repo file" for every stage) contradicted its own drift rule and ADR-022 §5; see progress.md, Known deviations.*
*Test:* `decisions::pending_from_stage5_not_flagged` (AC-F020-08, -09, AC-F014-14).

## 13. Lock (`lock`)

**DR-155 · `lock` · Pin.** Pinning writes `.foreman/blueprint.lock.json` (commit, path, normalized `sourceSha256`, blueprint and grammar versions, time, and `{textSha256, kind}` per item) and stores the text in `templates/<sourceSha256>.md`. Re-pinning is allowed only before Stage 1 is signed ("upgrading the pin is v1.1"). A pin is refused only for a file that differs from HEAD (DR-142) or after Stage 1 is signed; annotation errors do not refuse it (as built in M1): they are listed (AC-F001-15) and make the affected steps not runnable. Whether a pin should also refuse errors in the stages P1 uses is open (progress.md, Known deviations).
*Test:* `lock::pin_contents`, `lock::repin_window` (AC-F002-01, -03).

**DR-156 · `lock` · Restore and verify.** On open, the template comes from app data by hash; if missing, from `cat-file blob <commit>:<path>` in the blueprint repo, re-hashed. A mismatch, a missing repo or commit, or a missing git opens the project read-only with the reason.
*Test:* `lock::restore_from_git`, `lock::hash_mismatch_read_only` (AC-F002-04…-07).

**DR-157 · `lock` · Item drift.** On open, each item's current `textSha256` is compared with the lock; a difference (reworded bullet, or bullets swapped without moving IDs) shows "re-confirm kind" and blocks attesting that item until Billy confirms (native dialog), stored in `approvals` `itemKinds`.
*Example:* swapping `bp:1149` and `bp:1150` while keeping `fresh-clone=attest ci-gates=attest` in order → both items drift. *Test:* `template::swapped_bullets_reconfirm` (AC-F001-16, -17).

## 14. Paths and watching (`pathpolicy`, `app::watch`)

**DR-160 · `pathpolicy` · Write allowlist first.** Inside a project, a write path must be under `docs/` or `spec/`, or be a typed exception (`.foreman/` decision files, `spec/.claude/agents/<known>.md` for `council-setup`, root `CLAUDE.md` via the assembler with a native confirmation, `info/exclude` via its writer). Then SR-30's deny rules (components `.git .claude .husky .vscode .idea .github .foreman` at any depth after NFC + case folding; names `.mcp.json CLAUDE.local.md .envrc .gitmodules .gitattributes .npmrc`), ≤ 1,024 bytes total and ≤ 255 per component, no `..`, absolute paths, NUL, control or bidi characters. Annotation `out`/`appends` paths are checked with the same function (purpose `TemplateOut`, no exceptions).
*Examples:* `spec/council/r6-feature-specs.md` → allowed; `spec/.claude/settings.json` → refused (`.claude`); `README.md` → refused (not on the allowlist); `docs/.GIT/x` → refused (folded `.git`). *Test:* `fs_policy::allowlist_only`, `fs_policy::*` golden suite, `fs_policy::rejects_overlong_path` (AC-F021-01…-05).

**DR-161 · `pathpolicy` · Reads.** Repo reads use the same syntax rules plus the secret patterns (`.env*`, `*.pem`, `*.key`, `id_*`, `*.p12`, `.npmrc`, `.netrc`, `.ssh/`, `.aws/`, `.config/gcloud/`), refuse symlinks at any component, non-regular files and `nlink > 1`.
*Test:* `fs_policy::read_refuses_symlink_at_any_component`, `secrets::excluded_from_views_and_watcher` (AC-F021-07, AC-XC-11).

**DR-162 · `app::watch` · Exact interest set.** The watcher acts only on: `docs/handoff/stage-\d{2}\.md`, `docs/handoff/stage-\d{2}-attachments\.md`, each applicable step's declared `out` paths, `docs/lessons.md`, and `.foreman/{project,overlay,blueprint.lock}.json` (plus `events/` in app data, P2). Every hint is re-read through `store` and compared by `ReadStamp`; it never starts an action.
*Example:* a write to `docs/handoff/stage-02.md` → `packet:changed`; to `docs/handoff/stage-02.md.orig` → ignored. *Test:* `watcher::interest_set_exact_names`, `watcher::events_never_act` (AC-F013-04…-07).

---

## 15. Gaps and contradictions found while writing this file

These were for the Stage 4.7 review; each rule above states the choice made. **Resolved in the 4.7 fix pass:** items 1–6, 9, 13 and 14 in PROPOSAL B-1 (which now names DR-020 to DR-038 as its normative grammar); items 10 and 11 in ADR-012; item 12 in DR-143 and AC-F014-06; item 7 stays as DR-113.

1. **`fm:if` vs fenced prompts.** PROPOSAL B-1 says `fm:if` is block-level and that the PDF is unchanged, but every prompt is a fenced code block, inside which an HTML comment is literal text. DR-032 splits a prompt into several fences around `fm:if` blocks (block-level, invisible comments, but the PDF shows the prompt as two or three code boxes). Alternative: line-level markers inside the fence, filtered by the render script.
2. **`kind=followup` means two things.** r6 F-007 defines followup as "copy and focus terminal", but PROPOSAL B-1's 2.3 example uses `kind=followup` for a step MASTER-SPEC treats as re-armed. DR-068 and lint DR-042 reconcile them.
3. **Steps without a RUN WITH.** PROPOSAL places `fm:step` "directly under a RUN WITH", yet asks for `kind=action` (Council setup), `kind=wait` (after 1.3) and `kind=manual` (2.4) steps that have none, and 5.1's second prompt has none. DR-007 and the `1.3b`/`2.4b`/`5.1b`/`1.4b` IDs fill this; the action vocabulary is keyed by step ID (DR-036).
4. **No wait duration in the grammar.** DR-067 asks Billy for the date.
5. **`in=X` on 1.1** (PROPOSAL B-1) but `[X]` is in 1.2's prompt (`bp:426`).
6. **Chat effort.** r6's effort list has no value for chat ("effort adaptive"); DR-023 adds `adaptive` for chat only. `effortLevel` rejects `max` and `ultracode` (Claude Code docs), which S-3's fail branch needs (DR-117).
7. **Chain start not applicable.** 3.2 is "same chat" as 3.1, which is conditional; for Foreman 3.1 doesn't apply and 3.2 would lose the Stage 2 packet pasted in 3.1. DR-113 starts a fresh chain with context.
8. **Packet formats don't mark condition-dependent keys** (Stage 3's `PRICING`… when 3.1 doesn't apply; Foreman's own `stage-03.md` omits them). DR-136 accepts `not applicable: <reason>` only with a non-empty SKIPPED queue.
9. **Packet formats are inside prompt fences**, not separate fences (ADR-005 says "fences starting `=== HANDOFF PACKET`"). DR-017 extracts them from inside the fence.
10. **Stage record and commit OID**: the record is in the commit it would describe; the OID is kept in app data (`progress.json` `signing[NN].commit`). Resolved in ADR-012 (decision 7) in the Stage 4.7 fix pass.
11. **ADR-012's operation enum** listed `Status` and `DiffNoExt`, which revised SR-45 forbids. Resolved in the Stage 4.7 fix pass: the enum is now exactly DR-140's list.
12. **Signing paths.** PROPOSAL B-3 commits `docs/lessons.md` with the packet; r6 F-014's path list omits it. DR-143 includes it when changed.
13. **Item IDs and the lock.** The lock keys items by ID alone, so IDs must be unique across the template (DR-034); the proposal's examples (`staging`, `fresh-clone`) don't say so.
14. **`[BUDGET_TIME]`'s example value is a placeholder** (`[your numbers]`), so the TradeLog harness fixture "from the blueprint's example variables" can't resolve it without a concrete value (DR-083).
15. **1.4 when 1.3 doesn't apply**: `[paste numbers]` and "If they missed the threshold" assume the validation ran; a `fm:if commercial` part around those sentences would remove a meaningless required input (not in PROPOSAL B-5).
16. **Conditional council outputs.** 2.2's `out=` lists `r1-market.md` and `r2-growth.md`, which aren't produced when `commercial = no`; `out` entries can't be conditional, so missing outputs are warnings only (round tracker).
17. **Stage 17 has no `### Stage 17 gate` heading** (found building M1-13, 2026-10-20). DR-011 says gate instances exist for Stages 5–17, but in v2.1 and v2.2 only Stages 5–16 have the heading; Stage 17 ends with prose ("Write the final packet with Gate prompt B …, run Gate prompt C, and sign it"). Foreman follows DR-011's mechanism (instances come from the heading), so v2.2 yields gate instances for 5–16. Stage 17 is unannotated in v2.2 (needs annotation until M4-04), and the release that annotates it should add the heading (proposal for the retro). `template::gate_instances_stages_5_to_17` pins exactly this.
