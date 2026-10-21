=== HANDOFF PACKET · Stage 1 → Stage 2 ===
PROJECT: TradeLog
APPROVED BY: Billy Ribeiro on 2026-10-20
APP: TradeLog — the trade journal that tells you whether you have an edge
FOR: active retail and prop traders | also: trading coaches
PROBLEM: "I log trades in a spreadsheet and still can't tell if my setups work" (42 of 60 surveyed)
DOES:
  - imports fills from brokers and groups them into trades
  - computes expectancy, R-multiple and drawdown per setup
CORE LOOP: log or import the day's trades, tag setups, review the edge report weekly
WOW MOMENT: the first import shows expectancy per setup in under a minute
WITH (must integrate): CSV from IBKR, Tradovate, Thinkorswim
WITH (constraints): $0 cash; 10 hours a week for 12 weeks
STACK PREFERENCE: Svelte 5 + SvelteKit 3, TypeScript strict, pnpm · Rust + Axum · Postgres (Stage 2 decides on evidence)
NOT DOING (v1): automated trading, social feeds
MONETIZATION HYPOTHESIS: $19/month, 14-day trial; 31% of the waitlist said they'd pay
SUCCESS METRICS: 40% week-4 retention; 5% trial-to-paid
KILL CRITERION: under 100 waitlist signups in 7 days
RISKIEST ASSUMPTION: traders will import instead of journaling by hand; the CSV importer tests it
PRESS RELEASE + FAQ: attached below the packet
OPEN ITEMS: none
NEXT: Stage 2 · Architect's Design Studio · chat + Claude Code council
