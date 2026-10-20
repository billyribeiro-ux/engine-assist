# ADR-021: One launch pipeline: fingerprint every Claude Code input, block auth-changing keys, check auth fail-closed, write a fresh run settings file

## Status
Proposed

## Date
2026-10-19

## Context

F-008: every launch goes through `prepare_launch` in ≤ 50 ms with cached data (NFR-04). Revised `docs/requirements.md`:

- **SR-06:** fingerprint every Claude Code input that applies to the step:
  - user and managed settings;
  - `mcpServers` in `~/.claude.json` (user scope and this project's path);
  - user memory and `~/.claude/{agents,skills,commands,rules,output-styles}/**`;
  - every `CLAUDE.md`/`CLAUDE.local.md` from the cwd up to `/` plus their `@` imports;
  - the project's `.claude/**` (excluding `worktrees/`) and `.mcp.json`;
  - enabled plugin folders;
  - every local file named as a hook, `statusLine`, `fileSuggestion` or helper program.

  Any change needs one acknowledgement, with a redacted diff (SR-55). "Verify in 4.2: the exact set Claude Code loads."
- **SR-07:** block `apiKeyHelper`, `awsAuthRefresh`, `awsCredentialExport`, `gcpAuthRefresh`, `otelHeadersHelper`, and `env` keys matching `ANTHROPIC_*`, `CLAUDE_CODE_*`, `CLAUDE_CONFIG_DIR`, `AWS_*`, `GOOGLE_*`, `CLOUDSDK_*`, `AZURE_*`, `*_PROXY`/`*_proxy`, `NODE_EXTRA_CA_CERTS`, `NODE_TLS_REJECT_UNAUTHORIZED`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `NODE_OPTIONS`, `DYLD_*`. The only exception is a key-value hash Billy allowlisted, for user settings only.
- **SR-08:** `claude auth status` only after SR-06/07 pass, in the step cwd with the launch environment, cached against the full fingerprint; launch only on `authMethod` `claude.ai` or `oauth_token`; fail closed on missing or unknown output, a non-zero exit, or a 10 s timeout.
- **SR-09:** a fresh run settings file (`0600`, app data, hash recorded) with:
  - `disableAllHooks: false`;
  - `permissions.disableBypassPermissionsMode: "disable"`;
  - deny rules with **absolute `//` paths**: `Read(//<repo>/.foreman/**)`, `Edit(//<repo>/.foreman/**)`, `Read(//<app-data>/**)`, `Edit(//<app-data>/**)`;
  - the sensor hooks in P2.

  "Verify in 4.2: absolute-path rule syntax."
- **SR-19:** preview and confirmations. **SR-55:** redacted snapshots.

Doc checks (code.claude.com, 2026-10-19):

- **permissions:** `//path` = "Absolute path from filesystem root" (example `Read(//Users/alice/secrets/**)`); a single leading `/` is "relative to the settings source"; "use `//Users/alice/file` for absolute paths". `permissions.disableBypassPermissionsMode` set to `"disable"` "works from any scope". Deny rules written through a symlinked directory also apply at the real location (v2.1.268+). Writes by Bash aren't bound by Read/Edit rules. **SR-09's syntax is confirmed.** Because the run settings file lives in app data, a single-`/` rule would anchor in app data, so `//` is required.
- **settings:**
  - precedence: managed > `--settings` > `.claude/settings.local.json` > `.claude/settings.json` > `~/.claude/settings.json`;
  - `~/.claude.json` holds MCP servers and per-project state;
  - Claude Code **watches settings files and reloads them mid-session, including `apiKeyHelper`**;
  - some keys are ignored in repository files (the Scope column of the settings index).
- **managed-settings:** macOS files `/Library/Application Support/ClaudeCode/managed-settings.json` and `managed-settings.d/*.json`; MDM domain `com.anthropic.claudecode`; server-managed settings from the claude.ai console.

## Decision

1. **`foreman-app::launch::prepare(step)`** gathers facts (via `foreman-sys` and `store::outside`) and calls pure core functions. The order is fixed, and the first blocker stops later checks that depend on it:
   1. version (`claude -v` every launch; floor, ADR-017);
   2. contract (P2);
   3. fingerprint + SR-07 scan;
   4. acknowledgement;
   5. auth (cached);
   6. transcript (continuing steps);
   7. inputs;
   8. run inputs;
   9. prompt hygiene (SR-04, SR-28);
   10. argv and env (ADR-007);
   11. run settings;
   12. confirmation reason (SR-19).

   The result is a `LaunchPlan` stored under a one-time `planId`, valid for 60 s and for the exact project revision and fingerprint.
2. **Fingerprint sources** (`core::fingerprint::sources(cwd, repo_root, home, config_dir)` returns the list; `store::outside` reads it read-only with size caps: 256 KB per file, except `~/.claude.json`, whose own cap is **64 MB** because Claude Code keeps per-project history in it. `~/.claude.json` is parsed as a **stream** (serde_json's streaming deserializer with `IgnoredAny` for every other key), keeping only the `mcpServers` paths below; nothing else is buffered. Larger than the cap → block, naming the file.
   **Symlinks.** Repo content (`.claude/**`, `.mcp.json`, `CLAUDE.md` inside the repo) is read with no symlink at any component, as SR-31 says. For the read-only sources **outside** the repo (`~/.claude/**`, `~/.claude.json`, managed settings, ancestor `CLAUDE.md` files above the repo), a symlink is followed only when its target is a regular file outside every registered project; Foreman hashes the target and records both the link path and the resolved target, so a retarget changes the fingerprint. A symlink into a registered project, to a directory loop, or to a non-regular file blocks the launch with the path. (Dotfile managers commonly symlink `~/.claude/settings.json` and `CLAUDE.md`; S-7′ measures Billy's real `~/.claude` for sizes and symlinks.)
   - `$CLAUDE_CONFIG_DIR` or `~/.claude`: `settings.json`, `CLAUDE.md`, `{agents,skills,commands,rules,output-styles}/**`, enabled plugin folders (from `enabledPlugins` in the settings, resolved under `plugins/`);
   - `~/.claude.json`: **only** the `mcpServers` object at user scope and under `projects[<repo path>]`, parsed and re-serialized canonically. Other keys (sign-in session, trust state) are never retained, logged or diffed;
   - managed: `/Library/Application Support/ClaudeCode/managed-settings.json` and `managed-settings.d/*.json`;
   - `CLAUDE.md` and `CLAUDE.local.md` in the cwd and every ancestor up to `/`, plus `@`-imports (resolved relative to the importing file, depth ≤ 5, each read under the same caps);
   - repo: `.claude/**` except `worktrees/`, and `.mcp.json` at the cwd and the repo root;
   - program files: every local path named as a hook `command`, `statusLine.command`, `fileSuggestion`, `apiKeyHelper`-family helper, or MCP `command` that's an absolute or repo-relative path is hashed. A bare command name is recorded as a name.

   A cache keyed by `(path, dev, ino, size, mtime)` avoids re-hashing unchanged files (NFR-04).
3. **Blocking** (SR-07): `auth_key_hits` scans every parsed settings source (user, managed, project, local, and any `env` maps in `~/.claude.json` project entries) for the keys and env patterns above. The block names the file and key. The allowlist exception stores `sha256(key=value)` for user settings only.
4. **Acknowledgement** (SR-06, SR-55): a changed fingerprint shows a **redacted** diff. Values under `env` or `headers`, any `apiKey*` key, any key containing `token`/`secret`/`password` at any depth, and any string matching the secret-scanner patterns become `‹redacted sha256:8›`. Acknowledging is a native dialog raised by Rust naming the changed sources (SR-24a); the webview banner only shows the diff. Then the new snapshot is stored redacted (`0600`) in `cache/fingerprints/<cwdHash>.json`.
5. **Auth** (SR-08): `claude auth status` runs with the exact launch env in the step cwd, only after steps 3–4 pass. Foreman parses only the JSON `authMethod` and the exit code. Anything unexpected blocks. Results are cached on `(fingerprint, claude version, env names+hash)`.
6. **Run settings file** (SR-09), written fresh via `store` to `runs/<runId>/settings.json` (`0600`):
   ```json
   { "disableAllHooks": false,
     "permissions": { "disableBypassPermissionsMode": "disable",
       "deny": ["Read(//<repo abs>/.foreman/**)", "Edit(//<repo abs>/.foreman/**)",
                "Read(//<app-data abs>/**)", "Edit(//<app-data abs>/**)"] },
     "hooks": { "...P2 sensor, exec form..." } }
   ```
   - Absolute paths are the canonical repo and app-data paths. A path containing glob metacharacters (`*`, `?`, `[`, `{`) is escaped or, if it can't be, blocks with an explanation.
   - The hash is recorded in `run.json`.
   - The threat model states these rules bind Read/Edit/Write tools, not Bash.
7. **Residual risks**, recorded for the Stage 4.3 threat model:
   - Server-managed settings and MDM profiles can't be fingerprinted from files. The auth check still catches an auth-method change.
   - Claude Code reloads settings mid-session, so a change made **after** launch (for example by Claude itself editing `.claude/settings.local.json`) isn't caught until the next launch. The watcher marks `config:changed` for open runs so the UI warns. Nothing is triggered automatically (SR-18).

## Alternatives considered

- **Fingerprint only the repo's `.claude/` files** (R-03's first fix). Misses user and managed settings, ancestor `CLAUDE.md`, user MCP servers and helper scripts (r5b M8, the security review).
- **Launch with `--setting-sources` limited to user + `--settings`.** It would shrink the attack surface but change Claude Code's behavior in Billy's repos (project agents, project settings), which the blueprint relies on. Not in the frozen spec.
- **Hash `~/.claude.json` whole.** It holds the sign-in session and per-project trust state that change constantly, so it would cause noisy acknowledgements and retain sensitive data. Only `mcpServers` is extracted.

## Consequences

- The first launch in a cwd always asks for an acknowledgement, and later ones only when something changed.
- Plugin-folder hashing can be slower for large plugins. The stat cache keeps repeat launches within NFR-04; a first-time scan is excluded from that NFR and shown as "Checking Claude Code config…".
- **UNVERIFIED:** the plugins folder layout and the `enabledPlugins` key format. Tried: the settings and managed-settings docs (which describe plugins only by reference). M3 reads the plugins docs and adds a fixture. If the layout can't be resolved, the whole `plugins/` directory is fingerprinted, which is noisier but still safe.

## Verification

- `preflight::hook_script_change_detected`, `preflight::ancestor_claude_md_detected`, `preflight::user_mcp_server_detected`, and one test per source.
- `preflight::blocks_<key>` (one per key and pattern), `preflight::allowlisted_user_key_passes`.
- `preflight::auth_checked_in_step_cwd`, `preflight::blocks_api_key_auth`, `preflight::auth_unknown_output_blocks`, `preflight::auth_after_fingerprint`.
- `run_settings::contents_snapshot`, `run_settings::deny_paths_absolute` (cwd = project and cwd = spec, and a project path containing a space, e.g. `~/Developer/Trade Log`), `run_settings::fresh_per_run`.
- `fingerprint::claude_json_streamed_64mb` (a 60 MB `~/.claude.json` fixture: only `mcpServers` kept, memory bounded; 65 MB blocks), `fingerprint::outside_symlink_followed_to_regular_file` (link and target recorded; retarget changes the fingerprint), `fingerprint::outside_symlink_into_project_blocks`, `fs_policy::read_refuses_symlink_at_any_component` (repo content unchanged).
- `preflight::config_snapshot_redacts_secrets`, `preflight::diff_shows_redacted_change`, `preflight::confirm_required_for_*`.
- The S-1′ probe confirms the run settings parse, including deny rules for a repo path with a space in it. S-7′ decides whether a project `env` block needs approval and measures Billy's real `~/.claude` and `~/.claude.json` (sizes, symlinks) against the caps above.
- `criterion` `bench_prepare_launch` (NFR-04, warm cache).

## Revisit when

- Claude Code adds a config source or a program-running key. Each new Claude Code version's contract probe notes are checked, and Doctor flags unknown top-level settings keys in fingerprinted files as "not understood by Foreman".

## Exit path

The source list and key lists are data in `foreman-core::fingerprint`. Adding a source is a list entry plus a test. If fingerprinting proves unworkable, the S-7′ fail branch (refuse any cwd whose project settings set `env` until approved) is the stricter fallback.

## Amended 2026-10-20 (M3 runners)

- **`forceLoginMethod` / `forceLoginGatewayUrl` block (SR-07 amended).** S-7′ rehearsal on Claude Code 2.1.304: a project `.claude/settings.json` with `forceLoginMethod: "console"` left `auth status` at `authMethod: "oauth_token"` and added `forcedLoginMethod: "console"`, so SR-08's `authMethod` check alone would have launched. The authentication docs (code.claude.com/docs/en/authentication, "Restrict login to your organization", read 2026-10-20) list `"claudeai"`, `"console"` and `"gateway"`, scope "Any file", and `forceLoginGatewayUrl` selects the gateway sign-in. Decision: `auth_key_hits` blocks `forceLoginMethod` with any value other than `"claudeai"` and any `forceLoginGatewayUrl`, naming file and key (allowlistable in user settings like the other keys); `claude auth status`'s `forcedLoginMethod` other than `claudeai` also fails the auth check (SR-08 amended), which covers managed and server-managed sources the fingerprint can't read. Tests: `preflight::blocks_force_login_method_console`, `…_gateway`, `preflight::blocks_force_login_gateway_url`, `preflight::force_login_method_claudeai_passes`, `preflight::blocks_api_key_auth` (forced-method case).
- **Plugins.** The on-disk layout is still undocumented (the plugin reference documents `${CLAUDE_PLUGIN_DATA}` as `~/.claude/plugins/data/<id>/` only). When any `enabledPlugins` entry is `true`, the whole `<cfg>/plugins/` folder is fingerprinted except `data/` (runtime state, not configuration), hashed while streaming (≤ 64 MB per file, never kept) with the stat cache; the 256 KB cap applies to parsed and Markdown sources only.
- **Symlinked folders block** for every folder source (including the source folder itself, e.g. a symlinked `~/.claude/agents`), as decided above; S-7′'s inventory decides whether to relax this for dotfile managers.
- **Rule paths.** Escaping inside `Read(…)` rules isn't documented, so a repo or app-data path containing `* ? [ ] { } ( ) \` or a control character blocks (`RunSettingsPathUnsafe`) instead of being escaped. Spaces are kept (S-1′ checks them).
- **Auth cache key.** `envKey` leaves out `FOREMAN_RUN_ID` (new every run; it can't change authentication), so the cache can hit (files.md §4.9 amended).
- **`~/.claude.json` with `CLAUDE_CONFIG_DIR`.** Where the global config lives when `CLAUDE_CONFIG_DIR` is set is UNVERIFIED; both `~/.claude.json` and `$CLAUDE_CONFIG_DIR/.claude.json` are fingerprinted. Project `env` maps there are reduced to names plus value hashes before they leave the streaming reader.
- Code: `foreman_core::fingerprint` (sources, `derived_sources`, `auth_key_hits`, `redact`, `snapshot`, `diff`), `foreman_core::store::config_read` (reads under the rules above), `foreman_sys::fingerprint::gather`, `foreman_sys::launch::prepare_launch` (DR-120 order; `claude -v` runs in parallel with the reads).
