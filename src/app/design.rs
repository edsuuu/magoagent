//! Portal Design, as the reference app's: arm it on a portal, draw around what should change, and
//! the circled region goes to the agent picked in the composer, with a comment. Kept annotations
//! wait for the next send. The page is a native view above everything the app draws, so the
//! overlay (design.js) lives in the page; the app keeps each region's picture (macOS: the page's
//! snapshot) and the annotations of every portal.

use crate::prelude::*;

/// The overlay's script; `__CONFIG__` becomes the nonce, the agents, the annotations and whether
/// the annotations' composer opens at once.
const DESIGN_SCRIPT: &str = include_str!("design.js");

/// A stroke this small is a tap: nothing to select.
const TAP: f64 = 5.0;

/// The most a page's message may carry: a stroke of 2000 points and a long comment fit.
pub(crate) const MESSAGE_MAX: usize = 256 * 1024;

/// The box of a stroke in the page's CSS px; None when the stroke is only a tap.
pub(crate) fn design_region(points: &[[f64; 2]]) -> Option<[f64; 4]> {
    let mut min = [f64::MAX; 2];
    let mut max = [f64::MIN; 2];
    for p in points {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    let (w, h) = (max[0] - min[0], max[1] - min[1]);
    (w >= TAP || h >= TAP).then_some([min[0], min[1], w, h])
}

/// A kept annotation: a region of the page (its px, scroll included), the note written over it,
/// its picture, the page it was on and how much of it showed (0 × 0 = not told).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Annotation {
    pub(crate) rect: [f64; 4],
    pub(crate) note: String,
    pub(crate) png: Option<Vec<u8>>,
    pub(crate) url: String,
    pub(crate) view: [f64; 2],
}

/// The region the stroke made, its picture once the page has taken it, and the visible area then.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Region {
    pub(crate) rect: [f64; 4],
    pub(crate) png: Option<Vec<u8>>,
    pub(crate) view: [f64; 2],
}

/// Enviar, held until the person says so in the app's own dialog: the page draws the composer,
/// so it could make up a region, a comment and the click.
#[derive(Clone, Debug)]
#[cfg_attr(target_os = "linux", allow(dead_code))] // no portals on Linux
pub(crate) struct DesignConfirm {
    pub(crate) portal: String,
    pub(crate) agent: String,
    pub(crate) name: String,
    pub(crate) prompt: String,
    /// When it showed (`now`): a click in its first moments was aimed at what was there before.
    pub(crate) since: f64,
}

/// Design armed on a portal's open tab.
#[derive(Clone, Debug)]
pub(crate) struct DesignSession {
    pub(crate) portal: String,
    pub(crate) tab: u64,
    /// Counts the strokes, so a picture that comes back late goes only to its own region.
    pub(crate) seq: u64,
    pub(crate) region: Option<Region>,
}

/// A region as the prompt names it: where it is, its picture (a file the agent opens), its note,
/// the page it was on when that is another than the portal's now, and the visible area.
#[derive(Default)]
pub(crate) struct Mark {
    pub(crate) rect: [f64; 4],
    pub(crate) picture: Option<String>,
    pub(crate) note: String,
    pub(crate) page: Option<String>,
    pub(crate) view: [f64; 2],
}

impl Mark {
    fn line(&self) -> String {
        let [x, y, w, h] = self.rect;
        let mut line = format!("x {} y {}, {}×{} px na página.", x.round(), y.round(), w.round(), h.round());
        if let Some(page) = &self.page {
            line += &format!(" Em {page}.");
        }
        let [vw, vh] = self.view;
        if vw > 0.0 && vh > 0.0 {
            line += &format!(" Área visível: {}×{} px.", vw.round(), vh.round());
        }
        if let Some(picture) = &self.picture {
            line += &format!(" Recorte (imagem): {picture}.");
        }
        if !self.note.is_empty() {
            line += &format!(" Nota: {}", self.note);
        }
        line
    }
}

/// What the agent reads: the kept annotations numbered, then the circled region, then the comment.
pub(crate) fn design_prompt(portal: &str, url: &str, kept: &[Mark], region: Option<&Mark>, comment: &str) -> String {
    let mut text = format!("Anotação no portal \"{portal}\" ({url})");
    for (i, mark) in kept.iter().enumerate() {
        text += &format!("\nAnotação {}: {}", i + 1, mark.line());
    }
    if let Some(mark) = region {
        text += &format!("\nRegião circulada: {}", mark.line());
    }
    format!("{text}\n---\n{comment}")
}

