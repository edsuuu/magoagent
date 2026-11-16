//! The ears' dictionary (`Prefs.dictionary`, edited through the MCP's `dictionary`): the names they must spell right,
//! and what to write for what they hear. The words go to the engine as a hint of its vocabulary (`vocabulary`), and
//! every stretch heard passes `apply` before it is used.

use crate::prelude::*;

/// "Heard" becomes "written": `cloud code` → `Claude Code`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Replacement {
    pub(crate) from: String,
    pub(crate) to: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Dictionary {
    pub(crate) words: Vec<String>,
    pub(crate) replacements: Vec<Replacement>,
}

/// The hint's size limit: engines take a short prompt (OpenAI's reads the last 224 tokens).
const VOCABULARY_LIMIT: usize = 600;

fn same(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// `text` with every whole-word, case-insensitive `from` written as `to`.
fn replace_phrase(text: &str, from: &str, to: &str) -> String {
    let (chars, wanted): (Vec<char>, Vec<char>) = (text.chars().collect(), from.chars().collect());
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < chars.len() {
        let fits = at + wanted.len() <= chars.len() && chars[at..at + wanted.len()].iter().zip(&wanted).all(|(a, b)| same(*a, *b));
        let starts = at == 0 || !chars[at - 1].is_alphanumeric();
        let ends = chars.get(at + wanted.len()).is_none_or(|c| !c.is_alphanumeric());
        if !wanted.is_empty() && fits && starts && ends {
            out.push_str(to);
            at += wanted.len();
        } else {
            out.push(chars[at]);
            at += 1;
        }
    }
    out
}

impl Dictionary {
    /// Adds a word the ears must spell right; false when it is empty or already there (in any case).
    pub(crate) fn add_word(&mut self, word: &str) -> bool {
        let word = word.trim();
        if word.is_empty() || self.words.iter().any(|w| w.to_lowercase() == word.to_lowercase()) {
            return false;
        }
        self.words.push(word.into());
        true
    }

    pub(crate) fn remove_word(&mut self, word: &str) -> bool {
        let before = self.words.len();
        self.words.retain(|w| w.to_lowercase() != word.trim().to_lowercase());
        self.words.len() != before
    }

    /// Adds "heard → written"; the same "heard" (in any case) is replaced. False when either side is empty.
    pub(crate) fn add_replacement(&mut self, from: &str, to: &str) -> bool {
        let (from, to) = (from.trim(), to.trim());
        if from.is_empty() || to.is_empty() {
            return false;
        }
        self.replacements.retain(|r| r.from.to_lowercase() != from.to_lowercase());
        self.replacements.push(Replacement { from: from.into(), to: to.into() });
        true
    }

    pub(crate) fn remove_replacement(&mut self, from: &str) -> bool {
        let before = self.replacements.len();
        self.replacements.retain(|r| r.from.to_lowercase() != from.trim().to_lowercase());
        self.replacements.len() != before
    }

    /// What the engine is told it may hear, empty without words. Nothing that curl's form syntax reads (`;`, quotes) goes in.
    pub(crate) fn vocabulary(&self) -> String {
        let mut hint = String::new();
        for word in self.words.iter().map(|w| w.replace([';', '"', '\\', '\n'], " ")) {
            let next = if hint.is_empty() { word.clone() } else { format!("{hint}, {word}") };
            if next.chars().count() > VOCABULARY_LIMIT {
                break;
            }
            hint = next;
        }
        if hint.is_empty() { hint } else { format!("Vocabulário: {hint}.") }
    }

    /// `heard` as the person wants it written: the replacements (longest "heard" first), then each word in its own spelling.
    pub(crate) fn apply(&self, heard: &str) -> String {
        let mut rules: Vec<(&str, &str)> = self.replacements.iter().map(|r| (r.from.as_str(), r.to.as_str())).collect();
        rules.sort_by_key(|(from, _)| std::cmp::Reverse(from.chars().count()));
        let mut text = rules.iter().fold(heard.to_string(), |text, (from, to)| replace_phrase(&text, from, to));
        for word in &self.words {
            text = replace_phrase(&text, word, word);
        }
        text
    }

    /// One line of what is in it, for the Maestro.
    pub(crate) fn summary(&self) -> String {
        let words = if self.words.is_empty() { "nenhuma palavra".to_string() } else { self.words.join(", ") };
        let replacements: Vec<String> = self.replacements.iter().map(|r| format!("{} → {}", r.from, r.to)).collect();
        format!("Palavras: {words}. Trocas: {}.", if replacements.is_empty() { "nenhuma".into() } else { replacements.join("; ") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacements_swap_whole_words_in_any_case() {
        let mut dictionary = Dictionary::default();
        assert!(dictionary.add_replacement("cloud code", "Claude Code"));
        assert!(dictionary.add_replacement("ok", "OK"));
        assert_eq!(dictionary.apply("abre o Cloud Code, ok? tokyo"), "abre o Claude Code, OK? tokyo");
        // the same "heard" is replaced, not doubled
        assert!(dictionary.add_replacement("CLOUD CODE", "Claude"));
        assert_eq!(dictionary.replacements.len(), 2);
        assert!(!dictionary.add_replacement("", "x") && !dictionary.add_replacement("x", " "));
        assert!(dictionary.remove_replacement("Ok") && !dictionary.remove_replacement("ok"));
    }

    #[test]
    fn a_word_is_always_written_as_it_was_added() {
        let mut dictionary = Dictionary::default();
        assert!(dictionary.add_word("Supabase") && !dictionary.add_word("supabase") && !dictionary.add_word("  "));
        assert_eq!(dictionary.apply("usei o supabase e o SUPABASE"), "usei o Supabase e o Supabase");
        assert!(dictionary.remove_word("SUPABASE") && dictionary.words.is_empty());
    }

    #[test]
    fn the_vocabulary_is_a_short_safe_hint() {
        let mut dictionary = Dictionary::default();
        assert_eq!(dictionary.vocabulary(), "");
        dictionary.add_word("Supabase");
        dictionary.add_word("a;b\"c");
        assert_eq!(dictionary.vocabulary(), "Vocabulário: Supabase, a b c.");
        for n in 0..200 {
            dictionary.add_word(&format!("palavra{n}"));
        }
        assert!(dictionary.vocabulary().chars().count() < VOCABULARY_LIMIT + 20);
    }

    #[test]
    fn it_survives_an_old_preferences_file() {
        let prefs: Dictionary = serde_json::from_str(r#"{"words":["Mago"]}"#).unwrap();
        assert_eq!((prefs.words.len(), prefs.replacements.len()), (1, 0));
    }
}
