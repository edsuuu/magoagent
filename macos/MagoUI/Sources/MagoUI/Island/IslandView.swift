// The island drawn: its black shape hung from the screen's edge (square on that edge, the ears around a notch), the
// closed notch with Luna's face, the compact island with Luna and a mini per agent, and the open panel with the header
// and the page (`paint.rs`). Each content comes and goes on its own fade, as `Layers` in mod.rs.

import AppKit
import SwiftUI

enum IslandInk {
    static let ink = rgb(0xf5f6f8), dim = rgb(0x8e939c), dim3 = rgb(0x6b7079), rowInk = rgb(0xc5c8cd)
    static let card = rgb(0x141518), cardFlat = rgb(0x0e0f11), tabOn = rgb(0x1d1f23), hover = rgb(0x1b1d21)
    static let amber = rgb(0xf5a524), red = rgb(0xf4505e), cyan = rgb(0x22d3ee), green = rgb(0x22c55e), mint = rgb(0x34d399)
    static let blue = rgb(0x3b9eff), picked = rgb(0x818cf8), black = rgb(0x0b0c0e)
    /// Início's red (`feed::RED`), the ask's links light enough on its wash (`ask::LINK`), Spotify's green.
    static let feedRed = rgb(0xf85149), link = rgb(0xd4d7dd), spotify = "#1ed760"
    static let hairline = Color.white.opacity(9 / 255)
    static let colors = ["#000000", "#222327", "#182a4e", "#1f3a2c", "#3a1f3f"]

    static func rgb(_ value: UInt32) -> Color {
        Color(red: Double(value >> 16 & 0xff) / 255, green: Double(value >> 8 & 0xff) / 255, blue: Double(value & 0xff) / 255)
    }

    /// "#rrggbb" (what Rust sends); black for anything else.
    static func hex(_ text: String) -> Color {
        UInt32(text.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16).map(rgb) ?? .black
    }

    /// From `from` to `to`, `along` of the way, channel by channel as stored (`lerp_to_gamma`).
    static func mix(_ from: String, _ to: String, _ along: Double) -> Color {
        let value = { (text: String) in UInt32(text.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) ?? 0 }
        let (a, b) = (value(from), value(to))
        let channel = { (shift: UInt32) in (Double(a >> shift & 0xff) * (1 - along) + Double(b >> shift & 0xff) * along) / 255 }
        return Color(red: channel(16), green: channel(8), blue: channel(0))
    }

    /// The bars' two colours: the cover's, else its tint, else Spotify's green (`pill::spotify`).
    static func bars(_ song: IslandSong?) -> (String, String) {
        if let colours = song?.colours, colours.count == 2 { return (colours[0], colours[1]) }
        let tint = song?.tint ?? spotify
        return (tint, tint)
    }

    /// A cover still to come: a diagonal of two colours picked by its name (`music::placeholder`).
    static func placeholder(_ name: String) -> (Color, Color) {
        let pairs: [(UInt32, UInt32)] = [(0x7c3aed, 0xdb2777), (0x0ea5e9, 0x22c55e), (0xf59e0b, 0xef4444), (0x334155, 0x64748b), (0x16a34a, 0x4ade80)]
        let sum = name.utf8.reduce(UInt(0)) { $0 &* 31 &+ UInt($1) }
        let pair = pairs[Int(sum % UInt(pairs.count))]
        return (rgb(pair.0), rgb(pair.1))
    }

    /// `color` moved `amount` of the way to white (`lighten`).
    static func lighten(_ text: String, _ amount: Double) -> Color {
        guard let value = UInt32(text.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) else { return ink }
        let toward = { (channel: UInt32) in (Double(channel) + (255 - Double(channel)) * amount) / 255 }
        return Color(red: toward(value >> 16 & 0xff), green: toward(value >> 8 & 0xff), blue: toward(value & 0xff))
    }
}

/// The island's outline: round, square on the edge of the screen it hangs from (`corners`).
struct IslandOutline: Shape {
    var corner: CGFloat
    var dock: IslandDock

