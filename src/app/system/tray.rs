//! Fechar mantém em segundo plano (Ajustes › Geral): the window's close hides it and the shells and agents keep
//! running. An icon by the clock (Windows), in the menu bar (macOS) or in the top bar (Linux, an AppIndicator), there
//! from the moment the app opens as Chrome's, brings it back (Mostrar in its menu, or a click on Windows and macOS) or
//! quits (Sair); opening the app again also does it (`raise` through the bridge, main.rs). Ctrl+Q quits for real.

use crate::app::system::close_ask::CloseAnswer;
use crate::prelude::*;

/// What the icon asked for.
#[derive(Debug, PartialEq)]
pub(crate) enum TrayAsk {
    Show,
    Quit,
    PauseAlerts(u64),
}

/// The ids of the icon's menu, to tell what was picked.
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
struct Items {
    show: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
    pause_half_hour: tray_icon::menu::MenuId,
    pause_hour: tray_icon::menu::MenuId,
    pause_until_open: tray_icon::menu::MenuId,
}

/// The icon and its menu.
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn build() -> Option<(tray_icon::TrayIcon, Items)> {
    use tray_icon::menu::{Menu, MenuItem};
    let picture = image::load_from_memory(crate::app_icon()).ok()?.thumbnail(64, 64).into_rgba8();
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

    let items = Items {
        show: show.id().clone(),
        quit: quit.id().clone(),
        pause_half_hour: pause_half_hour.id().clone(),
        pause_hour: pause_hour.id().clone(),
        pause_until_open: pause_until_open.id().clone(),
    };

    Some((icon, items))
}

/// What a pick in the menu asks (`None`: no item of it).
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn picked(items: &Items, id: &tray_icon::menu::MenuId) -> Option<TrayAsk> {
    if *id == items.quit {
        Some(TrayAsk::Quit)
    } else if *id == items.show {
        Some(TrayAsk::Show)
    } else if *id == items.pause_half_hour {
        Some(TrayAsk::PauseAlerts(30 * 60))
    } else if *id == items.pause_hour {
        Some(TrayAsk::PauseAlerts(60 * 60))
    } else if *id == items.pause_until_open {
        Some(TrayAsk::PauseAlerts(u64::MAX))
    } else {
        None
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub(crate) struct Tray {
    /// Windows and macOS: the icon lives on the app's thread; Linux: on GTK's (`new`).
    #[cfg(not(target_os = "linux"))]
    _icon: tray_icon::TrayIcon,
    /// Linux: the menu's ids, once GTK's thread has made the icon (`built` brings them).
    items: Option<Items>,
    #[cfg(target_os = "linux")]
    built: std::sync::mpsc::Receiver<Option<Items>>,
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl Tray {
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn new() -> Option<Self> {
        let (icon, items) = build()?;

        Some(Self { _icon: icon, items: Some(items) })
    }

    /// The AppIndicator is GTK's: the icon is made on GTK's thread (system/gtk.rs, shared with the portals' pages) and
    /// lives there for as long as the app does; the app does not wait for it (`poll` takes the menu when it is ready).
    /// None without the library (libayatana-appindicator3, or the older libappindicator3: the tray-icon crate would stop
    /// the app without it).
    #[cfg(target_os = "linux")]
    pub(crate) fn new() -> Option<Self> {
        let library = |name: &str| unsafe { libloading::Library::new(name) }.is_ok();

        if !library("libayatana-appindicator3.so.1") && !library("libappindicator3.so.1") {
            return None;
        }

        thread_local! {
            /// The icon, on GTK's thread.
            static ICON: std::cell::RefCell<Option<tray_icon::TrayIcon>> = const { std::cell::RefCell::new(None) };
        }

        let (made, built) = std::sync::mpsc::channel();

        let sent = crate::app::system::gtk::run(move || {
            let items = build().map(|(icon, items)| {
                ICON.with(|kept| *kept.borrow_mut() = Some(icon));
                items
            });

            let _ = made.send(items);
        });

        sent.then_some(Self { items: None, built })
    }

    pub(crate) fn poll(&mut self) -> Option<TrayAsk> {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent, menu::MenuEvent};

        #[cfg(target_os = "linux")]
        if self.items.is_none() {
            self.items = self.built.try_recv().ok().flatten();
        }

        let items = self.items.as_ref()?;
        let mut ask = None;

        // a click on the icon (Windows and macOS; the AppIndicator only has its menu)
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                ask = Some(TrayAsk::Show);
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match picked(items, &event.id) {
                Some(TrayAsk::Quit) => return Some(TrayAsk::Quit),
                Some(other) => ask = Some(other),
                None => {}
            }
        }

        ask
    }
}

/// Elsewhere there is no icon: opening the app again brings the window back.
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) struct Tray;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
impl Tray {
    pub(crate) fn new() -> Option<Self> {
        None
    }

