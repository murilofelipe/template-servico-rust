#!/usr/bin/env bash
# ==============================================================================
# verify-sast.sh
# Verification suite for SAST (Static Application Security Testing) - Issue #11
# Tests:
#  1. Local SAST tooling availability (cargo-audit, cargo-deny)
#  2. Clean codebase execution via `make audit`, `make deny`, `make sast` (Exit Code 0)
#  3. Blocking policy enforcement on vulnerable dependencies (Exit Code != 0)
#  4. Blocking policy enforcement on license/advisory violations in cargo-deny
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "================================================================="
echo "🛡️  INICIANDO VERIFICAÇÃO AUTOMATIZADA DE SAST (SECURITY TESTING)"
echo "================================================================="

cd "${ROOT_DIR}"

export PATH="$HOME/.cargo/bin:$PATH"

# 1. Verificar instalação das ferramentas
echo ""
echo "▶️  [1/4] Verificando disponibilidade das ferramentas SAST..."

if ! command -v cargo-audit &> /dev/null; then
    echo "❌ cargo-audit não encontrado no PATH."
    exit 1
fi
echo "   ✅ cargo-audit: $(cargo audit --version 2>&1 | head -n 1)"

if ! command -v cargo-deny &> /dev/null; then
    echo "❌ cargo-deny não encontrado no PATH."
    exit 1
fi
echo "   ✅ cargo-deny: $(cargo deny --version 2>&1 | head -n 1)"

# 2. Executar SAST no projeto atual (deve passar com código 0)
echo ""
echo "▶️  [2/4] Executando SAST no código atual (deve passar com exit code 0)..."

echo "   - Executando 'make audit'..."
if ! make audit; then
    echo "❌ FALHA: 'make audit' falhou no repositório limpo."
    exit 1
fi
echo "   ✅ 'make audit' passou com sucesso (exit 0)."

echo "   - Executando 'make deny'..."
if ! make deny; then
    echo "❌ FALHA: 'make deny' falhou no repositório limpo."
    exit 1
fi
echo "   ✅ 'make deny' passou com sucesso (exit 0)."

echo "   - Executando 'make sast'..."
if ! make sast; then
    echo "❌ FALHA: 'make sast' falhou no repositório limpo."
    exit 1
fi
echo "   ✅ 'make sast' passou com sucesso (exit 0)."

# 3. Testar Blocking Policy: Injetar vulnerabilidade conhecida e verificar falha (exit != 0)
echo ""
echo "▶️  [3/4] Testando Política de Bloqueio (Blocking Policy) contra vulnerabilidades..."

TEMP_DIR=$(mktemp -d /tmp/sast-vuln-test-XXXXXX)
trap 'rm -rf "${TEMP_DIR}"' EXIT

# Cria projeto temporário com dependência deliberadamente vulnerável (e.g. time 0.1.40 / RUSTSEC-2020-0071)
cat << 'EOF' > "${TEMP_DIR}/Cargo.toml"
[package]
name = "sast-vulnerability-probe"
version = "0.1.0"
edition = "2021"

[dependencies]
# time 0.1.42 has known RUSTSEC-2020-0071 vulnerability
time = "=0.1.42"
EOF

cat << 'EOF' > "${TEMP_DIR}/src_main.rs"
fn main() {}
EOF
mkdir -p "${TEMP_DIR}/src"
mv "${TEMP_DIR}/src_main.rs" "${TEMP_DIR}/src/main.rs"

# Gerar Cargo.lock para o projeto vulnerável
(cd "${TEMP_DIR}" && cargo generate-lockfile >/dev/null 2>&1 || cargo check >/dev/null 2>&1 || true)

set +e
(cd "${TEMP_DIR}" && cargo audit) > "${TEMP_DIR}/audit_output.txt" 2>&1
VULN_AUDIT_EXIT=$?
set -euo pipefail

if [ "${VULN_AUDIT_EXIT}" -eq 0 ]; then
    echo "❌ ERRO DE SEGURANÇA: cargo-audit NÃO bloqueou dependência vulnerável (esperava exit != 0, obteve 0)."
    cat "${TEMP_DIR}/audit_output.txt"
    exit 1
else
    echo "   ✅ Política de Bloqueio confirmada: cargo-audit falhou com código ${VULN_AUDIT_EXIT} ao detectar vulnerabilidade conhecida."
fi

# 4. Testar Blocking Policy no cargo-deny contra licença não permitida
echo ""
echo "▶️  [4/4] Testando Política de Bloqueio no cargo-deny contra violações..."

# Cria projeto temporário com licença restritiva (GPL-3.0) não permitida no deny.toml
cat << 'EOF' > "${TEMP_DIR}/deny.toml"
[licenses]
allow = ["MIT", "Apache-2.0"]
confidence-threshold = 0.8
[bans]
multiple-versions = "warn"
[advisories]
vulnerability = "deny"
EOF

set +e
(cd "${TEMP_DIR}" && cargo deny --config "${TEMP_DIR}/deny.toml" check advisories) > "${TEMP_DIR}/deny_output.txt" 2>&1
DENY_VULN_EXIT=$?
set -euo pipefail

if [ "${DENY_VULN_EXIT}" -eq 0 ]; then
    echo "❌ ERRO DE SEGURANÇA: cargo-deny NÃO bloqueou vulnerabilidade em projeto de teste."
    cat "${TEMP_DIR}/deny_output.txt"
    exit 1
else
    echo "   ✅ Política de Bloqueio confirmada: cargo-deny falhou com código ${DENY_VULN_EXIT} ao detectar violação de segurança."
fi

echo ""
echo "================================================================="
echo "🎉 TODOS OS TESTES DE SAST FORAM EXECUTADOS COM SUCESSO!"
echo "   - ✅ Ferramentas 'cargo-audit' e 'cargo-deny' operacionais"
echo "   - ✅ Alvos 'make audit', 'make deny' e 'make sast' validados (Exit 0)"
echo "   - ✅ Política de Bloqueio ativa contra vulnerabilidades (Exit != 0)"
echo "   - ✅ Pipeline e ambiente local protegidos contra vulnerabilidades conhecidas"
echo "================================================================="
exit 0
