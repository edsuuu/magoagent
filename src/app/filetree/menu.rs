//! The file tree's menus, as the reference app draws them: the row's right-click menu, the view
//! options and the small name popover under a row (new file, new folder, rename). Sizes are the
//! reference's at 13 px to the rem: a menu is 265 wide (the options 202, their submenu 148), a row
//! 26 high, the popover 208 wide.

use crate::app::filetree::ui::{Act, outline};
use crate::app::filetree::*;
use crate::app::ui::menu_look::{ICON_SIZE, MENU_PAD, MENU_TEXT_SIZE, MenuIcon, ROW_GAP, ROW_HEIGHT, ROW_PAD, SEP_HEIGHT, menu_colors, paint_menu_icon_sized};
use crate::prelude::*;
use std::path::Path;

pub(crate) const CONTEXT_WIDTH: f32 = 265.0;
pub(crate) const VIEW_WIDTH: f32 = 202.0;
const SORT_WIDTH: f32 = 148.0;
const POPOVER_WIDTH: f32 = 208.0;
/// A menu's own padding and border, off its width.
const FRAME: f32 = MENU_PAD + 1.0;

/// The reference's file shortcuts as its menus write them: on the Mac the glyphs (⌥⇧⌘C), elsewhere
/// the words in the order of its keycaps (Alt+Shift+Ctrl+C), the forward delete as ⌦.
pub(crate) fn keys(action: &str, mac: bool) -> &'static str {
    match (action, mac) {
        ("copy", true) => "⌘C",
        ("copy", false) => "Ctrl+C",
        ("cut", true) => "⌘X",
        ("cut", false) => "Ctrl+X",
        ("paste", true) => "⌘V",
        ("paste", false) => "Ctrl+V",
        ("copyPath", true) => "⌥⌘C",
        ("copyPath", false) => "Alt+Ctrl+C",
        ("copyRelativePath", true) => "⌥⇧⌘C",
        ("copyRelativePath", false) => "Alt+Shift+Ctrl+C",
        ("newFile", true) => "⌘N",
        ("newFile", false) => "Ctrl+N",
        ("newFolder", true) => "⌥⌘N",
        ("newFolder", false) => "Alt+Ctrl+N",
        ("rename", true) => "⌘↩",
        ("rename", false) => "F2",
        ("diff", true) => "⌘D",
        ("diff", false) => "Ctrl+D",
        (_, true) => "⌘⌫",
        _ => "⌦",
    }
}

fn shortcut(action: &str) -> &'static str {
    keys(action, cfg!(target_os = "macos"))
}

/// What a row's drawing is of: one of the app's menu icons or one of lucide's drawn here.
#[derive(Clone, Copy)]
pub(crate) enum G {
    Menu(MenuIcon),
    FolderOpen,
    Link,
    CornerDownRight,
    Scissors,
    PenLine,
    FolderPlus,
    FilePlus,
    List,
    ArrowUpDown,
    ArrowUpToLine,
    PanelRight,
}

