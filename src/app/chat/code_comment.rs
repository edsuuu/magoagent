//! "Comentar para o Maestro": a block selected in a code editor (the IDE's or the file tree's pane) gets a note in a small
//! popover, and the file, lines, code and note go to the Maestro as one message. The editors only hand a `NoteReq` over;
//! the popover and the sending are the app's, so both editors share them.

use crate::prelude::*;
use egui::{pos2, vec2};
use std::path::{Path, PathBuf};

/// A selection to comment on, as an editor hands it to the app: the file, its lines (1-based, both included),
/// the language, the selected text and where the popover goes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NoteReq {
    pub path: PathBuf,
    pub from: usize,
    pub to: usize,
    pub lang: String,
    pub code: String,
    pub anchor: Pos2,
}

/// The request for the chars `sel` (start, end) of `text`; None when nothing is selected.
pub(crate) fn note_req(path: &Path, lang: &str, text: &str, sel: (usize, usize), anchor: Pos2) -> Option<NoteReq> {
    let (a, b) = (sel.0.min(sel.1), sel.0.max(sel.1));
    if a == b {
        return None;
    }
    let code: String = text.chars().skip(a).take(b - a).collect();
    let from = crate::ide::text::line_of(text, a) + 1;
    let to = crate::ide::text::line_of(text, b - 1) + 1;
    Some(NoteReq { path: path.to_path_buf(), from, to, lang: lang.into(), code: code.trim_end_matches('\n').into(), anchor })
}

/// The message to the Maestro: where the code is, the code, a rule and the note.
pub(crate) fn code_comment_prompt(path: &str, from: usize, to: usize, lang: &str, code: &str, comment: &str) -> String {
    format!("Comentário em {path}:{from}-{to}\n```{lang}\n{code}\n```\n---\n{}", comment.trim())
}

/// The path as the message shows it: relative to `root`, with `/`.
pub(crate) fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// The popover being filled in.
pub(crate) struct Note {
    pub req: NoteReq,
    pub rel: String,
    /// The node it came from (IDE or tree): a chat roped to it is the first to hear.
    pub source: String,
    pub root: String,
    pub comment: String,
    pub ide_look: bool,
    pub busy: Option<String>,
    pub focused: bool,
}

impl DrawingApp {
    pub(crate) fn open_maestro_note(&mut self, source: &str, root: &str, ide_look: bool, req: NoteReq) {
        let rel = relative(Path::new(root), &req.path);
        self.maestro_note = Some(Note { req, rel, source: source.into(), root: root.into(), comment: String::new(), ide_look, busy: None, focused: false });
    }

    /// The chat that hears it: one roped to `source`, else the chat in use, else a new one in `root`.
    fn maestro_chat(&mut self, source: &str, root: &str) -> Option<String> {
        let roped = self.links().get(source).and_then(|l| l.iter().find(|o| self.chat_node(o).is_some()).cloned());
        if let Some(id) = roped.or_else(|| self.chat_agent.clone().filter(|id| self.is_chat(id))) {
            return Some(id);
        }
        let installed = self.installed_agents();
        if installed.is_empty() {
            self.toast("Nenhum agente instalado: instale o claude ou o codex e volte aqui.");
            return None;
        }
        let mut form = crate::app::chat::direct::NewChat::from_prefs(&self.prefs, &installed);
        form.cwd = root.into();
        self.create_chat(&form, &installed);
        self.chat_agent.clone()
    }

    /// Sends `prompt` to the Maestro and shows the answer in the mini chat. Err: it didn't go (and why).
    pub(crate) fn comment_to_maestro(&mut self, ctx: &Context, source: &str, root: &str, prompt: &str) -> Result<(), String> {
        let id = self.maestro_chat(source, root).ok_or("Sem chat para o Maestro.")?;
        if self.chat_runs.contains_key(&id) {
            return Err("O Maestro ainda está respondendo: espere ou pare com Esc.".into());
        }
        if !self.chat_send(ctx, &id, prompt) {
            return Err("Não consegui enviar ao Maestro.".into());
        }
        self.open_mini_chat(&id);
        self.toast("Comentário enviado ao Maestro");
        Ok(())
    }

