# Etapa base para instalar o cargo-chef
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /usr/src/app

# Etapa de preparacao (gera o recipe.json)
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Etapa de build
FROM chef AS builder
COPY --from=planner /usr/src/app/recipe.json recipe.json
# Buid das dependencias (faz cache na camada do docker)
RUN cargo chef cook --release --recipe-path recipe.json
# Build da aplicacao (so roda se houver alteracoes no source)
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin template-servico-rust

# Etapa de Runtime
FROM debian:bookworm-slim AS runtime

# Instalacao de dependencias essenciais e certificados
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libpq5 && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Criacao de usuario nao-root para seguranca
RUN groupadd -r appuser && useradd -r -g appuser -d /usr/local/bin appuser

WORKDIR /usr/local/bin

COPY --from=builder /usr/src/app/target/release/template-servico-rust ./template-servico-rust

# Transfere a propriedade dos binarios para o usuario nao-root
RUN chown appuser:appuser ./template-servico-rust

USER appuser

EXPOSE 3000

CMD ["./template-servico-rust"]
