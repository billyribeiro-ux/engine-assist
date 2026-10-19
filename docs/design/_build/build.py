#!/usr/bin/env python3
"""Build the self-contained mockups in docs/design/mockups/ (stdlib only).

Each screen in _build/screens/<name>.html is a body fragment with placeholders.
This script inlines tokens.css + base.css, the icon sprite, the shared sidebar,
strip, title bar and legend, and a tiny theme-toggle script, so every output
file has no external requests.

    python3 docs/design/_build/build.py
"""

from __future__ import annotations

import html
import re
from pathlib import Path

BUILD = Path(__file__).resolve().parent
DESIGN = BUILD.parent
OUT = DESIGN / "mockups"

# ---------------------------------------------------------------- icons
ICONS = {
    "terminal": '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M5 6.8l2 1.6-2 1.6M8.5 10.4H11"/>',
    "chat": '<path d="M2.8 4h10.4v6.6H8.2L5 13.2v-2.6H2.8z"/>',
    "desktop": '<rect x="2" y="3" width="12" height="8" rx="1"/><path d="M6 13.5h4M8 11v2.5"/>',
    "lock": '<rect x="4" y="7" width="8" height="6" rx="1"/><path d="M5.6 7V5.6a2.4 2.4 0 0 1 4.8 0V7"/>',
    "check": '<path d="M3.5 8.5l3 3 6-7"/>',
    "cross": '<path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/>',
    "dot": '<circle cx="8" cy="8" r="3.6" fill="currentColor" stroke="none"/>',
    "ring": '<circle cx="8" cy="8" r="4.2"/>',
    "magnifier": '<circle cx="7" cy="7" r="3.8"/><path d="M9.8 9.8L13.2 13.2"/>',
    "stamp": '<g class="stamp"><circle cx="8" cy="8" r="6.2"/><circle cx="8" cy="8" r="4.4" stroke-width="0.9"/><path d="M5.9 8.1l1.5 1.5 2.8-3.1"/></g>',
    "question": '<path d="M6 6.1a2.1 2.1 0 1 1 2.8 2c-.5.2-.8.6-.8 1.1v.5"/><circle cx="8" cy="12" r=".7" fill="currentColor" stroke="none"/>',
    "copy": '<rect x="5.5" y="5.5" width="8" height="8" rx="1"/><path d="M3.5 10.5v-6a1 1 0 0 1 1-1h6"/>',
    "paste": '<rect x="3.5" y="3.5" width="9" height="10.5" rx="1"/><rect x="6" y="2" width="4" height="3" rx=".8"/>',
    "chev": '<path d="M6 4l4 4-4 4"/>',
    "chev-down": '<path d="M4 6l4 4 4-4"/>',
    "plus": '<path d="M8 3.5v9M3.5 8h9"/>',
    "warn": '<path d="M8 2.6L14.2 13.2H1.8z"/><path d="M8 6.8v2.8"/><circle cx="8" cy="11.4" r=".7" fill="currentColor" stroke="none"/>',
    "info": '<circle cx="8" cy="8" r="6"/><path d="M8 7.3v3.8"/><circle cx="8" cy="5.2" r=".7" fill="currentColor" stroke="none"/>',
    "sidebar": '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M6 3v10"/>',
    "inspector": '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M10 3v10"/>',
    "drawer": '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M2 9.5h12"/>',
    "search": '<circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/>',
    "sliders": '<path d="M2.5 5h11M2.5 11h11"/><circle cx="6" cy="5" r="1.7" fill="var(--sidebar)"/><circle cx="10.5" cy="11" r="1.7" fill="var(--sidebar)"/>',
    "clock": '<circle cx="8" cy="8" r="5.6"/><path d="M8 5v3.2l2.1 1.3"/>',
    "log": '<path d="M5.5 4.5h8M5.5 8h8M5.5 11.5h8"/><circle cx="3" cy="4.5" r=".6" fill="currentColor"/><circle cx="3" cy="8" r=".6" fill="currentColor"/><circle cx="3" cy="11.5" r=".6" fill="currentColor"/>',
    "pulse": '<path d="M1.8 8.5h3l1.6-4 3 7.2 1.6-3.2h3.2"/>',
    "folder": '<path d="M2.5 4.2h4l1.5 1.5h5.5v6.8h-11z"/>',
    "tray": '<path d="M2.5 9.5l2-6h7l2 6v3.5h-11z"/><path d="M2.5 9.5h3l1 1.5h3l1-1.5h3"/>',
    "hatch": '<rect x="3" y="3" width="10" height="10"/><path d="M3 8l5-5M3 13l10-10M8 13l5-5"/>',
    "dashsq": '<rect x="3" y="3" width="10" height="10" stroke-dasharray="2 1.6"/>',
    "open": '<path d="M5 11l6-6M6 5h5v5"/>',
    "restore": '<path d="M5 4.5L2.5 7 5 9.5"/><path d="M2.5 7H10a3.2 3.2 0 0 1 0 6.4H7.5"/>',
    "wrench": '<path d="M10.2 2.8a3 3 0 0 0-3.6 4l-4 4a1.2 1.2 0 0 0 1.7 1.7l4-4a3 3 0 0 0 4-3.6l-1.8 1.8-1.7-.4-.4-1.7z"/>',
    "import": '<path d="M8 2.5v7.5M5 7l3 3 3-3M3 13.5h10"/>',
    "branch": '<circle cx="5" cy="3.8" r="1.5"/><circle cx="5" cy="12.2" r="1.5"/><circle cx="11" cy="5.8" r="1.5"/><path d="M5 5.3v5.4M11 7.3c0 2.3-2.4 2.6-5.4 3.8"/>',
    "external": '<path d="M9 3h4v4M13 3L7.5 8.5M11.5 9.5V13H3V4.5h3.5"/>',
    "skip": '<path d="M4 4l5 4-5 4zM11.5 4v8"/>',
    "play": '<path d="M5 3.5l7.5 4.5L5 12.5z"/>',
    "pen": '<path d="M10.8 2.8l2.4 2.4-7.6 7.6-3 .6.6-3z"/>',
    "file": '<path d="M4 2.5h5l3 3v8H4z"/><path d="M9 2.5v3h3"/>',
    "link": '<path d="M7 9a2.6 2.6 0 0 0 3.7 0l2-2a2.6 2.6 0 0 0-3.7-3.7l-.8.8M9 7a2.6 2.6 0 0 0-3.7 0l-2 2A2.6 2.6 0 0 0 7 12.7l.8-.8"/>',
    "stop": '<rect x="4" y="4" width="8" height="8" rx="1"/>',
    "broken": '<path d="M7 9a2.6 2.6 0 0 0 3.7 0l2-2a2.6 2.6 0 0 0-3.7-3.7M9 7a2.6 2.6 0 0 0-3.7 0l-2 2A2.6 2.6 0 0 0 7 12.7M2.5 2.5l2 2M11.5 11.5l2 2"/>',
}


