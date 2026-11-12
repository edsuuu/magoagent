use super::*;

/// What `from` can reach: everything when the caller is outside the canvas, else what is
/// connected to it, plus the notes chained to its notes (note to note, like the reference app).
pub(super) fn reachable(nodes: &[Node], from: &str) -> Vec<Node> {
    if chat::outside(nodes, from) {
        return nodes.iter().filter(|n| n.id != from).cloned().collect();
    }
    let links = nodes.iter().find(|n| n.id == from).map(|n| n.links.clone()).unwrap_or_default();
    let mut out: Vec<Node> = nodes.iter().filter(|n| n.kind != NodeKind::Note && links.contains(&n.id)).cloned().collect();
    let mut notes = note_tree(nodes, from);
    // `find` takes the first match: the nearest note of a name wins
    notes.sort_by_key(|(depth, _)| *depth);
    out.extend(notes.into_iter().map(|(_, n)| n.clone()));
    out
}

/// The notes `from` reaches, each followed by the notes chained to it one level deeper (all the
/// notes, from outside the canvas).
pub(super) fn note_tree<'a>(nodes: &'a [Node], from: &str) -> Vec<(usize, &'a Node)> {
    let note = |id: &str| nodes.iter().find(|n| n.id == id && n.kind == NodeKind::Note);
    let caller = nodes.iter().find(|n| n.id == from);
    let roots: Vec<&Node> = match caller {
        _ if chat::outside(nodes, from) => nodes.iter().filter(|n| n.kind == NodeKind::Note).collect(),
        Some(caller) => caller.links.iter().filter_map(|l| note(l)).collect(),
        // a caller the canvas no longer has reaches nothing, like `reachable`
        None => Vec::new(),
    };
    // a note roped straight to the caller is a root, never a sibling's child
    let direct = |id: &str| caller.is_some_and(|c| c.links.iter().any(|l| l == id));
    let (mut out, mut seen) = (Vec::new(), std::collections::HashSet::new());
    let mut stack: Vec<(usize, &Node)> = roots.into_iter().rev().map(|n| (0, n)).collect();
    while let Some((depth, n)) = stack.pop() {
        if seen.insert(n.id.as_str()) {
            out.push((depth, n));
            stack.extend(n.links.iter().rev().filter_map(|l| note(l)).filter(|m| !seen.contains(m.id.as_str()) && !direct(&m.id)).map(|m| (depth + 1, m)));
        }
    }
    out
}

/// The portal a vault login goes to: the one open on that site, else the only one the caller reaches.
pub(super) fn portal_of_site(nodes: &[Node], from: &str, site: &str) -> Result<Node, String> {
    let portals: Vec<Node> = reachable(nodes, from).into_iter().filter(|n| n.kind == NodeKind::Portal).collect();
    let site = site.trim().to_lowercase();
    let open = portals.iter().find(|n| !site.is_empty() && n.text.to_lowercase().contains(&site));
    let only = || if let [one] = &portals[..] { Some(one) } else { None };
    open.or_else(only)
        .cloned()
        .ok_or_else(|| format!("Nenhum navegador aberto em {site}: abra o site num navegador ligado a você (portal_navigate) e tente de novo."))
}

/// The node of that name and kind `from` reaches; an agent (`Terminal`) is a terminal's or a chat's.
pub(super) fn find(nodes: &[Node], from: &str, name: &str, kind: NodeKind) -> Result<Node, String> {
    let agent = |k: NodeKind| matches!(k, NodeKind::Terminal | NodeKind::Chat);
    let candidates: Vec<Node> = reachable(nodes, from).into_iter().filter(|n| n.kind == kind || (agent(kind) && agent(n.kind))).collect();
    let exact = candidates.iter().find(|n| n.name == name);
    let loose = || candidates.iter().find(|n| n.name.to_lowercase() == name.trim().to_lowercase());
    exact.or_else(loose).cloned().ok_or_else(|| {
        let what = match kind {
            NodeKind::Terminal | NodeKind::Chat => "agente",
            NodeKind::Note => "nota",
            NodeKind::Portal => "portal",
            NodeKind::Ide => "IDE",
            NodeKind::Tree => "árvore de arquivos",
        };
        let names: Vec<String> = candidates.iter().map(|n| format!("\"{}\"", n.name)).collect();
        let options = if names.is_empty() { "nenhum".to_string() } else { names.join(", ") };
        format!("Nenhum {what} \"{name}\" conectado. Disponíveis: {options}.")
    })
}

/// The IDE an agent means: by name, or the only one it reaches when it names none.
pub(super) fn find_ide(nodes: &[Node], from: &str, name: &str) -> Result<Node, String> {
    if name.trim().is_empty()
        && let [ide] = &reachable(nodes, from).into_iter().filter(|n| n.kind == NodeKind::Ide).collect::<Vec<_>>()[..]
    {
        return Ok(ide.clone());
    }
    find(nodes, from, name, NodeKind::Ide)
}

/// An agent's path in an IDE: relative to its project folder, or absolute inside it; never above it.
pub(super) fn ide_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path.trim());
    if path.as_os_str().is_empty() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("diga um caminho dentro do projeto, sem `..`".into());
    }
    let full: PathBuf = root.join(path).components().collect();
    // Windows paths ignore case (c:\proj\a.rs is C:\Proj\a.rs); whole components, so C:\Proj-x stays out
    let key =
        |c: std::path::Component| if cfg!(windows) { c.as_os_str().to_string_lossy().to_lowercase() } else { c.as_os_str().to_string_lossy().into_owned() };
    let n = root.components().count();
    let inside = full.components().count() >= n && full.components().zip(root.components()).all(|(a, b)| key(a) == key(b));
    // the project's own spelling, so an open tab of the same file is found
    inside
        .then(|| root.components().chain(full.components().skip(n)).collect())
        .ok_or_else(|| format!("{} está fora do projeto ({})", path.display(), root.display()))
}
