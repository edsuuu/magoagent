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
                        FTSeam(id: id, treeWidth: tree)
                        FTEditorPane(state: state)
                            .frame(maxWidth: .infinity)
                    }
                }
            }
            .background(Color(nsColor: .windowBackgroundColor))
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
    @State private var start: CGFloat?

    var body: some View {
        Rectangle()
            .fill(Color.primary.opacity(0.12))
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