    pub(crate) fn poll(&mut self) -> Option<TrayAsk> {
        None
    }
}

pub(crate) fn alerts_paused(until: Option<u64>, now: u64) -> bool {
    until.is_some_and(|deadline| deadline == u64::MAX || deadline > now)
}

pub(crate) fn unix_seconds() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |duration| duration.as_secs())
}

/// Whether this pass makes the icon: the first one, as Chrome's is there from the start; a script runs without one.
pub(crate) fn makes_icon(tried: bool, scripted: bool) -> bool {
    !tried && !scripted
}

/// What the window's close does: hide (the app keeps running) or let it close.
pub(crate) fn hides_on_close(close_to_background: bool, quitting: bool, scripted: bool) -> bool {
    close_to_background && !quitting && !scripted
}

impl DrawingApp {
    /// The close that keeps the app running: what is open is saved and the window goes out of sight.
    fn hide_window(&mut self, ctx: &Context) {
        self.save_on_exit();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));

        if cfg!(target_os = "linux") {
            // Wayland can't hide a window (winit ignores Visible there): it goes to the dock instead
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    /// Each pass, shown or hidden: the close that hides, Ctrl+Q, the icon's asks and a second launch's `raise`.
    pub(crate) fn background_window(&mut self, ctx: &Context) {
        if !cfg!(test) && makes_icon(self.tray_tried, self.script.is_some()) {
            self.tray_tried = true;
            self.tray = Tray::new();
        }

        if ctx.input_mut(|input| input.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Q))) {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if ctx.input(|input| input.viewport().close_requested()) && hides_on_close(self.prefs.close_to_background, self.quitting, self.script.is_some()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);

            // agents at work go on hidden only with the person's yes (close_ask.rs)
            if self.agents_at_work().is_empty() && self.chat_queue.is_empty() {
                self.hide_window(ctx);
            } else {
                self.close_ask = true;
                self.raise_window = true;
            }
        }

        if self.close_ask
            && let Some(answer) = self.close_ask_ui(ctx)
        {
            self.close_ask = false;

            match answer {
                CloseAnswer::Background => self.hide_window(ctx),
                CloseAnswer::StopAndClose => {
                    let said = self.stop_all();

                    self.toast(said);
                    self.hide_window(ctx);
                }
                CloseAnswer::Stay => {}
            }
        }

        match self.tray.as_mut().and_then(Tray::poll) {
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
            focus_window(ctx, &crate::window_names().0);

            #[cfg(target_os = "linux")]
            if crate::app::island::x11::active() {
                self.raising = Some(ctx.input(|input| input.time) + RAISE_FOR);
            }
        }

        #[cfg(target_os = "linux")]
        self.raise_x11(ctx);
    }

    /// X11: the window's own ask (winit's focus) is lost while it is being shown again (it is not the window manager's
    /// yet) and does not bring it over the window the person is in. Once the window manager has it, the app asks as a
    /// taskbar would (`x11::activate`), which mutter honours: the person asked for it (the tray, the island, a launch).
    #[cfg(target_os = "linux")]
    fn raise_x11(&mut self, ctx: &Context) {
        let Some(until) = self.raising else { return };

        match crate::app::island::x11::own_window(&crate::window_names().0) {
            Some(window) => {
                crate::app::island::x11::activate(window);
                self.raising = None;
            }
            None if ctx.input(|input| input.time) > until => self.raising = None,
            None => ctx.request_repaint_after(std::time::Duration::from_millis(50)),
        }
    }
}

