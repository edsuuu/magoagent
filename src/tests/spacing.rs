//! The blank lines between the statements of a block, the code's own lint (rustfmt keeps the blank lines it finds and
//! makes none):
//! - a run of `let`s (or items) has a blank line after it, before a statement of another kind;
//! - a statement that takes more than one line has a blank line before it and after it;
//! - `return`, `break` and `continue` have a blank line before them.
//!
//! The first and the last statement of a block keep to its braces. `MAGOAGENT_FIX_SPACING=1 cargo test --release
//! statements_have_their_blank_lines` puts the missing blank lines in.

use super::*;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;

/// The first and the last line a statement takes, its attributes in (counted from 1).
fn lines(stmt: &syn::Stmt) -> (usize, usize) {
    let span = stmt.span();

    (span.start().line, span.end().line)
}

/// A `let` or an item (a `use`, a `const`, a `fn` inside the block).
fn declares(stmt: &syn::Stmt) -> bool {
    matches!(stmt, syn::Stmt::Local(_) | syn::Stmt::Item(_))
}

/// A `return`, a `break` or a `continue`.
fn leaves(stmt: &syn::Stmt) -> bool {
    matches!(stmt, syn::Stmt::Expr(syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_), _))
}

/// Whether two statements side by side have a blank line between them.
fn apart(before: &syn::Stmt, after: &syn::Stmt) -> bool {
    let long = |stmt: &syn::Stmt| {
        let (first, last) = lines(stmt);

        last > first
    };

    (declares(before) && !declares(after)) || long(before) || long(after) || leaves(after)
}

/// The lines of a file after which a blank line is missing.
struct Missing<'source> {
    source: Vec<&'source str>,
    after: Vec<usize>,
}

impl<'ast> Visit<'ast> for Missing<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        for pair in block.stmts.windows(2) {
            let (_, end) = lines(&pair[0]);
            let (start, _) = lines(&pair[1]);
            // the lines between them hold comments (the next one's, mostly) or nothing
            let blank = (end + 1..start).any(|line| self.source[line - 1].trim().is_empty());

            if start > end && apart(&pair[0], &pair[1]) && !blank {
                self.after.push(end);
            }
        }

        syn::visit::visit_block(self, block);
    }
}

/// The lines of `text` after which a blank line is missing.
fn missing(text: &str) -> syn::Result<Vec<usize>> {
    let file = syn::parse_file(text)?;
    let mut found = Missing { source: text.split('\n').collect(), after: Vec::new() };

    found.visit_file(&file);
    found.after.sort_unstable();
    found.after.dedup();

    Ok(found.after)
}

/// `text` with a blank line after each of `after` (counted from 1), in the file's own line ending.
fn spaced(text: &str, after: &[usize]) -> String {
    let blank = if text.contains("\r\n") { "\r" } else { "" };
    let mut out = Vec::new();

    for (index, line) in text.split('\n').enumerate() {
        out.push(line);

        if after.binary_search(&(index + 1)).is_ok() {
            out.push(blank);
        }
    }

    out.join("\n")
}

/// Every `.rs` under `folder`.
fn sources(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).unwrap().flatten() {
        let path = entry.path();

        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn the_rules_space_statements_out() {
    let code = "fn f() {\n    let a = 1;\n    let b = 2;\n    g(a, b);\n    if a > 0 {\n        h();\n    }\n    g(b, a);\n    return;\n}\n";
    let want = "fn f() {\n    let a = 1;\n    let b = 2;\n\n    g(a, b);\n\n    if a > 0 {\n        h();\n    }\n\n    g(b, a);\n\n    return;\n}\n";

    assert_eq!(missing(code).unwrap(), vec![3, 4, 7, 8]);
    assert_eq!(spaced(code, &missing(code).unwrap()), want);
    assert!(missing(want).unwrap().is_empty(), "spaced, nothing is missing");
    // a comment above the next statement stays with it, under the blank line
    let commented = "fn f() {\n    let a = 1;\n    // why\n    g(a);\n}\n";

    assert_eq!(spaced(commented, &missing(commented).unwrap()), "fn f() {\n    let a = 1;\n\n    // why\n    g(a);\n}\n");
    // first and last in their block, and one-line statements of a kind, keep together
    assert!(missing("fn f() {\n    if x {\n        return;\n    }\n\n    g();\n    h();\n}\n").unwrap().is_empty());
    assert_eq!(missing("fn f() {\n    let a = 1;\n    let b = a;\n}\n").unwrap(), Vec::<usize>::new());
    assert_eq!(missing("fn f() {\n    let a = 1;\n    let b = 2;\n}\n").unwrap(), Vec::<usize>::new());
    // the CRLF of a file checked out on Windows
    assert_eq!(spaced("a\r\nb\r\n", &[1]), "a\r\n\r\nb\r\n");
}

/// The whole code follows the rules above.
#[test]
fn statements_have_their_blank_lines() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fix = std::env::var_os("MAGOAGENT_FIX_SPACING").is_some();
    let mut files = Vec::new();
    let mut wrong = Vec::new();

    sources(&root.join("src"), &mut files);
    files.sort();

    for path in &files {
        let text = fs::read_to_string(path).unwrap();
        let after = missing(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let name = path.strip_prefix(root).unwrap_or(path).display().to_string();

        if after.is_empty() {
            continue;
        }

        if fix {
            fs::write(path, spaced(&text, &after)).unwrap();
        }

        wrong.extend(after.iter().map(|line| format!("{name}:{line}")));
    }

    if fix {
        return;
    }

    let shown: Vec<_> = wrong.iter().take(30).cloned().collect();

    assert!(
        wrong.is_empty(),
        "{} statements without the blank line after them (MAGOAGENT_FIX_SPACING=1 cargo test --release statements_have_their_blank_lines puts it in):\n{}",
        wrong.len(),
        shown.join("\n")
    );
}
