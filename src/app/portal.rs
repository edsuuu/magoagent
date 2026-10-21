//! Portals: small browsers on the canvas. Each tab is the system's own webview (wry: WebKit on
//! macOS, WebView2 on Windows) laid over the canvas and moved with it every frame; the app
//! draws the title strip, the tabs and the address bar. Agents drive them through the bridge.

use crate::prelude::*;

/// The title strip (the handle that drags the portal), the tabs and the address bar.
pub(crate) const PORTAL_TITLE: f32 = 26.0;
pub(crate) const PORTAL_TABS: f32 = 30.0;
pub(crate) const PORTAL_NAV: f32 = 34.0;
#[cfg(not(target_os = "linux"))]
pub(crate) const PORTAL_CHROME: f32 = PORTAL_TITLE + PORTAL_TABS + PORTAL_NAV;

/// The new / edit portal dialog.
#[derive(Clone, Debug)]
pub(crate) struct PortalDialog {
    /// The portal being edited; None creates one in `at`.
    pub(crate) target: Option<String>,
    pub(crate) at: Rect,
    pub(crate) spec: PortalSpec,
}

/// What pages report back (on the UI thread, from wry's handlers).
#[cfg(not(target_os = "linux"))]
pub(crate) enum PortalEvent {
    /// A tab finished loading an address (a link clicked inside the page, too).
    Loaded { portal: String, tab: u64, url: String },
    /// A link asked for a new window: it opens as a new tab.
    NewTab { portal: String, url: String },
    /// A tab started loading another page: what lived in the old one is gone.
    Navigating { portal: String, tab: u64 },
    /// The pencil's note from a tab's page (pencil.rs), and then the same note with the picture
    /// of the circled region, once the page's snapshot is in.
    Note { portal: String, tab: u64, message: String },
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // no picture on Windows yet
    Shot { portal: String, prompt: String, png: Option<Vec<u8>> },
}

/// An address the webview can load: "exemplo.com" → "https://exemplo.com".
pub(crate) fn normalize_url(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        "about:blank".into()
    } else if url.contains("://") || url.starts_with("about:") {
        url.into()
    } else if url.starts_with("localhost") || url.starts_with("127.") {
        format!("http://{url}")
    } else {
        format!("https://{url}")
    }
}

/// "https://docs.rs/egui/latest" → "docs.rs".
pub(crate) fn host_of(url: &str) -> &str {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or_default()
}

/// Opens an address in the system's browser.
pub(crate) fn open_external(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/c", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    let _ = command.arg(url).spawn();
}

/// One tab's live page.
#[cfg(not(target_os = "linux"))]
pub(crate) struct TabView {
    pub(crate) webview: wry::WebView,
    pub(crate) uid: u64,
    /// What it was last told to load (or reported loading), so the URL isn't re-sent.
    url: String,
    title: std::sync::Arc<std::sync::Mutex<String>>,
    /// The secret the pencil in this tab's page was armed with (pencil.rs).
    pencil: Option<String>,
}

#[cfg(not(target_os = "linux"))]
pub(crate) struct PortalView {
    pub(crate) tabs: Vec<TabView>,
    /// Box, zoom and open tab last applied, so unchanged values aren't re-sent every frame.
    shown: Option<(Rect, f32, usize)>,
}

