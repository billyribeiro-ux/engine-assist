#!/usr/bin/env python3
"""Screenshot every mockup in both themes at 1440x900 and 1280x800, and run axe-core.

    PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers python3 docs/design/_build/shoot.py \
        [--axe /path/to/axe.min.js] [--only name,name] [--no-shots]

axe-core is not vendored; pass the path to a locally installed axe.min.js.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from playwright.sync_api import sync_playwright

DESIGN = Path(__file__).resolve().parent.parent
MOCKUPS = DESIGN / "mockups"
SHOTS = DESIGN / "screenshots"
VIEWPORTS = [(1440, 900), (1280, 800)]
THEMES = {"night": "dark", "vellum": "light"}
EXTRA_STATES = {"gate": ["sign", "signed"]}  # :target states rendered with CSS only


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--axe")
    ap.add_argument("--only", default="")
    ap.add_argument("--no-shots", action="store_true")
    args = ap.parse_args()
    only = {s for s in args.only.split(",") if s}
    axe_src = Path(args.axe).read_text(encoding="utf-8") if args.axe else None
    SHOTS.mkdir(exist_ok=True)
    results: dict[str, list[dict]] = {}
    with sync_playwright() as p:
        browser = p.chromium.launch()
        for page_file in sorted(MOCKUPS.glob("*.html")):
            name = page_file.stem
            if only and name not in only:
                continue
            states = [""] + EXTRA_STATES.get(name, [])
            for state in states:
                url = page_file.as_uri() + (f"#{state}" if state else "")
                label = name + (f"-{state}" if state else "")
                for theme, scheme in THEMES.items():
                    for w, h in VIEWPORTS:
                        ctx = browser.new_context(viewport={"width": w, "height": h}, color_scheme=scheme,
                                                  reduced_motion="reduce")
                        page = ctx.new_page()
                        requests: list[str] = []
                        page.on("request", lambda r: requests.append(r.url) if not r.url.startswith("file:") else None)
                        page.goto(url)
                        page.wait_for_timeout(150)
                        if not args.no_shots:
                            page.screenshot(path=str(SHOTS / f"{label}-{theme}-{w}.png"))
                        if requests:
                            print(f"EXTERNAL REQUESTS in {label}: {requests}")
                        if axe_src and w == 1440:
                            page.add_script_tag(content=axe_src)
                            res = page.evaluate(
                                """async () => {
                                  const r = await axe.run(document, {runOnly: {type: 'tag',
                                    values: ['wcag2a','wcag2aa','wcag21a','wcag21aa','wcag22aa','best-practice']}});
                                  return r.violations.map(v => ({id: v.id, impact: v.impact, help: v.help,
                                    nodes: v.nodes.slice(0, 6).map(n => n.target.join(' ') + ' :: ' + (n.failureSummary || '').split('\\n').slice(0,3).join(' | '))}));
                                }"""
                            )
                            results[f"{label}-{theme}"] = res
                        ctx.close()
        browser.close()
    if axe_src:
        out = DESIGN.parent.parent / ".axe-results.json"
        print(json.dumps({k: [(v["id"], v["impact"], len(v["nodes"])) for v in vs] for k, vs in results.items()}, indent=1))
        Path(args.axe).parent.joinpath("axe-results.json").write_text(json.dumps(results, indent=1))


if __name__ == "__main__":
    main()
