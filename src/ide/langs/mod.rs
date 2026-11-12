//! The languages the editor knows: comment syntax, quotes, keywords and language servers.

use std::path::Path;
mod native;
mod others;
mod web;
use native::*;
use others::*;
use web::*;

#[derive(PartialEq)]
pub enum Kind {
    Code,
    Markup,
    Plain,
}

/// (argv, install hint) for a language server.
pub type Server = (&'static [&'static str], &'static str);

pub struct Lang {
    pub name: &'static str,
    pub id: &'static str, // LSP languageId
    pub kind: Kind,
    pub line: &'static [&'static str],
    pub block: Option<(&'static str, &'static str)>,
    pub quotes: &'static str,
    pub kw: &'static [&'static str],
    pub servers: &'static [Server],
}

const NPM_HTML: &str = "npm i -g vscode-langservers-extracted";
const TS: &[Server] = &[(&["typescript-language-server", "--stdio"], "npm i -g typescript-language-server typescript")];
const CLANGD: &[Server] = &[(&["clangd"], "winget install LLVM.LLVM")];

const CSS_KW: &[&str] = &["important", "inherit", "initial", "none", "unset", "auto"];
const LIT_KW: &[&str] = &["true", "false", "null", "yes", "no", "on", "off"];
const C_COMMENTS: &[&str] = &["//"];
const HASH: &[&str] = &["#"];
const BLOCK: Option<(&str, &str)> = Some(("/*", "*/"));

macro_rules! lang {
    ($name:expr, $id:expr, $kind:ident, $line:expr, $block:expr, $q:expr, $kw:expr, $srv:expr) => {
        Lang { name: $name, id: $id, kind: Kind::$kind, line: $line, block: $block, quotes: $q, kw: $kw, servers: $srv }
    };
}

