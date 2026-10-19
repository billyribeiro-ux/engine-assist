# The Claude Build Blueprint

**From "I want to build something" to Certificate of Occupancy: deployed, battle-tested, and earning.**

Version 2.1 · October 19, 2026 · Models covered: Claude Fable 5.1, Opus 5.5, Sonnet 5.5, Haiku 5.5

Worked example throughout: **TradeLog**, a trade-journal web app. Every prompt works for any project. Swap the Project Variables.

---

## How this blueprint works

You build software the way you build a house. You don't call a framing crew first. You go to an **architect** and say "I want a house." The architect asks how you live, what you need, what you dream about, and what the budget is. Then the architect's studio designs the entire house. Only after the drawings are approved and the permits are pulled does construction start, one trade at a time, each one inspected before the next.

This blueprint runs **19 stages (0–18) in strict order**:

| Phase | Stages | Where it happens | Who leads |
|---|---|---|---|
| **Pre-Construction** | 0–4 | Claude chat (one claude.ai Project per app), plus Claude Code for the design council | Claude as your **architect** and design studio; you approve |
| **Construction** | 5–15 | Claude Code (CLI and Desktop) | Claude as **builder and trades**; you inspect |
| **Occupancy** | 16–17 | Claude Code | Launch, real users, real money |
| **Ownership** | 18 | Both | Operate and grow |

### The relay: every stage hands a packet to the next

This is the core mechanic. **Every stage ends by producing a Handoff Packet:** a structured block with fixed headings that contains everything the next stage needs, organized exactly how the next prompt expects it. The next stage's first prompt begins with that packet.

```handoff
=== HANDOFF PACKET · Stage N → Stage N+1 ===
PROJECT:      [APP_NAME]
APPROVED BY:  [you, the DRI] on [date]
LOCKED:       decisions that can't change without a Change Request
INPUTS:       everything the next stage needs, under fixed headings
OPEN ITEMS:   none   ← must be empty, or the stage hasn't passed
NEXT:         Stage N+1 · surface · model · effort · mode
```

- **Chat stages (0–4):** the packet comes out as a fenced block. Paste it into the next prompt, and also save it to the claude.ai Project's knowledge.
- **Claude Code stages (5–18):** the packet is a file, `docs/handoff/stage-NN.md`. The next prompt tells Claude Code to read it. Files beat pasting in Claude Code: they're versioned, reviewable, and survive new sessions.

### Every prompt has a RUN WITH line

> **RUN WITH** · Surface · **Model** · effort **level** · **mode** · agents
>
> `the exact command or the menu settings to use`

How to set each one:

| Surface | Model | Effort | Mode |
|---|---|---|---|
| **Claude chat** | Model picker in the chat box | Adaptive: the chat sets thinking depth itself | n/a |
| **CLI, new session** | `claude --model opus` (aliases: `fable`, `opus`, `sonnet`, `haiku`, `opusplan`) | `--effort high` | `--permission-mode plan` (others: `default` = Manual, `acceptEdits`, `auto`, `dontAsk`) |
| **CLI, mid-session** | `/model sonnet` | `/effort medium` | Shift+Tab cycles modes |
| **Desktop Code tab** | Model menu: Cmd+Shift+I | Effort menu: Cmd+Shift+E | Mode menu: Cmd+Shift+M |

Always pass `--permission-mode` explicitly in the CLI. Recent versions start interactive sessions in **Auto** mode by default.

> **THE GOLDEN RULE**
> The expensive model decides; the cheap model executes.
>
> - **Decisions** (what to build, scope, architecture, money math, security, review) → **Opus 5.5**, or **Fable 5.1** when the work is long or genuinely ambiguous.
> - **Execution** of an approved spec → **Sonnet 5.5**.
> - **Mechanical work** (search, running tests, renames, formatting) → **Haiku 5.5**.
>
> Every hour spent in Pre-Construction saves many hours later, because cheaper models can only execute what has already been decided.

<!-- -->

> **YOU ARE THE DRI**
> Apple's practice of naming one **Directly Responsible Individual** for every outcome applies here: you are the DRI at every gate. Claude researches, designs, builds, and audits; you approve every Handoff Packet. Nothing advances without your sign-off.

---

## Project Variables

Fill these in once. Every prompt references them. TradeLog values are shown as the example.

| Variable | Meaning | TradeLog example |
|---|---|---|
| `[APP_NAME]` | Product name | TradeLog |
| `[ROUGH_IDEA]` | Your idea in your own words, however rough | "A journal where traders log trades and finally see if they actually have an edge" |
| `[TARGET_USERS]` | Who pays | Active retail and prop traders |
| `[MY_ADVANTAGES]` | Unfair advantages: audience, skills, data, distribution | 18,000-trader community; institutional trading background; Pine Script and ThinkScript expertise |
| `[BUDGET_TIME]` | Money and hours available | [your numbers] |
| `[PREFERRED_STACK]` | Stack you prefer (Claude still recommends on evidence first) | Svelte 5 + SvelteKit 3, TypeScript strict, pnpm · Rust + Axum · Postgres |
| `[QUALITY_BAR]` | Non-negotiables | Zero TS errors/warnings; clippy `-D warnings` clean; a test for every domain rule; WCAG 2.2 AA; p95 API < 200 ms; 99.9% uptime SLO |

Stack, payments, and hosting are decided **by evidence in Stage 2**, not filled in here.

---

## The Crew: models, effort, modes, agents

### Models

| Model | API $/MTok (in / out) | Job-site role | Default effort |
|---|---|---|---|
| **Fable 5.1** | $10 / $50 | Master builder for long autonomous work and hard ambiguity. Slowest. **Pro plan:** usage credits only. **Max plan:** up to 50% of the weekly limit, and it burns that limit faster | high |
| **Opus 5.5** | $4 / $20 | Architect and site foreman: plans, decides, reviews, builds features | medium |
| **Sonnet 5.5** | $2 / $10 | Skilled tradesperson: executes spec-complete work fast | medium (Claude Code) |
| **Haiku 5.5** | from $0.10 / $0.50 | Laborer: search, test runs, mechanical edits. Costs more per token on prompts over 100K | medium |

### Effort levels

| Level | Use it for |
|---|---|
| **low** | Quick exchanges you review step by step: small edits, renames, first sketches |
| **medium** | Clear-scope feature work (the default on Opus, Sonnet, and Haiku 5.5) |
| **high** | Bugs in existing code, audits, anything where edge cases are likely |
| **xhigh** | Deep reasoning: architecture, money math, concurrency, security design |
| **max** | Only the hardest problems. Applies to the current session only. Can overthink |
| `ultrathink` | One word in a Claude Code prompt: one deep turn without changing the session's effort |

### Permission modes (Claude Code)

| Mode | Behavior | Use it for |
|---|---|---|
| **Plan** | Read-only exploration, then a plan for you to approve | The start of every non-trivial task |
| **Manual** (`default`) | Asks before each edit and command | Money, auth, migrations, production |
| **Accept edits** | Auto-accepts file edits; asks before other commands | Normal building |
| **Auto** | A classifier vets each action | Long, low-supervision runs |
| **dontAsk** | CLI only. Denies anything not explicitly allowed | CI and scripts |
| **Bypass** | No prompts | Containers or VMs only, never your machine |

### Agents (full definitions in Appendix A; council agents in Stage 2)

| Agent | Model · Effort | Role | Modeled on |
|---|---|---|---|
| `Explore` | Haiku · low | Read-only code search | — |
| `test-runner` | Haiku · low | Runs checks, reports failures only | — |
| `implementer` | Sonnet · medium | Builds one spec-complete ticket, in its own worktree | — |
| `code-reviewer` | Opus · high | Reviews every change before merge | Google's code review standard |
| `debugger` | Opus · high | Root cause plus regression test | — |
| `design-reviewer` | Opus · high | Independent design review | Google design reviews |
| `security-reviewer` | Opus · high | Threat model and security verification | Microsoft SDL |
| `sre-reviewer` | Opus · high | Production readiness and launch review | Google SRE PRR and launch reviews |

---

## The Stage Map

| # | Construction stage | Software meaning | Surface | Model · Effort |
|---|---|---|---|---|
| 0 | Land Survey *(optional)* | Find where the money is | Chat + Research | Opus 5.5 |
| 1 | **Meet the Architect** | "I want to build X": discovery, brainstorm, validation → App Brief | Chat | Opus 5.5 |
| 2 | **Architect's Design Studio** | Competitor teardown, ultimate feature set, stack decision → frozen Master Spec | Chat + CLI council | Opus 5.5 / Fable 5.1 · xhigh/high |
| 3 | Permits & Zoning | Business model, pricing, security/privacy/legal requirements | Chat | Opus 5.5 |
| 4 | Construction Drawings | Design doc, ADRs, threat model, data/API, design system, tickets → Build Package | Chat + design review | Opus 5.5 (Fable 5.1 review) |
| 5 | Site Prep & Excavation | Repo, CI with security gates, environments, staging deploy | CLI | opusplan · medium/high |
| 6 | Foundation | Database, migrations, auth, domain core | CLI + Desktop | Opus 5.5 · high/xhigh |
| 7 | Framing | Walking skeleton end to end | Desktop | Opus/Sonnet · medium |
| 8 | Roofing | Security controls from the threat model | CLI | Opus 5.5 · xhigh |
| 9 | Plumbing | Data flows: imports, uploads, jobs, email | Desktop | Opus/Sonnet · medium–xhigh |
| 10 | Electrical | Payments and entitlements | CLI | Opus 5.5 · xhigh |
| 11 | HVAC | Observability, SLOs, error budgets, analytics | Desktop | Sonnet 5.5 · medium |
| 12 | Insulation & Drywall | Complete every feature, every state | Desktop | By ticket class |
| 13 | Siding & Exterior | Design system, polish, accessibility | Desktop | Opus/Sonnet · medium |
| 14 | Inspections | Final security review, pen test, fuzzing, load, architecture audit | CLI (fresh agents) | Opus 5.5 · high; Fable 5.1 audit |
| 15 | Paint & Trim | Copy, onboarding, SEO, emails, legal pages | Chat + Desktop | Opus → Sonnet · medium |
| 16 | Final Walkthrough | Production readiness review, launch review, drills, dogfood, beta | CLI | Opus 5.5 · high |
| 17 | **Certificate of Occupancy** | Ring-based rollout, live payments, first revenue | CLI | Sonnet 5.5 · Manual |
| 18 | Homeowner Maintenance | Incidents, postmortems, upgrades, change requests | All | By task |

---

## The Crew Loop: per-ticket build cycle

Stages 6–13 are built ticket by ticket with this loop. Wherever a stage says **run the Crew Loop**, it means this.

### Route each ticket by its class

| Ticket class | Model · Effort | Mode | Notes |
|---|---|---|---|
| **SPEC-COMPLETE** | Sonnet 5.5 · medium | Auto or Accept edits | Or delegate to `implementer` |
| **NEEDS-DESIGN** | Opus 5.5 · medium | Plan → Accept edits | Plan first |
| **HARD** (money, math, auth, concurrency, security) | Opus 5.5 · xhigh | Plan → Manual | You review every diff |
| **MECHANICAL** | Haiku 5.5 · low | Accept edits | Batch several |
| **BUG** | Opus 5.5 · high | Plan → Accept edits | Or `debugger` |
| **STUCK / VAGUE** | Fable 5.1 · high | Auto | Describe the outcome, not the steps |

### Loop step 1: Plan

> **RUN WITH** · Desktop, new session, worktree ON · **model and effort from the table above** · **Plan** mode · agents: `Explore`
>
> Desktop: Cmd+Shift+I (model) · Cmd+Shift+E (effort) · Cmd+Shift+M → Plan

```text
Ticket [ID] from docs/tickets.md. Read the ticket, every ADR it lists,
docs/domain-rules.md, and the latest docs/handoff/stage-NN.md. Use the
Explore agent to find every file this ticket touches.
In plan mode, give me:
1. the implementation plan: an ordered file list with the change in each
2. the tests you'll write, mapped one-to-one to the acceptance criteria
3. any ambiguity or ADR conflict, as numbered questions
Do not write code yet.
```

