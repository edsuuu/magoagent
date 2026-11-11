//! What a portal keeps beyond its pages: the user agent it tells sites, where its cookies live
//! (isolated, shared by the workspace, or global), whether it is muted, the size readout on its
//! header and the copy of its page's picture.

use crate::prelude::*;

/// Where a portal's cookies and logins live (Armazenamento).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum StorageScope {
    #[default]
    Isolated,
    Workspace,
    Global,
}

impl StorageScope {
    pub(crate) const ALL: [StorageScope; 3] = [StorageScope::Isolated, StorageScope::Workspace, StorageScope::Global];

    pub(crate) fn label(self) -> &'static str {
        match self {
            StorageScope::Isolated => "Isolado",
            StorageScope::Workspace => "Espaço de trabalho",
            StorageScope::Global => "Global",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            StorageScope::Isolated => "isolated",
            StorageScope::Workspace => "workspace",
            StorageScope::Global => "global",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == text || s.label().eq_ignore_ascii_case(text))
    }
}

/// Mostrar Tamanho: when a portal's header shows its width × height.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SizeReadout {
    Never,
    #[default]
    OnResize,
    Always,
}

impl SizeReadout {
    pub(crate) const ALL: [SizeReadout; 3] = [SizeReadout::Never, SizeReadout::OnResize, SizeReadout::Always];

    pub(crate) fn label(self) -> &'static str {
        match self {
            SizeReadout::Never => "Nunca",
            SizeReadout::OnResize => "Ao Redimensionar",
            SizeReadout::Always => "Sempre",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            SizeReadout::Never => "never",
            SizeReadout::OnResize => "onresize",
            SizeReadout::Always => "always",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == text.to_lowercase() || s.label().eq_ignore_ascii_case(text))
    }
}

/// The user agents the reference app offers: label, whether a phone's, and the string.
pub(crate) const USER_AGENTS: [(&str, bool, &str); 7] = [
    ("Chrome", false, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36"),
    ("Firefox", false, "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0"),
    ("Microsoft Edge", false, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36 Edg/125.0.0.0"),
    (
        "Safari (iPhone)",
        true,
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
    ),
    ("Chrome (Android)", true, "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Mobile Safari/537.36"),
    ("Firefox (Android)", true, "Mozilla/5.0 (Android 14; Mobile; rv:126.0) Gecko/126.0 Firefox/126.0"),
    (
        "Microsoft Edge (Android)",
        true,
        "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Mobile Safari/537.36 EdgA/125.0.0.0",
    ),
];

/// The label of the default row: what the system's webview is.
const DEFAULT_AGENT: &str = if cfg!(target_os = "macos") { "Padrão (WebKit)" } else { "Padrão (Chromium)" };

/// Chrome's desktop agent at a Chromium version ("131.0.2903.70"), the preset's without one.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn chrome_agent(version: Option<&str>) -> String {
    let preset = USER_AGENTS[0].2;
    match version.and_then(|v| v.split('.').next()).filter(|major| !major.is_empty() && major.bytes().all(|b| b.is_ascii_digit())) {
        Some(major) => preset.replace("Chrome/125.0.0.0", &format!("Chrome/{major}.0.0.0")),
        None => preset.into(),
    }
}

/// What a user agent given by name or by string means: a preset's string, "" for the default, or
/// the text itself (Personalizado).
pub(crate) fn resolve_user_agent(text: &str) -> String {
    let text = text.trim();
    match USER_AGENTS.iter().find(|(label, ..)| label.eq_ignore_ascii_case(text)) {
        Some((.., agent)) => (*agent).into(),
        None if text.eq_ignore_ascii_case("padrão") || text.eq_ignore_ascii_case("padrao") => String::new(),
        None => text.into(),
    }
}

/// The row's text for a user agent string.
fn user_agent_label(agent: &str) -> String {
    match (agent, USER_AGENTS.iter().find(|(.., s)| *s == agent)) {
        ("", _) => DEFAULT_AGENT.into(),
        (_, Some((label, ..))) => (*label).into(),
        _ => "Personalizado…".into(),
    }
}

/// Keeps a page's audio and video muted, the ones the page makes later too. ponytail: only
/// <audio> and <video> elements (no webview here has a mute switch; WebView2's needs a COM
/// interface wry doesn't expose), so Web Audio stays loud; per-system switches if that matters.
pub(crate) fn mute_script(on: bool) -> String {
    format!(
        "(()=>{{window.__magoMuted={on};\
         const f=()=>document.querySelectorAll('audio,video').forEach(m=>{{\
         if(window.__magoMuted){{if(!m.muted){{m.__magoM=1;m.muted=true}}}}else if(m.__magoM){{m.muted=false;delete m.__magoM}}}});\
         if(!window.__magoMute){{if(!{on})return;window.__magoMute=f;new MutationObserver(f).observe(document,{{childList:true,subtree:true}});\
         document.addEventListener('play',f,true)}}f()}})()"
    )
}

/// The header's readout: "800 × 600".
pub(crate) fn size_text(width: f32, height: f32) -> String {
    format!("{} × {}", width.round(), height.round())
}

/// A stable 16-byte name for a profile (the Mac keeps cookies in a data store named by it).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn store_id(name: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (half, seed) in out.chunks_mut(8).zip([0xcbf29ce484222325u64, 0x84222325cbf29ce4]) {
        let hash = name.bytes().fold(seed, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        half.copy_from_slice(&hash.to_le_bytes());
    }
    out
}

