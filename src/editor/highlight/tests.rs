use super::*;

fn styles(lang: &Lang, src: &str) -> Vec<(String, St)> {
    tokens(lang, src).into_iter().map(|(a, b, s)| (src[a..b].to_string(), s)).collect()
}

#[test]
fn php() {
    let t = styles(&PHP, "<?php // hi\nfunction foo($a) { return $this->bar() . $this->baz . \"x\"; } #[Attr(['a'])]");
    assert!(t.contains(&("// hi".into(), St::Comment)));
    assert!(t.contains(&("foo".into(), St::Function)));
    assert!(t.contains(&("$this".into(), St::Keyword)));
    assert!(t.contains(&("bar".into(), St::Method)));
    assert!(t.contains(&("baz".into(), St::Field)));
    assert!(t.contains(&("\"x\"".into(), St::Str)));
    assert!(t.contains(&("#[Attr(['a'])]".into(), St::Meta)));
}

#[test]
fn rust_lifetime_vs_char() {
    let t = styles(&RUST, "fn f<'a>(x: &'a str) -> char { 'z' } /// doc");
    assert!(t.contains(&("'a".into(), St::Keyword)));
    assert!(t.contains(&("'z'".into(), St::Str)));
    assert!(t.contains(&("/// doc".into(), St::Doc)));
}

#[test]
fn blade_and_html() {
    let t = styles(&BLADE, "<div class=\"x\">{{ $user->name }}</div>@if($a)<script>let x = 1;</script>{{-- c --}}");
    assert!(t.contains(&("div".into(), St::Tag)));
    assert!(t.contains(&("\"x\"".into(), St::Str)));
    assert!(t.contains(&("name".into(), St::Field)));
    assert!(t.contains(&("@if".into(), St::Keyword)));
    assert!(t.contains(&("let".into(), St::Keyword)));
    assert!(t.contains(&("{{-- c --}}".into(), St::Comment)));
}

#[test]
fn asm_sql_json() {
    let t = styles(&ASM, "_start:\n  mov rax, 60 ; exit\nsection .text");
    assert!(t.contains(&("_start".into(), St::Function)));
    assert!(t.contains(&("mov".into(), St::Keyword)));
    assert!(t.contains(&("rax".into(), St::Field)));
    assert!(t.contains(&("; exit".into(), St::Comment)));
    assert!(styles(&SQL, "select * FROM t").contains(&("FROM".into(), St::Keyword)));
    assert!(styles(&JSON, "{\"k\": \"v\"}").contains(&("\"k\"".into(), St::Field)));
}

#[test]
fn rainbow_brackets() {
    // brackets inside a string or a comment don't count; a closer takes its opener's level
    let brackets = |src: &str| -> Vec<(String, St)> { styles(&JS, src).into_iter().filter(|(_, s)| matches!(s, St::Bracket(_))).collect() };
    assert_eq!(
        brackets("f(\"(\", /* [ */ [1]) { x }"),
        vec![
            ("(".into(), St::Bracket(0)),
            ("[".into(), St::Bracket(0)),
            ("]".into(), St::Bracket(0)),
            (")".into(), St::Bracket(0)),
            ("{".into(), St::Bracket(0)),
            ("}".into(), St::Bracket(0))
        ]
    );
    // a missing ')' is closed by the '}' that comes after it
    assert_eq!(brackets("{ ( }"), vec![("{".into(), St::Bracket(0)), ("(".into(), St::Bracket(0)), ("}".into(), St::Bracket(0))]);
    // each kind counts its own levels: the '(' inside '[' and '{' is still level 0; a '(' inside '(' is level 1
    assert_eq!(
        brackets("({[(x)]})"),
        vec![
            ("(".into(), St::Bracket(0)),
            ("{".into(), St::Bracket(0)),
            ("[".into(), St::Bracket(0)),
            ("(".into(), St::Bracket(1)),
            (")".into(), St::Bracket(1)),
            ("]".into(), St::Bracket(0)),
            ("}".into(), St::Bracket(0)),
            (")".into(), St::Bracket(0))
        ]
    );
    assert_eq!(brackets("((()))").iter().map(|b| b.1).collect::<Vec<_>>(), [0, 1, 2, 2, 1, 0].map(St::Bracket));
    // a closer with no opener stays plain
    assert!(brackets(") ]").is_empty());
    // PHP tags pair with `?>` and take the level of their own kind
    let php = |src: &str| -> Vec<(String, St)> { styles(&PHP, src).into_iter().filter(|(_, s)| matches!(s, St::Bracket(_))).collect() };
    // the last `?>` has no opener left, so it stays plain
    assert_eq!(
        php("<?php f(); ?><?= $a ?>?>"),
        vec![
            ("<?php".into(), St::Bracket(0)),
            ("(".into(), St::Bracket(0)),
            (")".into(), St::Bracket(0)),
            ("?>".into(), St::Bracket(0)),
            ("<?=".into(), St::Bracket(0)),
            ("?>".into(), St::Bracket(0))
        ]
    );
    // a `{` opened in one island closes in the next
    assert_eq!(php("<?php if (a) { ?>x<?php } ?>").iter().map(|b| b.0.as_str()).collect::<Vec<_>>(), ["<?php", "(", ")", "{", "?>", "<?php", "}", "?>"]);
}

