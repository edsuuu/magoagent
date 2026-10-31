//! Under the chat's box: the chat's folder, model, effort and permission, each a chip that
//! changes it for the next message. The model's chip opens a menu like the desktop app's: the
//! models with what each is for and their number keys, Esforço, and Mais modelos for the older
//! ones. The chips show what the agent really runs: its own default model and effort (Claude
//! Code's settings.json, Codex's config.toml) until the chat picks others; Codex's models are
//! the ones its own cache lists.

use crate::app::chat_direct::{CHAT_MODES, Chat, folder_label};
use crate::app::chat_list::Look;
use crate::prelude::*;
use std::sync::OnceLock;

/// A model or an effort: what the agent is told, what the menu shows, and what it is for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Choice {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) note: String,
}

fn choice(id: &str, name: &str, note: &str) -> Choice {
    Choice { id: id.into(), name: name.into(), note: note.into() }
}

// ponytail: Claude's models are a fixed list (Claude Code has no list to read); update with each family
fn claude_models() -> Vec<Choice> {
    vec![
        choice("claude-opus-5-5", "Opus 5.5", "O mais capaz, para trabalho complexo"),
        choice("claude-fable-5-1", "Fable 5.1", "O Fable mais recente"),
        choice("claude-sonnet-5-5", "Sonnet 5.5", "Equilíbrio entre capacidade e velocidade"),
        choice("claude-haiku-5-5", "Haiku 5.5", "O mais rápido, para tarefas simples"),
    ]
}

fn claude_more() -> Vec<Choice> {
    [
        ("claude-haiku-4-5", "Haiku 4.5"),
        ("claude-sonnet-5", "Sonnet 5"),
        ("claude-opus-5", "Opus 5"),
        ("claude-fable-5", "Fable 5"),
        ("claude-opus-4-8", "Opus 4.8"),
        ("claude-opus-4-7", "Opus 4.7"),
        ("claude-opus-4-6", "Opus 4.6"),
        ("claude-sonnet-4-6", "Sonnet 4.6"),
    ]
    .into_iter()
    .map(|(id, name)| choice(id, name, ""))
    .collect()
}

const EFFORT_NAMES: [(&str, &str); 6] = [("low", "Baixo"), ("medium", "Médio"), ("high", "Alto"), ("xhigh", "Extra"), ("max", "Máx"), ("ultra", "Ultra")];

fn effort_name(id: &str) -> String {
    EFFORT_NAMES.iter().find(|(e, _)| *e == id).map_or_else(|| id.to_string(), |(_, n)| n.to_string())
}

/// Codex's models as its cache (~/.codex/models_cache.json) lists them, in its order, with the
/// efforts each takes; read once.
fn codex_cache() -> &'static Vec<(Choice, Vec<String>)> {
    static CACHE: OnceLock<Vec<(Choice, Vec<String>)>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let path = std::env::home_dir().map(|h| h.join(".codex").join("models_cache.json"));
        let json: Value = path.and_then(|p| fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let mut models: Vec<&Value> = json["models"].as_array().map(|a| a.iter().filter(|m| m["visibility"] == "list").collect()).unwrap_or_default();
        models.sort_by_key(|m| m["priority"].as_i64().unwrap_or(99));
        models
            .iter()
            .filter_map(|m| {
                let id = m["slug"].as_str()?;
                let name = m["display_name"].as_str().unwrap_or(id);
                let efforts = m["supported_reasoning_levels"]
                    .as_array()
                    .map(|l| l.iter().filter_map(|e| e["effort"].as_str().map(String::from)).collect())
                    .unwrap_or_default();
                Some((choice(id, name, m["description"].as_str().unwrap_or_default()), efforts))
            })
            .collect()
    })
}

pub(crate) fn models(agent: &str) -> Vec<Choice> {
    if agent != "codex" {
        return claude_models();
    }
    let cached: Vec<Choice> = codex_cache().iter().map(|(c, _)| c.clone()).collect();
    if cached.is_empty() { vec![choice("gpt-5-codex", "GPT-5 Codex", "")] } else { cached }
}

pub(crate) fn more_models(agent: &str) -> Vec<Choice> {
    if agent == "codex" { Vec::new() } else { claude_more() }
}

