# Foreman — Design Direction

Status: DRAFT v0.9, input to Stage 4.5 (design system and screens). Stage 4.5 turns this into tokens, components, and reviewable HTML mockups.

## 1. The idea: a drawing set, not a dashboard

Construction runs on a **drawing set**: a bound stack of numbered sheets, each with a **title block** in the corner (project, sheet number, revision, who approved it), all printed in cyanotype blue. Builders flip to the sheet they need, read the callouts, build, and get it stamped.

Foreman borrows that vernacular, because it *is* the subject: the 19 stages are the sheets, each step is a callout, each signed packet is a stamp. That gives Foreman an identity no generic tool has, and every one of these devices carries real information.

**Where the boldness goes:** one place only, the **sheet strip**. It's a cyanotype band across the top of a project, showing all 19 stages as numbered sheets with fine white linework, phase colors, and approval stamps. Everything else stays quiet, native, and disciplined, so the strip and the next action are what your eye finds.

## 2. Palette

Two themes: **Night** (default, dark) and **Vellum** (light). They follow the system setting unless you override it.

| Token | Night | Vellum | Use |
|---|---|---|---|
| `--cyanotype` | `#1E4E8C` | `#1E4E8C` | Sheet strip background; the primary action |
| `--linework` | `#E6EEF8` | `#E6EEF8` | Linework and text on cyanotype |
| `--ground` | `#191D22` | `#F2F4F6` | Window background |
| `--sheet` | `#20252B` | `#FFFFFF` | Content surface |
| `--rule` | `#313841` | `#D6DBE2` | Hairlines and dividers |
| `--ink` | `#E4E8ED` | `#1C232B` | Primary text |
| `--ink-muted` | `#9AA4B0` | `#5A6573` | Secondary text |
| `--pass` | `#3E9B63` | `#2E7D4F` | Inspection passed, signed |
| `--fail` | `#D9534A` | `#B8352D` | Inspection failed, invalid packet |
| `--attention` | `#F07A22` | `#C65F12` | Needs your decision or signature (used sparingly) |

Phase colors identify stages, muted so they don't compete with status:

| Phase | Token | Hex |
|---|---|---|
| Pre-Construction (0–4) | `--phase-pre` | `#4A7CC0` |
| Structure (5–8) | `--phase-structure` | `#B0663A` |
| Systems (9–11) | `--phase-systems` | `#3A938C` |
| Finishes (12–15) | `--phase-finish` | `#8064B0` |
| Occupancy (16–17) | `--phase-co` | `#4A9A5E` |
| Ownership (18) | `--phase-maint` | `#77808C` |

Rules:
- Status always uses shape plus color (a stamp, a check, a cross, a lock), never color alone.
- `--attention` appears only for something waiting on you: an unsigned valid packet, a decision, a failing gate you must look at.
- Every text and background pair meets WCAG 2.2 AA; verified with a contrast check in Stage 13.

## 3. Type

| Role | Face | Notes |
|---|---|---|
| Interface | SF Pro (`system-ui`, `-apple-system`) | Native macOS; Foreman should feel like a Mac app, not a website |
| Prompts, commands, packets | SF Mono (`ui-monospace`) | Only for real code and literal text you'll run or paste |
| Sheet numbers and title blocks | IBM Plex Sans Condensed, SemiBold (bundled locally, OFL) | The drafting-lettering voice, used only in the sheet strip and title blocks |

Scale (pt): 11 · 12 · 13 (body) · 15 · 18 · 22 · 28. Body line height 1.45. Prompt text in mono at 12.5 pt with line height 1.5 and a measure of at most 88 characters. Sentence case everywhere; no all-caps labels.

## 4. Layout

