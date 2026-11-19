//! The syntax check with a real parser (tree-sitter) for JavaScript, TypeScript, PHP, Blade and Rust: the ERROR and MISSING
//! nodes of the parse tree become problems with line, column and a message in Portuguese, as the reference IDEs show them.

mod blade;
#[cfg(test)]
mod tests;

use crate::editor::lsp::Diag;
use tree_sitter::{Language, Node, Parser};

const CAP: usize = 20;
const ERROR: u8 = 1;

/// The grammar of a language id (`editor/langs`), None when this check does not parse it.
fn grammar(id: &str) -> Option<Language> {
    Some(match id {
        "javascript" | "javascriptreact" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "typescriptreact" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        _ => return None,
    })
}

/// Whether `check` parses this language.
pub(crate) fn handles(id: &str) -> bool {
    id == "blade" || grammar(id).is_some()
}

/// The syntax errors of `text` written in the language `id`.
pub(crate) fn check(id: &str, text: &str) -> Vec<Diag> {
    if id == "blade" {
        return blade::check(text);
    }

    match grammar(id) {
        Some(language) => parse(&language, text.as_bytes(), text),
        None => vec![],
    }
}

/// Parses `source` (the bytes the parser sees) and reports its errors with places taken in `text` (same length).
fn parse(language: &Language, source: &[u8], text: &str) -> Vec<Diag> {
    let mut parser = Parser::new();

    if parser.set_language(language).is_err() {
        return vec![];
    }

    let Some(tree) = parser.parse(source, None) else { return vec![] };
    let mut found = vec![];

    if tree.root_node().has_error() {
        collect(tree.root_node(), source, &mut found);
    }

    found.sort_by_key(|(start, _, _)| *start);
    let lines: Vec<&str> = text.split('\n').collect();

    found.into_iter().take(CAP).map(|(start, end, message)| diag(&lines, start, end, message)).collect()
}

/// (row, byte column) of the start and of the end of the red mark, and the message.
type Found = ((usize, usize), (usize, usize), String);

fn collect(node: Node, source: &[u8], found: &mut Vec<Found>) {
    if node.is_missing() {
        let point = node.start_position();

        let wanted = match node.kind() {
            "identifier" | "name" | "property_identifier" | "type_identifier" | "variable_name" => "um nome".to_string(),
            kind => format!("`{kind}`"),
        };

        found.push(((point.row, point.column), (point.row, point.column + 1), format!("Esperado {wanted}")));
    } else if node.is_error() && report_unclosed(node, source.len(), found) {
    } else if node.is_error() {
        // the first token the parser could not place; what comes after it is probably fallout
        let mut token = node;

        while let Some(first) = token.child(0) {
            token = first;
        }

        let text = token.utf8_text(source).unwrap_or_default().lines().next().unwrap_or_default().trim();
        let (from, to) = (token.start_position(), token.end_position());
        let end = if to.row == from.row { to.column.max(from.column + 1) } else { from.column + text.len().max(1) };

        let message = match text {
            "" => "Sintaxe inválida".to_string(),
            text if text.chars().count() > 24 => format!("Token inesperado `{}…`", text.chars().take(24).collect::<String>()),
            text => format!("Token inesperado `{text}`"),
        };

        found.push(((from.row, from.column), (from.row, end), message));
    } else {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor).filter(|c| c.has_error()) {
            collect(child, source, found);
        }
    }
}

/// A problem from byte columns to character columns (the editor counts characters).
fn diag(lines: &[&str], start: (usize, usize), end: (usize, usize), message: String) -> Diag {
    let chars = |row: usize, byte: usize| {
        let line = lines.get(row).copied().unwrap_or_default();

        line.char_indices().take_while(|(at, _)| *at < byte).count() + byte.saturating_sub(line.len())
    };

    let (col, end_col) = (chars(start.0, start.1), chars(end.0, end.1));

    Diag { line: start.0, col, end_line: end.0, end_col: end_col.max(col + 1), severity: ERROR, message, source: "sintaxe".into() }
}

/// An ERROR that runs to the end of the text is usually a block left open (the parser cannot say where it was to close):
/// report the brackets of its tokens that nothing closed. False when there is none.
fn report_unclosed(node: Node, length: usize, found: &mut Vec<Found>) -> bool {
    if node.end_byte() < length.saturating_sub(1) {
        return false;
    }

    let (mut open, mut cursor) = (vec![], node.walk());
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        match current.kind() {
            "(" | "[" | "{" | "${" => open.push(current),
            ")" | "]" | "}" => {
                open.pop();
            }
            _ => stack.extend(current.children(&mut cursor).collect::<Vec<_>>().into_iter().rev()),
        }
    }

    for token in &open {
        let (point, kind) = (token.start_position(), token.kind().trim_start_matches('$'));

        let closer = match kind {
            "(" => ")",
            "[" => "]",
            _ => "}",
        };

        found.push(((point.row, point.column), (point.row, point.column + 1), format!("`{kind}` sem o `{closer}` correspondente")));
    }

    !open.is_empty()
}