def sprite() -> str:
    symbols = "".join(
        f'<symbol id="i-{name}" viewBox="0 0 16 16">{body}</symbol>' for name, body in ICONS.items()
    )
    return f'<svg width="0" height="0" style="position:absolute" aria-hidden="true">{symbols}</svg>'


def icon(name: str, cls: str = "icon") -> str:
    if name not in ICONS:
        raise SystemExit(f"unknown icon {name}")
    return f'<svg class="{cls}" aria-hidden="true" focusable="false"><use href="#i-{name}"/></svg>'


def glyph(name: str, color: str = "muted") -> str:
    return icon(name, f"icon glyph glyph-{color}")


# ---------------------------------------------------------------- strip
STAGES = [
    "Land Survey", "Meet the Architect", "Architect's Design Studio", "Permits & Zoning",
    "Construction Drawings", "Site Prep & Excavation", "Foundation", "Framing", "Roofing",
    "Plumbing", "Electrical", "HVAC", "Insulation & Drywall", "Siding & Exterior",
    "Inspections", "Paint & Trim", "Final Walkthrough", "Certificate of Occupancy",
    "Homeowner Maintenance",
]
PHASES = [
    ("pre", "Pre-construction", range(0, 5)),
    ("structure", "Structure", range(5, 9)),
    ("systems", "Systems", range(9, 12)),
    ("finish", "Finishes", range(12, 16)),
    ("co", "Occupancy", range(16, 18)),
    ("maint", "Ownership", range(18, 19)),
]

