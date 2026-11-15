# Prompt para o agente que testa no Linux

Cole isto no agente do Linux (Claude Code, Codex ou opencode), dentro do clone do repositório.

---

Você vai validar o MagoAgent num Linux de verdade e devolver um relatório. Não conserte nada sem avisar.

## Antes de tudo

1. `git fetch && git checkout porte-referencia && git pull`. Anote `git log --oneline -1` no topo do relatório.
2. Leia o `AGENTS.md` (regras, o roteiro `MAGOAGENT_SCRIPT`) e siga o roteiro completo de `docs/teste-linux.md`:
   as regras de lá valem aqui. Em especial:
   - sempre uma cena de teste (`MAGOAGENT_SCENE=/tmp/mago-teste/teste.mago`), nunca os dados reais;
   - não escreva em `~/.claude/settings.json`, `~/.codex` nem `~/.hermes`;
   - não peça nem digite senha de `sudo`: escreva o comando no relatório para a pessoa rodar;
   - não procure IP, endereço ou chave do OmniRoute: se o apelido `vps` não estiver configurado, marque NÃO TESTADO.
3. `cargo test --release`, `cargo clippy --all-targets --release -- -D warnings`,
   `rustfmt --edition 2024 --check $(find src -name "*.rs")` e `cargo build --release`.

## O que mudou e precisa de atenção (além do roteiro de `docs/teste-linux.md`)

Grave em vídeo (ou capturas seguidas) cada item da ilha. Para cada um: PASSOU, FALHOU, PARCIAL ou NÃO TESTADO, com a
evidência.

### 1. Cartão de pergunta e de permissão da ilha (`src/app/island/ask.rs`)
Use os hooks com o settings próprio do teste (seção 7 de `docs/teste-linux.md`).
- Um cartão só, com brilho ciano numa pergunta e âmbar numa permissão; o mago (ou o mascote do Claude) dentro dele, à
  esquerda; não há mais uma coluna separada.
- No topo: a bolinha da cor do agente, o nome e "está perguntando" (ou "está perguntando (1 de 3)"; "precisa de
  permissão" na permissão). A pergunta em até 2 linhas, inteira na dica do mouse.
- As opções em botões redondos lado a lado; a descrição de cada uma só na dica do mouse.
- Um clique numa opção responde e passa para a próxima pergunta; na última, envia. Escolha múltipla: os botões só
  marcam e o "OK" envia (apagado enquanto nada está marcado).
- Permissão: o comando numa linha, "Negar N" e "Permitir Y"; as teclas N e Y também respondem. Cliques nos primeiros
  600 ms do cartão não contam (de propósito); na pergunta não há essa trava.
- "Responder no terminal" (link cinza) só para sessões de fora do app.
- A ilha tem 200 de altura numa pergunta e 160 numa permissão.
- O ˄ no canto ("Depois — deixar esperando"), o ✕ e o Esc recolhem a ilha sem responder; o pedido continua esperando
  e volta quando a ilha abre. Com mais de um pedido, "+N na fila" ao lado do ˄.
- Os números 1 a 9 escolhem a opção como o clique.

### 2. Primeiro clique perdido
No Windows o primeiro clique numa ilha sem foco às vezes não vale (visto em "Conversar" e ao abrir a ilha compacta;
o segundo clique funciona). Confira no Linux, X11 e Wayland se houver: clique uma vez em outra janela, depois uma vez
num botão da ilha (abrir a compacta, "Conversar", uma opção de pergunta). Anote quantas de 10 tentativas o primeiro
clique valeu.

### 3. Aba Chat da ilha: a conversa compacta (`src/app/island/prompt.rs`)
- Abra a aba Chat (o balão) com um chat que já tenha mensagens: cartão com brilho índigo, o mago de 44 px à esquerda,
  suas mensagens em bolhas à direita e as respostas em texto cinza; pontinhos enquanto o agente trabalha.
- Embaixo, a pílula "agente · modelo" à direita e a caixa de uma linha ("Pergunte qualquer coisa…" / "Continuar…") com
  o botão redondo de enviar; Enter também envia.
- Altura 240, 40 a mais por mensagem, até 300.
- O ⤢ no canto do cartão (e a pílula do modelo) abre o mini chat completo na ilha (anexos, pasta, modo, modelo,
  ditado, voz). Ao fechar e reabrir a ilha, volta ao compacto. "Abrir em miniatura" vai direto ao completo.

### 4. Mascote do Claude Code
Quando o agente é o Claude Code, o mago vira o mascote laranja em pixel art: nos minis da compacta e nas pílulas, no
cartão da Início e no cartão de pergunta/permissão. Confira as expressões (pensando trabalhando, atento esperando,
pulinho ao terminar, dormindo).

### 5. Fluidez da ilha
A ilha anima a 60 quadros por segundo, aberta e compacta. Meça a CPU do processo por 20 s com a ilha fechada,
compacta e aberta (`pidstat -p <pid> 1 20` ou `top`), e diga se a animação parece fluida.

### 6. Aviso (toast) no design novo
O aviso no pé do canvas agora é de "vidro", como a barra de zoom. Para ver: com o app fechado, escreva lixo no
`chats.json` ao lado da cena de teste e abra o app. O aviso deve dizer "A lista de chats não pôde ser lida e começou
vazia. O arquivo antigo ficou na mesma pasta, como chats.json.bad-…", sem o caminho inteiro.

### 7. Ajustes no design novo
Abas Geral, Terminal, Agentes, Chat, Voz, Ok Mago, Integrações, Email, Dicionário e as que vêm depois (a faixa rola
pela roda ou pelas setas). Teste cada controle das abas novas:
- Chat: agente, pasta, permissão, estilo, "Compactar automaticamente em", "Painel Contexto aberto", "Hooks nos agentes
  do app".
- Ok Mago: "Ok Mago, …" chama o Maestro durante a chamada; "Responder em voz alta".
- Email: "Usar o do Gmail" preenche o servidor, "Testar" responde (não faça login em conta real).
- Dicionário: adicionar palavra e troca ("cloud code" → "Claude Code") e ver a troca no Ditar.
- Pelo MCP: `set_setting` com as chaves novas, e as ferramentas `dictionary` e `email`.

### 8. Correções da auditoria
- Uma cena de fora com `note.file` apontando para outro arquivo, ou com um botão de nota de ação inválida: o app não
  grava no arquivo de fora e descarta o botão.
- O autosave não trava a interface ao mexer muito rápido; fechar o app logo depois de uma mudança mantém a mudança.
- Dois agentes mexendo na mesma cena pelo MCP: o segundo recebe "a cena está ocupada por outro agente; tente de
  novo", e nenhum apaga a trava do outro.

### 9. Instalação pelo .deb e pelo apt
Seção 11 de `docs/teste-linux.md`.

## Relatório

`docs/relatorio-linux-<data>.md` com o ambiente (distro, GNOME/KDE, X11/Wayland, GPU), o commit, a tabela
item → resultado → evidência, os bugs com os passos para reproduzir e o que não deu para testar e por quê. Commit só
do relatório, sem linha de coautor.
