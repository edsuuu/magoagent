//! Portals: small browsers on the canvas. Each one is the system's own webview (wry: WebKit on
//! macOS, WebView2 on Windows) laid over the canvas and moved with it every frame; the app draws
//! the header and the address bar above the page. Agents drive them through the bridge.

use crate::prelude::*;

/// The header (globe and name) and the address bar above the page, in points.
pub(crate) const PORTAL_HEADER: f32 = 32.0;
pub(crate) const PORTAL_BAR: f32 = 40.0;
pub(crate) const PORTAL_CHROME: f32 = PORTAL_HEADER + PORTAL_BAR;

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
    /// A link asked for a new window: the page goes there, since a portal shows one page.
    NewTab { portal: String, url: String },
    /// A tab started loading another page: what lived in the old one is gone.
    Navigating { portal: String, tab: u64 },
    /// A message from the Design overlay in a tab's page (design.rs).
    Design { portal: String, tab: u64, message: String },
    /// The picture of a region the stroke made: `seq` tells it from the later strokes.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // no picture on Windows yet
    Shot { portal: String, seq: u64, png: Option<Vec<u8>> },
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

/// The address as the bar shows it: no scheme.
fn display_url(url: &str) -> &str {
    url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url)
}

/// Opens an address in the system's browser, or a folder in the file manager.
pub(crate) fn open_external(url: &str) {
    #[cfg(windows)]
    {
        // straight to the shell: `cmd /c start` parsed the address a second time
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(hwnd: isize, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
        }
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (op, file) = (wide("open"), wide(url));
        const SW_SHOWNORMAL: i32 = 1;
        // SAFETY: both strings are NUL-terminated and outlive the call
        unsafe { ShellExecuteW(0, op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
    }
    #[cfg(not(windows))]
    {
        let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(program).arg(url).spawn();
    }
}

/// One tab's live page.
#[cfg(not(target_os = "linux"))]
pub(crate) struct TabView {
    pub(crate) webview: wry::WebView,
    pub(crate) uid: u64,
    /// What it was last told to load (or reported loading), so the URL isn't re-sent.
    url: String,
    /// From the page's start to its finish: the bar shows × to stop it meanwhile.
    loading: bool,
    /// The secret the Design overlay in this tab's page was armed with (design.rs).
    pub(crate) secret: Option<String>,
}

#[cfg(not(target_os = "linux"))]
impl TabView {
    /// wry drops a page's `ipc.postMessage` when the page's URL isn't an `http::Uri`, as file:///C:/…
    /// on Windows: there Design's overlay keeps its messages and the app fetches them each frame.
    pub(crate) fn polls_design(&self) -> bool {
        cfg!(windows) && self.url.starts_with("file:")
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) struct PortalView {
    pub(crate) tabs: Vec<TabView>,
    /// Box, zoom, open tab and the holes cut in it last applied, so unchanged values aren't
    /// re-sent every frame.
    shown: Option<(Rect, f32, usize, Vec<Rect>)>,
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
    let (loaded, new_window, note) = (events.clone(), events.clone(), events.clone());
    let (portal_loaded, portal_window, portal_note) = (portal.to_string(), portal.to_string(), portal.to_string());
    let (ctx_note, ctx_loaded) = (ctx.clone(), ctx.clone());
    let webview = wry::WebViewBuilder::new()
        .with_url(url)
        .with_visible(false)
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
            let _ = note.send(PortalEvent::Design { portal: portal_note.clone(), tab: uid, message: request.into_body() });
            ctx_note.request_repaint();
        })
        .build_as_child(window)?;
    // wry anchors a child page to the window's bottom edge (ViewMinYMargin), so a window that
    // settles at another height would move the page off its card; its place is set from the
    // canvas instead, so AppKit must leave it alone
    #[cfg(target_os = "macos")]
    wry::WebViewExtMacOS::webview(&webview).setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::empty());
    Ok(TabView { webview, uid, url: url.into(), loading: true, secret: None })
}

