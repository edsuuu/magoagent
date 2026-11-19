//! The panels on the window's right (app/panes/) in SwiftUI: topic "panes" (Alterações and Nota rápida) and the
//! `ui.panes.*` verbs (macos/MagoUI/Sources/MagoUI/Panes). The egui panel keeps running unseen, so its thread still
//! recounts the lines and lists the worktrees: this reads its state, and each verb does what its click does. Opening,
//! closing and widening go by the MCP's `panel`; Nota rápida's text by `quick_note`.

use crate::app::filetree::native::snapshot::{IconRef, icon_of};
use crate::app::panes::PANES;
use crate::app::panes::changes::diff::{ADDED_ROW, REMOVED_ROW, line_parts, token_color};
use crate::app::panes::changes::list::{FileMatch, LIST_ROWS, file_match};
use crate::app::panes::changes::{ChangeGroup, ReviewComment, Worktree, comment_on, folder_name, key};
use crate::app::ui::agents::AgentRow;
use crate::prelude::*;
use std::path::Path;

/// A color as Swift's `Color(rgba:)` reads it.
fn rgba(color: Color32) -> [u8; 4] {
    color.to_srgba_unmultiplied()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PanesTopic {
    /// "changes" or "note"; none over the chat, which the panels don't cover.
    open: Option<&'static str>,
    width: f32,
    /// Alterações' badge: the changed files of the trees' folders.
    count: usize,
    /// How many rows the list shows before it scrolls.
    list_rows: usize,
    look: Look,
    note: String,
    /// Only while Alterações is open.
    changes: Option<ChangesModel>,
}

/// The panel's colors, the egui ones (`UiColors`, `glass_ink`, `glass_accent`, the diff's rows).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Look {
    bg: [u8; 4],
    text: [u8; 4],
    muted: [u8; 4],
    danger: [u8; 4],
    added: [u8; 4],
    selected: [u8; 4],
    field: [u8; 4],
    /// A button or a field under the pointer.
    raised: [u8; 4],
    ink: [u8; 4],
    accent: [u8; 4],
    added_row: [u8; 4],
    removed_row: [u8; 4],
}

#[derive(Serialize)]
struct ChangesModel {
    /// The picker's "Worktree: …"; none while no worktree is known.
    worktree: Option<String>,
    worktrees: Vec<WorktreeRow>,
    search: String,
    /// Under the search: how many files answer it.
    found: Option<String>,
    /// What shows when nothing changed.
    empty: Option<&'static str>,
    groups: Vec<GroupModel>,
    /// The file whose diff shows, its diff and the rows picked in it.
    selected: Option<String>,
    diff: Vec<DiffRow>,
    picked: Option<[usize; 2]>,
    /// "Comentar src/a.rs:2" while lines are picked: the comment's bar shows.
    comment: Option<String>,
    review: Option<ReviewModel>,
}

#[derive(Serialize)]
struct WorktreeRow {
    /// Empty: Todos, the trees' folders.
    path: String,
    label: String,
    on: bool,
    tip: String,
}

#[derive(Serialize)]
struct GroupModel {
    title: String,
    files: Vec<FileModel>,
}

#[derive(Serialize)]
struct FileModel {
    tree: String,
    path: String,
    name: String,
    color: [u8; 4],
    icon: IconRef,
    /// The lines added and removed, once counted (a binary file has none).
    lines: Option<(usize, usize)>,
    /// The changed line the search found the text in.
    code: Option<String>,
}

#[derive(Serialize)]
struct DiffRow {
    mark: char,
    old: Option<usize>,
    new: Option<usize>,
    /// The text in stretches of one color.
    runs: Vec<(String, [u8; 4])>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewModel {
    heading: String,
    comments: Vec<CommentModel>,
    note: String,
    agents: Vec<AgentModel>,
    /// The agent it goes to (the first when the one picked is gone).
    agent: Option<String>,
    can_send: bool,
    can_ask: bool,
}

#[derive(Serialize)]
struct CommentModel {
    place: String,
    first: String,
    note: String,
}

#[derive(Serialize)]
struct AgentModel {
    id: String,
    /// Its name (the picker shows it), and its name with what it is in the list.
    name: String,
    label: String,
    cwd: String,
}

impl DrawingApp {
    /// Each frame with the shell on.
    pub(super) fn native_panes(&self) {
        crate::app::native::publish("panes", &self.panes_topic());
    }

