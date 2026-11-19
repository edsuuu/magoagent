//! MCP server (`magoagent mcp`): lets any AI client draw on the MagoAgent canvas,
//! read it back and look at a rendered PNG. JSON-RPC over stdio, one message per line.
//! The shared autosave file is the source of truth, so it works with the app open or closed;
//! the open app merges these edits live and Cmd+Z undoes them.

use crate::prelude::*;
use std::io::{BufRead, Write};

pub(crate) mod actions;
pub(crate) mod app_tools;
pub(crate) mod bridge;
pub(crate) mod calls;
mod elements;
mod notes;

use calls::*;
use elements::*;
use notes::*;
#[cfg(test)]
mod tests;

pub(crate) const PROTOCOL_VERSION: &str = "2025-06-18";

pub(crate) const INSTRUCTIONS: &str = "MagoAgent é um quadro branco com traço feito à mão. Coordenadas em pixels, x para a direita e \
y para baixo; o canvas é infinito. Elementos são objetos JSON: type (rectangle, ellipse, diamond, text, \
arrow, line, freedraw), x, y, width, height, strokeColor e backgroundColor (hex ou \"transparent\"), fillStyle \
(solid, hachure, cross-hatch), strokeWidth (1, 2, 4), strokeStyle (solid, dashed, dotted), roughness (0 arquiteto, \
1 artista, 2 cartunista), opacity (0-100), angle (radianos), fontSize, text e points (relativos a x,y). Atalhos: \
`label: {text}` coloca um texto centralizado dentro da forma ou no meio da seta (id \"<id>-label\", \
acompanha o dono e interrompe a linha da seta atrás dele, então use label em setas de \"sim\"/\"não\"); \
textos que cruzam linhas ganham contorno na cor do fundo automaticamente; \
para contêineres/cards use type \"frame\" (name opcional, vazio esconde o título): mover o quadro \
leva tudo o que está dentro dele; setas com `start: {id}` e \
`end: {id}` se prendem às formas e as acompanham quando elas se movem. Dê `id` aos elementos para referenciá-los. \
Formas, textos e notas aceitam `link` (um arquivo, \"src/app/x.rs\" ou \"src/app/x.rs:120\", ou um endereço): o desenho ganha \
uma marca de link no canto que, clicada, abre o arquivo numa Árvore de Arquivos do canvas na linha, com um botão Voltar; ao \
desenhar sobre o código que a pessoa pediu para abrir ou ver na IDE do canvas, ponha em cada caixa o `link` do arquivo (e da \
linha) de que ela fala, com o caminho absoluto. file_link abre um desses links (ou volta) como o clique da pessoa. \
Fluxo sugerido: planeje o layout (ex.: caixas de 180x80 com 80px de espaço), crie tudo com add_elements, chame \
view para ver a imagem e corrija com update_elements. Antes de desenhar, leia get_scene e desenhe num espaço livre: nunca \
em cima de janelas, notas ou desenhos que já estão no canvas, nem uma coisa sua em cima de outra (janelas e notas que \
caem em cima de algo vão sozinhas para o lugar livre mais perto). Notas: a sua nasce ao seu lado, ligada a você por \
corda; para mudar uma, passe o `id` dela (o mesmo título também muda a que você já tem) em vez de abrir outra igual. \
Com muitas notas no canvas, agrupe as do mesmo assunto (canvas_group create com um nome, ou note_stack stack num \
fichário) e dê a cada assunto uma cor diferente (`color` da nota ou canvas_group color), para a pessoa achar cada um. \
Agentes: o canvas também tem terminais (type \"terminal\" com name, command e cwd) e uma seta presa a dois deles os conecta; \
um agente novo para uma tarefa é recruit_agent, que segue o ajuste Invocação dos agentes (um chat do app ou um terminal), não um terminal com claude. Um terminal com command \"claude\" retoma a conversa de antes ao reiniciar \
(claude --resume); update_elements com resume false desliga isso, resume true liga. De dentro de um terminal do MagoAgent, list_agents mostra quem está \
ligado a você, ask_agent manda um prompt a outro agente e espera a resposta, read_terminal lê a tela dele, e \
read_note, write_note e create_note mexem nas notas conectadas. `note` cria uma nota no canvas ou muda texto, título e cor (10 papéis); com `buttons` [{label, action, args}] (ou `note_buttons`) a nota ganha botões que a pessoa clica para rodar uma ação do app (as ferramentas do app: chat_new, open_path, ide_folder...). Navegadores (type \"portal\" com url e name) são \
páginas web no canvas: portal_navigate, portal_read e portal_eval os controlam. Árvores de Arquivos (type \"filetree\" com root, a pasta do projeto; com editor true ganham o painel de código) mostram o projeto; \
Para a pessoa acompanhar o seu trabalho numa árvore \
ligada a você, use ide_open (abre pastas na árvore e arquivos numa linha) e ide_search (procura no projeto); o resto está em file_tree. Para montar um time: recruit_agent cria um \
agente abaixo de você já ligado a você (com o model e o effort que você escolher), create_portal abre uma página à sua direita, connect liga duas coisas do \
canvas e notify avisa a pessoa (com o lugar tomado, cada um vai para o lugar livre mais perto de você; a resposta diz onde). Para rodar \
testes, uma API ou um servidor e acompanhar, create_terminal abre um terminal à sua direita \
rodando o comando; todo terminal, navegador ou árvore que você abre (também por add_elements) fica ligado a você por corda, e você o \
acompanha com read_terminal. Para mover ou redimensionar uma janela (terminal, navegador, árvore de arquivos), use update_elements com \
x, y, width e height. Pediu para abrir, mostrar ou ver algo (arquivo, pasta, PDF, página, chat, um ponto do canvas): faça o que for preciso e abra na tela da pessoa com a ferramenta certa (open_path, ide_folder, focus_canvas, show_screen, chat_open), sem perguntar e sem só descrever onde está. \
Cofre de senhas: vault_list mostra as contas que a pessoa cadastrou (site, usuário, tipo), vault_accounts as de um site e \
vault_fill digita a senha (ou o e-mail, num login via Google/Apple) no campo focado do navegador aberto nesse site, sem que \
você veja o segredo. Quando um site tem mais de uma conta, pergunte à pessoa qual usar antes de vault_fill; nunca peça a \
senha em texto nem tente lê-la da página.";

