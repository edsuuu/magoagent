//! O que o painel "Contexto" do chat mostra e de onde vem: o evento `system`/`init` do stream do
//! Claude Code (modelo, conectores e ferramentas), o `usage` das respostas (tokens em uso agora),
//! o `/context` em modo print (a divisão por categoria, a janela e a margem do auto-compact) e o
//! `claude mcp list` (os conectores antes da primeira resposta). Aqui só o que é puro: ler esses
//! textos e formatar números; a busca em segundo plano e a pintura ficam em chat_context_ui.rs.

use crate::prelude::*;

/// Como um conector está: o `status` do init e a linha do `claude mcp list`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Connected,
    Failed,
    NeedsAuth,
    Pending,
}

impl Status {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Status::Connected => "Conectado",
            Status::Failed => "Falhou",
            Status::NeedsAuth => "Precisa entrar",
            Status::Pending => "Conectando",
        }
    }

    fn of(word: &str) -> Status {
        let word = word.to_lowercase();
        if word.contains("needs") || word.contains("auth") {
            Status::NeedsAuth
        } else if word.contains("fail") || word.contains("error") {
            Status::Failed
        } else if word.contains("connected") && !word.contains("dis") {
            Status::Connected
        } else {
            Status::Pending
        }
    }
}

/// Um conector (servidor MCP): só o nome e o estado, nunca o comando, o endereço ou o que há de
/// segredo na configuração.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Server {
    pub(crate) name: String,
    pub(crate) status: Status,
}

/// Uma linha da tabela de categorias do `/context`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cat {
    pub(crate) name: String,
    pub(crate) tokens: u64,
    pub(crate) pct: f32,
}

/// Uma ferramenta MCP da tabela do `/context`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ToolRow {
    pub(crate) server: String,
    pub(crate) name: String,
    pub(crate) tokens: u64,
}

/// O `/context` lido: usado / janela, as categorias, as ferramentas MCP e os arquivos de memória.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Report {
    pub(crate) model: String,
    pub(crate) used: u64,
    pub(crate) window: u64,
    pub(crate) cats: Vec<Cat>,
    pub(crate) tools: Vec<ToolRow>,
    /// (caminho, tokens)
    pub(crate) memory: Vec<(String, u64)>,
}

impl Report {
    pub(crate) fn cat(&self, name: &str) -> u64 {
        self.cats.iter().find(|c| c.name == name).map_or(0, |c| c.tokens)
    }

    /// O que falta para o auto-compact agir: a janela menos o buffer dele e o que já se usou.
    pub(crate) fn until_compact(&self) -> u64 {
        self.window.saturating_sub(self.cat("Autocompact buffer")).saturating_sub(self.used)
    }
}

/// "221k", "26.7k", "622", "1.2M" (o que o `/context` imprime, com ou sem "~") em tokens.
pub(crate) fn parse_tokens(text: &str) -> Option<u64> {
    let text = text.trim().trim_start_matches('~').replace(',', "");
    let (number, scale) = match text.chars().last()? {
        'k' | 'K' => (&text[..text.len() - 1], 1e3),
        'm' | 'M' => (&text[..text.len() - 1], 1e6),
        _ => (text.as_str(), 1.0),
    };
    number.trim().parse::<f64>().ok().map(|n| (n * scale).round() as u64)
}

/// 301800 → "301.8k", 221000 → "221k", 622 → "622", 1_200_000 → "1.2M".
pub(crate) fn short(n: u64) -> String {
    let trim = |v: f64, unit: &str| format!("{}{unit}", format!("{v:.1}").trim_end_matches('0').trim_end_matches('.'));
    match n {
        0..=999 => n.to_string(),
        1_000..=999_949 => trim(n as f64 / 1e3, "k"),
        _ => trim(n as f64 / 1e6, "M"),
    }
}

/// 55.2 → "55,2%": a vírgula decimal, como no resto do app.
pub(crate) fn percent(pct: f32) -> String {
    format!("{pct:.1}%").replace('.', ",")
}

/// O nome de um conector como aparece nas ferramentas dele: "claude.ai Gmail" → "claude_ai_Gmail".
pub(crate) fn norm(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

/// A ferramenta sem o prefixo do conector: "mcp__rea__xrefs" → "xrefs".
pub(crate) fn tool_name<'a>(full: &'a str, server: &str) -> &'a str {
    full.strip_prefix("mcp__").and_then(|r| r.strip_prefix(norm(server).as_str())).and_then(|r| r.strip_prefix("__")).unwrap_or(full)
}

/// As células de uma linha de tabela markdown; None para o traço e para o que não é linha.
fn cells(line: &str) -> Option<Vec<&str>> {
    let line = line.trim();
    if !line.starts_with('|') {
        return None;
    }
    let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
    (!cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':'))).then_some(cells)
}

