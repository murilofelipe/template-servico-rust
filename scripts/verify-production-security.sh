#!/usr/bin/env bash
# ==============================================================================
# verify-production-security.sh
# Verifies that in production mode (ENVIRONMENT=production), CORS allows only
# configured origins in ALLOWED_ORIGINS, blocks unauthorized origins, and
# injects all recommended security headers on all HTTP responses.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

find_available_port() {
    python3 -c 'import socket; s=socket.socket(); s.bind(("", 0)); print(s.getsockname()[1]); s.close()' 2>/dev/null || echo "3993"
}

PORT="${PORT:-$(find_available_port)}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/template_servico_rust_test}"
LOG_FILE="$(mktemp /tmp/template_srv_prod_sec_XXXXXX.log)"

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
echo "🔒 [PROD SECURITY VERIFICATION] Iniciando validação em Produção..."
echo "================================================================="

cd "${ROOT_DIR}"

# Build binary
echo "📦 Compilando binário do serviço..."
cargo build --bin template-servico-rust

# Run the server in production mode with dynamic ALLOWED_ORIGINS
ALLOWED_ORIGINS="https://app.example.com,https://dashboard.example.com"
echo "🚀 Iniciando servidor com ENVIRONMENT=production na porta ${PORT}..."
echo "   ALLOWED_ORIGINS=\"${ALLOWED_ORIGINS}\""

ENVIRONMENT="production" \
PORT="${PORT}" \
HOST="127.0.0.1" \
DATABASE_URL="${DATABASE_URL}" \
ALLOWED_ORIGINS="${ALLOWED_ORIGINS}" \
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
# 1. Test CORS: Allowed Origin (https://app.example.com)
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [1/5] Testando CORS com Origem Autorizada (https://app.example.com)..."
RESP_HEADERS=$(curl -s -i -H "Origin: https://app.example.com" "http://127.0.0.1:${PORT}/health")

if echo "${RESP_HEADERS}" | grep -qi "access-control-allow-origin: https://app.example.com"; then
    echo "   ✅ Sucesso: Header 'access-control-allow-origin: https://app.example.com' retornado corretamente."
else
    echo "   ❌ FALHA: Header 'access-control-allow-origin' não encontrado para origem autorizada."
    echo "${RESP_HEADERS}"
    exit 1
fi

# ------------------------------------------------------------------------------
# 2. Test CORS: Second Allowed Origin (https://dashboard.example.com)
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [2/5] Testando CORS com Segunda Origem Autorizada (https://dashboard.example.com)..."
RESP_HEADERS_2=$(curl -s -i -H "Origin: https://dashboard.example.com" "http://127.0.0.1:${PORT}/health")

if echo "${RESP_HEADERS_2}" | grep -qi "access-control-allow-origin: https://dashboard.example.com"; then
    echo "   ✅ Sucesso: Header 'access-control-allow-origin: https://dashboard.example.com' retornado corretamente."
else
    echo "   ❌ FALHA: Header 'access-control-allow-origin' não encontrado para https://dashboard.example.com."
    echo "${RESP_HEADERS_2}"
    exit 1
fi

# ------------------------------------------------------------------------------
# 3. Test CORS: Unauthorized Origin (https://malicious.evil.com) -> MUST BE BLOCKED
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [3/5] Testando CORS com Origem Não Autorizada (https://malicious.evil.com)..."
UNAUTH_HEADERS=$(curl -s -i -H "Origin: https://malicious.evil.com" "http://127.0.0.1:${PORT}/health")

if echo "${UNAUTH_HEADERS}" | grep -qi "access-control-allow-origin"; then
    echo "   ❌ FALHA DE SEGURANÇA: Origem não autorizada recebeu 'access-control-allow-origin'!"
    echo "${UNAUTH_HEADERS}"
    exit 1
else
    echo "   ✅ Sucesso: Origem não autorizada bloqueada (nenhum header 'access-control-allow-origin' retornado)."
fi

# ------------------------------------------------------------------------------
# 4. Test CORS: Preflight OPTIONS Request
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [4/5] Testando Preflight OPTIONS request para CORS..."
PREFLIGHT_ALLOWED=$(curl -s -i -X OPTIONS "http://127.0.0.1:${PORT}/users" \
    -H "Origin: https://app.example.com" \
    -H "Access-Control-Request-Method: POST" \
    -H "Access-Control-Request-Headers: content-type,authorization")

if echo "${PREFLIGHT_ALLOWED}" | grep -qi "access-control-allow-origin: https://app.example.com"; then
    echo "   ✅ Sucesso: Preflight permitido para origem autorizada."
else
    echo "   ❌ FALHA: Preflight não retornou allow-origin para origem autorizada."
    echo "${PREFLIGHT_ALLOWED}"
    exit 1
fi

PREFLIGHT_DENIED=$(curl -s -i -X OPTIONS "http://127.0.0.1:${PORT}/users" \
    -H "Origin: https://malicious.evil.com" \
    -H "Access-Control-Request-Method: POST")

if echo "${PREFLIGHT_DENIED}" | grep -qi "access-control-allow-origin"; then
    echo "   ❌ FALHA DE SEGURANÇA: Preflight não autorizado recebeu allow-origin!"
    echo "${PREFLIGHT_DENIED}"
    exit 1
else
    echo "   ✅ Sucesso: Preflight bloqueado para origem não autorizada."
fi

# ------------------------------------------------------------------------------
# 5. Test Security Headers
# ------------------------------------------------------------------------------
echo ""
echo "▶️  [5/5] Validando presença de Security Headers recomendados na resposta..."

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

HEALTH_HEADERS=$(curl -s -i "http://127.0.0.1:${PORT}/health")
check_header "x-content-type-options" "nosniff" "${HEALTH_HEADERS}" "/health"
check_header "x-frame-options" "DENY" "${HEALTH_HEADERS}" "/health"
check_header "strict-transport-security" "max-age=31536000" "${HEALTH_HEADERS}" "/health"
check_header "content-security-policy" "default-src 'self'" "${HEALTH_HEADERS}" "/health"
check_header "referrer-policy" "strict-origin-when-cross-origin" "${HEALTH_HEADERS}" "/health"
check_header "x-xss-protection" "0" "${HEALTH_HEADERS}" "/health"
check_header "permissions-policy" "geolocation=\(\)" "${HEALTH_HEADERS}" "/health"

# Check on 404 response
NOT_FOUND_HEADERS=$(curl -s -i "http://127.0.0.1:${PORT}/non-existent-endpoint-404")
check_header "x-content-type-options" "nosniff" "${NOT_FOUND_HEADERS}" "/404"
check_header "x-frame-options" "DENY" "${NOT_FOUND_HEADERS}" "/404"

echo ""
echo "================================================================="
echo "🎉 VALIDAÇÃO DE SEGURANÇA EM PRODUÇÃO CONCLUÍDA COM SUCESSO!"
echo "   - ✅ CORS dinâmico: origens autorizadas permitidas"
echo "   - ✅ CORS dinâmico: origens não autorizadas bloqueadas"
echo "   - ✅ Preflight OPTIONS validado"
echo "   - ✅ Todos os Security Headers verificados"
echo "================================================================="