    pub(super) fn panes_topic(&self) -> PanesTopic {
        let (colors, dark) = (self.colors(), self.dark());
        let open = self.panes.open.filter(|_| !self.chat_open);

        PanesTopic {
            open: open.and_then(|pane| PANES.iter().find(|p| p.0 == pane)).map(|p| p.1),
            width: self.panel_width(),
            count: self.changed_file_count(),
            list_rows: LIST_ROWS,
            look: Look {
                bg: rgba(colors.island),
                text: rgba(colors.text),
                muted: rgba(colors.muted),
                danger: rgba(colors.danger),
                added: rgba(crate::app::filetree::diff::ADDED),
                selected: rgba(colors.selected),
                field: rgba(colors.surface_low),
                raised: rgba(colors.surface_high),
                ink: rgba(glass_ink(dark)),
                accent: rgba(glass_accent(dark)),
                added_row: rgba(ADDED_ROW),
                removed_row: rgba(REMOVED_ROW),
            },
            note: self.prefs.quick_note.clone(),
            changes: (open == Some(crate::app::panes::Pane::Changes)).then(|| self.changes_model()),
        }
    }

    /// What `changes_pane` draws, in the same order and with the same words.
    fn changes_model(&self) -> ChangesModel {
        let changes = &self.panes.changes;
        let groups = self.shown_groups();
        let current = changes.worktree.as_deref().map(key);

        let worktree = (!changes.worktrees.is_empty() || changes.worktree.is_some()).then(|| {
            let shown = changes.worktree.as_ref().map_or("Todos".to_string(), |path| {
                changes.worktrees.iter().find(|w| key(&w.path) == key(path)).map_or_else(|| folder_name(path), Worktree::label)
            });

            format!("Worktree: {shown}")
        });

        let all =
            WorktreeRow { path: String::new(), label: "Todos".into(), on: current.is_none(), tip: "As alterações de todas as Árvores do canvas".into() };

        let worktrees = [all]
            .into_iter()
            .chain(changes.worktrees.iter().map(|w| WorktreeRow {
                path: w.path.to_string_lossy().into_owned(),
                label: if w.main { format!("{} (principal)", w.label()) } else { w.label() },
                on: current.as_deref() == Some(key(&w.path).as_str()),
                tip: w.path.to_string_lossy().into_owned(),
            }))
            .collect();

        let nothing = groups.iter().all(|g| g.files.is_empty());

        let empty = nothing.then(|| match &changes.worktree {
            Some(_) if changes.worktree_git.is_none() => "Lendo o worktree…",
            Some(_) => "Nenhuma alteração pendente neste worktree.",
            None if groups.is_empty() => "Nenhuma Árvore de Arquivos de um repositório git no canvas.",
            None => "Nenhuma alteração pendente.",
        });

        let shown = if nothing { Vec::new() } else { self.change_list(&groups) };
        let query = changes.search.trim();

        let found = (!nothing && !query.is_empty()).then(|| {
            let found: usize = shown.iter().map(|g| g.files.len()).sum();
            let total: usize = groups.iter().map(|g| g.files.len()).sum();

            if found == 0 { format!("Nada com “{query}” no nome nem nas mudanças.") } else { format!("{found} de {total} arquivos") }
        });

        let ink = glass_ink(self.dark());

        let diff = changes
            .selected
            .as_ref()
            .map(|_| {
                changes
                    .diff
                    .iter()
                    .map(|line| {
                        // a line around the changes in a fainter ink, as egui's
                        let text = ink.gamma_multiply(if line.mark == ' ' { 0.75 } else { 1.0 });
                        let runs = line_parts(line).into_iter().map(|(part, kind)| (part, rgba(token_color(kind, self.dark(), text)))).collect();

                        DiffRow { mark: line.mark, old: line.old, new: line.new, runs }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let comment = self.picked_comment("").map(|c| format!("Comentar {}{}", c.place(), if c.removed { " (removidas)" } else { "" }));
        let review = (!nothing || !changes.review.comments.is_empty()).then(|| self.review_model(&groups));

        ChangesModel {
            worktree,
            worktrees,
            search: changes.search.clone(),
            found,
            empty,
            groups: shown,
            selected: changes.selected.as_ref().map(|(_, path)| path.to_string_lossy().into_owned()),
            diff,
            picked: changes.review.picked.map(|rows| [rows.0, rows.1]),
            comment,
            review,
        }
    }

    /// The files of each folder that answer the search (`changes_list`'s filter).
    fn change_list(&self, groups: &[ChangeGroup]) -> Vec<GroupModel> {
        let query = &self.panes.changes.search;
        let dark = self.dark();

        groups
            .iter()
            .map(|group| {
                let files = group
                    .files
                    .iter()
                    .filter_map(|(path, status)| {
                        let name = path.strip_prefix(&group.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
                        let found = file_match(&name, self.changed_lines(path), query)?;

                        Some(FileModel {
                            tree: group.tree.clone(),
                            path: path.to_string_lossy().into_owned(),
                            color: rgba(crate::app::filetree::ui::status_color(*status, dark)),
                            icon: icon_of(path, false),
                            lines: self.change_line_counts(path),
                            code: match found {
                                FileMatch::Code(line) => Some(line),
                                FileMatch::Name => None,
                            },
                            name,
                        })
                    })
                    .collect();

                GroupModel { title: group.title.clone(), files }
            })
            .filter(|group| !group.files.is_empty())
            .collect()
    }

    /// The agents a review can go to (those in these folders first, the open file's before them), and the one it goes to.
    fn review_targets(&self, groups: &[ChangeGroup]) -> (Vec<(AgentRow, bool)>, Option<String>) {
        let changes = &self.panes.changes;
        let mut roots: Vec<PathBuf> = Vec::new();

        if let Some((_, path)) = &changes.selected {
            roots.extend(groups.iter().filter(|g| path.starts_with(&g.root)).map(|g| g.root.clone()));
        }

        roots.extend(groups.iter().filter(|g| !g.files.is_empty()).map(|g| g.root.clone()));
        let agents = self.review_agents(&roots);
        let agent = agents.iter().find(|(row, _)| row.id == changes.review.agent).or(agents.first()).map(|(row, _)| row.id.clone());

        (agents, agent)
    }

    fn review_model(&self, groups: &[ChangeGroup]) -> ReviewModel {
        let review = &self.panes.changes.review;
        let (agents, agent) = self.review_targets(groups);
        let count = review.comments.len();

        ReviewModel {
            heading: if count == 0 { "Revisão".to_string() } else { format!("Revisão · {count}") },
            comments: review
                .comments
                .iter()
                .map(|c| CommentModel {
                    place: c.place(),
                    first: c.excerpt.lines().find(|l| l.starts_with(['+', '-'])).or(c.excerpt.lines().next()).unwrap_or_default().to_string(),
                    note: c.note.clone(),
                })
                .collect(),
            note: review.note.clone(),
            can_send: !agents.is_empty() && (count > 0 || !review.note.trim().is_empty()),
            can_ask: !agents.is_empty() && groups.iter().any(|g| !g.files.is_empty()),
            agents: agents
                .iter()
                .map(|(row, inside)| {
                    let mut label = row.spec.label().to_string();

                    if row.spec.chat {
                        label += " (chat)";
                    }

                    if *inside {
                        label += " · nesta pasta";
                    }

                    AgentModel { id: row.id.clone(), name: row.spec.label().to_string(), label, cwd: row.spec.cwd.clone() }
                })
                .collect(),
            agent,
        }
    }

    /// The comment the picked rows of the file shown would make, with `note`.
    fn picked_comment(&self, note: &str) -> Option<ReviewComment> {
        let changes = &self.panes.changes;
        let (rows, (_, path)) = (changes.review.picked?, changes.selected.as_ref()?);
        let root = self.shown_groups().into_iter().find(|g| g.files.iter().any(|(p, _)| p == path))?.root;

        comment_on(&root, path, &changes.diff, rows, note)
    }

    /// `ui.panes.<verb>`: what the egui panel's clicks do.
    pub(crate) fn native_panes_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();

        match verb {
            // a file of the list: its diff shows (pressed again, it hides)
            "select" => self.select_change(&text("tree"), Path::new(&text("path"))),
            "open" => {
                let path = PathBuf::from(text("path"));
                let root = self.shown_groups().into_iter().find(|g| g.files.iter().any(|(p, _)| *p == path)).map(|g| g.root);

                self.open_change_in_tree(&text("tree"), root.as_deref(), &path);
            }
            "search" => self.panes.changes.search = text("text"),
            "worktree" => self.set_worktree(Some(PathBuf::from(text("path"))).filter(|p| !p.as_os_str().is_empty())),
            "pick" => self.pick_rows(args)?,
            "comment" => {
                let comment = self.picked_comment(&text("note")).ok_or("Escolha as linhas do diff a comentar.")?;

                if comment.note.is_empty() {
                    return Err("Diga o que está errado aqui.".into());
                }

                let review = &mut self.panes.changes.review;

                review.comments.push(comment);
                (review.draft, review.picked) = (String::new(), None);
            }
            "remove" => {
                let index = args["index"].as_u64().map(|n| n as usize).filter(|n| *n < self.panes.changes.review.comments.len());

                self.panes.changes.review.comments.remove(index.ok_or("Esse comentário não existe.")?);
            }
            "note" => self.panes.changes.review.note = text("text"),
            "agent" => self.panes.changes.review.agent = text("id"),
            "send" | "pr" => {
                let groups = self.shown_groups();
                let agent = self.review_targets(&groups).1.ok_or("Nenhum agente no canvas para receber a revisão: chame um primeiro.")?;
                let review = &self.panes.changes.review;
                let message = if verb == "send" { self.review_text(&review.comments, &review.note, &groups) } else { self.pr_text(&groups) };
                let said = self.send_to_agent(&agent, message)?;

                if verb == "send" {
                    let review = &mut self.panes.changes.review;

                    (review.comments, review.note) = (Vec::new(), String::new());
                    self.toast(format!("Revisão enviada. {said}"));
                } else {
                    self.toast(format!("Análise de PR pedida. {said}"));
                }
            }
            // Nota rápida's Levar para o canvas, said as the egui button says it
            "to_canvas" => {
                let said = self.quick_note_to_canvas()?;

                self.toast(said);
            }
            _ => return Err(format!("ação de interface desconhecida: ui.panes.{verb}")),
        }

        Ok(String::new())
    }

    /// A press on the diff's `row` (none: let the lines go), as egui's: `extend` (Shift, or a drag going on) stretches from
    /// the first one picked; a `toggle` (a click) on the one line picked lets it go; else that line alone.
    fn pick_rows(&mut self, args: &Value) -> Result<(), String> {
        let changes = &mut self.panes.changes;

        let Some(row) = args["row"].as_u64().map(|n| n as usize) else {
            changes.review.picked = None;

            return Ok(());
        };

        if row >= changes.diff.len() {
            return Err(format!("O diff não tem a linha {row}."));
        }

        let (extend, toggle) = (args["extend"].as_bool().unwrap_or(false), args["toggle"].as_bool().unwrap_or(false));

        changes.review.picked = match changes.review.picked {
            Some((anchor, _)) if extend => Some((anchor, row)),
            Some(rows) if toggle && rows == (row, row) => None,
            _ => Some((row, row)),
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::panes::Pane;
    use crate::app::panes::changes::diff_lines;
    use crate::editor::Status;

    fn topic(app: &DrawingApp) -> Value {
        serde_json::to_value(app.panes_topic()).unwrap()
    }

    #[test]
    fn nota_rapida_shows_its_text_and_goes_to_the_canvas() {
        let mut app = DrawingApp::default();

        assert!(topic(&app)["open"].is_null() && topic(&app)["changes"].is_null());
        app.act("panel", &json!({ "name": "note", "open": true })).unwrap();
        app.act("quick_note", &json!({ "action": "set", "text": "comprar pão" })).unwrap();
        let shown = topic(&app);

        assert_eq!((shown["open"].as_str(), shown["note"].as_str(), shown["width"].as_f64()), (Some("note"), Some("comprar pão"), Some(340.0)));
        app.chat_open = true;
        assert!(topic(&app)["open"].is_null(), "the panels don't cover the chat");
        app.chat_open = false;
        app.native_panes_act("to_canvas", &json!({})).unwrap();
        assert_eq!((app.prefs.quick_note.as_str(), app.scene.elements.len()), ("", 2), "a note and its text");
        assert!(app.native_panes_act("to_canvas", &json!({})).is_err(), "empty");
    }

    #[test]
    fn alteracoes_lists_the_files_and_its_diff_lines_get_a_comment() {
        let mut app = DrawingApp::default();
        let root = PathBuf::from("/repo");
        let file = root.join("src").join("a.rs");
        let changes = &mut app.panes.changes;

        // a worktree read: its group is what the panel shows
        (changes.worktree, changes.worktree_git) = (Some(root.clone()), Some(("main".into(), vec![(file.clone(), Status::Modified)])));
        (changes.selected, changes.diff) = (Some((String::new(), file.clone())), diff_lines("fn a() {}\nb\n", "fn a() {}\nB\n"));
        app.panes.open = Some(Pane::Changes);
        let shown = topic(&app);
        let changes = &shown["changes"];

        assert_eq!(changes["worktree"], "Worktree: repo", "a worktree no list knows: its folder");
        assert_eq!(changes["groups"][0]["title"], "repo · main");
        assert_eq!(changes["groups"][0]["files"][0]["name"], "src/a.rs");
        let marks: Vec<&str> = changes["diff"].as_array().unwrap().iter().map(|row| row["mark"].as_str().unwrap()).collect();

        assert_eq!(marks, [" ", "-", "+"]);
        assert_eq!(changes["diff"][0]["runs"][0][0], "fn a() {}", "the text in stretches of one color (no language read here)");
        assert!(changes["comment"].is_null() && changes["review"]["heading"] == "Revisão");
        // a click picks a line, Shift stretches, a click on the one line picked lets it go
        let pick = |app: &mut DrawingApp, args: Value| app.native_panes_act("pick", &args).unwrap();

        pick(&mut app, json!({ "row": 2, "toggle": true }));
        assert_eq!(topic(&app)["changes"]["comment"], "Comentar src/a.rs:2");
        pick(&mut app, json!({ "row": 1, "extend": true }));
        assert_eq!(app.panes.changes.review.picked, Some((2, 1)));
        pick(&mut app, json!({ "row": 1, "toggle": true }));
        pick(&mut app, json!({ "row": 1, "toggle": true }));
        assert_eq!(app.panes.changes.review.picked, None);
        assert!(app.native_panes_act("pick", &json!({ "row": 9 })).is_err());
        pick(&mut app, json!({ "row": 1, "toggle": true }));
        assert_eq!(topic(&app)["changes"]["comment"], "Comentar src/a.rs:2 (removidas)");
        assert!(app.native_panes_act("comment", &json!({ "note": " " })).is_err(), "says what is wrong");
        app.native_panes_act("comment", &json!({ "note": "não mude o b" })).unwrap();
        let review = &topic(&app)["changes"]["review"];

        assert_eq!((review["heading"].as_str(), review["comments"][0]["place"].as_str()), (Some("Revisão · 1"), Some("src/a.rs:2")));
        assert_eq!(review["comments"][0]["first"], "-b");
        assert!(app.panes.changes.review.picked.is_none(), "the lines go once commented");
        app.native_panes_act("remove", &json!({ "index": 0 })).unwrap();
        assert!(app.native_panes_act("remove", &json!({ "index": 0 })).is_err());
        // the search keeps what answers it, and says so
        app.native_panes_act("search", &json!({ "text": "zzz" })).unwrap();
        let changes = &topic(&app)["changes"];

        assert!(changes["groups"].as_array().unwrap().is_empty());
        assert_eq!(changes["found"], "Nada com “zzz” no nome nem nas mudanças.");
        // Todos: the trees' folders again
        app.native_panes_act("worktree", &json!({ "path": "" })).unwrap();
        assert!(app.panes.changes.worktree.is_none() && app.panes.changes.selected.is_none());
        assert_eq!(topic(&app)["changes"]["empty"], "Nenhuma Árvore de Arquivos de um repositório git no canvas.");
    }
}