/// Points of an arc on lucide's 24-grid, in degrees clockwise from east.
pub(crate) fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    (0..=16)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / 16.0).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// `g` in a square of `size` at `c`: lucide's 24-unit drawings at 2 units of stroke.
pub(crate) fn paint_g(p: &Painter, g: G, c: Pos2, size: f32, color: Color32) {
    let stroke = Stroke::new(2.0 * size / 24.0, color);
    let line = |pts: &[(f32, f32)]| outline(p, c, size, pts, false, stroke);
    let shape = |pts: &[(f32, f32)]| outline(p, c, size, pts, true, stroke);
    match g {
        G::Menu(icon) => paint_menu_icon_sized(p, icon, c, color, size),
        G::FolderOpen => {
            line(&[(6.0, 14.0), (7.5, 11.1), (9.24, 10.0), (20.0, 10.0), (21.94, 12.5), (20.4, 18.5), (18.45, 20.0), (4.0, 20.0), (2.0, 18.0), (2.0, 5.0)]);
            line(&[(2.0, 5.0), (4.0, 3.0), (7.9, 3.0), (9.6, 3.9), (10.4, 5.1), (12.07, 6.0), (18.0, 6.0), (20.0, 8.0), (20.0, 10.0)]);
        }
        G::Link => {
            let mut left = vec![(9.0, 17.0), (7.0, 17.0)];
            left.extend(arc(7.0, 12.0, 5.0, 90.0, 270.0));
            left.push((9.0, 7.0));
            let mut right = vec![(15.0, 7.0), (17.0, 7.0)];
            right.extend(arc(17.0, 12.0, 5.0, 270.0, 450.0));
            right.push((15.0, 17.0));
            line(&left);
            line(&right);
            line(&[(8.0, 12.0), (16.0, 12.0)]);
        }
        G::CornerDownRight => {
            line(&[(15.0, 10.0), (20.0, 15.0), (15.0, 20.0)]);
            let mut pts = vec![(4.0, 4.0), (4.0, 11.0)];
            pts.extend(arc(8.0, 11.0, 4.0, 180.0, 90.0));
            pts.push((20.0, 15.0));
            line(&pts);
        }
        G::Scissors => {
            line(&arc(6.0, 6.0, 3.0, 0.0, 360.0));
            line(&arc(6.0, 18.0, 3.0, 0.0, 360.0));
            line(&[(20.0, 4.0), (8.12, 15.88)]);
            line(&[(14.47, 14.48), (20.0, 20.0)]);
            line(&[(8.12, 8.12), (12.0, 12.0)]);
        }
        G::PenLine => {
            line(&[(13.0, 21.0), (21.0, 21.0)]);
            shape(&[(21.17, 6.81), (17.19, 2.83), (3.84, 16.17), (3.34, 17.0), (2.02, 21.35), (2.64, 21.97), (6.99, 20.65), (7.82, 20.15)]);
        }
        G::FolderPlus => {
            line(&[(12.0, 10.0), (12.0, 16.0)]);
            line(&[(9.0, 13.0), (15.0, 13.0)]);
            shape(&[
                (4.0, 20.0),
                (20.0, 20.0),
                (22.0, 18.0),
                (22.0, 8.0),
                (20.0, 6.0),
                (12.1, 6.0),
                (10.4, 5.1),
                (9.6, 3.9),
                (7.9, 3.0),
                (4.0, 3.0),
                (2.0, 5.0),
                (2.0, 18.0),
            ]);
        }
        G::FilePlus => {
            shape(&[(6.0, 22.0), (4.0, 20.0), (4.0, 4.0), (6.0, 2.0), (14.0, 2.0), (15.7, 2.7), (19.3, 6.3), (20.0, 8.0), (20.0, 20.0), (18.0, 22.0)]);
            line(&[(14.0, 2.0), (14.0, 7.0), (15.0, 8.0), (20.0, 8.0)]);
            line(&[(9.0, 15.0), (15.0, 15.0)]);
            line(&[(12.0, 18.0), (12.0, 12.0)]);
        }
        G::List => {
            for y in [6.0, 12.0, 18.0] {
                line(&[(8.0, y), (21.0, y)]);
                line(&[(3.0, y), (3.01, y)]);
            }
        }
        G::ArrowUpDown => {
            line(&[(21.0, 16.0), (17.0, 20.0), (13.0, 16.0)]);
            line(&[(17.0, 20.0), (17.0, 4.0)]);
            line(&[(3.0, 8.0), (7.0, 4.0), (11.0, 8.0)]);
            line(&[(7.0, 4.0), (7.0, 20.0)]);
        }
        G::PanelRight => {
            shape(&[(5.0, 3.0), (19.0, 3.0), (21.0, 5.0), (21.0, 19.0), (19.0, 21.0), (5.0, 21.0), (3.0, 19.0), (3.0, 5.0)]);
            line(&[(15.0, 3.0), (15.0, 21.0)]);
        }
        G::ArrowUpToLine => {
            line(&[(5.0, 3.0), (19.0, 3.0)]);
            line(&[(18.0, 13.0), (12.0, 7.0), (6.0, 13.0)]);
            line(&[(12.0, 7.0), (12.0, 21.0)]);
        }
    }
}

/// What a row of a menu has at its left, in the 26 the reference leaves for it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Lead {
    None,
    /// A radio's dot, filled when it is the one chosen.
    Dot(bool),
    /// A checkbox's tick, when it is checked.
    Tick(bool),
}

/// A row of these menus: `icon` of `icon_size`, the label, the shortcut at the right or a chevron.
pub(crate) struct Row<'a> {
    pub(crate) icon: Option<G>,
    pub(crate) size: f32,
    pub(crate) label: &'a str,
    pub(crate) keys: &'a str,
    pub(crate) lead: Lead,
    pub(crate) on: bool,
    pub(crate) arrow: bool,
    pub(crate) lit: bool,
}

