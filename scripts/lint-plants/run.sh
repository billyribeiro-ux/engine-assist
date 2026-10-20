#!/usr/bin/env bash
# Planted-violation proof for the security lint rules (M0-03). See run.mjs.
set -euo pipefail
cd "$(dirname "$0")/../.."
exec node scripts/lint-plants/run.mjs "$@"
