#!/usr/bin/env bash
# ==============================================================================
# verify-security.sh
# Master verification suite for Basic Security (CORS and Security Headers) - Issue #10.
# Runs both Production and Development security verification scripts.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "================================================================="
echo "🛡️  INICIANDO BATERIA DE TESTES DE SEGURANÇA BÁSICA (CORS & HEADERS)"
echo "================================================================="

cd "${ROOT_DIR}"

# 1. Production security checks
echo ""
echo "▶️  [1/2] Executando verificação de segurança em Produção..."
bash "${SCRIPT_DIR}/verify-production-security.sh"

# 2. Development security checks
echo ""
echo "▶️  [2/2] Executando verificação de segurança em Desenvolvimento..."
bash "${SCRIPT_DIR}/verify-development-security.sh"

echo ""
echo "================================================================="
echo "🎉 TODOS OS TESTES DE SEGURANÇA PASSARAM COM SUCESSO!"
echo "   - ✅ Modo Produção: CORS dinâmico restringe origens conforme ALLOWED_ORIGINS"
echo "   - ✅ Modo Produção: Origens não autorizadas bloqueadas com segurança"
echo "   - ✅ Modo Desenvolvimento: CORS permissivo para agilidade no desenvolvimento"
echo "   - ✅ Security Headers: Injetados em todas as respostas HTTP (X-Content-Type-Options, X-Frame-Options, HSTS, CSP, etc.)"
echo "================================================================="
