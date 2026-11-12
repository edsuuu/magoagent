/// A voice command the person says to the call, by the agent's name ("Claude, se muta").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VoiceCmd {
    /// Stop listening to the conversation: answers wait with a tone until called by name.
    Mute,
    /// Back to listening to everything; what was held is said.
    Unmute,
    /// Say what is held.
    Speak,
    /// Called by name with a request: it goes to the agent even on standby.
    Ask(String),
}

/// What `text` tells the agent named `name` ("claude", "codex"), when it is called by name.
pub(crate) fn voice_command(text: &str, name: &str) -> Option<VoiceCmd> {
    let clean: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' }).collect();
    let words: Vec<&str> = clean.split_whitespace().collect();
    let aliases: &[&str] = match name {
        "codex" => &["codex", "códex", "kodex", "codecs"],
        _ => &["claude", "cloud", "claud", "claudio", "clode", "cláudio", "claudio"],
    };
    // "ei Claude", "ok Claude", "hey Claude"
    let start = words.iter().position(|w| aliases.contains(w)).filter(|&i| i <= 1)?;
    let rest = words[start + 1..].join(" ");
    let has = |options: &[&str]| options.iter().any(|o| rest.contains(o));
    if has(&["desmuta", "desmute", "unmute", "pode ouvir", "volta a ouvir", "acorda", "liga o microfone", "wake up"]) {
        return Some(VoiceCmd::Unmute);
    }
    if has(&["se muta", "muta", "mudo", "silêncio", "silencio", "fica quieto", "cala", "para de ouvir", "mute", "quiet", "stop listening"]) {
        return Some(VoiceCmd::Mute);
    }
    if rest.is_empty() || has(&["pode falar", "fala", "pode responder", "responde", "manda", "go ahead", "you can talk", "speak", "answer"]) {
        return Some(VoiceCmd::Speak);
    }
    Some(VoiceCmd::Ask(rest))
}

/// A yes or a no to a question the call asked ("sim", "pode", "não precisa"); None for anything else.
pub(crate) fn yes_or_no(text: &str) -> Option<bool> {
    let clean: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' }).collect();
    let first = clean.split_whitespace().next()?;
    if ["não", "nao", "no", "nope", "deixa", "negativo"].contains(&first) {
        return Some(false);
    }
    ["sim", "pode", "isso", "quero", "claro", "fica", "por", "yes", "yeah", "ok", "okay", "beleza", "positivo", "exato"].contains(&first).then_some(true)
}
