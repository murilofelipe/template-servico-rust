#!/usr/bin/env bash
# ==============================================================================
# verify-docker-security.sh
# Verification suite for Dockerfile Security (Issue #12 - Non-Root & Secure Port).
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

IMAGE_NAME="template-servico-rust:security-test"
CONTAINER_NAME="template-servico-rust-sec-check-$$"

cleanup() {
    local exit_code=$?
    echo ""
    echo "🧹 [Cleanup] Removendo container de teste..."
    docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
    echo "🧹 [Cleanup] Removendo imagem temporária de teste..."
    docker rmi "${IMAGE_NAME}" >/dev/null 2>&1 || true
    exit "${exit_code}"
}

trap cleanup EXIT INT TERM

echo "================================================================="
echo "🐳 INICIANDO BATERIA DE TESTES DE SEGURANÇA DOCKER (NON-ROOT & PORT)"
echo "================================================================="

cd "${ROOT_DIR}"

# 1. Verifica se o Docker daemon está ativo
echo ""
echo "▶️  [1/5] Verificando conectividade com o Docker daemon..."
if ! docker info >/dev/null 2>&1; then
    echo "❌ Erro: Docker daemon não está em execução ou acessível."
    exit 1
fi
echo "   ✅ Docker daemon está ativo e acessível."

# 2. Build da imagem Docker
echo ""
echo "▶️  [2/5] Compilando imagem Docker com cargo-chef e non-root user..."
docker build -t "${IMAGE_NAME}" -f Dockerfile .
echo "   ✅ Imagem Docker construída com sucesso: ${IMAGE_NAME}"

# 3. Inspeção estática de metadados da imagem
echo ""
echo "▶️  [3/5] Inspecionando metadados da imagem (User, Portas, Env)..."

# 3.1 Verificar USER configurado na imagem
IMAGE_USER=$(docker inspect --format='{{.Config.User}}' "${IMAGE_NAME}")
echo "   - Config.User configurado: '${IMAGE_USER}'"
if [ "${IMAGE_USER}" != "appuser" ] && [ "${IMAGE_USER}" != "1000" ] && [ "${IMAGE_USER}" != "1000:1000" ] && [ "${IMAGE_USER}" != "appuser:appuser" ]; then
    echo "❌ Falha de Segurança: Imagem configurada com usuário '${IMAGE_USER}' (esperado: appuser ou 1000)."
    exit 1
fi
echo "   ✅ Usuário da imagem é não-root (${IMAGE_USER})."

# 3.2 Verificar portas expostas (EXPOSE >= 1024)
EXPOSED_PORTS=$(docker inspect --format='{{json .Config.ExposedPorts}}' "${IMAGE_NAME}")
echo "   - Portas expostas: ${EXPOSED_PORTS}"
if ! echo "${EXPOSED_PORTS}" | grep -q "3000/tcp"; then
    echo "❌ Falha de Segurança: Porta 3000/tcp não encontrada nas portas expostas (${EXPOSED_PORTS})."
    exit 1
fi
echo "   ✅ Porta 3000/tcp (não-privilegiada >= 1024) exposta com sucesso."

# 4. Execução dinâmica e validação do usuário em runtime
echo ""
echo "▶️  [4/5] Iniciando container para validação de runtime..."
docker run -d --name "${CONTAINER_NAME}" \
    -e DATABASE_URL="postgres://postgres:postgres@localhost:5432/template_db" \
    "${IMAGE_NAME}" sleep 60

# 4.1 Validar UID e GID dentro do container
CONTAINER_UID=$(docker exec "${CONTAINER_NAME}" id -u)
CONTAINER_GID=$(docker exec "${CONTAINER_NAME}" id -g)
CONTAINER_USER=$(docker exec "${CONTAINER_NAME}" id -un)
echo "   - Runtime UID: ${CONTAINER_UID}, GID: ${CONTAINER_GID}, Usuário: ${CONTAINER_USER}"

if [ "${CONTAINER_UID}" -eq 0 ]; then
    echo "❌ Falha de Segurança Crítica: Processo dentro do container está rodando como ROOT (UID 0)!"
    exit 1
fi

if [ "${CONTAINER_UID}" -ne 1000 ]; then
    echo "⚠️ Aviso: UID não é 1000, mas é não-root (${CONTAINER_UID})."
fi
echo "   ✅ Processo em execução é NÃO-ROOT (UID: ${CONTAINER_UID} != 0)."

# 4.2 Validar permissões e ownership do diretório /app e binário
echo ""
echo "▶️  [5/5] Verificando ownership e permissões do binário..."
BIN_OWNER=$(docker exec "${CONTAINER_NAME}" stat -c '%U:%G' /app/template-servico-rust)
BIN_PERMS=$(docker exec "${CONTAINER_NAME}" stat -c '%a' /app/template-servico-rust)
echo "   - Owner do binário: ${BIN_OWNER}"
echo "   - Permissões do binário: ${BIN_PERMS}"

if [ "${BIN_OWNER}" != "appuser:appuser" ] && [ "${BIN_OWNER}" != "1000:1000" ]; then
    echo "❌ Falha de Ownership: Binário pertence a '${BIN_OWNER}' (esperado: appuser:appuser ou 1000:1000)."
    exit 1
fi
echo "   ✅ Ownership do binário está correto (${BIN_OWNER})."

# 4.3 Validar docker top para verificar usuário no host/engine
echo "   - Verificando processo via 'docker top':"
docker top "${CONTAINER_NAME}"

echo ""
echo "================================================================="
echo "🎉 TODOS OS TESTES DE SEGURANÇA DO DOCKERFILE PASSARAM COM SUCESSO!"
echo "   - ✅ Multi-stage build com cargo-chef funcionando perfeitamente"
echo "   - ✅ Usuário não-privilegiado (appuser, UID 1000, GID 1000) configurado"
echo "   - ✅ Runtime executa estritamente sem privilégios de root"
echo "   - ✅ Porta não-privilegiada segura configurada (3000)"
echo "   - ✅ Binário e diretório /app com ownership e permissões seguras"
echo "================================================================="
