#!/usr/bin/env python3
"""WCAG 2.2 contrast check for Foreman's design tokens.

Parses docs/design/tokens.css (stdlib only), resolves var() references per theme,
computes the contrast ratio for every pair declared in PAIRS, writes the table to
docs/design/contrast.md, and exits non-zero if any pair is below its minimum.

Usage:  python3 docs/design/contrast-check.py [--no-write]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
TOKENS = HERE / "tokens.css"
REPORT = HERE / "contrast.md"

TEXT = 4.5       # normal text
LARGE = 3.0      # large text (>= 18.66px bold or >= 24px)
UI = 3.0         # UI components, graphical objects, focus indicators

# (foreground token, background token, minimum, where it's used)
# Every pair is checked in both themes.
PAIRS: list[tuple[str, str, float, str]] = [
    # Body text
    ("--ink", "--ground", TEXT, "Primary text on the window"),
    ("--ink", "--sheet", TEXT, "Primary text on the content sheet"),
    ("--ink", "--sheet-raised", TEXT, "Selected step, dialogs, hover rows"),
    ("--ink", "--well", TEXT, "Prompt and code blocks"),
    ("--ink", "--sidebar", TEXT, "Sidebar items"),
    ("--ink", "--selected", TEXT, "Selected sidebar row"),
    ("--ink", "--hatch", TEXT, "Text over not-applicable hatching"),
    ("--ink", "--attention-wash", TEXT, "Body text in attention banners"),
    ("--ink", "--fail-wash", TEXT, "Body text in error banners"),
    ("--ink", "--pass-wash", TEXT, "Body text in pass rows"),
    # Secondary text
    ("--ink-muted", "--ground", TEXT, "Secondary text on the window"),
    ("--ink-muted", "--sheet", TEXT, "Spec lines, captions"),
    ("--ink-muted", "--sheet-raised", TEXT, "Captions in the selected step"),
    ("--ink-muted", "--well", TEXT, "Comments in prompt blocks"),
    ("--ink-muted", "--sidebar", TEXT, "Sidebar captions"),
    ("--ink-muted", "--selected", TEXT, "Caption in a selected sidebar row"),
    ("--ink-muted", "--hatch", TEXT, "Not-applicable reason over hatching"),
    # Accent text
    ("--accent-ink", "--sheet", TEXT, "Links, resolved variables"),
    ("--accent-ink", "--sheet-raised", TEXT, "Links in the selected step"),
    ("--accent-ink", "--well", TEXT, "Resolved variables in prompts"),
    ("--accent-ink", "--selected", TEXT, "Selected sidebar row label"),
    ("--accent-ink", "--ground", TEXT, "Links on the window"),
    # Status text and marks
    ("--pass", "--sheet", TEXT, "Pass / signed label"),
    ("--pass", "--sheet-raised", TEXT, "Pass label in selected row"),
    ("--pass", "--ground", TEXT, "Pass label on the window"),
    ("--pass", "--pass-wash", TEXT, "Pass label in pass row"),
    ("--pass", "--sidebar", TEXT, "Doctor dot label in sidebar"),
    ("--fail", "--sheet", TEXT, "Fail / invalid label"),
    ("--fail", "--sheet-raised", TEXT, "Fail label in selected row"),
    ("--fail", "--ground", TEXT, "Fail label on the window"),
    ("--fail", "--fail-wash", TEXT, "Fail label in error banner"),
    ("--fail", "--well", TEXT, "Validation problem inside a packet block"),
    ("--attention", "--sheet", TEXT, "Needs-you label"),
    ("--attention", "--sheet-raised", TEXT, "Needs-you label in selected row"),
    ("--attention", "--ground", TEXT, "Needs-you label on the window"),
    ("--attention", "--attention-wash", TEXT, "Unresolved-token pill, banners"),
    ("--attention", "--sidebar", TEXT, "Waiting badge in sidebar"),
    ("--attention", "--well", TEXT, "Unresolved token inside a prompt"),
    # Primary action
    ("--primary-ink", "--primary-bg", TEXT, "Primary button label"),
    ("--primary-ink", "--primary-bg-hover", TEXT, "Primary button label, hover"),
    ("--primary-disabled-ink", "--primary-disabled-bg", TEXT, "Disabled primary (exempt; kept legible)"),
    # Sheet strip
    ("--strip-line", "--strip-bg", TEXT, "Sheet numbers and marks on the strip"),
    ("--strip-muted", "--strip-bg", TEXT, "Locked sheet numbers, group labels"),
    ("--strip-current-ink", "--strip-current-bg", TEXT, "Current sheet number"),
    ("--strip-attention", "--strip-bg", TEXT, "Waiting-for-signature mark"),
    ("--strip-fail", "--strip-bg", TEXT, "Invalid-packet mark"),
    ("--focus-on-strip", "--strip-bg", UI, "Focus ring on the strip"),
    ("--phase-pre", "--strip-bg", UI, "Phase edge 0-4"),
    ("--phase-structure", "--strip-bg", UI, "Phase edge 5-8"),
    ("--phase-systems", "--strip-bg", UI, "Phase edge 9-11"),
    ("--phase-finish", "--strip-bg", UI, "Phase edge 12-15"),
    ("--phase-co", "--strip-bg", UI, "Phase edge 16-17"),
    ("--phase-maint", "--strip-bg", UI, "Phase edge 18"),
    # Phase on sheet surfaces
    ("--phase-pre-ink", "--sheet", UI, "Phase tick 0-4 on sheet"),
    ("--phase-structure-ink", "--sheet", UI, "Phase tick 5-8 on sheet"),
    ("--phase-systems-ink", "--sheet", UI, "Phase tick 9-11 on sheet"),
    ("--phase-finish-ink", "--sheet", UI, "Phase tick 12-15 on sheet"),
    ("--phase-co-ink", "--sheet", UI, "Phase tick 16-17 on sheet"),
    ("--phase-maint-ink", "--sheet", UI, "Phase tick 18 on sheet"),
    ("--phase-pre-ink", "--sidebar", UI, "Phase tick 0-4 in sidebar"),
    ("--phase-structure-ink", "--sidebar", UI, "Phase tick 5-8 in sidebar"),
    # Controls and focus
    ("--control-edge", "--sheet", UI, "Input, checkbox, toggle borders"),
    ("--control-edge", "--sheet-raised", UI, "Input borders in dialogs"),
    ("--control-edge", "--ground", UI, "Input borders on the window"),
    ("--control-edge", "--well", UI, "Input borders in wells"),
    ("--accent-ink", "--sheet", UI, "Checked toggle track, checkbox fill"),
    ("--sheet", "--accent-ink", UI, "Toggle knob and check mark on the fill"),
    ("--focus", "--ground", UI, "Focus ring on the window"),
    ("--focus", "--sheet", UI, "Focus ring on the sheet"),
    ("--focus", "--sheet-raised", UI, "Focus ring in selected rows, dialogs"),
    ("--focus", "--well", UI, "Focus ring in wells"),
    ("--focus", "--sidebar", UI, "Focus ring in the sidebar"),
    ("--focus", "--term-bg", UI, "Focus ring around the terminal"),
    # Terminal drawer
    ("--term-ink", "--term-bg", TEXT, "Terminal output"),
    ("--term-muted", "--term-bg", TEXT, "Terminal dim text, dividers"),
    ("--pass", "--term-bg", TEXT, "Success lines in the terminal"),
    ("--fail", "--term-bg", TEXT, "Error lines in the terminal"),
    ("--attention", "--term-bg", TEXT, "Now/Next mismatch, prompts waiting"),
    ("--accent-ink", "--term-bg", TEXT, "Paths in the terminal"),
]

HEX = re.compile(r"^#([0-9a-fA-F]{6})$")
DECL = re.compile(r"(--[\w-]+)\s*:\s*([^;]+);")
VAR = re.compile(r"^var\((--[\w-]+)\)$")


def strip_comments(css: str) -> str:
    return re.sub(r"/\*.*?\*/", "", css, flags=re.S)


def blocks(css: str) -> list[tuple[str, str]]:
    """Return (selector, body) for every block, flattening @media wrappers."""
    out: list[tuple[str, str]] = []
    i = 0
    while i < len(css):
        brace = css.find("{", i)
        if brace == -1:
            break
        selector = css[i:brace].strip()
        depth, j = 1, brace + 1
        while depth and j < len(css):
            depth += {"{": 1, "}": -1}.get(css[j], 0)
            j += 1
        body = css[brace + 1 : j - 1]
        if selector.startswith("@media"):
            for sel, inner in blocks(body):
                out.append((f"{selector} {sel}", inner))
        else:
            out.append((selector, body))
        i = j
    return out


def decls(body: str) -> dict[str, str]:
    return {k: v.strip() for k, v in DECL.findall(body)}


def load_themes() -> dict[str, dict[str, str]]:
    css = strip_comments(TOKENS.read_text(encoding="utf-8"))
    night: dict[str, str] = {}
    vellum_attr: dict[str, str] = {}
    vellum_media: dict[str, str] = {}
    for selector, body in blocks(css):
        d = decls(body)
        sel = " ".join(selector.split())
        if sel.startswith("@media (prefers-reduced-motion") or sel.startswith("@media (prefers-contrast"):
            continue
        if sel in (':root, [data-theme="night"]', ":root"):
            night.update(d)
        elif sel == '[data-theme="vellum"]':
            vellum_attr.update(d)
        elif sel.startswith("@media (prefers-color-scheme: light)"):
            vellum_media.update(d)
    if vellum_attr != vellum_media:
        drift = sorted(set(vellum_attr.items()) ^ set(vellum_media.items()))
        raise SystemExit(f"Vellum blocks drifted: {drift}")
    vellum = {**night, **vellum_attr}
    return {"Night": night, "Vellum": vellum}


def resolve(theme: dict[str, str], name: str, seen: frozenset[str] = frozenset()) -> str:
    if name in seen:
        raise SystemExit(f"Circular var() at {name}")
    if name not in theme:
        raise SystemExit(f"Token {name} is not defined")
    value = theme[name]
    m = VAR.match(value)
    if m:
        return resolve(theme, m.group(1), seen | {name})
    if not HEX.match(value):
        raise SystemExit(f"Token {name} must be a 6-digit hex to be checked, got {value!r}")
    return value.upper()


def luminance(hex_value: str) -> float:
    h = hex_value.lstrip("#")
    def channel(c: float) -> float:
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (channel(int(h[k : k + 2], 16) / 255) for k in (0, 2, 4))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def ratio(a: str, b: str) -> float:
    la, lb = luminance(a), luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def main() -> int:
    themes = load_themes()
    rows: list[str] = []
    failures = 0
    for fg, bg, minimum, use in PAIRS:
        cells = []
        for theme_name, theme in themes.items():
            f, b = resolve(theme, fg), resolve(theme, bg)
            r = ratio(f, b)
            ok = r + 1e-9 >= minimum
            failures += 0 if ok else 1
            cells.append(f"`{f}` on `{b}` **{r:.2f}** {'pass' if ok else 'FAIL'}")
            if not ok:
                print(f"FAIL {theme_name}: {fg} on {bg} = {r:.2f} (needs {minimum})", file=sys.stderr)
        kind = "text 4.5" if minimum == TEXT else "UI 3.0"
        rows.append(f"| `{fg}` | `{bg}` | {kind} | {cells[0]} | {cells[1]} | {use} |")

    total = len(PAIRS) * len(themes)
    summary = f"{total - failures} of {total} checks pass" + ("" if failures == 0 else f"; {failures} FAIL")
    if "--no-write" not in sys.argv:
        REPORT.write_text(
            "# Contrast table\n\n"
            "Generated by `python3 docs/design/contrast-check.py` from `docs/design/tokens.css`. "
            "Do not edit by hand. WCAG 2.2 AA: 4.5:1 for text, 3:1 for UI components, "
            "graphical objects and focus indicators.\n\n"
            f"**Result: {summary}.**\n\n"
            "| Foreground | Background | Minimum | Night | Vellum | Used for |\n"
            "|---|---|---|---|---|---|\n" + "\n".join(rows) + "\n",
            encoding="utf-8",
        )
    print(summary)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
