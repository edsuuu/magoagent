//! Blade (Laravel): the PHP inside `{{ }}`, `{!! !!}`, `@php … @endphp` and the conditions of `@if`/`@while` is parsed as PHP
//! (the rest of the file blanked out, so the places stay the same), and the directives that open a block must meet their
//! `@end…` (`@if/@endif`, `@foreach/@endforeach`, `@section/@endsection`…).

use super::{CAP, Found, diag, parse};
use crate::editor::lsp::Diag;

/// Directives that open a block; the closer each one waits for is `@end<name>`.
const OPENERS: [&str; 27] = [
    "if",
    "unless",
    "isset",
    "foreach",
    "forelse",
    "for",
    "while",
    "section",
    "push",
    "pushonce",
    "prepend",
    "prependonce",
    "once",
    "auth",
    "guest",
    "can",
    "cannot",
    "canany",
    "env",
    "production",
    "switch",
    "component",
    "slot",
    "error",
    "fragment",
    "session",
    "empty",
];
/// Directives that close a `@section` without an `@endsection`.
const SECTION_CLOSERS: [&str; 4] = ["show", "stop", "append", "overwrite"];
/// Conditions of their own that open an `@if` block (closed by `@endif`).
const IF_LIKE: [&str; 2] = ["hassection", "sectionmissing"];
/// Directives whose parenthesis holds a PHP expression.
const WITH_EXPRESSION: [&str; 4] = ["if", "elseif", "unless", "while"];