    var animatableData: CGFloat {
        get { corner }
        set { corner = newValue }
    }

    func path(in rect: CGRect) -> Path {
        let round = { (square: Bool) in square ? 0 : min(corner, rect.height / 2, rect.width / 2) }
        return UnevenRoundedRectangle(
            topLeadingRadius: round(dock == .top || dock == .left), bottomLeadingRadius: round(dock == .bottom || dock == .left),
            bottomTrailingRadius: round(dock == .bottom || dock == .right), topTrailingRadius: round(dock == .top || dock == .right)
        ).path(in: rect)
    }
}

/// Its top corners turned out around a notch (`ears`): from its top edge it curves out into the bar beside it, as the
/// notch it grows out of does; each ear goes a point into the island so no seam shows.
struct IslandEars: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        let ear = IslandSize.ear
        for side in [-1.0, 1.0] {
            let edge = side < 0 ? rect.minX : rect.maxX
            let centre = CGPoint(x: edge + side * ear, y: rect.minY + ear)
            let arc = (0...12).map { step in
                let turn = Double.pi / 2 * Double(step) / 12
                return CGPoint(x: centre.x - side * ear * cos(turn), y: centre.y - ear * sin(turn))
            }
            path.addLines([CGPoint(x: edge - side, y: rect.minY), CGPoint(x: edge - side, y: rect.minY + ear)] + arc)
            path.closeSubpath()
        }
        return path
    }
}

struct IslandView: View {
    let model: IslandModel

    var body: some View {
        let fill = IslandInk.hex(model.snapshot.color)
        let outline = IslandOutline(corner: model.corner, dock: model.dock)
        ZStack(alignment: .topLeading) {
            ZStack(alignment: .topLeading) {
                if model.notch != nil { IslandEars().fill(fill) }
                outline.fill(fill)
                IslandLayers(model: model).clipShape(outline)
            }
            .frame(width: model.rect.width, height: model.rect.height)
            .contentShape(outline)
            .onTapGesture { model.click() }
            .gesture(DragGesture(minimumDistance: 4).onChanged { _ in model.dragged() }.onEnded { _ in model.dropped() })
            .onHover { inside in if inside, model.shape != .open { NSCursor.pointingHand.set() } }
            .offset(x: model.rect.minX, y: model.rect.minY)
        }
        .frame(width: model.room.width, height: model.room.height, alignment: .topLeading)
        .environment(\.colorScheme, .dark)
    }
}

/// The closed, compact and open contents, each on its own fade, and the countdown bar.
struct IslandLayers: View {
    let model: IslandModel

    var body: some View {
        ZStack(alignment: .topLeading) {
            if model.shape == .closed {
                IslandSmall(model: model, compact: false)
                    .transition(.asymmetric(insertion: .opacity.animation(.easeOut(duration: 0.3).delay(0.12)), removal: .opacity.animation(.easeIn(duration: 0.18))))
            }
            if model.shape == .compact {
                IslandSmall(model: model, compact: true).transition(.opacity.animation(.easeInOut(duration: 0.25)))
            }
            if model.shape == .open {
                IslandOpen(model: model).transition(.opacity.animation(.easeInOut(duration: 0.2)))
            }
            // the reference's countdown: a thin bar at the bottom, 160 wide when it starts, gone when the island folds
            if model.shape == .open, let left = model.countdown, left > 0 {
                Capsule().fill(Color.white.opacity(89 / 255))
                    .frame(width: 160 * left, height: 2)
                    .position(x: model.rect.width / 2, y: model.rect.height - 1)
                    .allowsHitTesting(false)
            }
        }
        .frame(width: model.rect.width, height: model.rect.height, alignment: .topLeading)
    }
}

// MARK: Luna and the minis on a canvas

