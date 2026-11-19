import AppKit
import SwiftUI

/// The picked file's diff in a card (app/panes/changes/diff.rs): the old and new numbers and the mark in a gutter, the code
/// in its language's colors scrolling sideways, a whole row of green or red under an added or removed line. A click picks
/// a line for a comment (again: lets it go); Shift+click or a drag picks more.
struct DiffBox: View {
    let rows: [DiffRow]
    let picked: [Int]?
    let look: PaneLook
    @State private var drag: (start: Int, last: Int, moved: Bool)?

    private let line: CGFloat = 17
    /// Hack's advance at 12 points.
    private let charWidth: CGFloat = 12 * 0.602

    private var faint: Color { Color(rgba: look.ink).opacity(0.45) }
    private var numberWidth: CGFloat {
        let widest = rows.map { max($0.old ?? 0, $0.new ?? 0) }.max() ?? 0
        return CGFloat(max(String(widest).count, 2)) * charWidth + 8
    }
    private var textWidth: CGFloat {
        CGFloat(rows.map { $0.runs.reduce(0) { $0 + $1.text.count } }.max() ?? 0) * charWidth + 12
    }

    var body: some View {
        Group {
            if rows.isEmpty {
                Text("Não há diferenças com o HEAD.").font(.app(14))
            } else {
                HStack(alignment: .top, spacing: 0) {
                    gutter
                    GeometryReader { room in
                        ScrollView(.horizontal) {
                            VStack(alignment: .leading, spacing: 0) {
                                ForEach(rows.indices, id: \.self) { index in
                                    code(rows[index]).frame(width: max(room.size.width, textWidth), height: line, alignment: .leading).background { ground(index) }
                                }
                            }
                        }
                        .scrollIndicators(.automatic)
                    }
                    .frame(height: CGFloat(rows.count) * line)
                }
                .coordinateSpace(.named("diff"))
                .contentShape(Rectangle())
                .gesture(pick)
                .onHover { inside in if inside { NSCursor.pointingHand.push() } else { NSCursor.pop() } }
                .accessibilityLabel("Linhas do diff")
            }
        }
        .paneCard(look)
        if !rows.isEmpty {
            Text("Clique numa linha para comentar; Shift+clique ou arraste para pegar mais.").font(.app(11)).foregroundStyle(Color(rgba: look.muted))
        }
    }

    private var gutter: some View {
        let width = numberWidth
        return VStack(alignment: .leading, spacing: 0) {
            ForEach(rows.indices, id: \.self) { index in
                let row = rows[index]
                HStack(spacing: 0) {
                    Text(row.old.map(String.init) ?? "").frame(width: width - 4, alignment: .trailing)
                    Text(row.new.map(String.init) ?? "").frame(width: width, alignment: .trailing)
                    Text(row.mark == " " ? "" : row.mark).font(.code(12)).foregroundStyle(markColor(row.mark)).padding(.leading, 6)
                    Spacer(minLength: 0)
                }
                .font(.code(11)).foregroundStyle(faint)
                .frame(width: width * 2 + charWidth + 6, height: line)
                .background { ground(index) }
            }
        }
    }

    @ViewBuilder private func code(_ row: DiffRow) -> some View {
        if row.mark == "…" {
            Text("⋯").font(.code(12)).foregroundStyle(faint).padding(.leading, 4)
        } else {
            Text(colored(row)).font(.code(12)).lineLimit(1).fixedSize().padding(.leading, 4)
        }
    }

    private func colored(_ row: DiffRow) -> AttributedString {
        var out = AttributedString()
        for run in row.runs {
            var part = AttributedString(run.text)
            part.foregroundColor = Color(rgba: run.color)
            out += part
        }
        return out
    }

    private func markColor(_ mark: String) -> Color {
        switch mark {
        case "+": Color(rgba: look.added)
        case "-": Color(rgba: look.danger)
        default: faint
        }
    }

    /// The row's green or red, with the accent over it while picked (the change still shows).
    private func ground(_ index: Int) -> some View {
        let mark = rows[index].mark
        let change = mark == "+" ? Color(rgba: look.addedRow) : mark == "-" ? Color(rgba: look.removedRow) : .clear
        let lit = picked.map { index >= min($0[0], $0[1]) && index <= max($0[0], $0[1]) } ?? false
        return change.overlay { Color(rgba: look.accent).opacity(lit ? 0.3 : 0) }
    }

    private func rowAt(_ y: CGFloat) -> Int {
        min(max(Int((y / line).rounded(.down)), 0), rows.count - 1)
    }

    /// A press that does not move is a click (Shift stretches); one that moves picks from where it started to where it is.
    private var pick: some Gesture {
        DragGesture(minimumDistance: 0, coordinateSpace: .named("diff"))
            .onChanged { gesture in
                let shift = NSEvent.modifierFlags.contains(.shift)
                guard var state = drag else {
                    let start = rowAt(gesture.startLocation.y)
                    drag = (start, start, false)
                    return
                }
                if !state.moved {
                    guard hypot(gesture.translation.width, gesture.translation.height) >= 3 else { return }
                    state.moved = true
                    paneVerb("pick", ["row": state.start, "extend": shift])
                }
                let row = rowAt(gesture.location.y)
                if row != state.last {
                    paneVerb("pick", ["row": row, "extend": true])
                    state.last = row
                }
                drag = state
            }
            .onEnded { gesture in
                if drag?.moved != true {
                    paneVerb("pick", ["row": rowAt(gesture.location.y), "extend": NSEvent.modifierFlags.contains(.shift), "toggle": true])
                }
                drag = nil
            }
    }
}