/// The efforts `model` takes: Codex's cache says per model; Claude's go up to Máx.
pub(crate) fn efforts(agent: &str, model: &str) -> Vec<Choice> {
    let ids: Vec<String> = match agent {
        "codex" => codex_cache()
            .iter()
            .find(|(c, _)| c.id == model)
            .map(|(_, e)| e.clone())
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| ["low", "medium", "high", "xhigh"].map(String::from).to_vec()),
        _ => ["low", "medium", "high", "xhigh", "max"].map(String::from).to_vec(),
    };
    ids.iter().map(|id| choice(id, &effort_name(id), if id == "high" { "Recomendado" } else { "" })).collect()
}

/// The agent's own default model and effort, as its settings say: Claude Code's settings.json
/// (`model`, an alias like "opus[1m]", and the model's `effortLevel`), Codex's config.toml
/// (`model`, `model_reasoning_effort`). ("", "") when it has none.
pub(crate) fn agent_default(agent: &str) -> (String, String) {
    static FOUND: OnceLock<[(String, String); 2]> = OnceLock::new();
    let found = FOUND.get_or_init(|| {
        let home = std::env::home_dir().unwrap_or_default();
        let settings: Value =
            fs::read_to_string(home.join(".claude").join("settings.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let claude_model = claude_alias(settings["model"].as_str().unwrap_or_default());
        let claude_effort =
            settings["modelSettings"][&claude_model]["effortLevel"].as_str().or(settings["effortLevel"].as_str()).unwrap_or_default().to_string();
        let config = fs::read_to_string(home.join(".codex").join("config.toml")).unwrap_or_default();
        // the top-level keys, before the first [table]
        let top = |key: &str| {
            config
                .lines()
                .take_while(|l| !l.trim_start().starts_with('['))
                .find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().trim_matches('"').to_string()))
                .unwrap_or_default()
        };
        [(claude_model, claude_effort), (top("model"), top("model_reasoning_effort"))]
    });
    found[(agent == "codex") as usize].clone()
}

/// Claude Code's aliases to the model they run now: "opus[1m]" → claude-opus-5-5.
fn claude_alias(model: &str) -> String {
    let base = model.split('[').next().unwrap_or(model).trim();
    match base {
        "opus" | "default" | "best" => "claude-opus-5-5",
        "sonnet" => "claude-sonnet-5-5",
        "haiku" => "claude-haiku-5-5",
        "fable" => "claude-fable-5-1",
        other => other,
    }
    .to_string()
}

/// The name the menu gives `id`; the id itself when it isn't on the lists.
pub(crate) fn label(list: &[Choice], id: &str) -> String {
    list.iter().find(|c| c.id == id).map_or_else(|| id.to_string(), |c| c.name.clone())
}

/// The agent's flags for a model and an effort; none when they are its defaults.
pub(crate) fn tuning_args(agent: &str, model: &str, effort: &str) -> Vec<String> {
    let mut args = Vec::new();
    if !model.is_empty() {
        args.extend([if agent == "codex" { "-m" } else { "--model" }.to_string(), model.to_string()]);
    }
    if !effort.is_empty() {
        if agent == "codex" {
            args.extend(["-c".to_string(), format!("model_reasoning_effort={effort}")]);
        } else {
            args.extend(["--effort".to_string(), effort.to_string()]);
        }
    }
    args
}

/// A row of the menu: the name and, under it, what it is for; the key at the right and a check
/// when it is the one.
fn row(ui: &mut egui::Ui, name: &str, note: &str, on: bool, key: &str, c: &UiColors) -> bool {
    let height = if note.is_empty() { 30.0 } else { 44.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect, 6, c.surface_high);
    }
    let left = rect.left() + 12.0;
    if note.is_empty() {
        p.text(Pos2::new(left, rect.center().y), egui::Align2::LEFT_CENTER, name, FontId::proportional(14.0), c.text);
    } else {
        p.text(Pos2::new(left, rect.top() + 8.0), egui::Align2::LEFT_TOP, name, FontId::proportional(14.0), c.text);
        p.text(Pos2::new(left, rect.top() + 26.0), egui::Align2::LEFT_TOP, note, FontId::proportional(11.5), c.muted);
    }
    let mut right = rect.right() - 10.0;
    if on {
        paint_icon(p, Icon::Check, Pos2::new(right - 6.0, rect.center().y), c.text);
    }
    right -= 22.0;
    if !key.is_empty() {
        p.text(Pos2::new(right, rect.center().y), egui::Align2::RIGHT_CENTER, key, FontId::proportional(12.5), c.muted);
    }
    response.clicked()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FooterAct {
    Folder,
    Model(String),
    Effort(String),
    Mode(usize),
}

