//! The tabs of a file tree node's editor pane, small as the reference IDE's: the file's icon and its name, the one forward
//! on a rounded blue chip, the others bare; the X at the right, an unsaved text's dot in its place.

use crate::app::filetree::menu::{Row, menu_row};
use crate::app::filetree::pane::{Editor, PaneAct, glyph_x};
use crate::app::filetree::ui::{Palette, cr, fit};
use crate::app::ui::menu_look::menu_frame;
use crate::ide::{icons, theme as t};
use crate::prelude::*;

/// The strip's height, a chip's, its widest and the room around its name: before it (the icon) and after it (the X).
pub(crate) const STRIP: f32 = 30.0;
const CHIP: f32 = 22.0;
const WIDEST: f32 = 210.0;
const BEFORE: f32 = 25.0;
const AFTER: f32 = 22.0;

/// The tabs in `strip` (the pane's top, up to its buttons), painted inside `clip`; `dirty` says which have unsaved text.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tabs_ui(ui: &mut egui::Ui, ed: &Editor, strip: Rect, clip: Rect, z: f32, dark: bool, c: &Palette, dirty: &[bool], acts: &mut Vec<PaneAct>) {
    let sp = ui.painter().with_clip_rect(clip);
    let font = FontId::proportional(12.0 * z);
    let names: Vec<String> = ed.tabs.iter().map(|tab| tab.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())).collect();
    let galley = |name: &str, italics: bool, color: Color32| {
        let mut job = egui::text::LayoutJob::simple_singleline(fit(&sp, name, &font, (WIDEST - BEFORE - AFTER) * z), font.clone(), color);
        job.sections[0].format.italics = italics;
        sp.layout_job(job)
    };
    let preview = |i: usize| ed.preview.as_ref() == Some(&ed.tabs[i].path);
    let widths: Vec<f32> = names.iter().enumerate().map(|(i, name)| galley(name, preview(i), c.fg).size().x + (BEFORE + AFTER) * z).collect();
    // the tab forward stays in view: the strip slides left by what would be cut of it
    let (lead, gap) = (4.0 * z, 2.0 * z);
    let forward = ed.tabs.iter().position(|tab| ed.active.as_ref() == Some(&tab.path));
    let end = forward.map_or(0.0, |i| lead + widths[..=i].iter().sum::<f32>() + gap * i as f32);
    let mut x = strip.left() + lead - (end + lead - strip.width()).max(0.0);
    for (i, tab) in ed.tabs.iter().enumerate() {
        let chip = Rect::from_min_size(Pos2::new(x, strip.center().y - CHIP * z / 2.0), Vec2::new(widths[i], CHIP * z));
        x += widths[i] + gap;
        if !chip.intersects(clip) {
            continue;
        }
        let (active, italic) = (forward == Some(i), preview(i));
        let r = ui.interact(chip.intersect(clip), ui.scope_id().with(("tab", i)), Sense::click());
        let slot = Rect::from_center_size(Pos2::new(chip.right() - 11.0 * z, chip.center().y), Vec2::splat(14.0 * z));
        let over_x = ui.rect_contains_pointer(slot) && r.hovered();
        if active {
            // the IDE's selected tab; on the light card, the same blue lightened
            let (fill, edge) = if dark { (t::TAB_SELECTED, t::TAB_BORDER) } else { (Color32::from_rgb(0xdc, 0xe7, 0xff), Color32::from_rgb(0xa6, 0xc0, 0xf7)) };
            sp.rect(chip, cr(6.0 * z), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
        } else if r.hovered() {
            sp.rect_filled(chip, cr(6.0 * z), c.fg.gamma_multiply(0.07));
        }
        icons::file_on(&sp, &tab.path, Pos2::new(chip.left() + 13.0 * z, chip.center().y), 0.875 * z, !dark);
        // unsaved text shows its dot until the pointer is on the X; the tab forward always has its X
        if dirty[i] && !over_x {
            sp.circle_filled(slot.center(), 3.0 * z, Color32::from_rgb(0xf0, 0x88, 0x3e));
        } else if r.hovered() || active {
            if over_x {
                sp.rect_filled(slot, cr(3.0 * z), c.fg.gamma_multiply(0.15));
            }
            glyph_x(&sp, slot.center(), 8.5 * z, Stroke::new((1.0 * z).max(0.75), c.muted));
        }
        let color = if active { c.fg } else { c.muted };
        let name = galley(&names[i], italic, color);
        sp.galley(Pos2::new(chip.left() + BEFORE * z, chip.center().y - name.size().y / 2.0), name, color);
        if r.clicked() {
            acts.push(if over_x { PaneAct::Close(tab.path.clone()) } else { PaneAct::Select(tab.path.clone()) });
        }
        if r.middle_clicked() {
            acts.push(PaneAct::Close(tab.path.clone()));
        }
        let unsaved = if dirty[i] { ", alterações não salvas" } else { "" };
        let r = r.on_hover_text(format!("{}{unsaved}", tab.path.display()));
        let (path, pinned) = (tab.path.clone(), !italic);
        egui::Popup::context_menu(&r).frame(menu_frame(dark)).show(|ui| {
            ui.set_width(190.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            let item = |ui: &mut egui::Ui, label: &str| menu_row(ui, Row::new(None, label)).clicked();
            if !pinned && item(ui, "Manter Aberto") {
                acts.push(PaneAct::Pin(path.clone()));
                ui.close();
            }
            if item(ui, "Fechar Aba") {
                acts.push(PaneAct::Close(path.clone()));
                ui.close();
            }
            if item(ui, "Fechar Outras Abas") {
                acts.push(PaneAct::CloseOthers(path.clone()));
                ui.close();
            }
            if item(ui, "Fechar Todas as Abas") {
                acts.push(PaneAct::CloseAll);
                ui.close();
            }
            if item(ui, "Copiar Caminho") {
                acts.push(PaneAct::CopyPath(path.clone()));
                ui.close();
            }
            if item(ui, "Mostrar na Árvore") {
                acts.push(PaneAct::RevealInTree(path.clone()));
                ui.close();
            }
        });
    }
}