impl<'a> Row<'a> {
    pub(crate) fn new(icon: Option<G>, label: &'a str) -> Self {
        Row { icon, size: ICON_SIZE, label, keys: "", lead: Lead::None, on: true, arrow: false, lit: false }
    }
}

/// Draws a row across the menu and tells what the pointer did to it.
pub(crate) fn menu_row(ui: &mut egui::Ui, row: Row) -> egui::Response {
    let colors = menu_colors(ui.visuals().dark_mode);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), if row.on { Sense::click() } else { Sense::hover() });
    let response = if row.on { response.on_hover_cursor(CursorIcon::PointingHand) } else { response };
    let fade = |c: Color32| if row.on { c } else { c.gamma_multiply(0.5) };
    let p = ui.painter();
    let lit = row.lit || (row.on && response.hovered());
    if lit {
        p.rect_filled(rect, 4, colors.hover);
    }
    let mid = rect.center().y;
    let mut x = rect.left() + ROW_PAD;
    if row.lead != Lead::None {
        let dot = Pos2::new(x + 5.69, mid);
        match row.lead {
            Lead::Dot(true) => {
                p.circle_filled(dot, 3.25, fade(colors.text));
            }
            Lead::Tick(true) => paint_menu_icon_sized(p, MenuIcon::Check, dot, fade(colors.text), ICON_SIZE),
            _ => {}
        }
        x = rect.left() + 26.0;
    }
    if let Some(icon) = row.icon {
        paint_g(p, icon, Pos2::new(x + row.size / 2.0, mid), row.size, fade(if lit { colors.text } else { colors.muted }));
        x += row.size + ROW_GAP;
    }
    p.text(Pos2::new(x, mid), egui::Align2::LEFT_CENTER, row.label, FontId::proportional(MENU_TEXT_SIZE), fade(colors.text));
    if row.arrow {
        paint_menu_icon_sized(p, MenuIcon::Chevron, Pos2::new(rect.right() - ROW_PAD - ICON_SIZE / 2.0, mid), fade(colors.muted), ICON_SIZE);
    } else if !row.keys.is_empty() {
        p.text(Pos2::new(rect.right() - ROW_PAD, mid), egui::Align2::RIGHT_CENTER, row.keys, FontId::proportional(10.0), fade(colors.muted));
    }
    response
}

/// A hairline between groups of rows, to the menu's edges (`-mx-1 my-1`).
fn separator(ui: &mut egui::Ui) {
    let colors = menu_colors(ui.visuals().dark_mode);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), SEP_HEIGHT), Sense::hover());
    ui.painter().hline(rect.left() - MENU_PAD..=rect.right() + MENU_PAD, rect.center().y, Stroke::new(1.0, colors.border));
}

/// The folder a paste or a new file goes into: the folder row itself, or the one a file is in.
pub(crate) fn target_dir(row: Option<&TreeRow>, shown: &Path) -> PathBuf {
    match row {
        Some(r) if r.dir => r.path.clone(),
        Some(r) => r.path.parent().map_or_else(|| shown.to_path_buf(), Path::to_path_buf),
        None => shown.to_path_buf(),
    }
}

/// What "Colar" says: the count of what was copied, or of what moves here.
pub(crate) fn paste_label(clip: Option<(usize, bool)>) -> String {
    match clip {
        None => "Colar".into(),
        Some((1, false)) => "Colar 1 Item".into(),
        Some((n, false)) => format!("Colar {n} Itens"),
        Some((1, true)) => "Mover 1 Item para Cá".into(),
        Some((n, true)) => format!("Mover {n} Itens para Cá"),
    }
}

