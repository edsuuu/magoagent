//! Conversas' Exibição, the button beside + and «: how the list shows the chats, kept with the preferences
//! (`Prefs::chat_list`). Comfortable rows or compact ones (one line each: a smaller wizard, the name, a dot for the state
//! and the time), in sections by space, by folder or by day, or in one list, in the order they were made, the most recent
//! first or by name, every agent or only Claude Code's or Codex's, and with Luna (the mini wizard) on each row or not. The
//! MCP's `set_setting` changes them too (`chat_list_*`).

use super::running::Entry;
use super::{ACCENT, Look};
use crate::app::ui::menu_look::{MENU_MIN, MENU_PAD, MenuIcon, ROW_PAD, SEP_HEIGHT, menu_choice, menu_colors, menu_frame, paint_menu_icon_sized};
use crate::prelude::*;

/// How Conversas lists the chats; the defaults are the list as it always was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ListPrefs {
    /// One line a chat, without the last line under the name.
    pub(crate) compact: bool,
    /// In sections by `group` ("PRINCIPAL", "MAGOAGENT", "HOJE"); off, one list. Earlier builds kept it as `by_space`.
    #[serde(alias = "by_space")]
    pub(crate) grouped: bool,
    pub(crate) group: ListGroup,
    pub(crate) order: ListOrder,
    pub(crate) agent: AgentFilter,
    /// Luna, the mini wizard at the left of each row; off, the name starts there.
    pub(crate) luna: bool,
}

impl Default for ListPrefs {
    fn default() -> Self {
        ListPrefs { compact: false, grouped: true, group: ListGroup::Space, order: ListOrder::Made, agent: AgentFilter::All, luna: true }
    }
}

/// What the sections of the list are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ListGroup {
    #[default]
    Space,
    /// The folder each one works in.
    Folder,
    /// The day it last changed, the newest first.
    Day,
}

impl ListGroup {
    pub(crate) const ALL: [ListGroup; 3] = [ListGroup::Space, ListGroup::Folder, ListGroup::Day];

    pub(crate) fn label(self) -> &'static str {
        match self {
            ListGroup::Space => "Espaço",
            ListGroup::Folder => "Pasta",
            ListGroup::Day => "Dia",
        }
    }

    /// The MCP's name for it.
    pub(crate) fn key(self) -> &'static str {
        match self {
            ListGroup::Space => "space",
            ListGroup::Folder => "folder",
            ListGroup::Day => "day",
        }
    }

    /// By the MCP's name or the menu's.
    pub(crate) fn named(name: &str) -> Option<ListGroup> {
        let name = name.trim().to_lowercase();

        ListGroup::ALL.into_iter().find(|group| group.key() == name || group.label().to_lowercase() == name || (name == "espaco" && *group == ListGroup::Space))
    }
}

/// The order of the rows (in each space, when grouped).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ListOrder {
    /// As they were made, the oldest first.
    #[default]
    Made,
    /// By the time at the right of the row, the newest first.
    Recent,
    Name,
}

impl ListOrder {
    pub(crate) const ALL: [ListOrder; 3] = [ListOrder::Made, ListOrder::Recent, ListOrder::Name];

    pub(crate) fn label(self) -> &'static str {
        match self {
            ListOrder::Made => "Criação",
            ListOrder::Recent => "Recentes",
            ListOrder::Name => "Nome",
        }
    }

    /// The MCP's name for it.
    pub(crate) fn key(self) -> &'static str {
        match self {
            ListOrder::Made => "created",
            ListOrder::Recent => "recent",
            ListOrder::Name => "name",
        }
    }

    /// By the MCP's name or the menu's.
    pub(crate) fn named(name: &str) -> Option<ListOrder> {
        let name = name.trim().to_lowercase();

        ListOrder::ALL.into_iter().find(|order| order.key() == name || order.label().to_lowercase() == name || (name == "criacao" && *order == ListOrder::Made))
    }

    /// `entries` in this order; the ones that tie keep theirs.
    pub(crate) fn sort(self, entries: &mut [&Entry]) {
        match self {
            ListOrder::Made => {}
            ListOrder::Recent => entries.sort_by(|a, b| a.ago.unwrap_or(f64::INFINITY).total_cmp(&b.ago.unwrap_or(f64::INFINITY))),
            ListOrder::Name => entries.sort_by_cached_key(|entry| entry.name.to_lowercase()),
        }
    }
}

