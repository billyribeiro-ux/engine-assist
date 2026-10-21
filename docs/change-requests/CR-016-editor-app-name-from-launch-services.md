# CR-016: Resolve the editor's app name from the system, not a fixed table

STATUS: proposed
REQUESTED BY: Claude (security fixes, branch stage/05-secfix) · 2026-10-20
DECIDED BY: <Billy> · <date>

## What changes
The security review of 2026-10-20 (W-1) asked for a native "Use <App name>?" dialog that shows
the **resolved** app name before the editor bundle ID changes. As built, `settings_update` asks
that dialog and refuses apps that run what they open (`foreman-app::editor::REFUSED`), but the
name comes from a fixed table of common editors (`editor::KNOWN`: Visual Studio Code, Cursor, Zed,
Sublime Text, TextEdit, BBEdit, Nova, CotEditor, Xcode). Any other ID is shown as the ID, with
"Foreman doesn't know this app ID. Make sure it is a text editor."

Resolving the ID through the system needs one of these, and each one changes a frozen list:

- **Option A:** run `/usr/bin/mdfind 'kMDItemCFBundleIdentifier == "<id>"'` through
  `foreman-sys::spawn` (fixed argv, app-data cwd, 5 s; the ID is already restricted to
  `[A-Za-z0-9-]` and dots, so it can't change the query) and take the `.app` name from the
  first path. This changes SR-01 (the program allowlist) and needs Spotlight indexing on.
- **Option B:** read `Contents/Info.plist` (`CFBundleIdentifier`, `CFBundleName`) of the `.app`
  bundles in `/Applications`, `/Applications/Utilities`, `/System/Applications`,
  `/System/Applications/Utilities` and `~/Applications`, through new entries in
  `store::outside`'s fixed list. This changes SR-31a and needs a binary-plist parser (`plist`
  1.10.2 is already in the macOS dependency tree through `tauri`).

With either option, an ID that resolves to no installed app would be refused ("No app with this
ID is installed"), and the refusal list would also be checked against the resolved app's name.

## Why
A name read from the system can't be faked by choosing an ID outside the table, and a typo in
the ID shows up before saving. The table covers the common editors; the bundle-ID denylist is the
control that matters for W-1, and it is built.

## Impact
- Scope / feature catalog (F-IDs): F-029 (Settings).
- ADRs: ADR-007 (spawned programs) for A; ADR-011 (outside reads) for B.
- Threat model: T-004 (the dialog's name becomes authoritative).
- Data model and migrations: none.
- API contract and versioning: none (`settings_update`'s dialog text only).
- Timeline: small (either option, with a Mac check of the result).

## Recommendation
APPROVE B if the dialog should name the app (no new program, no Spotlight dependency), or keep
the table (REJECT) since the denylist already closes W-1.
