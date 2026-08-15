# ==============================================================================
# Multi-stage Dockerfile para template-servico-rust
# Otimizado com cargo-chef, usuário não-root (UID 1000) e porta segura (3000)
# ==============================================================================

# Etapa 1: Base com cargo-chef instalado para caching inteligente de dependências
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /usr/src/app

# Etapa 2: Planner - extrai o esqueleto do projeto (recipe.json)
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Etapa 3: Builder - compila dependências e a aplicação em release
FROM chef AS builder
COPY --from=planner /usr/src/app/recipe.json recipe.json
# Build das dependências (camada cacheada entre builds se Cargo.lock não mudar)
RUN cargo chef cook --release --recipe-path recipe.json
# Build da aplicação com SQLx offline mode
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin template-servico-rust

# Etapa 4: Runtime mínima baseada em Debian Slim com usuário não-privilegiado
FROM debian:bookworm-slim AS runtime

# Instalação de dependências essenciais de execução e certificados CA
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libpq5 curl && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Configuração de usuário e grupo não-root (UID/GID 1000)
ARG APP_USER=appuser
ARG APP_UID=1000
ARG APP_GID=1000

RUN groupadd -g ${APP_GID} ${APP_USER} && \
    useradd -u ${APP_UID} -g ${APP_USER} -m -d /home/${APP_USER} -s /bin/sh ${APP_USER}

# Diretório da aplicação
WORKDIR /app

# Cópia do binário compilado com permissões e ownership atribuídos diretamente
COPY --from=builder --chown=${APP_USER}:${APP_USER} /usr/src/app/target/release/template-servico-rust /app/template-servico-rust

# Garante permissões adequadas de execução
RUN chmod 755 /app/template-servico-rust

# Variáveis de ambiente padrão para execução em container
ENV HOST=0.0.0.0 \
    PORT=3000 \
    ENVIRONMENT=production

# Porta padrão não-privilegiada (>= 1024)
EXPOSE 3000

# Troca para usuário não-privilegiado (execução segura sem root)
USER ${APP_USER}

# Executa a aplicação
CMD ["/app/template-servico-rust"]
