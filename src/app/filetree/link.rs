//! Links to files: a path a chat's words name (`src/app/x.rs:120`) or a drawing carries (`Element::link`), opened in a
//! file tree on the canvas at its line, the tree in view and with the keys, and a "Voltar" button back to where the
//! person was.

use crate::app::chat::direct::folder;
use crate::app::chat::segments::{is_local, resolve};
use crate::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

/// A file and the line to show (from 1), when said.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FileLink {
    pub(crate) path: PathBuf,
    pub(crate) line: Option<usize>,
}

/// Where the person was before a link took them to a file, put back by "Voltar".
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LinkReturn {
    pub(crate) chat_open: bool,
    pub(crate) chat_agent: Option<String>,
    pub(crate) pan: Vec2,
    pub(crate) zoom: f32,
    pub(crate) elevated: Option<String>,
    pub(crate) card_focus: Option<String>,
    /// The tree the link opened the file in.
    pub(crate) tree: String,
}

/// A link clicked where only the context is at hand (a chat's words, a step's card): the file, and the chat or element
/// it was in (`take_link_ask`).
const LINK_ASK: &str = "file_link_ask";

/// The chat whose words are being drawn and its folder, while `agent_words_in` draws them: its paths are links.
const LINK_BASE: &str = "file_link_base";

/// A link clicked this frame: `file_links_tick` opens it.
pub(crate) fn ask_open(ctx: &Context, link: FileLink, near: Option<String>) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::unique(LINK_ASK), (link, near)));
}

fn take_link_ask(ctx: &Context) -> Option<(FileLink, Option<String>)> {
    let id = egui::Id::unique(LINK_ASK);

    ctx.data_mut(|data| {
        let ask = data.get_temp::<(FileLink, Option<String>)>(id);

        data.remove::<(FileLink, Option<String>)>(id);
        ask
    })
}

/// The chat's words drawn by `draw` make links of their paths, relative ones from `base`.
pub(crate) fn with_link_base<R>(ctx: &Context, base: Option<PathBuf>, chat: Option<String>, draw: impl FnOnce() -> R) -> R {
    let id = egui::Id::unique(LINK_BASE);
    let before = link_base(ctx);

    ctx.data_mut(|data| data.insert_temp(id, (base, chat)));
    let out = draw();

    ctx.data_mut(|data| match before {
        Some(before) => {
            data.insert_temp(id, before);
        }
        None => {
            data.remove::<(Option<PathBuf>, Option<String>)>(id);
        }
    });

    out
}

/// The base `with_link_base` set: None outside a chat's words (no links there).
pub(crate) fn link_base(ctx: &Context) -> Option<(Option<PathBuf>, Option<String>)> {
    ctx.data(|data| data.get_temp::<(Option<PathBuf>, Option<String>)>(egui::Id::unique(LINK_BASE)))
}

/// `path`, `path:120`, `path:120:4`, `path:120-130` and `path#L120`: the path and its line.
pub(crate) fn split_line(token: &str) -> (&str, Option<usize>) {
    if let Some((path, anchor)) = token.rsplit_once("#L") {
        let digits: String = anchor.chars().take_while(char::is_ascii_digit).collect();

        if !digits.is_empty() {
            return (path, digits.parse().ok());
        }
    }

    let (mut rest, mut line) = (token, None);

    // the line, then maybe the column after it: the first number from the left is the line
    for _ in 0..2 {
        let Some((head, tail)) = rest.rsplit_once(':') else { break };
        let number = tail.split('-').next().unwrap_or_default();

        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            break;
        }

        (rest, line) = (head, number.parse().ok());
    }

    (rest, line.filter(|line| *line > 0))
}

/// A word that may name a file: no spaces, quotes nor wildcards, not an address, and a name with an extension
/// (`x.rs`, `a/b.toml`, `.env`), its line after it or not.
pub(crate) fn path_like(token: &str) -> bool {
    let (path, _) = split_line(token);
    let name = path.rsplit(['/', '\\']).next().unwrap_or_default();
    let extension = name.rsplit_once('.').map(|(_, extension)| extension).unwrap_or_default();

    !path.is_empty()
        && !path.contains(|letter: char| letter.is_whitespace() || "`\"'<>|*?[](){}".contains(letter))
        && is_local(path)
        && (1..=10).contains(&extension.len())
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && extension.bytes().any(|byte| byte.is_ascii_alphabetic())
}

