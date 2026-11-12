//! The agent's text cut into what the chat draws: words, code blocks, pictures (`![alt](src)`) and
//! files (a line that is only a link to one, or only a path in backticks). Pure: the pictures and
//! files are looked up when drawn (chat_media.rs).

use crate::prelude::*;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Seg {
    /// Markdown words, trimmed.
    Text(String),
    /// A fenced block: the first line's words after the fence, and the code.
    Code {
        info: String,
        code: String,
    },
    Image {
        alt: String,
        src: String,
    },
    /// `linked`: written as a link, so shown even when the file isn't there.
    File {
        label: String,
        path: String,
        linked: bool,
    },
}

impl Seg {
    /// The markdown it came from, for a place that draws no pictures or files.
    pub(crate) fn raw(&self) -> String {
        match self {
            Seg::Text(text) => text.clone(),
            Seg::Code { info, code } => format!("```{info}\n{code}\n```"),
            Seg::Image { alt, src } => format!("![{alt}]({src})"),
            Seg::File { label, path, linked: true } => format!("[{label}]({path})"),
            Seg::File { path, .. } => format!("`{path}`"),
        }
    }
}

/// Cuts `text` into segments, in order. Fences are lines that start with ``` (an unclosed one runs
/// to the end, as while the answer still streams).
pub(crate) fn segments(text: &str) -> Vec<Seg> {
    let (mut out, mut prose, mut code) = (Vec::new(), Vec::<&str>::new(), Vec::<&str>::new());
    let (mut info, mut inside) = (String::new(), false);
    fn words(out: &mut Vec<Seg>, prose: &mut Vec<&str>) {
        let joined = prose.join("\n");
        if !joined.trim().is_empty() {
            out.push(Seg::Text(joined.trim().to_string()));
        }
        prose.clear();
    }
    fn block(out: &mut Vec<Seg>, code: &mut Vec<&str>, info: &str) {
        if code.iter().any(|l| !l.trim().is_empty()) {
            out.push(Seg::Code { info: info.to_string(), code: code.join("\n").trim_end().to_string() });
        }
        code.clear();
    }
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("```") {
            if inside {
                block(&mut out, &mut code, &info);
            } else {
                words(&mut out, &mut prose);
                info = rest.trim().to_string();
            }
            inside = !inside;
        } else if inside {
            code.push(line);
        } else if let Some(seg) = file_line(t) {
            words(&mut out, &mut prose);
            out.push(seg);
        } else if t.contains("![") && images_in(line).iter().any(|p| matches!(p, Piece::Image(..))) {
            // a picture in the middle of a line: the words around it stay words
            for piece in images_in(line) {
                match piece {
                    Piece::Text(words_) => prose.push(words_),
                    Piece::Image(alt, src) => {
                        words(&mut out, &mut prose);
                        out.push(Seg::Image { alt, src });
                    }
                }
            }
        } else {
            prose.push(line);
        }
    }
    if inside {
        block(&mut out, &mut code, &info);
    } else {
        words(&mut out, &mut prose);
    }
    out
}

enum Piece<'a> {
    Text(&'a str),
    Image(String, String),
}

/// A line cut at its `![alt](src)` pictures; the text between (when not blank) comes as pieces.
fn images_in(line: &str) -> Vec<Piece<'_>> {
    let (mut out, mut rest) = (Vec::new(), line);
    while let Some(start) = rest.find("![") {
        let after = &rest[start + 2..];
        let Some(mid) = after.find("](") else { break };
        let Some(end) = after[mid + 2..].find(')') else { break };
        let (alt, src) = (&after[..mid], &after[mid + 2..mid + 2 + end]);
        if !rest[..start].trim().is_empty() {
            out.push(Piece::Text(&rest[..start]));
        }
        out.push(Piece::Image(alt.to_string(), target(src)));
        rest = &after[mid + 2 + end + 1..];
    }
    if !rest.trim().is_empty() {
        out.push(Piece::Text(rest));
    }
    out
}

/// A link's target without its title (`path "title"`) or its angle brackets (`<a path>`).
fn target(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('<').and_then(|r| r.split_once('>')) {
        return inner.0.to_string();
    }
    raw.split_once(" \"").map_or(raw, |(path, _)| path).trim().to_string()
}