/// Leaves `holes` (window points) out of a page's native view, so the app's floating UI over it
/// shows through while the page keeps its size; `origin` is where the view's box starts.
// ponytail: square holes, so a rounded menu's corners show the canvas instead of the page; a
// CreateRoundRectRgn per hole with the egui corner radius if that ever shows
#[cfg(windows)]
fn cut_page(webview: &wry::WebView, origin: Pos2, holes: &[Rect], pixels_per_point: f32) {
    use wry::WebViewExtWindows;
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> isize;
        fn CombineRgn(target: isize, a: isize, b: isize, mode: i32) -> i32;
        fn DeleteObject(object: isize) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWindowRgn(hwnd: isize, region: isize, redraw: i32) -> i32;
    }
    const RGN_DIFF: i32 = 4;
    // wry's own container window, which holds the page's
    let hwnd = webview.hwnd().0 as isize;
    let px = |v: f32| (v * pixels_per_point).round() as i32;
    // SAFETY: plain GDI handles; the cut-outs are freed here, the shape goes to the window
    unsafe {
        if holes.is_empty() {
            SetWindowRgn(hwnd, 0, 1);
            return;
        }
        let shape = CreateRectRgn(0, 0, 1 << 20, 1 << 20);
        for hole in holes {
            let (min, max) = (hole.min - origin, hole.max - origin);
            let cut = CreateRectRgn(px(min.x), px(min.y), px(max.x), px(max.y));
            CombineRgn(shape, shape, cut, RGN_DIFF);
            DeleteObject(cut);
        }
        SetWindowRgn(hwnd, shape, 1);
    }
}

/// A clicked part of a portal's address bar.
enum Chrome {
    Back,
    Forward,
    Reload,
    Stop,
    Navigate(String),
}

