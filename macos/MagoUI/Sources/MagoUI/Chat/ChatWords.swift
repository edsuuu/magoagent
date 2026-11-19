// The agent's words as the egui chat draws them (chat/markdown.rs, chat/media.rs), from the pieces Rust cut
// (native/words.rs): each paragraph a grey bubble of 14 pt lines (headings 20/17/15, quotes with their bar, list items with
// their marks), tables a grid, code a card with Copiar, pictures up to 420×320 and files a card. The files the words
// name are links in the code colour: a click opens them in the Árvore on the canvas (MCP `file_link`, Voltar there).
import AppKit
import SwiftUI

/// The size of a message's words (markdown.rs `PROSE`).
let prose: CGFloat = 14

struct AgentWords: View {
    let chat: String
    let parts: [ChatPart]
    /// The conversation's column: a bubble is up to 720 wide and 48 short of it.
    let width: CGFloat
    @Environment(\.chatLook) private var look

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(parts.enumerated()), id: \.offset) { _, part in
                switch part.kind {
                case "text":
                    ForEach(Array((part.paras ?? []).enumerated()), id: \.offset) { _, para in
                        if let rows = para.rows, !rows.isEmpty {
                            WordsTable(chat: chat, rows: rows)
                        } else {
                            Bubble(chat: chat, lines: para.lines ?? [])
                                .frame(maxWidth: min(width - 48, 720), alignment: .leading)
                        }
                    }
                case "code":
                    CodeCard(info: part.info ?? "", code: part.text ?? "")
                case "image":
                    Picture(path: part.text ?? "")
                default:
                    FileCard(chat: chat, part: part)
                }
            }
        }
    }
}

/// A paragraph in its bubble: radius 16, 12×7 around, hugging its words.
private struct Bubble: View {
    let chat: String
    let lines: [ChatLine]
    @Environment(\.chatLook) private var look

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(lines.enumerated()), id: \.offset) { _, line in MarkdownLine(chat: chat, line: line) }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(RoundedRectangle(cornerRadius: 16).fill(look.agentBubble))
    }
}

/// One line of markdown.rs's `md_lines`.
struct MarkdownLine: View {
    let chat: String
    let line: ChatLine
    @Environment(\.chatLook) private var look

    var body: some View {
        switch line.kind {
        case "heading":
            Runs(chat: chat, runs: line.runs, size: [20, 17, 15][min(max(line.level ?? 1, 1), 3) - 1], color: look.words)
                .padding(.top, 4)
        case "quote":
            // the bar, 3 pt and translucent so it reads on the grey bubble
            Runs(chat: chat, runs: line.runs, size: prose, color: look.muted)
                .padding(.leading, 12)
                .overlay(alignment: .leading) { Rectangle().fill(look.tint(64)).frame(width: 3).padding(.leading, 4.5) }
        case "item":
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(line.mark ?? "•").font(.app(prose)).foregroundStyle(look.muted)
                Runs(chat: chat, runs: line.runs, size: prose, color: look.words)
            }
            .padding(.leading, 4 + CGFloat(line.level ?? 0) * 7)
        default:
            Runs(chat: chat, runs: line.runs, size: prose, color: look.words)
        }
    }
}

/// A line's runs: **bold** (drawn again a pixel to the right, as egui does: the font has no bold), *italics*, `code` in
/// monospace 1.5 smaller in the code colour, and the files named as links.
struct Runs: View {
    let chat: String
    let runs: [ChatRun]
    let size: CGFloat
    let color: Color
    @Environment(\.chatLook) private var look
    @Environment(\.displayScale) private var scale

    var body: some View {
        Text(words(ghost: false))
            .tint(look.code)
            .textSelection(.enabled)
            .fixedSize(horizontal: false, vertical: true)
            .overlay(alignment: .topLeading) {
                if runs.contains(where: { $0.bold == true }) {
                    Text(words(ghost: true)).offset(x: 1 / scale).allowsHitTesting(false)
                }
            }
            .environment(\.openURL, OpenURLAction { url in open(url) })
    }