#[cfg(not(target_os = "linux"))]
fn new_tab_view(
    window: &impl wry::raw_window_handle::HasWindowHandle,
    portal: &str,
    url: &str,
    events: &std::sync::mpsc::Sender<PortalEvent>,
    ctx: &Context,
) -> wry::Result<TabView> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let uid = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let title = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let (title_in, loaded, new_window, note) = (title.clone(), events.clone(), events.clone(), events.clone());
    let (portal_loaded, portal_window, portal_note) = (portal.to_string(), portal.to_string(), portal.to_string());
    let ctx_note = ctx.clone();
    let (ctx_title, ctx_loaded) = (ctx.clone(), ctx.clone());
    let webview = wry::WebViewBuilder::new()
        .with_url(url)
        .with_visible(false)
        .with_document_title_changed_handler(move |new| {
            *title_in.lock().unwrap() = new;
            ctx_title.request_repaint();
        })
        .with_on_page_load_handler(move |event, url| {
            let portal = portal_loaded.clone();
            let _ = loaded.send(match event {
                wry::PageLoadEvent::Started => PortalEvent::Navigating { portal, tab: uid },
                wry::PageLoadEvent::Finished => PortalEvent::Loaded { portal, tab: uid, url },
            });
            ctx_loaded.request_repaint();
        })
        .with_new_window_req_handler(move |url, _| {
            let _ = new_window.send(PortalEvent::NewTab { portal: portal_window.clone(), url });
            wry::NewWindowResponse::Deny
        })
        .with_ipc_handler(move |request| {
            let _ = note.send(PortalEvent::Note { portal: portal_note.clone(), tab: uid, message: request.into_body() });
            ctx_note.request_repaint();
        })
        .build_as_child(window)?;
    Ok(TabView { webview, uid, url: url.into(), title, pencil: None })
}

/// A clicked part of a portal's tabs or address bar.
enum Chrome {
    Select(usize),
    Close(usize),
    NewTab,
    Back,
    Forward,
    Reload,
    Navigate(String),
}

