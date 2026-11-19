import AppKit
import SwiftUI

/// The canvas on macOS: Rust's scene drawn as it publishes it (src/app/canvas/native.rs), the pan and zoom done here,
/// the pointer and the keys sent back as `ui.canvas.*`.
struct CanvasView: View {
    private let store = MagoStore.shared
    @State private var cache = CanvasCache()
    @State private var camera = CanvasViewport()
    @State private var serial: UInt64 = 0
    @State private var size = CGSize.zero
    /// Bumped when a picture or the hand font arrives, so the canvas draws again.
    @State private var arrivals = 0

    var body: some View {
        let list = store.decode(CanvasSceneList.self, topic: "scene")
        let overlay = store.decode(CanvasOverlay.self, topic: "canvas")
        let entries = list.map { cache.sync($0, store: store) } ?? []
        let hand = CanvasFonts.hand
        let images = cache.images.mapValues(\.image)
        let camera = camera
        let dark = overlay?.dark ?? false
        let _ = arrivals
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                Canvas { context, size in
                    context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(canvasColor(list?.ground ?? [255, 255, 255, 255])))
                    if let grid = list?.grid { paintGrid(&context, size: size, camera: camera, color: canvasColor(grid)) }
                    let seen = camera.world(size)
                    var world = context
                    world.translateBy(x: camera.x, y: camera.y)
                    world.scaleBy(x: camera.zoom, y: camera.zoom)
                    for entry in entries where canvasRect(entry.drawing.reach).intersects(seen) {
                        paintEntry(&world, entry, hand: hand, image: images[entry.id], dark: dark)
                    }
                }
                CanvasInput(handTool: overlay?.tool == "hand") { handle($0) }
                ForEach(entries.filter { $0.drawing.node != nil }, id: \.id) { entry in
                    hosted(entry, camera: camera, seen: camera.world(geometry.size))
                }
                MagoViews.pages?()
                if let overlay {
                    Canvas { context, _ in paintOverlay(&context, overlay, camera: camera, hand: hand) }
                        .allowsHitTesting(false)
                    if let editing = overlay.editing {
                        CanvasTextEditor(editing: editing, camera: camera, hand: hand).id(editing.id)
                    }
                }
            }
            .clipped()
            .onAppear { resize(geometry.size) }
            .onChange(of: geometry.size) { resize(geometry.size) }
        }
        .onChange(of: overlay?.camera.serial, initial: true) { adopt(overlay?.camera) }
        .onChange(of: self.camera) { sendView() }
        .task { await CanvasFonts.register(store: store); arrivals += 1 }
        .task(id: list?.version) { await fetchImages(entries) }
    }

    /// A window's live view over its card, below the title strip (the strip stays Rust's: it moves the window).
    @ViewBuilder
    private func hosted(_ entry: CanvasEntry, camera: CanvasViewport, seen: CGRect) -> some View {
        let rect = canvasRect(entry.drawing.rect)
        if let node = entry.drawing.node, rect.intersects(seen), let view = CanvasArea.view(kind: node.kind, id: entry.id) {
            let inner = CGSize(width: rect.width, height: max(rect.height - node.header, 0))
            let at = camera.screen(rect.minX, rect.minY + node.header)
            view.frame(width: inner.width, height: inner.height)
                .scaleEffect(camera.zoom, anchor: .topLeading)
                .frame(width: inner.width * camera.zoom, height: inner.height * camera.zoom, alignment: .topLeading)
                .clipShape(UnevenRoundedRectangle(bottomLeadingRadius: 8 * camera.zoom, bottomTrailingRadius: 8 * camera.zoom))
                .offset(x: at.x, y: at.y)
        }
    }

    private func viewArgs() -> [String: Any] {
        ["x": Double(camera.x), "y": Double(camera.y), "zoom": Double(camera.zoom), "width": Double(size.width), "height": Double(size.height)]
    }

    private func sendView() {
        store.send("ui.canvas.view", viewArgs())
    }

    private func resize(_ size: CGSize) {
        self.size = size
        sendView()
    }

    /// Rust moved the camera (zoom to fit, focus_canvas, a glide): its camera wins.
    private func adopt(_ rust: CanvasCamera?) {
        guard let rust, rust.serial != serial else { return }
        serial = rust.serial
        camera = CanvasViewport(x: rust.x, y: rust.y, zoom: rust.zoom)
    }

    private func handle(_ event: CanvasInputEvent) {
        switch event {
        case let .pointer(phase, at, flags, clicks, middle):
            var args = viewArgs()
            args["phase"] = phase
            args["px"] = Double(at.x)
            args["py"] = Double(at.y)
            args["clicks"] = clicks
            args["button"] = middle ? "middle" : "primary"
            args["shift"] = flags.contains(.shift)
            args["alt"] = flags.contains(.option)
            args["cmd"] = flags.contains(.command)
            store.send("ui.canvas.pointer", args)
        case let .pan(dx, dy):
            camera.x += dx
            camera.y += dy
        case let .zoom(factor, at):
            camera.zoom(by: factor, at: at)
        case let .key(name, flags):
            store.send("ui.canvas.key", ["key": name, "cmd": flags.contains(.command), "shift": flags.contains(.shift), "alt": flags.contains(.option)])
        }
    }

    /// The pictures not here yet, each asked for once per revision (the data URL comes back as the reply).
    private func fetchImages(_ entries: [CanvasEntry]) async {
        for entry in entries where entry.drawing.image && cache.images[entry.id]?.rev != entry.drawing.rev {
            let reply = await store.act("ui.canvas.image", ["id": entry.id])
            guard reply.ok, let comma = reply.text.firstIndex(of: ","),
                  let data = Data(base64Encoded: String(reply.text[reply.text.index(after: comma)...]), options: .ignoreUnknownCharacters),
                  let image = NSImage(data: data) else { continue }
            cache.images[entry.id] = (entry.drawing.rev, image)
            arrivals += 1
        }
    }
}

/// The text being typed (a text, a label or a note's markdown): every change goes to Rust, Esc or Cmd+Enter ends it.
struct CanvasTextEditor: View {
    let editing: CanvasEditing
    let camera: CanvasViewport
    let hand: String?
    @State private var value = ""
    @FocusState private var focused: Bool

    var body: some View {
        let rect = camera.screen(canvasRect(editing.rect))
        let size = editing.size * camera.zoom
        let font: Font = editing.note ? .system(size: size, design: .monospaced) : .custom(hand ?? "Excalifont", size: size)
        // a text grows sideways as it is typed (egui's desired_width: its width plus two letters)
        let width = editing.note ? rect.width : rect.width + size * 2
        TextEditor(text: $value)
            .font(font)
            .foregroundStyle(canvasColor(editing.color))
            .multilineTextAlignment(editing.center ? .center : .leading)
            .scrollContentBackground(.hidden)
            .scrollIndicators(.never)
            .focused($focused)
            .frame(width: max(width, 40), height: max(rect.height, size * 1.25))
            .offset(x: rect.minX, y: rect.minY)
            .onAppear {
                value = editing.value
                focused = true
            }
            .onChange(of: value) { MagoStore.shared.send("ui.canvas.text", ["value": value]) }
            .onKeyPress(.escape) {
                MagoStore.shared.send("ui.canvas.text_done")
                return .handled
            }
            .onKeyPress(.return, phases: .down) { press in
                guard press.modifiers.contains(.command) else { return .ignored }
                MagoStore.shared.send("ui.canvas.text_done")
                return .handled
            }
    }
}
