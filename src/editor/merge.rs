//! Conflicts a merge or a stash leaves in a repository folder: a file's three versions from the
//! index (`:1:` base, `:2:` mine, `:3:` theirs), the whole-file answers (Aceitar Meus / Aceitar
//! Deles) and the merge by chunk behind the three-pane window (mergeview.rs). Nothing here knows
//! who shows it: the file tree does, and the MCP's `file_tree` answers with `describe` and `resolve`.

use super::git::git;
use super::text::file_name;
use similar::{Algorithm, DiffTag, capture_diff_slices};
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    Mine,
    Theirs,
}

/// A run of lines of the three versions: untouched, changed by one side (already in the result)
/// or by both (a conflict, for the person to answer side by side).
#[derive(Debug, PartialEq)]
pub struct Chunk {
    pub base: Range<usize>,
    pub mine: Range<usize>,
    pub theirs: Range<usize>,
    /// both sides changed these lines, each its own way
    pub conflict: bool,
    /// the sides in the result, in the order they went in; none: the base's lines
    pub taken: Vec<Side>,
    /// the sides already answered, applied or ignored: [mine, theirs]
    pub done: [bool; 2],
}

impl Chunk {
    pub fn pending(&self) -> bool {
        self.conflict && self.done != [true; 2]
    }
}

/// The base's ranges one side changed, each with the side's lines that take its place.
fn hunks(base: &[&str], side: &[&str]) -> Vec<(Range<usize>, Range<usize>)> {
    capture_diff_slices(Algorithm::Myers, base, side)
        .iter()
        .map(|op| op.as_tag_tuple())
        .filter(|(tag, ..)| *tag != DiffTag::Equal)
        .map(|(_, b, s)| (b, s))
        .collect()
}

/// A file being merged by chunk: the three versions line by line and what the result takes of each chunk.
pub struct Merge {
    pub path: PathBuf,
    pub base: Vec<String>,
    pub mine: Vec<String>,
    pub theirs: Vec<String>,
    pub chunks: Vec<Chunk>,
    /// the conflict the arrows walked to, and the chunk to bring into view
    pub current: Option<usize>,
    pub scroll_to: Option<usize>,
}

impl Merge {
    /// The three-way merge by lines: between the lines neither side touched, a chunk for every
    /// stretch one side changed (taken, as git does) or both did (a conflict, unless they agree).
    /// Changes that touch are one chunk, as in git.
    pub fn new(path: PathBuf, base: &str, mine: &str, theirs: &str) -> Self {
        // split, not lines(): the last piece says whether the text ends in a line break
        let [b, m, t] = [base, mine, theirs].map(|s| s.split('\n').collect::<Vec<_>>());
        let (hm, ht) = (hunks(&b, &m), hunks(&b, &t));
        let (mut chunks, mut at, mut i, mut j) = (vec![], 0, 0, 0);
        let (mut dm, mut dt) = (0isize, 0isize); // how far each side's lines are from the base's
        let plain = |r: Range<usize>, dm: isize, dt: isize| {
            let shift = |d: isize| (r.start as isize + d) as usize..(r.end as isize + d) as usize;
            Chunk { mine: shift(dm), theirs: shift(dt), base: r, conflict: false, taken: vec![], done: [true; 2] }
        };
        while i < hm.len() || j < ht.len() {
            let start = hm.get(i).map_or(usize::MAX, |h| h.0.start).min(ht.get(j).map_or(usize::MAX, |h| h.0.start));
            let (mut end, i0, j0) = (start, i, j);
            loop {
                let seen = (i, j);
                while hm.get(i).is_some_and(|h| h.0.start <= end) {
                    end = end.max(hm[i].0.end);
                    i += 1;
                }
                while ht.get(j).is_some_and(|h| h.0.start <= end) {
                    end = end.max(ht[j].0.end);
                    j += 1;
                }
                if seen == (i, j) {
                    break;
                }
            }
            if at < start {
                chunks.push(plain(at..start, dm, dt));
            }
            // a side's lines for the region: where it starts there, up to where it ends after that side's changes
            let side = |hs: &[(Range<usize>, Range<usize>)], d: &mut isize| {
                let from = (start as isize + *d) as usize;
                *d += hs.iter().map(|h| h.1.len() as isize - h.0.len() as isize).sum::<isize>();
                from..(end as isize + *d) as usize
            };
            let (rm, rt) = (side(&hm[i0..i], &mut dm), side(&ht[j0..j], &mut dt));
            // one side left it alone, or both wrote the same: nothing to ask
            let taken = if m[rm.clone()] == t[rt.clone()] || t[rt.clone()] == b[start..end] {
                Some(Side::Mine)
            } else if m[rm.clone()] == b[start..end] {
                Some(Side::Theirs)
            } else {
                None
            };
            chunks.push(Chunk {
                base: start..end,
                mine: rm,
                theirs: rt,
                conflict: taken.is_none(),
                done: [taken.is_some(); 2],
                taken: taken.into_iter().collect(),
            });
            at = end;
        }
        if at < b.len() {
            chunks.push(plain(at..b.len(), dm, dt));
        }
        let own = |v: Vec<&str>| v.into_iter().map(String::from).collect();
        Merge { path, base: own(b), mine: own(m), theirs: own(t), chunks, current: None, scroll_to: None }
    }

