//! New UI icons, embedded (see assets/ide-icons/NOTICE), rasterized with resvg into textures
//! cached per size in the context.

use super::{highlight, theme as t};
use eframe::egui::{self, Color32, Painter, Rect, Stroke, Ui, vec2};
use std::path::Path;

/// File name (the cache key) and SVG source.
pub(crate) type Svg = (&'static str, &'static str);

macro_rules! icons {
    ($($name:ident = $file:literal;)*) => {
        $(pub(crate) const $name: Svg = ($file, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/ide-icons/", $file, ".svg")));)*
    };
}

icons! {
    PROJECT = "toolwindows_project";
    PROBLEMS = "toolwindows_problems";
    COMMIT = "toolwindows_commit";
    CLAUDE = "ilha_claude";
    CLAUDE_MARK = "claude";
    OPENAI = "openai";
    BRANCH = "ilha_branch";
    MENU = "general_menu";
    CHEVRON_DOWN = "general_chevronDown";
    CHEVRON_RIGHT = "general_chevronRight";
    CHEVRON_UP = "general_chevronUp";
    SEARCH = "general_search";
    CLOSE_SMALL = "general_closeSmall";
    CLOSE_SMALL_HOVER = "general_closeSmallHovered";
    REFRESH = "general_refresh";
    FOLDER = "nodes_folder";
    ERROR = "status_error";
    WARNING = "status_warning";
    DIFF = "vcs_diff";
    FT_TEXT = "fileTypes_text";
    FT_JSON = "fileTypes_json";
    FT_XML = "fileTypes_xml";
    FT_HTML = "fileTypes_html";
    FT_CSS = "fileTypes_css";
    FT_JS = "fileTypes_javaScript";
    FT_TS = "fileTypes_typeScript";
    FT_YAML = "fileTypes_yaml";
    FT_TOML = "fileTypes_toml";
    FT_SQL = "fileTypes_sql";
    FT_SHELL = "fileTypes_shell";
    FT_MD = "fileTypes_markdown";
    FT_C = "fileTypes_c";
    FT_CPP = "fileTypes_cpp";
    FT_H = "fileTypes_h";
    FT_CS = "fileTypes_Csharp";
    FT_IMAGE = "fileTypes_image";
    FT_ARCHIVE = "fileTypes_archive";
    FT_CONFIG = "fileTypes_config";
    FT_DOCKER = "fileTypes_docker";
    FT_CSV = "fileTypes_csv";
    FT_PROPERTIES = "fileTypes_properties";
    FT_EDITORCONFIG = "fileTypes_editorConfig";
    FT_IGNORED = "fileTypes_ignored";
    FT_WINDOWS = "fileTypes_microsoftWindows";
    FT_UNKNOWN = "fileTypes_unknown";
}

/// The icon rasterized for `px` pixels, made once per size.
fn texture(ctx: &egui::Context, (name, svg): Svg, px: u32) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique(("ide_icon", name, px));
    if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(tex);
    }
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let image = egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pixmap.data());
    let tex = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

pub(crate) fn paint(p: &Painter, src: Svg, rect: Rect) {
    paint_tint(p, src, rect, Color32::WHITE);
}

/// `paint` with the icon's pixels multiplied by `tint`: a white mark in the current color.
pub(crate) fn paint_tint(p: &Painter, src: Svg, rect: Rect, tint: Color32) {
    let px = (rect.width() * p.pixels_per_point()).round().max(1.0) as u32;
    if let Some(tex) = texture(p.ctx(), src, px) {
        p.image(tex.id(), rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
    }
}

/// 16px icon centred at `c`.
pub(crate) fn at(p: &Painter, src: Svg, c: egui::Pos2) {
    paint(p, src, Rect::from_center_size(c, vec2(16.0, 16.0)));
}

/// Icon + response for a toolbar-sized button.
pub(crate) fn button(ui: &mut Ui, src: Svg, tip: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), egui::Sense::click());
    let bg = if selected || resp.is_pointer_button_down_on() {
        t::PRESSED
    } else if resp.hovered() {
        t::HOVER
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 6, bg);
    paint(ui.painter(), src, Rect::from_center_size(rect.center(), vec2(18.0, 18.0)));
    resp.on_hover_text(tip)
}

fn file_icon(path: &Path) -> Option<Svg> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    if name == "dockerfile" || name.starts_with("docker-compose") || name.ends_with(".dockerfile") {
        return Some(FT_DOCKER);
    }
    if name.starts_with(".env") {
        return Some(FT_CONFIG);
    }
    if matches!(name.as_str(), ".gitignore" | ".gitattributes" | ".dockerignore" | ".npmignore" | ".prettierignore" | ".eslintignore") {
        return Some(FT_IGNORED);
    }
    Some(match ext {
        "md" | "markdown" | "mdx" => FT_MD,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" | "bmp" => FT_IMAGE,
        "zip" | "gz" | "tar" | "7z" | "rar" | "jar" | "phar" | "tgz" => FT_ARCHIVE,
        "csv" | "tsv" => FT_CSV,
        "ini" | "conf" | "cfg" | "neon" => FT_CONFIG,
        "properties" => FT_PROPERTIES,
        "editorconfig" => FT_EDITORCONFIG,
        "exe" | "dll" | "bat" | "cmd" | "ps1" | "msi" => FT_WINDOWS,
        "txt" | "log" | "lock" if name != "composer.lock" && name != "package-lock.json" => FT_TEXT,
        "h" => FT_H,
        _ => match highlight::detect(path).id {
            "html" => FT_HTML,
            "xml" => FT_XML,
            "css" => FT_CSS,
            "javascript" | "javascriptreact" => FT_JS,
            "typescript" | "typescriptreact" => FT_TS,
            "json" => FT_JSON,
            "yaml" => FT_YAML,
            "toml" => FT_TOML,
            "sql" => FT_SQL,
            "shellscript" => FT_SHELL,
            "c" => FT_C,
            "cpp" => FT_CPP,
            "csharp" => FT_CS,
            _ => return None,
        },
    })
}

/// Languages whose JetBrains icons are proprietary get a lookalike badge in the same style.
fn badge(path: &Path) -> Option<(u32, &'static str)> {
    Some(match highlight::detect(path).id {
        "php" => (0xB589EC, "P"),
        "blade" => (0xF05340, "B"),
        "rust" => (0xE0956D, "R"),
        "go" => (0x4DB6D0, "Go"),
        "python" => (0x6CA6E0, "Py"),
        "asm" => (0xA1A3AB, "As"),
        _ => return None,
    })
}

/// A file's 16px icon centred at `c`, scaled by `zoom` (1 inside the IDE).
pub(crate) fn file(p: &Painter, path: &Path, c: egui::Pos2, zoom: f32) {
    if let Some(src) = file_icon(path) {
        return paint(p, src, Rect::from_center_size(c, vec2(16.0, 16.0) * zoom));
    }
    let Some((rgb, label)) = badge(path) else { return paint(p, FT_UNKNOWN, Rect::from_center_size(c, vec2(16.0, 16.0) * zoom)) };
    let color = Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    let r = Rect::from_center_size(c, vec2(13.0, 13.0) * zoom);
    p.rect(r, 3, color.gamma_multiply(0.18), Stroke::new(1.0, color), egui::StrokeKind::Inside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, label, t::ui(if label.len() > 1 { 7.0 } else { 9.0 } * zoom), color);
}
