//! The review of Alterações, as a pull request's: lines picked in a diff get a comment (the file, the lines, the stretch
//! of the diff and what is wrong), the comments pile up and go, with a general note, to an agent in one message; or the
//! agent is asked to review the whole diff of the folder or worktree as a PR. The agents working in that folder come
//! first. The panel's part is comments.rs; the MCP's, actions.rs.

use super::diff::{DiffLine, ordered};
use super::{ChangeGroup, key};
use crate::app::chat::code_comment::relative;
use crate::app::ui::agents::AgentRow;
use crate::editor::Status;
use crate::prelude::*;
use std::path::Path;

/// The lines of a comment's stretch the message carries.
// ponytail: a longer stretch goes cut (its place still says every line); to evolve, send the rest as a file the agent reads.
const EXCERPT_LINES: usize = 40;

/// The review being written: its comments, the general note, the comment on the lines picked, the agent it goes to
/// (its id) and the diff's rows picked (first press, last); whether the comment's field is to take the keys, and whether
/// those rows are to be brought out from under the comment's bar (it shows on the frame after they are picked).
#[derive(Default)]
pub(crate) struct Review {
    pub(crate) comments: Vec<ReviewComment>,
    pub(crate) note: String,
    pub(crate) draft: String,
    pub(crate) agent: String,
    pub(crate) picked: Option<(usize, usize)>,
    pub(crate) focus_draft: bool,
    pub(crate) reveal: bool,
}

/// A comment on lines of a changed file.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReviewComment {
    /// The folder of the changes (a tree's or a worktree's) and the file.
    pub(crate) root: PathBuf,
    pub(crate) path: PathBuf,
    /// The first and last lines (from 1): of the file now, or of the HEAD when only removed lines were picked.
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) removed: bool,
    /// The diff's lines picked, each after its mark (+, - or a space).
    pub(crate) excerpt: String,
    pub(crate) note: String,
}

impl ReviewComment {
    /// "src/main.rs:12-14", the file from its folder.
    pub(crate) fn place(&self) -> String {
        let file = relative(&self.root, &self.path);

        if self.from == self.to { format!("{file}:{}", self.from) } else { format!("{file}:{}-{}", self.from, self.to) }
    }
}

/// Whether an agent whose folder is `cwd` works in `root`: in it or under it, but not in a worktree of its own inside
/// it (`.magoagent/worktrees/`), which is another checkout.
pub(crate) fn works_in(cwd: &str, root: &Path) -> bool {
    if cwd.trim().is_empty() {
        return false;
    }

    let (cwd, root) = (key(Path::new(cwd.trim())), key(root));
    let Some(rest) = cwd.trim_end_matches('/').strip_prefix(root.trim_end_matches('/')) else { return false };

    (rest.is_empty() || rest.starts_with('/')) && !rest.starts_with("/.magoagent/")
}

/// The comment on the diff's rows `rows` (either order) of `path` in `root`: the new file's lines they hold, or the
/// HEAD's when they are all removed ones; none when they hold no line (only a `…`).
pub(crate) fn comment_on(root: &Path, path: &Path, lines: &[DiffLine], rows: (usize, usize), note: &str) -> Option<ReviewComment> {
    let (first, last) = ordered(rows);
    let picked = lines.get(first..=last.min(lines.len().checked_sub(1)?))?;
    let new: Vec<usize> = picked.iter().filter_map(|l| l.new).collect();
    let (numbers, removed) = if new.is_empty() { (picked.iter().filter_map(|l| l.old).collect::<Vec<_>>(), true) } else { (new, false) };
    let (from, to) = (*numbers.iter().min()?, *numbers.iter().max()?);
    let excerpt = picked.iter().filter(|l| l.mark != '…').map(|l| format!("{}{}", l.mark, l.text)).collect::<Vec<_>>().join("\n");

    Some(ReviewComment { root: root.to_path_buf(), path: path.to_path_buf(), from, to, removed, excerpt, note: note.trim().to_string() })
}

