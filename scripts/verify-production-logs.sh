#!/usr/bin/env bash
# ==============================================================================
# verify-production-logs.sh
# Verifies that in production mode (ENVIRONMENT=production), logs emitted
# to stdout/stderr are valid structured JSON and capture HTTP requests/responses.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

find_available_port() {
    python3 -c 'import socket; s=socket.socket(); s.bind(("", 0)); print(s.getsockname()[1]); s.close()' 2>/dev/null || echo "3991"
}

PORT="${PORT:-$(find_available_port)}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/template_servico_rust_test}"
LOG_FILE="$(mktemp /tmp/template_srv_prod_logs_XXXXXX.log)"

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
echo "🔍 [PROD VERIFICATION] Iniciando validação de logs em Produção..."
echo "================================================================="

cd "${ROOT_DIR}"

# Always ensure binary is up to date
echo "📦 Compilando binário do serviço..."
cargo build --bin template-servico-rust

# Run the server in production mode
echo "🚀 Iniciando servidor com ENVIRONMENT=production na porta ${PORT}..."
ENVIRONMENT="production" \
PORT="${PORT}" \
HOST="127.0.0.1" \
DATABASE_URL="${DATABASE_URL}" \
RUST_LOG="info,template_servico_rust=debug,tower_http=info" \
./target/debug/template-servico-rust > "${LOG_FILE}" 2>&1 &
SERVER_PID=$!

# Wait for the server to become healthy
echo "⏳ Aguardando servidor iniciar..."
SERVER_READY=0
for i in {1..30}; do
    if curl -s -f "http://127.0.0.1:${PORT}/health" >/dev/null 2>&1; then
        SERVER_READY=1
        break
    fi
    if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
        echo "❌ Erro: O processo do servidor encerrou prematuramente."
        cat "${LOG_FILE}"
        exit 1
    fi
    sleep 0.5
done

if [[ "${SERVER_READY}" -ne 1 ]]; then
    echo "❌ Timeout aguardando servidor responder na porta ${PORT}."
    cat "${LOG_FILE}"
    exit 1
fi

echo "📡 Enviando requisições HTTP para disparar logs de request/response..."
curl -s "http://127.0.0.1:${PORT}/health" >/dev/null
curl -s "http://127.0.0.1:${PORT}/users" >/dev/null
sleep 0.5

# Stop server gracefully
kill "${SERVER_PID}" 2>/dev/null || true
wait "${SERVER_PID}" 2>/dev/null || true
unset SERVER_PID

echo "📄 Analisando formato dos logs emitidos:"
echo "-----------------------------------------------------------------"
cat "${LOG_FILE}"
echo "-----------------------------------------------------------------"

# Validate that all non-empty lines are valid JSON
echo "🔬 Validando se todas as linhas de log são JSON válido..."
python3 - <<EOF
import sys
import json

log_file = "${LOG_FILE}"
total_lines = 0
json_lines = 0
found_request_log = False
found_response_log = False

with open(log_file, "r", encoding="utf-8") as f:
    for line_num, line in enumerate(f, 1):
        stripped = line.strip()
        if not stripped:
            continue
        total_lines += 1
        try:
            parsed = json.loads(stripped)
            json_lines += 1
            
            # Verify standard structured log fields exist
            for required_field in ("timestamp", "level", "target", "fields"):
                if required_field not in parsed:
                    print(f"❌ Linha {line_num} não contém o campo obrigatório '{required_field}': {stripped}")
                    sys.exit(1)

            # Check request/response tracking
            fields = parsed.get("fields", {})
            msg = fields.get("message", "")
            if "started processing request" in msg or "request" in str(parsed):
                found_request_log = True
            if "finished processing request" in msg or "status" in str(fields):
                found_response_log = True
        except json.JSONDecodeError as e:
            print(f"❌ Linha {line_num} NÃO é um JSON válido: {stripped}")
            print(f"   Erro de parse: {e}")
            sys.exit(1)

if total_lines == 0:
    print("❌ Nenhum log foi emitido!")
    sys.exit(1)

print(f"✅ Total de {json_lines}/{total_lines} linhas validadas com sucesso como JSON estruturado!")
if not found_request_log:
    print("❌ Erro: Log de requisição HTTP (on_request) não encontrado nos logs JSON!")
    sys.exit(1)
if not found_response_log:
    print("❌ Erro: Log de resposta HTTP (on_response) não encontrado nos logs JSON!")
    sys.exit(1)
print("✅ Logs de request e response confirmados no JSON!")
EOF

echo "================================================================="
echo "✅ [PROD VERIFICATION] Validação concluída com SUCESSO!"
echo "================================================================="