/// Whose chats the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentFilter {
    #[default]
    All,
    Claude,
    Codex,
}

impl AgentFilter {
    pub(crate) const ALL: [AgentFilter; 3] = [AgentFilter::All, AgentFilter::Claude, AgentFilter::Codex];

    pub(crate) fn label(self) -> &'static str {
        match self {
            AgentFilter::All => "Todos",
            AgentFilter::Claude => "Claude Code",
            AgentFilter::Codex => "Codex",
        }
    }

    /// The MCP's name for it.
    pub(crate) fn key(self) -> &'static str {
        match self {
            AgentFilter::All => "all",
            AgentFilter::Claude => "claude",
            AgentFilter::Codex => "codex",
        }
    }

    /// By the MCP's name or the menu's.
    pub(crate) fn named(name: &str) -> Option<AgentFilter> {
        let name = name.trim().to_lowercase();

        AgentFilter::ALL.into_iter().find(|agent| agent.key() == name || agent.label().to_lowercase() == name)
    }

    /// Whether a row of `program` ("Claude Code", "Codex", a program outside the app) is shown.
    pub(crate) fn keeps(self, program: &str) -> bool {
        match self {
            AgentFilter::All => true,
            AgentFilter::Claude | AgentFilter::Codex => program == self.label(),
        }
    }
}

impl ListPrefs {
    /// "compacta, agrupada por pasta, por nome, só Codex, sem a Luna": the list as it shows now, for the MCP.
    pub(crate) fn summary(&self) -> String {
        let density = if self.compact { "compacta" } else { "confortável" };
        let grouped = if self.grouped { format!("agrupada por {}", self.group.label().to_lowercase()) } else { "sem grupos".into() };
        let order = format!("por {}", self.order.label().to_lowercase());
        let agent = if self.agent == AgentFilter::All { "todos os agentes".to_string() } else { format!("só {}", self.agent.label()) };
        let luna = if self.luna { "" } else { ", sem a Luna" };

        format!("{density}, {grouped}, {order}, {agent}{luna}")
    }

    /// The MCP's `set_setting` of `chat_list_density`, `chat_list_group`, `chat_list_order`, `chat_list_agent` and
    /// `chat_list_luna`.
    pub(crate) fn set(&mut self, name: &str, value: &str) -> Result<String, String> {
        let unknown = || format!("ajuste desconhecido: {name} = {value}");

        match (name, value.trim().to_lowercase().as_str()) {
            ("chat_list_density", "compact" | "compacta") => self.compact = true,
            ("chat_list_density", "comfortable" | "confortável" | "confortavel") => self.compact = false,
            ("chat_list_group", "on" | "ligado") => self.grouped = true,
            ("chat_list_group", "off" | "desligado" | "none" | "nenhum") => self.grouped = false,
            ("chat_list_group", group) => (self.grouped, self.group) = (true, ListGroup::named(group).ok_or_else(unknown)?),
            ("chat_list_luna", "on" | "ligado" | "mostrar") => self.luna = true,
            ("chat_list_luna", "off" | "desligado" | "ocultar") => self.luna = false,
            ("chat_list_order", order) => self.order = ListOrder::named(order).ok_or_else(unknown)?,
            ("chat_list_agent", agent) => self.agent = AgentFilter::named(agent).ok_or_else(unknown)?,
            _ => return Err(unknown()),
        }

        Ok(format!("Conversas agora: {}.", self.summary()))
    }
}

/// Exibição's button at `rect` and its menu, which stays open while the list changes behind it; true when `display`
/// changed. A dot on the button says a filter hides some agents' chats.
pub(super) fn display_button(ui: &mut egui::Ui, rect: Rect, display: &mut ListPrefs, look: &Look) -> bool {
    let response = ui.interact(rect, ui.scope_id().with("chat_display"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Exibição");
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&response));

    if response.hovered() || open {
        ui.painter().rect_filled(rect, 7, look.hover);
    }

    paint_menu_icon_sized(ui.painter(), MenuIcon::Sliders, rect.center(), look.soft, 16.0);

    if display.agent != AgentFilter::All {
        ui.painter().circle_filled(rect.right_top() + Vec2::new(-6.0, 6.0), 3.0, ACCENT);
    }

    let before = display.clone();
    let dark = response.ctx.global_style().visuals.dark_mode;

    egui::Popup::menu(&response)
        .align(egui::RectAlign::BOTTOM_END)
        .frame(menu_frame(dark))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| display_menu(ui, display));

    *display != before
}