### Loop step 2: Build, tests first

> **RUN WITH** · same session · same model · **Accept edits** (HARD class: **Manual**) · agents: `test-runner`
>
> Cmd+Shift+M → Accept edits (or Manual)

```text
Approved. Write the tests for every acceptance criterion first and show
me they fail. Then implement until they pass. Afterwards, use the
test-runner agent for the full suite. For UI work, verify in the Browser
pane at desktop and mobile widths, including empty, loading, error, and
permission-denied states.
```

**SPEC-COMPLETE shortcut** (replaces steps 1–2):

> **RUN WITH** · Desktop · **Sonnet 5.5** · effort **medium** · **Accept edits** · agents: `implementer`, `test-runner`, `code-reviewer`

```text
Use the implementer agent on ticket [ID] from docs/tickets.md. When it
reports back, run the test-runner agent, then the code-reviewer agent.
```

### Loop step 3: Review

> **RUN WITH** · same session · switch to **Opus 5.5** · effort **high** · **Accept edits** · agents: `code-reviewer`
>
> Run `/code-review` first, use **Walk through in diff**, then:

```text
Use the code-reviewer agent on this diff. Fix every CRITICAL and WARNING.
List the SUGGESTIONs and Nits and I'll decide.
```

### Loop step 4: Commit and PR

> **RUN WITH** · same session · **Sonnet 5.5** · effort **low** · **Accept edits**

```text
Write a conventional commit message referencing [ID] and the ADRs it
touches. Open a PR whose description lists each acceptance criterion and
how it was verified, plus the test commands run.
```

### Loop step 5: Close out

In Desktop, click **CI** in the status bar and turn on **Auto-fix CI & address comments**. When the PR merges, archive the session. That frees the worktree and retires its context.

---

## The Stage Gate: closing every Claude Code stage

Every Claude Code stage (5–17) ends with the same three prompts: inspection, handoff packet, and lessons. The stage-specific inspection list is in each stage.

### Gate prompt A: Inspection

> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** mode · agents: `test-runner` + the stage's reviewer agent
>
> `claude --model opus --effort high --permission-mode plan`

```text
Run the Stage [N] inspection for [APP_NAME]. Read docs/handoff/stage-[N-1].md
for context. For each inspection item below, verify it with real evidence
(command output, test names, file paths, URLs) and mark PASS or FAIL.
Use the test-runner agent for every automated check. Do not fix anything;
just report.
INSPECTION ITEMS:
[paste the stage's inspection list]
```

### Gate prompt B: Handoff Packet

> **RUN WITH** · same session · **Opus 5.5** · effort **medium** · Shift+Tab → **Accept edits**

```text
All items PASS. Write docs/handoff/stage-[N].md in exactly this format:

=== HANDOFF PACKET · Stage [N] → Stage [N+1] ===
PROJECT: [APP_NAME]
APPROVED BY: <leave blank for me to sign>
COMPLETED: what this stage delivered (bullets, with file paths)
EVIDENCE: each inspection item → PASS + its evidence
DECISIONS: any new or amended ADRs (IDs + one line each)
STATE: what now exists and works (URLs, commands, environments)
KNOWN LIMITS: deliberate deferrals, each with its ticket ID
OPEN ITEMS: none
NEXT: Stage [N+1] · <name> · <surface> · <model> · <effort> · <mode>
INPUTS FOR NEXT STAGE: the files, tickets, and facts it must read first

Commit it with the message "handoff: stage [N]".
```

### Gate prompt C: Lessons capture

> **RUN WITH** · same session · **Opus 5.5** · effort **medium** · **Accept edits**

```text
Before we close Stage [N], capture what this stage taught us. Review
this session, the PR history for the stage, and every finding the
reviewers raised. Append an entry to docs/lessons.md in the format at
the top of that file, covering: anything I had to fix or redo, anything
a reviewer or gate caught that the builder should have caught, any
prompt that produced weak output, any agent that missed something, and
anything that wasted usage. For each, propose the exact edit (to a
prompt, an agent file, CLAUDE.md, or a gate) that would prevent it next
time. If nothing went wrong, write "clean stage" with one line on why.
```

You read it, add your name and date under APPROVED BY, and commit. **Then** the next stage starts.

---

# PART A: PRE-CONSTRUCTION

## Stage 0: Land Survey. Find where the money is

*Optional. Use it when you know you want to build, but not what. Before buying land, you survey: soil, zoning, what the neighborhood sells for. Here, Claude researches where demand, willingness to pay, and your unfair advantages overlap. If you already know what you want to build, skip to Stage 1.*

### Prompt 0.1: Opportunity scan

> **RUN WITH** · Claude chat, new Project "Opportunity Research" · **Opus 5.5** · effort adaptive · **Research ON**

```text
Act as a venture analyst and a principal engineer together. Use Research
and web search to find software products I can build that have proven,
growing demand and clear willingness to pay, where my advantages give me
an edge.

My advantages: [MY_ADVANTAGES]
Budget and time: [BUDGET_TIME]
Stack I build in best: [PREFERRED_STACK]

Method: find demand evidence, not opinions. Look for people paying for
clunky tools, recurring complaints in reviews and forums, search-trend
growth, job posts describing manual work a product could replace, and
spreadsheet workflows people share. For each candidate, find the
incumbents, their pricing, their scale (public numbers only), and their
top complaints. Prefer recurring-revenue B2B or prosumer products.

Deliver 10 candidates, each with: the problem and who pays, demand
evidence (cited), incumbents and their weaknesses (cited), a realistic
price point, how my advantages create an edge, build size (S/M/L) for a
solo principal engineer using Claude Code, and the biggest risk.
Score each 1-5 on demand, willingness to pay, competition gap, my edge,
build speed, and defensibility. Show a table sorted by total.
Cite every factual claim. Mark estimates as estimates.
```

### Prompt 0.2: Red-team and pick

> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
Take the top 3. Argue as hard as you can that each will fail. Then pick
the one that survives best, and design a 7-day experiment that would
confirm or kill it.

When I say "approved", output the OPPORTUNITY PACKET exactly in this
format, inside one code block:

=== HANDOFF PACKET · Stage 0 → Stage 1 ===
PROJECT: <working name>
APPROVED BY: <blank>
IDEA: <one sentence>
FOR: <who pays, specifically>
PROBLEM: <their pain, in their words>
EVIDENCE: <the 5 strongest demand signals, each with a link>
INCUMBENTS: <name — price — biggest weakness>, one per line
PRICE POINT: <estimate + reasoning>
MY EDGE: <why I win>
TOP RISKS: <3 bullets>
VALIDATION EXPERIMENT: <the 7-day test + pass threshold>
OPEN ITEMS: none
NEXT: Stage 1 · Meet the Architect · Claude chat · Opus 5.5
```

> **INSPECTION: pass before Stage 1**
>
> - 10 candidates, each with cited demand evidence
> - The pick wins on evidence, not on excitement
> - A validation experiment with a numeric pass threshold

<!-- -->

> **HANDOFF** → the Opportunity Packet. Save it to the new `[APP_NAME]` Project's knowledge.

---

## Stage 1: Meet the Architect. "I want to build…"

*You walk into the architect's office and say: "I want a house." A great architect doesn't start drawing. They ask how you live, who lives there, what you love and hate about your current place, your budget, your dream rooms. They sketch options, push back, and refine with you until the vision is clear. Then they write the design brief everything else follows.*

*Here, Claude is your architect. You bring the rough idea (or the Stage 0 packet). Claude interviews you, brainstorms with you, challenges you, and validates demand with you. When you both agree, it hands you the **App Brief**.*

### Prompt 1.1: The architect interview

> **RUN WITH** · Claude chat, new Project `[APP_NAME]` · **Opus 5.5** · effort adaptive · web search ON

```text
You are my product architect: part principal product engineer, part
designer, part skeptical investor. I want to build something. You lead
this conversation the way a world-class architect leads a client
meeting: understand deeply before designing anything.

My idea, in my words: [ROUGH_IDEA]
(If I attached a Stage 0 Opportunity Packet, start from that.)
Who I think it's for: [TARGET_USERS]
My advantages: [MY_ADVANTAGES]
Budget and time: [BUDGET_TIME]

How to run this:
1. Restate the idea in one sentence and name its riskiest assumption.
2. Interview me in rounds of at most 5 numbered questions: the users and
   their day, the pain, what they use today, what "amazing" looks like,
   how they pay, how I reach them, and constraints. Wait for my answers
   before the next round.
3. Between rounds, reflect back what you've learned and challenge
   anything weak. Disagree when you should.
4. When you understand it, propose 2-3 distinct product concepts
   (different angles on the same problem), with the tradeoffs of each.
Do not design features in detail or pick technology yet.
```

### Prompt 1.2: Brainstorm and converge

> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
We're going with concept [X] (with these changes: [changes]). Now
brainstorm it fully with me:
- the core loop: what a user does daily or weekly, and why they return
- every feature idea worth considering, wild ones included, grouped by
  job-to-be-done
- the "wow" moment: the first-session experience that makes them stay
- what we will deliberately NOT do
Then narrow it to a v1 that still feels remarkable, and tell me what you
would cut and why.
```

### Prompt 1.3: Demand validation kit

> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
Before anything gets built, design a 7-day demand test:
1. Landing-page copy: headline, subhead, 3 benefits, a pricing teaser,
   and a pre-order or waitlist call to action
2. A 5-question survey that reveals willingness to pay (Van Westendorp
   pricing questions plus "what do you use today?")
