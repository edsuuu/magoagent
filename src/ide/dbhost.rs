//! The IDE's side of the database panel: the tabs of tables and consoles are its own.
use super::*;
use crate::db::{Event, Open};

impl Ide {
    /// The data sources window is up (an Esc then belongs to it, not to the canvas).
    pub(super) fn db_dialog_open(&self) -> bool {
        self.db.dialog_open()
    }

    /// What the database does with its panel closed too: the trees' and the open tables' answers.
    pub(super) fn db_background(&mut self) {
        self.db.background();
        for tab in &mut self.tabs {
            if let Some(sheet) = tab.sheet.as_mut() {
                sheet.poll(&self.ctx);
            }
        }
    }

    pub(super) fn db_panel(&mut self, ui: &mut Ui) {
        if self.files_rx.is_none() {
            let files = std::mem::take(&mut self.files);
            self.db.discover(&files, &self.ctx);
            self.files = files;
        } else {
            self.db.ensure_loaded();
        }
        tool_header(ui, "Banco de dados");
        let tabs = &self.tabs;
        let busy = |id: &str| tabs.iter().any(|t| t.sheet.as_ref().is_some_and(|s| s.source() == id && s.busy()));
        let events = self.db.panel(ui, &self.ctx.clone(), &busy);
        self.db_events(events);
    }

    /// The data sources window, wherever the IDE shows (not only with the Database panel open).
    pub(super) fn db_dialog(&mut self, ctx: &egui::Context) {
        let events = self.db.dialog(ctx);
        self.db_events(events);
    }

    fn db_events(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::Open(open) => self.open_table(open),
                Event::Closed(id) => {
                    self.tabs.retain(|t| t.sheet.as_ref().is_none_or(|s| s.source() != id));
                    self.active = self.active.min(self.tabs.len().saturating_sub(1));
                }
            }
        }
    }

    fn open_table(&mut self, open: Open) {
        if let Some(i) = self.tabs.iter().position(|t| t.sheet.as_ref().is_some_and(|s| open.is_shown_by(s))) {
            self.active = i;
            return;
        }
        let id = self.id;
        let Some((path, sheet)) = self.db.new_sheet(&self.ctx, id, &open) else { return };
        let mut tab = Tab::new(path, String::new(), false);
        tab.sheet = Some(Box::new(sheet));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    pub(super) fn sheet_view(&mut self, ui: &mut Ui) {
        if let Some(sheet) = self.tabs[self.active].sheet.as_mut() {
            sheet.ui(ui);
        }
    }
}
