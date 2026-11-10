//! The binder: named notes in Markdown that the Maestro and the agents read and write between
//! sessions (decisions, what's pending, a project's context), one file each in `fichario/` beside
//! the drawing. Not the canvas's sticky notes. The MCP's `binder` drives it; the Maestro's prompt
//! gets its index (names and first lines).

use crate::prelude::*;
use std::path::Path;

/// The most a note holds, in bytes: 64 KB.
pub(crate) const NOTE_MAX: usize = 64 * 1024;

/// The binder of the drawing in use (MAGOAGENT_SCENE moves it too).
pub(crate) fn binder_dir() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("fichario"))
}

/// A note's name as a file name every system takes: accents dropped, lowercase, anything but
/// letters, digits and `_` made `-` (slashes, dots, so no `..` and no hidden file), Windows'
/// reserved names prefixed, 80 characters at most. None when nothing is left.
pub(crate) fn note_file(name: &str) -> Option<String> {
    const ACCENTED: &str = "áàâãäåéèêëíìîïóòôõöúùûüçñýÿ";
    const PLAIN: &str = "aaaaaaeeeeiiiiooooouuuucnyy";
    let fold = |c: char| ACCENTED.chars().position(|a| a == c).and_then(|i| PLAIN.chars().nth(i)).unwrap_or(c);
    let mut out = String::new();
    for c in name.to_lowercase().chars().map(fold) {
        let c = if c.is_ascii_alphanumeric() || c == '_' { c } else { '-' };
        if !(c == '-' && (out.is_empty() || out.ends_with('-'))) {
            out.push(c);
        }
    }
    let out: String = out.chars().take(80).collect::<String>().trim_end_matches('-').to_string();
    if out.is_empty() {
        return None;
    }
    let reserved = ["con", "prn", "aux", "nul"].contains(&out.as_str())
        || (out.len() == 4 && (out.starts_with("com") || out.starts_with("lpt")) && out.as_bytes()[3].is_ascii_digit());
    Some(format!("{}{out}.md", if reserved { "_" } else { "" }))
}

fn file_of(dir: &Path, name: &str) -> Result<(PathBuf, String), String> {
    let file = note_file(name).ok_or("Diga o nome da nota (name), com letras ou números.")?;
    let stem = file.trim_end_matches(".md").to_string();
    Ok((dir.join(file), stem))
}

fn kb(bytes: usize) -> String {
    format!("{:.1} KB", bytes as f64 / 1024.0).replace('.', ",")
}

/// A note's first line with words in it, without Markdown's marks, 100 characters at most.
fn first_line(text: &str) -> String {
    let line = text.lines().map(|l| l.trim().trim_start_matches(['#', '-', '>', '*', ' '])).find(|l| !l.is_empty()).unwrap_or_default();
    line.chars().take(100).collect()
}

/// Every note, by name: (name, size in bytes, first line).
pub(crate) fn list(dir: &Path) -> Vec<(String, u64, String)> {
    let Ok(read) = fs::read_dir(dir) else { return Vec::new() };
    let mut notes: Vec<(String, u64, String)> = read
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()? != "md" {
                return None;
            }
            let text = fs::read_to_string(&path).ok()?;
            Some((path.file_stem()?.to_string_lossy().into_owned(), text.len() as u64, first_line(&text)))
        })
        .collect();
    notes.sort();
    notes
}

pub(crate) fn read(dir: &Path, name: &str) -> Result<String, String> {
    let (path, stem) = file_of(dir, name)?;
    fs::read_to_string(&path).map_err(|_| format!("Não há nota {stem} no fichário."))
}

/// Writes the note (`append`: after what it has), refusing what would pass `NOTE_MAX`.
pub(crate) fn write(dir: &Path, name: &str, text: &str, append: bool) -> Result<String, String> {
    let (path, stem) = file_of(dir, name)?;
    if text.trim().is_empty() {
        return Err("Diga o texto da nota (text).".into());
    }
    let mut body = if append { fs::read_to_string(&path).unwrap_or_default() } else { String::new() };
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str(text.trim_end());
    body.push('\n');
    if body.len() > NOTE_MAX {
        return Err(format!(
            "A nota {stem} ficaria com {}, acima do limite de {} por nota: resuma o que ela tem ou continue numa nota nova.",
            kb(body.len()),
            kb(NOTE_MAX)
        ));
    }
    fs::create_dir_all(dir).and_then(|_| write_atomic(&path, &body)).map_err(|e| format!("Não foi possível gravar a nota {stem}: {e}"))?;
    Ok(format!("Nota {stem} {} ({}).", if append { "acrescentada" } else { "gravada" }, kb(body.len())))
}

