//! The "Banco de Dados" of a file tree node: the sources panel over the file rows (the IDE's own panel, `crate::db`),
//! and the grids and consoles it opens as tabs of the editor pane. The MCP's `db_*` actions of `file_tree` are here too.

use crate::app::filetree::editor::Body;
use crate::app::filetree::menu::arc;
use crate::app::filetree::pane::{Editor, PaneTab};
use crate::app::filetree::ui::{area, outline};
use crate::db::{Db, Event, Open, Sheet};
use crate::ide::theme as t;
use crate::prelude::*;

/// lucide's database: the top ellipse, the two sides and the two lower curves.
pub(crate) fn glyph_db(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    let flat = |cy: f32, from: f32, to: f32| arc(12.0, cy, 9.0, from, to).iter().map(|(x, y)| (*x, cy + (*y - cy) * 0.33)).collect::<Vec<_>>();
    outline(p, c, size, &flat(5.0, 0.0, 360.0), true, stroke);
    outline(p, c, size, &[(3.0, 5.0), (3.0, 19.0)], false, stroke);
    outline(p, c, size, &[(21.0, 5.0), (21.0, 19.0)], false, stroke);
    outline(p, c, size, &flat(12.0, 0.0, 180.0), false, stroke);
    outline(p, c, size, &flat(19.0, 0.0, 180.0), false, stroke);
}

/// The panel in `rect`, drawn as the IDE draws it (its own dark look); what it asks of the node, back.
pub(crate) fn db_ui(ctx: &Context, id: &str, rect: Rect, clip: Rect, z: f32, db: &mut Db, ed: Option<&Editor>) -> Vec<Event> {
    let busy = |source: &str| ed.is_some_and(|ed| ed.tabs.iter().any(|t| matches!(&t.body, Body::Sheet(s) if s.source() == source && s.busy())));
    // the IDE's widgets have sizes of their own: laid out at 100% and the layer scaled with the canvas (its pointer too)
    let scale = egui::emath::TSTransform::new(rect.min.to_vec2() * (1.0 - z), z);
    ctx.set_transform_layer(egui::LayerId::new(egui::Order::Middle, egui::Id::unique(("ft_db", id))), scale);
    let (rect, clip) = (Rect::from_min_size(rect.min, rect.size() / z), scale.inverse() * clip);
    area(ctx, ("ft_db", id), rect, clip, |ui| {
        ui.painter().with_clip_rect(rect.intersect(clip)).rect_filled(rect, 4, t::ISLAND);
        let mut events = vec![];
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect.intersect(clip));
            t::style(ui.style_mut());
            ui.horizontal(|ui| {
                ui.set_height(30.0);
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Banco de Dados").color(t::TEXT).strong());
            });
            events = db.panel(ui, ctx, &busy);
        });
        events
    })
}

/// A grid or console, as the IDE's pane draws it, in `body`.
pub(crate) fn sheet_ui(ui: &mut egui::Ui, body: Rect, sheet: &mut Sheet) {
    ui.painter().with_clip_rect(body.intersect(ui.clip_rect())).rect_filled(body.shrink2(Vec2::new(1.0, 1.0)), 0, t::ISLAND);
    ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
        ui.set_clip_rect(body.intersect(ui.clip_rect()));
        t::style(ui.style_mut());
        sheet.ui(ui);
    });
}

impl Editor {
    /// The answers of the open tables and consoles, with the pane shown or not.
    pub(crate) fn poll_sheets(&mut self, ctx: &Context) {
        for tab in &mut self.tabs {
            if let Body::Sheet(s) = &mut tab.body {
                s.poll(ctx);
            }
        }
    }

    /// Opens what the panel asked: the tab that already shows it comes forward, else `made` (a new sheet) takes a tab.
    pub(crate) fn open_sheet(&mut self, open: &Open, made: impl FnOnce() -> Option<(PathBuf, Sheet)>) {
        if let Some(tab) = self.tabs.iter().find(|t| matches!(&t.body, Body::Sheet(s) if open.is_shown_by(s))) {
            self.active = Some(tab.path.clone());
            return;
        }
        let Some((source, sheet)) = made() else { return };
        let source = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let source = source.split('@').next().unwrap_or_default().to_string();
        let base = sheet.table.as_ref().map_or_else(|| format!("Console · {source}"), |t| format!("{t} [{source}]")).replace(['/', '\\'], "∕");
        // two consoles of a source tell apart by a number
        let path = (1..)
            .map(|n| PathBuf::from(if n == 1 { base.clone() } else { format!("{base} ({n})") }))
            .find(|p| !self.tabs.iter().any(|t| &t.path == p))
            .unwrap();
        self.tabs.push(PaneTab { path: path.clone(), body: Body::Sheet(Box::new(sheet)) });
        self.active = Some(path);
    }

