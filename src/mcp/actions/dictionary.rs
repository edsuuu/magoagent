//! Ajustes › Dicionário through the MCP's `dictionary`: the words the ears spell right and what they write for what they hear.

use crate::prelude::*;

impl DrawingApp {
    pub(super) fn act_dictionary(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let dictionary = &mut self.prefs.dictionary;
        match text("action").as_str() {
            "" | "list" => Ok(dictionary.summary()),
            "add_word" if dictionary.add_word(&text("word")) => Ok(format!("{} entrou no dicionário.", text("word"))),
            "add_word" => Err("diga a palavra em `word` (e ela já pode estar no dicionário)".into()),
            "remove_word" if dictionary.remove_word(&text("word")) => Ok(format!("{} saiu do dicionário.", text("word"))),
            "remove_word" => Err(format!("{} não está no dicionário", text("word"))),
            "add_replacement" if dictionary.add_replacement(&text("heard"), &text("written")) => {
                Ok(format!("Quando ouvir “{}”, escreve “{}”.", text("heard"), text("written")))
            }
            "add_replacement" => Err("diga o que é ouvido em `heard` e o que escrever em `written`".into()),
            "remove_replacement" if dictionary.remove_replacement(&text("heard")) => Ok(format!("A troca de “{}” saiu.", text("heard"))),
            "remove_replacement" => Err(format!("não há troca para “{}”", text("heard"))),
            other => Err(format!("ação desconhecida: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maestro_edits_the_dictionary() {
        let mut app = DrawingApp::default();
        app.act_dictionary(&json!({ "action": "add_word", "word": "Supabase" })).unwrap();
        app.act_dictionary(&json!({ "action": "add_replacement", "heard": "cloud code", "written": "Claude Code" })).unwrap();
        assert_eq!(app.prefs.dictionary.apply("o cloud code e o supabase"), "o Claude Code e o Supabase");
        assert!(app.act_dictionary(&json!({ "action": "list" })).unwrap().contains("Supabase"));
        assert!(app.act_dictionary(&json!({ "action": "add_word", "word": "supabase" })).is_err());
        app.act_dictionary(&json!({ "action": "remove_word", "word": "SUPABASE" })).unwrap();
        app.act_dictionary(&json!({ "action": "remove_replacement", "heard": "Cloud Code" })).unwrap();
        assert!(app.prefs.dictionary.words.is_empty() && app.prefs.dictionary.replacements.is_empty());
        assert!(app.act_dictionary(&json!({ "action": "x" })).is_err());
    }
}