/// The right-click menu of a row (`row`), or of the folder shown (None). `clip` is what the
/// clipboard holds: how many, and whether it is to move.
pub(crate) fn row_menu(ui: &mut egui::Ui, tree: &str, row: Option<&TreeRow>, shown: &Path, clip: Option<(usize, bool)>, changed: bool, acts: &mut Vec<Act>) {
    ui.set_width(CONTEXT_WIDTH - 2.0 * FRAME);
    ui.spacing_mut().item_spacing.y = 0.0;
    let dir = target_dir(row, shown);
    let item = |ui: &mut egui::Ui, icon: G, label: &str, action: &str, on: bool| {
        let clicked = menu_row(ui, Row { keys: if action.is_empty() { "" } else { shortcut(action) }, on, ..Row::new(Some(icon), label) }).clicked();
        if clicked {
            ui.close();
        }
        clicked
    };
    let ask = |kind: NameKind, text: &str| Act::Ask(TreeAsk::Name { tree: tree.to_string(), at: dir.clone(), kind, text: text.into() });
    let paste = paste_label(clip);
    if let Some(r) = row {
        if item(ui, G::FolderOpen, "Abrir", "", true) {
            acts.push(if r.dir { Act::Into(r.path.clone()) } else { Act::Activate(r.path.clone()) });
        }
        separator(ui);
        if item(ui, G::Link, "Copiar Caminho", "copyPath", true) {
            acts.push(Act::CopyPath(r.path.clone(), false));
        }
        if item(ui, G::CornerDownRight, "Copiar Caminho Relativo", "copyRelativePath", true) {
            acts.push(Act::CopyPath(r.path.clone(), true));
        }
        if changed && !r.dir {
            separator(ui);
            // a file git sees as changed: its diff against the last commit, in a tab
            if item(ui, G::ArrowUpDown, "Mostrar Diff", "diff", true) {
                acts.push(Act::Diff(r.path.clone()));
            }
        }
        separator(ui);
        if item(ui, G::Menu(MenuIcon::Copy), "Copiar", "copy", true) {
            acts.push(Act::Copy(r.path.clone()));
        }
        if item(ui, G::Scissors, "Recortar", "cut", true) {
            acts.push(Act::Cut(r.path.clone()));
        }
        if item(ui, G::Menu(MenuIcon::Paste), &paste, "paste", clip.is_some()) {
            acts.push(Act::Paste(dir.clone()));
        }
        separator(ui);
        if item(ui, G::PenLine, "Renomear", "rename", true) {
            acts.push(Act::Ask(TreeAsk::Name { tree: tree.to_string(), at: r.path.clone(), kind: NameKind::Rename, text: split_name(&r.name).0.to_string() }));
        }
        if item(ui, G::Menu(MenuIcon::Trash), "Mover para a Lixeira", "delete", true) {
            acts.push(Act::Ask(TreeAsk::Trash { tree: tree.to_string(), paths: vec![r.path.clone()] }));
        }
    } else {
        if item(ui, G::Menu(MenuIcon::Paste), &paste, "paste", clip.is_some()) {
            acts.push(Act::Paste(dir.clone()));
        }
    }
    separator(ui);
    if item(ui, G::FolderPlus, "Nova Pasta", "newFolder", true) {
        acts.push(ask(NameKind::NewFolder, "Nova Pasta"));
    }
    if item(ui, G::FilePlus, "Novo Arquivo", "newFile", true) {
        acts.push(ask(NameKind::NewFile, "Sem título"));
    }
}

/// The view menu: the reference's "Opções de Exibição". Its other views (icons, differences,
/// graph) are not here.
pub(crate) fn view_menu(ui: &mut egui::Ui, spec: &FileTreeSpec, expanded: bool, acts: &mut Vec<Act>) {
    ui.set_width(VIEW_WIDTH - 2.0 * FRAME);
    ui.spacing_mut().item_spacing.y = 0.0;
    let small = 11.375;
    // the one view
    menu_row(ui, Row { size: small, lead: Lead::Dot(true), ..Row::new(Some(G::List), "Visualização em Lista") });
    separator(ui);
    let label = if spec.editor { "Ocultar Editor" } else { "Mostrar Editor" };
    if menu_row(ui, Row { size: small, ..Row::new(Some(G::PanelRight), label) }).clicked() {
        acts.push(Act::ToggleEditor);
        ui.close();
    }
    separator(ui);
    let (eye, label) = if spec.hide_hidden { (MenuIcon::EyeOff, "Mostrar Arquivos Ocultos") } else { (MenuIcon::Eye, "Ocultar Arquivos Ocultos") };
    if menu_row(ui, Row { size: small, ..Row::new(Some(G::Menu(eye)), label) }).clicked() {
        acts.push(Act::Hidden);
        ui.close();
    }
    separator(ui);
    sort_menu(ui, spec, small, acts);
    separator(ui);
    if menu_row(ui, Row { size: small, on: expanded, ..Row::new(Some(G::ArrowUpToLine), "Recolher Tudo") }).clicked() {
        acts.push(Act::CollapseAll);
        ui.close();
    }
}