pub static PHP: Lang = lang!(
    "PHP",
    "php",
    Code,
    &["//", "#"],
    BLOCK,
    "\"'`",
    PHP_KW,
    &[(&["intelephense", "--stdio"], "npm i -g intelephense"), (&["phpactor", "language-server"], "composer global require phpactor/phpactor")]
);
pub static BLADE: Lang = lang!("Blade", "blade", Markup, &[], Some(("{{--", "--}}")), "\"'", PHP_KW, &[]);
pub static HTML: Lang = lang!("HTML", "html", Markup, &[], Some(("<!--", "-->")), "\"'", &[], &[(&["vscode-html-language-server", "--stdio"], NPM_HTML)]);
pub static XML: Lang = lang!("XML", "xml", Markup, &[], Some(("<!--", "-->")), "\"'", &[], &[]);
pub static CSS: Lang = lang!("CSS", "css", Code, C_COMMENTS, BLOCK, "\"'", CSS_KW, &[(&["vscode-css-language-server", "--stdio"], NPM_HTML)]);
pub static JS: Lang = lang!("JavaScript", "javascript", Code, C_COMMENTS, BLOCK, "\"'`", JS_KW, TS);
pub static JSX: Lang = lang!("JavaScript React", "javascriptreact", Code, C_COMMENTS, BLOCK, "\"'`", JS_KW, TS);
pub static TSL: Lang = lang!("TypeScript", "typescript", Code, C_COMMENTS, BLOCK, "\"'`", TS_KW, TS);
pub static TSX: Lang = lang!("TypeScript React", "typescriptreact", Code, C_COMMENTS, BLOCK, "\"'`", TS_KW, TS);
pub static JSON: Lang = lang!("JSON", "json", Code, C_COMMENTS, BLOCK, "\"", LIT_KW, &[(&["vscode-json-language-server", "--stdio"], NPM_HTML)]);
pub static RUST: Lang = lang!("Rust", "rust", Code, C_COMMENTS, BLOCK, "\"'", RUST_KW, &[(&["rust-analyzer"], "rustup component add rust-analyzer")]);
pub static GO: Lang = lang!("Go", "go", Code, C_COMMENTS, BLOCK, "\"'`", GO_KW, &[(&["gopls"], "go install golang.org/x/tools/gopls@latest")]);
pub static PY: Lang = lang!(
    "Python",
    "python",
    Code,
    HASH,
    None,
    "\"'",
    PY_KW,
    &[(&["pyright-langserver", "--stdio"], "npm i -g pyright"), (&["pylsp"], "pip install python-lsp-server")]
);
pub static C: Lang = lang!("C", "c", Code, C_COMMENTS, BLOCK, "\"'", C_KW, CLANGD);
pub static CPP: Lang = lang!("C++", "cpp", Code, C_COMMENTS, BLOCK, "\"'", CPP_KW, CLANGD);
pub static CS: Lang = lang!(
    "C#",
    "csharp",
    Code,
    C_COMMENTS,
    BLOCK,
    "\"'",
    CS_KW,
    &[(&["csharp-ls"], "dotnet tool install -g csharp-ls"), (&["OmniSharp", "-lsp"], "winget install OmniSharp.OmniSharp")]
);
pub static ASM: Lang = lang!("Assembly", "asm", Code, &[";", "#", "//"], BLOCK, "\"'", ASM_KW, &[(&["asm-lsp"], "cargo install asm-lsp")]);
pub static SQL: Lang = lang!("SQL", "sql", Code, &["--", "#"], BLOCK, "\"'`", SQL_KW, &[]);
pub static SH: Lang = lang!("Shell", "shellscript", Code, HASH, None, "\"'`", SH_KW, &[(&["bash-language-server", "start"], "npm i -g bash-language-server")]);
pub static YAML: Lang = lang!("YAML", "yaml", Code, HASH, None, "\"'", LIT_KW, &[(&["yaml-language-server", "--stdio"], "npm i -g yaml-language-server")]);
pub static TOML: Lang = lang!("TOML", "toml", Code, HASH, None, "\"'", LIT_KW, &[]);
// ponytail: no language server hints for these yet, add `servers` (jdtls, kotlin-language-server, sourcekit-lsp, solargraph, lua-language-server, dart) when wanted
pub static JAVA: Lang = lang!("Java", "java", Code, C_COMMENTS, BLOCK, "\"'", JAVA_KW, &[]);
pub static KOTLIN: Lang = lang!("Kotlin", "kotlin", Code, C_COMMENTS, BLOCK, "\"'", KOTLIN_KW, &[]);
pub static SWIFT: Lang = lang!("Swift", "swift", Code, C_COMMENTS, BLOCK, "\"", SWIFT_KW, &[]);
pub static DART: Lang = lang!("Dart", "dart", Code, C_COMMENTS, BLOCK, "\"'", DART_KW, &[]);
pub static RUBY: Lang = lang!("Ruby", "ruby", Code, HASH, Some(("=begin", "=end")), "\"'`", RUBY_KW, &[]);
pub static LUA: Lang = lang!("Lua", "lua", Code, &["--"], Some(("--[[", "]]")), "\"'", LUA_KW, &[]);
pub static MARKDOWN: Lang = lang!("Markdown", "markdown", Markup, &[], Some(("<!--", "-->")), "", &[], &[]);
pub static TEXT: Lang = lang!("Plain text", "plaintext", Plain, &[], None, "", &[], &[]);

pub fn detect(path: &Path) -> &'static Lang {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name.ends_with(".blade.php") {
        return &BLADE;
    }
    if name == "dockerfile" || name == "makefile" || name.starts_with(".env") {
        return &SH;
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "php" | "phtml" => &PHP,
        "html" | "htm" | "vue" | "svelte" => &HTML,
        "xml" | "svg" | "xaml" | "csproj" => &XML,
        "css" | "scss" | "sass" | "less" => &CSS,
        "js" | "mjs" | "cjs" => &JS,
        "jsx" => &JSX,
        "ts" | "mts" | "cts" => &TSL,
        "tsx" => &TSX,
        "json" | "jsonc" | "lock" => &JSON,
        "rs" => &RUST,
        "go" => &GO,
        "py" | "pyi" => &PY,
        "c" | "h" => &C,
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "ino" => &CPP,
        "cs" => &CS,
        "asm" | "s" | "nasm" | "inc" => &ASM,
        "sql" => &SQL,
        "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" => &SH,
        "yml" | "yaml" => &YAML,
        "toml" | "ini" | "conf" | "cfg" => &TOML,
        "java" => &JAVA,
        "kt" | "kts" => &KOTLIN,
        "swift" => &SWIFT,
        "dart" => &DART,
        "rb" | "rake" | "gemspec" => &RUBY,
        "lua" => &LUA,
        "md" | "markdown" | "mdx" => &MARKDOWN,
        _ => &TEXT,
    }
}
