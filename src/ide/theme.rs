//! "Islands Dark" — values read from the PhpStorm 2026.2.1 install:
//!   lib/intellij.platform.ide.impl.jar!/themes/islands/ManyIslandsDark.theme.json  (UI)
//!   lib/intellij.platform.ide.impl.jar!/themes/islands/IslandSchemeDark.xml        (editor)
//! Token names in comments are the PhpStorm keys.

//! Applied to the IDE's own `Ui` (and its popups) only: the canvas keeps its own look and fonts.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style, TextStyle,
};
use std::{path::PathBuf, sync::Arc};

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
const fn white(a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const(255, 255, 255, a)
}

// UI
pub(crate) const FRAME: Color32 = hex(0x26282C); // main-window-bg (layer-1, gray-30)
pub(crate) const ISLAND: Color32 = hex(0x191A1C); // editor-bg / tool-window-bg (layer-0, gray-10)
pub(crate) const INLINE: Color32 = hex(0x212326); // layer-0-bg-inline (gray-20)
pub(crate) const POPUP: Color32 = hex(0x26282C); // popup-bg
pub(crate) const POPUP_BORDER: Color32 = hex(0x33353B); // popup-border (gray-40)
pub(crate) const CONTROL_BORDER: Color32 = hex(0x40434A); // control-border (gray-50)
pub(crate) const TEXT: Color32 = hex(0xD1D3D9); // text-default (gray-130)
pub(crate) const MUTED: Color32 = hex(0x9FA2A8); // text-muted (gray-100)
pub(crate) const SECONDARY: Color32 = hex(0x73767C); // text-secondary (gray-80)
pub(crate) const HOVER: Color32 = white(0x17); // toolbar-bg-hovered
pub(crate) const PRESSED: Color32 = white(0x29); // toolbar-bg-pressed
pub(crate) const ROW_HOVER: Color32 = white(0x10); // selection-bg-hovered
pub(crate) const SELECTION: Color32 = hex(0x2A4371); // selection-bg-active (blue-50)
pub(crate) const SELECTION_INACTIVE: Color32 = hex(0x33353B); // selection-bg-inactive (gray-40)
pub(crate) const SEARCH_MATCH: Color32 = Color32::from_rgba_unmultiplied_const(0xBA, 0x97, 0x52, 0x80); // search-match-bg
pub(crate) const LINK: Color32 = hex(0x71A1FE); // text-link (blue-100)
pub(crate) const TAB_SELECTED: Color32 = hex(0x233558); // tab-selected-bg-active (blue-40)
pub(crate) const TAB_BORDER: Color32 = hex(0x2E4D89); // tab-selected-border-active (blue-60)
pub(crate) const ACCENT: Color32 = hex(0x3871E1); // accent-brand-bg (blue-80)
pub(crate) const ISLAND_RADIUS: u8 = 10; // Island.arc = 20 (diameter)
pub(crate) const ISLAND_GAP: i8 = 3; // Island.borderWidth = 6, split between neighbours

// Editor scheme
pub(crate) const EDITOR_TEXT: Color32 = hex(0xBCBEC4); // TEXT
pub(crate) const CARET: Color32 = hex(0xCED0D6); // CARET_COLOR
pub(crate) const CARET_ROW: Color32 = hex(0x1F2024); // CARET_ROW_COLOR
pub(crate) const LINE_NUMBER: Color32 = hex(0x4B5059); // LINE_NUMBERS_COLOR
pub(crate) const LINE_NUMBER_CARET: Color32 = hex(0xA1A3AB); // LINE_NUMBER_ON_CARET_ROW_COLOR
pub(crate) const EDITOR_SELECTION: Color32 = hex(0x214283); // SELECTION_BACKGROUND (Darcula parent)
pub(crate) const KEYWORD: Color32 = hex(0xCF8E6D); // DEFAULT_KEYWORD
pub(crate) const STRING: Color32 = hex(0x6AAB73); // DEFAULT_STRING
pub(crate) const NUMBER: Color32 = hex(0x2AACB8); // DEFAULT_NUMBER
pub(crate) const COMMENT: Color32 = hex(0x7A7E85); // DEFAULT_LINE_COMMENT
pub(crate) const DOC_COMMENT: Color32 = hex(0x5F826B); // DEFAULT_DOC_COMMENT (italic)
pub(crate) const FUNCTION: Color32 = hex(0x56A8F5); // DEFAULT_FUNCTION_DECLARATION
pub(crate) const METHOD: Color32 = hex(0x57AAF7); // DEFAULT_INSTANCE_METHOD
pub(crate) const FIELD: Color32 = hex(0xC77DBB); // DEFAULT_INSTANCE_FIELD / DEFAULT_CONSTANT
pub(crate) const METADATA: Color32 = hex(0xB3AE60); // DEFAULT_METADATA
pub(crate) const BRACKETS: [Color32; 3] = [hex(0xE8BA36), hex(0xD16D9E), hex(0x179FFF)]; // Rainbow Brackets, by depth
pub(crate) const INDENT_GUIDE: Color32 = hex(0x393B40); // indent guide (PhpStorm Islands Dark)
pub(crate) const INDENT_GUIDE_SELECTED: Color32 = hex(0x4E5157); // the guide of the caret's block
pub(crate) const TAG: Color32 = hex(0xD5B778); // HTML_TAG_NAME
pub(crate) const ERROR: Color32 = hex(0xFA6675); // ERRORS_ATTRIBUTES effect (wave)
pub(crate) const WARNING: Color32 = hex(0xF2C55C); // WARNING_ATTRIBUTES effect (wave)