    /// `comment_to_maestro` without a frame's context (the MCP): the message waits in the outbox for the next frame.
    pub(crate) fn queue_to_maestro(&mut self, source: &str, root: &str, prompt: String) -> Result<(), String> {
        let id = self.maestro_chat(source, root).ok_or("Sem chat para o Maestro.")?;
        if self.chat_runs.contains_key(&id) {
            return Err("O Maestro ainda está respondendo: tente de novo depois.".into());
        }
        self.chat_outbox.push((id.clone(), prompt));
        self.open_mini_chat(&id);
        Ok(())
    }

    /// The popover, over the selection: a multi-line box, Cancelar and Enviar (Ctrl+Enter sends, Esc cancels).
    pub(crate) fn maestro_note_ui(&mut self, ctx: &Context) {
        let Some(mut note) = self.maestro_note.take() else { return };
        let (mut send, mut keep) = (false, true);
        let dark = ctx.global_style().visuals.dark_mode;
        let frame = if note.ide_look {
            use crate::ide::theme as t;
            egui::Frame::NONE.fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(8).inner_margin(10)
        } else {
            crate::app::ui::menu_look::menu_frame(dark).inner_margin(10)
        };
        let (muted, accent) = if note.ide_look {
            (crate::ide::theme::MUTED, crate::ide::theme::ACCENT)
        } else {
            (crate::app::ui::menu_look::menu_colors(dark).muted, Color32::from_rgb(0x38, 0x71, 0xe1))
        };
        let (r, a) = (ctx.content_rect(), note.req.anchor);
        let at = pos2(a.x.min(r.right() - 350.0).max(r.left()), (a.y + 6.0).min(r.bottom() - 200.0));
        egui::Area::new(egui::Id::unique("maestro_note")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(320.0);
                ui.label(egui::RichText::new("Comentar para o Maestro").strong());
                ui.label(egui::RichText::new(format!("{}:{}-{}", note.rel, note.req.from, note.req.to)).color(muted).small());
                ui.add_space(4.0);
                let field = ui.add(
                    egui::TextEdit::multiline(&mut note.comment).hint_text("O que o Maestro deve saber ou fazer").desired_rows(3).desired_width(f32::INFINITY),
                );
                if !note.focused {
                    field.request_focus();
                    note.focused = true;
                }
                if let Some(why) = &note.busy {
                    ui.label(egui::RichText::new(why).color(Color32::from_rgb(0xe0, 0x9a, 0x2b)).small());
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let ready = !note.comment.trim().is_empty();
                    let button = egui::Button::new(egui::RichText::new("Enviar").color(Color32::WHITE)).fill(accent).min_size(vec2(72.0, 24.0));
                    send = ui.add_enabled(ready, button).clicked() || (ready && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)));
                    keep = !ui.add(egui::Button::new("Cancelar").min_size(vec2(72.0, 24.0))).clicked() && !ui.input(|i| i.key_pressed(Key::Escape));
                });
            });
        });
        if !keep {
            return;
        }
        if send {
            let prompt = code_comment_prompt(&note.rel, note.req.from, note.req.to, &note.req.lang, &note.req.code, &note.comment);
            match self.comment_to_maestro(ctx, &note.source, &note.root, &prompt) {
                Ok(()) => return,
                // the words stay in the box, with the reason
                Err(why) => note.busy = Some(why),
            }
        }
        self.maestro_note = Some(note);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_and_request() {
        let p = code_comment_prompt("src/a.rs", 3, 4, "rust", "fn a() {}\nfn b() {}", "  troque o nome ");
        assert_eq!(p, "Comentário em src/a.rs:3-4\n```rust\nfn a() {}\nfn b() {}\n```\n---\ntroque o nome");
        let text = "um\ndois\ntrês\n";
        let r = note_req(Path::new("x.txt"), "text", text, (3, 8), Pos2::ZERO).unwrap();
        assert_eq!((r.from, r.to, r.code.as_str()), (2, 2, "dois"));
        let r = note_req(Path::new("x.txt"), "text", text, (3, 13), Pos2::ZERO).unwrap();
        assert_eq!((r.from, r.to), (2, 3));
        assert!(note_req(Path::new("x.txt"), "text", text, (2, 2), Pos2::ZERO).is_none());
        assert_eq!(relative(Path::new("C:/p"), Path::new("C:/p/src/a.rs")), "src/a.rs");
    }
}