/// What picking `id` stores: "" when it is the agent's default (the chat keeps following it).
fn pick(id: &str, default: &str) -> String {
    if id == default { String::new() } else { id.to_string() }
}

impl DrawingApp {
    /// The chips, left to right, on `row`; what was picked is applied outside the closure
    /// (`chat_footer_act`).
    pub(crate) fn chat_footer(&self, ui: &mut egui::Ui, row_rect: Rect, chat: &Chat, look: &Look) -> Option<FooterAct> {
        let c = self.colors();
        let fixed = chat.session.is_some();
        let folder_hint = if fixed { "A pasta fica fixa depois da primeira mensagem" } else { "Pasta de trabalho · clique para mudar" };
        let (default_model, default_effort) = agent_default(&chat.agent);
        let model_id = if chat.model.is_empty() { default_model.clone() } else { chat.model.clone() };
        let effort_id = if chat.effort.is_empty() { default_effort.clone() } else { chat.effort.clone() };
        let (list, more) = (models(&chat.agent), more_models(&chat.agent));
        let all: Vec<Choice> = list.iter().chain(&more).cloned().collect();
        let effort_list = efforts(&chat.agent, &model_id);
        let model = if model_id.is_empty() { "Modelo padrão".to_string() } else { label(&all, &model_id) };
        let effort = if effort_id.is_empty() { "Esforço padrão".to_string() } else { format!("Esforço {}", effort_name(&effort_id)) };
        let mode = CHAT_MODES[chat.mode.min(CHAT_MODES.len() - 1)].to_string();
        let chips = [
            ("folder", format!("Pasta {}", folder_label(&chat.cwd)), folder_hint, !fixed),
            ("model", model, "Modelo do agente", true),
            ("effort", effort, "Esforço do agente", true),
            ("mode", mode, "O que o agente pode fazer", true),
        ];
        let font = FontId::proportional(12.0);
        let mut x = row_rect.left() + 10.0;
        let mut act = None;
        for (what, text, hint, enabled) in chips {
            let galley = ui.painter().layout_no_wrap(text, font.clone(), look.muted);
            let chip = Rect::from_min_size(Pos2::new(x, row_rect.top()), Vec2::new(galley.size().x + 12.0, row_rect.height()));
            // the paste hint keeps the right end of the row
            if chip.right() > row_rect.right() - 230.0 {
                break;
            }
            let sense = if enabled { Sense::click() } else { Sense::hover() };
            let response = ui.interact(chip, ui.scope_id().with(("chat_footer", what)), sense).on_hover_text(hint);
            if enabled && response.hovered() {
                ui.painter().rect_filled(chip, 8.0, look.raised);
                response.clone().on_hover_cursor(CursorIcon::PointingHand);
            }
            let color = if enabled { look.soft } else { look.muted };
            ui.painter().galley(Pos2::new(chip.left() + 6.0, chip.center().y - galley.size().y / 2.0), galley, color);
            x = chip.right() + 4.0;
            if what == "folder" {
                if response.clicked() {
                    act = Some(FooterAct::Folder);
                }
                continue;
            }
            // the others open a menu over the chip
            egui::Popup::menu(&response).show(|ui| {
                ui.set_min_width(if what == "model" { 280.0 } else { 190.0 });
                ui.spacing_mut().item_spacing.y = 1.0;
                let efforts_menu = |ui: &mut egui::Ui, act: &mut Option<FooterAct>| {
                    for e in &effort_list {
                        if row(ui, &e.name, "", e.id == effort_id, &e.note, &c) {
                            *act = Some(FooterAct::Effort(pick(&e.id, &default_effort)));
                            ui.close();
                        }
                    }
                };
                match what {
                    "model" => {
                        for (k, m) in list.iter().enumerate() {
                            let key = if k < 9 { (k + 1).to_string() } else { String::new() };
                            if row(ui, &m.name, &m.note, m.id == model_id, &key, &c) {
                                act = Some(FooterAct::Model(pick(&m.id, &default_model)));
                                ui.close();
                            }
                        }
                        menu_line(ui, &c);
                        let current = if effort_id.is_empty() { "Padrão".to_string() } else { effort_name(&effort_id) };
                        ui.menu_button(format!("Esforço   {current}"), |ui| {
                            ui.set_min_width(170.0);
                            efforts_menu(ui, &mut act);
                        });
                        if !more.is_empty() {
                            ui.menu_button("Mais modelos", |ui| {
                                ui.set_min_width(150.0);
                                for m in &more {
                                    if row(ui, &m.name, "", m.id == model_id, "", &c) {
                                        act = Some(FooterAct::Model(pick(&m.id, &default_model)));
                                        ui.close();
                                    }
                                }
                            });
                        }
                    }
                    "effort" => efforts_menu(ui, &mut act),
                    _ => {
                        for (k, name) in CHAT_MODES.iter().enumerate() {
                            if row(ui, name, "", chat.mode == k, "", &c) {
                                act = Some(FooterAct::Mode(k));
                                ui.close();
                            }
                        }
                    }
                }
            });
        }
        act
    }

