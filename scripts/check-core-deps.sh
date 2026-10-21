#!/usr/bin/env bash
# M0-01 / ADR-003 `core-deps` and `sys-deps`: the core never depends on Tauri, tokio, the PTY, the
# watcher or an HTTP stack; foreman-sys never depends on Tauri.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
core=$(cargo tree -p foreman-core -e normal --prefix none --target all)
for banned in tauri tokio portable-pty notify reqwest hyper; do
  if grep -Eq "^${banned}([-_][a-z0-9-]+)? v" <<<"$core"; then
    echo "foreman-core depends on ${banned}:" >&2
    grep -E "^${banned}([-_][a-z0-9-]+)? v" <<<"$core" >&2
    fail=1
  fi
done
sys=$(cargo tree -p foreman-sys -e normal --prefix none --target all)
if grep -Eq "^tauri([-_][a-z0-9-]+)? v" <<<"$sys"; then
  echo "foreman-sys depends on tauri" >&2
  fail=1
fi
if [ "$fail" -ne 0 ]; then exit 1; fi
echo "core-deps: foreman-core has no tauri/tokio/portable-pty/notify/reqwest/hyper; foreman-sys has no tauri"