/// How long a window asked back is looked for (s): the window manager takes it in a few frames.
#[cfg(target_os = "linux")]
const RAISE_FOR: f64 = 3.0;

/// The window `title` of `ctx` in front with the keyboard: on Windows without winit's made-up Alt (`plain::bring_forward`).
pub(crate) fn focus_window(ctx: &Context, title: &str) {
    #[cfg(windows)]
    if crate::app::voice::popup::plain::bring_forward(title) {
        return;
    }

    let _ = title;

    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    #[test]
    fn each_item_of_the_icons_menu_asks_its_own_thing() {
        use tray_icon::menu::MenuId;

        let items = Items {
            show: MenuId::new("mostrar"),
            quit: MenuId::new("sair"),
            pause_half_hour: MenuId::new("30"),
            pause_hour: MenuId::new("60"),
            pause_until_open: MenuId::new("reabrir"),
        };

        assert_eq!(picked(&items, &MenuId::new("mostrar")), Some(TrayAsk::Show));
        assert_eq!(picked(&items, &MenuId::new("sair")), Some(TrayAsk::Quit));
        assert_eq!(picked(&items, &MenuId::new("30")), Some(TrayAsk::PauseAlerts(30 * 60)));
        assert_eq!(picked(&items, &MenuId::new("60")), Some(TrayAsk::PauseAlerts(60 * 60)));
        assert_eq!(picked(&items, &MenuId::new("reabrir")), Some(TrayAsk::PauseAlerts(u64::MAX)));
        assert_eq!(picked(&items, &MenuId::new("outro")), None, "another menu's item asks nothing");
    }

    /// One pass of the app's loop over the window (no window: the commands it sends come back).
    fn pass(app: &mut DrawingApp, ctx: &Context, time: f64) -> Vec<egui::ViewportCommand> {
        let raw = egui::RawInput { time: Some(time), ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.background_window(ctx));

        output.textures_delta.clear();
        output.viewport_output.get(&egui::ViewportId::ROOT).map(|viewport| viewport.commands.clone()).unwrap_or_default()
    }

    #[test]
    fn asked_back_the_window_is_shown_restored_and_focused_and_on_x11_brought_as_a_taskbar_would() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();

        // the bridge's `raise` (a second launch), the icon's Mostrar, the island's Abrir: the same ask
        app.apply_agent_command(crate::mcp::bridge::UiCommand::Raise);
        let sent = pass(&mut app, &ctx, 1.0);

        assert!(!app.raise_window, "asked once");

        for wanted in [egui::ViewportCommand::Visible(true), egui::ViewportCommand::Minimized(false), egui::ViewportCommand::Focus] {
            assert!(sent.contains(&wanted), "{wanted:?} in {sent:?}");
        }

        #[cfg(target_os = "linux")]
        if crate::app::island::x11::active() {
            // no window of this process is the window manager's here: it is looked for a while, then given up
            assert!(app.raising.is_some(), "looked for until the window manager has it");
            pass(&mut app, &ctx, 2.0);
            assert!(app.raising.is_some());
            pass(&mut app, &ctx, 1.0 + RAISE_FOR + 0.5);
            assert_eq!(app.raising, None, "given up");
        }

        assert!(pass(&mut app, &ctx, 9.0).is_empty(), "nothing more is asked of the window");
    }

    #[test]
    fn the_icon_is_made_on_the_first_pass_once_and_never_for_a_script() {
        assert!(makes_icon(false, false), "the app just opened: the icon goes by the clock at once");
        assert!(!makes_icon(true, false), "made (or missing its library) once, never asked again");
        assert!(!makes_icon(false, true), "a scripted run leaves the person's top bar alone");
    }

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