    /// A chip's pick: the folder opens the picker; the others set the chat's next message.
    pub(crate) fn chat_footer_act(&mut self, act: FooterAct) {
        if act == FooterAct::Folder {
            if let Some(dir) = rfd::FileDialog::new().set_title("Pasta de trabalho").pick_folder() {
                self.chat_set_folder(&dir.to_string_lossy());
            }
            return;
        }
        let Some(chat) = self.current_chat_mut() else { return };
        match act {
            FooterAct::Model(model) => chat.model = model,
            FooterAct::Effort(effort) => chat.effort = effort,
            FooterAct::Mode(mode) => chat.mode = mode.min(CHAT_MODES.len() - 1),
            FooterAct::Folder => {}
        }
        self.save_chats();
    }

    /// The chat's folder, while no message went (the agent's session lives in the folder).
    pub(crate) fn chat_set_folder(&mut self, dir: &str) {
        let Some(chat) = self.current_chat_mut() else { return };
        if chat.session.is_some() {
            return;
        }
        chat.cwd = dir.trim().to_string();
        let agent = chat.name.split(" · ").next().unwrap_or(&chat.agent).to_string();
        chat.name = format!("{agent} · {}", folder_label(&chat.cwd));
        self.save_chats();
    }

    fn current_chat_mut(&mut self) -> Option<&mut Chat> {
        let id = self.chat_agent.clone()?;
        self.chats.iter_mut().find(|c| c.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat_direct::NewChat;

    #[test]
    fn the_chips_change_the_next_message_and_the_folder_locks_after_the_first() {
        let mut app = DrawingApp::default();
        let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
        app.chat_footer_act(FooterAct::Model("claude-opus-5-5".into()));
        app.chat_footer_act(FooterAct::Effort("medium".into()));
        app.chat_footer_act(FooterAct::Mode(1));
        let chat = app.chats[0].clone();
        assert_eq!((chat.model.as_str(), chat.effort.as_str(), chat.mode), ("claude-opus-5-5", "medium", 1));
        assert_eq!(tuning_args("claude", &chat.model, &chat.effort), ["--model", "claude-opus-5-5", "--effort", "medium"]);
        assert_eq!(tuning_args("codex", "gpt-5", "high"), ["-m", "gpt-5", "-c", "model_reasoning_effort=high"]);
        assert_eq!(tuning_args("claude", "", ""), Vec::<String>::new());
        // the menu names them; an id off the lists shows as it is
        assert_eq!(label(&more_models("claude"), "claude-opus-4-7"), "Opus 4.7");
        assert_eq!(label(&models("claude"), "claude-x"), "claude-x");
        // the agent's own default, by its alias; picking it keeps following it
        assert_eq!(claude_alias("opus[1m]"), "claude-opus-5-5");
        assert_eq!(claude_alias("claude-sonnet-4-6"), "claude-sonnet-4-6");
        assert_eq!((pick("claude-opus-5-5", "claude-opus-5-5"), pick("claude-haiku-5-5", "claude-opus-5-5")), (String::new(), "claude-haiku-5-5".into()));
        assert_eq!(efforts("claude", "").iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Baixo", "Médio", "Alto", "Extra", "Máx"]);
        app.chat_footer_act(FooterAct::Model(String::new()));
        assert_eq!(app.chats[0].model, "");
        app.chat_set_folder("C:\\trabalho\\loja");
        assert_eq!((app.chats[0].cwd.as_str(), app.chats[0].name.as_str()), ("C:\\trabalho\\loja", "Claude Code · loja"));
        app.chats[0].session = Some("s1".into());
        app.chat_set_folder("~/outra");
        assert_eq!(app.chats[0].cwd, "C:\\trabalho\\loja", "fixed once the session exists");
    }
}