/// What a canvas of the island draws, in its own coordinates.
enum IslandSprite {
    /// The closed and compact Luna has her own motion, so the cross-fade with the page's never flips her state each frame.
    case luna(center: CGPoint, size: CGFloat, state: LunaState, motion: String = "island-main")
    case mini(line: IslandLine, center: CGPoint, body: CGFloat, mark: Bool)
    /// The song's cover is a view; these are the sound's bars from `x` (`count` of them, `bar` wide, `gap` apart).
    case bars(x: CGFloat, y: CGFloat, count: Int, bar: CGFloat, gap: CGFloat, tall: CGFloat, upright: Bool, colours: (String, String))
    /// What a session outside the app is at, on its mini's shoulder (`shoulder_mark`).
    case shoulder(line: IslandLine, center: CGPoint, body: CGFloat)
    case voice(center: CGPoint)
    /// The check Luna wears while an agent finished and the island was not opened yet (`done::paint_check`).
    case check(luna: CGPoint, size: CGFloat)
}

/// One canvas redrawn every display frame (120 Hz on ProMotion) with Luna, the minis and the bars.
struct IslandCanvas: View {
    let model: IslandModel
    let sprites: [IslandSprite]

    var body: some View {
        GeometryReader { proxy in
            let frame = proxy.frame(in: .global)
            TimelineView(.animation) { timeline in
                Canvas { context, _ in
                    let time = timeline.date.timeIntervalSinceReferenceDate
                    let cursor = IslandScreens.cursor
                    let pointer = CGPoint(x: cursor.x - model.origin.x - frame.minX, y: cursor.y - model.origin.y - frame.minY)
                    for sprite in sprites { paint(sprite, &context, time: time, pointer: pointer) }
                }
            }
        }
        .allowsHitTesting(false)
    }

    private func paint(_ sprite: IslandSprite, _ context: inout GraphicsContext, time: Double, pointer: CGPoint) {
        let live = model.live
        switch sprite {
        case let .luna(center, size, state, id):
            let motion = Luna.motion(id)
            motion.advance(state, time, gaze: lunaGaze(center, pointer))
            let calm = [.idle, .sleeping, .working, .done].contains(state)
            let dances = model.snapshot.lunaDances && model.snapshot.song?.playing == true && calm && !live.dance.isEmpty && live.dance != "nap"
                && !motion.dizzy(time)
            Luna.paint(&context, center: center, size: size, motion: motion, time: time, color: IslandInk.hex(model.snapshot.lunaColor),
                       dance: dances ? LunaDance.at(dance: live.dance, beat: live.beat) : nil)
        case let .mini(line, center, body, mark):
            let motion = Luna.motion("island-mini-\(line.id)")
            motion.advance(LunaState(rawValue: line.mago) ?? .idle, time, gaze: motion.glance(time))
            Luna.paintMini(&context, center: center, body: body, color: IslandInk.hex(line.color), motion: motion, time: time)
            if mark { miniBadge(&context, line, center: center, body: body, time: time) }
        case let .bars(x, y, count, bar, gap, tall, upright, colours):
            let bands = model.snapshot.song?.playing == true ? (live.bands ?? []) : []
            for index in 0..<count {
                // the closed notch is small: its bars are the bands two by two
                let level = count < IslandPill.bars
                    ? max(band(bands, index * 2), band(bands, index * 2 + 1)) : band(bands, index)
                let height = bar + (tall - bar) * min(max(pow(level, 1.6), 0), 1)
                let along = x + CGFloat(index) * (bar + gap)
                let rect = upright ? CGRect(x: y - height / 2, y: along, width: height, height: bar) : CGRect(x: along, y: y - height / 2, width: bar, height: height)
                // the first bar the first colour, the last the second, the ones between a bit of each
                let share = count > 1 ? Double(index) / Double(count - 1) : 0
                context.fill(Path(roundedRect: rect, cornerRadius: bar / 2), with: .color(IslandInk.mix(colours.0, colours.1, share)))
            }
        case let .shoulder(line, center, body):
            shoulder(&context, line, center: center, body: body, time: time)
        case let .check(luna, size):
            let radius = max(size * 0.2, 4)
            let at = CGPoint(x: luna.x + size * 0.34, y: luna.y - size * 0.3)
            context.fill(Path(ellipseIn: CGRect(x: at.x - radius - 1.5, y: at.y - radius - 1.5, width: (radius + 1.5) * 2, height: (radius + 1.5) * 2)), with: .color(IslandInk.black))
            context.fill(Path(ellipseIn: CGRect(x: at.x - radius, y: at.y - radius, width: radius * 2, height: radius * 2)), with: .color(IslandInk.mint))
            // the chat lists' check (24 across), as wide as the seal
            let scale = radius * 1.5 / 24
            var mark = Path()
            for (index, point) in [(5.0, 12.5), (9.5, 17.0), (19.0, 7.5)].enumerated() {
                let to = CGPoint(x: at.x + (point.0 - 12) * scale, y: at.y + (point.1 - 12) * scale)
                if index == 0 { mark.move(to: to) } else { mark.addLine(to: to) }
            }
            context.stroke(mark, with: .color(IslandInk.black), style: StrokeStyle(lineWidth: max(radius * 0.3, 1.2), lineCap: .round, lineJoin: .round))
        case let .voice(center):
            let level = live.mic
            for index in 0..<3 {
                let height = 4 + 12 * min(level * [0.7, 1, 0.8][index] * 2 + 0.1 * sin(time * 9 + Double(index)), 1)
                let rect = CGRect(x: center.x - 6 + CGFloat(index) * 5, y: center.y - height / 2, width: 2.5, height: height)
                context.fill(Path(roundedRect: rect, cornerRadius: 1.25), with: .color(IslandInk.ink))
            }
        }
    }

