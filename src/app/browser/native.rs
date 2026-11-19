//! macOS with SwiftUI drawing the canvas (`native::enabled("canvas")`): egui's window is see-through, so the pages go into
//! the canvas's own NSView (`mago_ui_page_host`, macos/MagoUI/Sources/MagoUI/Browser) and `sync_portals` places them there
//! as it does over egui's, in the same points. The card's body and its bar (back, forward, reload, the address, Design) are
//! SwiftUI's: topic `portals`, verbs `ui.browser.*`.

use super::*;
use std::collections::BTreeMap;
use wry::raw_window_handle::{AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};

/// The canvas's NSView, as the window wry makes the pages in.
pub(crate) struct PageHost(std::ptr::NonNull<std::ffi::c_void>);

impl HasWindowHandle for PageHost {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: Swift hands the view only while it is in a window, and a page made in it keeps it alive
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(self.0))) })
    }
}

/// Where the pages go while SwiftUI draws the canvas: its view, None while it is off screen (the chats' screen).
pub(crate) fn page_host() -> Option<PageHost> {
    crate::app::native::page_host().map(PageHost)
}

/// One browser's card as SwiftUI draws it (`PortalCard` in Browser/BrowserArea.swift).
#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct PortalCard {
    /// The address whole (where an edit starts) and as the bar shows it.
    url: String,
    address: String,
    /// The open tab is loading: × stops it.
    loading: bool,
    /// The live bar shows (`portal_chrome`'s rule): not hidden, big enough on screen, nothing over the canvas.
    bar: bool,
    /// Ocultar barra: the page takes the whole card.
    hidden: bool,
    unloaded: bool,
    /// Design is armed on it.
    design: bool,
}

/// Design's Enviar, waiting for the person (`design_confirm_ui`).
#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct DesignAsk {
    name: String,
    prompt: String,
    chat: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct PortalsTopic {
    cards: BTreeMap<String, PortalCard>,
    confirm: Option<DesignAsk>,
}

impl DrawingApp {
    pub(crate) fn portals_topic(&self) -> PortalsTopic {
        let covered = self.portals_covered();

        let cards = self
            .scene
            .elements
            .iter()
            .filter_map(|element| {
                let spec = element.portal.as_ref()?;

                let card = PortalCard {
                    url: spec.url.clone(),
                    address: display_url(&spec.url).to_string(),
                    loading: self.tab_loading(&element.id),
                    bar: !spec.chrome_hidden && !covered && self.chrome_live(element) && !self.lifted_other(element),
                    hidden: spec.chrome_hidden,
                    unloaded: spec.unloaded,
                    design: self.design.as_ref().is_some_and(|design| design.portal == element.id),
                };

                Some((element.id.clone(), card))
            })
            .collect();

        let confirm = self.design_confirm.as_ref().map(|ask| DesignAsk { name: ask.name.clone(), prompt: ask.prompt.clone(), chat: self.is_chat(&ask.agent) });

        PortalsTopic { cards, confirm }
    }

    /// Each frame while SwiftUI draws the canvas: the browsers' cards.
    pub(crate) fn native_browser(&self) {
        crate::app::native::publish("portals", &self.portals_topic());
    }

    /// `ui.browser.<verb>` with `portal` (the element's id): the bar's buttons and address, Design, and Design's Enviar
    /// (`send`, with `send` true or false), as egui's bar and dialog do them.
    pub(crate) fn native_browser_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let id = args["portal"].as_str().unwrap_or_default().to_string();

        if verb == "send" {
            if args["send"].as_bool() != Some(true) {
                self.design_confirm = None;

                return Ok(String::new());
            }

            let ctx = crate::app::native::context().ok_or("a interface não está ligada ao app")?;

            self.finish_design_send(ctx, true);

            return Ok(String::new());
        }

        if !self.scene.elements.iter().any(|element| element.id == id && element.portal.is_some()) {
            return Err(format!("Navegador não encontrado: {id}"));
        }

        let chrome = match verb {
            "back" => Chrome::Back,
            "forward" => Chrome::Forward,
            "reload" => Chrome::Reload,
            "stop" => Chrome::Stop,
            "navigate" => Chrome::Navigate(args["url"].as_str().unwrap_or_default().to_string()),
            "design" => {
                self.toggle_design(&id);

                return Ok(String::new());
            }
            _ => return Err(format!("Ação do navegador desconhecida: {verb}")),
        };

        self.apply_chrome(&id, chrome);
        Ok(String::new())
    }
}
