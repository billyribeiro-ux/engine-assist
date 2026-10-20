# CR-001 · Allow the latest framework releases inside the 7-day release-age window

Status: APPROVED by Billy Ribeiro, 2026-10-20. He requires the latest Svelte 5, SvelteKit 3, pnpm and Tauri releases ("latest fucking updates as of Oct 19, 2026"; "PNPM latest as well").
Date: 2026-10-20

**What.** Exact-version exceptions to SR-61's 7-day minimum release age (`minimumReleaseAgeExclude` in `pnpm-workspace.yaml`):
- `@sveltejs/kit@3.1.0`
- `@tauri-apps/api@2.12.4`
- `@tauri-apps/cli@2.12.3`
- the CLI's 11 platform binaries at 2.12.3

**Why.** The product's core framework releases must be the latest. These are first-party releases published by the frameworks' own organizations.

**Impact.**
- **Scope:** none.
- **ADRs:** ADR-024 has a log entry for this change.
- **Requirements:** SR-61 gains a dated exception list. Dev tools still follow the 7-day rule.
- **Timeline and cost:** none.

**Risk.** A compromised release in the first week would be installed. Mitigations:
- the exceptions name exact versions only;
- each package comes from its framework's own organization;
- each version is reviewed before it's added;
- `pnpm audit` and `cargo audit` still run.

**Expiry.** Remove each entry once its version is 7 days old: all three versions can be dropped on 2026-10-21. For future framework releases, add a new dated entry with the same review, recorded in the ADR-024 log.
