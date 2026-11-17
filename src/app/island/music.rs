//! The Spotify tab (asked: "um player do spotify, com fila, aleatório, curtidas, playlists e busca"). At the left the
//! player: the cover, the song, where it comes from, the time, the buttons (aleatório, voltar, tocar, pular, repetir,
//! o coração), the volume and the device, with the Luna dancing next to the name. At the right the search and three
//! lists: A seguir, Playlists and Curtidas. A press is a word for `spotify_press` (`integrations/spotify/controls.rs`).

use super::icons;
use super::marquee::marquee;
use super::paint::{CARD, CARD_FLAT, DIM, DIM_3, INK, font, semi};
use crate::app::chat::list::clipped;
use crate::app::integrations::spotify::controls::LIKED_SONGS;
use crate::app::integrations::spotify::library::{ERROR_FOR_MS, PlaylistItem};
use crate::app::integrations::spotify::playback::Repeat;
use crate::app::integrations::spotify::queue::QueueItem;
use crate::app::integrations::spotify::{Now, Player};
use crate::prelude::*;

/// The island's height on this tab.
pub(super) const HEIGHT: f32 = 318.0;
const LEFT_W: f32 = 318.0;
const COVER: f32 = 112.0;
/// What the song's name leaves for the heart at its end.
const HEART_ROOM: f32 = 26.0;
const ROW_H: f32 = 36.0;
pub(super) const GREEN: Color32 = Color32::from_rgb(0x1e, 0xd7, 0x60);
const RED: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);
const HOVER: Color32 = Color32::from_rgb(0x1b, 0x1d, 0x21);
const TRACK: Color32 = Color32::from_rgb(0x2b, 0x2e, 0x33);
/// The search waits this long after the last key before it asks.
const SEARCH_AFTER: f64 = 0.45;

/// The lists under the search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum List {
    #[default]
    Next,
    Playlists,
    Liked,
}

impl List {
    const EVERY: [List; 3] = [List::Next, List::Playlists, List::Liked];

    fn label(self) -> &'static str {
        match self {
            List::Next => "A seguir",
            List::Playlists => "Playlists",
            List::Liked => "Curtidas",
        }
    }
}

/// What the tab keeps between frames.
#[derive(Default)]
pub(crate) struct Page {
    list: List,
    /// The search as typed, what was last asked, and when the last key came (the island's clock).
    query: String,
    asked: String,
    typed_at: f64,
    /// The devices' list in place of the others ("Soloist ▾").
    devices: bool,
    /// The volume while it is dragged (0 to 1).
    dragging: Option<f32>,
}

/// "1:23" (and "1:02:03" past the hour).
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
}

/// The search to ask now: typed, unlike the last one asked, quiet for `SEARCH_AFTER`, two letters at least.
fn search_due(page: &Page, time: f64) -> Option<String> {
    let query = page.query.trim();
    (query.chars().count() >= 2 && query != page.asked.trim() && time - page.typed_at >= SEARCH_AFTER).then(|| query.to_string())
}

/// What a press on a song of the queue does: it is `index` songs ahead, so the player skips that many and one.
fn skips_to(index: usize) -> Vec<String> {
    vec!["next".to_string(); index + 1]
}

/// The two colours of a cover not come down yet, out of its name (each list item has its own).
fn placeholder(name: &str) -> (Color32, Color32) {
    const PAIRS: [((u8, u8, u8), (u8, u8, u8)); 5] = [
        ((0x7c, 0x3a, 0xed), (0xdb, 0x27, 0x77)),
        ((0x0e, 0xa5, 0xe9), (0x22, 0xc5, 0x5e)),
        ((0xf5, 0x9e, 0x0b), (0xef, 0x44, 0x44)),
        ((0x33, 0x41, 0x55), (0x64, 0x74, 0x8b)),
        ((0x16, 0xa3, 0x4a), (0x4a, 0xde, 0x80)),
    ];
    let sum = name.bytes().fold(0usize, |sum, byte| sum.wrapping_mul(31).wrapping_add(byte as usize));
    let ((r1, g1, b1), (r2, g2, b2)) = PAIRS[sum % PAIRS.len()];
    (Color32::from_rgb(r1, g1, b1), Color32::from_rgb(r2, g2, b2))
}