/// Takes the note out of the binder into `recovered` (never deleted), saying where.
pub(crate) fn delete(dir: &Path, recovered: &Path, name: &str) -> Result<String, String> {
    let (path, stem) = file_of(dir, name)?;
    if !path.is_file() {
        return Err(format!("Não há nota {stem} no fichário."));
    }
    let to = recovered.join(format!("fichario-{stem}-{}.md", now_ms()));
    fs::create_dir_all(recovered).and_then(|_| fs::rename(&path, &to)).map_err(|e| format!("Não foi possível tirar a nota {stem}: {e}"))?;
    Ok(format!("Nota {stem} tirada do fichário; ficou guardada em {}.", to.display()))
}

/// The Maestro's index: a line per note, its name and first line (50 notes at most).
pub(crate) fn index(dir: &Path) -> String {
    list(dir).iter().take(50).map(|(name, _, first)| format!("- {name}: {first}")).collect::<Vec<_>>().join("\n")
}

impl DrawingApp {
    /// The MCP's `binder`: list, read, write, append, delete.
    pub(crate) fn act_binder(&self, action: &str, name: &str, text: &str) -> Result<String, String> {
        let base = self.autosave.as_ref().ok_or("Sem pasta de dados: o fichário não tem onde ficar.")?;
        let dir = base.with_file_name("fichario");
        match action {
            "list" | "" => {
                let notes = list(&dir);
                if notes.is_empty() {
                    return Ok(format!("O fichário está vazio ({}).", dir.display()));
                }
                let lines: Vec<String> = notes.iter().map(|(name, size, first)| format!("- {name} ({}): {first}", kb(*size as usize))).collect();
                Ok(format!("{} notas no fichário ({}):\n{}", notes.len(), dir.display(), lines.join("\n")))
            }
            "read" => read(&dir, name),
            "write" => write(&dir, name, text, false),
            "append" => write(&dir, name, text, true),
            "delete" => delete(&dir, &base.with_file_name("recuperados"), name),
            other => Err(format!("action é list, read, write, append ou delete, não {other}.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_names_become_safe_file_names() {
        assert_eq!(note_file("Decisões do Projeto").as_deref(), Some("decisoes-do-projeto.md"));
        assert_eq!(note_file("  Ação / Integração\\Pagamentos  ").as_deref(), Some("acao-integracao-pagamentos.md"));
        assert_eq!(note_file("../../etc/passwd").as_deref(), Some("etc-passwd.md"));
        assert_eq!(note_file(".oculta").as_deref(), Some("oculta.md"));
        assert_eq!(note_file("v1.2_final").as_deref(), Some("v1-2_final.md"));
        for reserved in ["CON", "nul", "Com1", "LPT9"] {
            assert_eq!(note_file(reserved), Some(format!("_{}.md", reserved.to_lowercase())));
        }
        assert_eq!(note_file("console").as_deref(), Some("console.md"));
        for empty in ["", "   ", "..", "/", "???"] {
            assert_eq!(note_file(empty), None, "{empty:?}");
        }
        assert_eq!(note_file(&"a".repeat(200)).unwrap().len(), 80 + 3);
    }

    #[test]
    fn the_binder_writes_reads_lists_and_puts_aside() {
        let dir = std::env::temp_dir().join(format!("mago-fichario-{}", new_id()));
        let app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };
        let act = |action: &str, name: &str, text: &str| app.act_binder(action, name, text);
        assert!(act("list", "", "").unwrap().starts_with("O fichário está vazio"));
        assert!(act("write", "Decisões", "# Decisões da loja\n- pagamento pelo Pix").unwrap().starts_with("Nota decisoes gravada"));
        act("append", "decisões", "- frete grátis acima de 200").unwrap();
        assert_eq!(act("read", "DECISÕES", "").unwrap(), "# Decisões da loja\n- pagamento pelo Pix\n- frete grátis acima de 200\n");
        act("write", "Pendências", "falta o login").unwrap();
        let listed = act("list", "", "").unwrap();
        assert!(listed.starts_with("2 notas") && listed.contains("- decisoes (0,1 KB): Decisões da loja") && listed.contains("- pendencias"), "{listed}");
        assert_eq!(index(&dir.join("fichario")), "- decisoes: Decisões da loja\n- pendencias: falta o login");
        // the size cap, with nothing written past it
        let err = act("append", "pendencias", &"x".repeat(NOTE_MAX)).unwrap_err();
        assert!(err.contains("64,0 KB"), "{err}");
        assert_eq!(act("read", "pendencias", "").unwrap(), "falta o login\n");
        assert!(act("write", "..", "x").is_err() && act("write", "nota", "  ").is_err() && act("read", "nada", "").is_err());
        // deleting puts it in recuperados
        assert!(act("delete", "pendências", "").unwrap().contains("recuperados"));
        assert!(act("read", "pendencias", "").is_err());
        let kept: Vec<String> = fs::read_dir(dir.join("recuperados")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(kept.len() == 1 && kept[0].starts_with("fichario-pendencias-"), "{kept:?}");
        assert!(act("delete", "pendencias", "").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