# state -> (css class, mark icon or None, spoken status)
STATE = {
    "na": ("is-na", None, "not applicable"),
    "signed": ("", "stamp", "signed"),
    "progress": ("is-current", "dot", "in progress"),
    "inspecting-current": ("is-current", "magnifier", "inspecting"),
    "inspecting": ("", "magnifier", "inspecting"),
    "waiting": ("is-waiting", "magnifier", "packet waiting for your signature"),
    "ready": ("", "ring", "ready"),
    "locked": ("is-locked", None, "locked"),
    "annot": ("is-annot", None, "needs annotation"),
    "undecided": ("is-annot", "question", "a condition is undecided"),
    "undecided-only": ("is-locked", "question", "a condition is undecided"),
}


def strip_html(project: str, states: dict[int, tuple[str, str]], extra_marks: str = "", p2: bool = True) -> str:
    groups = []
    for key, label, rng in PHASES:
        items = []
        for n in rng:
            state, why = states[n]
            cls, mark, spoken = STATE[state]
            name = STAGES[n]
            aria = f"Stage {n}, {name}: {spoken}" + (f" ({why})" if why else "")
            current = ' aria-current="page"' if "is-current" in cls else ""
            mark_svg = ""
            if mark:
                mark_cls = "sheet-mark"
                mark_svg = f'<svg class="{mark_cls}" viewBox="0 0 16 16" aria-hidden="true"><use href="#i-{mark}"/></svg>'
            if n in EXTRA_MARKS.get(extra_marks, {}):
                mark_svg = EXTRA_MARKS[extra_marks][n]
            items.append(
                f'<li><a class="sheet {cls} sheet-{n}" href="#stage-{n}" aria-label="{html.escape(aria)}" '
                f'title="{n} {html.escape(name)}: {html.escape(spoken)}"{current}>'
                f'<span class="sheet-num" aria-hidden="true">{n}</span>{mark_svg}</a></li>'
            )
        groups.append(
            f'<li class="phase-group" style="--n:{len(rng)};--phase:var(--phase-{key})">'
            f'<ol class="phase-sheets" aria-label="{label}, stages {rng.start} to {rng.stop - 1}">{"".join(items)}</ol>'
            f'<span class="phase-dim" aria-hidden="true"><span>{label}</span></span></li>'
        )
    tag = '<span class="p2tag">P2</span>' if p2 else ""
    return (
        f'<nav class="strip p2" aria-label="{project} drawing set: 19 stages">{tag}'
        f'<ol class="strip-groups">{"".join(groups)}</ol></nav>'
    )


# Stamp that lands during the signing demo (gate.html#signed)
EXTRA_MARKS = {
    "signing": {
        5: '<svg class="sheet-mark mark-inspect" viewBox="0 0 16 16" aria-hidden="true"><use href="#i-magnifier"/></svg>'
           '<svg class="sheet-mark mark-stamp" viewBox="0 0 16 16" aria-hidden="true"><g class="stamp is-landing">'
           '<circle cx="8" cy="8" r="6.2"/><circle cx="8" cy="8" r="4.4" stroke-width="0.9"/>'
           '<path d="M5.9 8.1l1.5 1.5 2.8-3.1"/></g></svg>',
    }
}