/// A picture in `rect`, or its placeholder: a diagonal of two colours.
pub(super) fn picture(painter: &Painter, rect: Rect, round: u8, texture: Option<egui::TextureId>, name: &str) {
    if let Some(texture) = texture {
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.add(egui::epaint::RectShape::filled(rect, round, Color32::WHITE).with_texture(texture, uv));
        return;
    }
    // the rounded rect as epaint draws it, each point coloured by how far along the diagonal it is
    let (from, to) = placeholder(name);
    let mut tessellator = egui::epaint::Tessellator::new(painter.pixels_per_point(), Default::default(), [1, 1], Vec::new());
    let mut mesh = egui::Mesh::default();
    tessellator.tessellate_rect(&egui::epaint::RectShape::filled(rect, round, Color32::WHITE), &mut mesh);
    for vertex in &mut mesh.vertices {
        let along = (((vertex.pos.x - rect.left()) / rect.width() + (vertex.pos.y - rect.top()) / rect.height()) / 2.0).clamp(0.0, 1.0);
        vertex.color = from.lerp_to_gamma(to, along).gamma_multiply(f32::from(vertex.color.a()) / 255.0);
    }
    painter.add(mesh);
}

/// A round button of the player: its press. `hint` is for screen readers only: the hover is enough on screen (asked:
/// "tooltip nesses botões não precisa, só um hover já serve").
fn round_button(ui: &mut egui::Ui, center: Pos2, side: f32, id: &str, hint: &str) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(side));
    let response = ui.interact(rect, ui.scope_id().with(("island_music", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, hint));
    response
}

/// Set for a frame when a quiet line's "Abrir Ajustes" is pressed (`needs`), taken by `island_music`.
fn settings_asked() -> egui::Id {
    egui::Id::unique("island_music_open_settings")
}