    pub fn side(&self, c: &Chunk, side: Side) -> &[String] {
        match side {
            Side::Mine => &self.mine[c.mine.clone()],
            Side::Theirs => &self.theirs[c.theirs.clone()],
        }
    }

    /// The result's lines for a chunk: the sides taken, in order, or the base's.
    pub fn result(&self, c: &Chunk) -> Vec<&str> {
        if c.taken.is_empty() {
            return self.base[c.base.clone()].iter().map(String::as_str).collect();
        }
        c.taken.iter().flat_map(|s| self.side(c, *s)).map(String::as_str).collect()
    }

    /// How many lines `result` has, without making them.
    pub fn result_len(&self, c: &Chunk) -> usize {
        if c.taken.is_empty() { c.base.len() } else { c.taken.iter().map(|s| self.side(c, *s).len()).sum() }
    }

    /// Conflicts still waiting for an answer.
    pub fn pending(&self) -> usize {
        self.chunks.iter().filter(|c| c.pending()).count()
    }

    /// The chunks that are conflicts, in order: the numbers the window and the MCP count them by.
    pub fn conflicts(&self) -> Vec<usize> {
        (0..self.chunks.len()).filter(|&k| self.chunks[k].conflict).collect()
    }

    /// » or «: the side's lines go into the result, after the other side's when that one went in first.
    pub fn apply(&mut self, k: usize, side: Side) {
        let c = &mut self.chunks[k];
        if !std::mem::replace(&mut c.done[side as usize], true) {
            c.taken.push(side);
        }
    }

    /// ✕: the side's lines stay out.
    pub fn ignore(&mut self, k: usize, side: Side) {
        self.chunks[k].done[side as usize] = true;
    }

    /// The conflict as it was before any answer.
    pub fn reset(&mut self, k: usize) {
        (self.chunks[k].taken, self.chunks[k].done) = (vec![], [false; 2]);
    }

    /// The chunk answered at once: these sides, in this order (none: the base's lines).
    pub fn pick(&mut self, k: usize, sides: &[Side]) {
        (self.chunks[k].taken, self.chunks[k].done) = (sides.to_vec(), [true; 2]);
    }

    /// "Aceitar Esquerda" / "Aceitar Direita": the result is that side's whole file.
    pub fn accept(&mut self, side: Side) {
        for k in 0..self.chunks.len() {
            self.pick(k, &[side]);
        }
    }

    /// The next (`step` 1) or previous (-1) conflict comes into view; the ends stop the walk.
    pub fn go(&mut self, step: isize) {
        let all = self.conflicts();
        let Some(last) = all.len().checked_sub(1) else { return };
        let at = self.current.and_then(|k| all.iter().position(|&c| c == k));
        let next = at.map_or(if step > 0 { 0 } else { last }, |p| p.saturating_add_signed(step).min(last));
        (self.current, self.scroll_to) = (Some(all[next]), Some(all[next]));
    }

    /// The merged file.
    pub fn text(&self) -> String {
        self.chunks.iter().flat_map(|c| self.result(c)).collect::<Vec<_>>().join("\n")
    }
}