/// What a page is made with beyond its address.
#[cfg(not(target_os = "linux"))]
pub(crate) struct TabExtras {
    pub(crate) user_agent: String,
    pub(crate) muted: bool,
    /// The profile's folder name, which names the Mac's data store.
    pub(crate) store: String,
    pub(crate) profile: Option<PathBuf>,
}

/// The builder with the user agent, the mute and (macOS) the data store of `extras`.
#[cfg(not(target_os = "linux"))]
pub(crate) fn tune<'a>(mut builder: wry::WebViewBuilder<'a>, extras: &TabExtras) -> wry::WebViewBuilder<'a> {
    if !extras.user_agent.is_empty() {
        builder = builder.with_user_agent(extras.user_agent.clone());
    } else if cfg!(windows) {
        // WebView2's own agent says "Edg/", and Google answers it with a "Use o Chrome" popup on every
        // visit: the default is Chrome's agent at the engine's real version
        builder = builder.with_user_agent(chrome_agent(wry::webview_version().ok().as_deref()));
    }
    if extras.muted {
        builder = builder.with_initialization_script(mute_script(true));
    }
    #[cfg(target_os = "macos")]
    {
        use wry::WebViewBuilderExtDarwin;
        builder = builder.with_data_store_identifier(store_id(&extras.store));
    }
    let _ = &extras.store;
    builder
}

/// What the app owes the portals' extras between frames.
#[derive(Default)]
pub(crate) struct PortalExtras {
    /// Portals whose page's picture goes to the clipboard on the next frame.
    pub(crate) copy: Vec<String>,
    /// Portals whose pages run the mute script on the next frame.
    pub(crate) mute: Vec<String>,
}