```text
┌────────────┬──────────────────────────────────────────────────────┬───────────────┐
│ Projects   │  Sheet strip: 0 1 2 3 4 │ 5 6 7 8 │ 9 10 11 │ …  18     │  Inspector    │
│  TradeLog  │══════════════════════════════════════════════════════│               │
│  Foreman   │  Stage 6 · Foundation              ┌ title block ──┐ │  Packet       │
│  Swings    │  Database, auth, domain core       │ sheet 6 of 19 │ │  Gate         │
│            │                                    │ rev  bp 2.1   │ │  Lessons      │
│ Stages     │                                    │ DRI  Billy    │ │               │
│  ✓ 0 … 5   │                                    │ in progress   │ │  (contextual  │
│  ● 6       │                                    └───────────────┘ │   to the      │
│  ○ 7 … 18  │  6.1  Migrations          CLI · Opus 5.5 · high · Plan│   selected    │
│            │       ───────────────────────────────────────────────│   step or     │
│            │       resolved prompt (mono)…                        │   gate)       │
│            │       [Run in terminal ⌘↵] [Copy prompt]             │               │
│            │  +                                                   │               │
│            │  6.2  Authentication   n/a · single-user app         │               │
│            │  +                                                   │               │
├────────────┴──────────────────────────────────────────────────────┴───────────────┤
│  Terminal drawer: tabs per run (fm-tradelog-s06-6.1 …)                           ⌃` │
└────────────────────────────────────────────────────────────────────────────────────┘
```

- Left-aligned throughout. The sidebar uses macOS sidebar vibrancy; the title bar is transparent with the traffic lights inset.
- Steps are **not cards**. They're callouts on a sheet: a step number in the left margin (a real sequence), a one-line spec line (surface · model · effort · mode · agents), then the prompt. Hairline rules separate steps. Radius is 6 px on interactive controls only; the sheet itself has square edges, like paper.
- The inspector on the right is contextual: the selected step's packet, the stage's gate, or lessons. It collapses with ⌘⌥I.
- The terminal drawer slides up from the bottom, one tab per run, resizable, and remembers its height.

## 5. Screens

1. **Projects** (home): every project with its sheet strip in miniature, current stage, next step, and anything waiting on you. Empty state: "Start your first project" with the button that opens the wizard.
2. **New project wizard**, four short steps: name and folder → variables → the eight condition questions (each with one line on what it changes) → review the tailored path (which stages apply, which are not applicable, and why) → Create.
3. **Project**: sheet strip + stage workspace + inspector + terminal drawer (the layout above).
4. **Gate**: the stage's inspection items as a checklist with evidence, PASS/FAIL stamps, "Run inspection headless" and "Run in terminal", then Packet → Sign → Lessons. Sign is disabled until every applicable item passes, and the reason is stated.
5. **Tickets** (the Crew Loop): tickets grouped by milestone, each showing class, routed model and effort, loop step (Plan → Build → Review → Commit → Close), and its session. One action: "Run next loop step".
6. **Lessons**: list with filters (stage, root cause, status). Detail with "Promote to blueprint".
7. **Change requests**: list and detail, with the conditions and stages each CR changes.
8. **Run log**: searchable history of every run, with argv and session name; "Resume session" where possible.
9. **Doctor**: each environment check with a status, the version found, and how to fix any failure.
10. **Settings**.
11. **Command palette** (⌘K): projects, stages, steps, tickets, and every action.

## 6. Components

| Component | Notes |
|---|---|
| Sheet strip | 19 sheets grouped by phase. Each shows its number, phase color edge, and a status mark: blank (locked), outline (ready), filled dot (in progress), magnifier (inspecting), stamp (signed), hatched (not applicable). Hover shows the stage name; click opens it. |
| Title block | Bottom-right drafting box per stage: sheet number, revision (pinned blueprint version), DRI, status, date signed. |
| Spec line | The run config: surface icon, model, effort, mode, agents. Edited values (from the overlay) are underlined with a diff on hover. |
| Prompt block | Mono, wrapped, read-only by default. Resolved variables show subtly; unresolved ones show as attention pills you can click to fill. "Edit for this project" opens an overlay edit with a diff. |
| Step actions | One primary action that matches the surface: "Run in terminal", "Open in Desktop", or "Copy for chat". Secondary: Copy prompt, Copy command, Skip (only when allowed, with a reason field). |
| Insert point (+) | Between steps: Add prompt, Add inspection item, Add agent, Add note. Each asks "This project only" or "Propose for the blueprint". |
| Lock | On core items: "Core: protects production quality. Change it in the blueprint with a Change Request." |
| Not-applicable row | Hatched, one line: "Not applicable: this app doesn't take payments (payments = no)." Reactivates if a CR flips the condition. |
| Packet view | The packet's fields as a readable form, with validation problems inline. Sign shows your name and the date before confirming. |
| Inspection row | Item, status stamp, evidence, artifacts (paths and commands are copyable). |
| Terminal tab | Named after the session; shows a running, exited, or errored state. Closing a running tab asks first. |

## 7. Interaction and keyboard

| Action | Shortcut |
|---|---|
| Command palette | ⌘K |
| Run the selected step | ⌘↵ |
| Copy prompt · copy command | ⌘⇧C · ⌘⌥C |
| Next / previous step | J / K (or ↓ / ↑) |
| Next / previous stage | ⌘] / ⌘[ |
| Jump to a stage | ⌘G, then type the number |
| Toggle terminal drawer · inspector | ⌃` · ⌘⌥I |
| Open gate for this stage | ⌘⇧G |
| New project | ⌘N |
| Settings | ⌘, |

- Focus is always visible (2 px `--cyanotype` ring with a 2 px offset), and the full flow works without a mouse.
- Destructive or irreversible actions (closing a running terminal, signing, applying a CR) confirm, and say exactly what will happen.

## 8. Motion

One orchestrated moment: **the stamp**. When you sign a packet, the stage's sheet in the strip gets an approval stamp (a 220 ms scale-and-settle) and the next stage turns ready. Everything else moves only in answer to an action: the terminal drawer slides (160 ms), the inspector expands (140 ms), with ease-out. All motion turns off under `prefers-reduced-motion`.

## 9. Writing

- Name things by what they do: "Run in terminal", not "Execute". The same verb follows through: "Sign packet" → toast "Packet signed".
- Errors say what happened and how to fix it: "Claude Code 2.1.270 found. Opus 5.5 needs 2.1.280 or later. Run `claude update`, then check again."
- Empty states invite the next action: "No tickets yet. They come from Stage 4.6. Open Stage 4."
- Not-applicable and locked states always explain why, in one line.

## 10. Quality floor

WCAG 2.2 AA contrast in both themes, full keyboard operation, VoiceOver labels on every control and status, `prefers-reduced-motion` honored, and a layout that works from a 1280 × 800 window up to full screen. In narrower windows, the inspector collapses first, then the sidebar collapses to icons.