/// A whole line that names a file: `[name](path)` or `` `path` `` (a path with a folder or an
/// extension, no spaces).
fn file_line(t: &str) -> Option<Seg> {
    if let Some(inner) = t.strip_prefix('[').filter(|_| t.ends_with(')')) {
        let (label, rest) = inner.split_once("](")?;
        let path = target(rest.strip_suffix(')')?);
        // one link, not several: `[a](x) e [b](y)` is a sentence
        return (!label.contains(']') && !rest.contains("](") && is_local(&path)).then(|| Seg::File { label: label.to_string(), path, linked: true });
    }
    let path = t.strip_prefix('`')?.strip_suffix('`')?;
    let ext = path.rsplit_once('.').map(|(_, e)| e).filter(|e| (1..=8).contains(&e.len()) && e.bytes().all(|b| b.is_ascii_alphanumeric()))?;
    let _ = ext;
    (!path.contains(['`', ' ']) && path.contains(['/', '\\']) && is_local(path)).then(|| Seg::File {
        label: path.rsplit(['/', '\\']).next().unwrap_or(path).to_string(),
        path: path.to_string(),
        linked: false,
    })
}

/// Not an address (`https://…`, `mailto:`, `data:`) and not a page anchor.
pub(crate) fn is_local(target: &str) -> bool {
    let t = target.trim();
    let is_scheme = t.split_once(':').is_some_and(|(s, _)| s.len() > 1 && s.bytes().all(|b| b.is_ascii_alphabetic()) && s != "file");
    !t.is_empty() && !t.starts_with('#') && !is_scheme
}

/// `file:///C:/a%20b.png`, `~/x.png`, `a/b.png` (from `base`) as a path on disk.
pub(crate) fn resolve(target: &str, base: Option<&Path>) -> PathBuf {
    let t = target.trim();
    let t = t
        .strip_prefix("file:///")
        .map(|r| if cfg!(windows) { r.to_string() } else { format!("/{r}") })
        .or_else(|| t.strip_prefix("file://").map(str::to_string));
    let t = percent_decode(t.as_deref().unwrap_or(target.trim()));
    let path = match t.strip_prefix("~/").and_then(|rest| std::env::home_dir().map(|h| h.join(rest))) {
        Some(home) => home,
        None => PathBuf::from(&t),
    };
    match base {
        Some(base) if path.is_relative() => base.join(path),
        _ => path,
    }
}