    private func band(_ bands: [Double], _ index: Int) -> Double { index < bands.count ? bands[index] : 0 }

    /// `!` it waits on the person, `✓` its turn ended, `✕` it failed, in a ring of the card; stopped, a `z` drifting up.
    private func shoulder(_ context: inout GraphicsContext, _ line: IslandLine, center: CGPoint, body: CGFloat, time: Double) {
        let at = CGPoint(x: center.x + body * 0.42, y: center.y - body * 0.4)
        let radius = max(body * 0.23, 4.5)
        let mark: (Color, String, Color)? = switch (line.badge, line.state) {
        case ("approval", _), (_, "waiting"): (IslandInk.amber, "!", IslandInk.rgb(0x2b1a00))
        case ("finished", _): (IslandInk.mint, "✓", IslandInk.rgb(0x062b1d))
        case ("error", _): (IslandInk.red, "✕", IslandInk.rgb(0x2b0006))
        default: nil
        }
        let circle = { (r: CGFloat) in Path(ellipseIn: CGRect(x: at.x - r, y: at.y - r, width: 2 * r, height: 2 * r)) }
        if let mark {
            context.fill(circle(radius + 2), with: .color(IslandInk.cardFlat))
            context.fill(circle(radius), with: .color(mark.0))
            context.draw(Text(mark.1).font(.system(size: radius * 1.55, weight: .semibold)).foregroundStyle(mark.2), at: CGPoint(x: at.x, y: at.y + 0.5))
        } else if line.asleep {
            for (delay, size) in [(0.0, radius * 1.6), (0.5, radius * 1.2)] {
                let phase = (time * 0.45 + delay).truncatingRemainder(dividingBy: 1)
                let fade = (1 - phase) * min(phase * 4, 1)
                let drift = CGPoint(x: at.x + phase * 3, y: at.y - phase * 7 - 1)
                context.draw(Text("z").font(.system(size: size, weight: .semibold)).foregroundStyle(IslandInk.dim.opacity(fade * 230 / 255)), at: drift)
            }
        }
    }