/// The diff's rows of the file's lines `from..=to` (the MCP's comment): the lines now in it, with the removed ones just
/// before them (what they replaced); else removed lines the HEAD had there. None: those lines did not change.
pub(crate) fn rows_for(lines: &[DiffLine], from: usize, to: usize) -> Option<(usize, usize)> {
    let inside = |number: Option<usize>| number.is_some_and(|n| (from..=to).contains(&n));
    let changed_new = |l: &DiffLine| l.mark == '+' && inside(l.new);
    let removed_old = |l: &DiffLine| l.mark == '-' && inside(l.old);

    let (mut first, last) = match (lines.iter().position(changed_new), lines.iter().rposition(changed_new)) {
        (Some(first), Some(last)) => (first, last),
        _ => (lines.iter().position(removed_old)?, lines.iter().rposition(removed_old)?),
    };

    while first > 0 && lines[first - 1].mark == '-' {
        first -= 1;
    }

    Some((first, last))
}

/// A stretch for the message: `EXCERPT_LINES` lines at most.
fn clipped(excerpt: &str) -> String {
    let lines: Vec<&str> = excerpt.lines().collect();

    if lines.len() <= EXCERPT_LINES {
        return excerpt.to_string();
    }

    format!("{}\n… ({} linhas a mais)", lines[..EXCERPT_LINES].join("\n"), lines.len() - EXCERPT_LINES)
}

/// The review as one message, like a pull request's: where, then each comment (the file and lines, the stretch of the
/// diff, what is wrong), then the general note.
pub(crate) fn review_message(scope: &str, comments: &[ReviewComment], general: &str) -> String {
    let mut out = format!(
        "Revisão de código das alterações em {scope}, como numa revisão de pull request. Corrija cada ponto abaixo e, no fim, diga o que mudou em cada um."
    );

    for (index, comment) in comments.iter().enumerate() {
        let removed = if comment.removed { " (linhas removidas; números do HEAD)" } else { "" };
        let excerpt = clipped(&comment.excerpt);
        let fence = if excerpt.contains("```") { "````" } else { "```" };

        out += &format!("\n\n{}. {}{removed}\n{fence}diff\n{excerpt}\n{fence}\n{}", index + 1, comment.place(), comment.note.trim());
    }

    if !general.trim().is_empty() {
        out += &format!("\n\nObservação geral: {}", general.trim());
    }

    out
}

/// A folder whose whole diff goes to a PR analysis: where, its branch, and each file (from the folder) with its git
/// mark and lines added and removed.
pub(crate) struct PrScope {
    pub(crate) root: PathBuf,
    pub(crate) branch: String,
    pub(crate) files: Vec<(String, Status, Option<(usize, usize)>)>,
}

/// "Pedir análise de PR": the agent reviews the uncommitted diff of the folders as a pull request's reviewer would.
pub(crate) fn pr_message(scopes: &[PrScope]) -> String {
    let places: Vec<String> = scopes.iter().map(|s| s.root.display().to_string()).collect();

    let mut out = format!(
        "Faça uma análise de PR das alterações ainda não commitadas em {}, como quem revisa um pull request: leia o `git diff HEAD` e os \
         arquivos novos do `git status` ali. Aponte bugs, casos esquecidos, riscos, testes que faltam e problemas de nome ou de estilo, \
         cada um com arquivo:linha, do mais grave ao menos grave, e diga como corrigir. Não altere nada antes de eu responder.",
        places.join(" e ")
    );

    for scope in scopes {
        let branch = if scope.branch.is_empty() { String::new() } else { format!(" (ramo {})", scope.branch) };
        let count = if scope.files.len() == 1 { "1 arquivo alterado".to_string() } else { format!("{} arquivos alterados", scope.files.len()) };

        out += &format!("\n\n{}{branch}, {count}:", scope.root.display());

        for (file, status, lines) in &scope.files {
            let lines = lines.map_or(String::new(), |(added, removed)| format!(" (+{added} −{removed})"));

            out += &format!("\n{} {file}{lines}", crate::app::filetree::ui::badge(*status));
        }
    }

    out
}

impl DrawingApp {
    /// The agents a review can go to, terminals and chats: those working in `roots` first (true), then the rest.
    pub(crate) fn review_agents(&self, roots: &[PathBuf]) -> Vec<(AgentRow, bool)> {
        let mut rows: Vec<(AgentRow, bool)> = self
            .agent_rows()
            .into_iter()
            .map(|row| {
                let inside = roots.iter().any(|root| works_in(&row.spec.cwd, root));

                (row, inside)
            })
            .collect();

        rows.sort_by_key(|(_, inside)| !inside);
        rows
    }

