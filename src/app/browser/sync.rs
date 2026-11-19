use super::*;

impl DrawingApp {
    /// What is kept for each portal, for the `live` ones only: a portal gone (deleted, unloaded) leaves nothing behind.
    pub(super) fn forget_portals(&mut self, live: &[&str]) {
        let alive = |id: &String| live.contains(&id.as_str());

        self.portals.retain(|id, _| alive(id));
        self.portal_stills.retain(|id, _| alive(id));
        self.portal_motion.retain(|id, _| alive(id));
        self.portal_snaps.retain(|id, _| alive(id));
        self.portal_painted.get_mut().retain(|id, _| alive(id));
        self.annotations.retain(|id, _| alive(id));
    }

    /// One webview per tab of every portal, placed over its page box every frame (the open tab
    /// shows, the others wait hidden, alive). Also takes in what the pages reported.
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

                    // the picture the card carries while the canvas moves: the loaded page's, at the next still frame
                    if let Some(snap) = self.portal_snaps.get_mut(&portal) {
                        snap.0 = f64::MIN;
                    }

                    self.portal_extras.mute.push(portal.clone());

                    // into the drawing too: else its old address would load back
                    if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == portal).and_then(|e| e.portal.as_mut())
                        && spec.tab_urls().get(i) != Some(&url)
                    {
                        spec.set_tab_url(i, url);
                        self.dirty = true;
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
                PortalEvent::Copied { picture: Ok((w, h, rgba)) } => {
                    ctx.copy_image(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba));
                    self.toast("Captura de tela copiada.");
                }
                PortalEvent::Copied { picture: Err(error) } => self.toast(error),
                PortalEvent::Still { portal, picture: (w, h, rgba) } => {
                    // a texture at once: the card paints it the very frame it moves (`paint_portal`)
                    if let Some(snap) = self.portal_snaps.get_mut(&portal) {
                        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);

                        match &mut snap.1 {
                            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                            None => snap.1 = Some(ctx.load_texture(format!("portal_still_{portal}"), image, egui::TextureOptions::LINEAR)),
                        }
                    }
                }
            }
        }

        // Linux on Wayland: the portals show only their address (webkit.rs)
        #[cfg(target_os = "linux")]
        if !pages_open() {
            self.portals.clear();

            return;
        }

        // Linux: the app has the keyboard again (from a page, from another program): the pages stop taking keys
        #[cfg(target_os = "linux")]
        {
            let focused = ctx.input(|i| i.focused);
            let regained = focused && !self.portal_extras.had_keyboard;

            self.portal_extras.had_keyboard = focused;

            if regained && !self.portals.is_empty() {
                release_keys(None);
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
        if ctx.input(|i| !i.focused && i.pointer.any_pressed())
            && let Some(tab) = self.portals.values().flat_map(|v| &v.tabs).next()
        {
            let _ = tab.webview.focus_parent();
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

                Some((e.id.clone(), urls, spec.active_tab(), rect, zoom, self.cards_in_front(i)))
            })
            .collect();

        self.forget_portals(&live.iter().map(|(id, ..)| id.as_str()).collect::<Vec<_>>());
        // SwiftUI drawing the canvas stacks its own over the pages, and egui's areas are not on screen: no holes
        let swift = pages_in_swift();
        let marquee = self.marquee.zip(self.last_pointer).filter(|_| !swift).map(|(start, end)| Rect::from_two_pos(self.to_screen(start), end));

        if self.design.as_ref().is_some_and(|d| !live.iter().any(|(live, ..)| *live == d.portal)) {
            self.design = None;
        }

        // the native view takes whole points (wry rounds them), kept inside the page box so nothing
        // spills over the card's edge (Linux: `GtkPage::place`, in pixels)
        #[cfg(not(target_os = "linux"))]
        let bounds = |r: Rect| {
            let (min, max) = (r.min.ceil(), r.max.floor());

            wry::Rect {
                position: wry::dpi::LogicalPosition::new(min.x as f64, min.y as f64).into(),
                size: wry::dpi::LogicalSize::new((max.x - min.x) as f64, (max.y - min.y) as f64).into(),
            }
        };

        // parts of the cards (the portals' bars, a tree's panels, a focused terminal's grid, the cards painted again over
        // them), not floating UI: they stack with their cards, and a card in front of a page leaves its hole anyway
        let card_parts: Vec<egui::Id> = (self.scene.elements.iter().filter(|element| element.is_card()).flat_map(|element| self.live_layers(element)))
            .chain(self.covers.iter().chain(&self.covers_before).copied())
            .map(|layer| layer.id)
            .chain([egui::Id::unique("terminal_overlay")])
            .collect();

        for (id, urls, active, rect, zoom, fronts) in live {
            let events = self.portal_events.0.clone();
            let extras = self.tab_extras(&id);

            let view = self.portals.entry(id.clone()).or_insert_with(|| PortalView {
                tabs: Vec::new(),
                shown: None,
                veiled: false,
                context: PageContext::new(extras.profile.clone()),
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
            let mut holes = rect.filter(|_| !swift).map_or(Vec::new(), |r| Self::page_holes(ctx, r, &card_parts, &fronts));

            // the selection box stays on top: its outline cuts through the page
            if let (Some(r), Some(m)) = (rect, marquee) {
                holes.extend(marquee_edges(m).into_iter().map(|e| e.intersect(r)).filter(|e| e.width() >= 1.0 && e.height() >= 1.0));
            }

            // ponytail: Windows (a window region) and Linux (an X11 shape) cut a native view; on macOS
            // the page steps aside while anything lies over it. Upgrade: a mask on the NSView's layer
            #[cfg(target_os = "macos")]
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

            // Linux: the same, with the picture WebKit took while the page stood still (`portal_snaps`), but the page is
            // never hidden nor moved while the canvas moves: it stays where it was, cut away whole (`veil`), and takes
            // its new place and zoom at rest, still cut away, showing once it has drawn there. Hidden and shown again,
            // or given a new size at each frame, WebKitGTK drew it black or at 200×200 for a moment
            #[cfg(target_os = "linux")]
            let veiled = {
                let moved = rect.map(|r| {
                    // a page seen the first time is placed unseen too: it shows once it has drawn at its size
                    let at = self.portal_motion.entry(id.clone()).or_insert((r, self.now - PAGE_SETTLE));

                    if at.0 != r {
                        *at = (r, self.now);
                    }

                    self.now - at.1
                });

                let stage = motion(moved);

                if stage == Motion::Moving {
                    // the picture goes with the card (`paint_portal` paints it from the frame the card moved); with no
                    // picture yet the card shows its page box: never the page flickering
                    let picture = self.portal_snaps.get(&id).and_then(|snap| snap.1.clone());

                    // this frame's card carries the picture: painted where the page box is now, or carried since the
                    // last frame. Not when the view changed after the canvas painted (the zoom buttons): the card is
                    // still where the page is, this frame, and the page is cut away at the next
                    let pictured =
                        picture.is_none() || self.portal_stills.contains_key(&id) || (rect.is_some() && self.portal_painted.borrow().get(&id) == rect.as_ref());

                    if let Some(texture) = picture {
                        self.portal_stills.entry(id.clone()).or_insert(texture);
                    }

                    if !view.veiled && pictured {
                        if let Some(tab) = view.tabs.get(active) {
                            veil(&tab.webview);
                        }

                        view.veiled = true;
                    }

                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(PAGE_SETTLE));

                    continue;
                }

                match stage {
                    Motion::Unseen | Motion::Back => ctx.request_repaint_after(std::time::Duration::from_secs_f64(PAGE_SETTLE / 2.0)),
                    _ => {
                        self.portal_stills.remove(&id);
                    }
                }

                // standing still and shown: its picture again, once a second at most (asked on a frame; none while idle)
                if stage == Motion::Still && rect.is_some() && view.shown.is_some() {
                    let snap = self.portal_snaps.entry(id.clone()).or_insert((f64::MIN, None));

                    if self.now - snap.0 >= SNAP_EVERY
                        && let Some(tab) = view.tabs.get(active)
                    {
                        snap.0 = self.now;
                        let (events, portal) = (self.portal_events.0.clone(), id.clone());

                        tab.webview.picture(move |picture| {
                            if let Some((w, h, rgba, _)) = picture {
                                let _ = events.send(PortalEvent::Still { portal, picture: (w, h, rgba) });
                            }
                        });
                    }
                }

                // laying out unseen, the floating UI makes no holes: they would only send GTK the same page again
                if stage == Motion::Unseen {
                    holes.clear();
                }

                stage == Motion::Unseen
            };

            #[cfg(not(target_os = "linux"))]
            let veiled = false;

            // with the screen's scale: on another monitor the same points are other pixels
            let shown = rect.map(|r| (r, zoom, active, holes, ctx.pixels_per_point()));

            if shown != view.shown || veiled != view.veiled {
                for (i, tab) in view.tabs.iter().enumerate() {
                    let visible = shown.is_some() && i == active;

                    if let (true, Some((r, zoom, _, holes, scale))) = (visible, &shown) {
                        #[cfg(not(target_os = "linux"))]
                        let _ = (tab.webview.set_bounds(bounds(*r)), tab.webview.zoom(*zoom as f64), scale);

                        #[cfg(target_os = "linux")]
                        let _ = tab.webview.place(*r, *zoom, *scale);

                        #[cfg(target_os = "linux")]
                        if veiled {
                            veil(&tab.webview);
                        } else {
                            cut_page(&tab.webview, r.min.ceil(), holes, *scale);
                        }

                        #[cfg(windows)]
                        cut_page(&tab.webview, r.min.ceil(), holes, *scale);

                        #[cfg(target_os = "macos")]
                        let _ = holes;
                    }

                    let _ = tab.webview.set_visible(visible);
                }

                view.shown = shown;
                view.veiled = veiled;
            }
        }
    }
}