/// A resposta do `/context` (o campo `result` do `claude -p --output-format json`).
pub(crate) fn parse_context(text: &str) -> Result<Report, String> {
    let mut report = Report::default();
    let mut section = "";
    let mut seen = false;
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(title) = line.strip_prefix("### ") {
            section = title.trim();
            continue;
        }
        if let Some(model) = line.strip_prefix("**Model:**") {
            report.model = model.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("**Tokens:**") {
            // "36.8k / 400k (9%)"
            let (used, window) = rest.split_once('/').ok_or("não entendi a linha de tokens do /context")?;
            report.used = parse_tokens(used).ok_or("tokens usados ilegíveis")?;
            report.window = parse_tokens(window.split('(').next().unwrap_or_default()).ok_or("janela ilegível")?;
            seen = true;
        }
        let Some(c) = cells(line) else { continue };
        let tokens = |s: &str| parse_tokens(s).unwrap_or(0);
        match (section, c.as_slice()) {
            (_, [first, ..]) if ["Category", "Tool", "Type"].contains(first) => {}
            ("Estimated usage by category", [name, t, p]) => {
                report.cats.push(Cat { name: name.to_string(), tokens: tokens(t), pct: p.trim_end_matches('%').parse().unwrap_or(0.0) })
            }
            ("MCP Tools", [tool, server, t]) => {
                report.tools.push(ToolRow { server: server.to_string(), name: tool_name(tool, server).to_string(), tokens: tokens(t) })
            }
            ("Memory Files", [_, path, t]) => report.memory.push((path.to_string(), tokens(t))),
            _ => {}
        }
    }
    if seen { Ok(report) } else { Err("o /context não trouxe a linha de tokens".into()) }
}

/// O `result` do JSON que `claude -p --output-format json` imprime (ou o erro dele).
pub(crate) fn print_result(json: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(json.trim()).map_err(|_| "a saída do claude não era JSON".to_string())?;
    let text = v["result"].as_str().unwrap_or_default().to_string();
    if v["is_error"] == true { Err(text) } else { Ok(text) }
}

/// O evento `system`/`init` do stream: o modelo, os conectores com o estado e as ferramentas MCP.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Init {
    pub(crate) model: String,
    pub(crate) servers: Vec<Server>,
    pub(crate) mcp_tools: Vec<String>,
}

pub(crate) fn parse_init(event: &Value) -> Option<Init> {
    if event["type"] != "system" || event["subtype"] != "init" {
        return None;
    }
    let servers = event["mcp_servers"]
        .as_array()?
        .iter()
        .filter_map(|s| Some(Server { name: s["name"].as_str()?.to_string(), status: Status::of(s["status"].as_str().unwrap_or_default()) }))
        .collect();
    let mcp_tools = event["tools"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|t| t.starts_with("mcp__")).map(String::from).collect();
    Some(Init { model: event["model"].as_str().unwrap_or_default().to_string(), servers, mcp_tools })
}

/// Os tokens do contexto depois de uma resposta do assistente (não de um subagente): o que foi
/// lido, o que entrou no cache e o que ele escreveu.
pub(crate) fn context_used(event: &Value) -> Option<u64> {
    if event["type"] != "assistant" || !event["parent_tool_use_id"].is_null() {
        return None;
    }
    let usage = &event["message"]["usage"];
    let n = |k: &str| usage[k].as_u64().unwrap_or(0);
    let total = n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens") + n("output_tokens");
    (total > 0).then_some(total)
}

/// A saída de `claude mcp list`: "nome: alvo - ✔ Connected". Do alvo (que pode ter chave) só se lê o fim.
pub(crate) fn parse_mcp_list(text: &str) -> Vec<Server> {
    text.lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once(": ")?;
            let (_, state) = rest.rsplit_once(" - ")?;
            Some(Server { name: name.trim().to_string(), status: Status::of(state) })
        })
        .collect()
}

/// Os valores de "Compactar automaticamente em"; vazio é o padrão do modelo.
pub(crate) const COMPACT_CHOICES: [&str; 8] = ["100k", "200k", "300k", "400k", "500k", "600k", "800k", "1M"];

