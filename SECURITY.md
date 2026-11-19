# Segurança

## Versões que recebem correção

O MagoAgent está em alpha. Só a [última release](https://github.com/edsuuu/magoagent/releases/latest) recebe
correções de segurança. Se você usa uma versão mais antiga, atualize por Ajustes › Sobre › Atualizar (ou pelo apt, no
Linux) antes de relatar.

## Como relatar uma falha

**Não abra uma issue pública.** Use o relato privado do GitHub:
[Security › Report a vulnerability](https://github.com/edsuuu/magoagent/security/advisories/new). Só o mantenedor vê o
relato.

Mande, se puder:

- a versão do app e o sistema (Windows, macOS ou Linux, com a versão);
- o que a falha permite fazer e quem consegue explorá-la (outro programa no mesmo computador, uma página aberta no
  navegador do canvas, alguém na rede);
- os passos para reproduzir, ou uma prova de conceito;
- se ela já é pública em algum lugar.

Nunca mande tokens, senhas ou chaves de verdade no relato. Se precisar mostrar um, troque-o antes.

## O que acontece depois

O relato recebe uma resposta assim que possível. Confirmada a falha, a correção sai numa release, e o aviso de segurança
é publicado junto, com crédito a quem relatou, se a pessoa quiser.

## Pontos sensíveis do app

O que importa para quem procura falhas:

- **A ponte local:** o app, os terminais e o servidor MCP conversam por uma ponte local com token. Cada terminal tem o
  seu token, que morre com o shell. Fora de um terminal, o cliente usa o `bridge.json` ao lado da cena.
- **O MCP:** `magoagent mcp` fala JSON-RPC pelo stdin e pelo stdout de quem o abre. Ele desenha na cena e pede ações ao
  app aberto pela ponte.
- **Segredos:** as senhas e os tokens guardados pelo app (integrações, cofre do navegador) vão para o chaveiro do
  sistema. Só ficam no arquivo de preferências se o chaveiro recusar.
- **Atualizações:** o Atualizar baixa o instalador da release do GitHub e confere o sha256 que o GitHub publica antes de
  rodá-lo. No Linux, o repositório apt é assinado com a chave GPG do projeto.
- **Navegador no canvas:** páginas comuns, cada navegador com o seu perfil. Os agentes leem e controlam as páginas que
  estão ligadas a eles por uma corda.

Fora do escopo: falhas do próprio Claude Code, do Codex ou de outro agente rodando nos terminais, e o que um agente faz
com as permissões que a pessoa deu a ele.
