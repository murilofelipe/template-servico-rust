# template-servico-rust

Template base para microsserviços na linguagem Rust.

## Pipelines Integradas (GitHub Actions)
- **Qualidade e Cobertura:** Valida o código utilizando `cargo fmt`, `cargo clippy`, verifica cobertura via `cargo-tarpaulin` e previne duplicação via `jscpd` (limite de 5%).
- **Segurança (SAST):** Executa `cargo-audit` e `cargo-deny` para detectar vulnerabilidades conhecidas em dependências e validar licenças antes do build.
- **Versionamento:** Garante a padronização de commits através do `commitlint` (padrão Conventional Commits) em todos os Pull Requests.
- **CI/CD:** Valida qualidade, testes e SAST antes de construir a imagem Docker baseada no `Dockerfile` multi-stage e enviar para o GitHub Container Registry (GHCR) ao integrar na branch `develop`.

## Execução Local de Checagens
- `make audit`: Executa análise de vulnerabilidades com `cargo audit`.
- `make deny`: Executa análise de licenças e advisories com `cargo deny`.
- `make sast`: Executa a suíte completa de SAST (`cargo audit` + `cargo deny`).
- `make verify-sast`: Executa o script isolado de validação de SAST (`scripts/verify-sast.sh`).
- `make check`: Executa formatação, linter, SAST e testes.