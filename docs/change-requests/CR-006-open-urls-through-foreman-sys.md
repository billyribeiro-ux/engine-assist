# CR-006: URLs open through `foreman-sys` (`/usr/bin/open -u <url>`), not the opener plugin

STATUS: proposed
REQUESTED BY: the wave-2 chat build agent (tickets M3-12, M3-23) · 2026-10-20
DECIDED BY: <Billy Ribeiro> · <date>

## What changes

Every URL Foreman opens in the default browser (Copy and open's claude.ai links, terminal links, later the Doctor help links) opens through one `foreman-sys::spawn` request, `SpawnRequest::OpenUrl(OpenUrl)`, which runs `/usr/bin/open -u <url>` with the usual spawn checks (re-resolved program, cleared environment, cwd in app data, 10 s timeout). `OpenUrl` can only be built by `foreman_core::urls` after the SR-25 allowlist (`foreman_url`, `chat_url`) or the SR-29 terminal policy (`terminal_url`), and the spawn request re-checks that the argument is an `http(s)` URL. The app reaches it only through `app::os::Os::open_url` (tests use the scripted OS, which records the URL).

`-u` ("Opens URL with whatever application claims the url scheme, even if URL also matches a file path", open(1), checked 2026-10-20 at <https://dssw.co.uk/reference/open/>) keeps a URL from being read as a file path.

## Why

- The wave-2 instructions for this build say all opens go through `foreman-sys`'s `open` and nothing opens in the webview.
- `tauri-plugin-opener` isn't a dependency of `foreman-app` (`src-tauri/Cargo.toml` pins only dialog and clipboard-manager). The design doc (§3.2 plugin list, the SR-25 row "`app::os::open_url`") and ADR-014 still name the opener plugin's `open_url`.
- SR-01 lists `/usr/bin/open` "only in SR-25a's forms" (files: `-b <editor>`, `-R`). Opening a URL with it is a new form, so the requirement text changes; the behavior Billy sees (the default browser opens an allowed URL) does not.
- One spawn site keeps the single-`Command` rule (SR-02) and lets the URL checks live in the pure core, where they are unit-tested (`opener::only_allowed_urls`, `opener::terminal_link_scheme_policy`).

## Impact
- Scope / feature catalog (F-IDs): F-009 (terminal links), F-012 (Copy and open), F-019 (Doctor links, later). No acceptance criterion changes.
- ADRs (new, amended, superseded): ADR-014 amended: "`open_url` … after URL parsing" is `foreman-sys` `/usr/bin/open -u`, not the opener plugin; `open_js_links_on_click(false)` stays relevant only if the opener plugin is ever added.
- Threat model: T-093 and T-100 mitigations unchanged in substance (parse, allowlist, native dialog for terminal links). New residual to note: `open -u` hands the URL to LaunchServices, whose URL parser may differ from Foreman's; Foreman refuses whitespace, controls, backslashes and invisible characters, and allowlisted URLs are ASCII URL characters only, which leaves no parser disagreement on the host.
- Data model and migrations: none.
- API contract and versioning: none (the IPC commands are unchanged; `docs/contracts/ipc.md` notes the as-built path).
- Requirements: SR-01 gains "`/usr/bin/open -u <url>` for a URL that passed SR-25 or SR-29".
- Pricing and entitlements: none.
- Timeline: none; built in this branch.
- Cost (build + run): none; no new dependency.

## Recommendation
APPROVE. It removes a plugin dependency the design doc expected, keeps every spawn in one place, and the URL policy is stricter and tested in the core.

**Mac task:** confirm `/usr/bin/open -u https://claude.ai/` opens the default browser on Billy's macOS version (the flag is in the current open(1) page; the Mojave page lacks it).

## Deltas for Stage 4 (filled in when approved)
- docs/requirements.md: SR-01 (the `open` forms).
- docs/design-doc.md: §3.2 plugin list (opener not used), the SR-25 row.
- docs/adr/: ADR-014 amendment.
- docs/threat-model.md: T-093, T-100 residual note.
- docs/tickets.md: none.
