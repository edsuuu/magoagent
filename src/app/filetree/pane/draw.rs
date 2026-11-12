use super::*;

/// lucide's save, at 24-grid points.
pub(super) fn glyph_save(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    let grid = [(15.2, 3.0), (16.6, 3.6), (20.4, 7.4), (21.0, 8.8), (21.0, 19.0), (19.0, 21.0), (5.0, 21.0), (3.0, 19.0), (3.0, 5.0), (5.0, 3.0)];
    outline(p, c, size, &grid, true, stroke);
    outline(p, c, size, &[(17.0, 21.0), (17.0, 14.0), (16.0, 13.0), (8.0, 13.0), (7.0, 14.0), (7.0, 21.0)], false, stroke);
    outline(p, c, size, &[(7.0, 3.0), (7.0, 7.0), (8.0, 8.0), (15.0, 8.0)], false, stroke);
}

/// lucide's square-arrow-out-up-right.
pub(super) fn glyph_open(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    outline(p, c, size, &[(21.0, 13.0), (21.0, 19.0), (19.0, 21.0), (5.0, 21.0), (3.0, 19.0), (3.0, 5.0), (5.0, 3.0), (11.0, 3.0)], false, stroke);
    outline(p, c, size, &[(21.0, 3.0), (12.0, 12.0)], false, stroke);
    outline(p, c, size, &[(15.0, 3.0), (21.0, 3.0), (21.0, 9.0)], false, stroke);
}

/// lucide's settings-2: two lines, each with its knob.
pub(super) fn glyph_settings(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    outline(p, c, size, &[(20.0, 7.0), (11.0, 7.0)], false, stroke);
    outline(p, c, size, &[(14.0, 17.0), (5.0, 17.0)], false, stroke);
    outline(p, c, size, &crate::app::filetree::menu::arc(17.0, 17.0, 3.0, 0.0, 360.0), true, stroke);
    outline(p, c, size, &crate::app::filetree::menu::arc(7.0, 7.0, 3.0, 0.0, 360.0), true, stroke);
}

pub(crate) fn glyph_x(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    outline(p, c, size, &[(18.0, 6.0), (6.0, 18.0)], false, stroke);
    outline(p, c, size, &[(6.0, 6.0), (18.0, 18.0)], false, stroke);
}

