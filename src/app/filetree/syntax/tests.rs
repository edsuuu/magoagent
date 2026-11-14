use super::check;

fn found(id: &str, text: &str) -> Vec<(usize, usize, String)> {
    check(id, text).into_iter().map(|d| (d.line, d.col, d.message)).collect()
}

#[test]
fn javascript_and_typescript() {
    assert!(found("javascript", "const a = { b: [1, 2] };\nconsole.log(`x ${a}`, /re[/]/g);\n").is_empty());
    assert!(found("javascriptreact", "const A = () => <div className=\"x\">{1}</div>;\n").is_empty());
    assert!(found("typescript", "interface A { b: number }\nconst f = <T,>(x: T): T => x;\n").is_empty());
    assert!(found("typescriptreact", "const A = (p: { n: number }) => <b>{p.n}</b>;\n").is_empty());
    let bad = found("javascript", "function f() {\n  const x = (1;\n}\n");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert_eq!(bad[0].0, 1);
    let open = found("javascript", "if (a) {\n  b();\n");
    assert_eq!(open, vec![(1, 6, "Esperado `}`".to_string())]);
    assert!(!found("typescript", "const x: number = ;\n").is_empty());
    assert!(!found("typescriptreact", "const A = () => <div>;\n").is_empty());
}

#[test]
fn php() {
    assert!(found("php", "<p>it's</p>\n<?php\nnamespace A;\nclass B { public function f(int $x): string { return \"a{$x}\"; } }\n?>\n").is_empty());
    let bad = found("php", "<?php\n$a = 1\n$b = 2;\n");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert_eq!(bad[0].0, 1);
    assert!(!found("php", "<?php\nfunction f( { }\n").is_empty());
}

#[test]
fn rust() {
    assert!(found("rust", "fn f<'a>(x: &'a str) -> char { let r = r#\"(\"#; x.chars().next().unwrap_or('(') }\n").is_empty());
    assert_eq!(found("rust", "fn main() {\n    let x = 1\n    let y = 2;\n}\n").len(), 1);
    assert_eq!(found("rust", "fn main() {\n"), vec![(0, 10, "`{` sem o `}` correspondente".to_string())]);
    let bad = found("rust", "fn main() {\n    let x = (1;\n}\n");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert_eq!(bad[0].0, 1);
}

#[test]
fn blade() {
    let good = "@extends('layouts.app')\n@section('title', 'Home')\n@section('content')\n@if ($a > 1)\n  {{ $a }} {!! $b !!}\n@else\n  x\n@endif\n@foreach ($users as $user)\n  <li>{{ $user->name ?? 'n' }}</li>\n@endforeach\n@php\n  $z = 1;\n@endphp\n@endsection\n<a href=\"a@b.com\">{{-- {{ ( --}}</a>\n";
    assert_eq!(found("blade", good), vec![]);
    let unpaired = found("blade", "@if ($a)\n  x\n@foreach ($b as $c)\n@endforeach\n");
    assert_eq!(unpaired, vec![(0, 0, "`@if` sem `@endif`".to_string())]);
    assert_eq!(found("blade", "x\n@endforeach\n"), vec![(1, 0, "`@endforeach` sem `@foreach`".to_string())]);
    assert_eq!(found("blade", "@section('a')\n@if ($x)\n@endsection\n"), vec![(1, 0, "`@if` sem `@endif`".to_string())]);
    assert_eq!(found("blade", "<b>{{ $a </b>\n"), vec![(0, 3, "`{{` sem `}}`".to_string())]);
    let php = found("blade", "<b>{{ $a + }}</b>\n");
    assert_eq!(php.len(), 1, "{php:?}");
    assert_eq!(php[0].0, 0);
    assert!(!found("blade", "@php\n  $a = ;\n@endphp\n").is_empty());
    assert!(!found("blade", "@if ($a >)\n@endif\n").is_empty());
}
