import AppKit
import SwiftUI

// What egui draws over the canvas besides the scene (native/pieces.rs): the drawings' link marks, "Voltar", the
// minimap, and the enlarged window's grip, docked edge and the zone it would dock in.

/// All of them, over the cards, in the canvas's points (coordinate space "canvas").
struct CanvasPieces: View {
    let overlay: CanvasOverlay
    let camera: CanvasViewport
    let view: CGSize

    var body: some View {
        let seen = CGRect(origin: .zero, size: view).insetBy(dx: -20, dy: -20)
        ForEach(overlay.links, id: \.id) { mark in
            let bounds = canvasRect(mark.bounds)
            // at the drawing's top right corner, out of it (LINK_MARK 20: half of it and 2 out)
            let corner = camera.screen(bounds.maxX, bounds.minY)
            let center = CGPoint(x: corner.x + 12, y: corner.y - 12)
            if seen.contains(center) && max(bounds.width, bounds.height) * camera.zoom > 12 {
                LinkMarkView(mark: mark, chain: overlay.chain).offset(x: center.x - 10, y: center.y - 10)
            }
        }
        if let lift = overlay.lift { LiftHandles(lift: lift) }
        if let map = overlay.minimap {
            MinimapView(map: map, dark: overlay.dark)
                .offset(x: view.width - map.right - map.size[0], y: view.height - map.bottom - map.size[1])
        }
        if let back = overlay.back {
            // at the foot of the canvas, in the middle, its top 45 from the bottom
            BackPill(words: back).frame(width: view.width, height: max(view.height - 11, 0), alignment: .bottom)
        }
    }
}

/// The chain (filetree/link.rs `chain`): two stadiums along the diagonal.
func chainPath(center: CGPoint, size: CGFloat) -> Path {
    let along = CGPoint(x: 1 / 2.0.squareRoot(), y: -1 / 2.0.squareRoot())
    let across = CGPoint(x: -along.y, y: along.x)
    let (radius, half) = (size * 0.17, size * 0.2)
    var path = Path()
    for side in [-1.0, 1.0] {
        let middle = CGPoint(x: center.x + along.x * side * size * 0.17, y: center.y + along.y * side * size * 0.17)
        var points: [CGPoint] = []
        for (end, start) in [(half, -CGFloat.pi / 2), (-half, CGFloat.pi / 2)] {
            for step in 0...8 {
                let angle = start + .pi * CGFloat(step) / 8
                let (cosine, sine) = (cos(angle) * radius, sin(angle) * radius)
                points.append(CGPoint(x: middle.x + along.x * (end + cosine) + across.x * sine, y: middle.y + along.y * (end + cosine) + across.y * sine))
            }
        }
        path.addLines(points)
        path.closeSubpath()
    }
    return path
}

/// A drawing's link mark: its file in a tree beside it (an address on the web in the browser).
struct LinkMarkView: View {
    let mark: CanvasLinkMark
    let chain: CanvasChain
    @State private var hovering = false

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 6)
        let ink = canvasColor(chain.ink)
        Button {
            MagoStore.shared.send("ui.canvas.link", ["id": mark.id])
        } label: {
            Canvas { context, size in
                context.stroke(chainPath(center: CGPoint(x: size.width / 2, y: size.height / 2), size: size.width * 0.6), with: .color(ink), lineWidth: 1.4)
            }
            .frame(width: 20, height: 20)
            .background(shape.fill(canvasColor(hovering ? chain.hover : chain.fill)))
            .overlay(shape.strokeBorder(ink.opacity(0.6), lineWidth: 1))
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .onHover { hovering = linkHover($0) }
        .help("Abrir \(mark.link)")
    }
}

/// "Voltar" at the foot of the canvas while a link's tree is on screen (filetree/link.rs `link_back_ui`); × goes without
/// going back.
struct BackPill: View {
    let words: String
    @State private var hovering = false
    @State private var hoveringClose = false

