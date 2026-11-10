//! A portal page's console for the agents: the script that keeps what the page logged, the one that
//! reads it back and the text the agent gets.

use crate::prelude::*;

/// How many entries a page keeps, and how long each one can be.
const KEPT: usize = 200;
const LONGEST: usize = 2000;

/// Runs before the page's own scripts: keeps the last `KEPT` console calls, script errors and unhandled
/// rejections in `window.__magoConsole`. It never breaks the page and installing it twice does nothing.
#[cfg_attr(target_os = "linux", allow(dead_code))] // the page itself only exists off Linux
pub(crate) fn initialization_script() -> String {
    // an Error has no own enumerable fields (JSON.stringify gives "{}"): its stack says more
    format!(
        "(()=>{{try{{if(window.__magoConsoleInstalled)return;window.__magoConsoleInstalled=true;window.__magoConsole=[];\
         const show=value=>{{try{{if(typeof value==='string')return value;if(value instanceof Error)return value.stack||String(value);\
         const json=JSON.stringify(value);return json===undefined?String(value):json}}catch(_){{return String(value)}}}};\
         const add=(level,args)=>{{try{{const text=Array.from(args).map(show).join(' ').slice(0,{LONGEST});\
         window.__magoConsole.push({{level,text,time:Date.now()}});\
         if(window.__magoConsole.length>{KEPT})window.__magoConsole.splice(0,window.__magoConsole.length-{KEPT})}}catch(_){{}}}};\
         for(const level of ['log','info','warn','error','debug']){{try{{const original=console[level];\
         console[level]=function(...args){{add(level,args);try{{return original.apply(this,args)}}catch(_){{}}}}}}catch(_){{}}}}\
         window.addEventListener('error',event=>{{add('error',[event.error||event.message||'Erro de script',\
         event.filename?'('+event.filename+':'+event.lineno+')':''])}});\
         window.addEventListener('unhandledrejection',event=>{{add('error',['Promise rejeitada:',event.reason])}})\
         }}catch(_){{}}}})();"
    )
}

/// The script that answers with the kept entries (only those of `level`, when given) and, with `clear`,
/// drops the ones it read. It answers at once: the page's Promises are not awaited.
pub(crate) fn read_script(level: Option<&str>, clear: bool) -> Result<String, String> {
    if let Some(level) = level
        && !matches!(level, "error" | "warn" | "info" | "log" | "debug")
    {
        return Err(format!("nível de console desconhecido: {level} (use error, warn, info, log ou debug)"));
    }
    let level = level.map_or("null".into(), |level| format!("\"{level}\""));
    let clear = if clear { "window.__magoConsole=all.filter(entry=>!read.includes(entry));" } else { "" };
    Ok(format!(
        "(()=>{{try{{const level={level};const all=window.__magoConsole||[];\
         const read=level?all.filter(entry=>entry.level===level):all.slice();{clear}return read}}catch(_){{return []}}}})()"
    ))
}

/// The page's answer as the agent reads it: one `[level] text` line per entry.
pub(crate) fn format_response(json: &str) -> String {
    const EMPTY: &str = "O console está vazio.";
    // a script that answers a string comes back JSON-quoted: the array may be inside it
    let entries = match serde_json::from_str::<Value>(json) {
        Ok(Value::String(inner)) => serde_json::from_str::<Value>(&inner).unwrap_or_default(),
        Ok(value) => value,
        Err(_) => return EMPTY.into(),
    };
    let lines: Vec<String> = entries
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| Some(format!("[{}] {}", entry.get("level")?.as_str()?, entry.get("text")?.as_str()?)))
        .collect();
    if lines.is_empty() { EMPTY.into() } else { lines.join("\n") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_console_read_script_validates_level_and_clear() {
        let script = read_script(Some("warn"), true).unwrap();
        assert!(script.contains("const level=\"warn\""));
        // clearing drops only what was read: the other levels stay for a later read
        assert!(script.contains("window.__magoConsole=all.filter(entry=>!read.includes(entry));"));
        assert!(read_script(Some("trace"), false).unwrap_err().contains("trace"));
        let all = read_script(None, false).unwrap();
        assert!(all.contains("const level=null") && !all.contains("window.__magoConsole="));
        // it answers at once: nothing for the page to await
        assert!(!all.contains("async") && !all.contains("await") && !all.contains("Promise"));
    }

    #[test]
    fn portal_console_formats_json_entries() {
        assert_eq!(format_response(r#"[{"level":"error","text":"falhou"},{"level":"warn","text":"atenção"}]"#), "[error] falhou\n[warn] atenção");
        // the same array inside a JSON string
        assert_eq!(format_response(r#""[{\"level\":\"log\",\"text\":\"oi\"}]""#), "[log] oi");
        for empty in ["[]", "inválido", "null", "{}", "\"\""] {
            assert_eq!(format_response(empty), "O console está vazio.");
        }
    }

    #[test]
    fn portal_console_initialization_is_guarded_and_bounded() {
        let script = initialization_script();
        assert!(script.starts_with("(()=>{try{if(window.__magoConsoleInstalled)return;"));
        assert!(script.contains("length>200") && script.contains(".slice(0,2000)"));
        assert!(script.contains("value instanceof Error") && script.contains("unhandledrejection"));
        // the braces of the format! all closed
        assert_eq!(script.matches('{').count(), script.matches('}').count());
        assert_eq!(script.matches('(').count(), script.matches(')').count());
    }
}
