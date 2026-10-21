//! Search Everywhere (Shift Shift) with PhpStorm's tabs.

use super::find::{Find, Hit, match_text, popup_frame, rel};
use super::index::{self, Kind};
use super::*;

/// Everything the IDE can do by name, for "Localizar ação" (and its menu labels).
pub(super) const ACTIONS: &[(&str, &str, fn() -> Cmd)] = &[
    ("Salvar tudo", "Ctrl+S", || Cmd::SaveAll),
    ("Fechar aba", "Ctrl+W", || Cmd::CloseTab),
    ("Localizar no arquivo", "Ctrl+F", || Cmd::Find),
    ("Localizar nos arquivos…", "Ctrl+Shift+F", || Cmd::FindInFiles),
    ("Ir para a linha…", "Ctrl+G", || Cmd::GotoLine),
    ("Projeto", "Alt+1", || Cmd::ToggleProject),
    ("Problemas", "Alt+6", || Cmd::Tool(ToolWindow::Problems)),
    ("Banco de dados", "", || Cmd::Tool(ToolWindow::Database)),
    ("Claude Code (terminal ligado ao IDE)", "Ctrl+Esc", || Cmd::ClaudeCode),
    ("Enviar seleção ao Claude Code", "Ctrl+Alt+K", || Cmd::MentionClaude),
    ("Commit…", "Ctrl+K · Alt+0", || Cmd::Commit),
    ("Push…", "Ctrl+Shift+K", || Cmd::Push),
    ("Atualizar projeto…", "Ctrl+T", || Cmd::Pull),
    ("Comentar com comentário de linha", "Ctrl+/", || Cmd::Comment),
    ("Duplicar linha ou seleção", "Ctrl+D", || Cmd::Duplicate),
    ("Ir para a declaração", "Ctrl+B · Ctrl+Clique · F12", || Cmd::GotoDeclaration),
    ("Mostrar usos", "Alt+F7 · Ctrl+Clique na declaração", || Cmd::Usages),
    ("Ir para a implementação", "Ctrl+Alt+B", || Cmd::Implementations),
    ("Voltar", "Ctrl+Alt+Esquerda · Alt+Esquerda", || Cmd::Back),
    ("Avançar", "Ctrl+Alt+Direita · Alt+Direita", || Cmd::Forward),
];

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    All,
    Classes,
    Files,
    Symbols,
    Actions,
    Text,
}

const TABS: [(Tab, &str); 6] = [(Tab::All, "Tudo"), (Tab::Classes, "Classes"), (Tab::Files, "Arquivos"), (Tab::Symbols, "Símbolos"), (Tab::Actions, "Ações"), (Tab::Text, "Texto")];

pub struct Everywhere {
    pub tab: Tab,
    pub query: String,
    pub sel: usize,
    text: Find,
    opened: bool,
}

impl Everywhere {
    pub fn new(tab: Tab, query: &str) -> Self {
        Everywhere { tab, query: query.into(), sel: 0, text: Find::default(), opened: false }
    }
}