impl DrawingApp {
    /// Click: a 960×680 portal centered there; drag: that box. Asks for the address first.
    pub(crate) fn open_portal_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, Vec2::new(960.0, 680.0))
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(Vec2::new(280.0, 200.0)))
        };
        self.portal_dialog = Some(PortalDialog { target: None, at, spec: PortalSpec::default() });
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    pub(crate) fn confirm_portal_dialog(&mut self) {
        let Some(PortalDialog { target, at, mut spec }) = self.portal_dialog.take() else { return };
        let url = normalize_url(&spec.url);
        let active = spec.active_tab();
        spec.set_tab_url(active, url.clone());
        if spec.name.trim().is_empty() {
            spec.name = if host_of(&url).is_empty() { "Portal".into() } else { host_of(&url).into() };
        }
        self.snapshot();
        if let Some(id) = target {
            if let Some(e) = self.scene.elements.iter_mut().find(|e| e.id == id) {
                e.portal = Some(spec);
            }
            return;
        }
        let mut portal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        portal.style = node_style(portal.style);
        portal.portal = Some(spec);
        self.scene.elements.push(portal);
        self.selected = vec![self.scene.elements.len() - 1];
    }

    pub(crate) fn portal_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.portal_dialog.clone() else { return };
        let (mut open, mut confirm, mut cancel) = (true, false, false);
        let editing = dialog.target.is_some();
        egui::Window::new(if editing { "Editar portal" } else { "Novo portal" })
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(400.0);
                egui::Grid::new("portal_fields").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Endereço");
                    let address = ui.add(egui::TextEdit::singleline(&mut dialog.spec.url).hint_text("https://exemplo.com").desired_width(300.0));
                    if !editing && dialog.spec.url.is_empty() && !address.has_focus() {
                        address.request_focus();
                    }
                    ui.end_row();
                    ui.label("Nome");
                    ui.add(egui::TextEdit::singleline(&mut dialog.spec.name).hint_text("opcional").desired_width(300.0));
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    cancel = ui.button("Cancelar").clicked();
                    let label = egui::RichText::new(if editing { "Salvar" } else { "Criar" }).color(Color32::WHITE);
                    confirm = ui.add(egui::Button::new(label).fill(PRIMARY)).clicked();
                });
                confirm |= ui.input(|i| i.key_pressed(Key::Enter));
            });
        if !open || cancel {
            self.portal_dialog = None;
            return;
        }
        self.portal_dialog = Some(dialog);
        if confirm {
            self.confirm_portal_dialog();
        }
    }

    /// The frame, the title strip and the chrome's background; the tabs and address bar on
    /// top are live widgets (`portal_chrome`), the page a native view below them.
    pub(crate) fn paint_portal(&self, painter: &Painter, element: &Element) {
        let b = local_bounds(element);
        let rect = Rect::from_min_max(self.to_screen(b.min), self.to_screen(b.max));
        let zoom = self.zoom;
        let radius = (8.0 * zoom).min(255.0) as u8;
        painter.rect(rect, radius, Color32::from_gray(0x24), Stroke::new(1.0, Color32::from_gray(0x3a)), StrokeKind::Inside);
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        let spec = element.portal.clone().unwrap_or_default();
        let (muted, light) = (Color32::from_gray(0x8a), Color32::from_gray(0xd0));
        let title = Rect::from_min_size(rect.min, Vec2::new(rect.width(), PORTAL_TITLE * zoom));
        painter.rect_filled(title, egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, Color32::from_gray(0x22));
        let globe = title.left_center() + Vec2::new(14.0 * zoom, 0.0);
        if zoom >= 0.6 {
            paint_icon(&painter, Icon::Tool(Tool::Portal), globe, muted);
        }
        painter.text(globe + Vec2::new(14.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, &spec.name, FontId::proportional(12.0 * zoom), light);
        let chrome = Rect::from_min_size(title.left_bottom(), Vec2::new(rect.width(), (PORTAL_TABS + PORTAL_NAV) * zoom));
        painter.rect_filled(chrome, 0.0, Color32::from_gray(0x1d));
        painter.hline(chrome.x_range(), chrome.bottom(), Stroke::new(1.0, Color32::from_gray(0x33)));
        if !self.chrome_live(element) {
            // too small for the live tabs: just the address
            painter.text(chrome.left_center() + Vec2::new(10.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, &spec.url, FontId::proportional(11.0 * zoom), muted);
        }
        let page = Rect::from_min_max(chrome.left_bottom(), rect.max);
        let note = if cfg!(target_os = "linux") { "Portais ainda não abrem no Linux" } else { "Carregando…" };
        painter.text(page.center(), egui::Align2::CENTER_CENTER, note, FontId::proportional(13.0 * zoom), muted);
    }

    /// Native views sit above everything the app draws, so pages step aside while a menu or
    /// dialog is open.
    fn portals_covered(&self) -> bool {
        self.menu_open
            || self.extra_open
            || self.palette_open
            || self.export_open
            || self.terminal_dialog.is_some()
            || self.portal_dialog.is_some()
            || self.ide_dialog.is_some()
            || self.terminal_elevated.is_some()
            || self.chat_open
    }

    /// Big enough on screen for clickable tabs and address bar.
    fn chrome_live(&self, element: &Element) -> bool {
        self.zoom >= 0.45 && local_bounds(element).width() * self.zoom >= 200.0
    }

    /// Where a portal's page goes on screen, if it should show.
    #[cfg(not(target_os = "linux"))]
    fn portal_page_rect(&self, element: &Element) -> Option<Rect> {
        let b = local_bounds(element);
        let rect = Rect::from_min_max(self.to_screen(b.min + Vec2::new(0.0, PORTAL_CHROME)), self.to_screen(b.max)).shrink(1.0);
        let visible = rect.intersect(self.canvas_rect);
        let under_panel = self.agents_rect.is_some_and(|panel| panel.intersects(rect));
        (!self.portals_covered() && !under_panel && visible.width() > 4.0 && visible.height() > 4.0 && rect.height() > 20.0).then_some(rect)
    }

    /// The tabs and the address bar, as live widgets over each portal's chrome strip.
    pub(crate) fn portal_chrome(&mut self, ctx: &Context) {
        if self.portals_covered() {
            return;
        }
        let portals: Vec<Element> = self.scene.elements.iter().filter(|e| e.portal.is_some() && self.chrome_live(e)).cloned().collect();
        for element in portals {
            let zoom = self.zoom;
            let b = local_bounds(&element);
            let area = Rect::from_min_size(self.to_screen(b.min + Vec2::new(0.0, PORTAL_TITLE)), Vec2::new(b.width(), PORTAL_TABS + PORTAL_NAV) * zoom);
            if !area.intersects(self.canvas_rect) {
                continue;
            }
            let spec = element.portal.clone().unwrap_or_default();
            let urls = spec.tab_urls();
            let active = spec.active_tab();
            let titles: Vec<String> = urls.iter().enumerate().map(|(i, url)| self.tab_title(&element.id, i).unwrap_or_else(|| host_of(url).to_string())).collect();
            let editing = self.portal_url_edit.as_ref().filter(|(id, ..)| *id == element.id).cloned();
            let mut address = editing.as_ref().map_or(spec.url.clone(), |(_, text, _)| text.clone());
            let mut action = None;
            let mut focus_change = None;
            let (light, muted) = (Color32::from_gray(0xd0), Color32::from_gray(0x8a));
            egui::Area::new(egui::Id::unique(("portal_chrome", element.id.as_str()))).fixed_pos(area.min).order(egui::Order::Middle).show(ctx, |ui| {
                ui.set_clip_rect(area.intersect(self.canvas_rect));
                let (rect, _) = ui.allocate_exact_size(area.size(), Sense::hover());
                let p = ui.painter().clone();
                let font = FontId::proportional(12.0 * zoom);
                // tabs, browser style, with + at the end
                let row = Rect::from_min_size(rect.min, Vec2::new(rect.width(), PORTAL_TABS * zoom));
                let plus = 26.0 * zoom;
                let width = ((row.width() - plus - 12.0 * zoom) / urls.len() as f32).min(220.0 * zoom);
                for (i, title) in titles.iter().enumerate() {
                    let tab = Rect::from_min_size(row.min + Vec2::new(6.0 * zoom + i as f32 * width, 4.0 * zoom), Vec2::new(width - 2.0 * zoom, row.height() - 4.0 * zoom));
                    let response = ui.interact(tab, ui.scope_id().with(("tab", i)), Sense::click());
                    let fill = if i == active { Color32::from_gray(0x2e) } else if response.hovered() { Color32::from_gray(0x27) } else { Color32::TRANSPARENT };
                    let radius = (6.0 * zoom) as u8;
                    p.rect_filled(tab, egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, fill);
                    let close = Rect::from_center_size(tab.right_center() - Vec2::new(11.0 * zoom, 0.0), Vec2::splat(16.0 * zoom));
                    let text_room = Rect::from_min_max(tab.min + Vec2::new(10.0 * zoom, 0.0), Pos2::new(close.left() - 4.0 * zoom, tab.bottom()));
                    p.with_clip_rect(text_room.intersect(p.clip_rect())).text(text_room.left_center(), egui::Align2::LEFT_CENTER, title, font.clone(), if i == active { light } else { muted });
                    if urls.len() > 1 {
                        let x = ui.interact(close, ui.scope_id().with(("close", i)), Sense::click());
                        if x.hovered() {
                            p.rect_filled(close, (4.0 * zoom) as u8, Color32::from_gray(0x3a));
                        }
                        p.text(close.center(), egui::Align2::CENTER_CENTER, "×", FontId::proportional(13.0 * zoom), muted);
                        if x.clicked() {
                            action = Some(Chrome::Close(i));
                            continue;
                        }
                    }
                    if response.clicked() {
                        action = Some(Chrome::Select(i));
                    }
                }
                let add = Rect::from_min_size(row.min + Vec2::new(6.0 * zoom + urls.len() as f32 * width + 2.0 * zoom, 6.0 * zoom), Vec2::splat(plus - 6.0 * zoom));
                let add_response = ui.interact(add, ui.scope_id().with("new_tab"), Sense::click());
                if add_response.hovered() {
                    p.rect_filled(add, (4.0 * zoom) as u8, Color32::from_gray(0x2e));
                }
                p.text(add.center(), egui::Align2::CENTER_CENTER, "+", FontId::proportional(15.0 * zoom), light);
                if add_response.on_hover_text("Nova aba").clicked() {
                    action = Some(Chrome::NewTab);
                }
                // back, forward, reload and the address
                let nav = Rect::from_min_size(row.left_bottom(), Vec2::new(rect.width(), PORTAL_NAV * zoom));
                p.rect_filled(nav, 0.0, Color32::from_gray(0x2e));
                let mut x = nav.left() + 8.0 * zoom;
                for (icon, chrome, tip) in [(Icon::Undo, Chrome::Back, "Voltar"), (Icon::Redo, Chrome::Forward, "Avançar"), (Icon::Restart, Chrome::Reload, "Recarregar")] {
                    let button = Rect::from_min_size(Pos2::new(x, nav.center().y - 12.0 * zoom), Vec2::splat(24.0 * zoom));
                    let response = ui.interact(button, ui.scope_id().with(tip), Sense::click());
                    if response.hovered() {
                        p.rect_filled(button, (5.0 * zoom) as u8, Color32::from_gray(0x3a));
                    }
                    paint_icon(&p, icon, button.center(), light);
                    if response.on_hover_text(tip).clicked() {
                        action = Some(chrome);
                    }
                    x += 26.0 * zoom;
                }
                let field = Rect::from_min_max(Pos2::new(x + 6.0 * zoom, nav.center().y - 12.0 * zoom), Pos2::new(nav.right() - 8.0 * zoom, nav.center().y + 12.0 * zoom));
                p.rect_filled(field, (12.0 * zoom) as u8, Color32::from_gray(0x1d));
                let edit = egui::TextEdit::singleline(&mut address)
                    .frame(egui::Frame::NONE)
                    .font(font.clone())
                    .text_color(light)
                    .hint_text("Digite um endereço")
                    .desired_width(field.width() - 24.0 * zoom);
                let response = ui.put(field.shrink2(Vec2::new(12.0 * zoom, 2.0 * zoom)), edit);
                if editing.as_ref().is_some_and(|(.., focus)| *focus) {
                    response.request_focus();
                }
                if response.gained_focus() || (response.has_focus() && editing.is_none()) {
                    focus_change = Some(Some(address.clone()));
                } else if response.lost_focus() {
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        action = Some(Chrome::Navigate(address.clone()));
                    }
                    focus_change = Some(None);
                } else if response.has_focus() {
                    focus_change = Some(Some(address.clone()));
                }
            });
            match focus_change {
                Some(Some(text)) => self.portal_url_edit = Some((element.id.clone(), text, false)),
                Some(None) => self.portal_url_edit = None,
                None => {}
            }
            if let Some(action) = action {
                self.apply_chrome(&element.id, action);
            }
        }
    }

    fn apply_chrome(&mut self, id: &str, action: Chrome) {
        let script = match action {
            Chrome::Back => "history.back()",
            Chrome::Forward => "history.forward()",
            Chrome::Reload => "location.reload()",
            _ => "",
        };
        if !script.is_empty() {
            self.portal_script(id, script);
            return;
        }
        self.snapshot();
        let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == id).and_then(|e| e.portal.as_mut()) else { return };
        match action {
            Chrome::Select(tab) => spec.select_tab(tab),
            Chrome::Close(tab) => {
                spec.close_tab(tab);
                #[cfg(not(target_os = "linux"))]
                if let Some(view) = self.portals.get_mut(id) {
                    if tab < view.tabs.len() && view.tabs.len() > 1 {
                        view.tabs.remove(tab);
                        view.shown = None;
                    }
                }
            }
            Chrome::NewTab => {
                spec.open_tab("about:blank".into());
                // straight to typing its address
                self.portal_url_edit = Some((id.to_string(), String::new(), true));
            }
            Chrome::Navigate(url) => {
                let active = spec.active_tab();
                spec.set_tab_url(active, normalize_url(&url));
            }
            _ => {}
        }
    }

    /// The page's own title of a tab, once it has one.
    fn tab_title(&self, id: &str, tab: usize) -> Option<String> {
        #[cfg(not(target_os = "linux"))]
        if let Some(view) = self.portals.get(id).and_then(|v| v.tabs.get(tab)) {
            return Some(view.title.lock().unwrap().clone()).filter(|t| !t.trim().is_empty());
        }
        let _ = (id, tab);
        None
    }

    /// One webview per tab of every portal, placed over its box every frame (the open tab
    /// shows, the others wait hidden, alive). Also takes in what the pages reported.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn sync_portals(&mut self, window: &impl wry::raw_window_handle::HasWindowHandle, ctx: &Context) {
        let events: Vec<PortalEvent> = self.portal_events.1.try_iter().collect();
        for event in events {
            match event {
                PortalEvent::Loaded { portal, tab, url } => {
                    let Some(view) = self.portals.get_mut(&portal) else { continue };
                    let Some(i) = view.tabs.iter().position(|t| t.uid == tab) else { continue };
                    view.tabs[i].url = url.clone();
                    if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == portal).and_then(|e| e.portal.as_mut()) {
                        if spec.tab_urls().get(i) != Some(&url) {
                            spec.set_tab_url(i, url);
                            self.dirty = true;
                        }
                    }
                }
                PortalEvent::NewTab { portal, url } => {
                    if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == portal).and_then(|e| e.portal.as_mut()) {
                        spec.open_tab(url);
                        self.dirty = true;
                    }
                }
                PortalEvent::Navigating { portal, tab } => {
                    if let Some(t) = self.portals.get_mut(&portal).and_then(|v| v.tabs.iter_mut().find(|t| t.uid == tab)) {
                        t.pencil = None;
                    }
                }
                PortalEvent::Note { portal, tab, message } => self.pencil_note(&portal, tab, &message, ctx),
                PortalEvent::Shot { portal, prompt, png } => self.deliver_note(&portal, &prompt, png),
            }
        }
        let live: Vec<(String, Vec<String>, usize, Option<Rect>)> = self
            .scene
            .elements
            .iter()
            .filter_map(|e| {
                let spec = e.portal.as_ref()?;
                let urls = spec.tab_urls().iter().map(|u| normalize_url(u)).collect();
                Some((e.id.clone(), urls, spec.active_tab(), self.portal_page_rect(e)))
            })
            .collect();
        self.portals.retain(|id, _| live.iter().any(|(live, ..)| live == id));
        // ponytail: measured, not derived: in this full-size-content window WebKit draws a child
        // page 52pt above its (correct) frame on macOS, whatever the title bar; re-measure with a
        // red outline at the page rect if the window setup changes
        let dy = if cfg!(target_os = "macos") { 52.0 } else { 0.0 };
        let bounds = |r: Rect| wry::Rect {
            position: wry::dpi::LogicalPosition::new(r.min.x as f64, r.min.y as f64 + dy).into(),
            size: wry::dpi::LogicalSize::new(r.width() as f64, r.height() as f64).into(),
        };
        for (id, urls, active, rect) in live {
            let events = self.portal_events.0.clone();
            let view = self.portals.entry(id.clone()).or_insert_with(|| PortalView { tabs: Vec::new(), shown: None });
            view.tabs.truncate(urls.len());
            for (i, url) in urls.iter().enumerate() {
                match view.tabs.get_mut(i) {
                    Some(tab) if tab.url != *url => {
                        let _ = tab.webview.load_url(url);
                        tab.url = url.clone();
                    }
                    Some(_) => {}
                    None => match new_tab_view(window, &id, url, &events, ctx) {
                        Ok(tab) => {
                            view.tabs.push(tab);
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
            let shown = rect.map(|r| (r, self.zoom, active));
            if shown != view.shown {
                for (i, tab) in view.tabs.iter().enumerate() {
                    let visible = shown.is_some() && i == active;
                    if let (true, Some((r, zoom, _))) = (visible, shown) {
let _ = tab.webview.set_bounds(bounds(r));
                        let _ = tab.webview.zoom(zoom as f64);
                    }
                    let _ = tab.webview.set_visible(visible);
                }
                view.shown = shown;
            }
        }
    }

    /// Runs `script` in a portal's open tab; the reply comes back JSON-encoded.
    pub(crate) fn portal_eval(&self, id: &str, script: &str, reply: std::sync::mpsc::Sender<Result<String, String>>) {
        #[cfg(not(target_os = "linux"))]
        if let Some(tab) = self.open_tab_view(id) {
            let answer = reply.clone();
            let result = tab.webview.evaluate_script_with_callback(script, move |json| {
                let _ = answer.send(Ok(json));
            });
            if let Err(error) = result {
                let _ = reply.send(Err(error.to_string()));
            }
            return;
        }
        let _ = (id, script);
        let _ = reply.send(Err(if cfg!(target_os = "linux") { "portais ainda não abrem no Linux" } else { "esse portal não está aberto" }.into()));
    }

    #[cfg(not(target_os = "linux"))]
    fn open_tab_view(&self, id: &str) -> Option<&TabView> {
        let active = self.scene.elements.iter().find(|e| e.id == id)?.portal.as_ref()?.active_tab();
        self.portals.get(id)?.tabs.get(active)
    }

    pub(crate) fn portal_script(&self, id: &str, script: &str) {
        #[cfg(not(target_os = "linux"))]
        if let Some(tab) = self.open_tab_view(id) {
            let _ = tab.webview.evaluate_script(script);
        }
        let _ = (id, script);
    }

    /// Turns the pencil on in a portal's open tab: `script` runs with a fresh secret in place of
    /// `__NONCE__`, and only notes from that tab carrying it get through (`pencil_armed`), until
    /// it navigates. Any page can post to the app; this is what tells the pencil's notes apart.
    pub(crate) fn arm_pencil(&mut self, id: &str, script: &str) {
        #[cfg(not(target_os = "linux"))]
        {
            let active = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| Some(e.portal.as_ref()?.active_tab()));
            if let Some(tab) = active.and_then(|a| self.portals.get_mut(id)?.tabs.get_mut(a)) {
                // ponytail: the secret lives in the page's world, so a page that hooked
                // JSON.stringify before the user sent a note there can forge notes until it
                // navigates; closing that needs a pristine post captured by an initialization
                // script at document start (per platform), the way wry installs window.ipc
                let nonce = uuid::Uuid::new_v4().simple().to_string();
                let _ = tab.webview.evaluate_script(&script.replace("__NONCE__", &nonce));
                tab.pencil = Some(nonce);
            }
        }
        let _ = (id, script);
    }

    /// Whether `nonce` is the secret a tab's pencil was armed with.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn pencil_armed(&self, id: &str, tab: u64, nonce: Option<&str>) -> bool {
        let armed = self.portals.get(id).and_then(|v| v.tabs.iter().find(|t| t.uid == tab)).and_then(|t| t.pencil.as_deref());
        armed.is_some() && armed == nonce
    }

    /// An agent opening an address: in the open tab.
    pub(crate) fn portal_navigate(&mut self, id: &str, url: &str) {
        self.snapshot();
        if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == id).and_then(|e| e.portal.as_mut()) {
            let active = spec.active_tab();
            spec.set_tab_url(active, normalize_url(url));
        }
    }
}
