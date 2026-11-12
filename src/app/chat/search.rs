//! Searching the chats' conversations: a term found whatever its case or accents ("revisao" finds
//! "Revisão"), with the text around it.

/// One message to search: who said it and what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SearchMessage<'a> {
    pub(crate) who: &'a str,
    pub(crate) text: &'a str,
}

/// A message that has the term: who said it and the text around the first place it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SearchHit {
    pub(crate) who: String,
    pub(crate) excerpt: String,
}

/// How many characters of the message go before and after the term.
const AROUND: usize = 60;

/// The letter without its accent.
// ponytail: the accents of Portuguese (and the umlauts); other scripts match by case only
pub(crate) fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        _ => c,
    }
}

/// Each character lowercased and without accent, with the bytes of `s` it came from.
fn folded(s: &str) -> Vec<(char, usize, usize)> {
    s.char_indices()
        .flat_map(|(start, c)| {
            let end = start + c.len_utf8();
            c.to_lowercase().map(move |lower| (fold(lower), start, end))
        })
        .collect()
}

/// The text around `start..end`, `AROUND` characters to each side, cut at characters (never inside
/// one), with an ellipsis where it was cut and in one line.
fn excerpt(text: &str, start: usize, end: usize) -> String {
    let before = text[..start].char_indices().map(|(i, _)| i).collect::<Vec<_>>();
    let after = text[end..].char_indices().map(|(i, _)| end + i).collect::<Vec<_>>();
    let left = before.len().saturating_sub(AROUND);
    let begin = before.get(left).copied().unwrap_or(start);
    let finish = if after.len() > AROUND { after[AROUND] } else { text.len() };
    let line = text[begin..finish].split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{line}{}", if left > 0 { "…" } else { "" }, if after.len() > AROUND { "…" } else { "" })
}

/// The messages that have `term`, in order: one hit per message, at the first place it shows.
pub(crate) fn search(messages: &[SearchMessage<'_>], term: &str) -> Vec<SearchHit> {
    let needle = folded(term.trim()).into_iter().map(|x| x.0).collect::<Vec<_>>();
    if needle.is_empty() {
        return Vec::new();
    }
    messages
        .iter()
        .filter_map(|message| {
            let chars = folded(message.text);
            let pos = chars.windows(needle.len()).position(|w| w.iter().map(|x| x.0).eq(needle.iter().copied()))?;
            let (start, end) = (chars[pos].1, chars[pos + needle.len() - 1].2);
            Some(SearchHit { who: message.who.to_string(), excerpt: excerpt(message.text, start, end) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(text: &str, term: &str) -> String {
        search(&[SearchMessage { who: "Pessoa", text }], term).first().unwrap().excerpt.clone()
    }

    #[test]
    fn the_search_ignores_case_and_accents_and_gives_one_hit_per_message() {
        let messages = [
            SearchMessage { who: "Agente", text: "Revisão e revisao" },
            SearchMessage { who: "Pessoa", text: "nada aqui" },
            SearchMessage { who: "Pessoa", text: "A AÇÃO" },
        ];
        let hits = search(&messages, " REVISAO ");
        assert_eq!(hits, [SearchHit { who: "Agente".into(), excerpt: "Revisão e revisao".into() }]);
        assert_eq!(search(&messages, "ação").len(), 1);
        assert_eq!(search(&messages, "acao")[0].who, "Pessoa");
        assert!(search(&messages, "").is_empty() && search(&messages, "   ").is_empty());
        assert!(search(&messages, "ausente").is_empty());
    }

    #[test]
    fn the_excerpt_is_cut_around_the_term_at_whole_characters() {
        assert_eq!(hit("alvo no começo", "alvo"), "alvo no começo");
        let middle = hit(&format!("{}alvo{}", "x".repeat(70), "y".repeat(70)), "alvo");
        assert_eq!(middle, format!("…{}alvo{}…", "x".repeat(60), "y".repeat(60)));
        assert_eq!(hit(&format!("{}alvo", "x".repeat(70)), "alvo"), format!("…{}alvo", "x".repeat(60)));
        // characters of several bytes on both sides, and a term that is itself accented
        let wide = hit(&format!("{}🙂漢é{}álvo{}", "x".repeat(70), "界".repeat(4), "界".repeat(70)), "ALVO");
        assert!(wide.starts_with('…') && wide.contains("🙂漢é界界界界álvo") && wide.ends_with(&format!("{}…", "界".repeat(60))), "{wide}");
        // a message of several lines comes in one
        assert_eq!(hit("primeira linha\n\n  o alvo\tfica aqui\n", "alvo"), "primeira linha o alvo fica aqui");
    }
}
