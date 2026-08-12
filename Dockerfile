# Build stage
FROM rust:1.79-bookworm as builder

WORKDIR /usr/src/app
COPY . .

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/template-servico-rust /usr/local/bin/template-servico-rust

EXPOSE 3000

CMD ["template-servico-rust"]