/// Standard base64, for a picture the page shows.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    bytes
        .chunks(3)
        .flat_map(|c| {
            let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
            (0..4).map(move |i| if i <= c.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' })
        })
        .collect()
}

/// The picture of a region, as a data URL the page can show.
fn data_url(png: &Option<Vec<u8>>) -> Option<String> {
    png.as_ref().map(|png| format!("data:image/png;base64,{}", base64(png)))
}

/// `[a, b]` from a page's message; 0 for what is missing.
fn pair(value: &Value) -> [f64; 2] {
    [value[0].as_f64().unwrap_or(0.0), value[1].as_f64().unwrap_or(0.0)]
}

/// Text the page supplied, as one short line of the prompt.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(500).collect()
}

/// A comment or note as the page sent it, for the agent's prompt: blank runs closed up (nothing
/// hides below the dialog's fold behind empty lines) and at most `PAGE_TEXT` characters.
fn page_text(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in text.trim().lines().map(str::trim_end) {
        if !(line.is_empty() && out.last().is_some_and(|l| l.is_empty())) {
            out.push(line);
        }
    }
    let text = out.join("
");
    match text.char_indices().nth(PAGE_TEXT) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text,
    }
}

/// The page's textarea takes as much (design.js), so a person's comment is never cut.
pub(crate) const PAGE_TEXT: usize = 4000;

/// Saves a picture for the agent to open (in the temp folder): its path, if it was saved.
fn save_picture(png: &Option<Vec<u8>>) -> Option<String> {
    let dir = std::env::temp_dir().join("magoagent-design");
    let png = png.as_ref()?;
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{}.png", new_id()));
    fs::write(&path, png).ok()?;
    Some(path.display().to_string())
}

/// A PNG of `rect` (x, y, width, height in the page's CSS pixels) of what the page shows now;
/// `done` runs on the main thread when WebKit has it (None if it failed).
#[cfg(target_os = "macos")]
pub(crate) fn snapshot(webview: &wry::WebView, [x, y, w, h]: [f64; 4], done: impl FnOnce(Option<Vec<u8>>) + 'static) {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSDictionary, NSError, NSPoint, NSRect, NSSize};
    use wry::WebViewExtMacOS;
    let Some(mtm) = objc2::MainThreadMarker::new() else { return done(None) };
    let view = webview.webview();
    // the rect is in the view's points: the page's CSS px times its zoom (the canvas zoom, set in sync_portals)
    let zoom = unsafe { view.pageZoom() };
    let [x, y, w, h] = [x, y, w, h].map(|v| v * zoom);
    let config = unsafe { objc2_web_kit::WKSnapshotConfiguration::new(mtm) };
    unsafe { config.setRect(NSRect::new(NSPoint::new(x.max(0.0), y.max(0.0)), NSSize::new(w.max(1.0), h.max(1.0)))) };
    let done = std::cell::Cell::new(Some(done));
    let block = block2::RcBlock::new(move |image: *mut NSImage, _: *mut NSError| {
        let png = unsafe { image.as_ref() }.and_then(|image| {
            let tiff = image.TIFFRepresentation()?;
            let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
            unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }.map(|d| d.to_vec())
        });
        if let Some(done) = done.take() {
            done(png);
        }
    });
    unsafe { view.takeSnapshotWithConfiguration_completionHandler(Some(&config), &block) };
}

impl DrawingApp {
    /// The agent terminals roped to an element: the ones running a command. A plain shell would
    /// run each pasted line as a command.
    pub(crate) fn roped_agents(&self, id: &str) -> Vec<String> {
        self.links().get(id).into_iter().flatten().filter(|o| self.scene.elements.iter().any(|e| &e.id == *o && e.terminal.as_ref().is_some_and(|t| !t.command.trim().is_empty()))).cloned().collect()
    }

