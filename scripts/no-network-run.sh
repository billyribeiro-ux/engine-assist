#!/usr/bin/env bash
# Runs a command with IP networking denied (ADR-016 decision 6; M0-11, AC-M0-10).
#   scripts/no-network-run.sh cargo test --workspace
#   scripts/no-network-run.sh /Applications/Foreman.app/Contents/MacOS/foreman-app
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != "Darwin" ]; then
  echo "no-network-run: macOS only (sandbox-exec); see docs/runbooks/release.md for the pf fallback." >&2
  exit 2
fi
if ! command -v sandbox-exec >/dev/null; then
  echo "no-network-run: sandbox-exec is unavailable; use the pf anchor fallback (docs/runbooks/release.md)." >&2
  exit 2
fi
# Prove the profile is effective before trusting a green run.
if sandbox-exec -f scripts/sandbox/deny-network.sb /usr/bin/curl -sS -m 5 -o /dev/null https://example.com 2>/dev/null; then
  echo "no-network-run: the deny-network profile did NOT block an outbound request; use the pf fallback." >&2
  exit 3
fi
exec sandbox-exec -f scripts/sandbox/deny-network.sb "$@"
