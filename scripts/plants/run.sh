#!/usr/bin/env bash
# Stage 5 gate proof (M0-06, M0-07; AC-M0-04, AC-M0-06, AC-M0-17, SR-63): each plant must make its gate FAIL, and is
# then reverted. Files touched are backed up first and restored on exit, even on error.
#
#   scripts/plants/run.sh            # all plants
#   scripts/plants/run.sh type secret clippy vuln-js vuln-rust postinstall reqwest keyring git-npm
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$(pwd)
BACKUP=$(mktemp -d)
CREATED=()

backup() { for f in "$@"; do mkdir -p "$BACKUP/$(dirname "$f")"; cp -p "$f" "$BACKUP/$f"; done; }
restore() {
  (cd "$BACKUP" && find . -type f) | while read -r f; do cp -p "$BACKUP/$f" "$ROOT/$f"; done
  for f in "${CREATED[@]}"; do rm -f "$ROOT/$f"; done
  rm -rf "$BACKUP"
  mkdir -p "$BACKUP"
}
cleanup() {
  restore
  rm -rf "$BACKUP"
  pnpm install --frozen-lockfile --silent >/dev/null 2>&1 || true
}
trap cleanup EXIT

results=()
# Runs a command and succeeds only if it FAILS with output matching a pattern, so an unrelated
# failure (network, typo) can't count as the gate catching the plant.
fails_with() { # pattern, command...
  local pattern=$1; shift
  local out
  if out=$("$@" 2>&1); then echo "$out" | tail -20; return 0; fi
  if grep -Eq "$pattern" <<<"$out"; then return 1; fi
  echo "failed for another reason:"; echo "$out" | tail -20; return 0
}
expect_fail() { # name, command...
  local name=$1; shift
  echo "▶ plant: $name"
  if "$@" >"$BACKUP.log" 2>&1; then
    echo "  MISSED: $name did not fail its gate"; tail -20 "$BACKUP.log"; results+=("MISSED $name")
  else
    echo "  CAUGHT: $name"; results+=("CAUGHT $name")
  fi
  restore
}

plant_type() {
  CREATED+=("src/lib/__plant_type_error.ts")
  printf "export const plant: number = 'not a number';\n" > src/lib/__plant_type_error.ts
  expect_fail "type error (tsc)" fails_with "TS2322" node scripts/verify.mjs --only=tsc
}

plant_secret() {
  CREATED+=("src/lib/__plant_secret.ts")
  # A random, fake GitHub-token-shaped string, generated at runtime so this script isn't a finding.
  local tok
  tok="ghp_$(head -c 600 /dev/urandom | LC_ALL=C tr -dc 'A-Za-z0-9' | cut -c1-36)"
  printf "export const token = '%s';\n" "$tok" > src/lib/__plant_secret.ts
  expect_fail "fake secret (gitleaks)" fails_with "leaks found: [1-9]" node scripts/verify.mjs --only=secret-scan
}

plant_clippy() {
  backup crates/foreman-core/src/lib.rs
  printf '\n/// Plant.\npub fn plant() -> u32 {\n    let x = 1;\n    return x;\n}\n' >> crates/foreman-core/src/lib.rs
  expect_fail "clippy warning (needless_return)" fails_with "needless_return|let_and_return" node scripts/verify.mjs --only=clippy
}

plant_vuln_js() {
  backup package.json pnpm-lock.yaml
  # minimist 1.2.5: GHSA-xvch-5gv4-984h (prototype pollution).
  pnpm add -D minimist@1.2.5
  expect_fail "known-vulnerable JS dependency (pnpm audit)" fails_with "minimist" node scripts/verify.mjs --only=pnpm-audit
}

plant_vuln_rust() {
  backup crates/fake-claude/Cargo.toml Cargo.lock
  # time 0.1.45: RUSTSEC-2020-0071 (segfault in localtime_r), patched >= 0.2.23; not yanked.
  sed -i.bak 's/^serde_json = "=1.0.154"$/serde_json = "=1.0.154"\ntime = "=0.1.45"/' crates/fake-claude/Cargo.toml
  rm -f crates/fake-claude/Cargo.toml.bak
  cargo update -p time@0.1.45 2>/dev/null || cargo metadata --format-version 1 >/dev/null
  expect_fail "known-vulnerable Rust dependency (cargo audit)" fails_with "RUSTSEC-2020-0071" node scripts/verify.mjs --only=cargo-audit
}

plant_postinstall() {
  backup package.json pnpm-lock.yaml
  # esbuild 0.25.0 has a postinstall script and isn't in allowBuilds: strictDepBuilds must fail it.
  expect_fail "dependency with postinstall (pnpm install, strictDepBuilds)" fails_with "ERR_PNPM_IGNORED_BUILDS" pnpm add -D esbuild@0.25.0
}

plant_deny_crate() { # crate version reason
  backup crates/fake-claude/Cargo.toml Cargo.lock
  sed -i.bak "s/^serde_json = \"=1.0.154\"$/serde_json = \"=1.0.154\"\n$1 = \"=$2\"/" crates/fake-claude/Cargo.toml
  rm -f crates/fake-claude/Cargo.toml.bak
  cargo metadata --format-version 1 >/dev/null
  expect_fail "$1 dependency (cargo deny bans, $3)" fails_with "banned|explicitly banned" node scripts/verify.mjs --only=cargo-deny
}

plant_git_npm() {
  CREATED+=("scripts/plants/__plant-lock.yaml")
  cp pnpm-lock.yaml scripts/plants/__plant-lock.yaml
  cat >> scripts/plants/__plant-lock.yaml <<'LOCK'

  left-pad@git+https://github.com/left-pad/left-pad.git#5e8c1d1:
    resolution: {type: git, repo: https://github.com/left-pad/left-pad.git, commit: 5e8c1d1}
LOCK
  expect_fail "git-sourced npm package (pnpm-policy lockfile scan)" fails_with "git source" node scripts/check-lockfile-sources.mjs scripts/plants/__plant-lock.yaml
}

which=("$@")
[ ${#which[@]} -eq 0 ] && which=(type secret clippy vuln-js vuln-rust postinstall reqwest keyring git-npm)
for p in "${which[@]}"; do
  case "$p" in
    type) plant_type ;;
    secret) plant_secret ;;
    clippy) plant_clippy ;;
    vuln-js) plant_vuln_js ;;
    vuln-rust) plant_vuln_rust ;;
    postinstall) plant_postinstall ;;
    reqwest) plant_deny_crate reqwest 0.13.5 "SR-60" ;;
    keyring) plant_deny_crate keyring 4.2.0 "SR-51" ;;
    git-npm) plant_git_npm ;;
    *) echo "unknown plant: $p" >&2; exit 2 ;;
  esac
done

echo; printf '%s\n' "${results[@]}"
if printf '%s\n' "${results[@]}" | grep -q '^MISSED'; then exit 1; fi
echo "All plants were caught and reverted."