    var body: some View {
        HStack(spacing: 0) {
            Button {
                MagoStore.shared.send("file_link", ["action": "back"])
            } label: {
                HStack(spacing: 0) {
                    Text("←").frame(width: 18, alignment: .leading)
                    Text(words)
                }
                .font(.app(13.5))
                .padding(.leading, 14)
                .padding(.trailing, 3)
                .frame(maxHeight: .infinity)
                .background(Capsule().fill(Color.primary.opacity(hovering ? 0.08 : 0)).padding(3))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { hovering = linkHover($0) }
            Button {
                MagoStore.shared.send("ui.canvas.link_dismiss")
            } label: {
                Text("×").font(.app(16)).foregroundStyle(.primary.opacity(0.7))
                    .frame(width: 24, height: 24)
                    .background(Circle().fill(Color.primary.opacity(hoveringClose ? 0.1 : 0)).frame(width: 22, height: 22))
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .onHover { hoveringClose = $0 }
            .help("Fechar sem voltar")
            .padding(.trailing, 5)
        }
        .frame(height: 34)
        .glass(radius: 17)
    }
}

/// The minimap (ui/menus.rs `minimap`): the drawings as dots, the view as a box; pressed or dragged, the view goes
/// there (Rust holds the map still meanwhile); the X puts it away.
struct MinimapView: View {
    let map: CanvasMinimap
    let dark: Bool
    @State private var hoveringClose = false

    var body: some View {
        let (width, height) = (map.size[0], map.size[1])
        let accent = canvasColor(map.accent)
        ZStack(alignment: .topLeading) {
            Canvas { context, _ in
                for node in map.nodes {
                    let rect = canvasRect(node.rect)
                    if let fill = node.fill { context.fill(Path(roundedRect: rect, cornerRadius: 1), with: .color(canvasColor(fill))) }
                    if let stroke = node.stroke {
                        context.stroke(Path(roundedRect: rect.insetBy(dx: 0.5, dy: 0.5), cornerRadius: 1), with: .color(canvasColor(stroke)), lineWidth: 1)
                    }
                }
                let view = canvasRect(map.view)
                context.fill(Path(roundedRect: view, cornerRadius: 3), with: .color(accent.opacity(0.08)))
                context.stroke(Path(roundedRect: view.insetBy(dx: 0.75, dy: 0.75), cornerRadius: 3), with: .color(accent), lineWidth: 1.5)
            }
            .frame(width: width - 2 * map.inset, height: height - 2 * map.inset)
            .clipped()
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { press("down", $0.location) }
                    .onEnded { press("up", $0.location) }
            )
            .offset(x: map.inset, y: map.inset)
            // the X: 16 points, 8 from the corner
            Button {
                MagoStore.shared.send("minimap", ["open": false])
            } label: {
                Canvas { context, size in
                    let center = CGPoint(x: size.width / 2, y: size.height / 2)
                    context.fill(Path(ellipseIn: CGRect(origin: .zero, size: size)), with: .color(dark ? Color(white: 0x3a / 255) : Color(white: 0xf6 / 255)))
                    var cross = Path()
                    for sign in [-1.0, 1.0] {
                        cross.move(to: CGPoint(x: center.x - 3, y: center.y - 3 * sign))
                        cross.addLine(to: CGPoint(x: center.x + 3, y: center.y + 3 * sign))
                    }
                    context.stroke(cross, with: .color(Color.primary.opacity(hoveringClose ? 1 : 0.6)), lineWidth: 1.2)
                }
                .frame(width: 16, height: 16)
                .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .onHover { hoveringClose = linkHover($0) }
            .help("Fechar minimapa")
            .offset(x: width - 24, y: 8)
        }
        .frame(width: width, height: height, alignment: .topLeading)
        .glass(radius: 13)
    }

    private func press(_ phase: String, _ at: CGPoint) {
        MagoStore.shared.send("ui.canvas.minimap", ["phase": phase, "x": Double(at.x), "y": Double(at.y)])
    }
}

/// The enlarged window's handles (canvas/dock.rs `dock_grip`): its title dragged docks it at a side (the zone shows
/// where), a double click puts it back; docked, its inner edge sets its width.
struct LiftHandles: View {
    let lift: CanvasLift
    @State private var dragging = false

    var body: some View {
        if let zone = lift.zone.map(canvasRect) {
            let shape = RoundedRectangle(cornerRadius: 12)
            shape.fill(canvasPrimary.opacity(0.12))
                .overlay(shape.strokeBorder(canvasPrimary, lineWidth: 1.5))
                .frame(width: zone.width, height: zone.height)
                .offset(x: zone.minX, y: zone.minY)
                .allowsHitTesting(false)
        }
        if let grip = lift.grip.map(canvasRect) {
            Color.clear
                .contentShape(Rectangle())
                .frame(width: grip.width, height: grip.height)
                .onHover { inside in if inside { NSCursor.openHand.push() } else { NSCursor.pop() } }
                .gesture(
                    DragGesture(minimumDistance: 3, coordinateSpace: .named("canvas"))
                        .onChanged { value in
                            if !dragging {
                                dragging = true
                                dock("start", value.startLocation)
                            }
                            dock("move", value.location)
                        }
                        .onEnded { value in
                            dock("end", value.location)
                            dragging = false
                        }
                )
                .onTapGesture(count: 2) { dock("restore", .zero) }
                .offset(x: grip.minX, y: grip.minY)
        }
        if let edge = lift.edge.map(canvasRect) {
            Color.clear
                .contentShape(Rectangle())
                .frame(width: edge.width, height: edge.height)
                .onHover { inside in if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() } }
                .gesture(DragGesture(minimumDistance: 0, coordinateSpace: .named("canvas")).onChanged { dock("edge", $0.location) })
                .offset(x: edge.minX, y: edge.minY)
        }
    }

    private func dock(_ phase: String, _ at: CGPoint) {
        MagoStore.shared.send("ui.canvas.dock", ["phase": phase, "x": Double(at.x), "y": Double(at.y)])
    }
}