    /// The state's badge on a mini (`mini_badge`): on its upper left, a black ring and a disc of the state's colour;
    /// working, the disc pulses.
    private func miniBadge(_ context: inout GraphicsContext, _ line: IslandLine, center: CGPoint, body: CGFloat, time: Double) {
        let radius = body / 2
        let pulse = 1 + 0.25 * sin((time * 2.4).truncatingRemainder(dividingBy: 1) * 2 * .pi)
        let (ring, disc, color): (Double, Double, Color)
        switch (line.badge, line.state, line.mago) {
        case ("error", _, _), (_, _, "error"): (ring, disc, color) = (0.2, 0.135, IslandInk.red)
        case ("approval", _, _), (_, "waiting", _), (_, _, "needs_you"): (ring, disc, color) = (0.3, 0.23, IslandInk.amber)
        case (_, _, "asking"): (ring, disc, color) = (0.3, 0.23, IslandInk.cyan)
        case ("finished", _, _), (_, _, "done"): (ring, disc, color) = (0.2, 0.135, IslandInk.mint)
        case (_, _, "working"), (_, _, "listening"), (_, _, "speaking"): (ring, disc, color) = (0.2, 0.22 * pulse, IslandInk.blue)
        default: return
        }
        let at = CGPoint(x: center.x - radius * 0.72, y: center.y - radius * 0.72)
        let circle = { (r: Double) in Path(ellipseIn: CGRect(x: at.x - r, y: at.y - r, width: 2 * r, height: 2 * r)) }
        context.fill(circle(radius * ring * 1.25), with: .color(.black))
        context.fill(circle(radius * disc * 1.25), with: .color(color))
    }
}

/// A song's cover from its URL (`AsyncImage`), its placeholder while it comes.
struct IslandCover: View {
    let song: IslandSong
    let side: CGFloat
    let corner: CGFloat

    var body: some View {
        AsyncImage(url: URL(string: song.coverUrl)) { image in
            image.resizable().aspectRatio(contentMode: .fill)
        } placeholder: {
            let (from, to) = IslandInk.placeholder(song.title)
            LinearGradient(colors: [from, to], startPoint: .topLeading, endPoint: .bottomTrailing)
        }
        .frame(width: side, height: side)
        .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
    }
}

// MARK: closed and compact

/// The closed notch (Luna's face; the song's cover and bars around her, one under the other standing up on a side) or
/// the compact island (Luna, the song, a mini per agent in a 2×2 grid at the right, the call's bars) (`island_small`).
struct IslandSmall: View {
    let model: IslandModel
    let compact: Bool

