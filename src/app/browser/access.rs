use super::*;

impl DrawingApp {
    /// The roteiro's `shot` keeps the pages too (macOS): each open page as a PNG beside the window's
    /// picture, and the frame its native view has in the window, to check it against the card.
    #[cfg(target_os = "macos")]
    pub(crate) fn save_page_shots(&self, path: &str) {
        use wry::WebViewExtMacOS;

        for element in self.scene.elements.iter().filter(|e| e.portal.is_some()) {
            let Some(rect) = self.portal_page_rect(element) else { continue };
            let Some(tab) = self.open_tab_view(&element.id) else { continue };
            let frame = tab.webview.webview().frame();

            eprintln!(
                "pagina {}: no canvas {:?}, nativa origem ({}, {}) tamanho {}x{}",
                element.id, rect, frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
            );

            let (zoom, file) = (self.terminal_frame(element).1 as f64, format!("{path}-{}.png", element.id));
            let css = [0.0, 0.0, frame.size.width / zoom, frame.size.height / zoom];

            super::browser::design::snapshot(&tab.webview, css, move |png| {
                if let Some(png) = png {
                    let _ = std::fs::write(file, png);
                }
            });
        }
    }

    /// The same on Linux: each open page as WebKitGTK shows it, beside the window's picture.
    #[cfg(target_os = "linux")]
    pub(crate) fn save_page_shots(&self, path: &str) {
        for element in self.scene.elements.iter().filter(|e| e.portal.is_some()) {
            let Some(rect) = self.portal_page_rect(element) else { continue };
            let Some(tab) = self.open_tab_view(&element.id) else { continue };

            eprintln!("pagina {}: no canvas {:?}, endereço {}", element.id, rect, tab.webview.url().unwrap_or_default());
            // the screen's scale the page was last placed at
            let scale = self.portals.get(&element.id).and_then(|view| view.shown.as_ref()).map_or(1.0, |shown| shown.4);
            let (id, pixels) = (element.id.clone(), super::webkit::page_pixels(rect, scale));

            tab.webview.frame(move |window, laid_out, zoom| {
                eprintln!("pagina {id}: nativa {window:?} (o cartão em pixels: {pixels:?}), página {laid_out:?} px, zoom {zoom:.3}");
            });

            let file = format!("{path}-{}.png", element.id);

            tab.webview.picture(move |picture| {
                if let Some(png) = picture.and_then(|(width, height, rgba, _)| crate::app::browser::shot::png(width, height, rgba).ok()) {
                    let _ = std::fs::write(file, png);
                }
            });
        }
    }

    pub(crate) fn open_tab_view_mut(&mut self, id: &str) -> Option<&mut TabView> {
        let active = self.portal_spec(id)?.active_tab();

        self.portals.get_mut(id)?.tabs.get_mut(active)
    }

    /// Runs `script` in a portal's open tab; the reply comes back JSON-encoded.
    pub(crate) fn portal_eval(&mut self, id: &str, script: &str, reply: std::sync::mpsc::Sender<Result<String, String>>) {
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

        #[cfg(target_os = "linux")]
        if !pages_open() {
            let _ = reply.send(Err(PAGES_UNAVAILABLE.into()));

            return;
        }

        // a page of a workspace off view lives only if it was loaded when its space left the screen
        let why = if self.scene.elements.iter().any(|e| e.id == id) {
            "esse navegador não está aberto"
        } else {
            "a página desse navegador não está carregada: o espaço dela não foi aberto desde que o MagoAgent começou (ou a página foi descarregada). Siga com outro navegador, ou peça para abrirem o espaço dela uma vez"
        };

        let _ = reply.send(Err(why.into()));
    }

    /// A portal's spec, on the canvas shown or in a workspace off view (`away_portals`).
    pub(crate) fn portal_spec(&self, id: &str) -> Option<&PortalSpec> {
        let shown = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.portal.as_ref());

        shown.or_else(|| self.away_portals.get(id).map(|(_, spec)| spec))
    }

    pub(crate) fn open_tab_view(&self, id: &str) -> Option<&TabView> {
        let active = self.portal_spec(id)?.active_tab();

        self.portals.get(id)?.tabs.get(active)
    }

    /// The browser's own reload: a page that replaced `location` or is stuck in a script still goes.
    /// An unloaded portal loads back instead.
    pub(crate) fn portal_reload(&mut self, id: &str) {
        if let Some(i) = self.scene.elements.iter().position(|e| e.id == id)
            && self.unload_node(i, false)
        {
            return;
        }

        if let Some(tab) = self.open_tab_view(id) {
            let _ = tab.webview.reload();
        }
    }

    pub(crate) fn portal_script(&self, id: &str, script: &str) {
        if let Some(tab) = self.open_tab_view(id) {
            let _ = tab.webview.evaluate_script(script);
        }
    }

    /// Whether `nonce` is the secret the Design overlay of a tab was armed with.
    pub(crate) fn secret_armed(&self, id: &str, tab: u64, nonce: Option<&str>) -> bool {
        let armed = self.portals.get(id).and_then(|v| v.tabs.iter().find(|t| t.uid == tab)).and_then(|t| t.secret.as_deref());

        armed.is_some() && armed == nonce
    }

    /// An address typed in the bar, or an agent opening one: in the open tab.
    pub(crate) fn portal_navigate(&mut self, id: &str, url: &str) {
        // off view: the page loads it (`sync_portals`), and its space shows it on return
        if let Some((_, spec)) = self.away_portals.get_mut(id) {
            let active = spec.active_tab();

            spec.set_tab_url(active, normalize_url(url));

            return;
        }

        self.snapshot();

        if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == id).and_then(|e| e.portal.as_mut()) {
            let active = spec.active_tab();

            spec.set_tab_url(active, normalize_url(url));
        }
    }
}