ANNOT = {n: ("annot", "blueprint 2.2 annotates stages 0 to 5") for n in range(6, 19)}
STRIPS = {
    # TradeLog during Stage 2 (commercial = no, payments undecided)
    "s2": {0: ("na", "idea_known = yes"), 1: ("signed", "2026-10-12"), 2: ("progress", ""),
           3: ("locked", "until Stage 2 is signed"), 4: ("locked", ""), 5: ("locked", ""),
           **ANNOT, 10: ("undecided", "payments = undecided")},
    # TradeLog during Stage 1
    "s1": {0: ("na", "idea_known = yes"), 1: ("progress", ""), 2: ("locked", "until Stage 1 is signed"),
           3: ("locked", ""), 4: ("locked", ""), 5: ("locked", ""), **ANNOT,
           10: ("undecided", "payments = undecided")},
    # Wizard preview before the project exists
    "preview": {0: ("na", "idea_known = yes"), 1: ("ready", ""), 2: ("locked", ""), 3: ("locked", ""),
                4: ("locked", ""), 5: ("locked", ""), **ANNOT, 10: ("undecided", "payments = undecided")},
    # TradeLog during Stage 4 (assembler; payments decided no)
    "s4": {0: ("na", "idea_known = yes"), 1: ("signed", ""), 2: ("signed", ""), 3: ("signed", ""),
           4: ("progress", "assembling the build package"), 5: ("locked", "until Stage 4 is signed"), **ANNOT,
           10: ("na", "payments = no")},
    # TradeLog during the Stage 5 gate (later snapshot; payments decided no)
    "s5": {0: ("na", "idea_known = yes"), 1: ("signed", ""), 2: ("signed", ""), 3: ("signed", ""),
           4: ("signed", ""), 5: ("inspecting-current", "gate in progress"), **ANNOT,
           10: ("na", "payments = no")},
}


# ---------------------------------------------------------------- sidebar
SIDE_STAGES = {
    "s2": [("hatch", "muted", "0", "Land Survey", "not applicable", False),
           ("stamp", "pass", "1", "Meet the Architect", "signed", False),
           ("dot", "accent", "2", "Architect's Design Studio", "in progress", True),
           ("lock", "muted", "3", "Permits & Zoning", "locked", False),
           ("lock", "muted", "4", "Construction Drawings", "locked", False),
           ("lock", "muted", "5", "Site Prep & Excavation", "locked", False),
           ("dashsq", "muted", "6–18", "Need annotation", "need annotation", False)],
    "s1": [("hatch", "muted", "0", "Land Survey", "not applicable", False),
           ("dot", "accent", "1", "Meet the Architect", "in progress", True),
           ("lock", "muted", "2", "Architect's Design Studio", "locked", False),
           ("lock", "muted", "3", "Permits & Zoning", "locked", False),
           ("lock", "muted", "4", "Construction Drawings", "locked", False),
           ("lock", "muted", "5", "Site Prep & Excavation", "locked", False),
           ("dashsq", "muted", "6–18", "Need annotation", "need annotation", False)],
    "s4": [("hatch", "muted", "0", "Land Survey", "not applicable", False),
           ("stamp", "pass", "1–3", "Signed", "signed", False),
           ("dot", "accent", "4", "Construction Drawings", "in progress", True),
           ("lock", "muted", "5", "Site Prep & Excavation", "locked", False),
           ("dashsq", "muted", "6–18", "Need annotation", "need annotation", False)],
    "s5": [("hatch", "muted", "0", "Land Survey", "not applicable", False),
           ("stamp", "pass", "1–4", "Signed", "signed", False),
           ("magnifier", "accent", "5", "Site Prep & Excavation", "gate in progress", True),
           ("dashsq", "muted", "6–18", "Need annotation", "need annotation", False)],
}