//// The files in conflict under `root` (relative to it) and the versions the index has of each: [base, mine, theirs].
pub fn unmerged(root: &Path) -> Result<Vec<(String, [bool; 3])>, String> {
    let mut v: Vec<(String, [bool; 3])> = vec![];
    for entry in git(root, &["ls-files", "-u", "-z"])?.split('\0') {
        // `mode sha stage<TAB>path`
        let Some((meta, path)) = entry.split_once('\t') else { continue };
        let Some(stage) = meta.rsplit(' ').next().and_then(|s| s.parse::<usize>().ok()).filter(|s| (1..=3).contains(s)) else { continue };
        if v.last().is_none_or(|last| last.0 != path) {
            v.push((path.to_string(), [false; 3]));
        }
        v.last_mut().unwrap().1[stage - 1] = true;
    }
    Ok(v)
}

/// `text` written to `path` (with the line ends the file has on disk) and staged: its conflict is over.
fn write_resolved(root: &Path, path: &Path, text: &str) -> Result<(), String> {
    let crlf = std::fs::read_to_string(path).is_ok_and(|s| s.contains("\r\n"));
    std::fs::write(path, if crlf { text.replace('\n', "\r\n") } else { text.to_string() })
        .map_err(|e| format!("não foi possível gravar {}: {e}", file_name(path)))?;
    git(root, &["add", "--", &rel(root, path)]).map(|_| ())
}

pub(super) fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// The lines of one side of a conflict, for the MCP's listing; long ones are cut.
fn quote(lines: &[String]) -> String {
    const MOST: usize = 30;
    let mut out: Vec<String> = lines.iter().take(MOST).map(|l| format!("    {l}")).collect();
    if lines.is_empty() {
        out.push("    (nada: as linhas saem)".into());
    } else if lines.len() > MOST {
        out.push(format!("    … mais {} linhas", lines.len() - MOST));
    }
    out.join("\n")
}

/// What is still to do in `root` once a file left the conflicts.
fn left(root: &Path) -> String {
    let left = unmerged(root).map_or(0, |v| v.len());
    if left > 0 {
        return format!("Ainda há {} em conflito.", count(left, "arquivo", "arquivos"));
    }
    match git(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]) {
        Ok(_) => "Não há mais conflitos: falta o commit que conclui o merge.".into(),
        Err(_) => "Não há mais conflitos.".into(),
    }
}

impl Merge {
    /// `path` (absolute, under `root`) by chunk, from the three versions the index has of it.
    pub fn read(root: &Path, path: &Path) -> Result<Merge, String> {
        let file = file_name(path);
        // `./`: from `root`, which may sit inside its repository
        let stage = |n: usize| git(root, &["show", &format!(":{n}:./{}", rel(root, path))]).ok().map(|s| s.replace("\r\n", "\n"));
        let (mine, theirs) = match (stage(2), stage(3)) {
            (Some(mine), Some(theirs)) => (mine, theirs),
            (None, None) if stage(1).is_none() => return Err(format!("{file} não está em conflito.")),
            _ => return Err(format!("{file} foi excluído de um dos lados: não há trechos para mesclar. Escolha Aceitar Meus ou Aceitar Deles.")),
        };
        let base = stage(1).unwrap_or_default();
        if [&base, &mine, &theirs].iter().any(|s| s.contains('\0')) {
            return Err(format!("{file} é binário: não há trechos para mesclar. Escolha Aceitar Meus ou Aceitar Deles."));
        }
        Ok(Merge::new(path.to_path_buf(), &base, &mine, &theirs))
    }

    /// "Aplicar": the result goes to the file and to the index. Back: what is still to do.
    pub fn write(&self, root: &Path) -> Result<String, String> {
        if self.pending() > 0 {
            return Err(format!("Ainda há {} por resolver.", count(self.pending(), "conflito", "conflitos")));
        }
        // answered meanwhile somewhere else (a terminal, an agent): that answer stays
        let name = rel(root, &self.path);
        if !unmerged(root)?.iter().any(|(f, _)| *f == name) {
            return Err(format!("{name} não está mais em conflito: já foi resolvido por fora."));
        }
        write_resolved(root, &self.path, &self.text())?;
        Ok(format!("{name} mesclado. {}", left(root)))
    }
}