#[test]
fn bracket_pairs_match_by_kind_and_level() {
    let src = "f(a, [b]) (]";
    let b0 = St::Bracket(0);
    assert_eq!(pair(&JS, src, 1), Some((1..2, b0, Some(8..9))));
    assert_eq!(pair(&JS, src, 5), Some((5..6, b0, Some(7..8)))); // a level of its own kind: `[` is level 0
    assert_eq!(pair(&JS, src, 10), Some((10..11, b0, None))); // a `]` is not a `)`'s pair
    assert_eq!(pair(&JS, src, 2), None); // not a bracket
    assert_eq!(pair(&JS, "((1)(2))", 0), Some((0..1, b0, Some(7..8))));
    assert_eq!(pair(&JS, "((1)(2))", 1), Some((1..2, St::Bracket(1), Some(3..4))));
    assert_eq!(pair(&PHP, "<?php f(); ?>", 0), Some((0..5, b0, Some(11..13))));
}

#[test]
fn layout_covers_all_text() {
    // tokens must be sorted, non-overlapping and on char boundaries, or LayoutJob panics.
    let src = "<p>ção</p>{{ $é }} @foo(1) <style>a{b:1px}</style>";
    let mut at = 0;
    for (a, b, _) in tokens(&BLADE, src) {
        assert!(a >= at && b >= a && src.is_char_boundary(a) && src.is_char_boundary(b), "{a}..{b}");
        at = b;
    }
}

/// The seven newest languages are found by extension and color keywords, strings and comments.
#[test]
fn newer_languages() {
    for (file, id, src, kw, comment) in [
        (
            "A.java",
            "java",
            "// c
class A { String s = \"x\"; }",
            "class",
            "// c",
        ),
        (
            "a.kt",
            "kotlin",
            "// c
fun main() { val s = \"x\" }",
            "fun",
            "// c",
        ),
        (
            "a.swift",
            "swift",
            "// c
func f() { let s = \"x\" }",
            "func",
            "// c",
        ),
        (
            "a.dart",
            "dart",
            "// c
void main() { final s = \"x\"; }",
            "final",
            "// c",
        ),
        (
            "a.rb",
            "ruby",
            "# c
def f
  s = \"x\"
end",
            "def",
            "# c",
        ),
        (
            "a.lua",
            "lua",
            "-- c
local s = \"x\"
function f() end",
            "function",
            "-- c",
        ),
    ] {
        let lang = detect(std::path::Path::new(file));
        assert_eq!(lang.id, id, "{file}");
        let t = styles(lang, src);
        assert!(t.contains(&(kw.into(), St::Keyword)), "{file}: {t:?}");
        assert!(t.contains(&(comment.into(), St::Comment)), "{file}");
        assert!(t.contains(&("\"x\"".into(), St::Str)), "{file}");
    }
    assert_eq!(detect(std::path::Path::new("README.md")).id, "markdown");
    assert!(styles(detect(std::path::Path::new("a.md")), "a <!-- c --> <b>").contains(&("<!-- c -->".into(), St::Comment)));
    assert!(
        styles(
            &LUA,
            "--[[ a
b ]] x"
        )
        .contains(&(
            "--[[ a
b ]]"
                .into(),
            St::Comment
        ))
    );
}
