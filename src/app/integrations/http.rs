//! The integrations' network: curl, as the rest of the app (updates, voice). The address, the headers and the body go
//! in curl's config on its standard input and never on the command line, which any process of the machine can read:
//! a token is only ever written there. No redirect is followed, so nothing leaves for another host than the one asked.

use std::io::Write;
use std::process::Stdio;

/// The most an answer may weigh: what is read past it is dropped with an error.
const MAX_ANSWER: usize = 2 * 1024 * 1024;

pub(crate) struct Request<'a> {
    pub(crate) method: &'a str,
    pub(crate) url: &'a str,
    /// "Name: value", the ones that carry a secret included.
    pub(crate) headers: Vec<String>,
    pub(crate) body: Option<String>,
    pub(crate) timeout_seconds: u64,
}

pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) body: String,
}

/// What curl gets on its command line: flags only, nothing of the request.
pub(crate) fn arguments(request: &Request) -> Vec<String> {
    let flags = ["-sS", "--proto", "=http,https", "--max-filesize", "2097152", "-w", "\\n%{http_code}", "--max-time"];
    flags.iter().map(|flag| flag.to_string()).chain([request.timeout_seconds.to_string(), "-K".into(), "-".into()]).collect()
}

/// A header's value on one line: a secret pasted with a line break in it adds no header of its own.
pub(crate) fn one_line(text: &str) -> String {
    text.chars().filter(|letter| !letter.is_control()).collect()
}

/// A value inside a curl config's quotes: the escapes curl reads, line breaks included (a raw one would start a directive).
pub(crate) fn quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}

/// curl's config (`-K -`) for `request`, each value quoted as curl reads it.
pub(crate) fn config(request: &Request) -> String {
    let mut text = format!("url = \"{}\"\nrequest = \"{}\"\n", quote(&one_line(request.url)), quote(request.method));
    for header in &request.headers {
        text += &format!("header = \"{}\"\n", quote(&one_line(header)));
    }
    if let Some(body) = &request.body {
        text += &format!("data-binary = \"{}\"\n", quote(body));
    }
    text
}

/// The status and the body out of what curl printed (`-w` ends it with a line break and the status).
pub(crate) fn split_answer(output: &[u8]) -> Result<(u16, &[u8]), String> {
    if output.len() > MAX_ANSWER {
        return Err("a resposta veio grande demais".into());
    }
    let end = output.iter().rposition(|byte| *byte == b'\n').ok_or("resposta sem status")?;
    let status = std::str::from_utf8(&output[end + 1..]).ok().and_then(|text| text.trim().parse::<u16>().ok()).ok_or("resposta sem status")?;
    Ok((status, &output[..end]))
}

/// Sends `request` and reads the answer as text.
pub(crate) fn send(request: &Request) -> Result<Answer, String> {
    let (status, body) = send_bytes(request)?;
    Ok(Answer { status, body: String::from_utf8_lossy(&body).into_owned() })
}

/// Sends `request`: the status and the body as it came (a picture). An error is the network's (no connection, a
/// timeout), an HTTP status is the caller's to read.
pub(crate) fn send_bytes(request: &Request) -> Result<(u16, Vec<u8>), String> {
    let mut child = crate::editor::command("curl")
        .args(arguments(request))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("curl não pôde ser iniciado: {error}"))?;
    if let Some(mut input) = child.stdin.take() {
        let _ = input.write_all(config(request).as_bytes());
    }
    let output = child.wait_with_output().map_err(|error| format!("curl: {error}"))?;
    if !output.status.success() {
        return Err("sem conexão".into());
    }
    split_answer(&output.stdout).map(|(status, body)| (status, body.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_goes_in_the_config_on_stdin_and_never_on_the_command_line() {
        let token = "ghp_segredo123";
        let request = Request {
            method: "POST",
            url: "https://api.github.com/graphql",
            headers: vec![format!("Authorization: Bearer {token}"), "User-Agent: MagoAgent".into()],
            body: Some(r#"{"query":"a \"b\"\nc"}"#.into()),
            timeout_seconds: 15,
        };
        let arguments = arguments(&request);
        assert!(arguments.iter().all(|argument| !argument.contains(token) && !argument.contains("github.com")), "{arguments:?}");
        assert_eq!(&arguments[arguments.len() - 2..], ["-K", "-"], "the request comes by the standard input");
        let config = config(&request);
        assert!(config.contains("header = \"Authorization: Bearer ghp_segredo123\"\n"), "{config}");
        assert!(config.contains(r#"data-binary = "{\"query\":\"a \\\"b\\\"\\nc\"}""#), "{config}");
        assert!(config.starts_with("url = \"https://api.github.com/graphql\"\nrequest = \"POST\"\n"));
    }

    #[test]
    fn a_line_break_in_a_secret_adds_no_header() {
        let request = Request { method: "GET", url: "https://x", headers: vec!["Authorization: Bearer a\r\nX-Evil: 1".into()], body: None, timeout_seconds: 5 };
        assert_eq!(config(&request).lines().count(), 3, "url, request and one header");
    }

    #[test]
    fn the_status_comes_after_the_body() {
        assert_eq!(split_answer(b"{\"a\":1}\n200").unwrap(), (200, &b"{\"a\":1}"[..]));
        assert_eq!(split_answer(b"\n304").unwrap(), (304, &b""[..]));
        // a picture: bytes that are no text, line breaks among them
        assert_eq!(split_answer(b"\xff\xd8\n\x00\n200").unwrap(), (200, &b"\xff\xd8\n\x00"[..]));
        assert!(split_answer(b"no status").is_err() && split_answer(b"body\nabc").is_err());
        assert!(split_answer(&vec![b'a'; MAX_ANSWER + 1]).is_err());
    }
}