    /// Who an agent of "Para" is: the Maestro's chat as Luna (its title, named by what was talked about, after it),
    /// the others by their name.
    pub(crate) fn review_name(&self, row: &AgentRow) -> String {
        match self.chats.iter().find(|chat| chat.id == row.id) {
            Some(chat) if !chat.recruit => format!("Luna · {}", row.spec.label()),
            _ => row.spec.label().to_string(),
        }
    }

    /// The agent a review goes to: `name` (its name or id), else the one picked in the panel, else the first working in
    /// the folders shown.
    pub(crate) fn review_agent(&self, name: &str, groups: &[ChangeGroup]) -> Result<String, String> {
        if !name.trim().is_empty() {
            return self.agent_named(name).map(|(id, _)| id);
        }

        let roots: Vec<PathBuf> = groups.iter().map(|g| g.root.clone()).collect();
        let agents = self.review_agents(&roots);
        let picked = agents.iter().find(|(row, _)| row.id == self.panes.changes.review.agent).or(agents.first());

        picked.map(|(row, _)| row.id.clone()).ok_or_else(|| "Nenhum agente no canvas para receber a revisão: chame um primeiro.".into())
    }

    /// `text` to the agent `id`: a chat runs it (next frame), a terminal gets it typed as a prompt; refused, with why,
    /// when the terminal is not running or would take it as the answer to a menu.
    pub(crate) fn send_to_agent(&mut self, id: &str, text: String) -> Result<String, String> {
        let name = self.agent_label(id);

        if self.is_chat(id) {
            self.chat_outbox.push((id.to_string(), text));
        } else {
            match self.terminals.get(id) {
                Some(Ok(session)) => session.handle.ready().map_err(|why| format!("{name} {why}"))?,
                _ => return Err(format!("{name} não está rodando: abra o terminal e envie de novo.")),
            }

            self.send_prompt_after(id, &text, std::time::Duration::ZERO);
        }

        Ok(format!("Enviado a {name}."))
    }

    /// The review's message: where its comments are (each folder with its branch; none: the folders shown).
    pub(crate) fn review_text(&self, comments: &[ReviewComment], general: &str, groups: &[ChangeGroup]) -> String {
        let candidates: Vec<PathBuf> = if comments.is_empty() {
            groups.iter().filter(|g| !g.files.is_empty()).map(|g| g.root.clone()).collect()
        } else {
            comments.iter().map(|c| c.root.clone()).collect()
        };

        let mut roots: Vec<PathBuf> = Vec::new();

        for root in candidates {
            if !roots.iter().any(|known| key(known) == key(&root)) {
                roots.push(root);
            }
        }

        let scope: Vec<String> = roots
            .iter()
            .map(|root| match self.branch_of(root) {
                branch if branch.is_empty() => root.display().to_string(),
                branch => format!("{} (ramo {branch})", root.display()),
            })
            .collect();

        review_message(&scope.join(" e "), comments, general)
    }

    /// The PR analysis' message for the folders of `groups` that have changes.
    pub(crate) fn pr_text(&self, groups: &[ChangeGroup]) -> String {
        let scopes: Vec<PrScope> = groups
            .iter()
            .filter(|g| !g.files.is_empty())
            .map(|g| PrScope {
                root: g.root.clone(),
                branch: self.branch_of(&g.root),
                files: g.files.iter().map(|(path, status)| (relative(&g.root, path), *status, self.change_line_counts(path))).collect(),
            })
            .collect();

        pr_message(&scopes)
    }
}

#[cfg(test)]
mod tests {
    use super::super::diff::diff_lines;
    use super::*;

    #[test]
    fn an_agent_works_in_a_folder_but_not_in_a_worktree_of_its_own_inside_it() {
        let repo = Path::new("/repo");

        assert!(works_in("/repo", repo) && works_in("/repo/src/", repo) && works_in("/REPO/src", repo));
        assert!(!works_in("/repo2", repo), "another folder with the same start");
        assert!(!works_in("/repo/.magoagent/worktrees/x", repo), "a worktree of its own");
        assert!(works_in("/repo/.magoagent/worktrees/x/src", Path::new("/repo/.magoagent/worktrees/x")));
        assert!(!works_in("", repo));
    }

