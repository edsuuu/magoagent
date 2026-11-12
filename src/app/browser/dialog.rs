use super::*;

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
        self.tool = Tool::Select;
    }

    pub(crate) fn confirm_portal_dialog(&mut self) {
        let Some(PortalDialog { target, at, mut spec }) = self.portal_dialog.take() else { return };
        let url = normalize_url(&spec.url);
        let active = spec.active_tab();
        spec.set_tab_url(active, url.clone());
        if spec.name.trim().is_empty() {
            spec.name = if host_of(&url).is_empty() { "Navegador".into() } else { host_of(&url).into() };
        }
        self.snapshot();
        if let Some(id) = target {
            let old = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.portal.clone());
            self.portal_extras_changed(&id, old.as_ref(), &spec);
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
        let title = if editing { "Editar navegador" } else { "Novo navegador" };
        let (confirm, cancel) = dialog_ui(ctx, &c, title, if editing { "Salvar" } else { "Criar" }, |ui| {
            let address = dialog_field(ui, "Endereço", &mut dialog.spec.url, "https://exemplo.com", DIALOG_WIDTH, &c);
            if !editing && dialog.spec.url.is_empty() && !address.has_focus() {
                address.request_focus();
            }
            dialog_field(ui, "Nome", &mut dialog.spec.name, "opcional", DIALOG_WIDTH, &c);
            crate::app::browser::extras::dialog_extras(ui, &mut dialog.spec, editing, &c);
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
}
