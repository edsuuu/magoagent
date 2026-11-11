//! The terminals' own settings, drawn into any `ui`: the defaults (theme, font size, shell, copy on
//! select, the size of a new one) for Ajustes › Terminal to show, and one terminal's own look
//! (theme, font size, shell) for Editar's Aparência and Detalhes. Which shells the computer has.

use crate::prelude::*;
use std::sync::OnceLock;

/// The shells this computer has, as (name, program or path), the ones a terminal can pick.
pub(crate) fn detected_shells() -> &'static [(String, String)] {
    static SHELLS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    SHELLS.get_or_init(|| {
        if cfg!(windows) {
            let on_path = |exe: &str| std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(exe).is_file()));
            let git_bash = ["ProgramFiles", "ProgramFiles(x86)"]
                .iter()
                .filter_map(|var| std::env::var_os(var))
                .map(|dir| PathBuf::from(dir).join("Git").join("bin").join("bash.exe"))
                .find(|path| path.is_file());
            let mut shells = Vec::new();
            for (name, exe) in [("PowerShell 7", "pwsh.exe"), ("Windows PowerShell", "powershell.exe"), ("Prompt de Comando", "cmd.exe")] {
                if on_path(exe) {
                    shells.push((name.to_string(), exe.trim_end_matches(".exe").to_string()));
                }
            }
            if let Some(path) = git_bash {
                shells.push(("Git Bash".into(), path.to_string_lossy().into_owned()));
            }
            shells
        } else {
            let listed = fs::read_to_string("/etc/shells").unwrap_or_default();
            let mut shells: Vec<(String, String)> = Vec::new();
            for path in listed.lines().map(str::trim).filter(|l| l.starts_with('/') && !l.contains("nologin") && !l.ends_with("/false")) {
                let name = path.rsplit('/').next().unwrap_or(path).to_string();
                if std::path::Path::new(path).is_file() && !shells.iter().any(|(n, _)| *n == name) {
                    shells.push((name, path.to_string()));
                }
            }
            shells
        }
    })
}

/// What a shell is called: its name if it is one of the detected ones, else what was typed.
fn shell_name(program: &str) -> String {
    match program.trim() {
        "" => "Padrão do sistema".into(),
        program => detected_shells().iter().find(|(_, p)| p == program).map_or_else(|| program.to_string(), |(name, _)| name.clone()),
    }
}

fn theme_name(id: &str) -> &'static str {
    theme_choices().into_iter().find(|(i, _)| *i == id).map_or("Padrão", |(_, name)| name)
}

/// A size in pt: a minus, the number and a plus, between `FONT_SIZES`.
fn font_stepper(ui: &mut egui::Ui, size: &mut f32, c: &UiColors) {
    ui.horizontal(|ui| {
        let step = |ui: &mut egui::Ui, label: &str, delta: f32, size: &mut f32| {
            if ui
                .add(egui::Button::new(egui::RichText::new(label).size(14.0).color(c.text)).fill(c.surface_low).min_size(Vec2::splat(24.0)))
                .on_hover_cursor(CursorIcon::PointingHand)
                .clicked()
            {
                *size = (*size + delta).clamp(*FONT_SIZES.start(), *FONT_SIZES.end());
            }
        };
        step(ui, "−", -1.0, size);
        ui.add_sized([44.0, 24.0], egui::Label::new(egui::RichText::new(format!("{} pt", size.round())).size(13.0).color(c.text)));
        step(ui, "+", 1.0, size);
    });
}

fn shell_combo(ui: &mut egui::Ui, id: &str, shell: &mut String, inherited: &str, c: &UiColors) {
    let now = if shell.trim().is_empty() { format!("Igual ao padrão ({})", shell_name(inherited)) } else { shell_name(shell) };
    dialog_combo(ui, id, now, DIALOG_WIDTH, c, |ui| {
        ui.selectable_value(shell, String::new(), format!("Igual ao padrão ({})", shell_name(inherited)));
        for (name, program) in detected_shells() {
            ui.selectable_value(shell, program.clone(), name);
        }
    });
}

