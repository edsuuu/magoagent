# Contribuir com o MagoAgent

Obrigado pelo interesse! O MagoAgent está em alpha e muda rápido, então converse antes de começar algo grande: abra uma
[issue](https://github.com/edsuuu/magoagent/issues/new/choose) com a ideia (há modelos para bug, travamento e recurso).

Este guia é o resumo para pessoas. As regras completas do código, que valem também para os agentes de IA que trabalham
no projeto, ficam no [`AGENTS.md`](AGENTS.md).

## Preparar o ambiente

- **Rust:** a versão fica fixa no [`rust-toolchain.toml`](rust-toolchain.toml); o `rustup` a instala sozinho no
  primeiro `cargo`.
- **Linux (Debian/Ubuntu):**

  ```sh
  sudo apt-get install -y pkg-config libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libasound2-dev \
    libdbus-1-dev libxkbcommon-dev libwayland-dev libssl-dev libwebkit2gtk-4.1-dev
  ```

- **Windows:** as Build Tools do Visual Studio (C++). O navegador usa o WebView2, que já vem no Windows 10 e 11.
- **macOS:** as ferramentas de linha de comando do Xcode (`xcode-select --install`).

Para rodar:

```sh
cargo run --release
```

No macOS, `./build-macos-app.sh` gera e abre o `MagoAgent.app`.

Um build do repositório roda como "MagoAgent (development)", com dados próprios: ele não mexe nos desenhos nem nos
ajustes do app instalado.

## Fluxo de trabalho

1. Crie um ramo a partir da `dev` (`fix/…`, `feat/…`, `docs/…`).
2. Faça o menor diff que resolve, no estilo do arquivo.
3. Abra o pull request **para a `dev`**, preenchendo o modelo: o que mudou, como foi testado, antes e depois.
4. A `dev` vai para a `main` num PR do mantenedor. O CI roda nele, e uma versão nova no `Cargo.toml` gera a release.

Os commits e as descrições dos PRs são em português.

## Antes de abrir o PR

```sh
rustfmt --edition 2024 --check $(find src -name "*.rs")   # formatação (até 160 colunas)
cargo clippy --all-targets --release -- -D warnings       # lints
cargo test --release                                      # testes, sem avisos
cargo deny check                                          # dependências: falhas, licenças, origens
```

O app é para Windows, macOS e Linux: se puder, confira também os outros sistemas (`cargo check --release --target …`,
os alvos estão no `rust-toolchain.toml`). O CI confere todos no PR da `dev` para a `main`.

## Regras principais

- Tudo o que a pessoa vê (textos da interface, mensagens, erros) é em português do Brasil.
- Nada específico de um sistema sem o equivalente dos outros.
- Toda lógica nova vem com teste (`src/app/tests/`, `src/tests/` ou `src/mcp/tests.rs`).
- Toda ação que a pessoa faz no app tem a sua ferramenta no MCP (`src/mcp/`).
- Nada bloqueia o frame da interface: arquivo, rede e processos rodam numa thread.
- Comentário explica o código; o motivo da mudança vai na descrição do PR.
- Linhas em branco separam as partes de um bloco: depois dos `let`, em volta de uma instrução de várias linhas e antes
  de `return`. O `cargo test` confere, e `MAGOAGENT_FIX_SPACING=1 cargo test --release statements_have_their_blank_lines`
  arruma sozinho.

## Testar a interface sem mexer na tela

O `MAGOAGENT_SCRIPT` dirige o app por passos, numa cena de teste (nunca num desenho seu de verdade):

```sh
printf '{"version":1,"elements":[],"background":[255,255,255,255]}' > /tmp/teste.mago
MAGOAGENT_SCENE=/tmp/teste.mago MAGOAGENT_SCRIPT="size 1440 900; wait 2000; click 146 30; shot /tmp/a.png; quit" \
  target/release/magoagent
```

Os passos (`size`, `click`, `rclick`, `move`, `drag`, `type`, `key`, `wait`, `shot`, `quit`) estão no `AGENTS.md`.

## Segurança

Achou uma falha de segurança? Não abra uma issue pública: siga o [`SECURITY.md`](SECURITY.md).

## Licença

Ao contribuir, você concorda que a sua contribuição fica sob a [licença MIT](LICENSE) do projeto.
