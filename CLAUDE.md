# CLAUDE.md — Template de Microsserviço em Rust

> Carregado automaticamente no inicio de toda sessao neste projeto.

## Comece pelo snapshot (economia de contexto)

Para a maioria das tarefas, `docs/context-snapshot.md` traz o estado atual e
regras — comece por ele. Quem alterar arquitetura deve atualizar o snapshot.

## Leia antes de tarefas que toquem no assunto

1. `.junie/PROJECT_CONTEXT.md` — stack, convencoes, status atual
2. `.junie/LEARNINGS.md` — erros recorrentes e regras aprendidas
3. `BACKLOG.md` — backlog e sprints planejados
4. `AGENTS.md` — regras e contexto genérico para múltiplos agentes

## Regras deste projeto

- Web framework: Axum com tokio (runtime async).
- Qualidade de codigo: Rustfmt e Clippy, código limpo e idiomático.
- Testes automatizados com `cargo test` (unitários e de integração).
- Fluxo Gitflow estrito: `develop` para integração, `main` para produção, e uso de tags para releases (ver `AGENTS.md`).

## Ao final de uma sessao que mudou arquitetura ou aprendizado

Ofereça atualizar `.junie/PROJECT_CONTEXT.md` e `.junie/LEARNINGS.md` (se e quando existirem).