    /// The tabs of a source that was disconnected go.
    pub(crate) fn close_source(&mut self, source: &str) {
        self.tabs.retain(|t| !matches!(&t.body, Body::Sheet(s) if s.source() == source));
        if self.active.as_ref().is_some_and(|a| !self.tabs.iter().any(|t| &t.path == a)) {
            self.active = self.tabs.last().map(|t| t.path.clone());
        }
    }
}

impl DrawingApp {
    /// The database of tree `id`, made now if it never asked.
    pub(crate) fn ft_db(&mut self, id: &str) -> &mut Db {
        let store = self.filetrees.db_store.clone();
        self.filetrees.dbs.entry(id.to_string()).or_insert_with(|| Db::with_store(store))
    }

    /// Shows or hides the Banco de Dados panel of tree `id`.
    pub(crate) fn ft_show_db(&mut self, id: &str, show: bool) {
        if let Some(spec) = self.ft_spec_mut(id) {
            spec.db = show;
            spec.changes &= !show;
        }
    }

    /// A SQLite file the person opened: a source of the panel, which shows with it connected.
    pub(crate) fn ft_db_add(&mut self, id: &str, path: PathBuf) {
        let ctx = self.filetrees.ctx.clone().unwrap_or_default();
        match self.ft_db(id).add_sqlite(&path, &ctx) {
            Ok(()) => self.ft_show_db(id, true),
            Err(why) => self.toast(why),
        }
    }

    /// What the panel asked, done: tabs of the editor pane (shown first) or closed.
    pub(crate) fn ft_db_events(&mut self, id: &str, events: Vec<Event>) {
        let ctx = self.filetrees.ctx.clone().unwrap_or_default();
        for event in events {
            match event {
                Event::Open(open) => {
                    self.ft_show_editor(id, true);
                    let mut db = self.filetrees.dbs.remove(id).unwrap_or_default();
                    let seat = egui::Id::NULL.with(("ft_sheet", id));
                    self.ft_editor(id).open_sheet(&open, || db.new_sheet(&ctx, seat, &open));
                    self.filetrees.dbs.insert(id.to_string(), db);
                }
                Event::Closed(source) => {
                    if let Some(ed) = self.filetrees.editors.get_mut(id) {
                        ed.close_source(&source);
                    }
                }
            }
        }
    }

    /// The MCP's `db`, `db_add`, `db_sources`, `db_tables`, `db_open` and `db_query` of `file_tree`.
    pub(crate) fn act_tree_db(&mut self, id: &str, action: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let ctx = self.filetrees.ctx.clone().unwrap_or_default();
        match action {
            "db" => {
                let on = self.ft_spec(id).is_some_and(|s| s.db);
                self.ft_show_db(id, args["show"].as_bool().unwrap_or(!on));
                Ok(if self.ft_spec(id).is_some_and(|s| s.db) { "Painel Banco de Dados aberto.".into() } else { "Painel Banco de Dados fechado.".into() })
            }
            "db_add" => {
                let path = PathBuf::from(text("path"));
                let path = if path.is_absolute() { path } else { self.ft_spec(id).map(|s| s.shown().join(&path)).unwrap_or(path) };
                self.ft_db(id).add_sqlite(&path, &ctx)?;
                self.ft_show_db(id, true);
                Ok(self.ft_db(id).sources_text())
            }
            "db_sources" => Ok(self.ft_db(id).sources_text()),
            "db_tables" => self.ft_db(id).tables_text(&text("source"), &ctx),
            "db_query" => {
                let limit = args["limit"].as_u64().map_or(100, |n| n.clamp(1, 1000) as usize);
                self.ft_db(id).query_text(&text("source"), &text("query"), limit, &ctx)
            }
            // db_open: a table's grid in a tab; without `name`, a console
            _ => {
                let (source, name) = (text("source"), text("name"));
                let open = self.ft_db(id).open_named(&source, &name, &ctx)?;
                self.ft_db_events(id, vec![Event::Open(open)]);
                Ok(if name.is_empty() { "Console aberto.".into() } else { format!("Tabela {name} aberta.") })
            }
        }
    }
}