/// Densidade, Agrupar, Ordenar, Agente and Mostrar a Luna, each choice with the check on the current one.
fn display_menu(ui: &mut egui::Ui, display: &mut ListPrefs) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.set_min_width(MENU_MIN + 30.0);
    heading(ui, "Densidade");

    for (compact, label) in [(false, "Confortável"), (true, "Compacta")] {
        if menu_choice(ui, display.compact == compact, label, true) {
            display.compact = compact;
        }
    }

    separator(ui);

    heading(ui, "Agrupar");

    if menu_choice(ui, !display.grouped, "Sem grupos", true) {
        display.grouped = false;
    }

    for group in ListGroup::ALL {
        if menu_choice(ui, display.grouped && display.group == group, &format!("Por {}", group.label().to_lowercase()), true) {
            (display.grouped, display.group) = (true, group);
        }
    }

    separator(ui);
    heading(ui, "Ordenar");

    for order in ListOrder::ALL {
        if menu_choice(ui, display.order == order, order.label(), true) {
            display.order = order;
        }
    }

    separator(ui);
    heading(ui, "Agente");

    for agent in AgentFilter::ALL {
        if menu_choice(ui, display.agent == agent, agent.label(), true) {
            display.agent = agent;
        }
    }

    separator(ui);

    if menu_choice(ui, display.luna, "Mostrar a Luna", true) {
        display.luna = !display.luna;
    }
}

/// A group's name over its choices, in the menu's muted ink; measured, it asks only its own width.
fn heading(ui: &mut egui::Ui, text: &str) {
    let colors = menu_colors(ui.visuals().dark_mode);
    let galley = ui.painter().layout_no_wrap(text.into(), FontId::proportional(12.0), colors.muted);
    let width = if ui.is_sizing_pass() { galley.size().x + 2.0 * ROW_PAD } else { ui.available_width() };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());

    ui.painter().galley(Pos2::new(rect.left() + ROW_PAD, rect.bottom() - galley.size().y - 3.0), galley, colors.muted);
}

/// A hairline between the groups, to the menu's edges.
fn separator(ui: &mut egui::Ui) {
    let colors = menu_colors(ui.visuals().dark_mode);
    let width = if ui.is_sizing_pass() { 0.0 } else { ui.available_width() };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, SEP_HEIGHT), Sense::hover());

    ui.painter().hline(rect.left() - MENU_PAD..=rect.right() + MENU_PAD, rect.center().y, Stroke::new(1.0, colors.border));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mcp_names_each_choice_and_refuses_the_rest() {
        let mut display = ListPrefs::default();

        assert_eq!(display.summary(), "confortável, agrupada por espaço, por criação, todos os agentes", "the list as it always was");
        display.set("chat_list_density", "compacta").unwrap();
        display.set("chat_list_group", "off").unwrap();
        display.set("chat_list_order", "nome").unwrap();
        let said = display.set("chat_list_agent", "codex").unwrap();

        assert_eq!(said, "Conversas agora: compacta, sem grupos, por nome, só Codex.");
        assert_eq!((display.order, ListOrder::named("recent"), ListOrder::named("criacao")), (ListOrder::Name, Some(ListOrder::Recent), Some(ListOrder::Made)));
        assert_eq!(AgentFilter::named("Claude Code"), Some(AgentFilter::Claude));
        assert!(display.set("chat_list_agent", "gemini").is_err() && display.set("chat_list_density", "grande").is_err());
        display.set("chat_list_group", "pasta").unwrap();
        let said = display.set("chat_list_luna", "off").unwrap();

        assert_eq!(said, "Conversas agora: compacta, agrupada por pasta, por nome, só Codex, sem a Luna.");
        assert!(display.set("chat_list_group", "dia").is_ok() && display.group == ListGroup::Day && display.set("chat_list_group", "mês").is_err());
        assert!(AgentFilter::Codex.keeps("Codex") && !AgentFilter::Codex.keeps("Claude Code") && AgentFilter::All.keeps("Cursor"));
        // an old preferences file, without them, lists as before
        let old: Prefs = serde_json::from_str("{}").unwrap();

        assert_eq!(old.chat_list, ListPrefs::default());
        // one that turned the groups off before they had kinds keeps them off
        let old: ListPrefs = serde_json::from_str(r#"{"by_space": false}"#).unwrap();

        assert!(!old.grouped && old.luna);
    }
}
