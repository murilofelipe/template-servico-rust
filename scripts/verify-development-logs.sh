#!/usr/bin/env bash
# ==============================================================================
# verify-development-logs.sh
# Verifies that in development mode (ENVIRONMENT=development), logs emitted
# to stdout/stderr are formatted as human-readable text (pretty format, not JSON).
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

find_available_port() {
    python3 -c 'import socket; s=socket.socket(); s.bind(("", 0)); print(s.getsockname()[1]); s.close()' 2>/dev/null || echo "3992"
}

PORT="${PORT:-$(find_available_port)}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/template_servico_rust_test}"
LOG_FILE="$(mktemp /tmp/template_srv_dev_logs_XXXXXX.log)"

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
echo "🔍 [DEV VERIFICATION] Iniciando validação de logs em Desenvolvimento..."
echo "================================================================="

cd "${ROOT_DIR}"

# Always ensure binary is up to date
echo "📦 Compilando binário do serviço..."
cargo build --bin template-servico-rust

# Run the server in development mode
echo "🚀 Iniciando servidor com ENVIRONMENT=development na porta ${PORT}..."
ENVIRONMENT="development" \
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

echo "📡 Enviando requisição HTTP para disparar logs..."
curl -s "http://127.0.0.1:${PORT}/health" >/dev/null
sleep 0.5

# Stop server gracefully
kill "${SERVER_PID}" 2>/dev/null || true
wait "${SERVER_PID}" 2>/dev/null || true
unset SERVER_PID

echo "📄 Analisando formato dos logs emitidos:"
echo "-----------------------------------------------------------------"
cat "${LOG_FILE}"
echo "-----------------------------------------------------------------"

# Validate that logs are human-readable text and NOT JSON
echo "🔬 Validando se os logs estão em formato legível (pretty/text, não JSON)..."
python3 - <<EOF
import sys
import json

log_file = "${LOG_FILE}"
total_lines = 0
json_valid_lines = 0
found_request_log = False
found_response_log = False

with open(log_file, "r", encoding="utf-8") as f:
    content = f.read()

lines = [l.strip() for l in content.splitlines() if l.strip()]
if not lines:
    print("❌ Nenhum log foi emitido!")
    sys.exit(1)

for line in lines:
    total_lines += 1
    if "started processing request" in line or "GET /health" in line or "HTTP request" in line:
        found_request_log = True
    if "finished processing request" in line or "status: 200" in line or "status=200" in line or "status:" in line:
        found_response_log = True
    try:
        json.loads(line)
        json_valid_lines += 1
    except json.JSONDecodeError:
        pass

# In development mode, logs must NOT be raw single-line JSON objects
if json_valid_lines == total_lines and total_lines > 0:
    print("❌ Erro: Todos os logs foram emitidos como JSON bruto em modo de desenvolvimento!")
    sys.exit(1)

if not found_request_log:
    print("❌ Erro: Log de requisição HTTP não encontrado nos logs de desenvolvimento!")
    sys.exit(1)

if not found_response_log:
    print("❌ Erro: Log de resposta HTTP não encontrado nos logs de desenvolvimento!")
    sys.exit(1)

print(f"✅ Logs validados com sucesso como texto legível (não-JSON: {total_lines - json_valid_lines}/{total_lines} linhas)!")
print("✅ Logs de request e response confirmados no formato texto legível!")
EOF

echo "================================================================="
echo "✅ [DEV VERIFICATION] Validação concluída com SUCESSO!"
echo "================================================================="
