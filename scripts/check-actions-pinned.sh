#!/usr/bin/env bash
# CI `actions-pinned` (ADR-024, SR-61): every `uses:` is pinned to a 40-hex commit SHA, and every
# workflow sets `permissions: contents: read` at the top level.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0
shopt -s nullglob
for wf in .github/workflows/*.yml .github/workflows/*.yaml; do
  while IFS= read -r line; do
    ref=$(sed -E 's/.*uses:[[:space:]]*["'\'']?([^"'\''[:space:]]+).*/\1/' <<<"$line")
    case "$ref" in
      ./*) continue ;;
    esac
    if ! grep -Eq '@[0-9a-f]{40}$' <<<"$ref"; then
      echo "$wf: not pinned by SHA: $ref" >&2
      fail=1
    fi
  done < <(grep -E '^[[:space:]]*-?[[:space:]]*uses:' "$wf" || true)
  if ! grep -Pzq '(?m)^permissions:\n[[:space:]]+contents: read' "$wf"; then
    echo "$wf: missing top-level 'permissions: contents: read'" >&2
    fail=1
  fi
done
exit "$fail"
