# template-servico-rust

Template base para microsserviços na linguagem Rust.

## Pipelines Integradas (GitHub Actions)
- **Qualidade e Cobertura:** Valida o código utilizando `cargo fmt`, `cargo clippy`, verifica cobertura via `cargo-tarpaulin` e previne duplicação via `jscpd` (limite de 5%).
- **Versionamento:** Garante a padronização de commits através do `commitlint` (padrão Conventional Commits) em todos os Pull Requests.
- **CI/CD:** Constrói a imagem Docker baseada no `Dockerfile` multi-stage e envia para o GitHub Container Registry (GHCR) ao integrar na branch `develop`.