import AppKit
import PDFKit
import SwiftUI

/// The words in the middle of the pane (why there is nothing to show), with links under them.
struct FTMessage: View {
    let text: String
    let links: [(String, () -> Void)]

    var body: some View {
        VStack(spacing: 8) {
            Text(text).font(.app(12)).foregroundStyle(.secondary).multilineTextAlignment(.center)
            ForEach(Array(links.enumerated()), id: \.offset) { _, link in
                Button(link.0, action: link.1).buttonStyle(.link).font(.app(12))
            }
        }
        .padding(13).frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// A picture, fitted (or at its size after a click, with its dimensions) or a PDF's pages; loaded from the file by Swift.
struct FTMediaView: View {
    let state: FTState
    let doc: FTDoc
    @State private var actual = false

    var body: some View {
        VStack(spacing: 0) {
            if doc.pdf {
                FTPDFView(path: doc.path)
            } else if let image = NSImage(contentsOfFile: doc.path) {
                GeometryReader { geometry in
                    let natural = pixelSize(image)
                    let room = CGSize(width: max(geometry.size.width - 13, 1), height: max(geometry.size.height - 13, 1))
                    let fit = min(room.width / natural.width, room.height / natural.height, 1)
                    ScrollView([.horizontal, .vertical]) {
                        Image(nsImage: image).resizable().interpolation(.high)
                            .frame(width: actual ? natural.width : natural.width * fit, height: actual ? natural.height : natural.height * fit)
                            .frame(minWidth: geometry.size.width, minHeight: geometry.size.height)
                            .onTapGesture { actual.toggle() }
                            .onHover { inside in if inside { NSCursor.pointingHand.push() } else { NSCursor.pop() } }
                    }
                }
            } else {
                FTMessage(
                    text: "Não foi possível ler este arquivo.",
                    links: [
                        ("Tentar Novamente", { FT.send("retry", state.id, ["path": doc.path]) }),
                        ("Mostrar na Pasta", { FT.send("reveal", state.id, ["path": doc.path]) }),
                    ])
            }
            Divider()
            Text(footer).font(.app(11)).foregroundStyle(.secondary).frame(height: 22)
        }
    }

    private var footer: String {
        guard !doc.pdf, let image = NSImage(contentsOfFile: doc.path) else { return doc.sizeText ?? "" }
        let size = pixelSize(image)
        return "\(Int(size.width)) × \(Int(size.height)) · \(doc.sizeText ?? "")"
    }

    /// The picture's own size in pixels (its first bitmap's), not the points the image says.
    private func pixelSize(_ image: NSImage) -> CGSize {
        if let rep = image.representations.first, rep.pixelsWide > 0 { return CGSize(width: rep.pixelsWide, height: rep.pixelsHigh) }
        return image.size
    }
}

struct FTPDFView: NSViewRepresentable {
    let path: String

    func makeNSView(context: Context) -> PDFView {
        let view = PDFView()
        view.autoScales = true
        view.displayMode = .singlePageContinuous
        view.backgroundColor = NSColor(srgbRed: 0.32, green: 0.34, blue: 0.35, alpha: 1)
        view.document = PDFDocument(url: URL(fileURLWithPath: path))
        return view
    }

    func updateNSView(_ view: PDFView, context: Context) {
        if view.document?.documentURL?.path != path { view.document = PDFDocument(url: URL(fileURLWithPath: path)) }
    }
}

/// A commit's diff of a file, as git wrote it: added lines green, removed red, the hunks' headers as links. Read only; it can be selected.
struct FTCommitDiff: View {
    let text: String

    var body: some View {
        if let theme = FT.theme {
            ScrollView([.vertical, .horizontal]) {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(text.split(separator: "\n", omittingEmptySubsequences: false).enumerated()), id: \.offset) { _, line in
                        Text(String(line).isEmpty ? " " : String(line))
                            .font(.custom("Hack", size: 13))
                            .foregroundStyle(color(String(line), theme))
                            .textSelection(.enabled)
                    }
                }
                .padding(.horizontal, 10).padding(.vertical, 6)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(Color(hex: theme.background))
        }
    }

    private func color(_ line: String, _ theme: FTTheme) -> Color {
        switch line.first {
        case "+" where !line.hasPrefix("+++"): Color(hex: theme.string)
        case "-" where !line.hasPrefix("---"): Color(hex: theme.error)
        case "@": Color(hex: theme.link)
        case " ": Color(hex: theme.text)
        default: Color(hex: theme.comment)
        }
    }
}

/// A line with its changed words (UTF-16 ranges) on a stronger background.
func ftWords(_ text: String, _ words: [[Int]], _ color: Color) -> AttributedString {
    var attributed = AttributedString(text)
    for range in words where range.count == 2 {
        let utf16 = text.utf16
        guard range[0] < range[1], range[1] <= utf16.count,
            let from = utf16.index(utf16.startIndex, offsetBy: range[0], limitedBy: utf16.endIndex),
            let to = utf16.index(utf16.startIndex, offsetBy: range[1], limitedBy: utf16.endIndex),
            let lower = AttributedString.Index(from, within: attributed), let upper = AttributedString.Index(to, within: attributed)
        else { continue }
        attributed[lower..<upper].backgroundColor = color
    }
    return attributed
}

/// A file against the last commit, side by side or in one column: removed lines a soft red, added ones a soft green, the changed words
/// stronger; long unchanged stretches fold into a row that opens when pressed.
struct FTCompareView: View {
    let state: FTState
    let tab: String
    let compare: FTCompare

