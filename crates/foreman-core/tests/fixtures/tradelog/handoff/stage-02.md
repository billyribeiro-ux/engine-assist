=== HANDOFF PACKET · Stage 2 → Stage 3 ===
PROJECT: TradeLog
APPROVED BY: Billy Ribeiro on 2026-10-21
POSITIONING: TradeLog is the journal that answers "do I have an edge?". It beats TraderSync and Edgewonk on import speed and statistics. It's built by traders for traders.
V1 FEATURES: F-001 broker import; F-002 setup tagging; F-003 edge report
DIFFERENTIATORS: F-003 beats Edgewonk's stats by per-setup expectancy with confidence intervals
STACK (FINAL): frontend: SvelteKit 3
  backend: Rust + Axum
  data: Postgres
NFRs: p95 API < 200 ms; 99.9% availability
PRICING SIGNALS: $19–$29/month across competitors
DATA WE HANDLE: personal data (email), brokerage trade history
OPEN ITEMS: none
NEXT: Stage 3 · Permits & Zoning · Claude chat · Opus 5.5
FILES: spec/MASTER-SPEC.md, spec/council/r1…r8, spec/02-teardown.md
