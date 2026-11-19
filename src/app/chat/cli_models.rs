//! The models Claude Code itself offers: its stream-json `initialize` answers with them (no call to a
//! model, no session kept). Asked once a run in a thread and kept in `modelos-claude.json` in the data
//! folder, so a Claude Code update brings its new models with no MagoAgent update; footer.rs keeps a list
//! for when it can't answer.

use crate::prelude::*;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex, Once};

/// What the CLI said, this run or the last (its `models` entries); None until read.
static MODELS: Mutex<Option<Vec<Value>>> = Mutex::new(None);

fn cache() -> Option<PathBuf> {
    crate::file::data_dir().map(|d| d.join("modelos-claude.json"))
}

/// Claude Code's models as it lists them: the last run's until this run's answer; empty when neither.
pub(crate) fn claude_cli_models() -> Vec<Value> {
    static ASK: Once = Once::new();

    if !cfg!(test) {
        ASK.call_once(|| {
            std::thread::spawn(|| {
                if let Some(models) = ask_claude() {
                    if let Some(path) = cache() {
                        let _ = fs::write(path, serde_json::to_string(&models).unwrap_or_default());
                    }

                    *MODELS.lock().unwrap() = Some(models);
                }
            });
        });
    }

    let mut held = MODELS.lock().unwrap();

    held.get_or_insert_with(|| cache().and_then(|p| read_json(&p)).unwrap_or_default()).clone()
}

/// `claude -p` in stream-json, asked to initialize: the `models` of its answer. It is stopped once it
/// answered, or after 30 s.
fn ask_claude() -> Option<Vec<Value>> {
    let cli = agent_cli("claude")?;
    let mut command = cli.command();

    command.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--no-session-persistence"]);
    command.current_dir(std::env::temp_dir());

    let mut child =
        cli.finish(command).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;

    writeln!(child.stdin.as_mut()?, r#"{{"type":"control_request","request_id":"models","request":{{"subtype":"initialize"}}}}"#).ok()?;
    let out = child.stdout.take()?;
    let child = Arc::new(Mutex::new(child));
    let watch = Arc::clone(&child);

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(30));
        let _ = watch.lock().unwrap().kill();
    });

    let models = std::io::BufReader::new(out).lines().map_while(Result::ok).find_map(|line| models_of(&line));
    let mut child = child.lock().unwrap();
    let _ = child.kill();
    let _ = child.wait();

    models
}

/// The models of the `initialize` answer, if `line` is it.
fn models_of(line: &str) -> Option<Vec<Value>> {
    let v: Value = serde_json::from_str(line).ok()?;

    (v["type"] == "control_response").then(|| v["response"]["response"]["models"].as_array().cloned()).flatten().filter(|m| !m.is_empty())
}

/// The CLI's models as the menu's: (the current ones, by their alias, then the older ones by their id), each
/// with the model it runs, its name and its efforts. "default" is left out: the chip already shows the default.
pub(crate) fn split(models: &[Value]) -> (Vec<(String, String, String, Vec<String>)>, Vec<(String, String, String, Vec<String>)>) {
    let row = |m: &Value| {
        let id = m["resolvedModel"].as_str().or(m["value"].as_str())?.to_string();
        let name = m["displayName"].as_str().unwrap_or(&id).to_string();
        let note = m["description"].as_str().unwrap_or_default().to_string();
        let efforts = m["supportedEffortLevels"].as_array().map(|l| l.iter().filter_map(|e| e.as_str().map(String::from)).collect()).unwrap_or_default();

        Some((id, name, note, efforts))
    };

    let alias = |m: &&Value| m["value"].as_str().is_some_and(|v| v != "default" && !v.starts_with("claude-"));
    let older = |m: &&Value| m["value"].as_str().is_some_and(|v| v.starts_with("claude-"));

    (models.iter().filter(alias).filter_map(row).collect(), models.iter().filter(older).filter_map(row).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_initialize_answer_gives_the_current_and_the_older_models() {
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"models","response":{"models":[
            {"value":"default","resolvedModel":"claude-opus-5-5","displayName":"Default (recommended)"},
            {"value":"opus","resolvedModel":"claude-opus-5-5","displayName":"Opus 5.5","description":"Best","supportedEffortLevels":["low","high","max"]},
            {"value":"haiku","resolvedModel":"claude-haiku-5-5","displayName":"Haiku 5.5"},
            {"value":"claude-opus-4-8","resolvedModel":"claude-opus-4-8","displayName":"Opus 4.8"}]}}}"#
            .replace('\n', "");

        let models = models_of(&line).expect("its models");

        assert!(models_of(r#"{"type":"system","subtype":"init"}"#).is_none());
        let (now, older) = split(&models);

        assert_eq!(now.iter().map(|m| m.0.as_str()).collect::<Vec<_>>(), ["claude-opus-5-5", "claude-haiku-5-5"], "default left out");
        assert_eq!(now[0].1, "Opus 5.5");
        assert_eq!(now[0].3, ["low", "high", "max"]);
        assert_eq!(older.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(), ["Opus 4.8"]);
    }
}
