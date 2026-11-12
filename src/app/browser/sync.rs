use super::*;

impl DrawingApp {
    /// One webview per tab of every portal, placed over its page box every frame (the open tab
    /// shows, the others wait hidden, alive). Also takes in what the pages reported.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn sync_portals(&mut self, window: &impl wry::raw_window_handle::HasWindowHandle, ctx: &Context) {
        self.portal_extras_tick(ctx);
        let events: Vec<PortalEvent> = self.portal_events.1.try_iter().collect();
        for event in events {
            match event {
                PortalEvent::Loaded { portal, tab, url } => {
                    let Some(view) = self.portals.get_mut(&portal) else { continue };
                    let Some(i) = view.tabs.iter().position(|t| t.uid == tab) else { continue };
                    view.tabs[i].url = url.clone();
                    view.tabs[i].loading = false;
                    self.portal_extras.mute.push(portal.clone());
                    // into the drawing too: else its old address would load back
                    if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == portal).and_then(|e| e.portal.as_mut()) {
                        if spec.tab_urls().get(i) != Some(&url) {
                            spec.set_tab_url(i, url);
                            self.dirty = true;
                        }
                    }
                }
                PortalEvent::NewTab { portal, url } => {
                    // one page per portal: a link that asks for a window opens here, if it is the web
                    if !is_web(&url) {
                        continue;
                    }
                    if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == portal).and_then(|e| e.portal.as_mut()) {
                        let active = spec.active_tab();
                        spec.set_tab_url(active, url);
                        self.dirty = true;
                    }
                }
                PortalEvent::Navigating { portal, tab } => {
                    if let Some(t) = self.portals.get_mut(&portal).and_then(|v| v.tabs.iter_mut().find(|t| t.uid == tab)) {
                        t.secret = None;
                        t.loading = true;
                    }
                    // the overlay went with the old page
                    if self.design.as_ref().is_some_and(|d| d.portal == portal && d.tab == tab) {
                        self.design = None;
                    }
                }
                PortalEvent::Design { portal, tab, message } => self.design_message(&portal, tab, &message, ctx),
                PortalEvent::Shot { portal, seq, png } => self.design_shot(&portal, seq, png),
            }
        }
        // the overlay's kept messages, where they can't come by ipc (TabView::polls_design): every
        // 100 ms, however fast something else repaints
        let mut polled = false;
        if let Some(design) = &self.design {
            let tab = self.portals.get(&design.portal).and_then(|v| v.tabs.iter().find(|t| t.uid == design.tab));
            if let Some(tab) = tab.filter(|t| t.polls_design()) {
                if self.now - self.design_polled >= 0.1 {
                    let (events, portal, uid, ctx_note) = (self.portal_events.0.clone(), design.portal.clone(), tab.uid, ctx.clone());
                    let _ = tab.webview.evaluate_script_with_callback("window.__magoDesign ? window.__magoDesign.drain() : []", move |json| {
                        for message in serde_json::from_str::<Vec<String>>(&json).unwrap_or_default() {
                            let _ = events.send(PortalEvent::Design { portal: portal.clone(), tab: uid, message });
                            ctx_note.request_repaint();
                        }
                    });
                    polled = true;
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        if polled {
            self.design_polled = self.now;
        }
        // a page takes the keyboard when clicked and a click on the app doesn't take it back: the
        // window stays unfocused, so its fields show no caret and the keys go to the page
        if ctx.input(|i| !i.focused && i.pointer.any_pressed()) {
            if let Some(tab) = self.portals.values().flat_map(|v| &v.tabs).next() {
                let _ = tab.webview.focus_parent();
            }
        }
        // the page's zoom follows the scale its card is shown at (100% while enlarged)
        let live: Vec<_> = self
            .scene
            .elements
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let spec = e.portal.as_ref().filter(|p| !p.unloaded)?;
                let urls: Vec<String> = spec.tab_urls().iter().map(|u| normalize_url(u)).collect();
                // a hidden workspace's page stays loaded, out of sight
                let shown = i < self.scene.elements.len();
                let (rect, zoom) = if shown { (self.portal_page_rect(e), self.terminal_frame(e).1) } else { (None, 1.0) };
                Some((e.id.clone(), urls, spec.active_tab(), rect, zoom))
            })
            .collect();
        self.portals.retain(|id, _| live.iter().any(|(live, ..)| live == id));
        self.portal_stills.retain(|id, _| live.iter().any(|(live, ..)| live == id));
        self.portal_motion.retain(|id, _| live.iter().any(|(live, ..)| live == id));
        let marquee = self.marquee.zip(self.last_pointer).map(|(start, end)| Rect::from_two_pos(self.to_screen(start), end));
        self.annotations.retain(|id, _| live.iter().any(|(live, ..)| live == id));
        if self.design.as_ref().is_some_and(|d| !live.iter().any(|(live, ..)| *live == d.portal)) {
            self.design = None;
        }
        // the native view takes whole points (wry rounds them), kept inside the page box so nothing
        // spills over the card's edge
        let bounds = |r: Rect| {
            let (min, max) = (r.min.ceil(), r.max.floor());
            wry::Rect {
                position: wry::dpi::LogicalPosition::new(min.x as f64, min.y as f64).into(),
                size: wry::dpi::LogicalSize::new((max.x - min.x) as f64, (max.y - min.y) as f64).into(),
            }
        };
        // parts of the cards (the portals' bars, a focused terminal's grid), not floating UI
        let cards: Vec<egui::Id> =
            live.iter().map(|(id, ..)| egui::Id::unique(("portal_chrome", id.as_str()))).chain([egui::Id::unique("terminal_overlay")]).collect();
        for (id, urls, active, rect, zoom) in live {
            let events = self.portal_events.0.clone();
            let extras = self.tab_extras(&id);
            let view = self.portals.entry(id.clone()).or_insert_with(|| PortalView {
                tabs: Vec::new(),
                shown: None,
                context: wry::WebContext::new(extras.profile.clone()),
            });
            let matches = match_portal_tabs(&view.tabs.iter().map(|tab| tab.url.clone()).collect::<Vec<_>>(), &urls);
            let mut old_tabs: Vec<Option<TabView>> = view.tabs.drain(..).map(Some).collect();
            let mut tabs = Vec::with_capacity(urls.len());
            for (i, url) in urls.iter().enumerate() {
                match matches[i].and_then(|old| old_tabs.get_mut(old)?.take()) {
                    Some(mut tab) => {
                        if tab.url != *url {
                            let _ = tab.webview.load_url(url);
                            tab.url = url.clone();
                        }
                        tabs.push(tab);
                    }
                    None => match new_tab_view(window, &id, url, &events, ctx, &mut view.context, &extras) {
                        Ok(tab) => {
                            tabs.push(tab);
                            view.shown = None;
                        }
                        Err(error) => {
                            self.status = format!("Não foi possível abrir a página: {error}");
                            self.status_time = -1.0;
                            break;
                        }
                    },
                }
            }
            // a page left or changed place: the one now at the open tab's place may be a hidden one,
            // so where each is shown is set again
            if matches.iter().enumerate().any(|(i, old)| old.is_some_and(|old| old != i)) || tabs.len() < old_tabs.len() {
                view.shown = None;
            }
            view.tabs = tabs;
            let mut holes = rect.map_or(Vec::new(), |r| Self::page_holes(ctx, r, &cards));
            // the selection box stays on top: its outline cuts through the page
            if let (Some(r), Some(m)) = (rect, marquee) {
                holes.extend(marquee_edges(m).into_iter().map(|e| e.intersect(r)).filter(|e| e.width() >= 1.0 && e.height() >= 1.0));
            }
            // ponytail: only Windows cuts a native view (a window region); elsewhere the page steps
            // aside while anything lies over it. Upgrade: a mask on the NSView's layer on macOS
            #[cfg(not(windows))]
            let rect = rect.filter(|_| holes.is_empty());
            // the canvas moving (a pan, a zoom, the card dragged): the native view follows a frame
            // late and shakes, so its picture goes with the card instead and the page comes back
            // where the card stops
            #[cfg(windows)]
            {
                let moved = rect.map(|r| {
                    let at = self.portal_motion.entry(id.clone()).or_insert((r, f64::MIN));
                    if at.0 != r {
                        *at = (r, self.now);
                    }
                    self.now - at.1
                });
                // under the smallest zoom WebView2 takes, the page would lay out wider than its card
                // shows it (squeezed): the picture stands in for it there
                let tiny = zoom < MIN_PAGE_ZOOM;
                let moving = moved.is_some_and(|since| since < PAGE_SETTLE) || tiny;
                if moving && !self.portal_stills.contains_key(&id) && view.shown.is_some() {
                    let picture = view.tabs.get(active).and_then(|tab| crate::app::browser::shot::webview_rgba(&tab.webview));
                    if let Some((w, h, rgba)) = picture {
                        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                        self.portal_stills.insert(id.clone(), ctx.load_texture(format!("portal_still_{id}"), image, egui::TextureOptions::LINEAR));
                    }
                }
                if moving && self.portal_stills.contains_key(&id) {
                    for tab in &view.tabs {
                        let _ = tab.webview.set_visible(false);
                    }
                    view.shown = None;
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(PAGE_SETTLE));
                    continue;
                }
                // the picture stays over the page a moment, until the page has drawn at its new size:
                // the page shows first wholly cut away (laying out unseen), then whole, so it never
                // flashes its old picture stretched or blank
                if moved.is_none_or(|since| since > 3.0 * PAGE_SETTLE) {
                    self.portal_stills.remove(&id);
                } else if self.portal_stills.contains_key(&id) {
                    if let Some(r) = rect.filter(|_| moved.is_some_and(|since| since < 2.0 * PAGE_SETTLE)) {
                        holes = vec![r];
                    }
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(PAGE_SETTLE / 2.0));
                }
            }
            // with the screen's scale: on another monitor the same points are other pixels
            let shown = rect.map(|r| (r, zoom, active, holes, ctx.pixels_per_point()));
            if shown != view.shown {
                for (i, tab) in view.tabs.iter().enumerate() {
                    let visible = shown.is_some() && i == active;
                    if let (true, Some((r, zoom, _, holes, _))) = (visible, &shown) {
                        let _ = tab.webview.set_bounds(bounds(*r));
                        let _ = tab.webview.zoom(*zoom as f64);
                        #[cfg(windows)]
                        cut_page(&tab.webview, r.min.ceil(), holes, ctx.pixels_per_point());
                        #[cfg(not(windows))]
                        let _ = holes;
                    }
                    let _ = tab.webview.set_visible(visible);
                }
                view.shown = shown;
            }
        }
    }
}