enum Item {
    Header(&'static str),
    Symbol(index::Symbol),
    File(PathBuf),
    Action(&'static str, &'static str, Cmd),
    Line(usize),
    Text(Hit),
}

impl Item {
    fn cmd(&self) -> Option<Cmd> {
        match self {
            Item::Header(_) => None,
            Item::Symbol(s) => Some(Cmd::Open(s.path.clone(), Some((s.decl.line, s.decl.col)))),
            Item::File(p) => Some(Cmd::Open(p.clone(), None)),
            Item::Action(_, _, c) => Some(c.clone()),
            Item::Line(l) => Some(Cmd::Line(*l)),
            Item::Text(h) => Some(Cmd::Open(h.path.clone(), Some((h.line, h.col)))),
        }
    }
}

/// Lower = better; name prefix beats substring beats subsequence (camel-hump friendly).
fn score(name: &str, q: &str) -> Option<usize> {
    if q.is_empty() {
        return Some(name.len());
    }
    let n = name.to_lowercase();
    if n.starts_with(q) {
        Some(n.len())
    } else if n.contains(q) {
        Some(1_000 + n.len())
    } else {
        let mut it = n.chars();
        q.chars().all(|c| it.any(|h| h == c)).then_some(2_000 + n.len())
    }
}

/// Round symbol badge (C, I, m, ƒ…).
fn symbol_icon(ui: &Ui, kind: Kind, c: egui::Pos2) {
    let (l, rgb) = kind.badge();
    let col = Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    ui.painter().circle(c, 7.0, col.gamma_multiply(0.22), Stroke::new(1.0, col));
    ui.painter().text(c, Align2::CENTER_CENTER, l, t::ui(9.0), col);
}

impl Ide {
    pub(crate) fn search_everywhere(&mut self, ctx: &egui::Context) {
        let Some(se) = &mut self.palette else { return };
        let q = se.query.trim().to_lowercase();
        // tab switching: Tab / Shift+Tab like PhpStorm
        let (tab_fwd, tab_back) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::Tab), i.consume_key(Modifiers::SHIFT, Key::Tab)));
        if tab_fwd || tab_back {
            let i = TABS.iter().position(|(tb, _)| *tb == se.tab).unwrap_or(0) as isize + if tab_fwd { 1 } else { -1 };
            se.tab = TABS[i.rem_euclid(TABS.len() as isize) as usize].0;
            se.sel = 0;
        }
        if se.tab == Tab::Text || se.tab == Tab::All {
            se.text.query = se.query.trim().to_string();
            let files: Vec<PathBuf> = match &self.root {
                Some(r) => self.files.iter().filter(|p| index::indexable(r, p) || highlight::detect(p).kind != highlight::Kind::Plain).cloned().collect(),
                None => vec![],
            };
            if se.tab == Tab::Text {
                se.text.tick(ctx, &files);
            }
        }
        let mut items: Vec<Item> = vec![];
        let want = |tab: Tab| se.tab == tab || se.tab == Tab::All;
        let cap = |n: usize| if se.tab == Tab::All { n } else { 200 };
        if let Some(l) = q.strip_prefix(':').and_then(|l| l.trim().parse::<usize>().ok()) {
            items.push(Item::Line(l));
        }
        let push_group = |items: &mut Vec<Item>, header: &'static str, mut v: Vec<(usize, Item)>, n: usize| {
            if v.is_empty() {
                return;
            }
            v.sort_by_key(|(s, _)| *s);
            if se.tab == Tab::All {
                items.push(Item::Header(header));
            }
            items.extend(v.into_iter().take(n).map(|(_, i)| i));
        };
        if !q.starts_with(':') {
            if want(Tab::Classes) && !q.is_empty() {
                let v = self.index.symbols.iter().filter(|s| s.decl.kind.is_type()).filter_map(|s| score(&s.decl.name, &q).map(|sc| (sc, Item::Symbol(s.clone())))).collect();
                push_group(&mut items, "Classes", v, cap(5));
            }
            if want(Tab::Files) {
                let v = self.files.iter().filter_map(|p| fuzzy(&rel(&self.root, p), &q.replace('\\', "/")).map(|sc| (sc, Item::File(p.clone())))).collect();
                push_group(&mut items, "Arquivos", v, cap(if q.is_empty() { 12 } else { 8 }));
            }
            if want(Tab::Symbols) && !q.is_empty() {
                let v = self.index.symbols.iter().filter(|s| !s.decl.kind.is_type()).filter_map(|s| score(&s.decl.name, &q).map(|sc| (sc, Item::Symbol(s.clone())))).collect();
                push_group(&mut items, "Símbolos", v, cap(5));
            }
            if want(Tab::Actions) && (!q.is_empty() || se.tab == Tab::Actions) {
                let v = ACTIONS.iter().filter_map(|(l, k, c)| score(l, &q).or_else(|| l.to_lowercase().contains(&q).then_some(500)).map(|sc| (sc, Item::Action(l, k, c())))).collect();
                push_group(&mut items, "Ações", v, cap(5));
            }
            if se.tab == Tab::Text {
                items.extend(se.text.hits.iter().take(300).cloned().map(Item::Text));
            }
        }
        let selectable: Vec<usize> = (0..items.len()).filter(|&i| !matches!(items[i], Item::Header(_))).collect();
        let (down, up, enter, esc) = ctx.input_mut(|i| (
            i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape)));
        if down {
            se.sel = (se.sel + 1).min(selectable.len().saturating_sub(1));
        }
        if up {
            se.sel = se.sel.saturating_sub(1);
        }
        se.sel = se.sel.min(selectable.len().saturating_sub(1));
        let sel_item = selectable.get(se.sel).copied();
        let mut chosen = if enter { sel_item.and_then(|i| items[i].cmd()) } else { None };
        let mut close = esc || chosen.is_some();
        let screen = self.rect;
        let w = 720.0f32.min(screen.width() - 40.0);
        let root = self.root.clone();
        let area = egui::Area::new(self.id.with("search_everywhere")).order(Order::Foreground).fixed_pos(pos2(screen.center().x - w / 2.0, screen.top() + screen.height() * 0.12))
            .show(ctx, |ui| {
                theme::style(ui.style_mut());
                popup_frame(ui).show(ui, |ui| {
                    ui.set_width(w);
                    // tabs
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.set_height(38.0);
                        for (tb, name) in TABS {
                            let g = ui.painter().layout_no_wrap(name.into(), t::ui(t::UI_SIZE), t::TEXT);
                            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 18.0, 26.0), Sense::click());
                            if se.tab == tb {
                                ui.painter().rect_filled(r, 6, t::PRESSED);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, 6, t::HOVER);
                            }
                            ui.painter().galley(r.center() - g.size() / 2.0, g, if se.tab == tb { t::TEXT } else { t::MUTED });
                            if resp.clicked() {
                                se.tab = tb;
                                se.sel = 0;
                            }
                        }
                    });
                    ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                    // query
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let (ir, _) = ui.allocate_exact_size(vec2(18.0, 40.0), Sense::hover());
                        icons::at(ui.painter(), icons::SEARCH, ir.center());
                        let hint = match se.tab {
                            Tab::All => "Digite / para arquivos, : para linha",
                            Tab::Classes => "Pesquisar classes",
                            Tab::Files => "Pesquisar arquivos",
                            Tab::Symbols => "Pesquisar símbolos",
                            Tab::Actions => "Pesquisar ações",
                            Tab::Text => "Pesquisar texto nos arquivos",
                        };
                        let r = ui.add(TextEdit::singleline(&mut se.query).hint_text(hint).frame(Frame::NONE).desired_width(f32::INFINITY).font(t::ui(15.0)));
                        r.request_focus();
                        if r.changed() {
                            se.sel = 0;
                        }
                    });
                    ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                    egui::ScrollArea::vertical().max_height(screen.height() * 0.5).auto_shrink([false, true]).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add_space(4.0);
                        for (i, it) in items.iter().enumerate() {
                            if let Item::Header(h) = it {
                                ui.horizontal(|ui| {
                                    ui.add_space(14.0);
                                    ui.set_height(24.0);
                                    ui.label(egui::RichText::new(*h).color(t::SECONDARY).size(t::UI_SIZE - 1.0));
                                });
                                continue;
                            }
                            let selected = sel_item == Some(i);
                            let (resp, rect) = row(ui, selected, 28.0);
                            let y = rect.center().y;
                            let x0 = rect.left() + 22.0;
                            let p = ui.painter();
                            let font = t::ui(t::UI_SIZE);
                            match it {
                                Item::Symbol(s) => {
                                    symbol_icon(ui, s.decl.kind, pos2(x0, y));
                                    let name = if s.decl.kind.is_type() { s.decl.name.clone() } else { format!("{}()", s.decl.name) };
                                    let g = p.text(pos2(x0 + 16.0, y), Align2::LEFT_CENTER, name, font.clone(), t::TEXT);
                                    p.text(pos2(g.right() + 10.0, y), Align2::LEFT_CENTER, rel(&root, &s.path), font.clone(), t::SECONDARY);
                                }
                                Item::File(f) => {
                                    icons::file(ui.painter(), f, pos2(x0, y), 1.0);
                                    let g = p.text(pos2(x0 + 16.0, y), Align2::LEFT_CENTER, file_name(f), font.clone(), t::TEXT);
                                    let dir = Path::new(&rel(&root, f)).parent().map(|d| d.display().to_string()).unwrap_or_default();
                                    p.text(pos2(g.right() + 10.0, y), Align2::LEFT_CENTER, dir, font.clone(), t::SECONDARY);
                                }
                                Item::Action(l, k, _) => {
                                    p.text(pos2(x0 + 16.0 - 8.0, y), Align2::LEFT_CENTER, *l, font.clone(), t::TEXT);
                                    p.text(pos2(rect.right() - 16.0, y), Align2::RIGHT_CENTER, super::chrome::keys(k), font.clone(), t::SECONDARY);
                                }
                                Item::Line(l) => {
                                    p.text(pos2(x0, y), Align2::LEFT_CENTER, format!("Ir para a linha {l}"), font.clone(), t::TEXT);
                                }
                                Item::Text(h) => {
                                    let label = format!("{} {}", file_name(&h.path), h.line + 1);
                                    let lg = p.layout_no_wrap(label.clone(), font.clone(), t::SECONDARY);
                                    match_text(ui, pos2(x0 - 8.0, y), h, rect.width() - lg.size().x - 50.0);
                                    ui.painter().text(pos2(rect.right() - 16.0, y), Align2::RIGHT_CENTER, label, font.clone(), t::SECONDARY);
                                }
                                Item::Header(_) => {}
                            }
                            if selected && (down || up) {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                chosen = it.cmd();
                                close = true;
                            }
                        }
                        if items.is_empty() {
                            ui.add_space(16.0);
                            ui.vertical_centered(|ui| {
                                let msg = if se.text.searching { "Pesquisando…" } else if q.is_empty() { "Comece a digitar para pesquisar" } else { "Nada encontrado" };
                                ui.label(egui::RichText::new(msg).color(t::MUTED));
                            });
                            ui.add_space(16.0);
                        }
                        ui.add_space(4.0);
                    });
                    ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.set_height(28.0);
                        ui.label(egui::RichText::new("Tab troca de aba · Enter abre").color(t::SECONDARY).size(t::UI_SIZE - 1.0));
                    });
                });
            });
        let opened = std::mem::replace(&mut se.opened, true);
        let outside = ctx.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
        if outside && opened {
            close = true;
        }
        if close {
            self.palette = None;
        }
        if let Some(cmd) = chosen {
            self.run(cmd);
        }
    }
}