/// "Aceitar Meus" / "Aceitar Deles": the whole file as one side has it, staged. A side that
/// deleted the file takes it away.
pub fn resolve_side(root: &Path, path: &Path, side: Side) -> Result<String, String> {
    let name = rel(root, path);
    let has = unmerged(root)?.into_iter().find(|(f, _)| *f == name).ok_or(format!("{name} não está em conflito."))?.1;
    let (flag, kept, what) = match side {
        Side::Mine if has[1] => ("--ours", true, "ficou com a minha versão"),
        Side::Theirs if has[2] => ("--theirs", true, "ficou com a versão deles"),
        Side::Mine => ("", false, "foi excluído, como no meu lado"),
        Side::Theirs => ("", false, "foi excluído, como no lado deles"),
    };
    if kept {
        git(root, &["checkout", flag, "--", &name])?;
        git(root, &["add", "--", &name])?;
    } else {
        git(root, &["rm", "-q", "--", &name])?;
    }
    Ok(format!("{name} {what}. {}", left(root)))
}

/// For the MCP: the files in conflict under `root`; with `path`, that file's conflicts, numbered, with each side's lines.
pub fn describe(root: &Path, path: Option<&Path>) -> Result<String, String> {
    let Some(path) = path else {
        let files = unmerged(root)?;
        if files.is_empty() {
            return Ok("Nenhum arquivo em conflito.".into());
        }
        let line = |(f, has): &(String, [bool; 3])| match (has[1], has[2]) {
            (true, true) => format!("{f} (os dois lados mudaram)"),
            (false, true) => format!("{f} (excluído no meu lado, mudou no deles)"),
            (true, false) => format!("{f} (mudou no meu lado, excluído no deles)"),
            _ => format!("{f} (excluído nos dois lados)"),
        };
        return Ok(format!("Arquivos em conflito:\n{}", files.iter().map(line).collect::<Vec<_>>().join("\n")));
    };
    let merge = Merge::read(root, path)?;
    let conflicts = merge.conflicts();
    let mut out = vec![format!(
        "{}: {} (as alterações sem conflito dos dois lados já entram no resultado).",
        rel(root, path),
        count(conflicts.len(), "conflito", "conflitos")
    )];
    for (n, &k) in conflicts.iter().enumerate() {
        let c = &merge.chunks[k];
        out.push(format!(
            "Conflito {}, linha {} da minha versão:\n  minha:\n{}\n  deles:\n{}",
            n + 1,
            c.mine.start + 1,
            quote(merge.side(c, Side::Mine)),
            quote(merge.side(c, Side::Theirs))
        ));
    }
    Ok(out.join("\n"))
}

/// For the MCP: `path` resolved as `args` say: `side` (mine, theirs) takes the whole file; `picks`
/// answers each conflict in order (mine, theirs, both, both_theirs_first, base) over the automatic merge.
pub fn resolve(root: &Path, path: &Path, args: &serde_json::Value) -> Result<String, String> {
    match args["side"].as_str().unwrap_or("") {
        "mine" => return resolve_side(root, path, Side::Mine),
        "theirs" => return resolve_side(root, path, Side::Theirs),
        "" => {}
        other => return Err(format!("lado desconhecido: {other} (use mine ou theirs)")),
    }
    let picks = args["picks"].as_array().ok_or("diga side (mine ou theirs) ou picks (uma escolha por conflito)")?;
    let mut merge = Merge::read(root, path)?;
    let conflicts = merge.conflicts();
    if picks.len() != conflicts.len() {
        return Err(format!(
            "{} tem {} e vieram {} em picks: mande uma por conflito, na ordem em que conflicts os lista.",
            rel(root, path),
            count(conflicts.len(), "conflito", "conflitos"),
            picks.len()
        ));
    }
    for (&k, pick) in conflicts.iter().zip(picks) {
        merge.pick(
            k,
            match pick.as_str().unwrap_or("") {
                "mine" => &[Side::Mine],
                "theirs" => &[Side::Theirs],
                "both" => &[Side::Mine, Side::Theirs],
                "both_theirs_first" => &[Side::Theirs, Side::Mine],
                "base" => &[],
                other => return Err(format!("escolha desconhecida: {other} (use mine, theirs, both, both_theirs_first ou base)")),
            },
        );
    }
    merge.write(root)
}

/// `1 conflito`, `3 conflitos`.
pub(super) fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
