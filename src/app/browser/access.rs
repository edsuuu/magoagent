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

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn open_tab_view_mut(&mut self, id: &str) -> Option<&mut TabView> {
        let active = self.scene.elements.iter().find(|e| e.id == id)?.portal.as_ref()?.active_tab();
        self.portals.get_mut(id)?.tabs.get_mut(active)
    }

    /// Runs `script` in a portal's open tab; the reply comes back JSON-encoded.
    pub(crate) fn portal_eval(&mut self, id: &str, script: &str, reply: std::sync::mpsc::Sender<Result<String, String>>) {
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
        let _ = reply.send(Err(if cfg!(target_os = "linux") { "navegadores ainda não abrem no Linux" } else { "esse navegador não está aberto" }.into()));
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn open_tab_view(&self, id: &str) -> Option<&TabView> {
        let active = self.scene.elements.iter().find(|e| e.id == id)?.portal.as_ref()?.active_tab();
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
        #[cfg(not(target_os = "linux"))]
        if let Some(tab) = self.open_tab_view(id) {
            let _ = tab.webview.reload();
        }
        let _ = id;
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
