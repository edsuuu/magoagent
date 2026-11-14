use super::*;

pub(super) fn portal_eval_script(script: &str) -> String {
    let source = serde_json::to_string(script).unwrap_or_else(|_| "\"\"".into());
    format!("(()=>{{try{{return (0,eval)({source})}}catch(e){{return 'erro: '+(e&&e.message||String(e))}}}})()")
}

/// What a history action did, as the answer says it; `None` for one that doesn't exist.
pub(super) fn portal_history_action(action: &str) -> Option<&'static str> {
    match action {
        "back" => Some("voltou uma página"),
        "forward" => Some("avançou uma página"),
        "reload" => Some("recarregou a página"),
        _ => None,
    }
}

pub(super) fn portal_wait_script(selector: &str) -> String {
    let selector = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".into());
    format!(
        "(()=>{{const e=document.querySelector({selector});return !!(e&&e.getClientRects().length&&getComputedStyle(e).visibility!=='hidden'&&getComputedStyle(e).display!=='none')}})()"
    )
}

pub(super) fn portal_wait<F>(selector: &str, timeout: Duration, mut evaluate: F) -> Result<String, String>
where
    F: FnMut(&str) -> Result<String, String>,
{
    let script = portal_wait_script(selector);
    let deadline = Instant::now() + timeout.min(Duration::from_secs(30));
    loop {
        let result = evaluate(&script)?;
        if serde_json::from_str::<Value>(&result).ok().and_then(|v| v.as_bool()) == Some(true) {
            return Ok(format!("Elemento encontrado: {selector}"));
        }
        if Instant::now() >= deadline {
            return Ok(format!("Prazo esgotado ao esperar: {selector}"));
        }
        std::thread::sleep(Duration::from_millis(150).min(deadline.saturating_duration_since(Instant::now())));
    }
}

pub(super) fn portal_scroll_script(selector: Option<&str>, dy: Option<i64>) -> String {
    if let Some(selector) = selector {
        let selector = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".into());
        format!(
            "(()=>{{const e=document.querySelector({selector});if(!e)return 'nada encontrado';e.scrollIntoView({{block:'center',inline:'center'}});return 'Elemento rolado para a vista'}})()"
        )
    } else {
        format!("(()=>{{window.scrollBy(0,{});return 'Página rolada'}})()", dy.unwrap_or(0))
    }
}