pub(crate) const UI_SIZE: f32 = 13.0; // New UI default font size
pub(crate) const EDITOR_SIZE: f32 = 13.0; // editor default font size

/// Inter (or the app's UI font when no JetBrains install has it).
pub(crate) fn ui(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("ide".into()))
}

/// JetBrains Mono (or egui's monospace).
pub(crate) fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("ide-mono".into()))
}

/// Islands Dark on a `Ui`'s style: text styles on the IDE fonts, spacing and visuals.
pub(crate) fn style(s: &mut Style) {
    s.text_styles = [
        (TextStyle::Small, ui(UI_SIZE - 2.0)),
        (TextStyle::Body, ui(UI_SIZE)),
        (TextStyle::Button, ui(UI_SIZE)),
        (TextStyle::Heading, ui(UI_SIZE + 2.0)),
        (TextStyle::Monospace, mono(EDITOR_SIZE)),
    ]
    .into();
    s.spacing.item_spacing = egui::vec2(6.0, 2.0);
    s.spacing.button_padding = egui::vec2(6.0, 3.0);
    s.spacing.interact_size.y = 24.0;

    let v = &mut s.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = FRAME;
    v.window_fill = POPUP;
    v.window_stroke = Stroke::new(1.0, POPUP_BORDER);
    v.window_corner_radius = CornerRadius::same(12); // Popup arc
    v.menu_corner_radius = CornerRadius::same(8);
    v.extreme_bg_color = INLINE;
    v.faint_bg_color = INLINE;
    v.code_bg_color = INLINE;
    v.hyperlink_color = LINK;
    v.error_fg_color = ERROR;
    v.warn_fg_color = WARNING;
    v.selection.bg_fill = EDITOR_SELECTION;
    v.selection.stroke = Stroke::new(1.0, TEXT);
    v.text_cursor.stroke = Stroke::new(2.0, CARET);

    let r = CornerRadius::same(6); // Component.arc.compact
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = ISLAND;
    w.noninteractive.bg_stroke = Stroke::new(1.0, POPUP_BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    for (wv, fill) in [
        (&mut w.inactive, Color32::TRANSPARENT),
        (&mut w.hovered, HOVER),
        (&mut w.active, PRESSED),
        (&mut w.open, PRESSED),
    ] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::NONE;
        wv.fg_stroke = Stroke::new(1.0, TEXT);
        wv.corner_radius = r;
        wv.expansion = 0.0;
    }
    w.inactive.fg_stroke = Stroke::new(1.0, MUTED);
    // buttons and checkboxes get the New UI control border (menus hide button frames anyway)
    w.inactive.bg_stroke = Stroke::new(1.0, CONTROL_BORDER);
    w.hovered.bg_stroke = Stroke::new(1.0, CONTROL_BORDER);
}

/// `Popup::menu` in the IDE's look, submenus included (popups start from the app's style).
pub(crate) fn menu(response: &egui::Response) -> egui::Popup<'_> {
    let look = |s: &mut Style| {
        style(s);
        egui::containers::menu::menu_style(s);
    };
    let config = egui::containers::menu::MenuConfig::new().style(look);
    egui::Popup::menu(response)
        .style(look)
        .info(egui::UiStackInfo::new(egui::UiKind::Menu).with_tag_value(egui::containers::menu::MenuConfig::MENU_CONFIG_TAG, config))
}

/// PhpStorm uses Inter for the UI and JetBrains Mono for the editor. Both ship inside every
/// JetBrains install (jbr/lib/fonts). They become the "ide" and "ide-mono" families, in front
/// of the app's own proportional and monospace fonts (all there is without a JetBrains install).
pub(crate) fn add_fonts(fonts: &mut FontDefinitions) {
    let mut dirs: Vec<PathBuf> = vec![];
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let roots = [PathBuf::from("C:/Program Files/JetBrains"), PathBuf::from("/opt/JetBrains"), PathBuf::from("/Applications")];
    // Toolbox installs into ~/Applications on macOS
    for root in roots.into_iter().chain(home.map(|h| h.join("Applications"))) {
        if let Ok(rd) = std::fs::read_dir(root) {
            dirs.extend(rd.flatten().flat_map(|e| [e.path().join("jbr/lib/fonts"), e.path().join("Contents/jbr/Contents/Home/lib/fonts")]));
        }
    }
    let find = |name: &str| dirs.iter().find_map(|d| std::fs::read(d.join(name)).ok());
    for (key, file, family, base) in [
        ("inter", "Inter-Regular.otf", "ide", FontFamily::Proportional),
        ("jbmono", "JetBrainsMono-Regular.ttf", "ide-mono", FontFamily::Monospace),
    ] {
        let mut list = fonts.families.get(&base).cloned().unwrap_or_default();
        if let Some(bytes) = find(file) {
            fonts.font_data.insert(key.into(), Arc::new(FontData::from_owned(bytes)));
            list.insert(0, key.into());
        }
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }
}