    var body: some View {
        if let theme = FT.theme {
            VStack(spacing: 0) {
                toolbar(theme)
                Divider()
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(compare.rows.enumerated()), id: \.offset) { index, row in
                                rowView(row, theme).id(index)
                            }
                        }
                    }
                    .background(Color(hex: theme.background))
                    .onChange(of: compare.current) { _, current in
                        if compare.blocks.indices.contains(current) { withAnimation { proxy.scrollTo(compare.blocks[current], anchor: .center) } }
                    }
                    .onAppear {
                        if compare.blocks.indices.contains(compare.current) { proxy.scrollTo(compare.blocks[compare.current], anchor: .center) }
                    }
                }
            }
        }
    }

    private func toolbar(_ theme: FTTheme) -> some View {
        HStack(spacing: 8) {
            Button { FT.send("compare", state.id, ["path": tab, "go": -1]) } label: { Image(systemName: "chevron.up") }
                .disabled(compare.current <= 0).help("Mudança anterior")
            Button { FT.send("compare", state.id, ["path": tab, "go": 1]) } label: { Image(systemName: "chevron.down") }
                .disabled(compare.current + 1 >= compare.blocks.count).help("Próxima mudança")
            Text(compare.blocks.isEmpty ? "Sem diferenças" : "\(min(compare.current + 1, compare.blocks.count)) de \(compare.blocks.count)").foregroundStyle(.secondary)
            Text("−\(compare.removed) +\(compare.added)").foregroundStyle(.secondary)
            Spacer()
            Toggle("Unificado", isOn: Binding(get: { compare.unified }, set: { FT.send("compare", state.id, ["path": tab, "unified": $0]) }))
            Toggle("Recolher trechos sem mudança", isOn: Binding(get: { compare.collapse }, set: { FT.send("compare", state.id, ["path": tab, "collapse": $0]) }))
        }
        .toggleStyle(.checkbox).controlSize(.small).font(.app(11))
        .padding(.horizontal, 8).frame(height: 28)
    }

    @ViewBuilder
    private func rowView(_ row: FTCompareRow, _ theme: FTTheme) -> some View {
        if row.kind == "fold" {
            Button {
                FT.send("compare", state.id, ["path": tab, "fold": row.foldKey ?? 0])
            } label: {
                Text("\(row.foldCount ?? 0) linhas sem mudança").font(.app(11)).foregroundStyle(Color(hex: theme.muted))
                    .frame(maxWidth: .infinity).frame(height: 22).background(Color.white.opacity(0.04))
            }
            .buttonStyle(.plain)
        } else if compare.unified {
            let removed = row.left != nil
            line(
                number: removed ? row.leftNumber : row.rightNumber, text: row.left ?? row.right ?? "", words: removed ? row.leftWords : row.rightWords,
                fill: row.kind == "same" ? nil : Color(hex: removed ? theme.removedLine : theme.addedLine), word: Color(hex: removed ? theme.removedWord : theme.addedWord), theme)
        } else {
            HStack(spacing: 0) {
                line(
                    number: row.leftNumber, text: row.left ?? "", words: row.leftWords,
                    fill: row.kind == "same" ? nil : (row.left == nil ? Color.white.opacity(0.03) : Color(hex: theme.removedLine)), word: Color(hex: theme.removedWord), theme)
                Rectangle().fill(Color.white.opacity(0.1)).frame(width: 1)
                line(
                    number: row.rightNumber, text: row.right ?? "", words: row.rightWords,
                    fill: row.kind == "same" ? nil : (row.right == nil ? Color.white.opacity(0.03) : Color(hex: theme.addedLine)), word: Color(hex: theme.addedWord), theme)
            }
        }
    }

    private func line(number: Int?, text: String, words: [[Int]], fill: Color?, word: Color, _ theme: FTTheme) -> some View {
        HStack(spacing: 0) {
            Text(number.map(String.init) ?? "").font(.custom("Hack", size: 12)).foregroundStyle(Color(hex: theme.lineNumber))
                .frame(width: 44, alignment: .trailing).padding(.trailing, 8)
            // ponytail: a long line is cut at the column's edge; scroll sideways when it matters
            Text(ftWords(text.isEmpty ? " " : text, words, word)).font(.custom("Hack", size: 13)).foregroundStyle(Color(hex: theme.text))
                .lineLimit(1).textSelection(.enabled)
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, minHeight: 19, alignment: .leading)
        .background(fill ?? Color.clear)
    }
}

