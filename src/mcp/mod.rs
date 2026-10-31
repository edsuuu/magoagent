//! MCP server (`magoagent mcp`): lets any AI client draw on the MagoAgent canvas,
//! read it back and look at a rendered PNG. JSON-RPC over stdio, one message per line.
//! The shared autosave file is the source of truth, so it works with the app open or closed;
//! the open app merges these edits live and Cmd+Z undoes them.

use crate::prelude::*;
use std::io::{BufRead, Write};

mod calls;
mod elements;

use calls::*;
use elements::*;
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
Fluxo sugerido: planeje o layout (ex.: caixas de 180x80 com 80px de espaço), crie tudo com add_elements, chame \
view para ver a imagem e corrija com update_elements. \
Agentes: o canvas também tem terminais (type \"terminal\" com name, command e cwd; ex.: command \"claude\" ou \"codex\") \
e uma seta presa a dois deles os conecta. De dentro de um terminal do MagoAgent, list_agents mostra quem está \
ligado a você, ask_agent manda um prompt a outro agente e espera a resposta, read_terminal lê a tela dele, e \
read_note, write_note e create_note mexem nas notas adesivas conectadas. Portais (type \"portal\" com url e name) são \
páginas web no canvas: portal_navigate, portal_read e portal_eval os controlam. IDEs (type \"ide\" com root, a \
pasta do projeto, e name) mostram o projeto num editor; um terminal ligado a um IDE por uma seta abre o Claude Code \
integrado a ele (as mudanças propostas aparecem como diff no IDE). Para a pessoa acompanhar o seu trabalho num IDE \
ligado a você, use ide_open (abre pastas na árvore e arquivos numa linha), ide_search (procura no projeto) e ide_edit \
(troca um trecho de um arquivo à vista, salvando). Para montar um time: recruit_agent cria um \
agente abaixo de você já ligado a você, create_portal abre uma página à sua direita, connect liga duas coisas do \
canvas e notify avisa a pessoa. Para mover ou redimensionar uma janela (terminal, portal, IDE), use update_elements com \
x, y, width e height. \
Cofre de senhas: vault_list mostra as contas que a pessoa cadastrou (site, usuário, tipo), vault_accounts as de um site e \
vault_fill digita a senha (ou o e-mail, num login via Google/Apple) no campo focado do portal aberto nesse site, sem que \
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
