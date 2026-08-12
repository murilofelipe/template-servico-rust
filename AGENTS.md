---
name: read-context
description: Sempre ler a documentação de contexto ao iniciar a sessão deste projeto
---

# Regra: Leitura de Contexto Obrigatória

Ao iniciar o trabalho neste projeto (`template-servico-rust`), os agentes (Antigravity, Claude, Junie, etc.) DEVEM, antes de executar tarefas de codificação, análise ou criação de mockups, visualizar e ler os seguintes arquivos para se nortearem sobre o sistema:

1. A pasta `docs/` (especialmente arquivos de arquitetura, READMEs ou backups relevantes) caso exista.
2. Arquivos ocultos de prompt/instrução na raiz do projeto, especificamente `.claude`, `CLAUDE.md`, `.junie` ou `AGENTS.md`, caso existam.

Essa regra garante que as mudanças propostas estejam alinhadas com as diretrizes e o contexto global preestabelecido pelo usuário e equipe.

# Regra: Fluxo de Trabalho (Gitflow)

Todo desenvolvimento e assistência realizados por agentes neste projeto (e nos seus derivados) DEVE seguir rigorosamente a estratégia de Gitflow:

- **`develop`**: É a branch base para qualquer novo desenvolvimento. Nenhuma feature deve ser comitada ou aberta via Pull Request direto para a `main`. Tudo deve ser direcionado para `develop`.
- **`feature/*` ou `bugfix/*`**: Para criar novas features, o agente deve sempre criar uma branch a partir da `develop`.
- **`main`**: É a branch de produção. Ela reflete apenas código estável e não recebe PRs diretos de features, mas sim releases vindos da `develop`.
- **Tags de Release**: Sempre que uma release for realizada, uma tag Git (`vX.X.X` seguindo o Semantic Versioning) deve ser gerada e atualizada no projeto. Ao criar documentação de changelog ou release notes, lembre-se de associar a essas tags.

Agentes autônomos que realizam merges ou abrem PRs **não têm permissão** para contornar essa regra sem autorização explícita do usuário.
