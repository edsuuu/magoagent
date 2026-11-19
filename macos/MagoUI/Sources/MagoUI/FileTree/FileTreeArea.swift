import SwiftUI

/// The file tree on SwiftUI: `MagoViews.filetree` gives the canvas a `FileTreeView` for a file tree node's id.
@MainActor
public enum FileTreeArea {
    public static func start() {
        MagoViews.filetree = { id in AnyView(FileTreeView(id: id)) }
    }
}

/// The tree (rows, git, search, Alterações) and, when the node has its editor shown, the editor pane beside it.
public struct FileTreeView: View {
    let id: String
    public init(id: String) { self.id = id }

    /// The tree's narrowest, the pane's narrowest and the seam's width (the egui node's own).
    private static let treeMin: CGFloat = 280
    private static let paneMin: CGFloat = 240

    public var body: some View {
        if let state = FT.state(id) {
            GeometryReader { geometry in
                let width = geometry.size.width
                let tree = treeWidth(state, width)
                HStack(spacing: 0) {
                    FTTreeColumn(state: state)
                        .frame(width: state.editorOpen ? tree : width)
                    if state.editorOpen {
                        FTSeam(id: id, treeWidth: tree, dark: state.dark)
                        FTEditorPane(state: state)
                            .frame(maxWidth: .infinity)
                    }
                }
            }
            .background { Color(FTPalette(dark: state.dark).bg).allowsHitTesting(false) }
            .clipShape(RoundedRectangle(cornerRadius: 8.125))
            .overlay { RoundedRectangle(cornerRadius: 8.125).strokeBorder(Color(FTPalette(dark: state.dark).border), lineWidth: 1).allowsHitTesting(false) }
            .overlay(alignment: .bottom) { if let words = state.linkBack { FTLinkBack(id: id, words: words, dark: state.dark).padding(.bottom, 45) } }
            .environment(\.colorScheme, state.dark ? .dark : .light)
            .sheet(isPresented: Binding(get: { state.findOpen }, set: { if !$0 { FT.send("find_close", id) } })) {
                FTFindSheet(treeId: id, root: state.root)
            }
        } else {
            ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    /// What was dragged, or all but the default pane, kept between the narrowest the tree and the pane allow.
    private func treeWidth(_ state: FTState, _ width: CGFloat) -> CGFloat {
        let raw = state.treeWidth.map { CGFloat($0) } ?? (width - 601)
        return max(min(raw, max(width - Self.paneMin - 1, Self.treeMin)), Self.treeMin)
    }
}

/// The one-point line between the tree and the pane; dragged, it moves the split.
struct FTSeam: View {
    let id: String
    let treeWidth: CGFloat
    let dark: Bool
    @State private var start: CGFloat?

    var body: some View {
        Rectangle()
            .fill(Color(FTPalette(dark: dark).border))
            .frame(width: 1)
            .overlay(
                Color.clear
                    .frame(width: 9)
                    .contentShape(Rectangle())
                    .onHover { inside in if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() } }
                    .gesture(
                        DragGesture(minimumDistance: 1, coordinateSpace: .global)
                            .onChanged { drag in
                                let origin = start ?? treeWidth
                                start = origin
                                FT.send("seam", id, ["width": Double((origin + drag.translation.width).rounded())])
                            }
                            .onEnded { _ in start = nil }
                    )
            )
    }
}

/// "Voltar ao chat" (or "Voltar") over the foot of the tree a file link opened (`link_back_ui`): back to where it came from; its x only lets it go.
struct FTLinkBack: View {
    let id: String
    let words: String
    let dark: Bool

    var body: some View {
        let ink = Color(FTPalette(dark: dark).fg)
        HStack(spacing: 0) {
            Button { Task { await FT.act("link_back", id) } } label: {
                HStack(spacing: 6) {
                    FTGlyph(name: "arrow-left", size: 13.5, color: FTPalette(dark: dark).fg, stroke: 1.4)
                    Text(words).font(.app(13.5)).foregroundStyle(ink)
                }
                .padding(.leading, 14).padding(.trailing, 8).frame(height: 34).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            Button { FT.send("link_dismiss", id) } label: {
                FTGlyph(name: "x", size: 12, color: FTPalette(dark: dark).muted, stroke: 1.4).frame(width: 24, height: 24).contentShape(Circle())
            }
            .buttonStyle(.plain).help("Fechar sem voltar").padding(.trailing, 5)
        }
        .glass()
    }
}
