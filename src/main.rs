//! MagoAgent: a native whiteboard with a built-in MCP server (`magoagent mcp`).

mod app;
mod binding;
mod file;
mod geometry;
mod labels;
mod mcp;
mod model;
mod recognize;
mod render;
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

    pub(crate) use crate::app::{tool::*, ui::icons::*, ui::*, *};
    pub(crate) use crate::file::{interop::*, storage::*};
    pub(crate) use crate::render::{images::*, replay::*, rough::*, sticky::*, svg::*, *};
    pub(crate) use crate::{binding::*, geometry::*, labels::*, model::*, recognize::*};
}

use prelude::*;

fn main() -> eframe::Result {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        mcp::run();
        return Ok(());
    }
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([920.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            let mut app = DrawingApp { autosave: autosave_path(), ..Default::default() };
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
