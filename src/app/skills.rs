//! The Maestro's skills: a folder of the app's, handed to the Claude Code the app starts as a local
//! plugin (`--plugin-dir`), so the person's own `~/.claude` is never touched. Codex has nothing like it.

use crate::prelude::*;
use std::io::Write;
use std::path::Path;

/// One skill of the folder: `skills/<name>/SKILL.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Skill {
    pub(crate) name: String,
    pub(crate) description: String,
}

/// How long a skill's description shows, in bytes.
const DESCRIPTION: usize = 160;

/// The plugin's folder, beside the drawing.
pub(crate) fn skills_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("skills"))
}

/// The plugin as Claude Code wants it: `.claude-plugin/plugin.json` and `skills/`. What is there stays.
pub(crate) fn ensure(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path.join(".claude-plugin"))?;
    match fs::OpenOptions::new().write(true).create_new(true).open(path.join(".claude-plugin/plugin.json")) {
        Ok(mut file) => file.write_all(b"{\"name\":\"magoagent\"}\n")?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(path.join("skills"))
}

/// Shows the folder in the system's file manager, made first if it isn't there.
pub(crate) fn open(path: &Path) -> Result<(), String> {
    ensure(path).map_err(|e| e.to_string())?;
    let url = format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
    crate::app::portal::open_external(&url);
    Ok(())
}

/// The folder's skills, by name: each folder of `skills/` with a SKILL.md.
pub(crate) fn list(path: &Path) -> Vec<Skill> {
    let skill = |entry: fs::DirEntry| {
        let text = fs::read_to_string(entry.path().join("SKILL.md")).ok()?;
        Some(Skill { name: entry.file_name().to_string_lossy().into_owned(), description: description(&text) })
    };
    let mut skills: Vec<Skill> = fs::read_dir(path.join("skills")).ok().into_iter().flatten().flatten().filter_map(skill).collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// What a SKILL.md says it is for: the `description` of its front matter (quoted or not), else its
/// first line of text; cut at a whole character.
fn description(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    // the front matter between the two `---`, then the text
    let (head, body) = match lines.first() {
        Some(&"---") => match lines.iter().skip(1).position(|l| *l == "---") {
            Some(close) => (&lines[1..close + 1], &lines[close + 2..]),
            None => (&lines[1..], &lines[..0]),
        },
        _ => (&lines[..0], &lines[..]),
    };
    let named = head.iter().find_map(|l| l.strip_prefix("description:")).map(|d| d.trim().trim_matches(['\'', '"']));
    let raw = named.filter(|d| !d.is_empty()).or_else(|| body.iter().copied().find(|l| !l.is_empty())).unwrap_or_default();
    let mut end = raw.len().min(DESCRIPTION);
    while !raw.is_char_boundary(end) {
        end -= 1;
    }
    raw[..end].to_string()
}

/// What the chat's agent gets for the skills: Claude Code the plugin's folder, once it has a skill
/// (its manifest made if missing); Codex nothing.
pub(crate) fn plugin_args(agent: &str, path: Option<&Path>) -> Vec<String> {
    match path.filter(|p| agent != "codex" && !list(p).is_empty()) {
        Some(path) if ensure(path).is_ok() => vec!["--plugin-dir".into(), path.to_string_lossy().into_owned()],
        _ => Vec::new(),
    }
}

/// The MCP's `skills`: the list, or the folder opened.
pub(crate) fn act(path: &Path, action: &str) -> Result<String, String> {
    match action {
        "open" => {
            open(path)?;
            Ok(format!("Pasta de skills aberta: {}", path.display()))
        }
        "" | "list" => {
            let skills = list(path);
            if skills.is_empty() {
                return Ok(format!("Nenhuma skill ainda: ponha uma pasta com SKILL.md em {}.", path.join("skills").display()));
            }
            Ok(format!("Pasta: {}\n{}", path.display(), skills.iter().map(|s| format!("{}: {}", s.name, s.description)).collect::<Vec<_>>().join("\n")))
        }
        other => Err(format!("ação desconhecida: {other} (use list ou open)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let path = std::env::temp_dir().join(format!("mago-skills-{}", new_id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn skill(root: &Path, name: &str, text: &str) {
        fs::create_dir_all(root.join("skills").join(name)).unwrap();
        fs::write(root.join("skills").join(name).join("SKILL.md"), text).unwrap();
    }

    #[test]
    fn the_skills_are_listed_by_name_with_what_they_are_for() {
        let temp = temp();
        skill(&temp, "zeta", "---\nname: zeta\ndescription: \"entre aspas\"\n---\ncorpo");
        skill(&temp, "alpha", "---\ndescription: sem aspas\n---");
        skill(&temp, "fallback", "\n  Primeira linha\nresto");
        skill(&temp, "sem-campo", "---\nname: x\n---\nO texto depois");
        skill(&temp, "empty", "");
        fs::create_dir_all(temp.join("skills/missing")).unwrap();
        let found: Vec<(String, String)> = list(&temp).into_iter().map(|s| (s.name, s.description)).collect();
        let expected = [("alpha", "sem aspas"), ("empty", ""), ("fallback", "Primeira linha"), ("sem-campo", "O texto depois"), ("zeta", "entre aspas")];
        assert_eq!(found, expected.map(|(n, d)| (n.to_string(), d.to_string())));
        // a long description is cut at a whole character
        let long = description(&format!("---\ndescription: {}\n---", "é".repeat(100)));
        assert_eq!((long.chars().count(), long.len()), (80, 160));
        assert!(list(&temp.join("nowhere")).is_empty());
        let said = act(&temp, "list").unwrap();
        assert!(said.starts_with(&format!("Pasta: {}\nalpha: sem aspas\n", temp.display())), "{said}");
        assert!(act(&temp, "delete").is_err());
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn the_plugin_is_made_once_and_goes_to_claude_only_with_a_skill() {
        let temp = temp();
        assert!(act(&temp, "").unwrap().starts_with("Nenhuma skill ainda"));
        assert!(plugin_args("claude", Some(&temp)).is_empty(), "no skill, no argument: nothing changes for who has none");
        skill(&temp, "revisar", "---\ndescription: revisa um diff\n---");
        assert_eq!(plugin_args("claude", Some(&temp)), ["--plugin-dir".to_string(), temp.to_string_lossy().into_owned()]);
        assert_eq!(fs::read_to_string(temp.join(".claude-plugin/plugin.json")).unwrap(), "{\"name\":\"magoagent\"}\n", "the manifest came with it");
        assert!(plugin_args("codex", Some(&temp)).is_empty() && plugin_args("claude", None).is_empty());
        // what is there stays
        fs::write(temp.join(".claude-plugin/plugin.json"), "keep").unwrap();
        ensure(&temp).unwrap();
        assert_eq!(fs::read_to_string(temp.join(".claude-plugin/plugin.json")).unwrap(), "keep");
        fs::remove_dir_all(temp).unwrap();
    }
}