pub(super) fn check(text: &str) -> Vec<Diag> {
    let bytes = text.as_bytes();
    let mut mask: Vec<u8> = bytes.iter().map(|byte| if *byte == b'\n' { b'\n' } else { b' ' }).collect();
    let mut found: Vec<Found> = vec![];
    let starts: Vec<usize> = std::iter::once(0).chain(bytes.iter().enumerate().filter(|(_, byte)| **byte == b'\n').map(|(at, _)| at + 1)).collect();

    let place = |offset: usize| {
        let row = starts.partition_point(|start| *start <= offset) - 1;

        (row, offset - starts[row])
    };

    let mut stack: Vec<(&str, usize)> = vec![];
    let mut at = 0;

    while at < bytes.len() {
        let rest = &text[at..];

        if rest.starts_with("{{--") {
            match rest.find("--}}") {
                Some(end) => at += end + 4,
                None => {
                    found.push((place(at), place(at + 4), "Comentário não fechado".into()));

                    break;
                }
            }
        } else if rest.starts_with("@{{") {
            at += rest.find("}}").map_or(rest.len(), |end| end + 2);
        } else if rest.starts_with("{{") || rest.starts_with("{!!") {
            let (open, close) = if rest.starts_with("{{") { (2, "}}") } else { (3, "!!}") };

            match rest[open..].find(close) {
                Some(end) => {
                    mask[at] = b';';
                    mask[at + open..at + open + end].copy_from_slice(&bytes[at + open..at + open + end]);
                    mask[at + open + end] = b';';
                    at += open + end + close.len();
                }
                None => {
                    found.push((place(at), place(at + open), format!("`{}` sem `{close}`", &rest[..open])));
                    at += open;
                }
            }
        } else if let Some(escaped) = rest.strip_prefix("@@") {
            // Blade's escape: `@@foreach` is the text `@foreach`, no directive
            at += 2 + escaped.bytes().take_while(|byte| byte.is_ascii_alphabetic() || *byte == b'_').count();
        } else if bytes[at] == b'@' && (at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_')) {
            let word: String = rest[1..].chars().take_while(|c| c.is_ascii_alphabetic() || *c == '_').collect();

            if word.is_empty() {
                at += 1;

                continue;
            }

            let name = word.to_ascii_lowercase();
            let (start, after) = (at, at + 1 + word.len());
            let parens = paren_span(text, after);

            at = after;

            match name.as_str() {
                "php" if parens.is_none() => match text[after..].find("@endphp") {
                    Some(end) => {
                        mask[start] = b';';
                        mask[after..after + end].copy_from_slice(&bytes[after..after + end]);
                        mask[after + end] = b';';
                        at = after + end + 7;
                    }
                    None => found.push((place(start), place(after), "`@php` sem `@endphp`".into())),
                },
                "verbatim" => at = text[after..].find("@endverbatim").map_or(bytes.len(), |end| after + end + 12),
                "else" | "elseif" if stack.is_empty() => {
                    found.push((place(start), place(after), format!("`@{word}` fora de um bloco `@if`")));
                }
                _ => {
                    let closes = name.strip_prefix("end").and_then(|rest| OPENERS.iter().find(|opener| **opener == rest));
                    let name = if IF_LIKE.contains(&name.as_str()) { "if".to_string() } else { name };

                    let opens = OPENERS.iter().find(|opener| **opener == name).filter(|opener| match **opener {
                        // `@section('a', 'b')` and `@slot('a', 'b')` are whole; `@empty($x)` opens, a bare `@empty` is the
                        // middle of a forelse
                        "section" | "slot" => !parens.is_some_and(|(open, close)| has_comma(&text[open + 1..close])),
                        "empty" => parens.is_some(),
                        _ => true,
                    });

                    if let Some(opener) = opens {
                        stack.push((opener, start));
                    } else if let Some(opener) = closes.or_else(|| SECTION_CLOSERS.contains(&name.as_str()).then_some(&OPENERS[7])) {
                        match stack.iter().rposition(|(open, _)| open == opener) {
                            Some(index) => {
                                for (open, from) in stack.drain(index..).skip(1).collect::<Vec<_>>() {
                                    found.push((place(from), place(from + open.len() + 1), format!("`@{open}` sem `@end{open}`")));
                                }
                            }
                            None => found.push((place(start), place(after), format!("`@{word}` sem `@{opener}`"))),
                        }
                    }

                    if let Some((open, close)) = parens {
                        if WITH_EXPRESSION.contains(&name.as_str()) {
                            mask[start] = b';';
                            mask[open..=close].copy_from_slice(&bytes[open..=close]);
                        }

                        at = close + 1;
                    } else if text[after..].trim_start_matches([' ', '\t']).starts_with('(') {
                        found.push((place(after), place(after + 1), "`(` sem `)`".into()));
                    }
                }
            }
        } else {
            at += rest.chars().next().map_or(1, char::len_utf8);
        }
    }

    for (open, from) in stack {
        found.push((place(from), place(from + open.len() + 1), format!("`@{open}` sem `@end{open}`")));
    }

    mask.push(b';');
    let lines: Vec<&str> = text.split('\n').collect();
    let mut all: Vec<Diag> = found.into_iter().map(|(from, to, message)| diag(&lines, from, to, message)).collect();

    if mask.iter().any(|byte| !byte.is_ascii_whitespace() && *byte != b';') {
        all.extend(parse(&tree_sitter_php::LANGUAGE_PHP_ONLY.into(), &mask, text));
    }

    all.sort_by_key(|d| (d.line, d.col));
    all.truncate(CAP);
    all
}

/// The offsets of the `(` after `from` (blanks allowed) and of its matching `)`, strings respected.
fn paren_span(text: &str, from: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let open = from + text[from..].bytes().take_while(|byte| *byte == b' ' || *byte == b'\t').count();

    if bytes.get(open) != Some(&b'(') {
        return None;
    }

    let (mut depth, mut quote, mut at) = (0, None::<u8>, open);

    while at < bytes.len() {
        let byte = bytes[at];

        match quote {
            Some(_) if byte == b'\\' => at += 1,
            Some(q) if byte == q => quote = None,
            Some(_) => {}
            None if byte == b'\'' || byte == b'"' => quote = Some(byte),
            None if byte == b'(' => depth += 1,
            None if byte == b')' => {
                depth -= 1;

                if depth == 0 {
                    return Some((open, at));
                }
            }
            None => {}
        }

        at += 1;
    }

    None
}

/// A comma at the top level of `args`, outside strings and brackets: the second argument of an inline `@section`.
fn has_comma(args: &str) -> bool {
    let (mut depth, mut quote) = (0, None::<char>);

    for ch in args.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(ch),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => return true,
            _ => {}
        }
    }

    false
}
