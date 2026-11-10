use super::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CodeReference {
    pub project: String,
    pub file: String,
    pub from: usize,
    pub to: usize,
    pub revision: Revision,
    pub snippet: String,
    pub truncated: bool,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub comment: String,
    pub origin: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Revision {
    pub branch: String,
    pub commit: Option<String>,
    pub side: String,
}

const SNIPPET_LIMIT: usize = 16_000;

impl Ide {
    // ponytail: the reference's parts as arguments; a struct for them if a third caller appears
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn code_reference(
        &self,
        path: &Path,
        text: &str,
        from: usize,
        to: usize,
        comment: &str,
        origin: &str,
        side: &str,
    ) -> Result<CodeReference, String> {
        build_reference(&self.folder, &self.branch, path, text, from, to, comment, origin, side)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_reference(
    folder: &str,
    branch: &str,
    path: &Path,
    text: &str,
    from: usize,
    to: usize,
    comment: &str,
    origin: &str,
    side: &str,
) -> Result<CodeReference, String> {
    let root = Path::new(folder).canonicalize().map_err(|e| e.to_string())?;
    let file = path.canonicalize().map_err(|e| e.to_string())?;
    let relative = file.strip_prefix(&root).map_err(|_| "o arquivo fica fora da pasta do IDE".to_string())?;
    let lines: Vec<&str> = text.lines().collect();
    if from == 0 || to < from || to > lines.len() {
        return Err("intervalo de linhas inválido".into());
    }
    let full = lines[from - 1..to].join("\n");
    let snippet: String = full.chars().take(SNIPPET_LIMIT).collect();
    // through the IDE's own git: no console window flashes on Windows
    let commit = super::git::git(Path::new(folder), &["rev-parse", "--short", "HEAD"]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    Ok(CodeReference {
        project: folder.to_string(),
        file: relative.to_string_lossy().replace('\\', "/"),
        from,
        to,
        revision: Revision { branch: branch.to_string(), commit, side: side.into() },
        truncated: snippet.len() < full.len(),
        snippet,
        before: lines[from.saturating_sub(4)..from - 1].iter().map(|s| (*s).to_string()).collect(),
        after: lines[to..(to + 3).min(lines.len())].iter().map(|s| (*s).to_string()).collect(),
        comment: comment.trim().into(),
        origin: origin.into(),
    })
}

pub(crate) fn message(reference: &CodeReference) -> String {
    let json = serde_json::to_string_pretty(reference).unwrap_or_default();
    format!("Sobre {} (linhas {}–{}): {}\n```referencia-de-codigo\n{}\n```", reference.file, reference.from, reference.to, reference.comment, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_has_exact_selection_and_edge_context() {
        let dir = std::env::temp_dir().join(format!("mago-coderef-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.rs");
        let text = "one\ntwo\nthree\nfour\nfive";
        std::fs::write(&path, text).unwrap();
        let ide = Ide::new(egui::Context::default(), "test", dir.to_str().unwrap());
        let r = ide.code_reference(&path, text, 1, 2, "why", "editor", "local").unwrap();
        assert_eq!(r.snippet, "one\ntwo");
        assert!(r.before.is_empty());
        assert_eq!(r.after, ["three", "four", "five"]);
        let r = ide.code_reference(&path, text, 4, 5, "why", "diff", "head").unwrap();
        assert_eq!(r.before, ["one", "two", "three"]);
        assert!(r.after.is_empty());
        let decoded: CodeReference =
            serde_json::from_str(message(&r).split("```referencia-de-codigo\n").nth(1).unwrap().split("\n```").next().unwrap()).unwrap();
        assert_eq!(decoded, r);
        assert!(ide.code_reference(&path, text, 0, 1, "x", "editor", "local").is_err());
        assert!(ide.code_reference(&path, text, 2, 99, "x", "editor", "local").is_err());
        let huge = "x".repeat(SNIPPET_LIMIT + 20);
        std::fs::write(&path, &huge).unwrap();
        let r = ide.code_reference(&path, &huge, 1, 1, "x", "editor", "local").unwrap();
        assert!(r.truncated);
        assert_eq!(r.snippet.chars().count(), SNIPPET_LIMIT);
        let outside = dir.with_extension("outside");
        std::fs::write(&outside, "no").unwrap();
        assert!(ide.code_reference(&outside, "no", 1, 1, "x", "editor", "local").is_err());
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(dir);
    }
}