def sidebar(active: str) -> str:
    # active: today | tradelog-s1 | tradelog-s2 | tradelog-s5 | run-log | doctor | settings | template-health | setup
    def cur(key: str) -> str:
        return ' aria-current="page"' if active == key else ""

    proj_open = active.startswith("tradelog")
    stage_key = active.split("-")[1] if proj_open else ""
    stages = ""
    if proj_open:
        rows = []
        for g, color, num, name, spoken, current in SIDE_STAGES[stage_key]:
            cur_attr = ' aria-current="step"' if current else ""
            rows.append(
                f'<li><a class="side-stage" href="#stage-{num}"{cur_attr} '
                f'aria-label="Stage {num}, {html.escape(name)}: {spoken}">{glyph(g, color)}'
                f'<span class="num">{num}</span><span>{html.escape(name)}</span></a></li>'
            )
        stages = f'<ol class="side-stages" aria-label="TradeLog stages">{"".join(rows)}</ol>'
    tl_cur = ' aria-current="page"' if proj_open else ""
    if active == "none":
        return f"""
<nav class="sidebar" aria-label="Foreman">
  <div class="traffic" aria-hidden="true"><span></span><span></span><span></span></div>
  <div class="side-scroll">
    <a class="side-item" href="today.html">{icon("tray")}Today</a>
    <div class="side-head">Projects</div>
    <p class="side-stage" style="padding-left:var(--space-3);display:block">No projects yet</p>
  </div>
  <div class="side-foot">
    <a class="side-item" href="doctor.html">{icon("pulse")}Doctor</a>
    <a class="side-item" href="settings.html">{icon("sliders")}Settings<span class="meta">⌘,</span></a>
  </div>
</nav>"""
    return f"""
<nav class="sidebar" aria-label="Foreman">
  <div class="traffic" aria-hidden="true"><span></span><span></span><span></span></div>
  <div class="side-scroll">
    <a class="side-item" href="today.html"{cur("today")}>{icon("tray")}Today<span class="badge" aria-label="5 items waiting on you">5</span></a>
    <div class="side-head" id="side-projects">Projects</div>
    <ul aria-labelledby="side-projects">
      <li><a class="side-item" href="workspace.html"{tl_cur}>{icon("folder")}TradeLog<span class="meta">Stage {stage_key[1:] if stage_key in ("s1", "s4", "s5") else "2"}</span></a>{stages}</li>
      <li><a class="side-item" href="#foreman">{icon("folder")}Foreman<span class="badge" aria-label="1 problem">{glyph("cross", "fail")}</span></a></li>
      <li><a class="side-item" href="#swings">{icon("folder")}Swings<span class="badge" aria-label="Packet waiting for your signature"><span class="dot attention"></span></span></a></li>
    </ul>
  </div>
  <div class="side-foot">
    <a class="side-item" href="run-log.html"{cur("run-log")}>{icon("log")}Run log</a>
    <a class="side-item" href="template-health.html"{cur("template-health")}>{icon("file")}Template health</a>
    <a class="side-item" href="doctor.html"{cur("doctor")}>{icon("pulse")}Doctor<span class="badge" aria-label="1 problem">{glyph("cross", "fail")}</span></a>
    <a class="side-item" href="settings.html"{cur("settings")}>{icon("sliders")}Settings<span class="meta">⌘,</span></a>
  </div>
</nav>"""


def titlebar(spec: str) -> str:
    parts = spec.split("|")
    title, sub = parts[0], parts[1] if len(parts) > 1 else ""
    flags = parts[2] if len(parts) > 2 else ""
    buttons = f'<button class="btn ghost icon-only" aria-label="Commands and search (⌘K)" title="Commands and search ⌘K">{icon("search")}</button>'
    if "drawer" in flags:
        buttons += f'<button class="btn ghost icon-only" aria-label="Toggle terminal drawer (⌃`)" aria-pressed="true" title="Terminal ⌃`">{icon("drawer")}</button>'
    if "insp" in flags:
        buttons += f'<button class="btn ghost icon-only" aria-label="Toggle inspector (⌘⌥I)" aria-pressed="true" title="Inspector ⌘⌥I">{icon("inspector")}</button>'
    sub_html = f'<span class="titlebar-sub">{sub}</span>' if sub else ""
    return f'<header class="titlebar"><span class="titlebar-title">{title}</span>{sub_html}<span class="spacer"></span>{buttons}</header>'


def legend(text: str) -> str:
    key = text or '<span class="p2tag">P2</span> = phase 2'
    return (
        f'<aside class="mock-legend" aria-label="Mockup legend (not part of the app)">'
        f'<span class="p2key">Mockup: {key}</span>'
        f'<button type="button" id="theme-toggle">Theme: System</button></aside>'
    )