/// "Ordenar por" and its submenu, which opens level with the row's top edge.
fn sort_menu(ui: &mut egui::Ui, spec: &FileTreeSpec, small: f32, acts: &mut Vec<Act>) {
    use egui::containers::menu::{MenuConfig, SubMenu};
    let mut button = menu_row(ui, Row { size: small, arrow: true, ..Row::new(Some(G::ArrowUpDown), "Ordenar por") });
    let dark = ui.visuals().dark_mode;
    let colors = menu_colors(dark);
    // the submenu's box is the menu's: same surface, border and padding
    // it overlaps the menu by 4, as the reference does
    let config = MenuConfig::new().submenu_gap(-8.0).style(move |s: &mut egui::Style| {
        s.visuals.window_fill = colors.bg;
        s.visuals.window_stroke = Stroke::new(1.0, colors.border);
        s.spacing.menu_margin = egui::Margin::same(MENU_PAD as i8);
        s.spacing.item_spacing.y = 0.0;
    });
    // egui lines the first row of a submenu up with the button; the reference lines the box up
    let margin = MENU_PAD + 1.0;
    button.interact_rect = Rect::from_min_max(button.interact_rect.min + Vec2::new(0.0, margin), button.interact_rect.max - Vec2::new(0.0, margin));
    SubMenu::new().config(config).show(ui, &button, |ui| {
        ui.set_width(SORT_WIDTH - 2.0 * FRAME);
        ui.spacing_mut().item_spacing.y = 0.0;
        for by in SortBy::ALL {
            if menu_row(ui, Row { lead: Lead::Dot(spec.sort == by), ..Row::new(None, by.label()) }).clicked() {
                acts.push(Act::Sort(by));
                ui.close();
            }
        }
        separator(ui);
        if menu_row(ui, Row { lead: Lead::Tick(spec.reversed), ..Row::new(None, "Inverter Ordem") }).clicked() {
            acts.push(Act::Reverse);
            ui.close();
        }
    });
}

/// The base and the extension of a file name: the extension (with its dot) stays when renaming.
pub(crate) fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(cut) if cut > 0 => name.split_at(cut),
        _ => (name, ""),
    }
}

/// What the popover's two buttons decided: `Some(text)` to go ahead, None to leave it.
pub(crate) enum Popover {
    Open,
    Commit(String),
    Cancel,
}

