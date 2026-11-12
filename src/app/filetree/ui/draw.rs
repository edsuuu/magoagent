use super::*;

/// One node's areas: the header's buttons, then the rows with the search bar under them.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    ctx: &Context,
    id: &str,
    spec: &FileTreeSpec,
    frame: Rect,
    z: f32,
    canvas: Rect,
    dark: bool,
    clip_board: Option<(usize, bool)>,
    focused: bool,
    view: &mut TreeView,
    acts: &mut Vec<Act>,
) {
    let c = Palette::new(dark);
    let clip = frame.intersect(canvas);
    let measure = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::unique(("ft_measure", id))));
    let h = head(&measure, frame, z, &view.git);
    let stroke = |color: Color32| Stroke::new((1.0 * z).max(0.75), color);
    let (root, shown) = (spec.root_path(), spec.shown());
    // back (up one folder) and forward
    let nav = Rect::from_min_max(h.back.min, h.forward.max);
    area(ctx, ("ft_nav", id), nav, clip, |ui| {
        let p = ui.painter().clone();
        let divider = Rect::from_center_size(Pos2::new((h.back.right() + h.forward.left()) / 2.0, nav.center().y), Vec2::new(1.0, 11.375 * z));
        p.rect_filled(divider, 0, c.border);
        for (rect, dir, can, tip, act) in [(h.back, 2, shown != root, "Voltar", Act::Back), (h.forward, 0, !view.forward.is_empty(), "Avançar", Act::Forward)]
        {
            let r = ui.interact(rect, ui.scope_id().with(tip), Sense::click());
            if r.hovered() && can {
                p.rect_filled(rect, cr(rect.width() / 2.0), c.hover);
            }
            let color = if can { c.fg } else { c.fg.gamma_multiply(0.3) };
            chevron(&p, rect.center(), 13.0 * z, dir, stroke(color));
            if r.on_hover_cursor(if can { CursorIcon::PointingHand } else { CursorIcon::Default }).tip(tip).clicked() && can {
                acts.push(act);
                acts.push(Act::Focus);
            }
        }
    });
    // the view menu and the git pill
    let tools = h.pill.as_ref().map_or(h.view, |(pill, _)| h.view.union(*pill));
    let expanded = !view.expanded.is_empty();
    area(ctx, ("ft_tools", id), tools, clip, |ui| {
        let p = ui.painter().clone();
        let r = ui.interact(h.view, ui.scope_id().with("view"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        // no tip over its own menu
        let r = if egui::Popup::is_any_open(ui.ctx()) { r } else { r.tip("Opções de Exibição") };
        if r.hovered() {
            p.rect_filled(h.view, cr(h.view.height() / 2.0), c.hover);
        }
        // lucide's list (size-4) and chevrons-up-down (size-2.5)
        paint_g(&p, G::List, Pos2::new(h.view.left() + 4.875 * z + 6.5 * z, h.view.center().y), 13.0 * z, c.fg);
        let updown = Pos2::new(h.view.right() - 4.875 * z - 4.0625 * z, h.view.center().y);
        outline(&p, updown, 8.125 * z, &[(7.0, 15.0), (12.0, 20.0), (17.0, 15.0)], false, stroke(c.muted));
        outline(&p, updown, 8.125 * z, &[(7.0, 9.0), (12.0, 4.0), (17.0, 9.0)], false, stroke(c.muted));
        egui::Popup::menu(&r)
            .align(egui::RectAlign::BOTTOM_END)
            .gap(6.0)
            .frame(crate::app::ui::menu_look::menu_frame(dark))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| view_menu(ui, spec, expanded, acts));
        if let Some((rect, branch)) = &h.pill {
            let r = ui.interact(*rect, ui.scope_id().with("pill"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if r.clicked() {
                view.branches = crate::app::filetree::branch::local_branches(&root).unwrap_or_default();
            }
            p.rect(*rect, cr(rect.height() / 2.0), Color32::TRANSPARENT, Stroke::new(1.0, c.edge), StrokeKind::Inside);
            // lucide's git-branch (size-3)
            let icon = Pos2::new(rect.left() + 6.5 * z + z + 4.875 * z, rect.center().y);
            let size = 9.75 * z;
            let tint = stroke(c.muted);
            outline(&p, icon, size, &[(6.0, 3.0), (6.0, 15.0)], false, tint);
            outline(&p, icon, size, &arc(18.0, 6.0, 3.0, 0.0, 360.0), true, tint);
            outline(&p, icon, size, &arc(6.0, 18.0, 3.0, 0.0, 360.0), true, tint);
            outline(&p, icon, size, &arc(9.0, 9.0, 9.0, 0.0, 90.0), false, tint);
            let font = FontId::proportional(11.0 * z);
            let text =
                p.text(Pos2::new(icon.x + 4.875 * z + 3.25 * z, rect.center().y), egui::Align2::LEFT_CENTER, branch, font.clone(), c.fg.gamma_multiply(0.8));
            if view.git.changes > 0 {
                p.text(Pos2::new(text.right() + 3.25 * z, rect.center().y), egui::Align2::LEFT_CENTER, view.git.changes.to_string(), font, c.muted);
            }
            let n = view.git.changes;
            let mut query: String = ui.ctx().data(|d| d.get_temp(r.id.with("branch_query"))).unwrap_or_default();
            egui::Popup::menu(&r)
                .align(egui::RectAlign::BOTTOM_END)
                .gap(6.0)
                .frame(crate::app::ui::menu_look::menu_frame(dark))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| crate::app::filetree::branch::branch_menu(ui, &view.branches, &view.git.branch, acts, &mut query));
            // the search starts empty each time the picker opens
            let query_id = r.id.with("branch_query");
            if !egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&r)) {
                query.clear();
            }
            ui.ctx().data_mut(|d| d.insert_temp(query_id, query));
            if !egui::Popup::is_any_open(ui.ctx()) {
                r.tip(format!(
                    "Branch {}, {} arquivo{} alterado{}; clique para trocar",
                    view.git.branch,
                    n,
                    if n == 1 { "" } else { "s" },
                    if n == 1 { "" } else { "s" }
                ));
            }
        }
    });
    // the rows and the search bar: inset from the edges, which stay the canvas's to resize by
    let searching = !view.filter.trim().is_empty();
    let foot = FT_BAR * z + if view.dropped > 0 { FT_NOTE * z } else { 0.0 };
    let body = Rect::from_min_max(Pos2::new(frame.left() + 4.0, frame.top() + FT_BAR * z + 1.0), Pos2::new(frame.right() - 4.0, frame.bottom() - 4.0));
    let list = Rect::from_min_max(body.min, Pos2::new(body.right(), body.bottom() - foot + 4.0));
    // a search's results have their count over them, and two-line rows
    let (row_h, note_h) = if searching { (FT_HIT, if view.rows.is_empty() { 0.0 } else { FT_NOTE }) } else { (FT_ROW, 0.0) };
    let cut_h = if searching && view.hits_cut { FT_NOTE } else { 0.0 };
    let top = list.top() + note_h * z;
    let shown_h = (list.bottom() - top) / z;
    view.body_h = shown_h;
    let total = view.rows.len() as f32 * row_h + cut_h;
    let bar_w = if total > shown_h { FT_SCROLL * z } else { 0.0 };
    area(ctx, ("ft_body", id), body, clip, |ui| {
        let p = ui.painter().with_clip_rect(clip);
        if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(body) {
            acts.push(Act::Focus);
        }
        if ui.rect_contains_pointer(list) {
            view.scroll -= ui.input(|i| i.smooth_scroll_delta.y) / z;
        }
        if let Some((path, left)) = view.reveal.take() {
            match view.rows.iter().position(|r| r.path == path) {
                Some(i) => view.scroll = (i as f32 * row_h - (shown_h - row_h) / 2.0).max(0.0),
                None if left > 1 => view.reveal = Some((path, left - 1)),
                None => {}
            }
        }
        view.scroll = view.scroll.clamp(0.0, (total - shown_h).max(0.0));
        // empty space: the menu of the folder shown
        let blank = ui.interact(list, ui.scope_id().with("blank"), Sense::click());
        context(&blank, dark, |ui| row_menu(ui, id, None, &shown, clip_board, false, acts));
        if let Some(note) = view.empty_note(spec) {
            if searching {
                // the reference centers it
                let galley = p.layout(note, FontId::proportional(13.0 * z), c.muted, list.width() - 19.5 * z);
                p.galley(list.center() - galley.size() / 2.0, galley, c.muted);
            } else {
                let galley = p.layout(note, FontId::proportional(12.0 * z), c.muted, list.width() - 19.5 * z);
                p.galley(Pos2::new(list.left() + 9.75 * z, list.top() + 6.5 * z), galley, c.muted);
            }
        }
        let width = list.width() - bar_w;
        let first = (view.scroll / row_h).floor() as usize;
        let last = (((view.scroll + shown_h) / row_h).ceil() as usize + 1).min(view.rows.len());
        let list_clip = Rect::from_min_max(Pos2::new(list.left(), top), list.right_bottom()).intersect(clip);
        let ring = focused && view.kb;
        if note_h > 0.0 {
            let galley = p.layout_no_wrap(hits_note(view.rows.len()), FontId::proportional(11.0 * z), c.muted);
            p.galley(Pos2::new(list.left() + 8.125 * z, list.top() + (note_h * z - galley.size().y) / 2.0), galley, c.muted);
            p.hline(frame.left() + 1.0..=frame.right() - 1.0, list.top() + note_h * z, Stroke::new(1.0, c.border));
        }
        for (i, row) in view.rows.iter().enumerate().take(last).skip(first) {
            let rect = Rect::from_min_size(Pos2::new(list.left(), top + (i as f32 * row_h - view.scroll) * z), Vec2::new(width, row_h * z));
            let hit = rect.intersect(list_clip);
            if hit.height() < 1.0 {
                continue;
            }
            let r = ui.interact(hit, ui.scope_id().with(("row", i)), Sense::click());
            let p = p.with_clip_rect(list_clip);
            let selected = view.selected.as_ref() == Some(&row.path);
            if selected || (searching && r.hovered()) {
                p.rect_filled(rect, cr(4.0 * z), c.hover);
            }
            if selected && ring {
                p.rect_stroke(rect, cr(4.0 * z), Stroke::new(2.0 * z, c.ring), StrokeKind::Inside);
            }
            let cy = rect.center().y;
            if searching {
                // the file's icon (the IDE's, 13 px here), its name and, under it, the folder it is in
                let icon = Pos2::new(rect.left() + 8.125 * z + 5.6875 * z, cy);
                icons::file_on(&p, &row.path, icon, 0.8125 * z, !dark);
                let left = rect.left() + 8.125 * z + 11.375 * z + 6.5 * z;
                let font = FontId::proportional(12.0 * z);
                let name = fit(&p, &row.name, &font, rect.right() - 8.125 * z - left);
                // ponytail: every hit is two lines high, a file in the root too; the reference's is one when it has no folder
                let line = if row.hint.is_empty() { cy } else { cy - 7.35 * z };
                p.text(Pos2::new(left, line), egui::Align2::LEFT_CENTER, name, font, c.fg);
                if !row.hint.is_empty() {
                    let font = FontId::proportional(10.5 * z);
                    p.text(Pos2::new(left, cy + 8.4 * z), egui::Align2::LEFT_CENTER, fit(&p, &row.hint, &font, rect.right() - 8.125 * z - left), font, c.muted);
                }
            } else {
                let mark = view.git.of(&row.path, row.dir);
                let color = name_color(mark, view.git.ignored(&row.path), dark);
                // the indentation's guides: a line down from each folder above, under its chevron
                for level in 0..row.depth {
                    p.vline(rect.left() + (4.0 + level as f32 * 16.0 + 6.5) * z, rect.y_range(), Stroke::new(1.0, c.border));
                }
                let slot = Rect::from_center_size(Pos2::new(rect.left() + (4.0 + row.depth as f32 * 16.0) * z + 6.5 * z, cy), Vec2::splat(13.0 * z));
                if row.dir {
                    let chev = ui.interact(slot.intersect(list_clip), ui.scope_id().with(("chevron", i)), Sense::click());
                    if chev.hovered() {
                        p.rect_filled(slot, cr(2.0 * z), c.fg.gamma_multiply(0.1));
                    }
                    chevron(&p, slot.center(), 9.75 * z, if row.open { 1 } else { 0 }, stroke(c.muted));
                    if chev.clicked() {
                        acts.push(Act::Toggle(row.path.clone()));
                        acts.push(Act::Focus);
                    }
                }
                // the IDE's icons, in their own colors: only the name wears the git mark
                let icon = Pos2::new(slot.right() + 4.875 * z + 6.5 * z, cy);
                if row.dir {
                    icons::paint_on(&p, icons::FOLDER, Rect::from_center_size(icon, Vec2::splat(16.0 * z)), !dark);
                } else {
                    icons::file_on(&p, &row.path, icon, z, !dark);
                }
                let badge_w = if mark.is_some_and(|(_, rolled)| !rolled) { 16.25 * z + 4.875 * z } else { 0.0 };
                let left = icon.x + 6.5 * z + 4.875 * z;
                let font = FontId::proportional(13.0 * z);
                let name = fit(&p, &row.name, &font, rect.right() - 6.5 * z - badge_w - left);
                // an error in it (or under it): the name in red with a wavy line, as the reference's project view
                let bad = view.problems.contains(&row.path);
                let color = if bad { Some(error_color(dark)) } else { color };
                let text = p.text(Pos2::new(left, cy), egui::Align2::LEFT_CENTER, name, font, color.unwrap_or(c.fg));
                if bad {
                    wave(&p, text.left(), text.right(), text.bottom() - 1.0 * z, z, error_color(dark));
                }
                if !row.hint.is_empty() {
                    let font = FontId::proportional(11.0 * z);
                    let room = rect.right() - 6.5 * z - badge_w - text.right() - 6.0 * z;
                    p.text(Pos2::new(text.right() + 6.0 * z, cy), egui::Align2::LEFT_CENTER, fit(&p, &row.hint, &font, room), font, c.muted);
                }
                if let Some((s, false)) = mark {
                    // the reference's 10px bold mono letter: smaller in the app's wider one, struck twice for the weight
                    for dx in [0.0, 0.35 * z] {
                        p.text(
                            Pos2::new(rect.right() - 6.5 * z - 8.125 * z + dx, cy),
                            egui::Align2::CENTER_CENTER,
                            badge(s),
                            FontId::monospace(8.4 * z),
                            status_color(s, dark),
                        );
                    }
                }
            }
            if r.clicked() || r.secondary_clicked() {
                view.kb = false;
                acts.push(Act::Select(row.path.clone()));
                acts.push(Act::Focus);
            }
            if r.double_clicked() {
                acts.push(Act::Activate(row.path.clone()));
            }
            let diffable = matches!(view.git.of(&row.path, row.dir), Some((Status::Modified | Status::Renamed | Status::Conflict, false)));
            context(&r, dark, |ui| row_menu(ui, id, Some(row), &shown, clip_board, diffable, acts));
        }
        if cut_h > 0.0 {
            let note = Rect::from_min_size(Pos2::new(list.left(), top + (view.rows.len() as f32 * row_h - view.scroll) * z), Vec2::new(width, cut_h * z));
            if note.intersects(list_clip) {
                let p = p.with_clip_rect(list_clip);
                p.hline(frame.left() + 1.0..=frame.right() - 1.0, note.top(), Stroke::new(1.0, c.border));
                p.text(
                    Pos2::new(note.left() + 8.125 * z, note.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("Parou em {}. Existem mais.", hits_note(view.rows.len())),
                    FontId::proportional(11.0 * z),
                    c.muted,
                );
            }
        }
        // the scrollbar: 8 wide, its thumb the reference's `--border-strong`
        if bar_w > 0.0 {
            let track = Rect::from_min_max(Pos2::new(list.right() - bar_w, top), Pos2::new(list.right(), list.bottom()));
            let span = (total - shown_h).max(1.0);
            let thumb_h = (shown_h / total * track.height()).max(20.0 * z).min(track.height());
            let thumb = Rect::from_min_size(Pos2::new(track.left(), track.top() + view.scroll / span * (track.height() - thumb_h)), Vec2::new(bar_w, thumb_h));
            let drag = ui.interact(thumb.intersect(clip), ui.scope_id().with("thumb"), Sense::drag());
            if drag.dragged() {
                view.scroll = (view.scroll + drag.drag_delta().y / (track.height() - thumb_h).max(1.0) * span).clamp(0.0, span);
            }
            let color = if drag.hovered() || drag.dragged() { c.muted } else { c.strong };
            p.rect_filled(thumb, cr(4.0 * z), color);
        }
        // the search bar under the rows
        let bar = Rect::from_min_size(Pos2::new(frame.left(), list.bottom()), Vec2::new(frame.width(), FT_BAR * z));
        p.hline(frame.left() + 1.0..=frame.right() - 1.0, bar.top(), Stroke::new(1.0, c.border));
        // the editor's, Banco de Dados' and Alterações' buttons, at the bar's end (filetree/bar.rs)
        let mut edge = crate::app::filetree::bar::buttons(ui, &p, bar, z, spec, &c, dark, acts);
        // the count of the tree's rows, outside the field
        if searching {
            let count = format!("{} exibidos", view.tree_rows);
            let galley = p.layout_no_wrap(count, FontId::proportional(11.0 * z), c.muted);
            p.galley(Pos2::new(edge - galley.size().x, bar.center().y - galley.size().y / 2.0), galley.clone(), c.muted);
            edge -= galley.size().x + 6.5 * z;
        }
        let pill = Rect::from_min_max(Pos2::new(bar.left() + 6.5 * z, bar.center().y - 9.75 * z), Pos2::new(edge, bar.center().y + 9.75 * z));
        p.rect_filled(pill, cr(pill.height() / 2.0), c.hover);
        let lens = Pos2::new(pill.left() + 6.5 * z + 4.875 * z, pill.center().y);
        p.circle_stroke(lens + Vec2::new(-0.6 * z, -0.6 * z), 3.4 * z, stroke(c.muted));
        p.line_segment([lens + Vec2::new(1.9 * z, 1.9 * z), lens + Vec2::new(4.4 * z, 4.4 * z)], stroke(c.muted));
        let mut right = pill.right() - 6.5 * z;
        if !view.filter.is_empty() {
            let x = Rect::from_center_size(Pos2::new(right - 6.5 * z, pill.center().y), Vec2::splat(13.0 * z));
            let r = ui.interact(x, ui.scope_id().with("clear"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Limpar filtro");
            for (a, b) in [((-2.5, -2.5), (2.5, 2.5)), ((-2.5, 2.5), (2.5, -2.5))] {
                p.line_segment([x.center() + Vec2::new(a.0, a.1) * z, x.center() + Vec2::new(b.0, b.1) * z], stroke(c.muted));
            }
            if r.clicked() {
                view.filter.clear();
                view.dirty = true;
            }
            right = x.left() - 4.875 * z;
        }
        let field = Rect::from_min_max(Pos2::new(lens.x + 9.75 * z / 2.0 + 4.875 * z, pill.top()), Pos2::new(right, pill.bottom()));
        let edit = egui::TextEdit::singleline(&mut view.filter)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .align(egui::Align2::LEFT_CENTER)
            .font(FontId::proportional(12.0 * z))
            .text_color(c.fg)
            .hint_text(egui::RichText::new("Buscar").size(12.0 * z).color(c.muted))
            .desired_width(field.width());
        // as tall as its text, in the middle of the pill
        let line = Rect::from_center_size(field.center(), Vec2::new(field.width(), 15.0 * z));
        let typed = ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 0.0;
            ui.put(line, edit).changed()
        });
        if typed.inner {
            view.dirty = true;
            view.scroll = 0.0;
        }
        if view.dropped > 0 {
            let note = Rect::from_min_size(Pos2::new(frame.left(), bar.bottom()), Vec2::new(frame.width(), FT_NOTE * z));
            p.hline(frame.left() + 1.0..=frame.right() - 1.0, note.top(), Stroke::new(1.0, c.border));
            let text = format!("Exibindo um número limitado de itens. Mais {} não estão listados.", view.dropped);
            let font = FontId::proportional(11.0 * z);
            p.text(
                Pos2::new(note.left() + 8.125 * z, note.center().y),
                egui::Align2::LEFT_CENTER,
                fit(&p, &text, &font, note.width() - 16.25 * z),
                font,
                c.muted,
            );
        }
    });
}
