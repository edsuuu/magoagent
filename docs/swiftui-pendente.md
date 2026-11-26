# Interface SwiftUI (macOS): o que está pendente

Parado em 26/11/2026 a pedido do dono. Branch `wip/mac-swift-pendente` = `feat/mac-swift` + os dois rascunhos dos agentes
juntados + esta nota. O PR vai para a `dev` (não para a `main`), e só depois de mostrar o resultado ao dono.

## Estado

**Conferido** (build, `cargo test --release` com 1374 testes passando até o `9020b64a`, capturas lado a lado com o egui):

- Moldura, painéis da direita, canvas, chat e ilha em SwiftUI no modelo da `main` (commits `87840885` a `9020b64a`).
- Depois disso, só build e capturas, sem a rodada completa de testes:
  - `83f3e1b7`: Árvore, a barra de rolagem fina do egui.
  - `e08966d5`: Árvore, o nome do cabeçalho corta com "…".
  - `80ea15f7`: Terminal, os traços de caixa ligados.

**Compila, mas não foi conferido na tela:**

- `5eb3a732`: Terminal, o ícone de servidor no botão Processos (a captura não pegou o rodapé).

**Rascunho, sem build nem testes:** os dois commits abaixo. Os agentes pararam no meio por erro de assinatura da API
(`oauth_org_not_allowed`).

## Agente 1: Ajustes em SwiftUI como a main

Commit `e601f521` (branch `worktree-agent-a146effc013468908`), cerca de 1650 linhas a mais e 970 a menos em 15 arquivos.

- Já tem o rascunho antigo `swift/settings` (`d61922fb`) incorporado.
- Já tem o Rust de `src/app/ui/settings/native/`: `mod.rs`, `app.rs` (Atalhos, Dados, Sobre), `pages.rs`, `services.rs`,
  `acts.rs` e o `tests.rs` novo.
- No Swift, já tem `SettingsKit.swift` e `PresetRow.swift`.
- Atalhos e Dados foram conferidos em captura e batem com o egui.

**Falta:**

1. Compilar e corrigir.
2. Portar as páginas novas da `main`: Ilha, Canvas, Navegação e editor, Skills.
3. Esconder as abas de `HIDDEN_TABS` (Voz, Ok Mago).
4. Pôr o MCP em Integrações, com Remover.
5. Conferir cada aba em captura contra o egui.
6. Rodar os testes.

## Agente 2: painel Contexto do chat em SwiftUI

Commit `1b43c084` (branch `worktree-agent-a919781675b376305`), cerca de 270 linhas a mais e 90 a menos em 9 arquivos.

- Porte do `context_view.rs`.
- Os textos ficam num lugar só para o egui e o snapshot (`connected_words` em `context.rs`, `connectors_note` em
  `context_ui.rs`).
- Tópico novo em `src/app/chat/native/context.rs`.
- `ContextPanel.swift`: o painel na borda direita da tela de Chats.
- O anel de contexto do cabeçalho vira botão.

**Falta:**

1. Compilar e corrigir.
2. Conferir o painel aberto em captura contra o egui.
3. Rodar os testes (há um teste novo em `chat/native/tests.rs`).

## Pendências do trabalho principal

- **Árvore:** o polegar da barra sai uns 6 pt mais curto que no egui, porque a lista SwiftUI conta os 40 pt vazios do fim.
- **Terminal:**
  - Só os traços finos de caixa são desenhados na célula; os grossos (━ ┃) e duplos (═ ║) ainda vêm da fonte.
  - O popover Processos não foi conferido na tela.
- **Diferenças pequenas vistas nas capturas, ainda sem correção:**
  - O nome do cabeçalho da Árvore sai menos encorpado que no egui.
  - As linhas da Árvore ficam 1 pt à direita.
  - O azul do preenchimento da "Caixa" e o do minimapa estão um pouco diferentes.
- **Marcas nas bordas:** as capturas SwiftUI mostram marcas onde cápsulas e círculos com contorno tocam a moldura.
  Parece coisa da captura (`cacheDisplay`), mas não foi visto numa tela de verdade.
- **Outros sistemas:** o `cargo check` dos outros alvos falha nesta máquina por dependências em C (ambiente). Falta
  conferir no CI.

## Para fechar

1. Compilar com os dois rascunhos e terminar o que falta neles.
2. Rodar as conferências:
   - espaçamento (`MAGOAGENT_FIX_SPACING=1 cargo test --release statements_have_their_blank_lines`);
   - `rustfmt --edition 2024`;
   - `cargo clippy --all-targets --release -- -D warnings`;
   - `cargo test --release`.
3. Mostrar ao dono: capturas ou o app compilado (instalar só se ele pedir).
4. Abrir o PR para a `dev` pelo modelo `.github/pull_request_template.md`.

As worktrees antigas em `.claude/worktrees/` já foram integradas: só guardam pastas de build (`.build`, `.magoagent`).
Podem ser apagadas, mas pergunte ao dono antes. O mesmo vale para as pastas `target-*` sem versionar na raiz.
