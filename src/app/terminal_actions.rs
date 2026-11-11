//! The terminal card's options as the MCP's actions (what the right-click menu, the search bar,
//! the processes button and Editar's look do for a person): `terminal_menu`, `terminal_find`,
//! `terminal_look` and `terminal_processes`. Each finds its terminal by name, as `list_agents` shows it.

use crate::app::terminal_prefs::detected_shells;
use crate::prelude::*;

/// "on", "off", true or false in a tool's `value`; none when it says nothing.
fn switch(args: &Value) -> Option<bool> {
    match &args["value"] {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "on" | "ligar" | "sim" | "true" => Some(true),
            "off" | "desligar" | "não" | "nao" | "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

impl DrawingApp {
    /// The terminal called `name`: its place in the scene.
    fn terminal_named(&self, name: &str) -> Result<usize, String> {
        let i = self.node_named(name)?;
        if self.scene.elements[i].terminal.is_none() {
            return Err(format!("{name} não é um terminal"));
        }
        Ok(i)
    }

    fn live_session(&self, id: &str, name: &str) -> Result<&TerminalSession, String> {
        match self.terminals.get(id) {
            Some(Ok(session)) => Ok(session),
            _ => Err(format!("{name} não está rodando (descarregado, encerrado ou ainda abrindo)")),
        }
    }

    /// The MCP's `terminal_menu`: a row of the terminal card's menu, by `action`.
    pub(crate) fn act_terminal_menu(&mut self, args: &Value) -> Result<String, String> {
        let name = args["name"].as_str().unwrap_or_default().trim().to_string();
        let action = args["action"].as_str().unwrap_or_default();
        let i = self.terminal_named(&name)?;
        let id = self.scene.elements[i].id.clone();
        let value = args["value"].as_str().unwrap_or_default().trim().to_string();
        match action {
            "copy" => {
                let text = self.live_session(&id, &name)?.handle.selected_text();
                if text.is_empty() {
                    return Err(format!("Nada está selecionado em {name}."));
                }
                self.term_ui.copy_out = Some(text.clone());
                return Ok(text);
            }
            "paste" => {
                let text = args["text"].as_str().unwrap_or_default();
                if text.is_empty() {
                    return Err("diga o `text` a colar".into());
                }
                let handle = &self.live_session(&id, &name)?.handle;
                handle.write(paste_bytes(text, handle.mode().contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE)));
                return Ok(format!("Texto colado em {name}."));
            }
            "select_all" => self.live_session(&id, &name)?.handle.select_all(),
            "clear_selection" => self.live_session(&id, &name)?.handle.clear_selection(),
            "rename" if value.is_empty() => return Err("diga o nome novo em `value`".into()),
            "rename" => {
                self.set_name(&id, &value);
                return Ok(format!("Renomeado para {}.", self.scene.elements[i].terminal.as_ref().map_or("", |t| t.name.as_str())));
            }
            "monitor" => {
                let on = switch(args).unwrap_or(self.scene.elements[i].terminal.as_ref().is_some_and(|t| t.quiet));
                self.snapshot();
                if let Some(t) = self.scene.elements[i].terminal.as_mut() {
                    t.quiet = !on;
                }
                return Ok(format!("Monitorar atividade {} em {name}.", if on { "ligado" } else { "desligado" }));
            }
            "blur" => {
                let on = switch(args).unwrap_or(true);
                self.set_blurred(i, on);
                return Ok(format!("{name} {}.", if on { "desfocado" } else { "sem desfoque" }));
            }
            "reload" => {
                if !self.unload_node(i, false) {
                    self.terminals.remove(&id);
                }
            }
            "unload" => {
                self.unload_node(i, true);
            }
            "use_size" => {
                self.terminal_menu_action(&Context::default(), Some(i), "use_size");
            }
            "use_font" => self.use_font_of(i),
            "use_theme" => self.use_theme_of(i),
            "copy_element" => {
                self.selected = vec![i];
                self.clipboard = self.with_card_text(&self.selected).iter().filter_map(|k| self.scene.elements.get(*k).cloned()).collect();
                self.term_ui.copy_out = Some(json!({ "type": "magoagent/clipboard", "elements": interop_elements(&self.clipboard) }).to_string());
            }
            "duplicate" => {
                self.selected = vec![i];
                self.duplicate();
            }
            "lock" => {
                let on = switch(args).unwrap_or(!self.scene.elements[i].locked);
                if self.scene.elements[i].locked != on {
                    self.selected = vec![i];
                    self.toggle_lock_selected();
                }
                return Ok(format!("{name} {}.", if on { "bloqueado" } else { "desbloqueado" }));
            }
            "delete" => {
                self.selected = vec![i];
                self.delete_selected();
                return Ok(format!("{name} apagado (dá para desfazer)."));
            }
            other => {
                return Err(format!(
                    "ação desconhecida: {other} (copy, paste, select_all, clear_selection, rename, monitor, blur, reload, unload, use_size, use_font, use_theme, copy_element, duplicate, lock, delete)"
                ));
            }
        }
        Ok(format!("{action}: feito em {name}."))
    }

    /// The MCP's `terminal_find`: the search bar of a terminal. `find` (the default) searches
    /// `query` and says how many matches; `next` and `previous` step through them; `close` ends it.
    pub(crate) fn act_terminal_find(&mut self, args: &Value) -> Result<String, String> {
        let name = args["name"].as_str().unwrap_or_default().trim().to_string();
        let i = self.terminal_named(&name)?;
        let id = self.scene.elements[i].id.clone();
        let action = args["action"].as_str().filter(|a| !a.is_empty()).unwrap_or("find");
        if action == "close" {
            self.close_find();
            return Ok("Busca fechada.".into());
        }
        self.live_session(&id, &name)?;
        match action {
            "find" => {
                let query = args["query"].as_str().unwrap_or_default();
                if query.is_empty() {
                    return Err("diga a `query` a buscar".into());
                }
                self.open_find(&id, None);
                if let Some(f) = &mut self.term_ui.find {
                    (f.query, f.regex) = (query.to_string(), args["regex"].as_bool().unwrap_or(false));
                }
            }
            "next" | "previous" if self.term_ui.find.as_ref().is_none_or(|f| f.id != id) => return Err("não há busca aberta neste terminal; use find".into()),
            "next" => self.find_step(true),
            "previous" => self.find_step(false),
            other => return Err(format!("ação desconhecida: {other} (find, next, previous, close)")),
        }
        self.refresh_find();
        let Some(f) = &self.term_ui.find else { return Ok("Busca fechada.".into()) };
        if f.found.invalid {
            return Err(format!("`{}` não é uma regex válida", f.query));
        }
        Ok(match f.found.matches.len() {
            0 => format!("Nenhum resultado para \"{}\" em {name}.", f.query),
            n => format!("{} em {name}: {n} resultado{} (\"{}\").", f.summary(), if n == 1 { "" } else { "s" }, f.query),
        })
    }

    /// The MCP's `terminal_look`: one terminal's theme, font size (pt) and shell over the defaults
    /// (`name`), or the defaults themselves (no `name`; also `copy_on_select`). `reset` gives a
    /// terminal back the defaults; `list` tells the themes and the shells there are.
    pub(crate) fn act_terminal_look(&mut self, args: &Value) -> Result<String, String> {
        if args["action"] == "list" {
            let themes =
                theme_choices().iter().map(|(id, name)| format!("{} ({name})", if id.is_empty() { "padrão" } else { id })).collect::<Vec<_>>().join(", ");
            let shells = detected_shells().iter().map(|(name, program)| format!("{name} = {program}")).collect::<Vec<_>>().join(", ");
            return Ok(format!(
                "Temas: {themes}, ou `system` para seguir o tema do app. Tamanhos de fonte: {} a {} pt (a fonte é sempre a Hack). Shells: {shells}.",
                FONT_SIZES.start(),
                FONT_SIZES.end()
            ));
        }
        let known = |theme: &str| theme_choices().iter().map(|(id, _)| *id).any(|id| id == theme) || theme.eq_ignore_ascii_case("padrão");
        let theme = args["theme"].as_str().map(|t| t.trim().to_lowercase());
        if let Some(t) = theme.as_deref().filter(|t| !t.is_empty() && !known(t) && *t != "inherit") {
            return Err(format!("tema desconhecido: {t} (veja action list)"));
        }
        let theme = theme.map(|t| if t == "padrão" { String::new() } else { t });
        let size = args["font_size"].as_f64().map(|s| s as f32);
        let shell = args["shell"].as_str().map(|s| s.trim().to_string());
        let name = args["name"].as_str().unwrap_or_default().trim().to_string();
        if name.is_empty() {
            let p = &mut self.prefs.terminal;
            if let Some(t) = theme {
                p.theme = if t == "inherit" { String::new() } else { t };
            }
            if let Some(s) = size {
                p.font_size = font_size(Some(s), p.font_size);
            }
            if let Some(s) = shell {
                p.shell = s;
            }
            if let Some(c) = args["copy_on_select"].as_bool() {
                p.copy_on_select = c;
            }
            let p = &self.prefs.terminal;
            return Ok(format!(
                "Padrões dos terminais: tema {}, fonte {} pt, shell {}, copiar ao selecionar {}.",
                if p.theme.is_empty() { "padrão" } else { &p.theme },
                p.font_size,
                if p.shell.is_empty() { "do sistema" } else { &p.shell },
                if p.copy_on_select { "ligado" } else { "desligado" }
            ));
        }
        let i = self.terminal_named(&name)?;
        let id = self.scene.elements[i].id.clone();
        self.snapshot();
        let restart = {
            let look = &mut self.scene.elements[i].terminal.as_mut().ok_or("não é um terminal")?.look;
            let before = look.shell.clone();
            if args["reset"].as_bool() == Some(true) {
                *look = TermLook { blurred: look.blurred, ..Default::default() };
            }
            if let Some(t) = theme {
                look.theme = (t != "inherit").then_some(t);
            }
            if let Some(s) = size {
                look.font_size = Some(font_size(Some(s), TERMINAL_FONT));
            }
            if let Some(s) = shell {
                look.shell = s;
            }
            look.shell != before
        };
        // a terminal in another shell starts again: the one it runs in is the one it started with
        if restart {
            self.terminals.remove(&id);
        }
        let spec = self.scene.elements[i].terminal.clone().unwrap_or_default();
        let (theme, font, shell) = self.prefs.terminal.resolve(&spec.look);
        Ok(format!(
            "{name}: tema {}, fonte {font} pt, shell {}.",
            if theme.is_empty() { "padrão" } else { theme },
            if shell.is_empty() { "do sistema" } else { shell }
        ))
    }

    /// The MCP's `terminal_processes`: what runs under a terminal's shell, and `stop_pid` ends one.
    pub(crate) fn act_terminal_processes(&mut self, args: &Value) -> Result<String, String> {
        let name = args["name"].as_str().unwrap_or_default().trim().to_string();
        let i = self.terminal_named(&name)?;
        let id = self.scene.elements[i].id.clone();
        if let Some(pid) = args["stop_pid"].as_u64() {
            self.stop_terminal_process(&id, pid as u32)?;
            return Ok(format!("Processo {pid} de {name} parado."));
        }
        let rows = self.terminal_processes(&id);
        if rows.is_empty() {
            return Ok(format!("{name} não tem processos filhos."));
        }
        let mut out = format!("{name}: {} processo{}", rows.len(), if rows.len() == 1 { "" } else { "s" });
        for p in rows {
            out += &format!("\n- {} · PID {} · {}", p.name, p.pid, crate::app::terminal_procs::memory_text(p.bytes));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_terminal(name: &str) -> DrawingApp {
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 400.0 });
        e.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
        app.scene.elements.push(e);
        app
    }

    fn running(app: &mut DrawingApp, line: &str) {
        let id = app.scene.elements[0].id.clone();
        let args: Vec<&str> = if cfg!(windows) { vec!["/c", line] } else { vec!["-c", line] };
        let session = TerminalSession::spawn("", "", (60, 6), None, vec![], Some((if cfg!(windows) { "cmd" } else { "sh" }, &args)), 100).unwrap();
        app.terminals.insert(id, Ok(session));
    }

    fn wait_for(app: &DrawingApp, text: &str) {
        for _ in 0..100 {
            if matches!(app.terminals.values().next(), Some(Ok(s)) if s.handle.screen_text().contains(text)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("{text} never showed");
    }

    #[test]
    fn the_menu_through_the_mcp_selects_copies_blurs_locks_and_duplicates() {
        let mut app = app_with_terminal("Um");
        running(&mut app, "echo mcp-menu-test");
        wait_for(&app, "mcp-menu-test");
        let menu = |app: &mut DrawingApp, action: &str, extra: Value| {
            let mut args = json!({ "name": "Um", "action": action });
            args.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            app.act("terminal_menu", &args)
        };
        assert!(menu(&mut app, "copy", json!({})).is_err(), "nothing selected yet");
        menu(&mut app, "select_all", json!({})).unwrap();
        assert!(menu(&mut app, "copy", json!({})).unwrap().contains("mcp-menu-test"));
        assert!(app.term_ui.copy_out.as_deref().is_some_and(|t| t.contains("mcp-menu-test")), "queued for the clipboard");
        menu(&mut app, "clear_selection", json!({})).unwrap();
        assert!(menu(&mut app, "copy", json!({})).is_err());
        assert!(menu(&mut app, "paste", json!({})).is_err(), "a paste needs its text");
        menu(&mut app, "paste", json!({ "text": "x" })).unwrap();
        menu(&mut app, "blur", json!({})).unwrap();
        assert!(app.scene.elements[0].terminal.as_ref().unwrap().look.blurred);
        menu(&mut app, "blur", json!({ "value": "off" })).unwrap();
        assert!(!app.scene.elements[0].terminal.as_ref().unwrap().look.blurred);
        menu(&mut app, "monitor", json!({ "value": "off" })).unwrap();
        assert!(app.scene.elements[0].terminal.as_ref().unwrap().quiet);
        menu(&mut app, "lock", json!({})).unwrap();
        assert!(app.scene.elements[0].locked);
        assert!(menu(&mut app, "lock", json!({ "value": "off" })).unwrap().contains("desbloqueado"));
        menu(&mut app, "rename", json!({ "value": "Dois" })).unwrap();
        assert_eq!(app.scene.elements[0].terminal.as_ref().unwrap().name, "Dois");
        menu(&mut app, "copy_element", json!({ "name": "Dois" })).unwrap();
        assert!(app.term_ui.copy_out.as_deref().is_some_and(|t| t.contains("magoagent/clipboard")));
        menu(&mut app, "duplicate", json!({ "name": "Dois" })).unwrap();
        assert_eq!(app.scene.elements.len(), 2);
        menu(&mut app, "unload", json!({ "name": "Dois" })).unwrap();
        assert!(app.scene.elements[0].terminal.as_ref().unwrap().unloaded);
        menu(&mut app, "reload", json!({ "name": "Dois" })).unwrap();
        assert!(!app.scene.elements[0].terminal.as_ref().unwrap().unloaded);
        assert!(menu(&mut app, "nope", json!({ "name": "Dois" })).unwrap_err().contains("ação desconhecida"));
        assert!(app.act("terminal_menu", &json!({ "name": "Ninguém", "action": "copy" })).is_err());
        menu(&mut app, "delete", json!({ "name": "Dois" })).unwrap();
    }

    #[test]
    fn the_search_through_the_mcp_counts_steps_and_closes() {
        let mut app = app_with_terminal("Um");
        running(&mut app, "echo needle hay needle");
        wait_for(&app, "needle hay needle");
        let find = |app: &mut DrawingApp, args: Value| app.act("terminal_find", &args);
        assert!(find(&mut app, json!({ "name": "Um", "query": "needle" })).unwrap().contains("2 resultados"));
        assert!(find(&mut app, json!({ "name": "Um", "action": "next" })).unwrap().starts_with("1 de 2"));
        assert!(find(&mut app, json!({ "name": "Um", "action": "previous" })).unwrap().starts_with("2 de 2"));
        assert!(find(&mut app, json!({ "name": "Um", "query": "ne+dle", "regex": true })).unwrap().contains("2 resultados"));
        assert!(find(&mut app, json!({ "name": "Um", "query": "(", "regex": true })).unwrap_err().contains("regex válida"));
        assert!(find(&mut app, json!({ "name": "Um", "query": "absent" })).unwrap().contains("Nenhum resultado"));
        assert!(find(&mut app, json!({ "name": "Um" })).is_err(), "a search needs its query");
        find(&mut app, json!({ "name": "Um", "action": "close" })).unwrap();
        assert!(app.term_ui.find.is_none());
        assert!(find(&mut app, json!({ "name": "Um", "action": "next" })).is_err());
    }

    #[test]
    fn the_look_through_the_mcp_sets_a_terminal_and_the_defaults() {
        let mut app = app_with_terminal("Um");
        let look = |app: &mut DrawingApp, args: Value| app.act("terminal_look", &args);
        assert!(look(&mut app, json!({ "action": "list" })).unwrap().contains("nord"));
        let said = look(&mut app, json!({ "name": "Um", "theme": "dracula", "font_size": 20 })).unwrap();
        assert!(said.contains("dracula") && said.contains("20 pt"), "{said}");
        assert_eq!(app.scene.elements[0].terminal.as_ref().unwrap().look.font_size, Some(20.0));
        assert!(look(&mut app, json!({ "name": "Um", "theme": "no-such" })).is_err());
        look(&mut app, json!({ "name": "Um", "font_size": 99 })).unwrap();
        assert_eq!(app.scene.elements[0].terminal.as_ref().unwrap().look.font_size, Some(32.0), "kept in range");
        look(&mut app, json!({ "name": "Um", "reset": true })).unwrap();
        assert!(app.scene.elements[0].terminal.as_ref().unwrap().look.is_default());
        // the defaults: what a terminal with nothing of its own has
        look(&mut app, json!({ "theme": "nord", "font_size": 11, "copy_on_select": true })).unwrap();
        assert_eq!((app.prefs.terminal.theme.as_str(), app.prefs.terminal.font_size, app.prefs.terminal.copy_on_select), ("nord", 11.0, true));
        assert!(look(&mut app, json!({ "name": "Um" })).unwrap().contains("nord"));
        // another shell starts the terminal again
        let id = app.scene.elements[0].id.clone();
        running(&mut app, "echo x");
        look(&mut app, json!({ "name": "Um", "shell": "fish" })).unwrap();
        assert!(!app.terminals.contains_key(&id));
        look(&mut app, json!({ "name": "Um", "shell": "" })).unwrap();
    }

    #[test]
    fn the_processes_through_the_mcp_list_and_refuse_strangers() {
        let mut app = app_with_terminal("Um");
        assert!(app.act("terminal_processes", &json!({ "name": "Um" })).unwrap().contains("não tem processos"));
        assert!(app.act("terminal_processes", &json!({ "name": "Um", "stop_pid": std::process::id() })).is_err());
        assert!(app.act("terminal_processes", &json!({ "name": "Ninguém" })).is_err());
    }
}