/// A word with the marks a sentence puts around it (`(`, `"`, `.`, `,`, `:`) taken off: where it starts in `word`, and
/// what is left.
pub(crate) fn trimmed(word: &str) -> (usize, &str) {
    let start = word.len() - word.trim_start_matches(['(', '[', '"', '\'', '<', '*', '_', '«']).len();

    (start, word[start..].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '"', '\'', '>', '*', '_', '»']))
}

thread_local! {
    /// Whether a path was a file, and when it was looked at: a frame of a conversation asks for the same paths again.
    static SEEN: RefCell<HashMap<PathBuf, (bool, f64)>> = RefCell::new(HashMap::new());
}

/// The file `token` names, a relative path from `base`, when it is there (looked at again after 3 s, at `time`).
pub(crate) fn file_link_at(token: &str, base: Option<&Path>, time: f64) -> Option<FileLink> {
    if !path_like(token) {
        return None;
    }

    let (path, line) = split_line(token);
    let path = resolve(path, base);

    let there = SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();

        match seen.get(&path) {
            Some(&(there, at)) if (time - at).abs() < 3.0 => there,
            _ => {
                let there = path.is_file();

                if seen.len() > 512 {
                    seen.clear();
                }

                seen.insert(path.clone(), (there, time));
                there
            }
        }
    });

    there.then_some(FileLink { path, line })
}

