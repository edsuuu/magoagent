import AppKit
import SwiftUI

/// The canvas on macOS: Rust's scene drawn as it publishes it (src/app/canvas/native/), the pan and zoom done here,
/// the pointer and the keys sent back as `ui.canvas.*`. The drawings in one picture, each card (window or note) in a
/// layer of its own over them in the drawing's order, the enlarged one last over the blur, then what goes over all.
struct CanvasView: View {
    private let store = MagoStore.shared
    @State private var cache = CanvasCache()
    @State private var camera = CanvasViewport()
    @State private var serial: UInt64 = 0
    @State private var size = CGSize.zero
    /// Bumped when a picture or the hand font arrives, so the canvas draws again.
    @State private var arrivals = 0
    /// How far down each note's text is scrolled by the wheel, in its points (notes/scroll.rs): the session's, by id.
    @State private var noteScroll: [String: CGFloat] = [:]

    var body: some View {
        let list = store.decode(CanvasSceneList.self, topic: "scene")
        let overlay = store.decode(CanvasOverlay.self, topic: "canvas")
        let entries = list.map { cache.sync($0, store: store) } ?? []
        let hand = CanvasFonts.hand
        let images = cache.images.mapValues(\.image)
        let camera = camera
        let dark = overlay?.dark ?? false
        let handTool = overlay?.tool == "hand"
        let ground = canvasColor(list?.ground ?? [255, 255, 255, 255])
        let lift = overlay?.lift
        // the enlarged window is the list's last (sync_scene)
        let lifted = lift.flatMap { lift in entries.last.flatMap { $0.id == lift.id ? $0 : nil } }
        let cards = entries.filter { $0.card && $0 !== lifted }
        let liftedTree = lifted?.drawing.node?.kind == "filetree" ? lift.map { canvasRect($0.frame) } : nil
        let _ = arrivals
        GeometryReader { geometry in
            let view = geometry.size
            ZStack(alignment: .topLeading) {
                Canvas { context, size in
                    context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(ground))
                    if let grid = list?.grid { paintGrid(&context, size: size, camera: camera, color: canvasColor(grid)) }
                    let seen = camera.world(size)
                    var world = context
                    world.translateBy(x: camera.x, y: camera.y)
                    world.scaleBy(x: camera.zoom, y: camera.zoom)
                    for entry in entries where !entry.card && canvasRect(entry.drawing.reach).intersects(seen) {
                        paintEntry(&world, entry, hand: hand, image: images[entry.id], dark: dark)
                    }
                }
                CanvasInput(handTool: handTool, base: true) { handle($0, overlay: overlay) }
                ForEach(cards, id: \.id) { entry in
                    card(entry, place: camera.screen(canvasRect(entry.drawing.rect)), scale: camera.zoom, view: view, overlay: overlay)
                }
                if let lift {
                    if let skeleton = lift.skeleton {
                        Canvas { context, _ in paintSkeleton(&context, skeleton, camera: camera, dark: dark) }.allowsHitTesting(false)
                    }
                    if lift.blur > 0 {
                        // the canvas behind it blurred and tinted with the ground (lifted.rs), live windows too; a press
                        // there is the canvas's
                        Rectangle().fill(.regularMaterial).opacity(lift.blur).allowsHitTesting(false)
                        Rectangle().fill(ground.opacity(0.35 * lift.blur)).allowsHitTesting(false)
                        CanvasInput(handTool: handTool) { handle($0, overlay: overlay) }
                    }
                    if let lifted {
                        card(lifted, place: canvasRect(lift.frame), scale: lift.zoom, view: view, overlay: overlay)
                    }
                }
                MagoViews.pages?()
                if let overlay {
                    Canvas { context, _ in paintOverlay(&context, overlay, camera: camera, hand: hand) }
                        .allowsHitTesting(false)
                    CanvasPieces(overlay: overlay, camera: camera, view: view)
                    if let editing = overlay.editing {
                        if let note = editing.note {
                            // a lifted note is typed where it shows
                            let viewport = if let lift, let lifted, lift.id == note { liftViewport(lift, lifted) } else { camera }
                            let rect = viewport.screen(canvasRect(editing.rect))
                            let press = CanvasInputView.pressed.map { CGPoint(x: $0.x - rect.minX, y: $0.y - rect.minY) }
                            CanvasNoteEditor(editing: editing, zoom: viewport.zoom, scroll: noteScroll[note] ?? 0, press: press)
                                .frame(width: rect.width, height: rect.height)
                                .offset(x: rect.minX, y: rect.minY)
                                .id(editing.id)
                        } else {
                            CanvasTextEditor(editing: editing, camera: camera, hand: hand).id(editing.id)
                        }
                    }
                }
            }
            .coordinateSpace(name: "canvas")
            .clipped()
            .onAppear { resize(geometry.size) }
            .onChange(of: geometry.size) { resize(geometry.size) }
        }
        .onChange(of: overlay?.camera.serial, initial: true) { adopt(overlay?.camera) }
        .onChange(of: self.camera) { sendView() }
        .onChange(of: liftedTree, initial: true) { CanvasInputView.liftedTree = liftedTree }
        .task { await CanvasFonts.register(store: store); arrivals += 1 }
        .task(id: list?.version) { await fetchImages(entries) }
    }

    /// Where the enlarged card's own points show: its box's origin at the frame's, at its scale.
    private func liftViewport(_ lift: CanvasLift, _ entry: CanvasEntry) -> CanvasViewport {
        let (frame, rect) = (canvasRect(lift.frame), canvasRect(entry.drawing.rect))
        return CanvasViewport(x: frame.minX - rect.minX * lift.zoom, y: frame.minY - rect.minY * lift.zoom, zoom: lift.zoom)
    }

    private func card(_ entry: CanvasEntry, place: CGRect, scale: CGFloat, view: CGSize, overlay: CanvasOverlay?) -> some View {
        CanvasCard(entry: entry, place: place, scale: scale, view: view, hand: CanvasFonts.hand, dark: overlay?.dark ?? false,
                   handTool: overlay?.tool == "hand", scroll: noteScroll[entry.id] ?? 0, onEvent: { handle($0, overlay: overlay) }) { delta in
            guard let layout = entry.noteLayout, let note = entry.drawing.note else { return false }
            let overflow = max(layout.height - canvasRect(note.area).height, 0)
            noteScroll[entry.id] = min(max((noteScroll[entry.id] ?? 0) - delta, 0), overflow)
            return true
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

    private func handle(_ event: CanvasInputEvent, overlay: CanvasOverlay?) {
        switch event {
        case let .pointer(phase, at, flags, clicks, middle):
            // the minimap is a popover: a press anywhere else on the canvas puts it away
            if phase == "down" && overlay?.minimap != nil { store.send("minimap", ["open": false]) }
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
            if name == "Escape" && overlay?.minimap != nil {
                store.send("minimap", ["open": false])
                return
            }
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

/// A text or a label being typed: every change goes to Rust, Esc or Cmd+Enter ends it (a note's is CanvasNoteEditor).
struct CanvasTextEditor: View {
    let editing: CanvasEditing
    let camera: CanvasViewport
    let hand: String?
    @State private var value = ""
    @FocusState private var focused: Bool

    var body: some View {
        let rect = camera.screen(canvasRect(editing.rect))
        let size = editing.size * camera.zoom
        // a text grows sideways as it is typed (egui's desired_width: its width plus two letters)
        let width = rect.width + size * 2
        TextEditor(text: $value)
            .font(.custom(hand ?? "Excalifont", size: size))
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