/// The small popover under a row that asks for a name: a field, "Cancelar" and the button that does
/// it. `anchor` is the row it hangs from.
pub(crate) fn name_popover(ctx: &Context, ask: &mut TreeAsk, anchor: Rect, dark: bool) -> Popover {
    let TreeAsk::Name { at, kind, text, .. } = ask else { return Popover::Open };
    let c = menu_colors(dark);
    let uc = UiColors::new(dark);
    let accent = if dark { Color32::from_rgb(0x0a, 0x84, 0xff) } else { Color32::from_rgb(0x00, 0x7a, 0xff) };
    let name = at.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let (base, ext) = if *kind == NameKind::Rename { split_name(&name) } else { ("", "") };
    let (title, field) = match kind {
        NameKind::NewFile => ("Novo Arquivo".to_string(), "Nome do novo arquivo".to_string()),
        NameKind::NewFolder => ("Nova Pasta".to_string(), "Nome da nova pasta".to_string()),
        NameKind::Rename => ("Renomear".to_string(), format!("Novo nome para {name}")),
    };
    let problem = if text.trim().is_empty() { None } else { crate::ide::fileops::name_problem(text.trim(), std::env::consts::OS) };
    let unchanged = *kind == NameKind::Rename && text.trim() == base;
    let can = !text.trim().is_empty() && problem.is_none() && !unchanged;
    let first_id = egui::Id::unique("ft_name_popover");
    let first = !ctx.data(|d| d.get_temp::<bool>(first_id).unwrap_or(false));
    let mut decision = Popover::Open;
    let inner = POPOVER_WIDTH - 2.0 * 9.75 - 2.0;
    let area = egui::Area::new(egui::Id::unique("ft_name_popover_area"))
        .fixed_pos(anchor.left_bottom() + Vec2::new(0.0, 6.0))
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::new().fill(c.bg).stroke(Stroke::new(1.0, c.border)).corner_radius(6).inner_margin(9.75).show(ui, |ui| {
                ui.set_width(inner);
                ui.spacing_mut().item_spacing = Vec2::new(6.5, 6.5);
                ui.visuals_mut().selection.bg_fill = accent;
                ui.visuals_mut().selection.stroke = Stroke::new(1.0, Color32::WHITE);
                ui.label(egui::RichText::new(&title).size(12.5).color(c.text));
                ui.horizontal(|ui| {
                    let ext_w =
                        if ext.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(ext.to_string(), FontId::proportional(11.4), c.muted).size().x + 6.5 };
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(inner - ext_w, 26.0), Sense::hover());
                    ui.painter().rect_filled(rect, 6, if dark { Color32::from_rgb(0x1c, 0x1c, 0x1e) } else { Color32::from_rgb(0xf5, 0xf5, 0xf7) });
                    let edit = egui::TextEdit::singleline(text)
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .font(FontId::proportional(11.4))
                        .text_color(c.text)
                        .char_limit(255)
                        .desired_width(rect.width() - 16.0);
                    let response = ui.put(rect.shrink2(Vec2::new(8.0, 4.0)), edit).on_hover_text(field);
                    if first {
                        response.request_focus();
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                            let end = egui::text::CCursor::new(text.chars().count());
                            state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), end)));
                            state.store(ui.ctx(), response.id);
                        }
                    }
                    // the field is the only thing the popover has to focus: its ring stays
                    ui.painter().rect_stroke(rect, 6, Stroke::new(1.5, accent), StrokeKind::Inside);
                    if !ext.is_empty() {
                        ui.label(egui::RichText::new(ext).size(11.4).color(c.muted));
                    }
                });
                if !ext.is_empty() {
                    ui.label(egui::RichText::new(format!("Mantém a extensão {ext}.")).size(10.0).color(c.muted));
                }
                if let Some(why) = problem {
                    ui.label(egui::RichText::new(why).size(10.0).color(uc.danger));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let go = if *kind == NameKind::Rename { "Renomear" } else { "Criar" };
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.painter().layout_no_wrap(go.into(), FontId::proportional(11.4), c.text).size().x + 20.0, 24.0),
                        if can { Sense::click() } else { Sense::hover() },
                    );
                    let fill = if response.hovered() && can { accent.gamma_multiply(0.85) } else { accent };
                    ui.painter().rect_filled(rect, 6, if can { fill } else { fill.gamma_multiply(0.5) });
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        go,
                        FontId::proportional(11.4),
                        Color32::WHITE.gamma_multiply(if can { 1.0 } else { 0.6 }),
                    );
                    if can && response.clicked() {
                        decision = Popover::Commit(format!("{}{ext}", text.trim()));
                    }
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.painter().layout_no_wrap("Cancelar".into(), FontId::proportional(11.4), c.text).size().x + 20.0, 24.0),
                        Sense::click(),
                    );
                    if response.hovered() {
                        ui.painter().rect_filled(rect, 6, c.hover);
                    }
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Cancelar", FontId::proportional(11.4), c.text);
                    if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        decision = Popover::Cancel;
                    }
                });
            });
        });
    ctx.data_mut(|d| d.insert_temp(first_id, true));
    let pressed_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
    if matches!(decision, Popover::Open) {
        if ctx.input(|i| i.key_pressed(Key::Escape)) || pressed_outside {
            decision = Popover::Cancel;
        } else if can && ctx.input(|i| i.key_pressed(Key::Enter)) {
            decision = Popover::Commit(format!("{}{ext}", text.trim()));
        }
    }
    decision
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_written_as_the_reference_writes_them() {
        assert_eq!(keys("copyPath", false), "Alt+Ctrl+C");
        assert_eq!(keys("copyRelativePath", false), "Alt+Shift+Ctrl+C");
        assert_eq!(keys("delete", false), "⌦");
        assert_eq!(keys("rename", false), "F2");
        // the Mac keeps its glyphs
        assert_eq!(keys("copyPath", true), "⌥⌘C");
        assert_eq!(keys("copyRelativePath", true), "⌥⇧⌘C");
        assert_eq!(keys("delete", true), "⌘⌫");
        assert_eq!(keys("newFolder", true), "⌥⌘N");
    }

    #[test]
    fn a_rename_keeps_the_extension_and_paste_counts_what_it_holds() {
        assert_eq!(split_name("main.rs"), ("main", ".rs"));
        assert_eq!(split_name("a.tar.gz"), ("a.tar", ".gz"));
        assert_eq!(split_name(".gitignore"), (".gitignore", ""));
        assert_eq!(split_name("docs"), ("docs", ""));
        assert_eq!(paste_label(None), "Colar");
        assert_eq!(paste_label(Some((1, false))), "Colar 1 Item");
        assert_eq!(paste_label(Some((3, true))), "Mover 3 Itens para Cá");
    }
}