    /// The agents the composer offers for a portal: (terminal id, name).
    pub(crate) fn design_agents(&self, portal: &str) -> Vec<(String, String)> {
        self.roped_agents(portal)
            .into_iter()
            .map(|id| {
                let name = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.terminal.as_ref()).map_or_else(|| "Agente".into(), |t| t.name.clone());
                (id, name)
            })
            .collect()
    }

    /// The Design button: arms it on the portal, or takes it off where it is armed.
    pub(crate) fn toggle_design(&mut self, id: &str) {
        if self.design.as_ref().is_some_and(|d| d.portal == id) {
            self.design_exit();
        } else {
            self.arm_design(id, false);
        }
    }

    /// Arms Design on a portal's open page: the overlay goes into the page with the agents and the
    /// kept annotations. `panel` opens the annotations' composer at once.
    pub(crate) fn arm_design(&mut self, id: &str, panel: bool) {
        self.design_exit();
        #[cfg(not(target_os = "linux"))]
        self.arm_tab(id, panel);
        #[cfg(target_os = "linux")]
        let _ = (id, panel);
    }

    #[cfg(not(target_os = "linux"))]
    fn arm_tab(&mut self, id: &str, panel: bool) {
        let agents: Vec<Value> = self.design_agents(id).into_iter().map(|(id, name)| json!({ "id": id, "name": name })).collect();
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let mut config = json!({ "nonce": nonce, "agents": agents, "annotations": self.annotation_list(id), "panel": panel });
        let uid = match self.open_tab_view_mut(id) {
            Some(tab) => {
                config["queue"] = json!(tab.polls_design());
                let script = DESIGN_SCRIPT.replace("__CONFIG__", &config.to_string());
                // ponytail: the secret lives in the page's world, so a page that hooked
                // JSON.stringify before the user sent a note there can forge notes until it
                // navigates; closing that needs a pristine post captured by an initialization
                // script at document start (per platform), the way wry installs window.ipc
                tab.secret = Some(nonce);
                let _ = tab.webview.evaluate_script(&script);
                tab.uid
            }
            None => {
                self.toast("Nenhum quadro do portal está disponível. Saia do Design e tente novamente quando o portal estiver visível.");
                return;
            }
        };
        self.design = Some(DesignSession { portal: id.into(), tab: uid, seq: 0, region: None });
    }

    /// Takes Design off the page; the kept annotations stay.
    pub(crate) fn design_exit(&mut self) {
        if let Some(design) = self.design.take() {
            self.portal_script(&design.portal, "window.__magoDesign&&window.__magoDesign.stop()");
        }
    }

    /// The kept annotations as the page shows them: region, note and a thumbnail.
    fn annotation_list(&self, portal: &str) -> Vec<Value> {
        self.annotations.get(portal).into_iter().flatten().map(|a| json!({ "rect": a.rect, "note": a.note, "thumb": data_url(&a.png) })).collect()
    }

    /// Pushes the kept annotations to the page's overlay, if it is armed on the portal.
    fn design_sync(&self, portal: &str) {
        let list = Value::Array(self.annotation_list(portal));
        self.portal_script(portal, &format!("window.__magoDesign&&window.__magoDesign.sync({list})"));
    }

    pub(crate) fn keep_annotation(&mut self, portal: &str, annotation: Annotation) {
        self.annotations.entry(portal.into()).or_default().push(annotation);
    }

    /// The address of a portal's open tab.
    fn portal_url(&self, portal: &str) -> String {
        // the address the page is at now: a single-page app changes it without a load
        #[cfg(not(target_os = "linux"))]
        if let Some(url) = self.open_tab_view(portal).and_then(|t| t.webview.url().ok()).filter(|u| !u.is_empty()) {
            return url;
        }
        self.scene.elements.iter().find(|e| e.id == portal).and_then(|e| e.portal.as_ref()).map(|p| p.url.clone()).unwrap_or_default()
    }

    /// Drops one kept annotation; the rest renumber.
    pub(crate) fn drop_annotation(&mut self, portal: &str, index: usize) {
        if let Some(kept) = self.annotations.get_mut(portal) {
            if index < kept.len() {
                kept.remove(index);
            }
            if kept.is_empty() {
                self.annotations.remove(portal);
            }
        }
    }

    pub(crate) fn clear_annotations(&mut self, portal: &str) {
        self.annotations.remove(portal);
    }

    /// A message from the overlay in a tab's page: it must come from the armed tab and carry its
    /// secret, so no other page can drive Design.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn design_message(&mut self, portal: &str, tab: u64, message: &str, ctx: &Context) {
        if message.len() > MESSAGE_MAX || !self.design.as_ref().is_some_and(|d| d.portal == portal && d.tab == tab) {
            return;
        }
        let Ok(msg) = serde_json::from_str::<Value>(message) else { return };
        if !self.secret_armed(portal, tab, msg["nonce"].as_str()) {
            return;
        }
        match msg["op"].as_str().unwrap_or_default() {
            "region" => {
                let points: Vec<[f64; 2]> = msg["points"].as_array().into_iter().flatten().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect();
                let Some(rect) = design_region(&points) else {
                    self.portal_script(portal, "window.__magoDesign&&window.__magoDesign.error()");
                    return;
                };
                let Some(design) = self.design.as_mut() else { return };
                design.seq += 1;
                design.region = Some(Region { rect, png: None, view: pair(&msg["view"]) });
                self.portal_script(portal, &format!("window.__magoDesign&&window.__magoDesign.region({})", json!(rect)));
                // the picture of the region comes back on its own (macOS); elsewhere there is none
                #[cfg(target_os = "macos")]
                if let Some(view) = self.open_tab_view(portal) {
                    let seq = self.design.as_ref().map_or(0, |d| d.seq);
                    let events = self.portal_events.0.clone();
                    let (portal, ctx) = (portal.to_string(), ctx.clone());
                    // the snapshot is of what shows: the page's px less the scroll
                    let [sx, sy] = pair(&msg["scroll"]);
                    snapshot(&view.webview, [rect[0] - sx, rect[1] - sy, rect[2], rect[3]], move |png| {
                        let _ = events.send(PortalEvent::Shot { portal, seq, png });
                        ctx.request_repaint();
                    });
                    return;
                }
                let _ = ctx;
                self.portal_script(portal, "window.__magoDesign&&window.__magoDesign.preview(null)");
            }
            "keep" => {
                let note = page_text(msg["comment"].as_str().unwrap_or_default());
                let Some(region) = self.design.as_mut().and_then(|d| d.region.take()) else { return };
                let url = self.portal_url(portal);
                self.keep_annotation(portal, Annotation { rect: region.rect, note, png: region.png, url, view: region.view });
                self.design_sync(portal);
            }
            "drop" => {
                if let Some(index) = msg["index"].as_u64() {
                    self.drop_annotation(portal, index as usize);
                    self.design_sync(portal);
                }
            }
            "clear" => {
                self.clear_annotations(portal);
                self.design_sync(portal);
            }
            "cancel" => {
                if let Some(design) = self.design.as_mut() {
                    design.region = None;
                }
            }
            "exit" => self.design = None,
            "send" => {
                let comment = page_text(msg["comment"].as_str().unwrap_or_default());
                let agent = msg["agent"].as_str().unwrap_or_default().to_string();
                self.design_send(portal, &agent, &comment);
            }
            _ => {}
        }
    }

    /// The picture of a region came back: it goes with the region, if that is still the one drawn.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn design_shot(&mut self, portal: &str, seq: u64, png: Option<Vec<u8>>) {
        let Some(design) = self.design.as_mut().filter(|d| d.portal == portal && d.seq == seq) else { return };
        let Some(region) = design.region.as_mut() else { return };
        let preview = data_url(&png);
        region.png = png;
        self.portal_script(portal, &format!("window.__magoDesign&&window.__magoDesign.preview({})", json!(preview)));
    }

    /// Send: the kept annotations and the region, in one prompt with each picture, for the picked
    /// agent with the comment. Design ends; the text waits for the app's dialog (design_confirm_ui).
    #[cfg(not(target_os = "linux"))]
    fn design_send(&mut self, portal: &str, agent: &str, comment: &str) {
        let region = self.design.take().and_then(|d| d.region);
        let Some((_, agent_name)) = self.design_agents(portal).into_iter().find(|(id, _)| id == agent) else {
            self.toast("Nenhum agente ligado ao portal: a anotação não foi enviada");
            return;
        };
        let Some(element) = self.scene.elements.iter().find(|e| e.id == portal) else { return };
        let name = element.portal.as_ref().map_or("Portal".to_string(), |p| one_line(&p.name));
        let url = one_line(&self.portal_url(portal));
        let page = |at: &str| Some(one_line(at)).filter(|at| !at.is_empty() && *at != url);
        let kept: Vec<Mark> = self.annotations.get(portal).into_iter().flatten().map(|a| Mark { rect: a.rect, picture: save_picture(&a.png), note: a.note.clone(), page: page(&a.url), view: a.view }).collect();
        let region = region.map(|r| Mark { rect: r.rect, picture: save_picture(&r.png), view: r.view, ..Default::default() });
        let prompt = design_prompt(&name, &url, &kept, region.as_ref(), comment);
        self.design_confirm = Some(DesignConfirm { portal: portal.into(), agent: agent.into(), name: agent_name, prompt, since: self.now });
    }

    /// The app's own Enviar, out of the page's reach: only here does the text reach the agent's
    /// terminal. Cancelar keeps the annotations.
    pub(crate) fn design_confirm_ui(&mut self, ctx: &Context) {
        let Some(confirm) = &self.design_confirm else { return };
        let c = self.colors();
        // the page can ask for this dialog whenever it likes: only a click on Enviar, once it has
        // been on screen a moment, sends (an Enter typed elsewhere, held or with Shift doesn't)
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let settled = self.now - confirm.since > 0.4;
        let (ok, cancel) = dialog_ui(ctx, &c, &format!("Enviar a {}?", confirm.name), "Enviar", |ui| {
            dialog_note(ui, "Este texto vai ser digitado no terminal do agente. Confira e clique em Enviar:", &c);
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(260.0).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible).show(ui, |ui| {
                ui.label(egui::RichText::new(confirm.prompt.as_str()).monospace().size(11.5).color(c.text));
            });
        });
        let ok = ok && !enter && settled;
        if ok || cancel {
            self.finish_design_send(ok);
        }
    }

    /// The dialog's answer: Enviar types the text (if the agent is still roped) and the kept
    /// annotations go; Cancelar sends nothing. Refused, the dialog stays with its comment.
    pub(crate) fn finish_design_send(&mut self, send: bool) {
        let Some(confirm) = self.design_confirm.take().filter(|_| send) else { return };
        if !self.roped_agents(&confirm.portal).contains(&confirm.agent) {
            self.toast("O agente não está mais ligado ao portal: a anotação não foi enviada");
            return;
        }
        // refused (the toast says why): the comment and the annotations stay for another try
        if self.send_prompt(&confirm.agent, &confirm.prompt) {
            self.clear_annotations(&confirm.portal);
            self.toast("Anotação enviada a 1 agente");
        } else {
            self.design_confirm = Some(confirm);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_becomes_its_bounding_box_and_a_tap_selects_nothing() {
        assert_eq!(design_region(&[[10.0, 10.0], [50.0, 30.0], [30.0, 70.0]]), Some([10.0, 10.0, 40.0, 60.0]));
        assert_eq!(design_region(&[[10.0, 10.0], [12.0, 11.0]]), None);
        assert_eq!(design_region(&[]), None);
    }

    #[test]
    fn annotations_renumber_when_one_is_dropped() {
        let mut app = DrawingApp::default();
        app.keep_annotation("p", Annotation { rect: [0.0, 0.0, 10.0, 10.0], note: "um".into(), ..Default::default() });
        app.keep_annotation("p", Annotation { rect: [5.0, 5.0, 10.0, 10.0], note: "dois".into(), ..Default::default() });
        app.drop_annotation("p", 0);
        assert_eq!(app.annotations["p"].iter().map(|a| a.note.as_str()).collect::<Vec<_>>(), ["dois"]);
        app.drop_annotation("p", 0);
        assert!(!app.annotations.contains_key("p"));
    }

    #[test]
    fn prompt_lists_each_annotation_then_the_region_then_the_comment() {
        let kept = [Mark { rect: [1.0, 2.0, 3.0, 4.0], picture: Some("/tmp/a.png".into()), note: "troque a cor".into(), ..Default::default() }];
        let region = Mark { rect: [9.0, 8.0, 7.0, 6.0], ..Default::default() };
        let text = design_prompt("Loja", "https://exemplo.com/conta", &kept, Some(&region), "isso");
        assert_eq!(
            text,
            "Anotação no portal \"Loja\" (https://exemplo.com/conta)\nAnotação 1: x 1 y 2, 3×4 px na página. Recorte (imagem): /tmp/a.png. Nota: troque a cor\nRegião circulada: x 9 y 8, 7×6 px na página.\n---\nisso"
        );
    }

    #[test]
    fn a_mark_names_another_page_and_the_visible_area() {
        let mark = Mark { rect: [0.0, 1200.4, 300.0, 80.0], page: Some("https://exemplo.com/conta".into()), view: [1280.0, 720.0], ..Default::default() };
        assert_eq!(mark.line(), "x 0 y 1200, 300×80 px na página. Em https://exemplo.com/conta. Área visível: 1280×720 px.");
    }

    /// Enviar from the page only holds the text: the app's dialog sends it, Cancelar keeps the
    /// annotations, and each annotation says the page it was on when that is another.
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn the_page_send_waits_for_the_apps_dialog() {
        let mut app = DrawingApp::default();
        let mut portal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 800.0, height: 600.0 });
        portal.portal = Some(PortalSpec { name: "Loja".into(), url: "https://exemplo.com/".into(), ..Default::default() });
        let mut agent = app.new_element(ElementKind::Rectangle { x: 900.0, y: 0.0, width: 600.0, height: 400.0 });
        agent.terminal = Some(TerminalSpec { name: "Revisor".into(), command: "claude".into(), ..Default::default() });
        let (portal_id, agent_id) = (portal.id.clone(), agent.id.clone());
        app.scene.elements.extend([portal, agent]);
        app.tie_to_terminal(&agent_id, std::slice::from_ref(&portal_id));
        app.keep_annotation(&portal_id, Annotation { rect: [1.0, 2.0, 3.0, 4.0], url: "https://exemplo.com/conta".into(), ..Default::default() });
        app.keep_annotation(&portal_id, Annotation { rect: [5.0, 6.0, 7.0, 8.0], url: "https://exemplo.com/".into(), ..Default::default() });
        app.design_send(&portal_id, &agent_id, "isso");
        let confirm = app.design_confirm.clone().expect("held for the dialog");
        assert_eq!(confirm.name, "Revisor");
        assert!(confirm.prompt.contains("Anotação 1: x 1 y 2, 3×4 px na página. Em https://exemplo.com/conta."), "{}", confirm.prompt);
        assert!(confirm.prompt.contains("Anotação 2: x 5 y 6, 7×8 px na página.\n"), "{}", confirm.prompt);
        assert_eq!(app.annotations[&portal_id].len(), 2, "nothing went yet");
        app.finish_design_send(false);
        assert!(app.design_confirm.is_none());
        assert_eq!(app.annotations[&portal_id].len(), 2, "Cancelar keeps them");
        // Enviar with the agent's terminal not running: said, and they stay for another try
        app.design_send(&portal_id, &agent_id, "isso");
        app.finish_design_send(true);
        assert_eq!(app.annotations[&portal_id].len(), 2);
        assert_eq!(app.status, "Revisor não está rodando: abra o terminal e envie de novo.");
        assert!(app.design_confirm.as_ref().is_some_and(|c| c.prompt.contains("isso")), "the dialog stays with its comment");
        app.finish_design_send(false);
        // a send for an agent not roped is refused at once
        app.design_send(&portal_id, "outro", "x");
        assert!(app.design_confirm.is_none());
    }

    #[test]
    fn page_text_closes_blank_runs_and_is_capped() {
        assert_eq!(page_text("  ok




rode isso  
"), "ok

rode isso");
        assert_eq!(page_text(&"a".repeat(PAGE_TEXT + 10)).chars().count(), PAGE_TEXT + 1);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn an_enter_does_not_confirm_the_send() {
        let ctx = Context::default();
        let confirm = DesignConfirm { portal: "p".into(), agent: "a".into(), name: "Revisor".into(), prompt: "x".into(), since: 0.0 };
        let mut app = DrawingApp { design_confirm: Some(confirm), now: 5.0, ..Default::default() };
        let enter = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        ctx.run_ui(egui::RawInput { events: vec![enter], ..Default::default() }, |_| app.design_confirm_ui(&ctx)).textures_delta.clear();
        assert!(app.design_confirm.is_some(), "only a click on Enviar sends");
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!((base64(b"Man"), base64(b"Ma"), base64(b"M")), ("TWFu".to_string(), "TWE=".to_string(), "TQ==".to_string()));
    }
}
