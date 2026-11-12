use super::*;

impl DrawingApp {
    /// The card, its header (globe, name, and the size while it is resized) and the page's
    /// backdrop. The address bar on top is live (`portal_chrome`), the page a native view below.
    pub(crate) fn paint_portal(&self, painter: &Painter, element: &Element) {
        let b = local_bounds(element);
        let (rect, zoom) = self.terminal_frame(element);
        let radius = (8.0 * zoom).min(255.0) as u8;
        painter.rect(rect, radius, Color32::from_gray(0x24), Stroke::new(1.0, Color32::from_gray(0x3a)), StrokeKind::Inside);
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        let spec = element.portal.clone().unwrap_or_default();
        let (muted, light) = (Color32::from_gray(0x8a), Color32::from_gray(0xd0));
        let chrome = if spec.chrome_hidden { 0.0 } else { PORTAL_CHROME * zoom };
        if !spec.chrome_hidden {
            let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), PORTAL_HEADER * zoom));
            paint_icon_scaled(&painter, Icon::Tool(Tool::Portal), header.left_center() + Vec2::new(18.0 * zoom, 0.0), muted, 0.75 * zoom, 0.0);
            let name = if spec.name.is_empty() { "Navegador" } else { &spec.name };
            painter.text(header.left_center() + Vec2::new(32.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, name, FontId::proportional(12.0 * zoom), light);
            // the size only while this portal is being resized
            let mut right = header.right() - 12.0 * zoom;
            if self.size_shown(element) {
                let size = crate::app::browser::extras::size_text(b.width(), b.height());
                let galley = painter.text(Pos2::new(right, header.center().y), egui::Align2::RIGHT_CENTER, size, FontId::monospace(11.0 * zoom), muted);
                right -= galley.width() + 8.0 * zoom;
            }
            if spec.muted {
                crate::app::browser::extras::paint_muted(&painter, Pos2::new(right - 6.0 * zoom, header.center().y), zoom, muted);
            }
        }
        let page = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + chrome), rect.max);
        let note = if spec.unloaded {
            "Descarregado"
        } else if cfg!(target_os = "linux") {
            "Navegadores ainda não abrem no Linux"
        } else {
            "Carregando…"
        };
        painter.text(page.center(), egui::Align2::CENTER_CENTER, note, FontId::proportional(13.0 * zoom), muted);
        if spec.unloaded {
            // ponytail: no Recarregar button on the page; the bar's and the menu's bring it back
            painter.text(
                page.center() + Vec2::new(0.0, 20.0 * zoom),
                egui::Align2::CENTER_CENTER,
                "Parado para economizar recursos",
                FontId::proportional(11.5 * zoom),
                muted,
            );
        }
        // while the canvas moves, the page's picture moves with the card (`sync_portals`)
        #[cfg(not(target_os = "linux"))]
        if let (Some(still), Some(r)) = (self.portal_stills.get(&element.id), self.portal_page_rect(element)) {
            painter.image(still.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
    }

    /// Another window is enlarged over the canvas: this node's live parts (a portal's page and bar,
    /// a terminal's search bar and processes) stay under it.
    pub(crate) fn lifted_other(&self, element: &Element) -> bool {
        self.terminal_elevated.as_ref().is_some_and(|id| id != &element.id)
    }

    /// Native views sit above everything the app draws, so pages step aside while a menu or
    /// dialog is open.
    pub(super) fn portals_covered(&self) -> bool {
        self.extra_open
            || self.palette_open
            || self.dialog_open()
            || self.elevated_index().is_some_and(|i| self.scene.elements[i].portal.is_none())
            || self.chat_open
            || self.floors_ui.fx.engaged
    }

    /// Big enough on screen for a live address bar.
    pub(crate) fn chrome_live(&self, element: &Element) -> bool {
        // at the scale it is shown: an enlarged portal is at 100% whatever the canvas zoom
        let zoom = self.terminal_frame(element).1;
        zoom >= 0.45 && local_bounds(element).width() * zoom >= 200.0
    }

    /// Where a portal's page goes on screen, if it should show: the card's inside, under the
    /// header and the bar (nothing above it when the bar is hidden).
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn portal_page_rect(&self, element: &Element) -> Option<Rect> {
        let (frame, zoom) = self.terminal_frame(element);
        let chrome = if element.portal.as_ref()?.chrome_hidden { 0.0 } else { PORTAL_CHROME * zoom };
        let rect = Rect::from_min_max(frame.min + Vec2::new(0.0, chrome), frame.max).shrink(1.0);
        let visible = rect.intersect(self.canvas_rect);
        (!self.portals_covered() && !self.lifted_other(element) && visible.width() > 4.0 && visible.height() > 4.0 && rect.height() > 20.0).then_some(rect)
    }

    /// The app's own floating UI over a page (the bars, panels, menus, tooltips): what the page's
    /// native view must leave uncovered, as the cards do. `cards` are layers that belong to the
    /// cards (a portal's bar, a terminal's grid): they keep under the page, as before. Last
    /// frame's areas count too. In a fixed order, so an unchanged set compares equal from frame to
    /// frame.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn page_holes(ctx: &Context, page: Rect, cards: &[egui::Id]) -> Vec<Rect> {
        // off Windows a hole hides the whole page: a tooltip of its own bar would blink it away
        let tooltips = cfg!(windows);
        let mut holes: Vec<Rect> = ctx.memory(|m| {
            m.areas()
                .visible_layer_ids()
                .into_iter()
                .filter(|l| matches!(l.order, egui::Order::Middle | egui::Order::Foreground) || (tooltips && l.order == egui::Order::Tooltip))
                .filter(|l| !cards.contains(&l.id))
                .filter_map(|l| m.area_rect(l.id))
                .map(|r| r.intersect(page))
                .filter(|r| r.width() >= 1.0 && r.height() >= 1.0)
                .collect()
        });
        holes.sort_by(|a, b| [a.min.x, a.min.y, a.max.x, a.max.y].partial_cmp(&[b.min.x, b.min.y, b.max.x, b.max.y]).unwrap_or(std::cmp::Ordering::Equal));
        holes.dedup();
        holes
    }

    /// Whether a portal's open tab is still loading (the bar shows × then).
    pub(super) fn tab_loading(&self, id: &str) -> bool {
        #[cfg(not(target_os = "linux"))]
        if let Some(tab) = self.open_tab_view(id) {
            return tab.loading;
        }
        let _ = id;
        false
    }
}