    var body: some View {
        let snapshot = model.snapshot
        let size = model.rect.size
        let rect = CGRect(origin: .zero, size: size)
        let upright = model.dock.upright
        let notch = model.notch?.width ?? 0
        let state = LunaState(rawValue: snapshot.state) ?? .idle
        let song = snapshot.song
        let colours = IslandInk.bars(song)
        var sprites: [IslandSprite] = []
        var cover: CGRect?
        var cells: [(IslandLine, CGRect)] = []
        if !compact {
            let center = model.notch.map { _ in CGPoint(x: song != nil ? 33 : (size.width - notch) / 4, y: size.height / 2) }
                ?? CGPoint(x: size.width / 2, y: size.height / 2)
            sprites.append(.luna(center: center, size: 18, state: state, motion: "island-small"))
            if snapshot.finished { sprites.append(.check(luna: center, size: 18)) }
            if song != nil {
                // the cover 4 from the left end and the bars' middle 12 from the right; standing up, from the top and the
                // bottom, the bars lying down (`pill::spotify`)
                let total = 5 * 1.5 + 4 * 1.5
                let (along, across) = upright ? (size.height, size.width) : (size.width, size.height)
                cover = upright ? CGRect(x: across / 2 - 8, y: 4, width: 16, height: 16) : CGRect(x: 4, y: across / 2 - 8, width: 16, height: 16)
                sprites.append(.bars(x: along - 12 - total / 2, y: across / 2, count: 5, bar: 1.5, gap: 1.5, tall: 11, upright: upright, colours: colours))
            }
        } else {
            let wizard = upright ? CGPoint(x: size.width / 2, y: 24) : CGPoint(x: IslandSize.compactSide, y: size.height / 2 + 1)
            let lunaSize = model.notch.map { min(30, $0.height - 6) } ?? 30
            sprites.append(.luna(center: wizard, size: lunaSize, state: state, motion: "island-small"))
            if snapshot.finished { sprites.append(.check(luna: wizard, size: lunaSize)) }
            let lines = snapshot.lines.filter { song == nil || $0.id != "integration:spotify" }
            let places = upright ? islandUprightCells(rect, count: lines.count) : islandMiniCells(rect, count: lines.count)
            cells = Array(zip(lines, places))
            for (line, cell) in cells { sprites.append(.mini(line: line, center: CGPoint(x: cell.midX, y: cell.midY), body: cell.width, mark: true)) }
            if snapshot.voice {
                let x = notch > 0 ? IslandSize.compactSide + 15 + 10 + 10.5 : size.width / 2
                sprites.append(.voice(center: CGPoint(x: x, y: size.height / 2)))
            }
            if song != nil {
                let grid = places.dropFirst().reduce(places.first) { grid, cell in grid?.union(cell) }
                if upright {
                    let pair = IslandPill.cover + IslandPill.pairGap + IslandPill.barsRoom
                    let wizardEnd: CGFloat = 24 + IslandPill.wizard / 2
                    let gridTop: CGFloat = grid?.minY ?? size.height
                    let start: CGFloat = (wizardEnd + gridTop) / 2 - pair / 2
                    cover = CGRect(x: size.width / 2 - 11, y: start, width: 22, height: 22)
                    sprites.append(.bars(x: start + 22 + IslandPill.pairGap, y: size.width / 2, count: IslandPill.bars, bar: 1.5, gap: 3, tall: 16, upright: true, colours: colours))
                } else {
                    let (coverX, barsX) = IslandPill.spread(rect, minis: grid, notch: notch)
                    cover = CGRect(x: coverX, y: size.height / 2 - 11, width: 22, height: 22)
                    sprites.append(.bars(x: barsX, y: size.height / 2, count: IslandPill.bars, bar: 1.5, gap: 3, tall: 16, upright: false, colours: colours))
                }
            }
        }
        return ZStack(alignment: .topLeading) {
            IslandCanvas(model: model, sprites: sprites)
            if let song, let cover {
                // a click on the cover opens the island on the music's tab
                Button { model.open(tab: "music", focus: nil) } label: { IslandCover(song: song, side: cover.width, corner: compact ? 6 : 5) }
                    .buttonStyle(.plain)
                    .help(song.title)
                    .offset(x: cover.minX, y: cover.minY)
            }
            // a click on a mini opens the island on it: its page with it in the card
            ForEach(cells, id: \.0.id) { line, cell in
                Button { model.open(tab: line.tab, focus: line.id) } label: { Color.clear.contentShape(Rectangle()) }
                    .buttonStyle(.plain)
                    .frame(width: cell.width + 2, height: cell.height + 2)
                    .help(line.name)
                    .offset(x: cell.minX - 1, y: cell.minY - 1)
            }
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
    }
}

// MARK: open

/// The open panel: the header on the notch's row (around one) and the page under it (`island_open`).
struct IslandOpen: View {
    let model: IslandModel

    var body: some View {
        let size = model.rect.size
        // around a notch the panel starts above the island so its header's middle is the notch's
        let lift = model.notch.map { $0.height / 2 - IslandSize.headerMid } ?? 0
        let page = CGRect(x: 10, y: lift + IslandSize.pageTop, width: size.width - 20, height: size.height - lift - IslandSize.pageTop - 10)
        ZStack(alignment: .topLeading) {
            IslandHeader(model: model).frame(width: size.width, height: 50).offset(y: lift)
            IslandPage(model: model, size: page.size)
                .frame(width: max(page.width, 0), height: max(page.height, 0))
                .offset(x: page.minX, y: page.minY)
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
    }
}

/// A round button of the header: dim, lit under the pointer and when on.
struct IslandHeaderButton: View {
    let symbol: String
    let tip: String
    var on = false
    var size: CGFloat = 15
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size, weight: .medium))
                .foregroundStyle(on || hovered ? IslandInk.ink : IslandInk.dim)
                .frame(width: 24, height: 24)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(tip)
    }
}