impl DrawingApp {
    /// Click: a 960×680 portal centered there; drag: that box. Asks for the address first.
    pub(crate) fn open_portal_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, Vec2::new(960.0, 680.0))
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(PORTAL_MIN))
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
        let c = self.colors();
        let editing = dialog.target.is_some();
        let title = if editing { "Editar portal" } else { "Novo portal" };
        let (confirm, cancel) = dialog_ui(ctx, &c, title, if editing { "Salvar" } else { "Criar" }, |ui| {
            let address = dialog_field(ui, "Endereço", &mut dialog.spec.url, "https://exemplo.com", DIALOG_WIDTH, &c);
            if !editing && dialog.spec.url.is_empty() && !address.has_focus() {
                address.request_focus();
            }
            dialog_field(ui, "Nome", &mut dialog.spec.name, "opcional", DIALOG_WIDTH, &c);
        });
        if cancel {
            self.portal_dialog = None;
            return;
        }
        self.portal_dialog = Some(dialog);
        if confirm {
            self.confirm_portal_dialog();
        }
    }

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
            paint_icon_scaled(&painter, Icon::Tool(Tool::Portal), header.left_center() + Vec2::new(18.0 * zoom, 0.0), muted, 0.75, 0.0);
            let name = if spec.name.is_empty() { "Portal" } else { &spec.name };
            painter.text(header.left_center() + Vec2::new(32.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, name, FontId::proportional(12.0 * zoom), light);
            // the size only while this portal is being resized
            if self.resizing.as_ref().is_some_and(|r| r.originals.iter().any(|e| e.id == element.id)) {
                let size = format!("{} × {}", b.width().round(), b.height().round());
                painter.text(header.right_center() - Vec2::new(12.0 * zoom, 0.0), egui::Align2::RIGHT_CENTER, size, FontId::monospace(11.0 * zoom), muted);
            }
        }
        let page = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + chrome), rect.max);
        let note = if cfg!(target_os = "linux") { "Portais ainda não abrem no Linux" } else { "Carregando…" };
        painter.text(page.center(), egui::Align2::CENTER_CENTER, note, FontId::proportional(13.0 * zoom), muted);
    }

    /// Another window is enlarged over the canvas: this portal's page and bar stay under it.
    fn lifted_other(&self, element: &Element) -> bool {
        self.terminal_elevated.as_ref().is_some_and(|id| id != &element.id)
    }

    /// Native views sit above everything the app draws, so pages step aside while a menu or
    /// dialog is open.
    fn portals_covered(&self) -> bool {
        self.menu_open
            || self.extra_open
            || self.palette_open
            || self.export_open
            || self.dialog_open()
            || self.elevated_index().is_some_and(|i| self.scene.elements[i].portal.is_none())
            || self.chat_open
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
    fn tab_loading(&self, id: &str) -> bool {
        #[cfg(not(target_os = "linux"))]
        if let Some(tab) = self.open_tab_view(id) {
            return tab.loading;
        }
        let _ = id;
        false
    }

    /// The address bar over each portal's header: back, forward and reload (× while loading),
    /// and the address, which turns into a field on a click.
    pub(crate) fn portal_chrome(&mut self, ctx: &Context) {
        if self.portals_covered() {
            return;
        }
        let portals: Vec<Element> = self.scene.elements.iter().filter(|e| e.portal.as_ref().is_some_and(|p| !p.chrome_hidden) && self.chrome_live(e) && !self.lifted_other(e)).cloned().collect();
        for element in portals {
            let (frame, zoom) = self.terminal_frame(&element);
            let area = Rect::from_min_size(frame.min + Vec2::new(0.0, PORTAL_HEADER * zoom), Vec2::new(frame.width(), PORTAL_BAR * zoom));
            if !area.intersects(self.canvas_rect) {
                continue;
            }
            let spec = element.portal.clone().unwrap_or_default();
            let loading = self.tab_loading(&element.id);
            let editing = self.portal_url_edit.as_ref().filter(|(id, ..)| *id == element.id).cloned();
            let mut address = editing.as_ref().map_or(spec.url.clone(), |(_, text, _)| text.clone());
            let mut action = None;
            // None: no change; Some(None): stop editing; Some(Some(..)): keep editing that text
            let mut next_edit: Option<Option<(String, String, bool)>> = None;
            // not kept on screen by egui: moved down, the bar would lie over its own page
            egui::Area::new(egui::Id::unique(("portal_chrome", element.id.as_str()))).fixed_pos(area.min).constrain(false).order(egui::Order::Middle).show(ctx, |ui| {
                ui.set_clip_rect(area.intersect(self.canvas_rect));
                let (rect, _) = ui.allocate_exact_size(area.size(), Sense::hover());
                let p = ui.painter().clone();
                let (light, muted) = (Color32::from_gray(0xd0), Color32::from_gray(0x8a));
                // back, forward and reload in one pill: 4 pt inside, 24 pt buttons 2 pt apart
                let nav = Rect::from_min_size(Pos2::new(rect.left() + 8.0 * zoom, rect.center().y - 14.0 * zoom), Vec2::new(84.0 * zoom, 28.0 * zoom));
                p.rect_filled(nav, 14.0 * zoom, Color32::from_white_alpha(15));
                let slot = |i: f32| Rect::from_center_size(Pos2::new(nav.left() + (16.0 + 26.0 * i) * zoom, nav.center().y), Vec2::splat(24.0 * zoom));
                for (i, (icon, chrome, tip)) in [(Icon::Arrow(false), Chrome::Back, "Voltar"), (Icon::Arrow(true), Chrome::Forward, "Avançar")].into_iter().enumerate() {
                    let r = ui.interact(slot(i as f32), ui.scope_id().with(("nav", i)), Sense::click());
                    if r.hovered() {
                        p.circle_filled(r.rect.center(), 12.0 * zoom, Color32::from_white_alpha(26));
                    }
                    paint_icon_scaled(&p, icon, r.rect.center(), light, 0.875, 0.0);
                    if r.on_hover_text(tip).clicked() {
                        action = Some(chrome);
                    }
                }
                let reload = ui.interact(slot(2.0), ui.scope_id().with("reload"), Sense::click());
                if reload.hovered() {
                    p.circle_filled(reload.rect.center(), 12.0 * zoom, Color32::from_white_alpha(26));
                }
                if loading {
                    p.text(reload.rect.center(), egui::Align2::CENTER_CENTER, "×", FontId::proportional(14.0 * zoom), light);
                } else {
                    paint_icon_scaled(&p, Icon::Restart, reload.rect.center(), light, 0.75, 0.0);
                }
                if reload.on_hover_text(if loading { "Parar" } else { "Recarregar" }).clicked() {
                    action = Some(if loading { Chrome::Stop } else { Chrome::Reload });
                }
                // the address: a pill with a lens; a click turns it into a field
                let field = Rect::from_min_max(Pos2::new(nav.right() + 6.0 * zoom, nav.top()), Pos2::new(rect.right() - 8.0 * zoom, nav.bottom()));
                p.rect_filled(field, 14.0 * zoom, Color32::from_white_alpha(15));
                paint_icon_scaled(&p, Icon::Search, Pos2::new(field.left() + 16.0 * zoom, field.center().y), muted, 0.75, 0.0);
                let text_room = Rect::from_min_max(Pos2::new(field.left() + 28.0 * zoom, field.top()), Pos2::new(field.right() - 10.0 * zoom, field.bottom()));
                let font = FontId::monospace(12.0 * zoom);
                if let Some((_, _, focus)) = editing.as_ref() {
                    let edit = egui::TextEdit::singleline(&mut address)
                        .frame(egui::Frame::NONE)
                        .font(font)
                        .text_color(light)
                        .hint_text("Digite uma URL…")
                        .desired_width(text_room.width());
                    let response = ui.put(text_room, edit);
                    // a new edit starts with the whole address selected, as the reference app's does
                    let mut selected = false;
                    if *focus {
                        response.request_focus();
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                            let end = egui::text::CCursor::new(address.chars().count());
                            state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(egui::text::CCursor::new(0), end)));
                            state.store(ui.ctx(), response.id);
                            selected = true;
                        }
                    }
                    if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                        next_edit = Some(None);
                    } else if response.lost_focus() {
                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            action = Some(Chrome::Navigate(address.clone()));
                        }
                        next_edit = Some(None);
                    } else {
                        // still owed the selection while the field has no state yet (its first frame)
                        next_edit = Some(Some((element.id.clone(), address.clone(), *focus && !selected)));
                    }
                } else {
                    let click = ui.interact(field, ui.scope_id().with("address"), Sense::click());
                    let (text, color) = if spec.url.is_empty() { ("Digite uma URL…".to_string(), muted) } else { (display_url(&spec.url).to_string(), light) };
                    p.with_clip_rect(text_room.intersect(p.clip_rect())).text(text_room.left_center(), egui::Align2::LEFT_CENTER, text, font, color);
                    if click.on_hover_cursor(CursorIcon::Text).clicked() {
                        next_edit = Some(Some((element.id.clone(), spec.url.clone(), true)));
                    }
                }
            });
            match next_edit {
                Some(Some(state)) => self.portal_url_edit = Some(state),
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
            Chrome::Stop => {
                // a stopped load may not report its finish, so the bar goes back to ⟳ now
                self.set_loading(id, false);
                "window.stop()"
            }
            Chrome::Navigate(url) => return self.portal_navigate(id, &url),
        };
        self.portal_script(id, script);
    }

    #[cfg(not(target_os = "linux"))]
    fn set_loading(&mut self, id: &str, loading: bool) {
        let active = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| Some(e.portal.as_ref()?.active_tab()));
        if let Some(tab) = active.and_then(|a| self.portals.get_mut(id)?.tabs.get_mut(a)) {
            tab.loading = loading;
        }
    }

    #[cfg(target_os = "linux")]
    fn set_loading(&mut self, id: &str, loading: bool) {
        let _ = (id, loading);
    }

    /// One webview per tab of every portal, placed over its page box every frame (the open tab
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
                    view.tabs[i].loading = false;
                    if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == portal).and_then(|e| e.portal.as_mut()) {
                        if spec.tab_urls().get(i) != Some(&url) {
                            spec.set_tab_url(i, url);
                            self.dirty = true;
                        }
                    }
                }
                PortalEvent::NewTab { portal, url } => {
                    // one page per portal: a link that asks for a window opens here
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
        // the overlay's kept messages, where they can't come by ipc (TabView::polls_design)
        if let Some(design) = &self.design {
            let tab = self.portals.get(&design.portal).and_then(|v| v.tabs.iter().find(|t| t.uid == design.tab));
            if let Some(tab) = tab.filter(|t| t.polls_design()) {
                let (events, portal, uid, ctx_note) = (self.portal_events.0.clone(), design.portal.clone(), tab.uid, ctx.clone());
                let _ = tab.webview.evaluate_script_with_callback("window.__magoDesign ? window.__magoDesign.drain() : []", move |json| {
                    for message in serde_json::from_str::<Vec<String>>(&json).unwrap_or_default() {
                        let _ = events.send(PortalEvent::Design { portal: portal.clone(), tab: uid, message });
                        ctx_note.request_repaint();
                    }
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        // the page's zoom follows the scale its card is shown at (100% while enlarged)
        let live: Vec<_> = self
            .all_elements()
            .enumerate()
            .filter_map(|(i, e)| {
                let spec = e.portal.as_ref()?;
                let urls: Vec<String> = spec.tab_urls().iter().map(|u| normalize_url(u)).collect();
                // a hidden workspace's page stays loaded, out of sight
                // ponytail: where it navigates meanwhile isn't kept in its drawing; upgrade: write it to the parked scene
                let shown = i < self.scene.elements.len();
                let (rect, zoom) = if shown { (self.portal_page_rect(e), self.terminal_frame(e).1) } else { (None, 1.0) };
                Some((e.id.clone(), urls, spec.active_tab(), rect, zoom))
            })
            .collect();
        self.portals.retain(|id, _| live.iter().any(|(live, ..)| live == id));
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
        let cards: Vec<egui::Id> = live.iter().map(|(id, ..)| egui::Id::unique(("portal_chrome", id.as_str()))).chain([egui::Id::unique("terminal_overlay")]).collect();
        for (id, urls, active, rect, zoom) in live {
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
            let holes = rect.map_or(Vec::new(), |r| Self::page_holes(ctx, r, &cards));
            // ponytail: only Windows cuts a native view (a window region); elsewhere the page steps
            // aside while anything lies over it. Upgrade: a mask on the NSView's layer on macOS
            #[cfg(not(windows))]
            let rect = rect.filter(|_| holes.is_empty());
            let shown = rect.map(|r| (r, zoom, active, holes));
            if shown != view.shown {
                for (i, tab) in view.tabs.iter().enumerate() {
                    let visible = shown.is_some() && i == active;
                    if let (true, Some((r, zoom, _, holes))) = (visible, &shown) {
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

    /// The roteiro's `shot` keeps the pages too (macOS): each open page as a PNG beside the window's
    /// picture, and the frame its native view has in the window, to check it against the card.
    #[cfg(target_os = "macos")]
    pub(crate) fn save_page_shots(&self, path: &str) {
        use wry::WebViewExtMacOS;
        for element in self.scene.elements.iter().filter(|e| e.portal.is_some()) {
            let Some(rect) = self.portal_page_rect(element) else { continue };
            let Some(tab) = self.open_tab_view(&element.id) else { continue };
            let frame = tab.webview.webview().frame();
            eprintln!("pagina {}: no canvas {:?}, nativa origem ({}, {}) tamanho {}x{}", element.id, rect, frame.origin.x, frame.origin.y, frame.size.width, frame.size.height);
            let (zoom, file) = (self.terminal_frame(element).1 as f64, format!("{path}-{}.png", element.id));
            let css = [0.0, 0.0, frame.size.width / zoom, frame.size.height / zoom];
            super::design::snapshot(&tab.webview, css, move |png| {
                if let Some(png) = png {
                    let _ = std::fs::write(file, png);
                }
            });
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn open_tab_view_mut(&mut self, id: &str) -> Option<&mut TabView> {
        let active = self.scene.elements.iter().find(|e| e.id == id)?.portal.as_ref()?.active_tab();
        self.portals.get_mut(id)?.tabs.get_mut(active)
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
    pub(crate) fn open_tab_view(&self, id: &str) -> Option<&TabView> {
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

    /// Whether `nonce` is the secret the Design overlay of a tab was armed with.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn secret_armed(&self, id: &str, tab: u64, nonce: Option<&str>) -> bool {
        let armed = self.portals.get(id).and_then(|v| v.tabs.iter().find(|t| t.uid == tab)).and_then(|t| t.secret.as_deref());
        armed.is_some() && armed == nonce
    }

    /// An address typed in the bar, or an agent opening one: in the open tab.
    pub(crate) fn portal_navigate(&mut self, id: &str, url: &str) {
        self.snapshot();
        if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == id).and_then(|e| e.portal.as_mut()) {
            let active = spec.active_tab();
            spec.set_tab_url(active, normalize_url(url));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_url_drops_the_scheme() {
        assert_eq!(display_url("https://docs.rs/egui"), "docs.rs/egui");
        assert_eq!(display_url("http://localhost:3000"), "localhost:3000");
        assert_eq!(display_url("about:blank"), "about:blank");
    }
}