    /// The whole line, or (`ghost`) its bold runs alone, the rest clear in the same layout.
    private func words(ghost: Bool) -> AttributedString {
        var all = AttributedString()
        for (index, run) in runs.enumerated() {
            var piece = AttributedString(run.text)
            let code = run.code == true
            var font: Font = code ? .mono(max(size - 1.5, size / 2)) : .app(size)
            if run.italic == true { font = font.italic() }
            piece.font = font
            let ink = code || run.path != nil ? look.code : color
            piece.foregroundColor = ghost && run.bold != true ? .clear : ink
            if !ghost, run.path != nil { piece.link = URL(string: "mago-link:\(index)") }
            all += piece
        }
        return all
    }

    private func open(_ url: URL) -> OpenURLAction.Result {
        guard url.scheme == "mago-link", let index = Int(url.absoluteString.dropFirst("mago-link:".count)), let run = runs[safe: index], let path = run.path else {
            return .systemAction
        }
        var args: [String: Any] = ["path": path, "asker": chat]
        if let line = run.line { args["line"] = line }
        MagoStore.shared.send("file_link", args)
        return .handled
    }
}

/// A markdown table (markdown.rs `table`): the header muted and bold, the rows striped, in a thin frame of radius 8; it
/// scrolls sideways when wider than the conversation.
private struct WordsTable: View {
    let chat: String
    let rows: [[[ChatRun]]]
    @Environment(\.chatLook) private var look

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            Grid(alignment: .leading, horizontalSpacing: 0, verticalSpacing: 0) {
                ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                    GridRow {
                        ForEach(Array(row.enumerated()), id: \.offset) { _, cell in
                            Runs(chat: chat, runs: cell, size: 13, color: index == 0 ? look.tint(140) : look.words)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 6)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(index % 2 == 1 ? look.tint(8) : .clear)
                        }
                    }
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(look.tint(26)))
        }
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// A code block (list/mod.rs `code_box`): "lang · file" in its head with Copiar (Copiado a moment after), the code in
/// monospace 13 under it.
struct CodeCard: View {
    let info: String
    let code: String
    /// `code` is the end of something longer: Copiar asks for the whole of it.
    var full: (() async -> String)?
    @Environment(\.chatLook) private var look
    @State private var copied = false

    var body: some View {
        let (fill, edge, quiet, ink) = look.dark ? (look.band, look.line, look.muted, look.text) : (look.surfaceLow, look.tint(20), look.quiet, look.words)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 0) {
                Text(title).font(.mono(13)).foregroundStyle(quiet).lineLimit(1)
                Spacer(minLength: 8)
                Button {
                    Task {
                        let text = await full?() ?? code
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(text, forType: .string)
                        copied = true
                        try? await Task.sleep(for: .seconds(1.5))
                        copied = false
                    }
                } label: {
                    HStack(spacing: 5) {
                        Image(systemName: copied ? "checkmark" : "square.on.square").font(.system(size: 11))
                        Text(copied ? "Copiado" : "Copiar").font(.app(13))
                    }
                    .foregroundStyle(quiet)
                    .padding(.horizontal, 8)
                    .frame(height: 24)
                    .contentShape(Rectangle())
                }
                .buttonStyle(HoverGround(radius: 6, color: look.tint(18)))
            }
            .padding(.leading, 12)
            .padding(.trailing, 6)
            .frame(height: 34)
            Rectangle().fill(edge).frame(height: 1)
            ScrollView(.horizontal, showsIndicators: false) {
                Text(code).font(.mono(13)).foregroundStyle(ink).textSelection(.enabled).padding(.horizontal, 14).padding(.vertical, 10)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 10).fill(fill))
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(edge))
    }

    private var title: String {
        guard let cut = info.firstIndex(where: { $0 == " " || $0 == ":" }) else { return info }
        let file = info[info.index(after: cut)...].trimmingCharacters(in: .whitespaces).replacingOccurrences(of: "title=", with: "").trimmingCharacters(in: CharacterSet(charactersIn: "\""))
        return file.isEmpty ? String(info[..<cut]) : "\(info[..<cut]) · \(file)"
    }
}