    #[test]
    fn picked_lines_make_a_comment_with_their_numbers_and_stretch() {
        let old = "a\nb\nc\nd\n";
        let new = "a\nB\nc\nd\ne\n";
        let lines = diff_lines(old, new);
        let (root, path) = (Path::new("/repo"), Path::new("/repo/src/x.rs"));
        let row = |mark: char, text: &str| lines.iter().position(|l| l.mark == mark && l.text == text).unwrap();
        // the removed b and the added B
        let comment = comment_on(root, path, &lines, (row('+', "B"), row('-', "b")), " troque o B ").unwrap();

        assert_eq!((comment.from, comment.to, comment.removed), (2, 2, false));
        assert_eq!((comment.excerpt.as_str(), comment.note.as_str(), comment.place()), ("-b\n+B", "troque o B", "src/x.rs:2".to_string()));
        // only a removed line: the HEAD's number
        let removed = comment_on(root, path, &lines, (row('-', "b"), row('-', "b")), "x").unwrap();

        assert_eq!((removed.from, removed.removed), (2, true));
        // a stretch over context and the new line
        let stretch = comment_on(root, path, &lines, (row(' ', "c"), row('+', "e")), "x").unwrap();

        assert_eq!((stretch.from, stretch.to, stretch.place()), (3, 5, "src/x.rs:3-5".to_string()));
        // the MCP's lines back to rows: the new line with the removed one it replaced
        assert_eq!(rows_for(&lines, 2, 2), Some((row('-', "b"), row('+', "B"))));
        assert_eq!(rows_for(&lines, 5, 9), Some((row('+', "e"), row('+', "e"))));
        assert_eq!(rows_for(&lines, 1, 1), None, "an unchanged line");
    }

    #[test]
    fn the_review_reads_as_a_pull_request_review() {
        let comment = |from: usize, to: usize, note: &str| ReviewComment {
            root: PathBuf::from("/repo"),
            path: PathBuf::from("/repo/src/main.rs"),
            from,
            to,
            removed: false,
            excerpt: "-let x = 1;\n+let x = 2;".into(),
            note: note.into(),
        };

        let message = review_message("/repo (ramo main)", &[comment(12, 14, "x não pode mudar"), comment(3, 3, "nome ruim")], " teste junto ");

        assert!(message.starts_with("Revisão de código das alterações em /repo (ramo main), como numa revisão de pull request."), "{message}");
        assert!(message.contains("\n\n1. src/main.rs:12-14\n```diff\n-let x = 1;\n+let x = 2;\n```\nx não pode mudar"), "{message}");
        assert!(message.contains("\n\n2. src/main.rs:3\n"), "{message}");
        assert!(message.ends_with("\n\nObservação geral: teste junto"), "{message}");
        // a long stretch is cut
        let long = ReviewComment { excerpt: (0..60).map(|n| format!("+{n}")).collect::<Vec<_>>().join("\n"), ..comment(1, 60, "x") };

        assert!(review_message("/repo", &[long], "").contains("… (20 linhas a mais)"));
    }

    #[test]
    fn the_pr_request_lists_the_files_of_each_folder() {
        let scope = PrScope {
            root: PathBuf::from("/repo/.magoagent/worktrees/revisor"),
            branch: "mago/revisor".into(),
            files: vec![("src/a.rs".into(), Status::Modified, Some((3, 1))), ("novo.rs".into(), Status::Untracked, None)],
        };

        let message = pr_message(&[scope]);

        assert!(message.starts_with("Faça uma análise de PR das alterações ainda não commitadas em /repo/.magoagent/worktrees/revisor"), "{message}");
        assert!(message.contains("Não altere nada antes de eu responder."));
        assert!(message.ends_with("/repo/.magoagent/worktrees/revisor (ramo mago/revisor), 2 arquivos alterados:\nM src/a.rs (+3 −1)\nU novo.rs"), "{message}");
    }
}