fn percent_decode(text: &str) -> String {
    if !text.contains('%') {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(bytes.len()), 0);
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether the path's extension is a picture the chat shows.
pub(crate) fn is_picture(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

/// The bytes of a `data:image/…;base64,` address, when it is one.
pub(crate) fn data_uri(src: &str) -> Option<&str> {
    let (head, data) = src.strip_prefix("data:image/")?.split_once(',')?;
    head.ends_with(";base64").then_some(data)
}

/// A file's weight as the file card writes it: decimal units, KB without decimals, MB with one.
pub(crate) fn file_size(bytes: u64) -> String {
    let (mut value, mut unit) = (bytes as f64, 0);
    while value >= 1000.0 && unit < 4 {
        value /= 1000.0;
        unit += 1;
    }
    let digits = [0, 0, 1, 2, 2][unit];
    let number = format!("{value:.digits$}").replace('.', ",");
    let number = if digits > 0 { number.trim_end_matches('0').trim_end_matches(',').to_string() } else { number };
    match unit {
        0 => format!("{number} {}", if bytes == 1 { "byte" } else { "bytes" }),
        _ => format!("{number} {}", ["", "KB", "MB", "GB", "TB"][unit]),
    }
}

/// `text` with its middle replaced by "…" until `fits`: the beginning and the last 12 letters
/// stay (an extension and the end of a name matter more than the middle).
pub(crate) fn middle(text: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let tail = 12.min(chars.len().div_ceil(2));
    let tail: String = chars[chars.len() - tail..].iter().collect();
    for head in (0..chars.len() - tail.chars().count()).rev() {
        let shown = format!("{}…{tail}", chars[..head].iter().collect::<String>());
        if fits(&shown) {
            return shown;
        }
    }
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Seg {
        Seg::Text(s.into())
    }

    #[test]
    fn words_code_pictures_and_files_come_apart_in_order() {
        let md = "Olá **mundo**\n\n```rust\nfn a() {}\n```\n\n![gráfico](out/g.png)\n\n[relatório.md](docs/r.md)\n\nfim";
        assert_eq!(
            segments(md),
            vec![
                text("Olá **mundo**"),
                Seg::Code { info: "rust".into(), code: "fn a() {}".into() },
                Seg::Image { alt: "gráfico".into(), src: "out/g.png".into() },
                Seg::File { label: "relatório.md".into(), path: "docs/r.md".into(), linked: true },
                text("fim"),
            ]
        );
    }

    #[test]
    fn a_picture_in_a_sentence_keeps_the_words_around_it() {
        let got = segments("Veja ![a](x.png) agora");
        assert_eq!(got, vec![text("Veja"), Seg::Image { alt: "a".into(), src: "x.png".into() }, text("agora")]);
    }

    #[test]
    fn titles_angle_brackets_and_data_addresses_are_read() {
        assert_eq!(target("a b.png \"título\""), "a b.png");
        assert_eq!(target("<C:/Minha pasta/a.png>"), "C:/Minha pasta/a.png");
        assert_eq!(data_uri("data:image/png;base64,AAAA"), Some("AAAA"));
        assert_eq!(data_uri("https://x/a.png"), None);
    }

    #[test]
    fn only_a_whole_line_link_to_a_local_file_is_a_card() {
        assert!(matches!(segments("[a](https://x.com/a)").as_slice(), [Seg::Text(_)]));
        assert!(matches!(segments("[a](#topo)").as_slice(), [Seg::Text(_)]));
        assert!(matches!(segments("[a](x.md) e [b](y.md)").as_slice(), [Seg::Text(_)]));
        assert!(matches!(segments("veja [a](x.md) aqui").as_slice(), [Seg::Text(_)]));
        assert!(matches!(segments("`src/main.rs`").as_slice(), [Seg::File { linked: false, .. }]));
        // a word in backticks is not a path
        assert!(matches!(segments("`cargo`").as_slice(), [Seg::Text(_)]));
        assert!(matches!(segments("`a b/c.rs`").as_slice(), [Seg::Text(_)]));
    }

    #[test]
    fn an_unclosed_fence_is_code_and_an_empty_one_is_nothing() {
        assert_eq!(segments("```py\nprint(1)"), vec![Seg::Code { info: "py".into(), code: "print(1)".into() }]);
        assert!(segments("```\n\n```").is_empty());
        // pictures inside code stay code
        assert!(matches!(segments("```\n![a](b.png)\n```").as_slice(), [Seg::Code { .. }]));
    }

    #[test]
    fn paths_resolve_from_the_chats_folder_and_decode_percent() {
        let base = Path::new("/proj");
        assert_eq!(resolve("a/b.png", Some(base)), base.join("a/b.png"));
        assert_eq!(resolve("a%20b.png", Some(base)), base.join("a b.png"));
        assert_eq!(resolve("file:///tmp/x%20y.png", None), PathBuf::from(if cfg!(windows) { "tmp/x y.png" } else { "/tmp/x y.png" }));
        assert!(is_local("a/b.png") && is_local("C:/x.png") && is_local("file:///x.png") && !is_local("https://a") && !is_local("#a"));
    }

    #[test]
    fn sizes_read_as_the_file_card_writes_them() {
        assert_eq!(file_size(1), "1 byte");
        assert_eq!(file_size(12), "12 bytes");
        assert_eq!(file_size(1_500), "2 KB");
        assert_eq!(file_size(1_260_000), "1,3 MB");
        assert_eq!(file_size(2_000_000), "2 MB");
    }

    #[test]
    fn a_long_name_loses_its_middle_and_keeps_its_end() {
        let name = "relatorio-final-do-trimestre-v2.pdf";
        let shown = middle(name, |s| s.chars().count() <= 20);
        assert!(shown.chars().count() <= 20 && shown.contains('…') && shown.ends_with("trimestre-v2.pdf".get(4..).unwrap()));
        assert_eq!(middle("a.txt", |_| true), "a.txt");
    }

    #[test]
    fn a_streamed_answer_gives_words_with_a_picture_a_step_and_a_read_image() {
        use crate::transcript::{ChatItem, chat_items};
        let lines = [
            serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":"Pronto:\n\n![gráfico](out/g.png)\n\n[r.md](docs/r.md)"}]}}),
            serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"w1","name":"Write","input":{"file_path":"out/g.png","content":"x"}}]}}),
            serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"r1","content":[{"type":"image","source":{"type":"base64","media_type":"image/png","data":"AAAA"}}]}]}}),
        ];
        let items: Vec<ChatItem> = lines.iter().flat_map(chat_items).collect();
        let ChatItem::Agent(text) = &items[0] else { panic!("{items:?}") };
        assert!(matches!(segments(text).as_slice(), [Seg::Text(_), Seg::Image { .. }, Seg::File { .. }]));
        assert!(matches!(&items[1], ChatItem::Tool(s) if s.name == "Write" && s.files == ["out/g.png"]));
        assert!(items.iter().any(|i| matches!(i, ChatItem::Image { media, .. } if media == "image/png")));
    }
}
