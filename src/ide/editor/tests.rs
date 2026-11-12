use super::*;

#[test]
fn labels_open_a_row_above_their_declaration() {
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(egui::RawInput::default(), |_| {}); // sets the fonts up
    out.textures_delta.clear();
    let font = egui::FontId::monospace(13.0);
    let row_h = ctx.fonts_mut(|f| f.row_height(&font)).round();
    let job = egui::text::LayoutJob::simple("a\nb\nc\nd".into(), font, Color32::WHITE, f32::INFINITY);
    let mut galley = ctx.fonts_mut(|f| f.layout_job(job));
    let before: Vec<f32> = galley.rows.iter().map(|r| r.pos.y).collect();
    let height = galley.rows[0].rect().height();
    // two declarations: lines 1 and 3 get a row above them
    open_gaps(std::sync::Arc::make_mut(&mut galley), &[(1, row_h), (3, row_h)]);
    assert_eq!(galley.rows[0].pos.y, before[0]);
    assert_eq!(galley.rows[1].pos.y, before[1] + row_h);
    assert_eq!(galley.rows[2].pos.y, before[2] + row_h);
    assert_eq!(galley.rows[3].pos.y, before[3] + 2.0 * row_h);
    assert_eq!(galley.rows[0].rect().height(), height); // the row above keeps its height
}

#[test]
fn author_of_a_declaration() {
    let lines = ["class A {", "  f() {", "    x", "  }", "", "}", "y"];
    assert_eq!(decl_span(&lines, 0), (0, 6)); // through its closing brace, not `y`
    assert_eq!(decl_span(&lines, 1), (1, 4)); // a method ends at its own brace
    let blame = [Some("ana".to_string()), Some("edsuuu".into()), Some("edsuuu".into()), None];
    assert_eq!(decl_author(&blame, (0, 4)).as_deref(), Some("edsuuu +1"));
    assert_eq!(decl_author(&blame, (3, 4)), None); // only uncommitted lines
}

#[test]
fn a_guide_runs_inside_its_block_at_the_opening_lines_column() {
    // under the opening line, down to the line before the one that closes the block; none at column 0
    assert_eq!(indent_guides("fn f() {\n    if a {\n        x();\n    }\n\n}"), [(4, 1, 3)]);
    // a blank line keeps the guides of the block around it, and the ones a closing bracket ends after it
    let text = "impl A {\n    fn f() {\n        a();\n\n        b();\n\n    }\n\n    fn g() {\n        c();\n    }\n}";
    assert_eq!(indent_guides(text), [(4, 1, 6), (4, 8, 10)]);
    // lines aligned further in get the guide of the line they continue, not one a step
    assert_eq!(indent_guides("mod m {\n    let c = C { a: 1,\n                b: 2 };\n    x();\n}"), [(4, 1, 3)]);
    // a block still open at the end runs to the last line
    assert_eq!(indent_guides("class A:\n    def f(self):\n        pass"), [(4, 1, 3)]);
    // the caret's block is the innermost one around its line
    let guides = [(4, 1, 9), (8, 3, 6)];
    assert_eq!(caret_guide(&guides, 4), Some((8, 3, 6)));
    assert_eq!(caret_guide(&guides, 8), Some((4, 1, 9)));
    assert_eq!(caret_guide(&guides, 0), None);
}