/// The pane of a node: the strip of tabs, its save and close buttons, then the file (or why there is none).
#[allow(clippy::too_many_arguments)]
pub(crate) fn pane_ui(
    ctx: &Context,
    id: &str,
    frame: Rect,
    tree: Rect,
    pane: Rect,
    z: f32,
    canvas: Rect,
    dark: bool,
    elevated: bool,
    ed: &mut Editor,
    acts: &mut Vec<PaneAct>,
) {
    let c = Palette::new(dark);
    let clip = pane.intersect(canvas);
    let stroke = |color: Color32| Stroke::new((1.0 * z).max(0.75), color);
    // an edit keeps the tab it was made in; the disk's news, once a second
    if ed.active.as_ref().is_some_and(|a| ed.preview.as_ref() == Some(a)) && ed.dirty() {
        ed.preview = None;
    }
    let now = ctx.input(|i| i.time);
    if now - ed.last_check > 1.0 {
        ed.last_check = now;
        for tab in &mut ed.tabs {
            if let Body::Text(t) = &mut tab.body {
                t.check_disk(&tab.path);
            }
        }
    }
    // the project's news: its index, who wrote the code forward, where a Ctrl+click's name is declared
    match ed.code.poll() {
        Some(Found::At(path, line, col)) => acts.push(PaneAct::Goto(path, line, col)),
        Some(Found::Nowhere(word)) => acts.push(PaneAct::Missing(word)),
        // the click was on the declaration itself: its usages, as the reference IDE
        Some(Found::Own(name, path, line, col)) => ed.list_usages(&name, &path, line, col, ed.pressed, ctx),
        None => {}
    }
    ed.sync_compares();
    ed.problems.sync(
        ed.code.root(),
        ed.tabs.iter().filter_map(|t| match &t.body {
            Body::Text(b) => Some((t.path.as_path(), b.text.as_str(), b.lang)),
            _ => None,
        }),
    );
    let is_code = |t: &PaneTab| matches!(&t.body, Body::Text(b) if b.lang.kind == crate::ide::highlight::Kind::Code);
    if let Some(path) = ed.active.clone().filter(|a| ed.tabs.iter().any(|t| &t.path == a && is_code(t))) {
        ed.code.blame(&path, ctx);
    }
    let (dirty, wrap) = (ed.dirty(), ed.wrap);
    let tabs_dirty: Vec<bool> = ed.tabs.iter().map(PaneTab::dirty).collect();
    area(ctx, ("ft_pane", id), pane, clip, |ui| {
        let p = ui.painter().with_clip_rect(clip);
        let strip = Rect::from_min_size(pane.min, Vec2::new(pane.width(), STRIP * z));
        p.rect_filled(strip, 0, c.hover);
        p.hline(pane.left()..=pane.right(), strip.bottom(), Stroke::new(1.0, c.border));
        // the buttons at the right: for text the settings and save, for a picture or PDF the system's
        // program; always close the pane
        let button = 19.5 * z;
        let mut right = pane.right() - 3.25 * z;
        let mut buttons = vec![];
        let shown = ed.active.as_ref().and_then(|a| ed.tabs.iter().find(|t| &t.path == a));
        let media = shown.is_some_and(|t| matches!(t.body, Body::Media(_) | Body::External(_)));
        let tools: &[(&str, char)] = if shown.is_none_or(|t| matches!(t.body, Body::Diff(_) | Body::Compare(_))) {
            &[("Ocultar Editor", 'x')]
        } else if media {
            &[("Ocultar Editor", 'x'), ("Abrir no App Padrão", 'o')]
        } else {
            &[("Ocultar Editor", 'x'), ("Salvar", 's'), ("Configurações do Editor", 'g')]
        };
        for (tip, kind) in tools.iter().copied() {
            let rect = Rect::from_center_size(Pos2::new(right - button / 2.0, strip.center().y), Vec2::splat(button));
            right = rect.left();
            buttons.push((tip, kind, rect));
        }
        let tabs_end = right - 3.25 * z;
        for (tip, kind, rect) in buttons {
            let on = kind != 's' || dirty;
            let r = ui.interact(rect.intersect(clip), ui.scope_id().with(tip), if on { Sense::click() } else { Sense::hover() });
            if r.hovered() && on {
                p.rect_filled(rect, cr(button / 2.0), c.fg.gamma_multiply(0.1));
            }
            let color = if !on {
                c.muted.gamma_multiply(0.4)
            } else if r.hovered() {
                c.fg
            } else {
                c.muted
            };
            match kind {
                'x' => glyph_x(&p, rect.center(), 11.375 * z, stroke(color)),
                's' => glyph_save(&p, rect.center(), 11.375 * z, stroke(color)),
                'o' => glyph_open(&p, rect.center(), 11.375 * z, stroke(color)),
                _ => glyph_settings(&p, rect.center(), 11.375 * z, stroke(color)),
            }
            let r = r.on_hover_cursor(CursorIcon::PointingHand);
            let r = if kind == 'g' && egui::Popup::is_any_open(ui.ctx()) { r } else { r.tip(tip) };
            if kind == 'g' {
                egui::Popup::menu(&r).align(egui::RectAlign::BOTTOM_END).gap(6.0).frame(menu_frame(dark)).show(|ui| {
                    ui.set_width(160.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if menu_row(ui, Row { lead: Lead::Tick(ed.wrap), ..Row::new(None, "Quebrar Linhas") }).clicked() {
                        acts.push(PaneAct::Wrap);
                        ui.close();
                    }
                });
            } else if r.clicked() {
                let path = ed.active.clone().unwrap_or_default();
                acts.push(match kind {
                    'x' => PaneAct::Hide,
                    'o' => PaneAct::OpenDefault(path),
                    _ => PaneAct::Save,
                });
            }
        }
        // the tabs, up to the buttons
        let tabs = Rect::from_min_max(strip.min, Pos2::new(tabs_end, strip.bottom()));
        if elevated && ui.interact(tabs.intersect(clip), ui.scope_id().with("strip_blank"), Sense::click()).clicked() {
            acts.push(PaneAct::Leave);
        }
        tabs_ui(ui, ed, tabs, tabs.intersect(clip), z, dark, &c, &tabs_dirty, acts);
        // the file
        let body = Rect::from_min_max(Pos2::new(pane.left(), strip.bottom() + 1.0), pane.max);
        let active = ed.active.clone();
        let diags = active.as_ref().map(|a| ed.problems.for_file(a)).unwrap_or_default();
        match active.and_then(|a| ed.tabs.iter_mut().find(|t| t.path == a)) {
            None => {
                let galley = p.layout(
                    "Selecione um arquivo para editá-lo ou pré-visualizá-lo aqui.".into(),
                    FontId::proportional(12.0 * z),
                    c.muted,
                    (body.width() - 26.0 * z).max(10.0),
                );
                p.galley(body.center() - galley.size() / 2.0, galley, c.muted);
            }
            Some(tab) => {
                let (act, _) = ui.scope(|ui| body_ui(ui, body, &mut tab.body, &tab.path, z, wrap, &c, &ed.code, &diags)).inner;
                match act {
                    BodyAct::Code(click) => {
                        if let Body::Text(t) = &tab.body {
                            let label = |i: usize| t.laid.as_ref().and_then(|l| l.labels.get(i));
                            match click {
                                Click::Name(at, anchor) => {
                                    ed.pressed = anchor;
                                    // a framework's string answers now, a name once the project is searched (`poll`)
                                    if let Some(Found::At(path, line, col)) = ed.code.declaration(&tab.path, t.lang, &t.text, at, ctx) {
                                        acts.push(PaneAct::Goto(path, line, col));
                                    }
                                }
                                Click::Usages(i, anchor) => {
                                    if let Some(l) = label(i) {
                                        let decl = t.text.lines().nth(l.line).unwrap_or_default();
                                        ed.popup = Some(ListPopup::usages(&ed.code, &l.name, (&tab.path, l.line, l.col), decl, t.lang.id, anchor, ctx));
                                    }
                                }
                                // the author opens the side column of who wrote each line (and closes it again), not a popup
                                Click::Author(..) => acts.push(PaneAct::Annotate(tab.path.clone())),
                                Click::Commit(commit, anchor) => ed.popup = Some(ListPopup::files(&tab.path, commit, anchor, ctx)),
                                Click::Find(query) => acts.push(PaneAct::Find(query)),
                                Click::Comment(req) => acts.push(PaneAct::Comment(req)),
                                Click::Diff => acts.push(PaneAct::Compare(tab.path.clone())),
                                Click::Goto(line, col) => acts.push(PaneAct::Goto(tab.path.clone(), line, col)),
                            }
                        }
                    }
                    BodyAct::Reveal => acts.push(PaneAct::Reveal(tab.path.clone())),
                    BodyAct::OpenDefault => acts.push(PaneAct::OpenDefault(tab.path.clone())),
                    BodyAct::Retry => acts.push(PaneAct::Retry(tab.path.clone())),
                    BodyAct::KeepMine => acts.push(PaneAct::Save),
                    BodyAct::None => {}
                }
            }
        }
        // the seam: dragged, it moves the split
        let grab = Rect::from_min_max(Pos2::new(tree.right() - 3.0 * z, pane.top()), Pos2::new(tree.right() + 6.0 * z, pane.bottom()));
        let r = ui.interact(grab.intersect(clip), ui.scope_id().with("seam"), Sense::drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);
        if r.dragged() {
            if let Some(pos) = r.interact_pointer_pos() {
                acts.push(PaneAct::Seam(((pos.x - frame.left()) / z).round()));
            }
        }
    });
    // over everything: the list a declaration's label opened, and "Localizar nos Arquivos"
    match popup_ui(ctx, id, &mut ed.popup) {
        Some(Picked::Usage(path, line, col)) => acts.push(PaneAct::Goto(path, line, col)),
        Some(Picked::Commit(file, commit)) => acts.push(PaneAct::Diff(file, commit)),
        None => {}
    }
    if let Some(find) = &mut ed.find {
        match find_ui(ctx, id, ed.code.root(), canvas, find, &mut ed.texts) {
            FindAct::Open(path, line, col) => {
                ed.find = None;
                acts.push(PaneAct::Goto(path, line, col));
            }
            FindAct::Close => ed.find = None,
            FindAct::None => {}
        }
    }
}
