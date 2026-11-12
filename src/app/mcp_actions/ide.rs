use super::*;

impl DrawingApp {
    /// `diff` is the base and the mode of the action diff.
    pub(super) fn act_ide_git(&mut self, name: &str, action: &str, message: &str, files: &[String], diff: (String, String)) -> Result<String, String> {
        let (index, id) = self.ide_named(name)?;
        self.show_ide(index);
        let ide = self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?;
        match action {
            "panel" => ide.run(crate::ide::Cmd::Commit),
            "commit" if message.trim().is_empty() => return Err("diga a mensagem do commit".into()),
            "commit" => return ide.agent_commit(message.trim(), files),
            "push" => ide.run(crate::ide::Cmd::Push),
            "pull" => ide.run(crate::ide::Cmd::Pull),
            "diff" => return ide.agent_diff(&diff.0, &diff.1),
            other => return Err(format!("ação desconhecida: {other}")),
        }
        Ok(format!("{action} começou no IDE; o resultado aparece nele."))
    }

    /// The `ide_git` actions of branches, merge, stash and history (ide/branches.rs).
    pub(super) fn act_ide_git_more(&mut self, name: &str, args: &Value) -> Result<String, String> {
        let (index, id) = self.ide_named(name)?;
        self.show_ide(index);
        self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?.agent_git(args)
    }

    pub(super) fn act_ide_conflict(&mut self, name: &str, path: &str, action: &str) -> Result<String, String> {
        let (index, id) = self.ide_named(name)?;
        self.show_ide(index);
        self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?.resolve_conflict(path, action)
    }

    /// "Comentar para o Maestro" by the MCP: lines of a file of the IDE (or the first file tree) with a note, to the Maestro's chat.
    pub(super) fn act_maestro_comment(&mut self, name: &str, path: &str, from: usize, to: usize, comment: &str) -> Result<String, String> {
        if comment.trim().is_empty() {
            return Err("diga o comentário".into());
        }
        let (source, root) = match self.ide_named(name) {
            Ok((i, id)) => (id, self.scene.elements[i].ide.as_ref().map(|s| s.root.clone()).unwrap_or_default()),
            Err(_) => self
                .scene
                .elements
                .iter()
                .find_map(|e| e.filetree.as_ref().map(|f| (e.id.clone(), f.root.clone())))
                .ok_or("Não há IDE nem Árvore de Arquivos no canvas.")?,
        };
        let full = if Path::new(path).is_absolute() { PathBuf::from(path) } else { Path::new(&root).join(path) };
        let text = std::fs::read_to_string(&full).map_err(|e| e.to_string())?;
        let lines: Vec<&str> = text.lines().collect();
        if from == 0 || to < from || to > lines.len() {
            return Err("intervalo de linhas inválido".into());
        }
        let lang = crate::ide::highlight::detect(&full).id;
        let prompt = crate::app::chat::code_comment::code_comment_prompt(
            &crate::app::chat::code_comment::relative(Path::new(&root), &full),
            from,
            to,
            lang,
            &lines[from - 1..to].join("\n"),
            comment,
        );
        self.queue_to_maestro(&source, &root, prompt)?;
        Ok("Comentário enviado ao Maestro.".into())
    }

    pub(super) fn act_code_comment(&mut self, name: &str, path: &str, from: usize, to: usize, comment: &str) -> Result<String, String> {
        if comment.trim().is_empty() {
            return Err("diga o comentário".into());
        }
        let (index, id) = self.ide_named(name)?;
        self.show_ide(index);
        let ide = self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?;
        let full = Path::new(&ide.folder).join(path);
        let text = std::fs::read_to_string(&full).map_err(|e| e.to_string())?;
        let reference = ide.code_reference(&full, &text, from, to, comment, "editor", "local")?;
        if !ide.has_agent {
            return Err("Ligue um agente a este IDE para comentar.".into());
        }
        ide.to_agent.push(crate::ide::coderef::message(&reference));
        Ok("Comentário enviado ao agente ligado ao IDE.".into())
    }
}
