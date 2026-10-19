# ADR-019: Pin TypeScript 6.0.3 (strict) until the Svelte and ESLint toolchain accepts TypeScript 7

## Status
Proposed

## Date
2026-10-19

## Context

NFR-45 requires TypeScript strict plus `noUncheckedIndexedAccess` and `exactOptionalPropertyTypes`, with zero errors and warnings in `svelte-check` and ESLint. TypeScript 7.0 (the native port) is npm `latest`, but r1 found it "ships without a stable programmatic API, which the team expects to land in 7.1". Svelte tooling and typescript-eslint use that API.

Registry check, 2026-10-19:

- `typescript`: `latest` 7.0.2, `next` 7.1.0-dev.20261019.1; 6.0.3 released 2026-04-16.
- `@sveltejs/kit` 3.1.0 peers `typescript ^6.0.0`.
- `svelte-check` 4.7.7 peers `typescript ^5.0.0 || ^6.0.0`.
- `typescript-eslint` 8.72.0 peers `typescript >=4.8.4 <6.1.0`.

## Decision

- `typescript` **6.0.3**, exact in `package.json`. `tsconfig.json` extends `$app/tsconfig` (ADR-002) with `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, `verbatimModuleSyntax`.
- `svelte-check --fail-on-warnings` and ESLint (`typescript-eslint` 8.72.0 type-aware rules) are part of `pnpm verify` and CI.
- A pnpm override forbids any second TypeScript major in the tree (`pnpm why typescript` in CI shows exactly one version).

## Alternatives considered

- **TypeScript 7.0.2 now.** Faster type checking, but svelte-check, typescript-eslint and SvelteKit 3.1 don't accept it by their peer ranges; mixing `tsgo` for `tsc` with TS 6 for tools would give two type systems.
- **JavaScript with JSDoc types.** Weaker enforcement; doesn't meet NFR-45's strictness in Svelte components.
- **TypeScript 6.0 latest patch with a caret range.** A caret would admit 6.1, which `typescript-eslint` 8.72.0 excludes (`<6.1.0`). The exact pin avoids accidental breakage.

## Consequences

- We forgo TS 7's speed for now; the project is small, so the type-check time is acceptable.
- Upgrades are deliberate: when all three peers accept 7.x, one PR bumps them together.

## Verification

CI: `pnpm verify` (svelte-check 0 errors / 0 warnings, ESLint clean), `pnpm-policy` (a single TypeScript version), and the bindings drift check (ADR-004) compiling against the pinned TS.

## Revisit when

`svelte-check`, `typescript-eslint` and `@sveltejs/kit` all list TypeScript 7.x in their peer ranges, and TS 7.1's stable programmatic API has shipped.

## Exit path

Bump `typescript`, `typescript-eslint`, `svelte-check` and kit in one change. Code needs no edits beyond what the new checker reports.
