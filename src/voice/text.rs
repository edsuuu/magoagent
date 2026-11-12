/// The sentences of an answer, to say one at a time; Markdown marks and code blocks left out.
pub(crate) fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (k, part) in text.split("```").enumerate() {
        if k % 2 == 1 {
            continue;
        }
        let plain: String = part
            .lines()
            .map(|l| l.trim_start_matches(['#', '>', ' ', '-', '*']).trim())
            .filter(|l| !l.is_empty() && !l.starts_with('|'))
            .collect::<Vec<_>>()
            .join("\n")
            .replace("**", "")
            .replace('`', "");
        let mut current = String::new();
        let mut chars = plain.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\n' {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
                current.clear();
                continue;
            }
            current.push(ch);
            // a mark ends it only before a space: "chat.rs", "3.5" go on
            if matches!(ch, '.' | '!' | '?' | '…') && chars.peek().is_none_or(|c| c.is_whitespace()) && current.trim().chars().count() > 2 {
                out.push(current.trim().to_string());
                current.clear();
            }
        }
        if !current.trim().is_empty() {
            out.push(current.trim().to_string());
        }
    }
    out
}

/// What is said of an answer: its first three sentences, but none with a path, a file name or a
/// link; when the answer has more, "Os detalhes estão na tela." closes it.
pub(crate) fn spoken(text: &str) -> Vec<String> {
    let all = sentences(text);
    let mut out: Vec<String> = all.iter().take(3).filter(|s| !has_path(s)).cloned().collect();
    if out.len() < all.len() || text.contains("```") || text.lines().any(|l| l.trim_start().starts_with('|')) {
        out.push("Os detalhes estão na tela.".into());
    }
    out
}

/// Whether a sentence has what can't be said: a link, a path, a file name ("chat.rs:42").
pub(crate) fn has_path(sentence: &str) -> bool {
    sentence.split_whitespace().any(|w| {
        if w.contains("://") || w.contains('/') || w.contains('\\') || w.contains("::") {
            return true;
        }
        let w = w.trim_matches(|c: char| !c.is_alphanumeric()).split(':').next().unwrap_or_default();
        w.rsplit_once('.').is_some_and(|(name, ext)| {
            name.chars().any(char::is_alphabetic)
                && (1..=5).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic())
        })
    })
}