3. Pass/fail thresholds that justify building
4. The exact message to send to my audience
```

Run the test yourself, then paste the numbers into the same chat.

### Prompt 1.4: The App Brief (the handoff)

> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive

```text
Here are the validation results: [paste numbers]. If they missed the
threshold, tell me plainly and stop. Otherwise, write a press release
for launch day as if the product already exists (Amazon "working
backwards" style): headline, the problem, the solution, a customer
quote, and how to get started. Add an FAQ of the 8 hardest questions a
customer and an investor would ask, with answers.

Then, when I say "approved", output the APP BRIEF exactly in this
format, inside one code block:

=== HANDOFF PACKET · Stage 1 → Stage 2 ===
PROJECT: [APP_NAME]
APPROVED BY: <blank>
APP: <name> — <one-line pitch>
FOR: <primary persona> | also: <secondary personas>
PROBLEM: <the pain, in the users' words, with validation data>
DOES:
  - <capability 1, one sentence>
  - <capability 2>
  - ...
CORE LOOP: <what they do daily/weekly and why they return>
WOW MOMENT: <the first-session moment>
WITH (must integrate): <data sources, platforms, imports, devices>
WITH (constraints): <budget, time, compliance, platforms>
STACK PREFERENCE: [PREFERRED_STACK] (Stage 2 decides on evidence)
NOT DOING (v1): <non-goals>
MONETIZATION HYPOTHESIS: <model + price point + evidence>
SUCCESS METRICS: <activation, retention, conversion, revenue, with numbers>
KILL CRITERION: <the result that means we stop>
RISKIEST ASSUMPTION: <and how v1 tests it>
PRESS RELEASE + FAQ: <attached below the packet>
OPEN ITEMS: none
NEXT: Stage 2 · Architect's Design Studio · chat + Claude Code council
```

> **INSPECTION: pass before Stage 2**
>
> - The validation test hit its threshold. If not, go back to Stage 0 or the next concept. That's a win: you saved months.
> - The press release excites a real target user you show it to
> - The App Brief's OPEN ITEMS line is empty

<!-- -->

> **HANDOFF** → the App Brief + press release and FAQ. Save them to Project knowledge and copy them into a new folder `[app]-spec/01-app-brief.md`.

---

## Stage 2: Architect's Design Studio. Design the entire product

*The architect takes your brief back to the studio. The whole team works on it: the lead architect, the structural engineer, the interior designer, the cost estimator, and a skeptic who attacks every idea. They tour the best houses in the neighborhood, list what each one gets right and wrong, design a house that beats them all, and choose materials on evidence. The design is locked before construction drawings begin.*

*Here, Claude orchestrates a council of specialist agents in Claude Code. They tear down every competitor, mine real user pain, invent features that beat the market, attack the plan, recommend the stack on evidence, and freeze a **Master Product Spec**.*

> **WHY THE COUNCIL RUNS IN CLAUDE CODE**
> Claude Code runs several subagents in parallel, each in its own context window with its own model, effort, and web access. The chair session stays clean and only sees their summaries. A single chat thread can't do real parallel orchestration.

### Prompt 2.1: Competitor teardown

> **RUN WITH** · Claude chat, `[APP_NAME]` Project · **Opus 5.5** · effort adaptive · **Research ON**

```text
Here is the approved App Brief:
[paste the Stage 1 Handoff Packet]

Research every serious competitor for this product, including the
adjacent tools users cobble together (spreadsheets, scripts, plugins).
Find the 8-12 most relevant. For each: pricing and tiers (verify on
their site), the complete feature list grouped by job-to-be-done, what
users praise most and their top complaints (reviews, Reddit, forums,
app stores, with links), onboarding friction, performance, design
quality, integrations, and visible strategic direction (changelog,
roadmap, job posts).
Then build:
1. A feature matrix: competitors × features (has / partial / missing)
2. A pain map: the 15 most frequent unmet needs, ranked by frequency and
   intensity, each with source links
3. Table stakes: the features every serious product has, which we must match
Cite everything and mark estimates.
```

Save the result to `[app]-spec/02-teardown.md`.

### Council setup (once): agents in `[app]-spec/.claude/agents/`

Create these six files. Copy the folder for every future project.

```markdown
---
name: market-analyst
description: Competitive and market research specialist. Use for feature teardowns, pricing, and positioning evidence.
tools: Read, Write, WebSearch, WebFetch
model: sonnet
effort: medium
---
You research markets with evidence. Every claim gets a source link.
Separate facts from estimates. Write findings to the file path you're
given and return a 10-line summary.
```

```markdown
---
name: user-advocate
description: Voice of the user. Use to mine real complaints, workflows, and jobs-to-be-done.
tools: Read, Write, WebSearch, WebFetch
model: sonnet
effort: medium
---
You represent the paying user. Mine reviews, forums, and communities
for real pain, quoting briefly and linking every source. Describe the
jobs users hire products for and where current tools fail them. Write
to the given file and return a summary.
```

```markdown
---
name: product-visionary
description: Inventor of differentiated features. Use to design features that beat every competitor.
tools: Read, Write, WebSearch
model: opus
effort: high
---
You design features that make users switch. For each pain point,
propose the best possible solution, not the incremental one: 10x
improvements, workflows that remove whole steps, combinations no
competitor offers. For each feature: the problem, the solution, why it
beats each competitor's version, and how we'd measure success.
```

```markdown
---
name: principal-engineer
description: Feasibility, architecture, and stack evaluation. Use to cost features and judge technical risk.
tools: Read, Write, WebSearch, WebFetch
model: opus
effort: high
---
You judge feasibility for a solo principal engineer using Claude Code.
For each feature: build size (S/M/L/XL), technical risk, data and
infrastructure implications, and performance concerns. For stack
questions, ground every claim in current official documentation and
real adoption evidence, cited. No claims from memory.
```

```markdown
---
name: growth-strategist
description: Monetization, retention, and distribution. Use to judge which features drive revenue.
tools: Read, Write, WebSearch
model: sonnet
effort: medium
---
You judge every feature by revenue impact: acquisition, activation,
retention, expansion, and tier fit. Identify the habit loop, the
referral or community loop, and which features justify the paid tier.
```

```markdown
---
name: red-team
description: Adversarial reviewer. Use to attack the proposed product before it's frozen.
tools: Read, Write, WebSearch
model: opus
effort: high
---
Your job is to kill weak ideas. Attack every feature and decision:
nobody wants it, incumbents will copy it, too costly, too hard to
explain, legal or platform risk, dilutes focus. Rate each objection
fatal / serious / minor, with the evidence that would settle it.
```

### Prompt 2.2: Convene the council

> **RUN WITH** · Claude Code CLI in `[app]-spec/` · Chair: **Fable 5.1** · effort **high** (Max plan) — or **Opus 5.5** · effort **xhigh** (Pro plan / to save usage) · **Plan** mode first, then **Accept edits** · agents: all six council agents
>
> Max plan: `claude --model fable --effort high --permission-mode plan`  
> Pro plan: `claude --model opus --effort xhigh --permission-mode plan`

```text
You are the chair of the product design council for [APP_NAME].
Read 01-app-brief.md (the approved Stage 1 packet) and 02-teardown.md.
Goal: define a product clearly better than everything on the market,
specified completely before any code is written.

Run these rounds, using subagents in parallel wherever a round allows:
ROUND 1, Evidence (parallel):
 - market-analyst: fill the gaps in the teardown -> council/r1-market.md
 - user-advocate: deep pain mining + jobs-to-be-done -> council/r1-users.md
ROUND 2, Diverge (parallel, after reading Round 1):
 - product-visionary: the ultimate feature set -> council/r2-features.md
 - growth-strategist: revenue loops and tier fit -> council/r2-growth.md
 - principal-engineer: feasibility and cost of every proposed feature
   -> council/r2-feasibility.md
ROUND 3, Converge (you): score every feature on user value,
 differentiation vs. the best competitor, revenue impact, build cost,
 and risk. Sort into v1 (an MVP that still beats the market), v1.x, v2,
 and rejected (with the reason). -> council/r3-synthesis.md
ROUND 4, Attack: red-team attacks r3-synthesis.md -> council/r4-redteam.md
ROUND 5, Resolve (you): answer every fatal and serious objection, by
 changing the plan or citing evidence that defeats it.
 -> council/r5-resolution.md

First show me the plan for the rounds. After each round, give me a
5-line summary and wait for my "go".
```

### Prompt 2.3: Feature specifications

> **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits**

```text
ultrathink. For each v1 feature, write a specification a senior engineer
could build from: the user's starting situation, the exact behavior,
inputs and outputs, every edge case and failure state, and what "done"
means. Name the competitor whose version it beats and exactly why ours
is better. Flag anything ambiguous as a numbered question for me.
-> council/r6-feature-specs.md
```

### Prompt 2.4: Stack decision, evidence first

> **RUN WITH** · same session · same chair model · effort unchanged · **Accept edits** · agents: `principal-engineer`, `market-analyst`

```text
Recommend the best stack for [APP_NAME], layer by layer: frontend,
backend, data, cache/queue, search, file storage, auth, payments,
email, hosting, observability, CI/CD.

PART 1, evidence-first (ignore my preferences): for each layer, list the
requirements it must meet (from the feature specs and non-functional
needs), the 2-3 strongest candidates, and the winner. Judge on fit,
performance, maturity and support horizon, ecosystem and hiring pool,
security record, operating cost, and how well Claude Code works with it
(docs quality, tooling, training coverage). Have the principal-engineer
agent verify every version, support horizon, and capability claim
against current official docs and real adoption evidence, cited.

PART 2, compare with my preference: [PREFERRED_STACK]. For each layer
where you differ, say what I'd gain, what I'd lose, and your confidence
(high / medium / low). Where my preference is equal or within noise,
say so plainly.

PART 3: your final recommendation in one table, one line of reasoning
per layer. I make the final decision.
-> council/r7-stack.md
```

Record your final choice and reason at the bottom of `council/r7-stack.md`. Overriding Claude is fine; the written reason is what the ADRs build on.

### Prompt 2.5: Non-functional requirements

> **RUN WITH** · same session · same chair model · **Accept edits**

```text
Define the non-functional requirements as numbers: latency per endpoint
class, availability SLO and error budget, backup RPO/RTO, scale targets
at launch and at 100x, security level (OWASP ASVS level), accessibility
(WCAG level), browser and device support, data retention, privacy
requirements, and the maximum infrastructure cost per user per month.
-> council/r8-nfr.md
```

### Prompt 2.6: Freeze the Master Spec (the handoff)

> **RUN WITH** · same session · same chair model · **Manual** mode (Shift+Tab)

```text
Compile everything into MASTER-SPEC.md v1.0:
1. Vision and positioning: why we win, in 3 sentences
2. Competitor matrix (summary) and our differentiation per competitor
3. Feature catalog: IDs F-001…, each with the problem, the behavior,
   edge cases, release (v1 / v1.x / v2), and the competitor it beats
4. Non-goals and rejected ideas, with reasons
5. The stack decision table, with evidence links and my final choice
6. Non-functional requirements
7. Open questions. This list must be EMPTY to freeze; if anything is
   open, stop and ask me.
8. Change control: after v1.0, every change requires a Change Request
   (CR-NNN: what, why, impact on scope, ADRs, timeline, and cost)
   approved by me.
Put "Status: FROZEN v1.0 — <date>" at the top.

Then output the MASTER SPEC PACKET in one code block:

=== HANDOFF PACKET · Stage 2 → Stage 3 ===
PROJECT: [APP_NAME]
APPROVED BY: <blank>
POSITIONING: <3 sentences>
V1 FEATURES: <F-IDs + one-line each>
DIFFERENTIATORS: <what beats which competitor>
STACK (FINAL): <layer: choice> per line
NFRs: <the key numbers>
PRICING SIGNALS: <from teardown + growth round>
DATA WE HANDLE: <personal data, payment data, sensitive data classes>
OPEN ITEMS: none
NEXT: Stage 3 · Permits & Zoning · Claude chat · Opus 5.5
FILES: MASTER-SPEC.md, council/r1…r8, 02-teardown.md
```

Upload `MASTER-SPEC.md` and the packet to the `[APP_NAME]` claude.ai Project.

> **INSPECTION: pass before Stage 3**
>
> - The feature matrix covers every serious competitor, with sources
> - Every v1 feature beats a named competitor's version or is table stakes, and the spec says which
> - Every fatal or serious red-team objection is resolved in writing
> - The evidence-first stack recommendation exists for every layer, plus your recorded final choice
> - Open questions: zero. Status: FROZEN v1.0

<!-- -->

> **HANDOFF** → the Master Spec Packet + `MASTER-SPEC.md`

---

## Stage 3: Permits & Zoning. Business, security, privacy, legal

*No permit, no construction. You prove the house meets code and the budget works before breaking ground. Microsoft's Security Development Lifecycle starts the same way: security and privacy **requirements** are defined before design.*

### Prompt 3.1: Business model and pricing

> **RUN WITH** · Claude chat, `[APP_NAME]` Project · **Opus 5.5** · effort adaptive · web search ON

```text
Here is the frozen Master Spec Packet:
[paste the Stage 2 Handoff Packet]

Design the business model:
1. Pricing: tiers, what each includes, monthly vs annual, free trial or
   freemium (justified with competitor pricing, cited)
2. Unit economics per paying user per month: hosting, storage, email,
   payment fees, third-party APIs, any AI usage. Show the math and the
   gross margin; mark estimates.
3. Break-even subscriber count, plus 12-month revenue scenarios
   (conservative / base / strong) with explicit assumptions
4. Payment provider for the chosen stack: subscription support, a
   merchant-of-record option for sales tax/VAT, fees, payouts, disputes.
   Verify current terms on official sites.
5. Entitlement matrix: tier × feature (F-IDs)
```

### Prompt 3.2: Security, privacy, and compliance requirements

> **RUN WITH** · same chat · **Opus 5.5** · effort adaptive · web search ON

```text
Define the requirements we must design for, before any design exists:
1. Security requirements: authentication strength, session rules, data
   encryption in transit and at rest, secrets handling, audit logging,
   and the OWASP ASVS level from the NFRs
2. Privacy requirements: data classification for every data class in
   the packet, minimization, retention and deletion, user data export,
   consent, and the processors we'll share data with
3. Legal and compliance for selling to [TARGET_USERS] in [jurisdictions]:
   privacy laws (e.g., GDPR, CCPA), terms of service, refunds and
   cancellations, sales tax/VAT, cookie consent, accessibility
   obligations, and anything domain-specific
For each item: the requirement, which stage implements it, and whether
a lawyer must review it. This is a planning map, not legal advice.

When I say "approved", output this packet in one code block:

=== HANDOFF PACKET · Stage 3 → Stage 4 ===
PROJECT: [APP_NAME]
APPROVED BY: <blank>
PRICING: <tiers + prices>
ENTITLEMENTS: <tier: F-IDs>
UNIT ECONOMICS: <gross margin at each tier; break-even count>
PAYMENTS: <provider + why>
SECURITY REQUIREMENTS: <numbered, SR-01…>
PRIVACY REQUIREMENTS: <numbered, PR-01…, with data classes>
LEGAL/COMPLIANCE: <numbered, LC-01…, lawyer-review flagged>
OPEN ITEMS: none
NEXT: Stage 4 · Construction Drawings · Claude chat · Opus 5.5
```

TradeLog's domain items include a "not investment advice" disclaimer and safe handling of brokerage data imports.

> **INSPECTION: pass before Stage 4**
>
> - Gross margin is positive at the entry tier, with the math shown
> - Payment provider chosen and its current terms verified
> - Every security, privacy, and legal requirement is numbered and assigned to a stage

<!-- -->

> **HANDOFF** → the Business & Requirements Packet

---

## Stage 4: Construction Drawings. The complete build package

*The architect's concept becomes construction drawings: structural plans, electrical and plumbing schematics, materials lists, and specs every trade builds from. A senior reviewer who didn't draw them checks them before any trade sees them. Google works the same way: a design doc with alternatives considered and cross-cutting concerns, reviewed before implementation. Microsoft SDL puts **threat modeling** at design time.*

Each prompt below is its own chat in the `[APP_NAME]` Project. Paste the latest packet at the top of each.

### Prompt 4.1: Scope, milestones, and acceptance criteria

> **RUN WITH** · Claude chat, new conversation in the Project · **Opus 5.5** · effort adaptive

```text
Inputs: [paste the Stage 2 and Stage 3 Handoff Packets]
Write the SCOPE document for [APP_NAME]:
1. v1 features (by F-ID), with the non-goals from the Master Spec
2. Milestones M0-M6, each with a goal, its F-IDs, and a demoable outcome:
   M0 = repo, CI with security gates, environments, staging deploy
   M_last = hardening, readiness review, launch
3. For every feature: user stories with acceptance criteria in
   Given/When/Then form, covering empty states, errors, permissions,
   limits, concurrency, and tier entitlements
4. Map every SR-, PR-, and LC- requirement to the milestone that
   delivers it
Quality bar: [QUALITY_BAR]
```

### Prompt 4.2: Design doc and ADRs

> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** (switch to **Fable 5.1** if the system is platform-scale or genuinely ambiguous) · effort adaptive · web search ON

```text
Inputs: [paste the Stage 2 and 3 packets] + docs/scope.md (in Project knowledge)
Act as a principal architect designing for a 20-year product life.
Treat the final stack in the Stage 2 packet as fixed input.

PART 1: The design doc, in Google's structure:
- Context and scope
- Goals and non-goals
- The design: system-context diagram, components, data flow, trust
  boundaries, API sketch, data storage approach
- Alternatives considered, each with why it lost
- Cross-cutting concerns: security, privacy, observability,
  accessibility, cost, and migration/exit paths

PART 2: One ADR per significant decision (10-15), in exactly this format:
# ADR-NNN: <title>
Status: Proposed
Date: <today>
Context: <forces and requirements, including SR/PR/LC IDs>
Decision: <precise enough to implement>
Alternatives considered: <each, and why it lost>
Consequences: <positive, negative, risks, what becomes harder>
Verification: <how code, tests, or CI prove we follow it>
Revisit when: <the trigger that reopens it>
Exit path: <how we'd replace this choice>

Ground every choice in current official docs and verify versions.
Prefer boring, widely adopted technology with long support horizons.
Flag any low-confidence decision.
```

TradeLog's expected ADRs: monorepo layout · REST + OpenAPI with a generated TS client · session-cookie auth · sqlx + reversible migrations · money as integer minor units or decimals, never floats · pure-function stats engine · presigned uploads · shared validation schema · typed error envelope · entitlements as data · observability stack · testing strategy · environments, deploy rings, and rollback · data export.

### Prompt 4.3: Threat model

> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive

```text
Inputs: the design doc, the ADRs, and the Stage 3 packet (Project knowledge).
Build the threat model, Microsoft SDL style:
1. Data flow diagrams for each trust boundary (described, plus a diagram)
2. STRIDE analysis per element: spoofing, tampering, repudiation,
   information disclosure, denial of service, elevation of privilege
3. For each threat: likelihood, impact, the mitigating control, the
   SR/PR requirement it satisfies, and the test that will prove it
4. Abuse cases specific to this product (e.g., account sharing, scraping,
   payment fraud, data exfiltration through exports)
```

### Prompt 4.4: Data model, API contract, domain rules

> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive

```text
Inputs: the design doc, ADRs, threat model, and scope (Project knowledge).
Produce:
1. The Postgres schema as ordered, reversible SQL migrations: tables,
   constraints, indexes (each with the query it serves), foreign keys,
   and ownership rules
2. An OpenAPI 3.1 spec for every v1 endpoint: schemas, the error
   envelope, auth, pagination, rate limits, and API versioning
3. Domain invariants, numbered, each assigned to exactly one enforcing layer
4. Every computed value: the exact formula, units, rounding, edge cases,
   and a worked numeric example
```

For TradeLog, item 4 pins down expectancy, R-multiple, win rate, and max drawdown before any code exists.

### Prompt 4.5: Design system and key screens

> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive

```text
Inputs: the scope and Master Spec (Project knowledge).
Design the core screens of [APP_NAME]: [list screens], plus pricing,
upgrade prompts, and onboarding. Make them as a design I can review and
edit (the Design artifact type if available; otherwise one
self-contained HTML mockup).
Requirements: a distinctive, institutional-grade visual direction (never
a template look), dark mode first, data-dense but readable, WCAG 2.2 AA,
keyboard-first forms, mobile breakpoints. Show empty, loading, and error
states for every screen. Then list the design tokens (color, type
scale, spacing, radius, elevation, motion) as CSS custom properties.
```

### Prompt 4.6: Tickets

> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive

```text
Inputs: scope, ADRs, threat model, data model, design (Project knowledge).
Break every milestone into tickets sized for one Claude Code session
(1-4 hours of human work). For each ticket:
ID [M#-NN] | Title | Class: SPEC-COMPLETE / NEEDS-DESIGN / HARD /
MECHANICAL | Stage (6-13) | Depends on | ADRs | Threats mitigated |
Likely files | Acceptance criteria (Given/When/Then) | Required tests
(named) | Out of scope
Classify strictly: anything touching money, auth, math, or concurrency
is HARD.
```

### Prompt 4.7: Independent design review

> **RUN WITH** · Claude chat, a **brand-new conversation** (fresh context: it didn't write any of this) · **Fable 5.1** if available, otherwise **Opus 5.5** · effort adaptive

```text
You are a senior design reviewer who did not write any of these
documents. Review the design doc, ADRs, threat model, data model, API
spec, and tickets in this Project against the Master Spec and the NFRs.
Find: requirements with no design, designs with no requirement, ADRs
without real alternatives, threats without tests, schema or API gaps,
missing failure modes, tickets with untestable acceptance criteria, and
anything that will hurt in 5 years. Rate each finding BLOCKER / MAJOR /
MINOR, with the fix. Be specific and unsparing.
```

Fix every BLOCKER and MAJOR in the original chats, then mark all ADRs **Accepted**.

### Prompt 4.8: CLAUDE.md and the Build Package (the handoff)

> **RUN WITH** · Claude chat, new conversation · **Opus 5.5** · effort adaptive

```text
Generate the project CLAUDE.md. Keep it under 150 lines; it loads into
every Claude Code session, so every line costs tokens. Include only: a
one-line product description; the stack with exact versions; the repo
layout; exact commands (install, dev, typecheck, lint, unit,
integration, e2e, build, migrate); repo-specific conventions; the
definition of done; paths (not contents) to every doc; "never" rules
(e.g., never float money, never skip authz, never edit an applied
migration); and the rule "before planning any work, read the latest
docs/handoff/ packet and docs/lessons.md."

Then output the BUILD PACKAGE packet in one code block:

=== HANDOFF PACKET · Stage 4 → Stage 5 ===
PROJECT: [APP_NAME]
APPROVED BY: <blank>
REPO FILES (create exactly these):
  CLAUDE.md
  docs/MASTER-SPEC.md
  docs/scope.md
  docs/design-doc.md
  docs/adr/ADR-001.md …
  docs/threat-model.md
  docs/migrations/
  docs/openapi.yaml
  docs/domain-rules.md
  docs/design-tokens.md
  docs/tickets.md
  docs/requirements.md   (SR/PR/LC lists)
  docs/business-model.md
  docs/lessons.md        (empty, with the template header from Appendix F)
STACK + VERSIONS: <from ADRs>
MILESTONES: <M0…M_last, one line each>
FIRST STAGE: Stage 5 · Site Prep · Claude Code CLI · opusplan · medium · Plan
BLUEPRINT VERSION: <the version of this blueprint used>
OPEN ITEMS: none
```

> **INSPECTION: pass before Stage 5 (drawing sign-off)**
>
> - The independent review has zero open BLOCKER or MAJOR findings
> - Every requirement maps to a design element, and every threat maps to a control and a test
> - Every computed value has a formula and a worked example
> - Every ticket has a class, a stage, and named tests
> - Your test: you'd hand this package to a contractor you've never met

<!-- -->

> **HANDOFF** → the Build Package. Create the repo with exactly those files, then save the packet itself as `docs/handoff/stage-04.md`.

---

# PART B: STRUCTURE

## Stage 5: Site Prep & Excavation. Repo, CI, environments

*Clear the site, dig, and bring utilities to the lot. In software: the repo, toolchain, CI with security gates from day one (Microsoft SDL verification: static analysis, secret scanning, dependency governance), environments, and a staging deploy that works before any feature exists.*

| Crew for this stage | |
|---|---|
| Surface | Claude Code CLI |
| Models | opusplan for the scaffold · Opus 5.5 for the pipeline · Sonnet 5.5 for config |
| Agents | `test-runner`, `security-reviewer` |

One-time machine setup first: **Appendix A**.

### Prompt 5.1: Scaffold

> **RUN WITH** · CLI · **opusplan** (Opus plans, Sonnet executes) · effort **medium** · **Plan** mode · agents: `test-runner`
>
> `claude --model opusplan --effort medium --permission-mode plan`

```text
Read docs/handoff/stage-04.md, CLAUDE.md, docs/design-doc.md, and every
ADR. Plan the M0 scaffold exactly as the ADRs specify: the monorepo
layout, pnpm workspace, frontend app, backend service, shared types
package, local Postgres via docker compose, migration tooling, and
lint/format/typecheck configs at [QUALITY_BAR] strictness.
Use the latest stable versions, verified against official docs and the
registries, never from memory. Pin with lockfiles. List every file you
will create. Do not write files yet.
```

Approve the plan: Opus hands off and Sonnet executes. Then:

```text
Use the test-runner agent to run the full verification suite and fix
everything until it reports ALL GREEN.
```

### Prompt 5.2: CI with security gates

> **RUN WITH** · same session · `/model opus` · `/effort high` · Shift+Tab → **Plan** · agents: `security-reviewer`

```text
Plan a GitHub Actions CI pipeline that runs on every PR and blocks merge
on failure: typecheck, lint, unit, integration (real Postgres), build;
static analysis (CodeQL or Semgrep); secret scanning; dependency
vulnerability scanning (pnpm audit, cargo audit) and license checks;
SBOM generation for each build; and migration up/down tests. Add branch
protection: no direct pushes to main, every change via a reviewed PR.
Verify each tool's current setup in its official docs. Then implement
it after I approve.
```

### Prompt 5.3: ADR reality check

> **RUN WITH** · same session · **Opus 5.5** · effort **high** · **Plan**

```text
Now that real code and real library versions exist, re-check every ADR
against them and their current docs. For each, report VALID, NEEDS
AMENDMENT (what), or CONTRADICTED (evidence). Don't edit anything.
```

For each amendment you accept: *"Amend ADR-NNN with that change and add an* `Amended <date>: <reason>` *line."*

### Prompt 5.4: Automation and preview config

> **RUN WITH** · same session · `/model sonnet` · `/effort low` · **Accept edits**

```text
Create .claude/settings.json with a PostToolUse hook matching Edit|Write.
It reads tool_input.file_path from the hook's stdin JSON and formats
only that file (prettier for TS/Svelte, rustfmt for .rs), via
scripts/claude-format.sh, which must be executable.
Create .claude/launch.json with "web" (pnpm dev, port 5173, autoPort
true) and "api" (cargo run in apps/api, port 8080, autoPort false) so
the Desktop Browser pane can preview and auto-verify the app.
```

### Prompt 5.5: Staging pipeline

> **RUN WITH** · same session · `/model opus` · `/effort high` · **Plan** → **Accept edits**

```text
Implement the deploy ADR for staging: multi-stage Dockerfiles with
minimal runtime images, a non-root user, and healthchecks; signed build
artifacts with provenance. The deploy workflow runs only after CI passes
on main; migrations run before new code takes traffic; rollout is
health-checked; rollback is one command. Secrets come from the CI
secret store only. Write docs/runbook-deploy.md. List every secret I
must create by hand; never create or print secrets yourself.
```

### Stage 5 gate

Run the **Stage Gate** (prompts A, B, and C) with these items:

> **INSPECTION: pass before Stage 6**
>
> - A fresh clone, then `install` and `dev`, works with zero manual steps
> - CI is green, and an intentional type error, a planted fake secret, and a known-vulnerable dependency each fail it (prove the gates work, then revert)
> - Staging deploys automatically, and a rollback has actually been run once
> - The ADR reality check has no CONTRADICTED left open

---

## Stage 6: Foundation. Database, auth, domain core

*Everything sits on the foundation, and it's the most expensive thing to fix later. Here: migrations, authentication, and the core domain rules, each proven by tests.*

| Crew for this stage | |
|---|---|
| Surface | CLI for migrations and auth · Desktop for the domain core |
| Models | Opus 5.5 · high (migrations, auth) · xhigh (domain math) |
| Agents | `Explore`, `test-runner`, `code-reviewer`, `security-reviewer` |

Run the **Crew Loop** on every Stage 6 ticket. Nearly all of them are HARD.

### Prompt 6.1: Migrations

> **RUN WITH** · CLI · **Opus 5.5** · effort **high** · **Plan** → **Manual** · agents: `test-runner`
>
> `claude --model opus --effort high --permission-mode plan`

```text
Read docs/handoff/stage-05.md. Implement docs/migrations as the real,
ordered migrations. Every migration must be reversible and tested up
and down against a real Postgres in CI. Add a CI check that fails if an
already-applied migration file is edited.
```

### Prompt 6.2: Authentication

> **RUN WITH** · same session · **Opus 5.5** · `/effort xhigh` · **Plan** → **Manual** · agents: `security-reviewer`, `test-runner`

```text
Implement authentication per the auth ADR and SR requirements: sign-up,
sign-in, sign-out, email verification, password reset, and OAuth
providers [list]. Use httpOnly/Secure/SameSite session cookies with
rotation on privilege change, lockout and rate limits, and constant-time
comparisons. Every route defaults to deny. Write integration tests for
every flow and failure mode, plus the auth threats in
docs/threat-model.md. Then have the security-reviewer agent review the
diff.
```

### Prompt 6.3: Domain core (TradeLog: the stats engine)

> **RUN WITH** · Desktop, new session, worktree ON · **Opus 5.5** · effort **xhigh** · **Plan** → **Manual** · agents: `test-runner`, `code-reviewer`

```text
ultrathink. Implement the rules in docs/domain-rules.md as pure functions
with no I/O, using exact arithmetic per the money ADR. First derive each
computed value by hand on a fixture covering every listed edge case, and
show me the derivations. Then write golden tests from those values, then
property-based tests for every invariant, then the implementation.
```

### Stage 6 gate

> **INSPECTION: pass before Stage 7**
>
> - Every migration runs up and down cleanly in CI
> - Auth tests cover every flow, every failure mode, and every auth threat
> - The domain core has 100% coverage of `docs/domain-rules.md`, and you checked the hand derivations
> - `code-reviewer` and `security-reviewer` report zero CRITICAL findings

---

## Stage 7: Framing. The walking skeleton

*Framing shows the house's real shape before anything is finished. Every route and endpoint exists and is wired end to end (UI → API → DB → back), with real data flowing through the simplest version of each.*

| Crew for this stage | |
|---|---|
| Surface | Desktop, parallel worktree sessions |
| Models | Opus 5.5 · medium (structure) · Sonnet 5.5 · medium (spec-complete endpoints) |
| Agents | `implementer`, `test-runner`, `code-reviewer` |

### Prompt 7.1: Skeleton plan

> **RUN WITH** · Desktop · **Opus 5.5** · effort **medium** · **Plan** · agents: `Explore`

```text
Read docs/handoff/stage-06.md. Plan the walking skeleton: every page
route in the scope, the app shell and layouts, every OpenAPI endpoint
with handlers that validate input and hit the real database, the
generated TS client, and one Playwright test per route that loads it and
completes its primary action. Map each piece to its ticket ID and mark
which tickets can run in parallel worktrees. Don't build yet.
```

Then run the **Crew Loop** per ticket.

### Stage 7 gate

> **INSPECTION: pass before Stage 8**
>
> - A user can complete every core-loop action end to end on staging, even if the UI is plain
> - The generated client is the only way the frontend talks to the API
> - The skeleton e2e suite is green in CI

---

## Stage 8: Roofing. Security controls from the threat model

*The roof keeps everything else from rotting. Here you implement every control from the Stage 4 threat model, before the systems go in.*

| Crew for this stage | |
|---|---|
| Surface | CLI |
| Models | Opus 5.5 · xhigh |
| Agents | `security-reviewer`, `test-runner` |

### Prompt 8.1: Security envelope

> **RUN WITH** · CLI · **Opus 5.5** · effort **xhigh** · **Plan** → **Manual** · agents: `security-reviewer`, `test-runner`
>
> `claude --model opus --effort xhigh --permission-mode plan`

```text
Read docs/handoff/stage-07.md and docs/threat-model.md. Implement every
control in the threat model across the whole app:
- authz: ownership checks on every resource, plus an IDOR test for every
  resource ID
- validation: the shared schema on every input, rejecting unknown fields
- rate limits per route class, with tests
- security headers (CSP without unsafe-inline, HSTS, frame-ancestors),
  plus CSRF protection matching the session ADR
- the typed error envelope everywhere: no stack traces or internals
  reach clients
- secrets: env-only, validated at startup, never logged
- audit logging for security-relevant events (SR requirements)
Update docs/threat-model.md so every threat links to its control and its
passing test. Then have the security-reviewer agent review everything.
```

> **NOTE**
> Security-heavy prompts can trip Anthropic's safety classifiers, and Claude Code then reroutes the request to another model. That is expected routing for those domains, not a problem with your account.

### Stage 8 gate

> **INSPECTION: pass before Stage 9**
>
> - Every endpoint has an authz test and an IDOR test
> - Every threat in the model links to a control and a passing test
> - `security-reviewer` reports zero CRITICAL or HIGH findings

---

# PART C: SYSTEMS

## Stage 9: Plumbing. Data flows and integrations

*Pipes carry things in and out. Here: imports, file uploads, background jobs, transactional email, and third-party integrations, all idempotent, retry-safe, and bounded.*

| Crew for this stage | |
|---|---|
| Surface | Desktop |
| Models | Opus 5.5 · xhigh (uploads, imports) · Sonnet 5.5 · medium (spec-complete pieces) |
| Agents | `implementer`, `test-runner`, `debugger`, `security-reviewer` |

### Prompt 9.1: Data flows

> **RUN WITH** · Desktop · **Opus 5.5** · effort **medium** · **Plan** (switch to **xhigh** + **Manual** for upload tickets) · agents: `Explore`, `security-reviewer`

```text
Read docs/handoff/stage-08.md. Plan the data-flow tickets: [list, e.g.,
CSV import from N brokers, screenshot uploads, background
recalculation jobs, transactional email]. Every flow must be idempotent
and retry-safe, with bounded size and time, dead-lettering and alerting
on failure, and user-visible progress and error states. Uploads use
presigned URLs with type, size, and content-sniffing checks. Tests must
cover malformed input, duplicate delivery, partial failure, and
timeouts, plus fuzz tests for every parser.
```

Then run the **Crew Loop** per ticket.

### Stage 9 gate

> **INSPECTION: pass before Stage 10**
>
> - Re-running any job or import twice gives an identical result
> - Fuzz, malformed-file, and oversize tests pass; nothing crashes a worker
> - Failed jobs alert and can be recovered

---

## Stage 10: Electrical. Payments and entitlements

*Electrical powers the house, and miswiring it is dangerous. Payments power the business. Every wire gets tested.*

| Crew for this stage | |
|---|---|
| Surface | CLI |
| Models | Opus 5.5 · xhigh |
| Agents | `code-reviewer`, `security-reviewer`, `test-runner` |

### Prompt 10.1: Billing

> **RUN WITH** · CLI · **Opus 5.5** · effort **xhigh** · **Plan** → **Manual** (review every diff) · agents: `security-reviewer`, `test-runner`, `code-reviewer`
>
> `claude --model opus --effort xhigh --permission-mode plan`

```text
ultrathink. Read docs/handoff/stage-09.md and docs/business-model.md.
Implement billing with the chosen provider:
- checkout for each tier, monthly and annual
- webhooks: signature-verified, idempotent (store processed event IDs),
  safe out of order, and the single source of truth for subscription state
- entitlements as data, checked server-side on every gated action
- upgrade, downgrade, proration, cancel, reactivate, failed-payment
  dunning, refunds, and disputes
- customer billing portal access
- a daily reconciliation job comparing local state to the provider's,
  alerting on any mismatch
Use test mode and test keys only. Verify every API detail against the
provider's current official docs. Write tests that replay webhook
sequences with duplicates, out-of-order events, and failures.
```

### Stage 10 gate

> **INSPECTION: pass before Stage 11**
>
> - The full lifecycle works in test mode: subscribe → upgrade → downgrade → fail → recover → cancel → refund
> - Webhook replay tests pass with duplicate and out-of-order events
> - No gated feature is reachable from a lower tier (server-side tests)
> - You personally reviewed every payments diff

---

## Stage 11: HVAC. Observability, SLOs, and error budgets

*HVAC keeps the house livable and warns you when something's off. Google SRE runs on SLOs and error budgets: you decide how reliable "reliable enough" is, measure it, and slow releases when the budget is spent.*

| Crew for this stage | |
|---|---|
| Surface | Desktop |
| Models | Sonnet 5.5 · medium |
| Agents | `test-runner`, `sre-reviewer` |

### Prompt 11.1: Observability

> **RUN WITH** · Desktop · **Sonnet 5.5** · effort **medium** · **Accept edits** · agents: `test-runner`, then `sre-reviewer`

```text
Read docs/handoff/stage-10.md and the observability ADR. Implement:
structured JSON logs with request IDs propagated web → api → db; RED
metrics per endpoint; distributed traces; and error reporting with
release tags. Add SLOs from the NFRs, with error-budget dashboards and
burn-rate alerts. Add product analytics events for activation,
retention, conversion, and churn. Never log PII or secrets; add a test
that fails if a known secret pattern appears in log output.
Then have the sre-reviewer agent check that every user-visible failure
mode produces an alert.
```

### Stage 11 gate

> **INSPECTION: pass before Stage 12**
>
> - A forced error on staging appears in error reporting within minutes, with its trace
> - SLO and error-budget dashboards exist, and alerts fire on a simulated breach
> - The analytics funnel records a test user's journey end to end

---

# PART D: FINISHES

## Stage 12: Insulation & Drywall. Complete every feature

*The walls close in and the rooms become real. Every remaining ticket gets built to its full acceptance criteria, in every state.*

| Crew for this stage | |
|---|---|
| Surface | Desktop, parallel worktree sessions |
| Models | By ticket class (Crew Loop table) |
| Agents | All core agents |

### Prompt 12.1: Milestone sweep

> **RUN WITH** · Desktop · **Opus 5.5** · effort **medium** · **Plan** · agents: `Explore`

```text
Read docs/handoff/stage-11.md. List every open ticket in milestone [M#]
from docs/tickets.md with its class, dependencies, and build order.
Mark which can run in parallel worktrees, and the model and effort each
needs per the Crew Loop table.
```

Then run the **Crew Loop** on every ticket. Most of your tokens are spent here, so routing discipline matters most.

### Stage 12 gate

> **INSPECTION: pass before Stage 13**
>
> - Every ticket's acceptance criteria are verified by named tests
> - Every screen has working empty, loading, error, and permission-denied states
> - The full suite is green in CI on main

---

## Stage 13: Siding & Exterior. Design system, polish, accessibility

*The exterior is what everyone sees first. Apply the design system consistently, polish every interaction, and make it work for every device and every user.*

| Crew for this stage | |
|---|---|
| Surface | Desktop with the Browser pane |
| Models | Opus 5.5 · medium (design system) · Sonnet 5.5 · medium (application) |
| Agents | `code-reviewer` |

### Prompt 13.1: Apply the design system

> **RUN WITH** · Desktop · **Opus 5.5** · effort **medium** · **Plan** → **Accept edits** · agents: `code-reviewer`

```text
Read docs/handoff/stage-12.md and docs/design-tokens.md. Implement the
tokens as CSS custom properties and refactor every component to use
tokens only: no hard-coded colors, sizes, or spacing. Audit every route
in the Browser pane at 375, 768, 1280, and 1920 widths, in light and
dark mode. Fix layout breaks, focus states, and motion (respect
prefers-reduced-motion). Add a lint rule that fails on hard-coded colors.
```

### Prompt 13.2: Accessibility

> **RUN WITH** · Desktop · **Sonnet 5.5** · effort **medium** · **Accept edits** · agents: `test-runner`

```text
Audit every route for WCAG 2.2 AA in the Browser pane: keyboard-only
completion of every flow, focus order and visibility, labels, contrast,
target sizes, and screen-reader names for charts and tables. Fix every
issue, and add automated axe checks to the e2e suite so regressions
fail CI.
```

### Stage 13 gate

> **INSPECTION: pass before Stage 14**
>
> - Zero hard-coded design values (enforced by lint)
> - axe reports zero violations on every route; keyboard-only completion works
> - The UI matches the approved Stage 4.5 designs

---

## Stage 14: Inspections. Independent audits

*The city inspector didn't build the house, and that's why they catch what the builder missed. Every audit runs in a **fresh session** with an agent that has never seen the build. This is Microsoft SDL's verification phase (static analysis, fuzzing, penetration testing, review by someone other than the author), plus Google's production-reliability lens.*

| Crew for this stage | |
|---|---|
| Surface | CLI, a fresh session per audit, all in Plan mode (report only) |
| Models | Opus 5.5 · high · Fable 5.1 for the architecture audit |
| Agents | `security-reviewer`, `sre-reviewer`, `design-reviewer` |

### Prompt 14.1: Final security review and penetration test

> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** · agents: `security-reviewer`
>
> `claude --model opus --effort high --permission-mode plan`

```text
Read docs/handoff/stage-13.md, docs/threat-model.md, and
docs/requirements.md. Use the security-reviewer agent as an external
auditor who has never seen this code. Audit against OWASP ASVS level
[N]: authn, sessions, authz (attempt IDOR on every resource), input
validation, uploads, payments and webhooks, rate limits, headers,
secrets, dependencies, and every SR requirement. Run the static
analysis, secret scan, and dependency scan fresh, and review the fuzz
test results. Report each finding with severity, evidence, and the fix.
Do not fix anything.
```

### Prompt 14.2: Load, capacity, and failure modes

> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** → **Accept edits** (writes load tests only) · agents: `sre-reviewer`, `test-runner`

```text
Write and run load tests modeling 10x the expected users: a realistic
read/write mix, peak burst, overload past capacity, and a soak test.
Run EXPLAIN ANALYZE on every hot query. Measure Core Web Vitals on a
production build. Then have the sre-reviewer agent assess failure
modes: what happens when Postgres, object storage, email, or the payment
provider is slow or down, and whether clients retry safely. Report every
miss against the NFRs, with the fix and expected gain.
```

### Prompt 14.3: Architecture audit

> **RUN WITH** · CLI, fresh session · **Fable 5.1** · effort **high** (or **Opus 5.5** · **xhigh**) · **Plan** · agents: `design-reviewer`
>
> `claude --model fable --effort high --permission-mode plan`

```text
Use the design-reviewer agent as an independent principal architect
reviewing this system for a 20-year life. Compare the code with
docs/design-doc.md and every ADR. Find drift, hidden coupling, single
points of failure, data that can't be exported or migrated, dependencies
with short support horizons, and anything painful to change in 5 years.
Prioritize by risk. Report only.
```

Fix every CRITICAL and HIGH finding with the **Crew Loop**, then re-run the audit that found it.

### Stage 14 gate

> **INSPECTION: pass before Stage 15**
>
> - Zero open CRITICAL or HIGH findings across all three audits
> - Load tests meet the NFRs at 10x expected traffic, and overload degrades gracefully
> - Every drift is either fixed or recorded in a superseding ADR

---

## Stage 15: Paint & Trim. Copy, onboarding, growth surfaces

*Paint and trim turn a structure into a home people want: product copy, onboarding, emails, the marketing site, SEO, and legal pages.*

| Crew for this stage | |
|---|---|
| Surface | Claude chat for copy · Desktop for implementation |
| Models | Opus 5.5 (copy) · Sonnet 5.5 · medium (implementation) |
| Agents | `code-reviewer` |

### Prompt 15.1: Copy and onboarding

> **RUN WITH** · Claude chat, `[APP_NAME]` Project · **Opus 5.5** · effort adaptive

```text
Using the Master Spec, the press release, and the validation data,
write: the marketing site (hero, benefits, social-proof slots, pricing,
FAQ), onboarding (a first-run checklist that drives users to the wow
moment), every empty-state and error-message string, and the lifecycle
emails (welcome, activation nudge, trial ending, payment failed,
cancellation win-back). Voice: [describe the voice]. Every claim must
be true of the product as built.
Output it all as one structured COPY PACKET: grouped by page or surface,
each string with a stable key, ready to drop into the code.
```

### Prompt 15.2: Implementation

> **RUN WITH** · Desktop · **Sonnet 5.5** · effort **medium** · **Accept edits** · agents: `code-reviewer`

```text
Read docs/handoff/stage-14.md and docs/copy.md (the Copy Packet).
Implement the copy, the onboarding flow, and the emails. Add technical
SEO (metadata, Open Graph, sitemap, robots, structured data, canonical
URLs) and the legal pages from the LC requirements, marked for lawyer
review where flagged. Verify every page in the Browser pane.
```

### Stage 15 gate

> **INSPECTION: pass before Stage 16**
>
> - Someone outside your team reaches the wow moment without help
> - Every lifecycle email sends correctly in staging
> - Legal pages are live; lawyer-review items are reviewed or scheduled

---

# PART E: OCCUPANCY

## Stage 16: Final Walkthrough. Readiness review, drills, dogfood, beta

*Before the keys change hands, you walk every room, flip every switch, and test the smoke alarms. Google runs a Production Readiness Review and a launch checklist before launch: architecture and dependencies, capacity, failure modes, client behavior, automation, rollout planning, and external dependencies. Microsoft and Google both **dogfood**: the people who built it use it for real before customers do.*

| Crew for this stage | |
|---|---|
| Surface | CLI |
| Models | Opus 5.5 · high |
| Agents | `sre-reviewer`, `security-reviewer`, `test-runner` |

### Prompt 16.1: Production Readiness Review and launch review

> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** · agents: `sre-reviewer`, `security-reviewer`
>
> `claude --model opus --effort high --permission-mode plan`

```text
Read docs/handoff/stage-15.md. Run a Production Readiness Review for
[APP_NAME] with the sre-reviewer agent, covering Google's launch
checklist categories: architecture and dependencies, integration (DNS,
TLS, load balancing, monitoring), capacity planning, failure modes and
degraded modes, client behavior (retries, thundering herds), processes
and automation (no manual steps), development process (versioned code
and config, release process), external dependencies (vendor outages),
and rollout planning.
Add a launch review: security sign-off (security-reviewer), privacy (PR
requirements met, data export and deletion work), legal (LC items),
and accessibility. Also check: automated backups, a tested restore,
migration rollback, one-command app rollback, alert routing, an
incident response plan, a status page, and a support channel.
Mark every item DONE or MISSING, with evidence.
```

### Prompt 16.2: Drills

> **RUN WITH** · same session · **Opus 5.5** · effort **high** · **Manual**

```text
Write and walk me through three drills on staging, then record times
and results in docs/drills.md:
1. Restore the database from last night's backup into a fresh instance
   and verify the data
2. Deploy a deliberately broken release and roll it back within the
   runbook's time target
3. Kill the API under load and confirm recovery and alerting
```

### Dogfood, then beta

- **Ring 0: Dogfood (1 week).** You (and your team, if any) use the production app for real work. Every bug gets fixed with the Crew Loop (BUG class).
- **Ring 1: Beta (1–2 weeks).** 10–50 real users from your audience, on production, with live payments at a beta price.

### Stage 16 gate

> **INSPECTION: pass before Stage 17**
>
> - Every readiness and launch-review item is DONE
> - All three drills passed within their time targets
> - Dogfood complete; beta: at least one real payment and one real refund processed, SLOs held 7 consecutive days, zero open CRITICAL bugs

---

## Stage 17: Certificate of Occupancy. Ring-based launch and revenue

*The CO means the house is safe, complete, and legally occupied. The product is live, battle-tested, and earning. Like Microsoft's safe deployment practice and Google's staged rollouts and canaries, you widen exposure ring by ring, with bake time and health checks between rings.*

| Crew for this stage | |
|---|---|
| Surface | CLI |
| Models | Sonnet 5.5 · medium · Opus 5.5 · high for the health reviews |
| Agents | `sre-reviewer`, `test-runner` |

### Prompt 17.1: Production rollout plan

> **RUN WITH** · CLI · **Sonnet 5.5** · effort **medium** · **Manual**
>
> `claude --model sonnet --effort medium --permission-mode default`

```text
Read docs/handoff/stage-16.md. Prepare the production rollout:
- the production deploy workflow mirrors staging, with a manual
  approval gate
- live payment keys come from the secret store; production webhooks
  are registered
- feature flags gate every v1 feature
- a ring plan: Ring 1 (beta users, already live) → Ring 2 (10% of new
  signups) → Ring 3 (50%) → Ring 4 (everyone). Each ring bakes 24 hours
  minimum, with explicit health criteria to advance and an automatic
  rollback trigger
Write docs/launch-plan.md. Do not trigger any deploy; I will.
```

### Prompt 17.2: Ring health review (run before advancing each ring)

> **RUN WITH** · CLI, fresh session · **Opus 5.5** · effort **high** · **Plan** · agents: `sre-reviewer`
>
> `claude --model opus --effort high --permission-mode plan`

```text
Read docs/launch-plan.md. Review production health for the current ring:
SLOs and error-budget burn, error-rate trends, top errors with traces,
payment success rate, the signup → activation → paid funnel, and
anything anomalous. Recommend: ADVANCE, HOLD, or ROLL BACK, with evidence.
```

> **CERTIFICATE OF OCCUPANCY: all must be true**
>
> - Ring 4 is live: everyone is on the production release, deployed by the pipeline, with rollback proven
> - Live payments processing, reconciliation clean, real paying customers
> - SLOs held for 72 hours at Ring 4, with error budget remaining
> - Zero open CRITICAL/HIGH findings or bugs
> - Backups, restore, alerts, the incident plan, and the support channel all proven
> - Every ADR matches the code; the final handoff packet is signed
>
> **Not taking payments yet?** Then CO means every item above is verified in staging with test-mode payments, and the only remaining action is approving the Ring 2 deploy.

Write the final packet with Gate prompt B as `docs/handoff/stage-17.md`, run Gate prompt C, and sign it. That's your certificate. Then run the blueprint retro (Appendix F.5).

---

# PART F: OWNERSHIP

## Stage 18: Homeowner Maintenance. Operate and grow

*Houses that last get maintained on a schedule, not just when something breaks. Google writes blameless postmortems for every significant incident. Changes to the design go through change control.*

| Task | Run with | Mode · Agents |
|---|---|---|
| Known, reproducible bug | Desktop · Opus 5.5 · high | Plan → Accept edits · `debugger` |
| Intermittent or unclear production issue | CLI · **Fable 5.1** · high | Auto · `debugger`, `sre-reviewer` |
| Weekly dependency updates | CLI · Sonnet 5.5 · medium | Accept edits · `test-runner`, `security-reviewer` |
| Monthly ADR hygiene | CLI · Opus 5.5 · medium | Plan · `design-reviewer` |
| Quarterly restore drill | CLI · Opus 5.5 · high | Manual · `sre-reviewer` |
| New feature | Chat · Opus 5.5 → Change Request → Stage 4.1 / 4.2 / 4.6 deltas → Crew Loop | n/a |
| Blueprint retro (after CO) | CLI · Fable 5.1 · high | Plan → Accept edits · `design-reviewer` (Appendix F.5) |

### Prompt 18.1: Production incident

> **RUN WITH** · CLI · **Fable 5.1** · effort **high** · **Auto** · agents: `debugger`, `sre-reviewer`
>
> `claude --model fable --effort high --permission-mode auto`

```text
Production issue: [symptom, frequency, timeframe, trace IDs]. Find the
root cause. Investigate before changing anything. Reproduce it in a
failing test, fix it, and prove the fix in staging. Report the root
cause, the evidence chain, and the fix, then add the alert that would
have caught it earlier.
```

### Prompt 18.2: Blameless postmortem

> **RUN WITH** · same session · **Opus 5.5** · effort **high** · **Accept edits**

```text
Write a blameless postmortem in docs/postmortems/<date>-<slug>.md:
summary, user impact (duration, users, revenue), timeline, root cause
and contributing factors (systems and processes, never people), what
went well, what went badly, where we got lucky, and action items, each
with an owner, a ticket, and a priority. Required whenever there was
user-visible downtime beyond the SLO threshold, data loss, a rollback,
or an incident found by a human instead of monitoring.
```

### Prompt 18.3: Change Request (every new feature or spec change)

> **RUN WITH** · Claude chat, `[APP_NAME]` Project · **Opus 5.5** · effort adaptive

```text
Change Request CR-[NNN]: [what I want to change and why].
Assess it against the Master Spec: impact on scope, the feature catalog,
ADRs, the threat model, data model, pricing and entitlements, timeline,
and cost. Recommend APPROVE, MODIFY, or REJECT, with reasoning.
If approved, output a CR PACKET listing the exact deltas for Stage 4
(scope, ADRs, threats, data, tickets) in the same formats as the
original documents.
```

### Prompt 18.4: Monthly ADR hygiene

> **RUN WITH** · CLI · **Opus 5.5** · effort **medium** · **Plan** · agents: `design-reviewer`
>
> `claude --model opus --effort medium --permission-mode plan`

```text
Compare every Accepted ADR against the current code. List drift, any ADR
whose "Revisit when" trigger has fired, and dependencies approaching end
of support. Propose superseding ADRs where needed.
```

> **RULE**
> Claude Code never invents scope. Every new feature goes through a Change Request and the Construction Drawings first.

---

# APPENDICES

## Appendix A: One-time setup (do once, reused forever)

Run `claude update` first. Haiku 5.5 needs Claude Code v2.1.293 or later.

Every file in this appendix also ships as a real file in the **`claude-blueprint/` starter kit**. Run `./install.sh` to install them into `~/.claude` (it backs up anything it would replace), `./new-spec.sh <app>` to create a Stage 2 council folder, and `./new-repo-docs.sh <repo>` to add `docs/handoff/`, `docs/lessons.md`, and the templates to a project.

### A.1 `~/.claude/settings.json`

```json
{
  "model": "opus",
  "fallbackModel": ["sonnet", "haiku"],
  "permissions": {
    "defaultMode": "plan",
    "allow": [
      "Bash(pnpm install)",
      "Bash(pnpm run *)",
      "Bash(pnpm exec *)",
      "Bash(cargo build *)",
      "Bash(cargo test *)",
      "Bash(cargo clippy *)",
      "Bash(cargo fmt *)",
      "Bash(git status)",
      "Bash(git diff *)",
      "Bash(git log *)"
    ],
    "deny": [
      "Bash(git push --force *)",
      "Read(./.env)",
      "Read(./.env.*)"
    ]
  }
}
```

- `defaultMode: plan` makes every new session start safely in Plan mode.
- `fallbackModel` only kicks in when the primary model is overloaded or unavailable. Rate limits and billing errors never trigger it.
- **Do not** set `CLAUDE_CODE_EFFORT_LEVEL` globally. It overrides every agent's own effort setting.

### A.2 Global agents in `~/.claude/agents/`

**`explore.md`**: overrides the built-in Explore, which otherwise runs on your expensive main model.

```markdown
---
name: Explore
description: Fast read-only codebase search and file discovery.
tools: Read, Grep, Glob
model: haiku
effort: low
---
You locate code. Return file paths, symbol names, and one-line summaries
of what each relevant file does. Never paste whole files. Never propose
changes.
```

**`test-runner.md`**

```markdown
---
name: test-runner
description: Runs tests, type checks, linters, and scanners, and reports failures only. Use proactively after code changes.
tools: Read, Grep, Glob, Bash
model: haiku
effort: low
---
Run the verification commands listed in CLAUDE.md. Report ONLY failing
test names, file:line, the error, and the command that reproduces it.
If everything passes, reply "ALL GREEN" with the commands you ran.
Never modify code.
```

**`implementer.md`**

```markdown
---
name: implementer
description: Implements one fully specified ticket. Use for SPEC-COMPLETE work.
model: sonnet
effort: medium
isolation: worktree
---
Implement exactly the ticket given, nothing more. Follow CLAUDE.md and
the ADRs it references. Write the acceptance-criteria tests first, then
the code. Finish only when typecheck, lint, and tests pass. Report the
changed files and how each criterion was verified.
```

**`code-reviewer.md`**: modeled on Google's code review standard.

```markdown
---
name: code-reviewer
description: Principal-level reviewer. Use after any feature or fix, before commit.
tools: Read, Grep, Glob, Bash
model: opus
effort: high
memory: user
---
Run git diff and review only the changed code against CLAUDE.md and
docs/adr/. Approve when the change clearly improves overall code
health, even if it isn't perfect; never approve a change that makes it
worse. Check design, functionality, complexity, tests, naming,
comments, style, docs, security (validation, authz, secrets),
performance, accessibility, and ADR conformance. Facts and data
outweigh opinion. Output by priority: CRITICAL, WARNING, SUGGESTION,
and "Nit:" for optional polish, each with file:line and the concrete
fix. Check your memory for patterns seen before; save new recurring
issues when done.
```

**`debugger.md`**

```markdown
---
name: debugger
description: Root-cause specialist for failing tests, runtime errors, and unexpected behavior.
tools: Read, Edit, Bash, Grep, Glob
model: opus
effort: high
---
Reproduce first. Isolate the failure. Test hypotheses with evidence.
Fix the root cause, never the symptom. Add a regression test that fails
before the fix and passes after. Report the root cause, evidence, fix,
and regression test.
```

**`design-reviewer.md`**

```markdown
---
name: design-reviewer
description: Independent senior design reviewer. Use for design and architecture audits by someone who didn't write the design.
tools: Read, Grep, Glob, WebSearch, WebFetch
model: opus
effort: high
---
Review designs and code as an independent principal architect. Check
requirements against design, alternatives considered, cross-cutting
concerns (security, privacy, observability, cost), failure modes, data
portability, dependency support horizons, and long-term
maintainability. Rate findings BLOCKER / MAJOR / MINOR, each with
evidence and a fix. Report only; never edit.
```

**`security-reviewer.md`**: modeled on Microsoft SDL verification.

```markdown
---
name: security-reviewer
description: Security verification specialist. Use for threat-model checks, security reviews of diffs, and audits.
tools: Read, Grep, Glob, Bash
model: opus
effort: high
---
You verify security like an external auditor: you are not the author.
Check changes and systems against docs/threat-model.md and
docs/requirements.md (SR and PR items). Review authn, sessions, authz
and IDOR, validation, injection, uploads, secrets, crypto use, headers,
payments and webhooks, logging of sensitive data, and dependencies. Run
the project's static analysis, secret, and dependency scanners. Rate
findings CRITICAL / HIGH / MEDIUM / LOW, each with evidence and a fix.
Report only; never edit.
```

**`sre-reviewer.md`**: modeled on Google SRE production readiness reviews.

```markdown
---
name: sre-reviewer
description: Production readiness reviewer. Use for readiness reviews, launch checks, failure-mode analysis, and ring health reviews.
tools: Read, Grep, Glob, Bash, WebFetch
model: opus
effort: high
---
Review production readiness: architecture and dependencies, monitoring
and alerting on user-visible failures, SLOs and error budgets,
capacity, failure and degraded modes, client retry behavior, change
management and rollback, automation of manual steps, backups and
restore, and incident response. Mark each item DONE or MISSING with
evidence. Report only; never edit.
```

### A.3 Global `~/.claude/CLAUDE.md` (personal standards, ~40 lines max)

```markdown
# Global engineering standards
- TypeScript strict. Zero errors, zero warnings. No `any`, no `@ts-ignore`.
- pnpm only.
- Rust: clippy -D warnings clean; no unwrap() outside tests.
- Production-ready only: no TODOs, placeholders, or stubs.
- Read the latest docs/handoff/ packet, docs/lessons.md, and the
  relevant ADR before planning. If a change conflicts with an ADR, stop
  and say so. Never repeat a mistake already recorded in lessons.md.
- Every domain rule gets a test; every bug fix gets a regression test.
- Delegate: search → Explore, verification → test-runner,
  review → code-reviewer, security → security-reviewer.
- Terse responses. No recaps unless asked.
```

### A.4 claude.ai Projects

One Project per app. Every Stage 0–4 output and every Handoff Packet goes into Project knowledge, so each new chat starts with full context.

---

## Appendix B: Rules for maximum usage and efficiency

1. **One ticket, one fresh session.** Long sessions resend a growing context on every turn. Archive sessions when their PR merges.
2. **Packets, not history.** A new session reading a one-page handoff packet is far cheaper than a long session dragging its history along.
3. **Switch models only at task boundaries.** A model switch costs you the prompt cache. Change `/effort` before you change `/model`.
4. **Plan mode is the cheapest insurance there is.** An approved plan prevents expensive wrong turns.
5. **Pin every agent's model in frontmatter.** A search agent silently inheriting Opus or Fable is the most common hidden burn. Check `/tasks` to see what each agent runs on.
6. **Keep CLAUDE.md under 150 lines.** Point to docs; never paste them in.
7. **Use `/btw` for side questions** (Cmd+; in Desktop). The answer stays out of your history.
8. **Run `/compact` at milestones** in long sessions.
9. **Fable is a scalpel.** On Max it can use at most 50% of your weekly limit and burns it faster; on Pro every token is usage credits. Use it for the council chair, independent audits, and stuck or long-horizon work.
10. **Never default to max effort.** Try xhigh first; max can overthink.
11. **Watch usage:** `/usage` in the CLI, the usage ring in Desktop.

---

## Appendix C: How this maps to Apple, Google, Microsoft, and Amazon

These are publicly documented practices. Apple's internal software process isn't publicly documented beyond the DRI principle, so nothing more is claimed for it.

| Practice | Company (public source) | Where it lives in this blueprint |
|---|---|---|
| **DRI**: one named person accountable for every outcome | Apple | You sign every Handoff Packet |
| **Working Backwards**: write the launch press release and FAQ before building | Amazon | Stage 1.4, App Brief |
| **Design doc**: context, goals and non-goals, design, alternatives considered, cross-cutting concerns; reviewed before implementation | Google | Stage 4.2 design doc, Stage 4.7 independent review |
| **SDL requirements**: security and privacy requirements defined first | Microsoft | Stage 3.2 |
| **SDL design**: threat modeling with data flow diagrams | Microsoft | Stage 4.3; controls in Stage 8 |
| **SDL verification**: review by someone other than the author, static analysis, secret scanning, fuzzing, penetration testing, component governance | Microsoft | Stage 5.2 CI gates; Stage 9 fuzzing; Stage 14 audits |
| **Code review**: approve changes that improve code health; "Nit:" for optional comments | Google | `code-reviewer` agent; Crew Loop step 3 |
| **SLOs and error budgets** | Google SRE | Stage 11 |
| **Production Readiness Review + launch checklist** | Google SRE | Stage 16.1 |
| **Dogfooding** | Google, Microsoft | Stage 16, Ring 0 |
| **Safe deployment**: rings, progressive exposure, bake time, feature flags | Microsoft | Stage 17 ring plan |
| **Staged rollouts and canaries with automatic rollback** | Google SRE | Stage 17 |
| **Blameless postmortems** with defined triggers | Google SRE | Stage 18.2 |
| **Change control** after design freeze | Industry standard | Stage 2.6 freeze; Stage 18.3 Change Requests |
| **Continuous improvement of the process itself** | Google SRE (PRR continuous-improvement phase); Microsoft (continuous SDL, requirements revised yearly) | Appendix F calibration loop |

---

## Appendix D: Built for 20 years. Longevity principles

No stack survives 20 years unchanged, and anyone who promises that is guessing. What lasts is a system that **can be changed safely**. Build for replaceability, not permanence:

| Principle | How this blueprint enforces it |
|---|---|
| **Decisions are written down** | Design doc + ADRs, each with an **Exit path** |
| **Boring, widely adopted technology** | Stage 2.4 judges support horizon and adoption on evidence |
| **Contracts at every boundary** | OpenAPI-defined API, generated client, shared validation schema |
| **Data outlives code** | Reversible migrations, a data-export ADR, no vendor-locked formats |
| **Business logic is pure and tested** | Domain rules as pure functions with golden and property tests |
| **Change is cheap to verify** | Full CI gates, e2e suite, drills: upgrade anything and know within minutes if it broke |
| **Money is exact** | No floats for money; daily reconciliation |
| **Drift is caught on a schedule** | Monthly ADR hygiene, quarterly restore drills, weekly dependency updates |
| **Knowledge survives sessions** | Signed handoff packets in `docs/handoff/` record every stage |

---

## Appendix E: New-project quickstart checklist

- [ ] Fill in the Project Variables; create the claude.ai Project
- [ ] Stage 0: Land Survey (optional) → Opportunity Packet
- [ ] Stage 1: Meet the Architect → App Brief + press release, validation passed
- [ ] Stage 2: Design Studio → `MASTER-SPEC.md` FROZEN v1.0, stack chosen
- [ ] Stage 3: Permits → Business & Requirements Packet
- [ ] Stage 4: Construction Drawings → Build Package, independent review clean
- [ ] Stage 5: Site Prep → CI with security gates, staging, rollback tested
- [ ] Stage 6: Foundation → migrations, auth, domain core
- [ ] Stage 7: Framing → walking skeleton end to end
- [ ] Stage 8: Roofing → threat-model controls, all tested
- [ ] Stage 9: Plumbing → idempotent, fuzzed data flows
- [ ] Stage 10: Electrical → payments lifecycle tested
- [ ] Stage 11: HVAC → SLOs, error budgets, alerts
- [ ] Stage 12: Drywall → every feature complete
- [ ] Stage 13: Exterior → design system + accessibility
- [ ] Stage 14: Inspections → zero CRITICAL/HIGH findings
- [ ] Stage 15: Paint & Trim → copy, onboarding, SEO, legal
- [ ] Stage 16: Walkthrough → PRR, drills, dogfood, beta revenue
- [ ] **Stage 17: Certificate of Occupancy → live at Ring 4, battle-tested, earning**
- [ ] Stage 18: Maintenance schedule running
- [ ] Lessons captured at every gate (Gate prompt C, or the chat lessons prompt)
- [ ] Blueprint retro done; blueprint version bumped (Appendix F)

---

## Appendix F: The calibration loop. Make the blueprint better every project

No set of prompts is perfect on the first run. What makes this blueprint enterprise-grade over time is that **every miss becomes a permanent fix**. Google's readiness reviews end in a continuous-improvement phase, and Microsoft revises its SDL requirements every year. This loop does the same for your process.

### F.1 The lessons file: `docs/lessons.md`

Create it in every repo during Stage 4 with this header. Claude Code reads it before planning any work (it's in both CLAUDE.md files).

```markdown
# Lessons — [APP_NAME]

Format for every entry:

## L-NNN · Stage [N] · <date> · <short title>
WHAT HAPPENED: what went wrong or was wasted, with evidence
CAUGHT BY: me | code-reviewer | security-reviewer | sre-reviewer |
           design-reviewer | CI gate | inspection | production
SHOULD HAVE BEEN CAUGHT BY: the earliest point it could have been caught
ROOT CAUSE: prompt | agent | CLAUDE.md | spec gap | model choice |
            effort choice | missing gate | external change
COST: rework time, extra sessions, usage, or user impact
FIX (proposed): the exact edit — file, section, old text → new text
FIX (applied): blueprint vX.Y | project only | rejected (why)
STATUS: open | applied | rejected
```

### F.2 Lessons in the chat stages (0–4)

After each pre-construction packet is approved, in the same chat:

> **RUN WITH** · Claude chat, same conversation · **Opus 5.5** · effort adaptive

```text
Before we move on, review this conversation for lessons: questions you
should have asked earlier, answers I had to correct, output I had to
redo, and anything in your prompt instructions that was unclear or
missing. Write lesson entries in the L-NNN format (I'll paste the
format), each with the exact proposed edit to that stage's prompt in the
blueprint. If it went cleanly, say so in one line.
```

Paste the entries into `docs/lessons.md` once the repo exists (until then, keep them in Project knowledge).

### F.3 Lessons in the Claude Code stages (5–17)

Run **Gate prompt C** at the end of every stage. It's part of the Stage Gate, not optional.

### F.4 Mid-stage fixes: when the same mistake happens twice

> **RUN WITH** · CLI · **Opus 5.5** · effort **high** · **Plan** → **Accept edits**
>
> `claude --model opus --effort high --permission-mode plan`

```text
Lesson [L-NNN] has now happened twice. Read docs/lessons.md and find the
root cause. Propose the smallest permanent fix: a CLAUDE.md rule, an
agent instruction, a hook, a lint rule, or a CI check. Prefer automated
enforcement (hooks, lint, CI) over instructions, because instructions
can be ignored and checks can't. Implement it after I approve, and
mark the lesson "applied".
```

### F.5 The blueprint retro: after every Certificate of Occupancy

> **RUN WITH** · CLI, in your `claude-blueprint/` repo (where you keep this blueprint) · **Fable 5.1** · effort **high** (or **Opus 5.5** · **xhigh**) · **Plan** → **Accept edits** · agents: `design-reviewer`
>
> `claude --model fable --effort high --permission-mode plan`

```text
Here are the lessons from the project I just shipped:
[path to the project's docs/lessons.md, or paste it]
And the project metrics: [paste the metrics table from F.6]

Run a blueprint retro:
1. Group the lessons by root cause and by stage. Find the patterns.
2. For each pattern, propose a concrete edit to claude-build-blueprint.md:
   a prompt rewrite, a new inspection item, an agent change, a model or
   effort change, a new gate, or a removed step that added no value.
   Show old text → new text.
3. Have the design-reviewer agent challenge each proposed edit: does it
   fix the root cause, could it cause a new problem, and is it worth
   the added length?
4. Apply the surviving edits after I approve. Bump the version
   (MAJOR: stages or gates changed · MINOR: prompts or agents changed ·
   PATCH: wording) and add a CHANGELOG.md entry linking each change to
   its lesson IDs.
5. Re-render the PDF.
```

### F.6 Metrics to record per project

Fill this in during the retro. It tells you which stages are working and where usage goes.

| Metric | How to measure | What it tells you |
|---|---|---|
| Rework per stage | Lessons with "should have been caught" earlier than where they were | Which gates or prompts are weak |
| Escaped defects | Bugs found in beta or production | Whether inspections are rigorous enough |
| Findings per audit | CRITICAL/HIGH counts in Stage 14 | How much quality the build stages produce on their own |
| Usage per stage | `/usage` snapshots at each gate | Where to tighten model or effort routing |
| Fable spend | Share of weekly limit or credits used on Fable | Whether the scalpel is used where it pays |
| Time per stage | Dates on each signed packet | Where you're slowest, and why |
| Sessions per ticket | Desktop/CLI session count per ticket ID | Ticket sizing and spec quality |

### F.7 Version the blueprint itself

Keep the blueprint in its own git repo, `claude-blueprint/`, with `claude-build-blueprint.md`, the render script, `CHANGELOG.md`, and the reusable agent folders (global agents and the Stage 2 council). Every project records the blueprint version it used in `docs/handoff/stage-04.md`, so you can always tell which version of the process built which product.

> **RULE**
> A lesson isn't learned until it changes a prompt, an agent, a check, or a gate. Writing it down is step one; enforcing it is the point.

---

**Sources** (re-verify periodically; model lineups, prices, and plan terms change):

- Claude Code model configuration: https://code.claude.com/docs/en/model-config
- Claude Code permission modes: https://code.claude.com/docs/en/permission-modes
- Claude Code subagents: https://code.claude.com/docs/en/sub-agents
- Claude Code Desktop: https://code.claude.com/docs/en/desktop
- Claude Fable 5.1 overview and model comparison: https://platform.claude.com/docs/en/models/fable-5-1/overview
- Fable models on your plan: https://support.claude.com/en/articles/15424964-claude-fable-models-on-your-plan
- Google SRE, Reliable Product Launches at Scale: https://sre.google/sre-book/reliable-product-launches/
- Google SRE, The Evolving SRE Engagement Model (PRR): https://sre.google/sre-book/evolving-sre-engagement-model/
- Google SRE, Postmortem Culture: https://sre.google/sre-book/postmortem-culture/
- Google engineering practices, the standard of code review: https://google.github.io/eng-practices/review/reviewer/standard.html
- Design Docs at Google: https://www.industrialempathy.com/posts/design-docs-at-google/
- Microsoft Security Development Lifecycle: https://learn.microsoft.com/windows/security/threat-protection/msft-security-dev-lifecycle
- Microsoft, Evolving the SDL: https://www.microsoft.com/en-us/security/blog/2024/03/07/evolving-microsoft-security-development-lifecycle-sdl-how-continuous-sdl-can-help-you-build-more-secure-software/
- Microsoft safe deployment practices: https://learn.microsoft.com/devops/operate/safe-deployment-practices
- Apple's DRI practice: https://www.bitesizelearning.co.uk/resources/directly-responsible-individual-dri-apple
- Amazon Working Backwards PR/FAQ: https://www.koji.so/docs/working-backwards-pr-faq-guide