impl DrawingApp {
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn tab_extras(&self, id: &str) -> TabExtras {
        let spec = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.portal.clone()).unwrap_or_default();
        let space = self.current_space().map_or("principal", |s| s.id.as_str());
        let (store, profile) = match spec.storage {
            StorageScope::Isolated => (portal_profile_name(id), portal_profile_path(id)),
            StorageScope::Workspace => {
                let name = format!("espaco-{}", portal_profile_name(space));
                (name.clone(), autosave_path().map(|p| p.with_file_name("perfis").join(name)))
            }
            StorageScope::Global => ("global".into(), data_dir().map(|d| d.join("perfis").join("global"))),
        };
        TabExtras { user_agent: spec.user_agent, muted: spec.muted, store, profile }
    }

    /// A portal's user agent or storage changed: its pages start over, since both only count when
    /// a page is made. The mute only needs its script run again.
    pub(crate) fn portal_extras_changed(&mut self, id: &str, old: Option<&PortalSpec>, new: &PortalSpec) {
        let Some(old) = old else { return };
        #[cfg(not(target_os = "linux"))]
        if old.user_agent != new.user_agent || old.storage != new.storage {
            self.portals.remove(id);
        }
        if old.muted != new.muted {
            self.portal_extras.mute.push(id.to_string());
        }
    }

    /// The mute script in every page of a portal (a page made later has it from `tune`).
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn portal_apply_mute(&self, id: &str) {
        let Some(spec) = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.portal.as_ref()) else { return };
        let script = mute_script(spec.muted);
        for tab in self.portals.get(id).into_iter().flat_map(|v| &v.tabs) {
            let _ = tab.webview.evaluate_script(&script);
        }
    }

    /// Each frame: mutes asked for, pictures copied.
    pub(crate) fn portal_extras_tick(&mut self, ctx: &Context) {
        #[cfg(not(target_os = "linux"))]
        for id in std::mem::take(&mut self.portal_extras.mute) {
            self.portal_apply_mute(&id);
        }
        for id in std::mem::take(&mut self.portal_extras.copy) {
            match self.page_rgba(&id) {
                Ok((w, h, rgba)) => {
                    ctx.copy_image(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba));
                    self.toast("Captura de tela copiada.");
                }
                Err(error) => self.toast(error),
            }
        }
    }

    /// Copiar Captura de Tela: the open page's picture on the clipboard, a frame later.
    pub(crate) fn portal_copy_shot(&mut self, id: &str) -> Result<String, String> {
        if !cfg!(windows) {
            return Err("a captura da página só existe no Windows por enquanto".into());
        }
        let spec = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.portal.as_ref()).ok_or("não é um portal")?;
        if spec.unloaded {
            return Err("o portal está descarregado".into());
        }
        self.portal_extras.copy.push(id.to_string());
        Ok("Captura de tela copiada.".into())
    }

    /// Whether the header shows the size now.
    pub(crate) fn size_shown(&self, element: &Element) -> bool {
        match self.prefs.portal_size {
            SizeReadout::Never => false,
            SizeReadout::Always => true,
            SizeReadout::OnResize => self.resizing.as_ref().is_some_and(|r| r.originals.iter().any(|e| e.id == element.id)),
        }
    }

    /// A row of the portal's menu: `portal:mute`, `portal:shot`, `portal:size:<key>`, `portal:storage:<key>`.
    pub(crate) fn portal_menu(&mut self, id: &str, op: &str) {
        let (key, value) = match op.split_once(':') {
            Some(("size", key)) => ("size_readout", json!(key)),
            Some(("storage", key)) => ("storage", json!(key)),
            _ if op == "shot" => {
                if let Err(error) = self.portal_copy_shot(id) {
                    self.toast(error);
                }
                return;
            }
            _ => ("muted", json!(!self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.portal.as_ref()).is_some_and(|p| p.muted))),
        };
        if let Err(error) = self.act_portal_settings(&json!({ "portal": id, key: value })) {
            self.toast(error);
        }
    }

    /// The MCP's `portal_settings`: any of user_agent, storage, muted and (for every portal)
    /// size_readout.
    pub(crate) fn act_portal_settings(&mut self, args: &Value) -> Result<String, String> {
        let name = args["portal"].as_str().unwrap_or_default();
        let at = self.node_named(name)?;
        let id = self.scene.elements[at].id.clone();
        let old = self.scene.elements[at].portal.clone().ok_or_else(|| format!("{name} não é um portal"))?;
        let mut spec = old.clone();
        if let Some(agent) = args["user_agent"].as_str() {
            spec.user_agent = resolve_user_agent(agent);
        }
        if let Some(text) = args["storage"].as_str() {
            spec.storage = StorageScope::parse(text).ok_or("storage: isolated, workspace ou global")?;
        }
        if let Some(on) = args["muted"].as_bool() {
            spec.muted = on;
        }
        if let Some(text) = args["size_readout"].as_str() {
            self.prefs.portal_size = SizeReadout::parse(text).ok_or("size_readout: never, onresize ou always")?;
        }
        if spec != old {
            self.snapshot();
            self.portal_extras_changed(&id, Some(&old), &spec);
            self.scene.elements[at].portal = Some(spec);
        }
        Ok(format!("{name} ajustado."))
    }
}

