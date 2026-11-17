# Revisão da 0.0.20 no Windows

Nota para o agente que valida o MagoAgent num Windows de verdade. Esta revisão é só do que a **0.0.20** mudou e do que
não pôde ser testado no Linux, onde ela foi feita. Leia antes o `AGENTS.md`, com as regras e o roteiro
`MAGOAGENT_SCRIPT`.

## Regras

- Teste o ramo `feat/ilha-abas` (PR #55), na tag `v0.0.20` ou depois dela. Anote no topo do relatório o commit testado
  (`git log --oneline -1`).
- Use sempre uma **cena de teste**, nunca o desenho real do usuário. Em PowerShell:
  `$env:MAGOAGENT_SCENE = "$env:TEMP\mago-teste\teste.mago"`.
- Não escreva em `~\.claude\settings.json`, `~\.codex` nem `~\.hermes` (ler pode).
- Não procure endereço, IP nem chave de serviço nenhum. O que precisar de conta ou token que não esteja configurado
  fica como **NÃO TESTADO**, dizendo o que falta.
- Prefira o roteiro (`MAGOAGENT_SCRIPT`) a mexer no mouse e no teclado de quem usa a máquina. No Windows as teclas
  injetadas de fora não chegam ao app: teclas, só pelo roteiro.
- Cada item recebe **PASSOU**, **FALHOU**, **PARCIAL** ou **NÃO TESTADO**, com a evidência (captura, saída de comando,
  log) e, se falhou, como reproduzir.
- Não conserte nada sem dizer. Se consertar, faça um commit por correção, sem linha de coautor, e liste os commits.

## Preparar

```powershell
git clone -b feat/ilha-abas https://github.com/edsuuu/magoagent; cd magoagent
rustfmt --edition 2024 --check (Get-ChildItem src -Recurse -Filter *.rs).FullName
cargo clippy --all-targets --release -- -D warnings
cargo test --release
cargo build --release
```

O clippy e os testes **no Windows** são o primeiro item a conferir. O commit `8fdeff3` corrigiu três avisos de código
sem uso que só o clippy do Windows achou: `work_at` em `src/app/chat/mini.rs`, `mpris_track_id` em
`src/app/integrations/spotify/read.rs` e o `cpal::Stream` em `src/app/integrations/spotify/groove/capture.rs`. Isso
foi corrigido sem rodar o clippy no Windows. Os testes do Windows no CI foram cancelados antes do fim nas duas
primeiras tentativas, por falhas em outros jobs, então nunca chegaram a terminar. Se algum teste falhar só no
Windows, anote o nome e a mensagem.

Anote a versão do Windows, a escala da tela (100/125/150%), quantos monitores e o tempo de build.

## O que testar

### 1. Instalar a 0.0.20 pelo MSI
- O MSI `MagoAgent-0.0.20.msi` sai num rascunho de release da tag `v0.0.20`, que só o dono vê. Peça a ele o arquivo,
  ou rode `sh installer/build-msi.sh` se o WiX estiver instalado.
- Instale por cima da 0.0.19. Confira:
  - os dados continuam em `C:\Program Files\MagoAgent\data`;
  - Ajustes › Sobre mostra 0.0.20.
- O **Atualizar** da 0.0.19 não acha um rascunho. Enquanto o dono não publicar, marque esse item NÃO TESTADO.

### 2. A ilha se chama "Ilha da Luna" (risco alto)
O título da janela da ilha mudou de "Ilha do Maestro" para "Ilha da Luna", agora na constante `TITLE` de
`src/app/island/window.rs`. No Windows, o app **acha a ilha pelo título** com `FindWindowExW`
(`src/app/voice/popup.rs`, em `plain::reveal` e `plain::click_through`). Se algum lugar ainda procurar o título
antigo, a ilha fica invisível ou deixa de deixar o clique passar. Confira:
- a ilha aparece ao abrir o app, sem quadro e sem botão na barra de tarefas;
- fora da pílula, o clique passa para a janela de baixo; sobre ela, a ilha recebe o clique;
- ela abre, fecha sozinha e fecha pelo ✕;
- `Get-Process magoagent | Select MainWindowTitle` e as janelas do processo não mostram mais "Maestro".

### 3. Limpar todos os dados (Ajustes › Dados)
Fluxo:
1. O app pergunta antes de apagar.
2. Ele para os agentes e fecha.
3. Uma cópia dele, `magoagent limpar <pid> [--cofre] <pastas>`, espera o app sair.
4. A cópia apaga as pastas e, no app instalado, os segredos do cofre.
5. Ela abre o app de novo.

O código fica em `src/app/system/wipe.rs`. A cópia sai por `crate::editor::command`, com `CREATE_NO_WINDOW`.

**Comece pelo build de desenvolvimento** (`target\release\magoagent.exe`). Ele só apaga a pasta dele
(`%APPDATA%\MagoAgent-dev`) e não mexe no cofre nem no `%USERPROFILE%\.magoagent`, que são do app instalado. Confira:
- a pergunta lista as pastas certas;
- Cancelar não apaga nada;
- ao confirmar:
  - nenhuma janela de console pisca;
  - o app fecha e, poucos segundos depois, abre de novo sozinho, nas boas-vindas;
  - `%APPDATA%\MagoAgent-dev` foi recriada do zero;
  - `%USERPROFILE%\.magoagent` e as credenciais do Windows ficaram.

**No app instalado** isso apaga dados reais e as senhas guardadas. Só faça numa conta do Windows ou numa VM sem dados
de verdade, ou com o OK do dono. Se não puder, marque NÃO TESTADO. Confira:
- `C:\Program Files\MagoAgent\data` **continua existindo, vazia, e é reabastecida**. O commit `7fc838b` passou a
  esvaziá-la em vez de apagá-la; apagada, o app passaria a gravar em `%APPDATA%\MagoAgent` sem avisar;
- `%APPDATA%\MagoAgent` **não** foi criada de novo;
- `%USERPROFILE%\.magoagent` sumiu;
- no Gerenciador de Credenciais (Credenciais do Windows) somem os itens do MagoAgent:
  - `prefs-openai-key`;
  - `prefs-voice-studio-key`;
  - `integration-github-token`;
  - `integration-spotify-refresh-token`;
  - `integration-whatsapp-gowa-credential`;
  - as senhas do cofre do navegador;
- com um arquivo da pasta de dados aberto em outro programa, a limpeza não trava e apaga o resto. A cópia roda sem
  saída nenhuma, então o que não deu para apagar não aparece em lugar nenhum: anote o que ficou na pasta.

### 4. A Luna dança com a música (WASAPI)
No Windows, o app escuta o som do computador pelo loopback do WASAPI (cpal), em
`src/app/integrations/spotify/groove/capture.rs`. O commit `8fdeff3` mexeu no `Stream` que mantém essa escuta viva.
- Ligue Ajustes › Integrações › "A Luna dança com a música" e toque música em qualquer programa.
- A Luna dança na ilha, no ritmo, e para quando a música para.
- Se ela dança um instante e para, o stream está sendo solto cedo demais.

### 5. Aba do Spotify (SMTC)
No Windows, o app lê o que toca pelos controles de mídia do sistema (`src/app/integrations/spotify/smtc.rs`).
- Sem conta: com `MAGOAGENT_FAKE_SPOTIFY=1` e a cena de teste, a aba mostra o player falso: "Mentira ou Verdade",
  84 s de 198 s.
- Com o Spotify do Windows aberto, a aba mostra a música, a capa e o tempo, e os botões funcionam.
- O Soloist **não** aparece em Ajustes › Integrações no Windows, porque só existe no Linux.

### 6. Aba do GitHub
- Sem token: com `MAGOAGENT_FAKE_GITHUB=<repo>\src\app\integrations\fixtures\github_pulse.json` e a cena de teste,
  confira:
  - os filtros Abertos, Mesclados e Fechados;
  - o detalhe de um PR;
  - editar a descrição;
  - comentar. No modo falso nada sai para o GitHub.
- O texto e os acentos aparecem certos, e a lista rola.

### 7. Arrastar a ilha e trazer de volta
- Com dois monitores:
  - arraste a ilha até o topo de cada um; ela encaixa sem pular;
  - a área de trabalho de cada monitor vem de `plain::work_area`;
  - a ilha não fica embaixo da barra de tarefas.
- Ajustes › Geral › Trazer a ilha de volta põe a ilha no lugar padrão.
- Teste com a escala de 125% ou 150%, se der.

### 8. Boas-vindas
Rode com `MAGOAGENT_WELCOME=1` numa pasta de cena vazia. Para capturar a transição, use
`key Enter; wait 30; shot a.png; wait 1; shot b.png; wait 1500; shot c.png`. Com um `shot` logo antes do `key`, as
imagens podem sair fora de ordem.
- A transição entre slides:
  - o slide que sai desliza e some;
  - o novo entra pelo outro lado;
  - o fundo muda de cor e o botão escorrega até o lugar novo;
  - leva uns 0,45 s.
- Voltando (seta para a esquerda), o movimento é o contrário.
- A boca da Luna:
  - enquanto ela fala, um "D" pequeno entre os olhos, que abre e fecha;
  - quando ela olha para o balão, a boca vai junto com os olhos;
  - ao terminar de falar, some, sem virar um fio.
- No último slide, o espaço criado abre normalmente.

### 9. Chat só com a primeira mensagem
- Abra a tela de Chat (ou Nova conversa) e não envie nada:
  - nenhum cartão aparece no canvas;
  - o `chats.json` da pasta de dados não muda;
  - nada novo em `%USERPROFILE%\.magoagent`.
- Envie a primeira mensagem: aí o chat aparece no canvas e na lista.
- Escolha outro chat antes de enviar: o rascunho some.

### 10. A Luna nos textos
- Nada visível diz mais "Maestro". Confira:
  - Ajustes (Memória da Luna, Voz, Ok Mago, Agentes);
  - Comentar para a Luna na Árvore (Ctrl+Alt+M);
  - o cartão do chat no canvas;
  - `magoagent list` num terminal;
  - a amostra de voz ("Olá! Eu sou a Luna, a assistente do MagoAgent.").
- O "Ok Mago" continua igual.

### 11. Mensagem para o Claude Code de fora
- No Windows não há tmux, então a mensagem da ilha para uma sessão do Claude Code de fora chega **no fim da vez**
  (no Stop).
- Use um `--settings` próprio do teste com os hooks apontando para `<repo>\target\release\magoagent.exe hook external`.
  Não mexa no `~\.claude\settings.json`.
- Confira que a mensagem chega e que nada trava se a sessão terminar antes.

## Relatório

Grave o relatório em `docs/relatorio-windows-<data>.md`, ou entregue como resposta. Ele deve ter:
- o ambiente e o commit testado;
- a tabela: item → PASSOU/FALHOU/PARCIAL/NÃO TESTADO → evidência;
- os bugs, com passos para reproduzir;
- o que não deu para testar, e por quê.
