#!/usr/bin/env bash
# ==============================================================================
# verify-development-security.sh
# Verifies that in development mode (ENVIRONMENT=development), CORS is
# permissive (allowing any origin) while maintaining security headers.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

find_available_port() {
    python3 -c 'import socket; s=socket.socket(); s.bind(("", 0)); print(s.getsockname()[1]); s.close()' 2>/dev/null || echo "3994"
}

PORT="${PORT:-$(find_available_port)}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/template_servico_rust_test}"
LOG_FILE="$(mktemp /tmp/template_srv_dev_sec_XXXXXX.log)"

cleanup() {
    if [[ -n "${SERVER_PID:-}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
        kill "${SERVER_PID}" 2>/dev/null || true
        sleep 0.2
        if kill -0 "${SERVER_PID}" 2>/dev/null; then
            kill -9 "${SERVER_PID}" 2>/dev/null || true
        fi
        wait "${SERVER_PID}" 2>/dev/null || true
    fi
    rm -f "${LOG_FILE:-}"
}
trap cleanup EXIT INT TERM

echo "================================================================="
echo "🛠️  [DEV SECURITY VERIFICATION] Iniciando validação em Desenvolvimento..."
echo "================================================================="

cd "${ROOT_DIR}"

# Run the server in development mode
echo "🚀 Iniciando servidor com ENVIRONMENT=development na porta ${PORT}..."

ENVIRONMENT="development" \
PORT="${PORT}" \
HOST="127.0.0.1" \
DATABASE_URL="${DATABASE_URL}" \
RUST_LOG="info,tower_http=debug" \
./target/debug/template-servico-rust > "${LOG_FILE}" 2>&1 &
SERVER_PID=$!

# Wait for server to start
echo "⏳ Aguardando inicialização do servidor..."
SERVER_READY=0
for i in {1..30}; do
    if curl -s -f "http://127.0.0.1:${PORT}/health" >/dev/null 2>&1; then
        SERVER_READY=1
        break
    fi
    if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
        echo "❌ Erro: Servidor encerrou prematuramente."
        cat "${LOG_FILE}"
        exit 1
    fi
    sleep 0.5
done

if [[ "${SERVER_READY}" -ne 1 ]]; then
    echo "❌ Timeout aguardando inicialização do servidor."
    cat "${LOG_FILE}"
    exit 1
fi

echo "✅ Servidor pronto na porta ${PORT}."

# ------------------------------------------------------------------------------
# 1. Test CORS: Permissive origin (http://localhost:3000)
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [1/3] Testando CORS Permissivo com http://localhost:3000..."
RESP_HEADERS_1=$(curl -s -i -H "Origin: http://localhost:3000" "http://127.0.0.1:${PORT}/health")

if echo "${RESP_HEADERS_1}" | grep -qi "access-control-allow-origin"; then
    echo "   ✅ Sucesso: Header 'access-control-allow-origin' retornado em desenvolvimento."
else
    echo "   ❌ FALHA: Header 'access-control-allow-origin' ausente em ambiente de desenvolvimento."
    echo "${RESP_HEADERS_1}"
    exit 1
fi

# ------------------------------------------------------------------------------
# 2. Test CORS: Permissive origin with external domain (https://any-dev-client.local)
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [2/3] Testando CORS Permissivo com https://any-dev-client.local..."
RESP_HEADERS_2=$(curl -s -i -H "Origin: https://any-dev-client.local" "http://127.0.0.1:${PORT}/health")

if echo "${RESP_HEADERS_2}" | grep -qi "access-control-allow-origin"; then
    echo "   ✅ Sucesso: Header 'access-control-allow-origin' retornado para cliente arbitrário em desenvolvimento."
else
    echo "   ❌ FALHA: Header 'access-control-allow-origin' ausente para cliente em desenvolvimento."
    echo "${RESP_HEADERS_2}"
    exit 1
fi

# ------------------------------------------------------------------------------
# 3. Test Security Headers in Development
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [3/3] Validando presença de Security Headers no modo Desenvolvimento..."

check_header() {
    local header_name="$1"
    local expected_pattern="$2"
    local headers="$3"
    local endpoint="$4"

    if echo "${headers}" | grep -qi "^${header_name}:.*${expected_pattern}"; then
        echo "   ✅ [${endpoint}] ${header_name} presente e válido."
    else
        echo "   ❌ [${endpoint}] FALHA: Header '${header_name}' esperado não encontrado ou inválido."
        echo "Headers recebidos:"
        echo "${headers}"
        exit 1
    fi
}

DEV_HEADERS=$(curl -s -i "http://127.0.0.1:${PORT}/health")
check_header "x-content-type-options" "nosniff" "${DEV_HEADERS}" "/health"
check_header "x-frame-options" "DENY" "${DEV_HEADERS}" "/health"
check_header "strict-transport-security" "max-age=31536000" "${DEV_HEADERS}" "/health"
check_header "content-security-policy" "default-src 'self'" "${DEV_HEADERS}" "/health"
check_header "referrer-policy" "strict-origin-when-cross-origin" "${DEV_HEADERS}" "/health"
check_header "x-xss-protection" "0" "${DEV_HEADERS}" "/health"

echo ""
echo "================================================================="
echo "🎉 VALIDAÇÃO DE SEGURANÇA EM DESENVOLVIMENTO CONCLUÍDA COM SUCESSO!"
echo "   - ✅ CORS permissivo funcionando para múltiplas origens"
echo "   - ✅ Security Headers preservados"
echo "================================================================="