pub fn run() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };

        if line.trim().is_empty() {
            continue;
        }

        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle(&message),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };

        if let Some(reply) = reply {
            let _ = writeln!(out, "{reply}");
            let _ = out.flush();
        }
    }
}

pub(crate) static AGENT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// A readable name for the MCP client: "claude-code" → "Claude Code".
pub(crate) fn agent_label(client: &str) -> String {
    match client {
        "claude-ai" => "Claude".into(),
        "codex-mcp-client" => "Codex".into(),
        "cursor-vscode" => "Cursor".into(),
        "" => "IA".into(),
        other => other
            .split(['-', '_', ' '])
            .filter(|w| !w.is_empty() && *w != "mcp" && *w != "client")
            .map(|w| {
                let mut chars = w.chars();

                chars.next().map_or(String::new(), |first| first.to_uppercase().chain(chars).collect())
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// One JSON-RPC message in, at most one response out (notifications get none).
pub(crate) fn handle(message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let params = &message["params"];

    let result = match message["method"].as_str().unwrap_or_default() {
        "initialize" => {
            // the app shows this name on the cursor while it replays what the AI drew: the
            // terminal's own name when the AI runs inside one of the canvas terminals
            let terminal = std::env::var("MAGOAGENT_TERMINAL_NAME").ok().filter(|n| !n.is_empty());
            let _ = AGENT.set(terminal.unwrap_or_else(|| agent_label(params["clientInfo"]["name"].as_str().unwrap_or_default())));

            Ok(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "magoagent", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => Ok(call(params["name"].as_str().unwrap_or_default(), &params["arguments"])),
        other => Err(json!({ "code": -32601, "message": format!("método desconhecido: {other}") })),
    };

    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}
