//! The MCP's `changes_review`: what the person does in Alterações' review, by voice through the Maestro: list and pick
//! the worktrees, comment lines of a changed file, see and take comments away, send the review to an agent, or ask an
//! agent for a PR analysis of the whole diff.

use super::{ChangeGroup, ReviewComment, comment_on, file_diff, key, rows_for};
use crate::app::chat::code_comment::relative;
use crate::app::filetree::view::TreeGit;
use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn act_changes_review(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let number = |value: &Value| value.as_u64().map(|n| n as usize);
        let action = text("action");

        if !text("worktree").is_empty() && action != "worktrees" {
            self.choose_worktree(&text("worktree"))?;
        }

        match action.as_str() {
            "worktrees" => {
                if !text("worktree").is_empty() {
                    let picked = self.choose_worktree(&text("worktree"))?;

                    return Ok(format!("{picked}\n{}", self.describe_worktrees()));
                }

                Ok(self.describe_worktrees())
            }
            "comment" => {
                let comment = self.review_comment_at(&text("path"), number(&args["from"]), number(&args["to"]), &text("note"))?;
                let place = comment.place();

                self.panes.changes.review.comments.push(comment);
                Ok(format!("Comentário {} em {place} juntado à revisão.", self.panes.changes.review.comments.len()))
            }
            "list" => Ok(self.describe_review()),
            "remove" => {
                let count = self.panes.changes.review.comments.len();

                let index = number(&args["index"]).filter(|n| (1..=count).contains(n)).ok_or_else(|| {
                    if count == 0 { "A revisão não tem comentários.".to_string() } else { format!("Diga o número do comentário (index), de 1 a {count}.") }
                })?;

                let comment = self.panes.changes.review.comments.remove(index - 1);

                Ok(format!("Comentário {index} ({}) tirado.", comment.place()))
            }
            "clear" => {
                let review = &mut self.panes.changes.review;

                (review.comments, review.note, review.picked) = (Vec::new(), String::new(), None);
                Ok("Revisão limpa.".into())
            }
            "send" => {
                let groups = self.review_groups_now();
                let agent = self.review_agent(&text("agent"), &groups)?;
                let given = args["comments"].as_array();

                let comments = match given {
                    Some(list) => list
                        .iter()
                        .map(|c| {
                            self.review_comment_at(
                                c["path"].as_str().unwrap_or_default(),
                                number(&c["from"]),
                                number(&c["to"]),
                                c["note"].as_str().unwrap_or_default(),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    None => self.panes.changes.review.comments.clone(),
                };

                let note = if text("note").is_empty() { self.panes.changes.review.note.clone() } else { text("note") };

                if comments.is_empty() && note.trim().is_empty() {
                    return Err("Nada a enviar: junte comentários (comment), mande `comments` ou uma observação (note).".into());
                }

                let message = self.review_text(&comments, &note, &groups);
                let said = self.send_to_agent(&agent, message)?;

                if given.is_none() {
                    let review = &mut self.panes.changes.review;

                    (review.comments, review.note) = (Vec::new(), String::new());
                }

                let count = if comments.len() == 1 { "1 comentário".to_string() } else { format!("{} comentários", comments.len()) };

                Ok(format!("Revisão com {count} enviada. {said}"))
            }
            "pr" => {
                let groups = self.review_groups_now();

                if groups.iter().all(|g| g.files.is_empty()) {
                    return Err("Nenhuma alteração para analisar aqui.".into());
                }

                let agent = self.review_agent(&text("agent"), &groups)?;
                let message = self.pr_text(&groups);

                self.send_to_agent(&agent, message).map(|said| format!("Análise de PR pedida. {said}"))
            }
            _ => Err("Diga a ação (action): worktrees, comment, list, remove, clear, send ou pr.".into()),
        }
    }

    /// What the panel shows, the picked worktree's git read now when the panel didn't yet.
    pub(crate) fn review_groups_now(&mut self) -> Vec<ChangeGroup> {
        let changes = &mut self.panes.changes;

        if let Some(path) = changes.worktree.clone().filter(|_| changes.worktree_git.is_none()) {
            let read = TreeGit::read(&path);

            changes.worktree_git = Some((read.branch.clone(), crate::app::filetree::changes::sorted(&path, &read.files)));
        }

        self.shown_groups()
    }

    /// The comments, numbered, and the general note.
    fn describe_review(&self) -> String {
        let review = &self.panes.changes.review;

        let mut out = match review.comments.len() {
            0 => "A revisão não tem comentários.".to_string(),
            1 => "1 comentário:".to_string(),
            n => format!("{n} comentários:"),
        };

        for (index, comment) in review.comments.iter().enumerate() {
            out += &format!("\n{}. {} — {}", index + 1, comment.place(), comment.note);
        }

        if !review.note.trim().is_empty() {
            out += &format!("\nObservação geral: {}", review.note.trim());
        }

        out
    }

    /// A comment on the lines `from`–`to` of the changed file `path` (its path, from its folder, or its end): the
    /// stretch of its diff, or of the file when those lines did not change.
    fn review_comment_at(&mut self, path: &str, from: Option<usize>, to: Option<usize>, note: &str) -> Result<ReviewComment, String> {
        if path.trim().is_empty() {
            return Err("Diga o arquivo (path).".into());
        }

        if note.trim().is_empty() {
            return Err("Diga o que está errado (note).".into());
        }

        let from = from.filter(|n| *n > 0).ok_or("Diga a primeira linha (from; a primeira do arquivo é 1).")?;
        let to = to.unwrap_or(from).max(from);
        let (root, file) = self.change_path(path)?;
        let lines = file_diff(&file);

        if let Some(comment) = rows_for(&lines, from, to).and_then(|rows| comment_on(&root, &file, &lines, rows, note)) {
            return Ok(comment);
        }

        let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let excerpt: Vec<String> = text.lines().skip(from - 1).take(to - from + 1).map(|line| format!(" {line}")).collect();

        if excerpt.is_empty() {
            return Err(format!("{} tem só {} linhas.", relative(&root, &file), text.lines().count()));
        }

        Ok(ReviewComment { root, path: file, from, to, removed: false, excerpt: excerpt.join("\n"), note: note.trim().to_string() })
    }

    /// The changed file `wanted` names, with its folder: its whole path, its path from the folder, or the end of it;
    /// else a file in one of the folders shown.
    fn change_path(&mut self, wanted: &str) -> Result<(PathBuf, PathBuf), String> {
        let groups = self.review_groups_now();
        let given = PathBuf::from(wanted.trim());
        let short = wanted.trim().replace('\\', "/").trim_start_matches("./").to_lowercase();
        let files = || groups.iter().flat_map(|g| g.files.iter().map(move |(path, _)| (g, path)));

        let found = files()
            .find(|(g, path)| key(path) == key(&given) || relative(&g.root, path).to_lowercase() == short)
            .or_else(|| files().find(|(g, path)| relative(&g.root, path).to_lowercase().ends_with(&format!("/{short}"))));

        if let Some((group, path)) = found {
            return Ok((group.root.clone(), path.clone()));
        }

        for group in &groups {
            let file = if given.is_absolute() { given.clone() } else { group.root.join(&given) };

            if file.starts_with(&group.root) && file.is_file() {
                return Ok((group.root.clone(), file));
            }
        }

        Err(format!("Não achei {wanted} nas pastas das Alterações."))
    }
}
