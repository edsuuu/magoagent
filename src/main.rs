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
mod transcript;
#[cfg(test)]
mod tests;

/// What every module sees: std/egui/serde basics and the crate's shared items.
mod prelude {
    pub(crate) use std::{fs, path::PathBuf, time::SystemTime};

    pub(crate) use eframe::egui::{
        self, Color32, Context, CursorIcon, FontId, Key, Modifiers, Painter, Pos2, Rect, Sense, Shape,
        Stroke, StrokeKind, Vec2,
    };
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{Value, json};

    pub(crate) use crate::app::{ide::*, portal::*, script::*, terminal::*, tool::*, ui::icons::*, ui::*, *};
    pub(crate) use crate::file::*;
    pub(crate) use crate::render::{replay::*, rough::*, svg::*, *};
    pub(crate) use crate::{binding::*, bridge::*, geometry::*, labels::*, model::*, recognize::*, rope::*, terminal::*};
}

use prelude::*;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => {
            mcp::run();
            return Ok(());
        }
        // ponytail: the Windows build has no console, so these print nothing there; the MCP
        // tools are the way in on Windows until the CLI attaches to the parent console
        Some("list" | "ask" | "check" | "note" | "portal" | "help") => std::process::exit(cli(&args)),
        _ => {}
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_TITLE)
        .with_inner_size([1440.0, 920.0])
        .with_min_inner_size([920.0, 640.0]);
    if cfg!(target_os = "macos") {
        // the canvas runs under the title bar; the toolbar sits beside the window buttons
        viewport = viewport.with_fullsize_content_view(true).with_title_shown(false).with_titlebar_shown(false);
    }
    let native_options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            #[cfg(target_os = "macos")]
            remember_window(cc);
            let mut app = DrawingApp { autosave: autosave_path(), script: Script::from_env(), ..Default::default() };
            app.bridge = Bridge::start(Some(cc.egui_ctx.clone())).ok();
            if let Some(bridge) = &app.bridge {
                bridge.write_endpoint();
            }
            if let Some(prefs) = prefs_path()
                .and_then(|p| fs::read_to_string(p).ok())
                .and_then(|d| serde_json::from_str::<Prefs>(&d).ok())
            {
                app.prefs = prefs;
                app.saved_prefs = prefs;
                app.tool_locked = prefs.tool_locked;
                app.theme = prefs.theme;
            }
            if let Some((scene, stamp)) = app.autosave.as_deref().and_then(read_scene_file) {
                app.scene = scene.clone();
                app.synced = scene;
                app.synced_stamp = stamp;
            }
            Ok(Box::new(app))
        }),
    )
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