/// The dialog's rows for the user agent and the storage.
pub(crate) fn dialog_extras(ui: &mut egui::Ui, spec: &mut PortalSpec, editing: bool, c: &UiColors) {
    let label = |ui: &mut egui::Ui, text: &str| {
        ui.add_space(6.0);
        dialog_caption(ui, text, c);
    };
    label(ui, "Agente de Usuário");
    let custom_id = ui.scope_id().with("portal_ua_custom");
    let preset = spec.user_agent.is_empty() || USER_AGENTS.iter().any(|(.., s)| *s == spec.user_agent);
    let custom = !preset || ui.data(|d| d.get_temp::<bool>(custom_id)).unwrap_or(false);
    egui::ComboBox::from_id_salt("portal_user_agent")
        .selected_text(if custom { "Personalizado…".into() } else { user_agent_label(&spec.user_agent) })
        .width(DIALOG_WIDTH)
        .show_ui(ui, |ui| {
            let mut pick = |ui: &mut egui::Ui, text: &str, agent: &str| {
                if ui.selectable_label(!custom && spec.user_agent == agent, text).clicked() {
                    spec.user_agent = agent.into();
                    ui.data_mut(|d| d.insert_temp(custom_id, false));
                }
            };
            pick(ui, DEFAULT_AGENT, "");
            for (group, phone) in [("Computador", false), ("Celular", true)] {
                ui.label(egui::RichText::new(group).size(11.0).color(dialog_label(c)));
                for (name, _, agent) in USER_AGENTS.iter().filter(|(_, p, _)| *p == phone) {
                    pick(ui, name, agent);
                }
            }
            if ui.selectable_label(custom, "Personalizado…").clicked() {
                ui.data_mut(|d| d.insert_temp(custom_id, true));
                if preset {
                    spec.user_agent.clear();
                }
            }
        });
    if custom {
        dialog_field(ui, "Agente de usuário personalizado", &mut spec.user_agent, "Mozilla/5.0 …", DIALOG_WIDTH, c);
    }
    label(ui, "Armazenamento");
    let mut chosen = StorageScope::ALL.iter().position(|s| *s == spec.storage).unwrap_or(0);
    if segmented(ui, &mut chosen, &StorageScope::ALL.map(StorageScope::label), c) {
        spec.storage = StorageScope::ALL[chosen];
    }
    if editing {
        dialog_note(ui, "O agente de usuário e o armazenamento só valem quando a página carrega, então salvar recarrega este portal.", c);
    }
}

