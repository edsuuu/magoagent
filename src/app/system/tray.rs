//! Fechar mantém em segundo plano (Ajustes › Geral): the window's close hides it and the shells and agents keep
//! running. On Windows an icon by the clock brings it back (a click, or Mostrar in its menu) or quits (Sair);
//! elsewhere opening the app again does it (`raise` through the bridge, main.rs). Ctrl+Q quits for real.

use crate::prelude::*;

/// What the icon asked for.
#[derive(Debug, PartialEq)]
pub(crate) enum TrayAsk {
    Show,
    Quit,
    PauseAlerts(u64),
}

#[cfg(windows)]
pub(crate) struct Tray {
    _icon: tray_icon::TrayIcon,
    show: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
    pause_half_hour: tray_icon::menu::MenuId,
    pause_hour: tray_icon::menu::MenuId,
    pause_until_open: tray_icon::menu::MenuId,
}

#[cfg(windows)]
impl Tray {
    pub(crate) fn new() -> Option<Self> {
        use tray_icon::menu::{Menu, MenuItem};
        let picture = image::load_from_memory(include_bytes!("../../../assets/icon.png")).ok()?.thumbnail(64, 64).into_rgba8();
        let (width, height) = picture.dimensions();
        let icon = tray_icon::Icon::from_rgba(picture.into_raw(), width, height).ok()?;
        let show = MenuItem::new("Mostrar MagoAgent", true, None);
        let pause_half_hour = MenuItem::new("Pausar avisos por 30 min", true, None);
        let pause_hour = MenuItem::new("Pausar avisos por 1 h", true, None);
        let pause_until_open = MenuItem::new("Pausar avisos até reabrir", true, None);
        let quit = MenuItem::new("Sair do MagoAgent", true, None);
        let menu = Menu::new();
        menu.append(&show).ok()?;
        menu.append(&pause_half_hour).ok()?;
        menu.append(&pause_hour).ok()?;
        menu.append(&pause_until_open).ok()?;
        menu.append(&quit).ok()?;
        let icon =
            tray_icon::TrayIconBuilder::new().with_icon(icon).with_tooltip(APP_TITLE).with_menu(Box::new(menu)).with_menu_on_left_click(false).build().ok()?;
        Some(Self {
            _icon: icon,
            show: show.id().clone(),
            quit: quit.id().clone(),
            pause_half_hour: pause_half_hour.id().clone(),
            pause_hour: pause_hour.id().clone(),
            pause_until_open: pause_until_open.id().clone(),
        })
    }

    pub(crate) fn poll(&self) -> Option<TrayAsk> {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent, menu::MenuEvent};
        let mut ask = None;
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                ask = Some(TrayAsk::Show);
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit {
                return Some(TrayAsk::Quit);
            }
            if event.id == self.show {
                ask = Some(TrayAsk::Show);
            } else if event.id == self.pause_half_hour {
                ask = Some(TrayAsk::PauseAlerts(30 * 60));
            } else if event.id == self.pause_hour {
                ask = Some(TrayAsk::PauseAlerts(60 * 60));
            } else if event.id == self.pause_until_open {
                ask = Some(TrayAsk::PauseAlerts(u64::MAX));
            }
        }
        ask
    }
}

// ponytail: no tray off Windows (Linux would need libappindicator); opening the app again brings the window back
#[cfg(not(windows))]
pub(crate) struct Tray;

#[cfg(not(windows))]
impl Tray {
    pub(crate) fn new() -> Option<Self> {
        None
    }

    pub(crate) fn poll(&self) -> Option<TrayAsk> {
        None
    }
}

pub(crate) fn alerts_paused(until: Option<u64>, now: u64) -> bool {
    until.is_some_and(|deadline| deadline == u64::MAX || deadline > now)
}

pub(crate) fn unix_seconds() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |duration| duration.as_secs())
}

/// What the window's close does: hide (the app keeps running) or let it close.
pub(crate) fn hides_on_close(close_to_background: bool, quitting: bool, scripted: bool) -> bool {
    close_to_background && !quitting && !scripted
}

impl DrawingApp {
    /// Each pass, shown or hidden: the close that hides, Ctrl+Q, the icon's asks and a second launch's `raise`.
    pub(crate) fn background_window(&mut self, ctx: &Context) {
        if ctx.input_mut(|input| input.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Q))) {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|input| input.viewport().close_requested()) && hides_on_close(self.prefs.close_to_background, self.quitting, self.script.is_some()) {
            self.save_on_exit();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            if cfg!(target_os = "linux") {
                // Wayland can't hide a window (winit ignores Visible there): it goes to the dock instead
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            if self.tray.is_none() {
                self.tray = Tray::new();
            }
        }
        match self.tray.as_ref().and_then(Tray::poll) {
            Some(TrayAsk::PauseAlerts(seconds)) => {
                self.prefs.pause_alerts_until = if seconds == u64::MAX { Some(u64::MAX) } else { Some(unix_seconds().saturating_add(seconds)) };
            }
            Some(TrayAsk::Quit) => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(TrayAsk::Show) => {
                if self.prefs.pause_alerts_until == Some(u64::MAX) {
                    self.prefs.pause_alerts_until = None;
                }
                self.raise_window = true;
            }
            None => {}
        }
        if std::mem::take(&mut self.raise_window) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_close_hides_unless_quitting_off_or_scripted() {
        assert!(hides_on_close(true, false, false));
        assert!(!hides_on_close(false, false, false), "the setting off closes");
        assert!(!hides_on_close(true, true, false), "Ctrl+Q and Sair close");
        assert!(!hides_on_close(true, false, true), "a scripted run's quit closes");
    }

    #[test]
    fn pause_deadline_can_be_checked_without_a_clock_dependency() {
        assert!(alerts_paused(Some(1_900), 1_899));
        assert!(!alerts_paused(Some(1_900), 1_900));
        assert!(alerts_paused(Some(u64::MAX), u64::MAX - 1));
        assert!(!alerts_paused(None, 0));
    }
}