/// A button whose ground shows under the pointer.
struct HoverGround: ButtonStyle {
    let radius: CGFloat
    let color: Color
    @State private var hovered = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(RoundedRectangle(cornerRadius: radius).fill(hovered || configuration.isPressed ? color : .clear))
            .onHover { hovered = $0 }
    }
}

/// A picture on disk (media.rs `chat_picture`): up to 420×320, radius 10 with its edge; a click shows it large.
struct Picture: View {
    let path: String
    @Environment(\.chatLook) private var look
    @State private var zoomed = false

    var body: some View {
        if let image = Picture.load(path) {
            let scale = min(420 / image.size.width, 320 / image.size.height, 1)
            Image(nsImage: image)
                .resizable()
                .frame(width: max(image.size.width * scale, 24), height: max(image.size.height * scale, 24))
                .clipShape(RoundedRectangle(cornerRadius: 10))
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(look.edge))
                .help("Clique para ampliar")
                .onTapGesture { zoomed = true }
                .sheet(isPresented: $zoomed) {
                    Image(nsImage: image).resizable().scaledToFit().frame(maxWidth: 1000, maxHeight: 760).onTapGesture { zoomed = false }
                }
        } else {
            Text("Imagem ilegível").font(.app(13)).foregroundStyle(look.muted)
        }
    }

    // ponytail: read once; a file the agent rewrites keeps its first picture (as the egui chat), a cache that never empties
    @MainActor private static var cache: [String: NSImage] = [:]

    @MainActor static func load(_ path: String) -> NSImage? {
        if let image = cache[path] { return image }
        let image = NSImage(contentsOfFile: path)
        cache[path] = image
        return image
    }
}

/// A file the agent names, or a picture on the web (media.rs `card`): radius 16, a wash with a hairline, the page icon,
/// the name over its kind and weight; dimmed when it isn't there. A click opens it: text in a note on the canvas, code in
/// the Árvore, the rest in the system's program; a right click copies its path or shows its folder.
struct FileCard: View {
    let chat: String
    let part: ChatPart
    @Environment(\.chatLook) private var look
    @State private var hovered = false

    var body: some View {
        let missing = part.kind == "file" && (part.opens ?? "").isEmpty
        let fade = missing ? 0.6 : 1.0
        let path = part.text ?? ""
        HStack(spacing: 10) {
            Image(systemName: part.kind == "web" ? "photo" : "doc.text").font(.system(size: 20, weight: .light)).foregroundStyle(look.muted).frame(width: 32, height: 32)
            VStack(alignment: .leading, spacing: 1) {
                Text(part.info ?? "").font(.app(14)).foregroundStyle(look.text).lineLimit(1).truncationMode(.middle)
                Text(part.detail ?? "").font(.app(12)).foregroundStyle(look.muted).lineLimit(1)
            }
        }
        .padding(.leading, 10)
        .padding(.trailing, 14)
        .frame(height: 48)
        .frame(maxWidth: 420, alignment: .leading)
        .fixedSize(horizontal: true, vertical: false)
        .background(RoundedRectangle(cornerRadius: 16).fill(look.tint(hovered ? 22 : 12)))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(look.tint(20)))
        .opacity(fade)
        .contentShape(RoundedRectangle(cornerRadius: 16))
        .onHover { hovered = $0 }
        .help(path)
        .onTapGesture { open(path) }
        .contextMenu {
            if part.kind == "file" {
                if part.opens == "note" || part.opens == "tree" {
                    Button("Abrir no programa padrão") { MagoStore.shared.send("open_default", ["path": path]) }
                }
                Button("Copiar caminho") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(path, forType: .string)
                }
                if !missing {
                    Button("Mostrar na pasta") { MagoStore.shared.send("open_default", ["path": (path as NSString).deletingLastPathComponent]) }
                }
            }
        }
    }

    private func open(_ path: String) {
        switch part.kind == "web" ? "web" : (part.opens ?? "") {
        case "web": if let url = URL(string: path) { NSWorkspace.shared.open(url) }
        case "note": MagoStore.shared.send("open_as_note", ["path": path, "chat": chat])
        case "tree": MagoStore.shared.send("file_link", ["path": path, "asker": chat])
        case "default": MagoStore.shared.send("open_default", ["path": path])
        default: break
        }
    }
}