/// The muted speaker on a header: a small speaker with a slash through it.
pub(crate) fn paint_muted(painter: &Painter, center: Pos2, zoom: f32, color: Color32) {
    let s = zoom;
    let at = |x: f32, y: f32| center + Vec2::new(x, y) * s;
    painter.add(Shape::convex_polygon(vec![at(-5.0, -2.0), at(-2.0, -2.0), at(2.0, -5.0), at(2.0, 5.0), at(-2.0, 2.0), at(-5.0, 2.0)], color, Stroke::NONE));
    painter.line_segment([at(4.0, -3.0), at(8.0, 3.0)], Stroke::new(1.2 * s, color));
    painter.line_segment([at(8.0, -3.0), at(4.0, 3.0)], Stroke::new(1.2 * s, color));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_agent_is_chrome_at_the_engines_version() {
        let agent = chrome_agent(Some("131.0.2903.70"));
        assert!(agent.contains("Chrome/131.0.0.0") && !agent.contains("Edg/"), "{agent}");
        assert_eq!(chrome_agent(None), USER_AGENTS[0].2);
        assert_eq!(chrome_agent(Some("x")), USER_AGENTS[0].2);
    }

    #[test]
    fn user_agents_resolve_by_name_string_or_default() {
        assert!(resolve_user_agent("firefox").contains("Firefox/126.0"));
        assert!(resolve_user_agent(" Safari (iPhone) ").contains("iPhone"));
        assert_eq!(resolve_user_agent("Padrão"), "");
        assert_eq!(resolve_user_agent("MeuBot/1.0"), "MeuBot/1.0");
        assert_eq!(user_agent_label(USER_AGENTS[4].2), "Chrome (Android)");
        assert_eq!(user_agent_label("MeuBot/1.0"), "Personalizado…");
        assert!(USER_AGENTS.iter().all(|(_, phone, s)| *phone == (s.contains("Mobile") || s.contains("iPhone"))), "the groups match the strings");
    }

    #[test]
    fn the_mute_script_mutes_and_restores() {
        assert!(mute_script(true).starts_with("(()=>{window.__magoMuted=true;"));
        assert!(mute_script(false).contains("window.__magoMuted=false;") && mute_script(false).contains("if(!false)return;"));
        assert!(mute_script(true).contains("m.muted=true") && mute_script(true).contains("m.muted=false"));
    }

    #[test]
    fn size_text_rounds() {
        assert_eq!(size_text(960.4, 679.6), "960 × 680");
    }

    #[test]
    fn profile_stores_are_stable_and_apart() {
        assert_eq!(store_id("global"), store_id("global"));
        assert_ne!(store_id("global"), store_id("espaco-a"));
        assert_ne!(store_id("a"), store_id("b"));
    }

    #[test]
    fn the_spec_keeps_and_reads_back_what_a_portal_adds() {
        let spec = PortalSpec { user_agent: "X/1".into(), storage: StorageScope::Global, muted: true, ..Default::default() };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"global\""), "{json}");
        assert_eq!(serde_json::from_str::<PortalSpec>(&json).unwrap(), spec);
        // a plain portal writes none of it, and an old file reads as isolated, loud, default agent
        let plain = serde_json::to_string(&PortalSpec::default()).unwrap();
        assert!(!plain.contains("storage") && !plain.contains("muted") && !plain.contains("user_agent"), "{plain}");
        let old: PortalSpec = serde_json::from_str(r#"{"name":"a","url":"https://a.dev"}"#).unwrap();
        assert_eq!((old.storage, old.muted, old.user_agent.as_str()), (StorageScope::Isolated, false, ""));
    }

    #[test]
    fn settings_change_the_portal_and_the_readout() {
        let mut app = DrawingApp::default();
        let mut portal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 400.0 });
        portal.portal = Some(PortalSpec { name: "Docs".into(), url: "https://example.com".into(), ..Default::default() });
        app.scene.elements.push(portal);
        app.act(
            "portal_settings",
            &json!({ "portal": "Docs", "user_agent": "chrome (android)", "storage": "workspace", "muted": true, "size_readout": "always" }),
        )
        .unwrap();
        let spec = app.scene.elements[0].portal.clone().unwrap();
        assert!(spec.user_agent.contains("Pixel 8") && spec.storage == StorageScope::Workspace && spec.muted);
        assert_eq!(app.prefs.portal_size, SizeReadout::Always);
        assert!(app.size_shown(&app.scene.elements[0].clone()));
        assert!(app.act("portal_settings", &json!({ "portal": "Docs", "storage": "nuvem" })).is_err());
        // the menu has the rows
        let rows = format!("{:?}", app.context_entries(&crate::app::ui::context::Target::Element(app.scene.elements[0].id.clone())));
        for text in ["Mostrar Tamanho", "Copiar Captura de Tela", "Ativar som", "Armazenamento", "portal:storage:global"] {
            assert!(rows.contains(text), "{text}");
        }
        // the menu rows reach the same place
        let id = app.scene.elements[0].id.clone();
        app.portal_menu(&id, "mute");
        assert!(!app.scene.elements[0].portal.as_ref().unwrap().muted, "the row toggles the mute");
        app.portal_menu(&id, "storage:global");
        app.portal_menu(&id, "size:never");
        assert_eq!(app.scene.elements[0].portal.as_ref().unwrap().storage, StorageScope::Global);
        assert!(!app.size_shown(&app.scene.elements[0].clone()));
    }
}