/// O `--autocompact` do Claude Code, quando o chat escolheu um valor.
pub(crate) fn autocompact_args(prefs: &Prefs) -> Vec<String> {
    match prefs.chat_autocompact.as_str() {
        v if COMPACT_CHOICES.contains(&v) => vec!["--autocompact".into(), v.into()],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // o `result` de `claude -p "/context"` (2.1.322), com poucas linhas de ferramentas
    const CONTEXT: &str = "## Context Usage\n\n**Model:** claude-fable-5-2  \n**Tokens:** 301.8k / 400k (75%)\n\n### Estimated usage by category\n\n| Category | Tokens | Percentage |\n|----------|--------|------------|\n| System prompt | 4.1k | 1.0% |\n| System tools | 26.7k | 6.7% |\n| MCP tools | 12k | 3.0% |\n| MCP tools (deferred) | 188.8k | 47.2% |\n| Memory files | 21 | 0.0% |\n| Messages | 221k | 55.2% |\n| Free space | 65.2k | 16.3% |\n| Autocompact buffer | 33k | 8.3% |\n\n### MCP Tools\n\n| Tool | Server | Tokens |\n|------|--------|--------|\n| mcp__claude_ai_Gmail__reply | claude_ai_Gmail | 2k |\n| mcp__rea__xrefs | rea | 242 |\n\n### Memory Files\n\n| Type | Path | Tokens |\n|------|------|--------|\n| Project | C:\\proj\\CLAUDE.md | 21 |\n\n### Skills\n\n| Skill | Source | Tokens |\n|-------|--------|--------|\n| dataviz | Built-in | ~480 |\n";

    #[test]
    fn the_context_report_is_read_by_category() {
        let r = parse_context(CONTEXT).unwrap();
        assert_eq!((r.model.as_str(), r.used, r.window), ("claude-fable-5-2", 301_800, 400_000));
        assert_eq!(r.cats.len(), 8);
        assert_eq!(r.cats[5], Cat { name: "Messages".into(), tokens: 221_000, pct: 55.2 });
        assert_eq!(r.cat("MCP tools (deferred)"), 188_800);
        assert_eq!(
            r.tools,
            [
                ToolRow { server: "claude_ai_Gmail".into(), name: "reply".into(), tokens: 2000 },
                ToolRow { server: "rea".into(), name: "xrefs".into(), tokens: 242 },
            ]
        );
        assert_eq!(r.memory, [("C:\\proj\\CLAUDE.md".to_string(), 21)]);
        // 400k - 33k de buffer - 301.8k usados: os "65k até o auto-compact" do painel
        assert_eq!(short(r.until_compact()), "65.2k");
        assert!(parse_context("nada de útil").is_err());
    }

    #[test]
    fn numbers_read_and_print_like_the_panel() {
        assert_eq!(parse_tokens("~140"), Some(140));
        assert_eq!(parse_tokens("26.7k"), Some(26_700));
        assert_eq!(parse_tokens("1.2M"), Some(1_200_000));
        assert_eq!(parse_tokens("—"), None);
        assert_eq!([short(622), short(221_000), short(301_800), short(1_200_000), short(0)], ["622", "221k", "301.8k", "1.2M", "0"]);
        assert_eq!((percent(55.2), percent(0.0), percent(8.26)), ("55,2%".into(), "0,0%".into(), "8,3%".into()));
    }

    #[test]
    fn the_stream_gives_the_connectors_and_the_usage() {
        let init = json!({"type": "system", "subtype": "init", "model": "claude-fable-5-2",
            "tools": ["Bash", "mcp__rea__xrefs", "mcp__rea__open_binary", "mcp__claude_ai_Gmail__reply"],
            "mcp_servers": [{"name": "rea", "status": "connected"}, {"name": "claude.ai Gmail", "status": "needs-auth"},
                {"name": "phpstorm", "status": "failed"}, {"name": "windows-mcp", "status": "pending"}]});
        let init = parse_init(&init).unwrap();
        let states: Vec<Status> = init.servers.iter().map(|s| s.status).collect();
        assert_eq!(states, [Status::Connected, Status::NeedsAuth, Status::Failed, Status::Pending]);
        assert_eq!(init.mcp_tools.len(), 3);
        assert_eq!(tool_name("mcp__claude_ai_Gmail__reply", "claude.ai Gmail"), "reply");
        assert!(parse_init(&json!({"type": "assistant"})).is_none());
        let usage = json!({"type": "assistant", "parent_tool_use_id": null, "message": {"usage":
            {"input_tokens": 2, "cache_creation_input_tokens": 16678, "cache_read_input_tokens": 19911, "output_tokens": 4}}});
        assert_eq!(context_used(&usage), Some(36_595));
        let sub = json!({"type": "assistant", "parent_tool_use_id": "t1", "message": {"usage": {"input_tokens": 9}}});
        assert_eq!(context_used(&sub), None);
    }

    #[test]
    fn the_mcp_list_keeps_names_and_states_only() {
        let text = "Checking MCP server health…\n\nclaude.ai Gmail: https://gmail.example/mcp - ✔ Connected\nplugin:playwright:playwright: npx @playwright/mcp@latest - ✔ Connected\nphpstorm: http://127.0.0.1:1/stream (HTTP) - ✘ Failed to connect — ECONNREFUSED\nnotion: https://x.example/mcp?key=SEGREDO - ! Needs authentication\n";
        let list = parse_mcp_list(text);
        let seen: Vec<(&str, Status)> = list.iter().map(|s| (s.name.as_str(), s.status)).collect();
        assert_eq!(
            seen,
            [
                ("claude.ai Gmail", Status::Connected),
                ("plugin:playwright:playwright", Status::Connected),
                ("phpstorm", Status::Failed),
                ("notion", Status::NeedsAuth),
            ]
        );
    }

    #[test]
    fn auto_compact_goes_to_the_cli_only_for_a_known_value() {
        let mut prefs = Prefs::default();
        assert!(autocompact_args(&prefs).is_empty());
        prefs.chat_autocompact = "200k".into();
        assert_eq!(autocompact_args(&prefs), ["--autocompact", "200k"]);
        prefs.chat_autocompact = "x; rm".into();
        assert!(autocompact_args(&prefs).is_empty());
        assert_eq!(print_result(r#"{"is_error":false,"result":"oi"}"#), Ok("oi".into()));
        assert!(print_result(r#"{"is_error":true,"result":"falhou"}"#).is_err());
    }
}