def spec(values: str) -> str:
    labels = ["Surface", "Model", "Effort", "Mode", "Agents", "Note"]
    parts = values.split("|")
    out = []
    for i, raw in enumerate(parts):
        edited = raw.startswith("~")
        val = raw[1:] if edited else raw
        label = labels[i] if i < len(labels) else "Note"
        inner = val
        if i == 0:
            ic = "terminal" if val.startswith("CLI") else ("chat" if val.startswith("Chat") else "desktop")
            inner = f"{icon(ic, 'icon sm')}{val}"
        cls = "model" if i == 1 else ""
        if edited:
            cls = "edited"
        out.append(f'<div><dt>{label}</dt><dd class="{cls}">{inner}</dd></div>')
    return f'<dl class="spec-line" aria-label="Runs with">{"".join(out)}</dl>'


def mini(code: str, label: str) -> str:
    """Miniature strip. code: 19 chars, n=n/a s=signed c=current l=locked a=needs annotation."""
    cls = {"n": "na", "s": "sig", "c": "cur", "l": "", "a": "an"}
    starts = {5, 9, 12, 16, 18}
    cells = []
    for i, ch in enumerate(code):
        c = cls[ch] + (" gap" if i in starts else "")
        cells.append(f'<i class="{c.strip()}"></i>')
    return f'<span class="mini-strip p2" role="img" aria-label="{html.escape(label)}">{"".join(cells)}</span>'


THEME_JS = """<script>
(function () {
  var b = document.getElementById('theme-toggle'); if (!b) return;
  var order = ['system', 'night', 'vellum'], cur = 'system';
  try { cur = localStorage.getItem('fm-mock-theme') || 'system'; } catch (e) {}
  function apply(t) {
    cur = t;
    if (t === 'system') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.setAttribute('data-theme', t);
    b.textContent = 'Theme: ' + t.charAt(0).toUpperCase() + t.slice(1);
    try { localStorage.setItem('fm-mock-theme', t); } catch (e) {}
  }
  apply(cur);
  b.addEventListener('click', function () { apply(order[(order.indexOf(cur) + 1) % order.length]); });
})();
</script>"""

PLACEHOLDER = re.compile(r"\{\{(\w+)(?::([^}]*))?\}\}")


def render(text: str) -> str:
    def sub(m: re.Match[str]) -> str:
        kind, arg = m.group(1), m.group(2) or ""
        if kind == "sidebar":
            return sidebar(arg)
        if kind == "strip":
            key, _, extra = arg.partition(",")
            return strip_html("TradeLog", STRIPS[key], extra)
        if kind == "titlebar":
            return titlebar(arg)
        if kind == "legend":
            return legend(arg)
        if kind == "i":
            return icon(arg)
        if kind == "g":
            name, _, color = arg.partition(":")
            return glyph(name, color or "muted")
        if kind == "spec":
            return spec(arg)
        if kind == "mini":
            code, _, label = arg.partition(",")
            return mini(code, label)
        if kind == "p2":
            return '<span class="p2tag" title="Ships in phase 2">P2</span>'
        if kind == "include":
            return render((BUILD / "fragments" / f"{arg}.html").read_text(encoding="utf-8"))
        raise SystemExit(f"unknown placeholder {m.group(0)}")
    return PLACEHOLDER.sub(sub, text)


def build() -> None:
    tokens = (DESIGN / "tokens.css").read_text(encoding="utf-8")
    base = (BUILD / "base.css").read_text(encoding="utf-8")
    OUT.mkdir(exist_ok=True)
    for src in sorted((BUILD / "screens").glob("*.html")):
        raw = src.read_text(encoding="utf-8")
        title_m = re.match(r"<!--\s*title:\s*(.*?)\s*-->\s*", raw)
        title = title_m.group(1) if title_m else src.stem
        css_m = re.search(r"<style>(.*?)</style>", raw, re.S)
        extra_css = css_m.group(1) if css_m else ""
        body = raw[title_m.end():] if title_m else raw
        body = re.sub(r"<style>.*?</style>", "", body, flags=re.S)
        page = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(title)}</title>
<style>
{tokens}
{base}
{extra_css}
</style>
</head>
<body>
{sprite()}
{render(body)}
{THEME_JS}
</body>
</html>
"""
        (OUT / src.name).write_text(page, encoding="utf-8")
        print("wrote", (OUT / src.name).relative_to(DESIGN.parent.parent))


if __name__ == "__main__":
    build()
