# ADR-005: Blueprint parsing with comrak =0.56.0 and an owned `fm:` grammar parser

## Status
Proposed

## Date
2026-10-19

## Context

F-001 parses the blueprint (2,235 lines today; NFR-24 allows 20,000) into typed stages, steps, items and packet formats. **Annotations are the only source** of what Foreman acts on (R-05). Every error needs a file, line and column. Template load must take p95 ≤ 50 ms (NFR-03). The `fm:` grammar v1 is fixed by `spec/blueprint-v2.2/PROPOSAL.md` §B-1:

- `fm:grammar`, `fm:step` and `fm:verify` block-level HTML comments;
- `fm:if <expr>` … `fm:end` around conditional parts;
- `[optional: …]` parts.

r1 §A1 probed comrak and pulldown-cmark on the real blueprint: both produce identical structure; comrak gives `sourcepos` on every node.

## Decision

1. **Markdown:** `comrak` **=0.56.0**, `default-features = false` (verified 2026-10-19: max stable 0.56.0, 2026-10-06; `rust-version` 1.89; `pub fn parse_document<'a>(arena: &'a Arena<'a>, md: &str, options: &Options) -> Node<'a>`; `NodeValue::HtmlBlock` present in `src/nodes.rs`). Parse once with sourcepos, walk the AST once, and collect:
   - stages (`## Stage N:`);
   - steps (`### Prompt N.M:`, Gate prompts, Council setup, F.2);
   - RUN WITH blockquotes;
   - fenced `text` prompt blocks;
   - INSPECTION blockquotes and their bullets;
   - HANDOFF blockquotes;
   - packet formats (fences starting `=== HANDOFF PACKET`);
   - `HtmlBlock` nodes starting `<!-- fm:`.
2. **Grammar:** an owned, hand-written recursive-descent parser in `foreman-core::template::grammar`. There's no parser-generator dependency: the grammar is about 15 keys and the `when` expression language is `! & | ( )` over 12 IDs. Rules (each an error with line and column):
   - `fm:` inside a list item or inline: "annotations must be block-level";
   - an unknown key: an error within `v=1`, a warning for `v>1`;
   - values without spaces; repo-relative paths only, checked by `pathpolicy::check_write`;
   - `start` model, effort and mode vocabularies, with `bypassPermissions` **always** an error (SR-03);
   - unknown condition IDs;
   - `session=continue` with no chain, or a cwd that differs from its chain's;
   - verify entries ≠ bullets;
   - nested `fm:if`;
   - `auto` items: "auto items need Foreman v1.1", treated as attest.
3. **Item identity:** `normalize_item_text` (lowercase, collapse whitespace, strip emphasis, after CRLF/BOM/tab normalization) feeds SHA-256 per bullet. The lock stores `{textSha256, kind}` per item ID. A drift requires "re-confirm kind" (F-001 rule 6).
4. **Prose lint:** the r2 inference rules run over RUN WITH text and produce **warnings only**; they never supply values.
5. **Tokens:** `resolver::classify_tokens` sorts every bracket token into the six classes at parse time. An unknown token is a template error, never a guess.
6. **Cache:** `Template` is serialized to app data under the source SHA-256 and kept in memory by hash.

## Alternatives considered

- **pulldown-cmark 0.13.4.** Broader adoption (1,867 reverse deps), the same fidelity on the blueprint. Lost because it emits an event stream with byte ranges, so we'd build our own tree and line index. comrak's AST already matches the nesting we walk (blockquote → paragraph → inline code; list item → HTML block). It's the fallback.
- **A YAML/JSON template source with generated Markdown.** Rejected in r6 §6: the blueprint is edited as prose in retros, and a generator adds a second source of mistakes.
- **A parser generator (pest, nom) for the directive grammar.** More dependencies and error messages that are worse than hand-written ones for a grammar this small. nom's maintenance and pest's grammar files add little here.

## Consequences

- comrak ships breaking 0.x releases about monthly. The exact pin plus golden tests make each upgrade deliberate.
- Steps without `fm:step` are `needs-annotation`: shown, never runnable (Stages 6–18 until a later blueprint release).
- The parser has no I/O. Lock restore reads blueprint text through git (ADR-012) and hands it in.

## Verification

- `template::golden_v2_2` (insta) on the real annotated blueprint: zero errors; a snapshot of Stages 1–5, the Gate and F.2.
- Property tests: random valid annotations round-trip; key order doesn't matter; arbitrary input never panics (`template::never_panics`).
- `template::rejects_bypass_permissions`, `template::swapped_bullets_reconfirm` (R-06), `template::prose_change_lints_only`.
- `criterion` `bench_template_load`: real and synthetic 20,000-line files, p95 ≤ 50 ms.

## Revisit when

- comrak drops sourcepos or `HtmlBlock` fidelity.
- Grammar v2 needs nesting or expressions beyond boolean logic.

## Exit path

Swap the AST walker to pulldown-cmark behind the internal `BlockEvents` trait (stage, step, blockquote, fence, html-block with positions). The grammar parser and everything downstream consume `BlockEvents`, not comrak types.