/// Ajustes › Terminal's own controls: the defaults every terminal starts from. Returns whether
/// anything changed.
pub(crate) fn terminal_prefs_ui(ui: &mut egui::Ui, prefs: &mut TermPrefs, c: &UiColors) -> bool {
    let before = prefs.clone();
    let label = |ui: &mut egui::Ui, text: &str| dialog_caption(ui, text, c);
    label(ui, "Tema");
    dialog_combo(ui, "terminal_default_theme", theme_name(&prefs.theme).into(), DIALOG_WIDTH, c, |ui| {
        for (id, name) in theme_choices() {
            ui.selectable_value(&mut prefs.theme, id.to_string(), name);
        }
    });
    label(ui, "Tamanho da fonte");
    font_stepper(ui, &mut prefs.font_size, c);
    label(ui, "Shell");
    shell_combo(ui, "terminal_default_shell", &mut prefs.shell, "", c);
    ui.add_space(8.0);
    dialog_check(ui, &mut prefs.copy_on_select, "Copiar ao selecionar", c).tip("O que se seleciona num terminal vai para a área de transferência");
    if prefs.size.is_some() {
        ui.add_space(6.0);
        if ui.link("Voltar ao tamanho padrão de um terminal novo (80×24)").clicked() {
            prefs.size = None;
        }
    }
    *prefs != before
}

/// One terminal's own look, over the defaults: its theme, its font size and its shell (the
/// family is always Hack). `inherit` is what it has when it picks nothing.
pub(crate) fn terminal_look_ui(ui: &mut egui::Ui, look: &mut TermLook, prefs: &TermPrefs, c: &UiColors, appearance: bool) {
    let label = |ui: &mut egui::Ui, text: &str| dialog_caption(ui, text, c);
    if appearance {
        label(ui, "Tema");
        let now = look.theme.as_deref().map_or_else(|| format!("Igual ao padrão ({})", theme_name(&prefs.theme)), |id| theme_name(id).to_string());
        dialog_combo(ui, "terminal_theme", now, DIALOG_WIDTH, c, |ui| {
            ui.selectable_value(&mut look.theme, None, format!("Igual ao padrão ({})", theme_name(&prefs.theme)));
            for (id, name) in theme_choices() {
                ui.selectable_value(&mut look.theme, Some(id.to_string()), name);
            }
        });
        label(ui, "Tamanho da fonte");
        ui.horizontal(|ui| {
            let mut size = font_size(look.font_size, prefs.font_size);
            font_stepper(ui, &mut size, c);
            if size != font_size(look.font_size, prefs.font_size) {
                look.font_size = Some(size);
            }
            if look.font_size.is_some() && ui.link("Igual ao padrão").clicked() {
                look.font_size = None;
            }
        });
        ui.add_space(4.0);
        dialog_note(ui, "A fonte é sempre a Hack: só o tamanho muda.", c);
    } else {
        label(ui, "Shell");
        shell_combo(ui, "terminal_shell", &mut look.shell, &prefs.shell, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_computers_shells_are_named_and_a_typed_one_keeps_its_name() {
        assert_eq!(shell_name(""), "Padrão do sistema");
        assert_eq!(shell_name("my-shell"), "my-shell");
        if let Some((name, program)) = detected_shells().first() {
            assert_eq!(&shell_name(program), name);
        }
        assert!(!detected_shells().is_empty(), "a computer has a shell");
        assert_eq!(theme_name("nord"), "Nord");
        assert_eq!(theme_name("nope"), "Padrão");
    }

    #[test]
    fn the_controls_draw_and_change_nothing_by_themselves() {
        let ctx = Context::default();
        let app = DrawingApp::default();
        let c = app.colors();
        let (mut prefs, mut look) = (TermPrefs::default(), TermLook::default());
        ctx.run_ui(Default::default(), |ui| {
            assert!(!terminal_prefs_ui(ui, &mut prefs, &c));
            terminal_look_ui(ui, &mut look, &TermPrefs::default(), &c, true);
            terminal_look_ui(ui, &mut look, &TermPrefs::default(), &c, false);
        })
        .textures_delta
        .clear();
        assert!(look.is_default() && prefs == TermPrefs::default());
    }
}
