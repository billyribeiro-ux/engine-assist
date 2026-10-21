# Mockup parity (M5-07, AC-XC-26, EX-M5-02)

Every screen in `docs/design/mockups/` was compared with the built app, in Night and Vellum, at 1280 × 800 and 1440 × 900, in Chromium. This is the record: what was compared, what differs and why, what was fixed, and what only the Mac can settle.

## How it is checked

`tests/flows/visual.spec.ts` (run with `pnpm test:flows --project=chromium`) loads each mockup file and the matching mock-world route (`tests/flows/screens.ts`), with the mockup legend hidden, and asserts for each screen, theme and size:

- the shell sits where the mockup's does, to 2 px: sidebar, title bar, main column, sheet strip and title block;
- every element both draw (body, h1, sidebar, title bar, main, strip, title block, primary button, table, banner, callout) has the same computed color, background, font family, size and weight, radius and border color and width;
- on Gate and Assembler, where the data is the same, the whole page is within 2 % mean pixel difference of the mockup.

The app's screenshots are written to `docs/design/screenshots/app/<screen>-<theme>-1280.png` by `FM_WRITE_SHOTS=1`; the mockups' are `docs/design/screenshots/<screen>-<theme>-<width>.png`. `FM_DIFF_TSV=<file>` records each page's whole-frame difference.

Other specs that hold the same line: `layout.spec.ts` (no sideways scroll at 1280 and 1440 on every screen), `a11y.spec.ts` (axe, both themes, every screen and the sign sheet and palette), `status.spec.ts` (no status by color alone), `keyboard.spec.ts` (Tab, rings, no trap), and `src/lib/styles/styles.lint.test.ts` (no hard-coded design values, contrast pairs).

## Result

Every screen passes in both themes at both sizes in Chromium: the shell geometry and the paint of everything both draw are identical. The whole-frame difference at 1280 (mean absolute difference, 0 is identical) shows how much the content differs; it is not asserted except for the two screens with the same data.

| Screen | Night | Vellum | What differs from the mockup, and why |
| ------ | ----- | ------ | ------------------------------------- |
| Gate (Stage 5) | 0.002 | 0.002 | Nothing visible. |
| Assembler | 0.003 | 0.003 | At 1440 the mockup reserves an empty 300 px inspector column; the app draws none (see "Variations"). |
| Template health | 0.007 | 0.008 | Mock data. |
| Workspace (CLI) | 0.014 | 0.015 | "Export run sheet…" under the subtitle (F-027); mock data (counts). At 1440 the title block sits 8 px lower because of that link. |
| Workspace (chat) | 0.011 | 0.013 | Mock data. |
| Doctor | 0.018 | 0.021 | Extra rows the spec added after the mockup: session sensor, probe folder, one-time setup, Run probes now. |
| First run | 0.019 | 0.023 | Mock data. |
| Settings | 0.019 | 0.022 | A settings field is 406 px wide where the mockup's is 468 px; mock data. |
| Today | 0.023 | 0.034 | The "Interrupted sessions" section (spec addition); mock data. |
| Gate, sign sheet | 0.024 | 0.033 | The sheet opens with focus in the Name field; the mockup draws the focus ring on Sign packet as an illustration. The Decisions row appears only with a pending overlay change. |
| Palette | 0.026 | 0.050 | The page behind the palette is the app's workspace, not the mockup's. |
| Setup | 0.029 | 0.034 | Mock data and error state: the mockup draws the form with errors filled in. |
| Outside change | 0.025 | 0.028 | Fewer context lines in the diff. |
| Run log | 0.031 | 0.034 | See "Variations". |
| Wizard | 0.033 | 0.059 | The mockup draws step 4 ("Review your path"); the app opens on step 1. Same components and paint. |
| Packet | 0.034 | 0.041 | The app's state includes the "Normalize" card; mock data. |

## Fixed in this pass

- **Sidebar status marks were colors alone.** The project rows' badges and the Doctor badge were `.dot` spans that differed only by color (attention, pass). They are now shapes plus color from `src/lib/nav/status.ts` (`PROJECT`: cross "Has a problem", clock "Waiting on you", check "No problems"; the Doctor badge uses `DOCTOR`). `status.test.ts` holds the tables to one look per state.
- **Run log overflowed sideways** at 1280 and 1440: the action cell is now a wrapping flex cell (`RunLog.svelte`).
- **Packet page scrolled sideways** at 1440: the visually hidden "(problem)" text had no positioned ancestor (`Packet.svelte`).
- **Phase labels were clipped** under the strip: the 9 px dimension line cut the descenders of "Pre-construction" and "Systems"; now only the horizontal overflow is clipped (`SheetStrip.svelte`).
- **An added step's callout read "x-2-1"**, wider than its circle. It now shows "+1" (the n of `x-<stage>-<n>`); the full number stays in the row's accessible name and title (`StepRow.svelte`).

## Conflicts, settled by the order of truth (CLAUDE.md)

1. **Sidebar dots.** The mockups draw `.dot.attention` and `.dot.pass`, which differ by color alone. AC-XC-25 (`docs/scope.md`, source 5: "shape plus color, never color alone") outranks a mockup, so the app uses glyphs (above). The mockup files were not edited.
2. **Added step number.** No source defines how an added step is numbered on screen. DR-059 defines its ID (`x-<stage>-<n>`); the mockup draws "x‑2‑1" in the callout; the Rust `display_number` returns the ID. A callout cannot hold that text in its circle, so the app shows "+n" and keeps the ID in the accessible name. **A judgment call for Billy**: if the mockup's wording should win, the callout needs a wider shape.
3. **Palette focus.** The mockup and `components.md` make the palette a combobox whose focus is shown on the active option (`aria-activedescendant`), and its input has `outline: none`. AC-XC-21 asks for a 2 px ring on everything focused. `keyboard.spec.ts` treats a combobox's field as the one exception (the active option is highlighted); if AC-XC-21 should apply literally, the palette field needs a ring, and the mockup changes with it.
4. **Run log metrics.** The mockup shows four numbers; AC-F026-09 (scope, source 5) asks for the two-row strip the app has. The app follows the AC.

## Variations (spec additions and state differences, not defects)

- The Project menu button and inspector toggle in the title bar, and "Export run sheet…" in the workspace head (F-027), were added after the mockups were drawn.
- The Run log row has two actions (Resume or Show terminal, and Show or Hide command); the mockup has one. The mockup's "Copy command" on an errored row is not built.
- The assembler's mockup reserves an empty inspector column at 1440 on every page. The app draws the inspector only where it has content (workspace, gate, packet) and gives the assembler the full width. The width of the content column is the same up to its cap.
- The wizard's mockup shows step 4; the setup's mockup shows an error-filled form. The app's screenshots show its default states. Both are covered in component tests (`Wizard.svelte.test.ts`, `Setup.svelte.test.ts`).
- Gate-signed (the toast after signing) is a state the mockup draws with `:target`; `keyboard.spec.ts` signs by keyboard and checks the toast, and the sign sheet is compared above. The signed-sheet stamp is covered in `SheetStrip.svelte.test.ts` and `display.spec.ts`.

## Only the Mac can settle

- **WebKit pixel parity** (Mac check 24): Chromium is the proxy here. WKWebView renders text and 1 px lines differently; run `pnpm test:flows --project=webkit` for `visual.spec.ts` and `layout.spec.ts`, then look at both themes of every screen side by side with the mockups in the real app. The geometry and paint assertions should hold; record any sub-pixel text difference.
- **Run log mockup cross-check** in both themes (Mac check 7, M4-03).