/// What a "Localizar nos Arquivos" found, kept in a tab to look at while the code is edited.
struct FTResultsView: View {
    let state: FTState
    let results: FTResults

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(results.title).font(.app(12)).lineLimit(1)
                Spacer()
                Text(results.countNote).font(.app(11)).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 10).frame(height: 28)
            Divider()
            FTHitList(treeId: state.id, hits: results.hits, selected: results.selected, open: { hit in
                FT.send("goto", state.id, ["path": hit.path, "line": hit.line, "col": hit.col, "pinned": true])
            })
        }
    }
}

/// Matches one to a line: the line's text and, at the right, the file and the line.
struct FTHitList: View {
    let treeId: String
    let hits: [FTHit]
    var selected: Int?
    var pick: ((Int, FTHit) -> Void)?
    let open: (FTHit) -> Void

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(hits.enumerated()), id: \.element.id) { index, hit in
                        HStack(spacing: 10) {
                            Text(hit.text.trimmingCharacters(in: .whitespaces)).font(.custom("Hack", size: 12.5)).lineLimit(1)
                            Spacer(minLength: 8)
                            Text("\(hit.name) \(hit.line + 1)").font(.app(11.5)).foregroundStyle(.secondary).lineLimit(1)
                        }
                        .padding(.horizontal, 14).frame(height: 24)
                        .background(RoundedRectangle(cornerRadius: 6).fill(selected == index ? Color.accentColor.opacity(0.28) : Color.clear).padding(.horizontal, 6))
                        .contentShape(Rectangle())
                        .onTapGesture(count: 2) { open(hit) }
                        .onTapGesture { (pick ?? { _, hit in open(hit) })(index, hit) }
                        .help(hit.rel)
                        .id(index)
                    }
                }
            }
            .onChange(of: selected) { _, index in if let index { proxy.scrollTo(index) } }
        }
    }
}