impl DrawingApp {
    /// The tab inside `page`.
    pub(super) fn island_music(&mut self, ui: &mut egui::Ui, page: Rect, time: f64) {
        self.spotify_page_open();
        let gap = 10.0;
        let left = Rect::from_min_size(page.min, Vec2::new(LEFT_W, page.height()));
        let right = Rect::from_min_max(Pos2::new(left.right() + gap, page.top()), page.max);
        for card in [left, right] {
            ui.painter().rect(card, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        }
        let mut state = std::mem::take(&mut self.island.music);
        let mut pressed = Vec::new();
        match self.integrations.spotify.now.clone() {
            Some(now) => self.music_player(ui, left, &now, time, &mut state, &mut pressed),
            None => {
                let words = if self.prefs.integrations.spotify || self.prefs.integrations.soloist { "Nada tocando agora" } else { "O Spotify está desligado" };
                ui.painter().text(left.center() - Vec2::new(0.0, 8.0), egui::Align2::CENTER_CENTER, words, semi(13.0), DIM);
                let hint = "Toque algo no Spotify (ou escolha o Soloist no celular)";
                let galley = clipped(ui.painter(), hint.into(), font(11.0), DIM_3, left.width() - 28.0);
                ui.painter().galley(Pos2::new(left.center().x - galley.size().x / 2.0, left.center().y + 6.0), galley, DIM_3);
            }
        }
        self.music_lists(ui, right, time, &mut state, &mut pressed);
        self.island.music = state;
        if ui.ctx().data_mut(|data| data.remove_temp::<bool>(settings_asked())).unwrap_or(false) {
            self.open_settings_at("Integrações");
            self.raise_window = true;
        }
        for words in pressed {
            self.spotify_press(&words);
        }
    }

    /// The player's card.
    fn music_player(&self, ui: &mut egui::Ui, card: Rect, now: &Now, time: f64, state: &mut Page, pressed: &mut Vec<String>) {
        let inner = Rect::from_min_max(card.min + Vec2::new(16.0, 14.0), card.max - Vec2::new(16.0, 12.0));
        let painter = ui.painter().clone();
        let playback = &now.playback;
        let cover = Rect::from_min_size(inner.min, Vec2::splat(COVER));
        picture(&painter, cover, 10, self.integrations.spotify.cover.as_ref().map(egui::TextureHandle::id), &now.track.title);
        // the name, where it plays and the Luna dancing; the song; where it comes from
        let text_x = cover.right() + 14.0;
        let width = inner.right() - text_x;
        let name_y = inner.top() + 9.0;
        painter.circle_filled(Pos2::new(text_x + 3.5, name_y), 3.5, GREEN);
        let name = painter.text(Pos2::new(text_x + 12.0, name_y), egui::Align2::LEFT_CENTER, "Spotify", semi(12.5), INK);
        let place = match now.player {
            Player::App => "",
            Player::Spotifast => "no Spotifast",
            Player::Soloist => "no Soloist",
        };
        let kind = painter.text(Pos2::new(name.right() + 6.0, name_y), egui::Align2::LEFT_CENTER, place, font(11.0), DIM);
        let luna_at = Pos2::new(kind.right() + if place.is_empty() { 8.0 } else { 15.0 }, name_y - 2.0);
        if let Some(groove) = self.luna_groove() {
            crate::app::mascot::mago::paint_mago_dancing(&painter, "island-spotify-luna", luna_at, 16.0, self.luna_colour(GREEN), &groove, time);
            // the note floats up beside her while it plays
            if now.playing {
                let rise = (time * 0.8).fract() as f32;
                let note = luna_at + Vec2::new(11.0, -3.0 - rise * 7.0);
                painter.text(note, egui::Align2::CENTER_CENTER, "♪", font(10.0), GREEN.gamma_multiply(1.0 - rise));
            }
        }
        let mut y = inner.top() + 22.0;
        let mut title_y = y + 10.0;
        for (index, (text, size, bold, color)) in
            [(now.track.title.as_str(), 15.0, true, INK), (now.track.artist.as_str(), 12.0, false, DIM), (now.track.album.as_str(), 11.0, false, DIM_3)]
                .into_iter()
                .enumerate()
        {
            if text.is_empty() {
                continue;
            }
            let face = if bold { semi(size) } else { font(size) };
            let height = painter.layout_no_wrap(text.to_string(), face.clone(), color).size().y;
            if index == 0 {
                // the song's name rolls when it does not fit, as the system's own player; the heart sits at its end
                title_y = y + height / 2.0;
                marquee(ui, egui::Id::unique(("island_music_title", text)), text, face, color, (text_x, title_y), width - HEART_ROOM, true);
            } else {
                painter.galley(Pos2::new(text_x, y), clipped(&painter, text.to_string(), face, color, width), color);
            }
            y += height + 2.0;
        }
        if let Some(label) = playback.context.as_ref().map(|context| context.label()).filter(|label| !label.is_empty()) {
            let words = clipped(&painter, label, font(11.0), DIM, width - 31.0);
            let pill = Rect::from_min_size(Pos2::new(text_x, y + 6.0), Vec2::new(words.size().x + 31.0, 22.0));
            painter.rect_filled(pill, 11, HOVER);
            icons::paint(&painter, icons::FROM, Pos2::new(pill.left() + 14.0, pill.center().y), 12.0, GREEN);
            painter.galley(Pos2::new(pill.left() + 23.0, pill.center().y - words.size().y / 2.0), words, DIM);
        }
        // the time: a click on the bar goes there
        let time_y = inner.bottom() - 84.0;
        let position = now.position_at(now_ms());
        if let (Some(position), Some(duration)) = (position, playback.duration_ms.filter(|duration| *duration > 0)) {
            let mono = FontId::monospace(10.0);
            let gone = painter.text(Pos2::new(inner.left(), time_y), egui::Align2::LEFT_CENTER, clock(position), mono.clone(), DIM_3);
            let total = painter.text(Pos2::new(inner.right(), time_y), egui::Align2::RIGHT_CENTER, clock(duration), mono, DIM_3);
            let bar = Rect::from_min_max(Pos2::new(gone.right() + 8.0, time_y - 2.0), Pos2::new(total.left() - 8.0, time_y + 2.0));
            let response = ui.interact(bar.expand2(Vec2::new(0.0, 6.0)), ui.scope_id().with("island_music_seek"), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            painter.rect_filled(bar, 2, TRACK);
            let done = (position as f32 / duration as f32).clamp(0.0, 1.0);
            painter.rect_filled(
                Rect::from_min_max(bar.min, Pos2::new(bar.left() + bar.width() * done, bar.bottom())),
                2,
                if response.hovered() { GREEN } else { INK },
            );
            if let Some(at) = response.interact_pointer_pos().filter(|_| response.clicked()) {
                let fraction = ((at.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                pressed.push(format!("seek {}", (fraction * duration as f32) as u64));
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        // the buttons: aleatório, voltar, tocar ou pausar (in the middle), pular, repetir; the heart beside the song's name
        let row_y = inner.bottom() - 48.0;
        let step = inner.width() / 5.0;
        let center =
            |index: usize| if index == 5 { Pos2::new(inner.right() - 10.0, title_y) } else { Pos2::new(inner.left() + step * (index as f32 + 0.5), row_y) };
        let shuffle = playback.shuffle.unwrap_or(false);
        let repeat = playback.repeat.unwrap_or(Repeat::Off);
        let liked = playback.liked.or_else(|| self.integrations.spotify.library.hearts.get(&playback.track_uri).copied());
        let buttons: [(crate::editor::icons::Svg, &str, &str, bool, String); 6] = [
            (icons::SHUFFLE, "shuffle", "Aleatório", shuffle, format!("shuffle {}", if shuffle { "off" } else { "on" })),
            (icons::PREVIOUS, "previous", "Voltar", false, "previous".into()),
            (if now.playing { icons::PAUSE } else { icons::PLAY }, "play", if now.playing { "Pausar" } else { "Tocar" }, false, "playpause".into()),
            (icons::NEXT, "next", "Pular", false, "next".into()),
            (icons::REPEAT, "repeat", repeat.words(), repeat != Repeat::Off, format!("repeat {}", repeat.next().name())),
            (
                if liked == Some(true) { icons::HEART } else { icons::HEART_LINE },
                "heart",
                "Curtir",
                liked == Some(true),
                if liked == Some(true) { "unlike".into() } else { "like".into() },
            ),
        ];
        for (index, (icon, id, hint, lit, words)) in buttons.into_iter().enumerate() {
            let at = center(index);
            let play = id == "play";
            let response = round_button(ui, at, if play { 38.0 } else { 30.0 }, id, hint);
            let pushed = if response.is_pointer_button_down_on() { 0.94 } else { 1.0 };
            if play {
                painter.circle_filled(at, 19.0 * pushed, INK);
                icons::paint(&painter, icon, at, 16.0, Color32::from_rgb(0x11, 0x11, 0x11));
            } else {
                let ink = match (lit, response.hovered()) {
                    (true, _) => GREEN,
                    (false, true) => INK,
                    (false, false) => DIM,
                };
                icons::paint(&painter, icon, at, 16.0 * pushed, ink);
                if lit && id != "heart" {
                    painter.circle_filled(at + Vec2::new(0.0, 13.0), 2.0, GREEN);
                }
                if id == "repeat" && repeat == Repeat::Track {
                    painter.text(at + Vec2::new(7.0, -6.0), egui::Align2::CENTER_CENTER, "1", semi(8.0), GREEN);
                }
            }
            if response.clicked() {
                pressed.push(words);
            }
        }
        // the volume, and the device it plays on
        let volume_y = inner.bottom() - 11.0;
        icons::paint(&painter, icons::SPEAKER, Pos2::new(inner.left() + 8.0, volume_y), 16.0, DIM);
        let device = if playback.device.is_empty() { "Este computador".to_string() } else { playback.device.clone() };
        let device = clipped(&painter, format!("{device} ▾"), font(11.0), INK, 120.0);
        let pill = Rect::from_min_size(Pos2::new(inner.right() - device.size().x - 25.0, volume_y - 11.0), Vec2::new(device.size().x + 25.0, 22.0));
        let pill_response = ui.interact(pill, ui.scope_id().with("island_music_device"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        painter.rect_filled(pill, 11, if pill_response.hovered() || state.devices { TRACK } else { HOVER });
        painter.circle_filled(Pos2::new(pill.left() + 12.0, volume_y), 3.0, GREEN);
        painter.galley(Pos2::new(pill.left() + 19.0, volume_y - device.size().y / 2.0), device, INK);
        if pill_response.clicked() {
            state.devices = !state.devices;
        }
        if let Some(volume) = playback.volume {
            let track = Rect::from_min_max(Pos2::new(inner.left() + 25.0, volume_y - 2.0), Pos2::new(pill.left() - 12.0, volume_y + 2.0));
            let response = ui.interact(track.expand2(Vec2::new(4.0, 7.0)), ui.scope_id().with("island_music_volume"), Sense::click_and_drag());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            let pointer = response.interact_pointer_pos().map(|at| ((at.x - track.left()) / track.width()).clamp(0.0, 1.0));
            if response.dragged() || response.is_pointer_button_down_on() {
                state.dragging = pointer.or(state.dragging);
            }
            if (response.drag_stopped() || response.clicked())
                && let Some(level) = pointer.or(state.dragging)
            {
                pressed.push(format!("volume {}", (level * 100.0).round() as u8));
                state.dragging = None;
            }
            let level = state.dragging.unwrap_or(volume).clamp(0.0, 1.0);
            painter.rect_filled(track, 2, TRACK);
            let lit = if response.hovered() || state.dragging.is_some() { GREEN } else { DIM };
            painter.rect_filled(Rect::from_min_max(track.min, Pos2::new(track.left() + track.width() * level, track.bottom())), 2, lit);
        }
    }

    /// The search and the lists (or the devices).
    fn music_lists(&self, ui: &mut egui::Ui, card: Rect, time: f64, state: &mut Page, pressed: &mut Vec<String>) {
        let inner = Rect::from_min_max(card.min + Vec2::new(12.0, 10.0), card.max - Vec2::new(12.0, 10.0));
        let library = &self.integrations.spotify.library;
        // the search: it asks once the keys rest
        let field = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 30.0));
        let focused = ui.memory(|memory| memory.has_focus(egui::Id::unique("island_music_search")));
        ui.painter().rect(
            field,
            15,
            CARD_FLAT,
            Stroke::new(1.0, if focused { Color32::from_rgb(0x3a, 0x3d, 0x43) } else { Color32::from_white_alpha(9) }),
            StrokeKind::Inside,
        );
        icons::paint(ui.painter(), icons::SEARCH, Pos2::new(field.left() + 19.0, field.center().y), 14.0, DIM_3);
        let before = state.query.clone();
        // no margin of its own, and 3 px up: the font sits its letters low in the row, so they line up with the magnifier
        let edit_rect = Rect::from_min_max(Pos2::new(field.left() + 32.0, field.top() + 1.0), Pos2::new(field.right() - 12.0, field.bottom() - 7.0));
        let edit = ui.put(
            edit_rect,
            egui::TextEdit::singleline(&mut state.query)
                .id(egui::Id::unique("island_music_search"))
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .min_size(edit_rect.size())
                .font(font(12.0))
                .text_color(INK)
                .align(egui::Align2::LEFT_CENTER)
                .hint_text(egui::RichText::new("Buscar música, artista ou playlist").font(font(12.0)).color(DIM_3)),
        );
        if state.query != before {
            state.typed_at = time;
            state.devices = false;
        }
        let enter = edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if let Some(query) = search_due(state, time).or_else(|| (enter && !state.query.trim().is_empty()).then(|| state.query.trim().to_string())) {
            state.asked = query.clone();
            pressed.push(format!("search {query}"));
        } else if !state.query.trim().is_empty() && state.query.trim() != state.asked.trim() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
        }
        let mut y = field.bottom() + 8.0;
        let searching = !state.query.trim().is_empty();
        // the lists' pills, while nothing is searched
        if !searching && !state.devices {
            let mut x = inner.left();
            for list in List::EVERY {
                let galley = ui.painter().layout_no_wrap(list.label().to_string(), font(11.5), DIM);
                let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(galley.size().x + 22.0, 24.0));
                let response =
                    ui.interact(rect, ui.scope_id().with(("island_music_list", list.label())), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                let on = state.list == list;
                if on {
                    ui.painter().rect_filled(rect, 12, Color32::from_rgb(0xe8, 0xe9, 0xec));
                } else if response.hovered() {
                    ui.painter().rect_filled(rect, 12, HOVER);
                }
                let ink = if on { Color32::from_rgb(0x11, 0x11, 0x11) } else { DIM };
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, list.label(), font(11.5), ink);
                if response.clicked() {
                    state.list = list;
                }
                x = rect.right() + 4.0;
            }
            y += 32.0;
        }
        // what goes wrong shows a few seconds at the bottom
        let error = library.error.as_ref().filter(|(_, at)| now_ms() - at < ERROR_FOR_MS).map(|(text, _)| text.clone());
        let bottom = inner.bottom() - if error.is_some() { 16.0 } else { 0.0 };
        if let Some(error) = error {
            let galley = clipped(ui.painter(), error, font(10.5), RED, inner.width() - 12.0);
            ui.painter().galley(Pos2::new(inner.left() + 6.0, inner.bottom() - galley.size().y), galley, RED);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        let area = Rect::from_min_max(Pos2::new(card.left() + 6.0, y), Pos2::new(card.right() - 6.0, bottom));
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 1.0);
            ui.spacing_mut().scroll = crate::app::ui::scroll_style();
            let salt = ("island_music_rows", searching, state.devices, state.list);
            egui::ScrollArea::vertical().id_salt(salt).auto_shrink([false, false]).show(ui, |ui| {
                if state.devices {
                    self.music_devices(ui, state, pressed);
                } else if searching {
                    self.music_found(ui, state, pressed);
                } else {
                    self.music_list(ui, state.list, pressed);
                }
            });
        });
    }

    /// A list of the tab: A seguir, Playlists, Curtidas.
    fn music_list(&self, ui: &mut egui::Ui, list: List, pressed: &mut Vec<String>) {
        let library = &self.integrations.spotify.library;
        match list {
            List::Next => {
                for (index, item) in library.queue.iter().enumerate() {
                    if self.music_song(ui, item, false, pressed).clicked() {
                        pressed.extend(skips_to(index));
                    }
                }
                let note = library.queue_note.as_deref().unwrap_or("A fila está vazia.");
                if library.queue.is_empty() {
                    needs(ui, note);
                }
            }
            List::Playlists => {
                if !library.playlists.is_empty() {
                    let liked = PlaylistItem { uri: LIKED_SONGS.into(), name: "Músicas curtidas".into(), ..Default::default() };
                    self.music_playlist(ui, &liked, pressed);
                }
                for playlist in &library.playlists {
                    self.music_playlist(ui, playlist, pressed);
                }
                if library.playlists.is_empty() {
                    needs(ui, library.lists_note.as_deref().unwrap_or("Nenhuma playlist ainda."));
                }
            }
            List::Liked => {
                for item in &library.liked {
                    self.music_song(ui, item, true, pressed);
                }
                if library.liked.is_empty() {
                    needs(ui, library.lists_note.as_deref().unwrap_or("Nenhuma música curtida ainda."));
                }
            }
        }
    }

    /// What the search found: songs, then playlists.
    fn music_found(&self, ui: &mut egui::Ui, state: &Page, pressed: &mut Vec<String>) {
        let library = &self.integrations.spotify.library;
        let found = library.found.as_ref().filter(|found| found.query.trim() == state.asked.trim());
        match found {
            None if library.lists_note.is_some() => needs(ui, library.lists_note.as_deref().unwrap_or_default()),
            None => needs(ui, "Buscando…"),
            Some(found) if found.tracks.is_empty() && found.playlists.is_empty() => needs(ui, "Nada encontrado."),
            Some(found) => {
                if !found.tracks.is_empty() {
                    section(ui, "Músicas");
                    for item in &found.tracks {
                        self.music_song(ui, item, true, pressed);
                    }
                }
                if !found.playlists.is_empty() {
                    section(ui, "Playlists");
                    for playlist in &found.playlists {
                        self.music_playlist(ui, playlist, pressed);
                    }
                }
            }
        }
    }

    /// The account's devices: a press moves the music there.
    fn music_devices(&self, ui: &mut egui::Ui, state: &mut Page, pressed: &mut Vec<String>) {
        let devices = &self.integrations.spotify.library.devices;
        section(ui, "Tocar em");
        if devices.is_empty() {
            needs(ui, "Os aparelhos aparecem com a conta do Spotify conectada em Ajustes › Integrações.");
        }
        for device in devices {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            if response.hovered() {
                ui.painter().rect_filled(rect, 9, HOVER);
            }
            let color = if device.active { GREEN } else { DIM_3 };
            ui.painter().circle_filled(Pos2::new(rect.left() + 14.0, rect.center().y), 3.5, color);
            let words = clipped(ui.painter(), device.name.clone(), semi(12.0), if device.active { GREEN } else { INK }, rect.width() - 110.0);
            ui.painter().galley(Pos2::new(rect.left() + 26.0, rect.center().y - words.size().y / 2.0), words, INK);
            ui.painter().text(Pos2::new(rect.right() - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, &device.kind, font(10.5), DIM_3);
            if response.clicked() && !device.active {
                pressed.push(format!("device {}", device.id));
                state.devices = false;
            }
        }
    }

    /// A song's row: the cover, the title and the artist, the time; on hover Tocar and + Fila when `actions`. Its
    /// response (a press on the row itself).
    fn music_song(&self, ui: &mut egui::Ui, item: &QueueItem, actions: bool, pressed: &mut Vec<String>) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
        let response = response.on_hover_cursor(CursorIcon::PointingHand);
        let hovered = ui.rect_contains_pointer(rect);
        if hovered {
            ui.painter().rect_filled(rect, 9, HOVER);
        }
        let art = Rect::from_min_size(Pos2::new(rect.left() + 6.0, rect.center().y - 15.0), Vec2::splat(30.0));
        let texture = (!item.cover_url.is_empty()).then(|| self.spotify_thumbnail(&item.cover_url)).flatten();
        picture(ui.painter(), art, 5, texture, &item.title);
        let mut right = rect.right() - 8.0;
        if actions && hovered {
            for (label, words) in [("+ Fila", format!("enqueue {}", item.uri)), ("Tocar", format!("play {}", item.uri))] {
                let galley = ui.painter().layout_no_wrap(label.to_string(), font(10.5), INK);
                let chip = Rect::from_min_max(Pos2::new(right - galley.size().x - 18.0, rect.center().y - 11.0), Pos2::new(right, rect.center().y + 11.0));
                let chip_response = ui.interact(chip, ui.scope_id().with(("island_music_chip", label, &item.uri)), Sense::click());
                let fill = if chip_response.hovered() { Color32::from_rgb(0x3a, 0x3d, 0x43) } else { TRACK };
                ui.painter().rect_filled(chip, 11, fill);
                ui.painter().galley(Pos2::new(chip.left() + 9.0, chip.center().y - galley.size().y / 2.0), galley, INK);
                if chip_response.clicked() {
                    pressed.push(words);
                }
                right = chip.left() - 4.0;
            }
        } else if let Some(duration) = item.duration_ms {
            let words = ui.painter().text(Pos2::new(right, rect.center().y), egui::Align2::RIGHT_CENTER, clock(duration), FontId::monospace(10.0), DIM_3);
            right = words.left();
        }
        let text_x = art.right() + 10.0;
        let width = (right - 8.0 - text_x).max(20.0);
        let playing = self.integrations.spotify.now.as_ref().is_some_and(|now| !item.uri.is_empty() && now.playback.track_uri == item.uri);
        let title = clipped(ui.painter(), item.title.clone(), semi(12.0), if playing { GREEN } else { INK }, width);
        ui.painter().galley(Pos2::new(text_x, rect.center().y - 14.0), title, INK);
        let artist = clipped(ui.painter(), item.artist.clone(), font(10.5), DIM_3, width);
        ui.painter().galley(Pos2::new(text_x, rect.center().y + 1.0), artist, DIM_3);
        response
    }

    /// A playlist's row: on hover Tocar; a press on the row plays it too.
    fn music_playlist(&self, ui: &mut egui::Ui, playlist: &PlaylistItem, pressed: &mut Vec<String>) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
        let response = response.on_hover_cursor(CursorIcon::PointingHand);
        if response.hovered() {
            ui.painter().rect_filled(rect, 9, HOVER);
        }
        let art = Rect::from_min_size(Pos2::new(rect.left() + 6.0, rect.center().y - 15.0), Vec2::splat(30.0));
        if playlist.uri == LIKED_SONGS {
            let (from, to) = (Color32::from_rgb(0x45, 0x0a, 0xf5), Color32::from_rgb(0xc4, 0xef, 0xd9));
            ui.painter().rect_filled(art, 5, from.lerp_to_gamma(to, 0.35));
            icons::paint(ui.painter(), icons::HEART, art.center(), 14.0, INK);
        } else {
            let texture = (!playlist.cover_url.is_empty()).then(|| self.spotify_thumbnail(&playlist.cover_url)).flatten();
            picture(ui.painter(), art, 5, texture, &playlist.name);
        }
        let mut right = rect.right() - 8.0;
        if response.hovered() {
            let galley = ui.painter().layout_no_wrap("Tocar".to_string(), font(10.5), INK);
            let chip = Rect::from_min_max(Pos2::new(right - galley.size().x - 18.0, rect.center().y - 11.0), Pos2::new(right, rect.center().y + 11.0));
            ui.painter().rect_filled(chip, 11, TRACK);
            ui.painter().galley(Pos2::new(chip.left() + 9.0, chip.center().y - galley.size().y / 2.0), galley, INK);
            right = chip.left() - 4.0;
        }
        let text_x = art.right() + 10.0;
        let width = (right - 8.0 - text_x).max(20.0);
        let title = clipped(ui.painter(), playlist.name.clone(), semi(12.0), INK, width);
        ui.painter().galley(Pos2::new(text_x, rect.center().y - 14.0), title, INK);
        let mut small = playlist.owner.clone();
        if let Some(total) = playlist.total {
            small = if small.is_empty() { format!("{total} músicas") } else { format!("{small} · {total} músicas") };
        }
        let small = clipped(ui.painter(), small, font(10.5), DIM_3, width);
        ui.painter().galley(Pos2::new(text_x, rect.center().y + 1.0), small, DIM_3);
        if response.clicked() {
            pressed.push(format!("play {}", playlist.uri));
        }
    }
}

/// A list's title ("MÚSICAS"), small and spaced.
fn section(ui: &mut egui::Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let spaced: String = text.to_uppercase().chars().flat_map(|letter| [letter, '\u{2009}']).collect();
    ui.painter().text(Pos2::new(rect.left() + 6.0, rect.center().y + 2.0), egui::Align2::LEFT_CENTER, spaced.trim_end(), font(10.0), DIM_3);
}

/// Why a list is empty, in a quiet line; when it sends the person to Ajustes › Integrações, a button that opens it
/// (asked: "caso não tivesse a configuração de integração, poderia ter um botão para levar até lá").
fn needs(ui: &mut egui::Ui, text: &str) {
    let width = ui.available_width() - 12.0;
    ui.add_space(6.0);
    let galley = ui.painter().layout(text.to_string(), font(10.5), DIM_3, width);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), galley.size().y + 4.0), Sense::hover());
    ui.painter().galley(Pos2::new(rect.left() + 6.0, rect.top()), galley, DIM_3);
    if !text.contains("Ajustes › Integrações") {
        return;
    }
    ui.add_space(8.0);
    let words = ui.painter().layout_no_wrap("Abrir Ajustes › Integrações".into(), font(11.0), INK);
    let (button, response) = ui.allocate_exact_size(Vec2::new(words.size().x + 24.0, 26.0), Sense::click());
    let button = button.translate(Vec2::new(6.0, 0.0));
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    ui.painter().rect_filled(button, 13, if response.hovered() { TRACK } else { HOVER });
    ui.painter().galley(button.center() - words.size() / 2.0, words, INK);
    if response.clicked() {
        ui.ctx().data_mut(|data| data.insert_temp(settings_asked(), true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_reads_as_the_player_shows_it() {
        assert_eq!(clock(84_000), "1:24");
        assert_eq!(clock(198_999), "3:18");
        assert_eq!(clock(3_723_000), "1:02:03");
    }

    #[test]
    fn the_search_waits_for_the_keys_to_rest_and_asks_once() {
        let mut page = Page { query: "ev".into(), typed_at: 10.0, ..Default::default() };
        assert_eq!(search_due(&page, 10.2), None, "still typing");
        assert_eq!(search_due(&page, 10.5).as_deref(), Some("ev"));
        page.asked = "ev".into();
        assert_eq!(search_due(&page, 11.0), None, "asked already");
        page.query = "e".into();
        assert_eq!(search_due(&page, 12.0), None, "two letters at least");
    }

    #[test]
    fn a_song_of_the_queue_is_reached_by_skipping_to_it() {
        assert_eq!(skips_to(0), ["next"]);
        assert_eq!(skips_to(2), ["next", "next", "next"]);
        assert_ne!(placeholder("Evidências"), placeholder("Trem-Bala"), "each its own colours");
    }
}
