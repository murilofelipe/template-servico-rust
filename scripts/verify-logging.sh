#!/usr/bin/env bash
# ==============================================================================
# verify-logging.sh
# Comprehensive verification suite for Structured Logging (Issue #9).
# Runs both Production (JSON) and Development (Pretty) verification scripts.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "================================================================="
echo "🚀 INICIANDO BATERIA DE TESTES DE LOGGING ESTATÍSTICO/DINÂMICO"
echo "================================================================="

cd "${ROOT_DIR}"

# 1. Executa verificação em modo Produção
echo ""
echo "▶️  [1/2] Executando verificação de logs em Produção (JSON)..."
bash "${SCRIPT_DIR}/verify-production-logs.sh"

# 2. Executa verificação em modo Desenvolvimento
echo ""
echo "▶️  [2/2] Executando verificação de logs em Desenvolvimento (Pretty Text)..."
bash "${SCRIPT_DIR}/verify-development-logs.sh"

echo ""
echo "================================================================="
echo "🎉 TODOS OS TESTES DE LOGGING PASSARAM COM SUCESSO!"
echo "   - ✅ Modo Produção: Logs emitidos em JSON estruturado válido"
echo "   - ✅ Modo Desenvolvimento: Logs emitidos em texto legível (pretty format)"
echo "   - ✅ Middleware Tower-HTTP: Requisições e Respostas registradas automaticamente"
echo "================================================================="
