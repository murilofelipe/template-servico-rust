#!/usr/bin/env bash
#
# version-check.sh — valida a política de versionamento do Template Rust.
# Adaptado do projeto sition-web para o ecossistema Rust (Cargo.toml).
#
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

PKG="Cargo.toml"
CHANGELOG="CHANGELOG.md"

fail() { printf '  \033[31m✗ %s\033[0m\n' "$1" >&2; FAILED=1; }
ok()   { printf '  \033[32m✓ %s\033[0m\n' "$1"; }
info() { printf '  \033[33m• %s\033[0m\n' "$1"; }
FAILED=0

read_pkg_version() {
  if [ -z "${1:-}" ]; then
    grep -E '^version = ' "$PKG" | head -n1 | sed -E 's/version = "(.*)"/\1/'
  else
    git show "$1:$PKG" 2>/dev/null | grep -E '^version = ' | head -n1 | sed -E 's/version = "(.*)"/\1/' || echo ""
  fi
}

semver_gte() { [ "$1" = "$2" ] || [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -1)" = "$1" ]; }

apply_bump() {
  local v="${1%%-*}"; local kind="$2"
  IFS=. read -r M m p <<<"$v"
  case "$kind" in
    major) echo "$((M+1)).0.0" ;;
    minor) echo "$M.$((m+1)).0" ;;
    patch) echo "$M.$m.$((p+1))" ;;
  esac
}

BASE_REF="${1:-}"
if [ -z "$BASE_REF" ]; then
  BASE_REF="$(git rev-parse --verify -q origin/main >/dev/null && echo origin/main || echo main)"
fi
shift || true
BRANCHES=("$@")
[ ${#BRANCHES[@]} -eq 0 ] && BRANCHES=("$(git rev-parse --abbrev-ref HEAD)")

PR_VERSION="$(read_pkg_version '')"
BASE_VERSION="$(read_pkg_version "$BASE_REF" || true)"
[ -z "${BASE_VERSION:-}" ] && BASE_VERSION="0.0.0"

echo "Política de versionamento — base=$BASE_REF"
echo "  versão base=$BASE_VERSION · versão do PR=$PR_VERSION"
echo

# 1) Sem regressão
if [ "$BASE_VERSION" = "0.0.0" ]; then
  info "1) Sem versão base legível em $BASE_REF."
elif [ "$PR_VERSION" = "$BASE_VERSION" ]; then
  ok "1) Versão inalterada em relação à base ($BASE_VERSION)."
elif semver_gte "$PR_VERSION" "$BASE_VERSION"; then
  ok "1) Versão avançou: $BASE_VERSION → $PR_VERSION"
else
  fail "1) Regressão de versão: PR=$PR_VERSION é menor que base=$BASE_VERSION"
fi

# 2) Bloqueio de merges indevidos na main e coerência de release
REL_CHECKED=0
if [[ "$BASE_REF" == *"main"* ]]; then
  is_release=0
  for b in "${BRANCHES[@]}"; do
    if [[ "$b" == *release/* ]]; then
      is_release=1
      break
    fi
  done
  if [ "$is_release" -eq 0 ]; then
    fail "PRs para a branch main SÓ PODEM originar de branches 'release/*'. Respeite o Gitflow."
  fi
fi

for b in "${BRANCHES[@]}"; do
  case "$b" in
    *release/*)
      expected="${b##*release/}"
      REL_CHECKED=1
      if [ "$expected" = "$PR_VERSION" ]; then
        ok "2) Nome '$b' coerente com a versão $PR_VERSION"
      else
        fail "2) '$b' exige versão $expected, mas Cargo.toml está em $PR_VERSION"
      fi
      ;;
  esac
done
[ "$REL_CHECKED" -eq 0 ] && info "2) Nenhuma branch release/X.Y.Z envolvida na checagem de nome."

# 4) Incremento correto
LAST_TAG="$(git describe --tags --abbrev=0 2>/dev/null || true)"
if [ -z "$LAST_TAG" ]; then
  info "4) Nenhuma tag anterior — checagem de incremento pulada."
else
  RANGE="${LAST_TAG}..HEAD"
  subjects="$(git log --format='%s%n%b' "$RANGE" 2>/dev/null || true)"
  biggest="none"
  if grep -qE '^[a-z]+(\(.+\))?!:|BREAKING CHANGE' <<<"$subjects"; then
    biggest="major"
  elif grep -qE '^feat(\(.+\))?:' <<<"$subjects"; then
    biggest="minor"
  elif grep -qE '^fix(\(.+\))?:' <<<"$subjects"; then
    biggest="patch"
  fi
  tag_version="${LAST_TAG#v}"
  if [ "$biggest" = "none" ]; then
    info "4) Nenhum feat/fix/breaking desde $LAST_TAG — sem bump exigido."
  else
    expected_version="$(apply_bump "$tag_version" "$biggest")"
    if [ "${PR_VERSION%%-*}" = "$expected_version" ]; then
      ok "4) Incremento $biggest correto: $tag_version → $PR_VERSION"
    else
      fail "4) Commits pedem bump $biggest (→ $expected_version), mas a versão é $PR_VERSION"
    fi
  fi
fi

# 5) CHANGELOG
if [ ! -f "$CHANGELOG" ]; then
  fail "5) $CHANGELOG não existe"
elif grep -qE "^## \[${PR_VERSION}\]" "$CHANGELOG"; then
  ok "5) $CHANGELOG contém entrada '## [$PR_VERSION]'"
else
  fail "5) $CHANGELOG sem entrada '## [$PR_VERSION]'"
fi

echo
if [ "$FAILED" -ne 0 ]; then
  echo "Política de versionamento: FALHOU" >&2
  exit 1
fi
echo "Política de versionamento: OK"
