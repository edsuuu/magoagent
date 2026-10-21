//! Project index: declarations (classes, functions, methods…), who extends/implements what,
//! and identifier counts. Feeds Search Everywhere, Code Vision ("N usages") and Structure.
// ponytail: line-based heuristics, not a parser; a tree-sitter pass would make it exact.

use super::highlight;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Class,
    Interface,
    Trait,
    Enum,
    Struct,
    Function,
    Method,
    Constant,
    Type,
    Module,
}

impl Kind {
    pub fn is_type(self) -> bool {
        matches!(self, Kind::Class | Kind::Interface | Kind::Trait | Kind::Enum | Kind::Struct | Kind::Type)
    }
    /// Letter + colour for the round symbol icon (New UI style).
    pub fn badge(self) -> (&'static str, u32) {
        match self {
            Kind::Class => ("C", 0x4E8DE0),
            Kind::Interface => ("I", 0x5FAD65),
            Kind::Trait => ("T", 0xD69A6B),
            Kind::Enum => ("E", 0xB589EC),
            Kind::Struct => ("S", 0x4E8DE0),
            Kind::Function => ("ƒ", 0xC77DBB),
            Kind::Method => ("m", 0xE0956D),
            Kind::Constant => ("c", 0xC77DBB),
            Kind::Type => ("T", 0x4E8DE0),
            Kind::Module => ("M", 0x8B8E94),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decl {
    pub name: String,
    pub kind: Kind,
    pub line: usize,
    pub col: usize,
    /// types this one extends / implements (or, for Rust `impl Trait for T`, the trait)
    pub bases: Vec<String>,
}

fn is_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Identifiers of a line with their char columns.
fn words(line: &str) -> Vec<(usize, &str)> {
    let mut out = vec![];
    let mut start = None;
    for (col, (i, c)) in line.char_indices().enumerate() {
        match (is_id(c), start) {
            (true, None) => start = Some((col, i)),
            (false, Some((sc, si))) => {
                out.push((sc, &line[si..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some((sc, si)) = start {
        out.push((sc, &line[si..]));
    }
    out
}

const CONTROL: &[&str] = &["if", "for", "while", "switch", "return", "else", "new", "throw", "case", "delete", "sizeof", "catch", "using", "lock", "foreach"];

/// Declarations in one file. Works across the supported languages with keyword heuristics.
pub fn declarations(lang_id: &str, text: &str) -> Vec<Decl> {
    let mut out = vec![];
    let mut in_block_comment = false;
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.trim_start();
        let indent = raw.len() - line.len();
        if in_block_comment {
            in_block_comment = !line.contains("*/");
            continue;
        }
        if line.starts_with("/*") {
            in_block_comment = !line.contains("*/");
            continue;
        }
        if line.starts_with("//") || line.starts_with('*') || line.starts_with("--") || (line.starts_with('#') && !line.starts_with("#[")) || line.starts_with(';') {
            continue;
        }
        let ws = words(raw);
        if lang_id == "asm" {
            if indent == 0 && let Some((c, w)) = ws.first() && raw[w.len()..].starts_with(':') {
                out.push(Decl { name: w.to_string(), kind: Kind::Function, line: ln, col: *c, bases: vec![] });
            }
            continue;
        }
        let chars: Vec<char> = raw.chars().collect();
        let kw = ws.iter().position(|&(c, w)| {
            let keyword = matches!(w, "class" | "interface" | "trait" | "enum" | "struct" | "union" | "fn" | "function" | "def" | "func" | "mod" | "impl" | "record" | "namespace")
                || (w == "const" && matches!(lang_id, "php" | "javascript" | "javascriptreact" | "typescript" | "typescriptreact" | "go"))
                || (w == "type" && lang_id != "php");
            // not a keyword when used as a value/member/string: Foo::class, $fn, x.type, 'type' =>
            let prev = if c > 0 { chars[c - 1] } else { ' ' };
            keyword && !matches!(prev, ':' | '$' | '.' | '>' | '\'' | '"' | '-')
        });
        if let Some(k) = kw {
            let keyword = ws[k].1;
            if keyword == "impl" {
                // impl Trait for Type  -> Type implements Trait
                if let Some(f) = ws.iter().position(|(_, w)| *w == "for")
                    && f > k + 1
                    && let Some((c, ty)) = ws.get(f + 1)
                {
                    out.push(Decl { name: ty.to_string(), kind: Kind::Struct, line: ln, col: *c, bases: vec![ws[k + 1..f].last().unwrap().1.to_string()] });
                }
                continue;
            }
            // Go method: func (r *T) Name(
            let mut ni = k + 1;
            if keyword == "func" && line[4..].trim_start().starts_with('(') {
                let close = raw.find(')').unwrap_or(0);
                ni = ws.iter().position(|(c, _)| *c > raw[..close].chars().count()).unwrap_or(ws.len());
            }
            let Some((col, name)) = ws.get(ni) else { continue };
            // the name must follow the keyword directly (rejects closures `function ($x)`, `fn($x) =>`)
            let gap: String = chars[ws[k].0 + keyword.chars().count()..*col].iter().collect();
            let direct = gap.chars().all(|c| c.is_whitespace() || c == '&' || c == '*') || ni != k + 1;
            if !direct || name.chars().next().is_some_and(|c| c.is_ascii_digit()) || matches!(*name, "extends" | "implements" | "use" | "new") {
                continue;
            }
            let kind = match keyword {
                "class" | "record" => Kind::Class,
                "interface" => Kind::Interface,
                "trait" => Kind::Trait,
                "enum" => Kind::Enum,
                "struct" | "union" => Kind::Struct,
                "type" => Kind::Type,
                "mod" | "namespace" => Kind::Module,
                "const" => {
                    if raw.contains("=>") || raw.contains("function") { Kind::Function } else { Kind::Constant }
                }
                _ => {
                    if indent > 0 && lang_id != "go" || (keyword == "func" && ni > k + 1) { Kind::Method } else { Kind::Function }
                }
            };
            if kind == Kind::Module && lang_id == "php" {
                continue; // `namespace App\Models;` is not interesting in Structure/Search
            }
            if kind == Kind::Constant && indent > 0 && !matches!(lang_id, "php" | "csharp") {
                continue; // local consts
            }
            let mut bases = vec![];
            if kind.is_type() {
                let rest: Vec<&str> = ws[ni + 1..].iter().map(|(_, w)| *w).collect();
                let tail = &raw[raw.find(name).map_or(0, |i| i + name.len())..];
                match lang_id {
                    "python" if tail.trim_start().starts_with('(') => bases.extend(rest.iter().take_while(|w| **w != "metaclass").map(|w| w.to_string())),
                    "csharp" | "cpp" | "c" if tail.trim_start().starts_with(':') => {
                        bases.extend(rest.iter().filter(|w| !matches!(**w, "public" | "private" | "protected" | "virtual" | "where")).map(|w| w.to_string()))
                    }
                    _ => {
                        let mut on = false;
                        for w in rest {
                            if matches!(w, "extends" | "implements") {
                                on = true;
                            } else if on {
                                bases.push(w.to_string());
                            }
                        }
                    }
                }
                // drop namespace segments: App\Contracts\Foo -> Foo is the last word anyway; keep the last of each path
                bases.retain(|b| b.chars().next().is_some_and(|c| c.is_uppercase()));
            }
            out.push(Decl { name: name.to_string(), kind, line: ln, col: *col, bases });
            continue;
        }
        // JS/TS class methods: `async save(x: T): Promise<void> {`
        if lang_id.starts_with("javascript") || lang_id.starts_with("typescript") {
            if indent > 0 && line.ends_with('{') && let Some(p) = raw.find('(') {
                let before = words(&raw[..p]);
                const MODS: &[&str] = &["async", "static", "public", "private", "protected", "readonly", "get", "set", "override", "abstract"];
                if let Some((&(col, name), mods)) = before.split_last()
                    && mods.iter().all(|(_, w)| MODS.contains(w))
                    && raw[..p].trim_end().ends_with(name)
                    && !CONTROL.contains(&name)
                    && name != "function"
                {
                    out.push(Decl { name: name.to_string(), kind: Kind::Method, line: ln, col, bases: vec![] });
                }
            }
            continue;
        }
        // C-family functions/methods without a keyword: `type name(args) {`
        if matches!(lang_id, "c" | "cpp" | "csharp") && !line.ends_with(';') && (line.ends_with('{') || line.ends_with(')')) {
            if let Some(p) = raw.find('(') {
                let before = words(&raw[..p]);
                if before.len() >= 2 && !before.iter().any(|(_, w)| CONTROL.contains(w)) && raw[..p].trim_end().ends_with(before.last().unwrap().1) {
                    let (col, name) = *before.last().unwrap();
                    let kind = if indent > 0 { Kind::Method } else { Kind::Function };
                    out.push(Decl { name: name.to_string(), kind, line: ln, col, bases: vec![] });
                }
            }
        }
    }
    out
}

#[derive(Clone)]
pub struct Symbol {
    pub decl: Decl,
    pub path: PathBuf,
}

#[derive(Default)]
pub struct Index {
    pub symbols: Vec<Symbol>,
    /// identifier -> occurrences in the project
    pub words: HashMap<String, u32>,
    /// type name -> types that extend/implement it
    pub implementors: HashMap<String, Vec<(PathBuf, usize)>>,
    file_words: HashMap<PathBuf, HashMap<String, u32>>,
}

/// Directories whose contents are libraries/build output, not project code.
pub const LIBRARY_DIRS: &[&str] = &["vendor", "node_modules", "target", "dist", "build", "storage", "bootstrap/cache", "public/build", ".git", ".idea"];

pub fn indexable(root: &Path, p: &Path) -> bool {
    let rel = p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/");
    !LIBRARY_DIRS.iter().any(|d| rel.starts_with(&format!("{d}/")))
        && highlight::detect(p).kind != highlight::Kind::Plain
        && !matches!(highlight::detect(p).id, "json" | "yaml" | "toml" | "sql")
        && std::fs::metadata(p).is_ok_and(|m| m.len() < 1 << 20)
}

impl Index {
    pub fn build(root: &Path, files: &[PathBuf]) -> Index {
        let mut ix = Index::default();
        for p in files.iter().filter(|p| indexable(root, p)) {
            ix.update(p);
        }
        ix
    }

    /// (Re)index one file, replacing what it contributed before.
    pub fn update(&mut self, p: &Path) {
        self.remove(p);
        let Ok(text) = std::fs::read_to_string(p) else { return };
        let lang = highlight::detect(p);
        // templates only contribute usages (Blade calls `$user->initials()`), not declarations
        let decls = if lang.kind == highlight::Kind::Code { declarations(lang.id, &text) } else { vec![] };
        for d in &decls {
            for b in &d.bases {
                self.implementors.entry(b.clone()).or_default().push((p.to_path_buf(), d.line));
            }
        }
        // `impl Trait for Type` lines only describe implementors, not new symbols
        self.symbols.extend(decls.into_iter().filter(|d| !(lang.id == "rust" && d.kind == Kind::Struct && !d.bases.is_empty())).map(|decl| Symbol { decl, path: p.to_path_buf() }));
        let mut counts: HashMap<String, u32> = HashMap::new();
        for line in text.lines() {
            for (_, w) in words(line) {
                if w.len() > 1 && !w.chars().next().unwrap().is_ascii_digit() {
                    *counts.entry(w.to_string()).or_default() += 1;
                }
            }
        }
        for (w, n) in &counts {
            *self.words.entry(w.clone()).or_default() += n;
        }
        self.file_words.insert(p.to_path_buf(), counts);
    }

    fn remove(&mut self, p: &Path) {
        self.symbols.retain(|s| s.path != p);
        for v in self.implementors.values_mut() {
            v.retain(|(q, _)| q != p);
        }
        if let Some(old) = self.file_words.remove(p) {
            for (w, n) in old {
                if let Some(c) = self.words.get_mut(&w) {
                    *c = c.saturating_sub(n);
                }
            }
        }
    }

    /// Code Vision text for a declaration: "3 usages", "2 implementations"…
    pub fn vision(&self, d: &Decl) -> Option<String> {
        if d.kind == Kind::Module {
            return None;
        }
        let decls = self.symbols.iter().filter(|s| s.decl.name == d.name).count().max(1) as u32;
        let uses = self.words.get(&d.name).copied().unwrap_or(0).saturating_sub(decls);
        let mut parts = vec![match uses {
            0 => "nenhum uso".to_string(),
            1 => "1 uso".to_string(),
            n => format!("{n} usos"),
        }];
        if matches!(d.kind, Kind::Interface | Kind::Trait | Kind::Class) {
            let n = self.implementors.get(&d.name).map_or(0, |v| v.len());
            if n > 0 || d.kind == Kind::Interface {
                parts.push(if n == 1 { "1 implementação".into() } else { format!("{n} implementações") });
            }
        }
        Some(parts.join("   "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(lang: &str, src: &str) -> Vec<(String, Kind, usize)> {
        declarations(lang, src).into_iter().map(|d| (d.name, d.kind, d.line)).collect()
    }

    #[test]
    fn php_class_methods_and_bases() {
        let src = "<?php\nnamespace App;\n\nfinal class User extends Model implements Auditable, MustVerify\n{\n    public function isAdmin(): bool\n    {\n        return Foo::class;\n    }\n    const MAX = 1;\n}\ninterface Auditable {}\n";
        let d = declarations("php", src);
        assert_eq!(d[0].name, "User");
        assert_eq!(d[0].kind, Kind::Class);
        assert_eq!(d[0].bases, vec!["Model", "Auditable", "MustVerify"]);
        assert_eq!(names("php", src)[1..], [("isAdmin".into(), Kind::Method, 5), ("MAX".into(), Kind::Constant, 9), ("Auditable".into(), Kind::Interface, 11)]);
    }

    #[test]
    fn other_languages() {
        assert_eq!(names("rust", "pub struct A;\nimpl Display for A {\n    fn fmt(&self) {}\n}\nfn main() {}"),
            [("A".into(), Kind::Struct, 0), ("A".into(), Kind::Struct, 1), ("fmt".into(), Kind::Method, 2), ("main".into(), Kind::Function, 4)]);
        assert_eq!(names("go", "func (s *Server) Run() {}\nfunc main() {}"), [("Run".into(), Kind::Method, 0), ("main".into(), Kind::Function, 1)]);
        assert_eq!(names("python", "class A(Base):\n    def run(self):\n        pass\ndef top():\n    pass"),
            [("A".into(), Kind::Class, 0), ("run".into(), Kind::Method, 1), ("top".into(), Kind::Function, 3)]);
        assert_eq!(names("typescript", "export class A implements I {}\nexport const handler = async () => {};\nconst x = 1;"),
            [("A".into(), Kind::Class, 0), ("handler".into(), Kind::Function, 1), ("x".into(), Kind::Constant, 2)]);
        assert_eq!(names("cpp", "int main(int argc, char** argv) {\n    if (x) {\n    return f(1);\n}\nclass Foo : public Bar {"),
            [("main".into(), Kind::Function, 0), ("Foo".into(), Kind::Class, 4)]);
        assert_eq!(names("csharp", "public class Svc : IService\n{\n    public async Task Run(int x)\n    {"), [("Svc".into(), Kind::Class, 0), ("Run".into(), Kind::Method, 2)]);
        assert_eq!(names("asm", "_start:\n  mov rax, 1\nloop:"), [("_start".into(), Kind::Function, 0), ("loop".into(), Kind::Function, 2)]);
    }

    #[test]
    fn usages_and_implementations() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-ix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("I.php"), "<?php\ninterface Shape { public function area(); }\n").unwrap();
        std::fs::write(dir.join("A.php"), "<?php\nclass Sq implements Shape {\n    public function area() { return 1; }\n}\n$s = new Sq(); $s->area(); $t = new Sq();\n").unwrap();
        let files = vec![dir.join("I.php"), dir.join("A.php")];
        let mut ix = Index::build(&dir, &files);
        let shape = ix.symbols.iter().find(|s| s.decl.name == "Shape").unwrap().decl.clone();
        assert_eq!(ix.vision(&shape).unwrap(), "1 uso   1 implementação");
        let sq = ix.symbols.iter().find(|s| s.decl.name == "Sq").unwrap().decl.clone();
        assert_eq!(ix.vision(&sq).unwrap(), "2 usos");
        // re-indexing a file replaces its contribution
        std::fs::write(dir.join("A.php"), "<?php\nclass Sq {}\n").unwrap();
        ix.update(&dir.join("A.php"));
        assert_eq!(ix.vision(&shape).unwrap(), "nenhum uso   0 implementações");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