/// The tabs at the left; the settings, the sound and the close at the right, the voice while the call is on
/// (`island_header`).
struct IslandHeader: View {
    let model: IslandModel
    @State private var hoveredTab: String?

    static func symbol(_ tab: String) -> String {
        switch tab {
        case "home": "house"
        case "chat": "bubble.left"
        case "chats": "list.bullet"
        case "claude": "asterisk"
        case "github": "arrow.triangle.pull"
        case "music": "music.note"
        default: "plus"
        }
    }

    var body: some View {
        let snapshot = model.snapshot
        let pinned = snapshot.ask != nil
        let width = model.rect.width
        ZStack(alignment: .topLeading) {
            ForEach(Array(snapshot.tabs.enumerated()), id: \.element.tab) { index, tab in
                let on = snapshot.tab == tab.tab && !pinned && !snapshot.settings
                let hovered = hoveredTab == tab.tab
                Button { model.act("tab", ["tab": tab.tab]) } label: {
                    ZStack {
                        // a round mark under the icon, not a capsule
                        Circle().fill(on ? IslandInk.tabOn : hovered ? Color.white.opacity(18 / 255) : .clear).frame(width: 28, height: 28)
                        Image(systemName: Self.symbol(tab.tab)).font(.system(size: 14, weight: .medium))
                            .foregroundStyle(on ? IslandInk.ink : hovered ? IslandInk.rowInk : IslandInk.dim)
                    }
                    .frame(width: 30, height: 28)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .onHover { hoveredTab = $0 ? tab.tab : (hoveredTab == tab.tab ? nil : hoveredTab) }
                .help(tab.tip)
                .position(x: 14 + 15 + CGFloat(index) * 35, y: IslandSize.headerMid)
            }
            // the call's note between them, stopping before the camera around a notch
            if !snapshot.voiceNote.isEmpty {
                let left = 14 + CGFloat(snapshot.tabs.count) * 35 + 10
                let end = model.notch.map { width / 2 - $0.width / 2 - 8 } ?? width - 150
                Text(snapshot.voiceNote).font(.system(size: 11)).foregroundStyle(IslandInk.dim3).lineLimit(1)
                    .frame(width: max(end - left, 0), alignment: .leading)
                    .position(x: left + max(end - left, 0) / 2, y: IslandSize.headerMid)
            }
            HStack(spacing: 8) {
                // the call on hold (`voice::CALL_ON`): no Chat por voz; one still on can be ended
                if snapshot.callOn || snapshot.voice {
                    IslandHeaderButton(symbol: snapshot.voice ? "waveform" : "mic", tip: snapshot.voice ? "Encerrar o chat por voz" : "Chat por voz", on: snapshot.voice) {
                        model.app("voice_call", ["state": snapshot.voice ? "off" : "on"])
                    }
                }
                IslandHeaderButton(symbol: snapshot.settings ? "gearshape.fill" : "gearshape", tip: "Ajustes da ilha", on: snapshot.settings) { model.act("settings") }
                IslandHeaderButton(symbol: snapshot.sound ? "speaker.wave.2" : "speaker.slash", tip: snapshot.sound ? "Silenciar os sons" : "Ligar os sons") {
                    model.setting("island_sound", snapshot.sound ? "off" : "on")
                }
                if !pinned { IslandHeaderButton(symbol: "xmark", tip: "Recolher") { model.fold() } }
            }
            .frame(height: 24)
            .position(x: width - 13 - widthOfButtons(snapshot, pinned: pinned) / 2, y: IslandSize.headerMid)
        }
        .frame(width: width, height: 50, alignment: .topLeading)
    }

    /// Right to left, 32 apart from 25 in (`island_header`).
    private func widthOfButtons(_ snapshot: IslandSnapshot, pinned: Bool) -> CGFloat {
        let count = 2 + (snapshot.callOn || snapshot.voice ? 1 : 0) + (pinned ? 0 : 1)
        return CGFloat(count) * 24 + CGFloat(count - 1) * 8
    }
}