/// A file the tree's code panel shows: not a picture, a page, a PDF nor an archive (those open in their program).
pub(crate) fn opens_in_tree(path: &Path) -> bool {
    const ELSEWHERE: [&str; 18] =
        ["pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "html", "htm", "zip", "gz", "tar", "exe", "msi", "deb", "dmg"];

    let extension = path.extension().map(|extension| extension.to_string_lossy().to_lowercase()).unwrap_or_default();

    path.is_file() && !ELSEWHERE.contains(&extension.as_str())
}

/// What an element's `link` says: a path with its line (`src/x.rs:12`), or None for an address on the web.
pub(crate) fn element_link(link: &str) -> Option<FileLink> {
    let link = link.trim();

    if !is_local(link) {
        return None;
    }

    let (path, line) = split_line(link);

    Some(FileLink { path: resolve(path, None), line })
}

/// The folder a tree opened for `file` shows: the chat's folder when the file is in it, else the nearest folder over it
/// with a `.git` (its project), else its own folder.
fn project_of(file: &Path, chat_folder: Option<&Path>) -> PathBuf {
    if let Some(chat_folder) = chat_folder.filter(|chat_folder| file.starts_with(chat_folder)) {
        return chat_folder.to_path_buf();
    }

    let parent = file.parent().unwrap_or(Path::new("/"));

    parent.ancestors().find(|dir| dir.join(".git").exists()).unwrap_or(parent).to_path_buf()
}

impl DrawingApp {
    /// A link clicked in a chat's words or a step's card this frame: opened (every frame, from `chat_nodes_tick`).
    pub(crate) fn file_links_tick(&mut self, ctx: &Context) {
        if let Some((link, near)) = take_link_ask(ctx) {
            let near = near.or_else(|| self.chat_agent.clone());

            if let Err(why) = self.open_file_link(&link, near.as_deref()) {
                self.toast(why);
            }
        }
    }

    /// Opens `link` in a file tree on the canvas: the one whose folder holds it, else a new one on its project beside
    /// `near` (the chat or the drawing it came from). The file is picked in the tree and open in its code panel at the
    /// line, the chat screen closes, the camera goes to the tree and the tree takes the keys; "Voltar" (`link_back_ui`)
    /// puts back what was on screen.
    pub(crate) fn open_file_link(&mut self, link: &FileLink, near: Option<&str>) -> Result<String, String> {
        let full = folder(&link.path.to_string_lossy()).filter(|path| path.is_file()).ok_or_else(|| format!("{} não existe.", link.path.display()))?;
        let chat_folder = near.and_then(|id| self.chats.iter().find(|chat| chat.id == id)).and_then(|chat| folder(&chat.cwd));
        let inside = |element: &Element| element.filetree.as_ref().is_some_and(|tree| full.starts_with(tree.root_path()));

        let index = match self.scene.elements.iter().position(inside) {
            Some(index) => index,
            None => {
                let around = near.and_then(|id| self.scene.elements.iter().find(|element| element.id == id)).map(element_bounds);

                self.tree_for_folder_near(&project_of(&full, chat_folder.as_deref()).to_string_lossy(), around)?
            }
        };

        let id = self.scene.elements[index].id.clone();

        // a second link from the tree the first one opened goes back to where the first one came from
        let back = match self.filetrees.link_back.take().filter(|back| !self.chat_open && self.card_focus.as_deref() == Some(back.tree.as_str())) {
            Some(back) => LinkReturn { tree: id.clone(), ..back },
            None => LinkReturn {
                chat_open: self.chat_open,
                chat_agent: self.chat_agent.clone(),
                pan: self.pan,
                zoom: self.zoom,
                elevated: self.terminal_elevated.clone(),
                card_focus: self.card_focus.clone(),
                tree: id.clone(),
            },
        };

        self.act_file_tree(&json!({ "action": "open", "tree": id, "path": full.to_string_lossy(), "line": link.line }))?;
        self.ft_show_editor(&id, true);
        self.ft_reveal_all(&id, &full);
        self.chat_open = false;
        self.set_elevated(None);
        // nothing picked on the canvas: its bar would cover the button back
        self.moving = false;
        self.selected.clear();
        self.fit_view(element_bounds(&self.scene.elements[index]), None);
        self.card_focus = Some(id);
        self.filetrees.link_back = Some(back);
        let line = link.line.map(|line| format!(" na linha {line}")).unwrap_or_default();

        Ok(format!("{} aberto na Árvore de Arquivos{line}. \"Voltar\" leva de volta.", full.display()))
    }

    /// "Voltar": the screen as it was before the last link (the chat open on its conversation, or the camera where it
    /// was), and the button goes.
    pub(crate) fn link_go_back(&mut self) -> Result<String, String> {
        let back = self.filetrees.link_back.take().ok_or("Nenhum link aberto para voltar.")?;

        (self.pan, self.zoom, self.glide) = (back.pan, back.zoom, None);
        self.set_elevated(back.elevated.filter(|id| self.scene.elements.iter().any(|element| &element.id == id)));
        self.card_focus = back.card_focus;

        if back.chat_open {
            self.chat_agent = back.chat_agent;
            self.show_chat();

            return Ok("De volta ao chat.".into());
        }

        Ok("De volta ao canvas onde estava.".into())
    }

    /// The tree's folders down to `path` open and its row picked: a tree just made has listed none yet.
    fn ft_reveal_all(&mut self, id: &str, path: &Path) {
        let root = self.ft_spec(id).map(|spec| spec.root_path()).unwrap_or_default();

        if let Some(view) = self.filetrees.views.get_mut(id) {
            for dir in path.ancestors().skip(1).take_while(|dir| dir.starts_with(&root) && *dir != root) {
                view.expanded.insert(dir.to_path_buf());
            }
        }

        self.ft_reveal(id, path);
    }

    /// The "Voltar" button at the foot of the canvas while a link's tree is on screen: back to the chat (or the camera) it came
    /// from; its ✕ only lets it go. Gone when the chat opens some other way or the tree goes.
    pub(crate) fn link_back_ui(&mut self, ctx: &Context) {
        let Some(back) = &self.filetrees.link_back else { return };

        if self.chat_open || !self.scene.elements.iter().any(|element| element.id == back.tree) {
            self.filetrees.link_back = None;

            return;
        }

        let dark = self.dark();
        let words = if back.chat_open { "Voltar ao chat" } else { "Voltar" };
        let ink = glass_ink(dark);
        let font = FontId::proportional(13.5);
        let width = ctx.fonts_mut(|fonts| fonts.layout_no_wrap(words.into(), font.clone(), ink).size().x) + 64.0;
        // at the foot of the canvas, in the middle, in the row of its corner pills (the top has a selection's bar)
        let at = Pos2::new(self.canvas_rect.center().x - width / 2.0, self.canvas_rect.bottom() - 45.0);
        let (mut back_pressed, mut dismissed) = (false, false);

        egui::Area::new(egui::Id::unique("file_link_back")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::hover());

            paint_glass(ui.painter(), rect, 17.0, dark, true);
            let close = Rect::from_center_size(Pos2::new(rect.right() - 17.0, rect.center().y), Vec2::splat(24.0));
            let main = Rect::from_min_max(rect.min, Pos2::new(close.left(), rect.max.y));
            let go = ui.interact(main, egui::Id::unique("file_link_back_go"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let shut = ui.interact(close, egui::Id::unique("file_link_back_shut"), Sense::click()).on_hover_text("Fechar sem voltar");
            let p = ui.painter();

            if go.hovered() {
                p.rect_filled(main.shrink(3.0), 14.0, ink.gamma_multiply(0.08));
            }

            if shut.hovered() {
                p.circle_filled(close.center(), 11.0, ink.gamma_multiply(0.1));
            }

            p.text(Pos2::new(rect.left() + 14.0, rect.center().y), egui::Align2::LEFT_CENTER, "←", font.clone(), ink);
            p.text(Pos2::new(rect.left() + 32.0, rect.center().y), egui::Align2::LEFT_CENTER, words, font.clone(), ink);
            p.text(close.center(), egui::Align2::CENTER_CENTER, "×", FontId::proportional(16.0), ink.gamma_multiply(0.7));
            (back_pressed, dismissed) = (go.clicked(), shut.clicked());
        });

        if back_pressed {
            let _ = self.link_go_back();
        } else if dismissed {
            self.filetrees.link_back = None;
        }
    }

    /// The mark at the corner of each drawing with a `link` in view: a click opens its file in a tree beside it (an
    /// address on the web in the browser), with "Voltar" back here.
    pub(crate) fn element_links_ui(&mut self, ctx: &Context) {
        if self.terminal_elevated.is_some() || self.editing.is_some() {
            return;
        }

        let view = self.canvas_rect;

        let marks: Vec<(String, String, Pos2)> = self
            .scene
            .elements
            .iter()
            .filter(|element| !element.is_node())
            .filter_map(|element| {
                let link = element.link.clone()?;
                let bounds = element_bounds(element);
                let corner = self.to_screen(bounds.right_top()) + Vec2::new(LINK_MARK * 0.5 + 2.0, -LINK_MARK * 0.5 - 2.0);

                (view.expand(LINK_MARK).contains(corner) && bounds.width().max(bounds.height()) * self.zoom > 12.0).then_some((
                    element.id.clone(),
                    link,
                    corner,
                ))
            })
            .collect();

        let c = self.colors();
        let mut clicked = None;

        for (id, link, corner) in marks {
            let rect = Rect::from_center_size(corner, Vec2::splat(LINK_MARK));

            egui::Area::new(egui::Id::unique(("element_link", id.as_str()))).order(egui::Order::Middle).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(rect.size(), Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(format!("Abrir {link}"));
                let p = ui.painter();

                p.rect_filled(rect, 6.0, if response.hovered() { c.surface_high } else { c.island });
                p.rect_stroke(rect, 6.0, Stroke::new(1.0, c.code.gamma_multiply(0.6)), StrokeKind::Inside);
                chain(p, rect.center(), rect.width() * 0.6, c.code);

                if response.clicked() {
                    clicked = Some((id.clone(), link.clone()));
                }
            });
        }

        let Some((id, link)) = clicked else { return };

        match element_link(&link) {
            Some(link) => {
                if let Err(why) = self.open_file_link(&link, Some(&id)) {
                    self.toast(why);
                }
            }
            None => crate::app::browser::open_external(&link),
        }
    }
}

/// The side of a drawing's link mark, on screen.
pub(crate) const LINK_MARK: f32 = 20.0;

/// Two links of a chain across `size` at `center`, slanting up to the right: the link mark.
fn chain(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let along = Vec2::new(1.0, -1.0).normalized();
    let across = along.rot90();
    let (radius, half) = (size * 0.17, size * 0.2);

    for side in [-1.0, 1.0] {
        let middle = center + along * side * size * 0.17;
        // a stadium: two half circles joined
        let mut points = Vec::new();

        for (end, start_angle) in [(half, -std::f32::consts::FRAC_PI_2), (-half, std::f32::consts::FRAC_PI_2)] {
            for step in 0..=8 {
                let angle = start_angle + std::f32::consts::PI * step as f32 / 8.0;

                points.push(middle + along * end + (along * angle.cos() + across * angle.sin()) * radius);
            }
        }

        painter.add(Shape::closed_line(points, Stroke::new(1.4, color)));
    }
}
