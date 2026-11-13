use super::*;

impl DrawingApp {
    /// "Comentar para o Maestro" by the MCP: lines of a file of the file tree with a note, to the Maestro's chat.
    pub(super) fn act_maestro_comment(&mut self, name: &str, path: &str, from: usize, to: usize, comment: &str) -> Result<String, String> {
        if comment.trim().is_empty() {
            return Err("diga o comentário".into());
        }
        // none named and several on the canvas: the first
        let first = || self.scene.elements.iter().find(|e| e.filetree.is_some()).map(|e| e.id.clone());
        let source = self.tree_named(name).or_else(|why| first().ok_or(why))?;
        let root = self.ft_spec(&source).map(|t| t.root.clone()).unwrap_or_default();
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
}
