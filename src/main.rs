//! MagoAgent: a native whiteboard with a built-in MCP server (`magoagent mcp`).

// no console window behind the app on Windows; `magoagent mcp` still talks over the pipes its client gives it
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod binding;
mod bridge;
mod file;
mod geometry;
mod ide;
mod labels;
mod mcp;
mod model;
mod recognize;
mod render;
mod rope;
mod terminal;
#[cfg(test)]
mod tests;
mod transcript;
mod voice;

/// What every module sees: std/egui/serde basics and the crate's shared items.
mod prelude {
    pub(crate) use std::{fs, path::PathBuf, time::SystemTime};

    pub(crate) use eframe::egui::{self, Color32, Context, CursorIcon, FontId, Key, Modifiers, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{Value, json};

    pub(crate) use crate::app::{ide::*, portal::*, script::*, terminal::*, tool::*, ui::icons::*, ui::*, *};
    pub(crate) use crate::file::*;
    pub(crate) use crate::render::{replay::*, rough::*, svg::*, *};
    pub(crate) use crate::{binding::*, bridge::*, geometry::*, labels::*, model::*, recognize::*, rope::*, terminal::*, voice::*};
}

use prelude::*;

fn main() -> eframe::Result {
    // a Claude Code session that started the app (show_app, a shell inside one) leaves its markers:
    // the `claude` in our terminals would run as its child session (no transcript, its ids).
    // Without CLAUDECODE the CLAUDE_CODE_* are the person's own settings, and stay
    if std::env::var_os("CLAUDECODE").is_some() {
        for (key, _) in std::env::vars_os() {
            if key.to_str().is_some_and(|k| k == "CLAUDECODE" || k.starts_with("CLAUDE_CODE_")) {
                // SAFETY: first thing in main, before any other thread starts
                unsafe { std::env::remove_var(key) };
            }
        }
    }
    // a window app has no console on Windows: a panic also goes to crash.log in the data folder
    let console = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = data_dir() {
            let at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let entry = format!("{at} {info}\n{}\n\n", std::backtrace::Backtrace::force_capture());
            let file = fs::create_dir_all(&dir).and_then(|_| fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log")));
            let _ = file.and_then(|mut f| std::io::Write::write_all(&mut f, entry.as_bytes()));
        }
        console(info);
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => {
            mcp::run();
            return Ok(());
        }
        Some("list" | "ask" | "check" | "note" | "portal" | "recruit" | "connect" | "notify" | "help") => {
            attach_console();
            std::process::exit(cli(&args))
        }
        _ => {}
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_TITLE)
        .with_inner_size([1440.0, 920.0])
        .with_min_inner_size([920.0, 640.0])
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).expect("assets/icon.png is a png"));
    // a scripted run (screen tests) opens without taking the keyboard from the person's app
    let scripted = std::env::var_os("MAGOAGENT_SCRIPT").is_some_and(|s| !s.is_empty());
    if scripted {
        viewport = viewport.with_active(false);
    }
    if cfg!(target_os = "macos") {
        // the canvas runs under the title bar; the toolbar sits beside the window buttons
        viewport = viewport.with_fullsize_content_view(true).with_title_shown(false).with_titlebar_shown(false);
    } else {
        // the app draws its own title bar, with the main menu in it (ui/titlebar.rs)
        viewport = viewport.with_decorations(false);
    }
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut native_options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if scripted {
        // an accessory app: no Dock icon, and it never steals the front from the app in use
        native_options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory).with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(|cc| {
            #[cfg(target_os = "macos")]
            remember_window(cc);
            let mut app = DrawingApp { autosave: autosave_path(), script: Script::from_env(), perf: app::script::Perf::from_env(), ..Default::default() };
            app.bridge = Bridge::start(Some(cc.egui_ctx.clone())).ok();
            if let Some(bridge) = &app.bridge {
                bridge.write_endpoint();
            }
            let (prefs, bad_prefs) = prefs_path().map(|p| read_prefs(&p)).unwrap_or_default();
            if let Some(prefs) = prefs {
                app.tool_locked = prefs.tool_locked;
                app.saved_prefs = prefs.clone();
                app.prefs = prefs;
            }
            install_fonts(&cc.egui_ctx, app.prefs.ligatures);
            app.ligatures_shown = app.prefs.ligatures;
            app.load_spaces();
            app.load_autosave();
            app.load_chats();
            // a roteiro drives the canvas
            app.chat_open = app.prefs.start_in_chat && app.script.is_none();
            if let Some(kept) = bad_prefs {
                // after the drawing's own warning, if it had one
                let earlier = std::mem::take(&mut app.status);
                let message = format!("Não foi possível ler preferences.json: ficou guardado em {} e as preferências voltaram ao padrão.", kept.display());
                app.toast(if earlier.is_empty() { message } else { format!("{earlier}\n{message}") });
            }
            Ok(Box::new(app))
        }),
    )
}

/// The Windows build has no console of its own: the CLI writes into the one it was typed in (a
/// pipe its caller gave it is used as is).
// ponytail: an interactive cmd or PowerShell doesn't wait for a windowed program, so the prompt can
// come back before the answer; a console-subsystem `magoagent.com` beside the exe would fix that
fn attach_console() {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn AttachConsole(process: u32) -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
        // SAFETY: no arguments to get wrong; with no parent console it just fails
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    }
}

#[cfg(target_os = "macos")]
thread_local! {
    static WINDOW: std::cell::RefCell<Option<objc2::rc::Retained<objc2_app_kit::NSWindow>>> = Default::default();
}

#[cfg(target_os = "macos")]
fn remember_window(cc: &eframe::CreationContext) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = cc.window_handle() else { return };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return };
    // SAFETY: the handle comes from the live window this callback was handed
    let view: &objc2_app_kit::NSView = unsafe { handle.ns_view.cast().as_ref() };
    WINDOW.with(|w| *w.borrow_mut() = view.window());
}

/// Centers the close / minimize / zoom buttons `center` points below the window top, like
/// Electron's trafficLightPosition: the title bar container grows to hold them. AppKit lays
/// them out again on resize, so this runs every frame.
#[cfg(target_os = "macos")]
pub(crate) fn place_window_buttons(center: f64) {
    use objc2_app_kit::NSWindowButton;
    WINDOW.with(|w| {
        let Some(window) = &*w.borrow() else { return };
        let kinds = [NSWindowButton::CloseButton, NSWindowButton::MiniaturizeButton, NSWindowButton::ZoomButton];
        let buttons: Vec<_> = kinds.into_iter().filter_map(|k| window.standardWindowButton(k)).collect();
        // SAFETY: plain view-tree lookups on the main thread: button → title bar → its container
        let container = buttons.first().and_then(|b| unsafe { b.superview() }).and_then(|v| unsafe { v.superview() });
        let Some(container) = container else { return };
        let mut bounds = container.frame();
        bounds.size.height = center * 2.0;
        bounds.origin.y = window.frame().size.height - bounds.size.height;
        container.setFrame(bounds);
        for (i, button) in buttons.iter().enumerate() {
            let mut origin = button.frame().origin;
            origin.x = 18.0 + i as f64 * 20.0;
            origin.y = (bounds.size.height - button.frame().size.height) / 2.0;
            button.setFrameOrigin(origin);
        }
    });
}
