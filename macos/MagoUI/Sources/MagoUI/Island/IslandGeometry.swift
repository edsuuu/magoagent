// The island's sizes, corners, docks and clock, ported from `src/app/island/` (mod.rs, window.rs, fsm.rs, minis.rs,
// pill.rs). Screen coordinates here are Rust's: points from the main screen's top left, y down.

import AppKit
import SwiftUI

enum IslandShape: String { case closed, compact, open }

enum IslandDock { case top, bottom, left, right, free
    var upright: Bool { self == .left || self == .right }
}

enum IslandSize {
    static let compactW: CGFloat = 188
    static let compactSide: CGFloat = 27
    static let minisSide: CGFloat = 27
    static let compactH: CGFloat = 38.5
    static let compactTall: CGFloat = 128
    static let compactTallSong: CGFloat = 176
    static let expandedW: CGFloat = 760
    static let closedW: CGFloat = 80
    static let closedH: CGFloat = 24
    static let closedCorner: CGFloat = 10
    static let compactCorner: CGFloat = 18
    static let expandedCorner: CGFloat = 22
    static let homeH: CGFloat = 160
    static let feedH: CGFloat = 270
    static let githubH: CGFloat = 404
    static let musicH: CGFloat = 318
    static let editH: CGFloat = 290
    static let questionH: CGFloat = 200
    static let approvalH: CGFloat = 160
    static let approvalOpenH: CGFloat = 380
    static let questionMost: CGFloat = 420
    static let pageTop: CGFloat = 42
    static let talkMin = CGSize(width: 420, height: 220)
    // around a camera notch
    static let closedWing: CGFloat = 30
    static let closedWingSong: CGFloat = 48
    static let compactWing: CGFloat = 54
    static let compactWingWide: CGFloat = 104
    static let headerWing: CGFloat = 14 + 7 * 35 + 6
    static let ear: CGFloat = 8
    static let headerMid: CGFloat = 8 + 17
    static let snap: CGFloat = 60
    static let room = CGSize(width: 720, height: 420)
    static let hitMargin: CGFloat = 14
}

/// The size and corner for `shape`; open, for what the page shows (`mod.rs::target`).
func islandTarget(_ shape: IslandShape, tab: String, pinned: Bool, settings: Bool, tall: Bool, body: CGFloat, chat: CGSize) -> (CGSize, CGFloat) {
    typealias S = IslandSize
    switch shape {
    case .closed: return (CGSize(width: S.closedW, height: S.closedH), S.closedCorner)
    case .compact: return (CGSize(width: S.compactW, height: S.compactH), S.compactCorner)
    case .open: break
    }
    let width = S.expandedW
    if pinned { return (CGSize(width: width, height: 42 + body + 10), S.expandedCorner) }
    if settings { return (CGSize(width: width, height: S.homeH), S.expandedCorner) }
    switch tab {
    case "home": return (CGSize(width: width, height: S.feedH), S.expandedCorner)
    case "github": return (CGSize(width: width, height: S.githubH), S.expandedCorner)
    case "music": return (CGSize(width: width, height: S.musicH), S.expandedCorner)
    case "chats", "claude": return (CGSize(width: width, height: tall ? S.editH : S.homeH), S.expandedCorner)
    case "chat": return (chat, S.expandedCorner)
    default: return (CGSize(width: width, height: S.homeH), S.expandedCorner)
    }
}

/// Standing up on a side of the screen (`mod.rs::upright`).
func islandUpright(_ shape: IslandShape, _ size: CGSize, song: Bool) -> CGSize {
    shape == .compact
        ? CGSize(width: IslandSize.compactH, height: song ? IslandSize.compactTallSong : IslandSize.compactTall)
        : CGSize(width: size.height, height: size.width)
}

/// Grown out of a camera notch (`mod.rs::around_notch`): closed and compact the notch with a wing each side, as tall as
/// it; open, its header on the notch's row, at least wide enough for the tabs and the buttons beside the camera.
func islandAroundNotch(_ shape: IslandShape, _ size: CGSize, notch: CGSize, song: Bool, voice: Bool) -> CGSize {
    let wings = { (wing: CGFloat) in notch.width + 2 * wing }
    switch shape {
    case .closed: return CGSize(width: wings(song ? IslandSize.closedWingSong : IslandSize.closedWing), height: notch.height)
    case .compact: return CGSize(width: wings(song || voice ? IslandSize.compactWingWide : IslandSize.compactWing), height: notch.height)
    case .open: return CGSize(width: max(size.width, wings(IslandSize.headerWing)), height: size.height)
    }
}

/// The conversation's size (`talk_size`): as pulled, or growing with what was said.
func islandTalkSize(saved: [Double]?, said: Int, work: CGRect) -> CGSize {
    guard let saved, saved.count == 2 else { return CGSize(width: IslandSize.expandedW, height: min(240 + CGFloat(said) * 40, 300)) }
    return clamp(CGSize(width: saved[0], height: saved[1]), least: IslandSize.talkMin, most: CGSize(width: work.width * 0.95, height: work.height * 0.95))
}

private func clamp(_ size: CGSize, least: CGSize, most: CGSize) -> CGSize {
    CGSize(width: min(max(size.width, min(least.width, most.width)), most.width), height: min(max(size.height, min(least.height, most.height)), most.height))
}

/// The room the window has for the island (`window_cap`): a little more than the conversation, never less than `room`.
func islandWindowCap(chat: CGSize) -> CGSize {
    CGSize(width: max(chat.width * 1.05, IslandSize.room.width), height: max(chat.height * 1.05, IslandSize.room.height))
}

// MARK: docks (window.rs)

func islandDockOf(_ anchor: CGPoint, work: CGRect) -> IslandDock {
    let at = { (value: CGFloat, edge: CGFloat) in abs(value - edge) < 0.5 }
    if at(anchor.x, work.minX) { return .left }
    if at(anchor.x, work.maxX - 1) { return .right }
    if anchor.y <= work.minY + 2 { return .top }
    if at(anchor.y, work.maxY - 1) { return .bottom }
    return .free
}

/// The island of `size` where it hangs from `anchor` (`hung`).
func islandHung(_ anchor: CGPoint, dock: IslandDock, size: CGSize) -> CGRect {
    let origin: CGPoint
    switch dock {
    case .top, .free: origin = CGPoint(x: anchor.x - size.width / 2, y: anchor.y)
    case .bottom: origin = CGPoint(x: anchor.x - size.width / 2, y: anchor.y + 1 - size.height)
    case .left: origin = CGPoint(x: anchor.x, y: anchor.y - size.height / 2)
    case .right: origin = CGPoint(x: anchor.x + 1 - size.width, y: anchor.y - size.height / 2)
    }
    return CGRect(origin: origin, size: size)
}

/// `rect` moved into `room` along each axis it fits in; along one it does not, centred (`centre`) or left (`kept_in`).
func islandKeptIn(_ rect: CGRect, room: CGRect, centre: Bool) -> CGRect {
    func axis(_ min: CGFloat, _ length: CGFloat, _ low: CGFloat, _ high: CGFloat) -> CGFloat {
        if high - low < length { return centre ? low + (high - low - length) / 2 : min }
        return Swift.min(Swift.max(min, low), high - length)
    }
    return CGRect(x: axis(rect.minX, rect.width, room.minX, room.maxX), y: axis(rect.minY, rect.height, room.minY, room.maxY), width: rect.width, height: rect.height)
}

func islandEven(_ value: CGFloat) -> CGFloat { (value / 2).rounded(.up) * 2 }

/// Where the window's top left goes for `room` hanging from `anchor`, kept on its monitor (`kept_origin`).
func islandKeptOrigin(_ anchor: CGPoint, room: CGSize, work: CGRect) -> CGPoint {
    let size = CGSize(width: islandEven(room.width), height: islandEven(room.height))
    let rect = islandKeptIn(islandHung(anchor, dock: islandDockOf(anchor, work: work), size: size), room: work, centre: false)
    return CGPoint(x: rect.minX.rounded(), y: rect.minY.rounded())
}

/// Where the island is in its window (`island_in`).
func islandIn(_ anchor: CGPoint, dock: IslandDock, origin: CGPoint, window: CGSize, island: CGSize) -> CGRect {
    let rect = islandHung(anchor, dock: dock, size: island).offsetBy(dx: -origin.x, dy: -origin.y)
    return islandKeptIn(rect, room: CGRect(origin: .zero, size: window), centre: true)
}

func islandKeepOnScreen(_ anchor: CGPoint, work: CGRect) -> CGPoint {
    let x = min(max(anchor.x, work.minX + 60), max(work.maxX - 60, work.minX + 60))
    switch islandDockOf(anchor, work: work) {
    case .left, .right: return CGPoint(x: anchor.x, y: min(max(anchor.y, work.minY), work.maxY - 1))
    case .top: return CGPoint(x: x, y: work.minY)
    case .bottom: return CGPoint(x: x, y: anchor.y)
    case .free: return CGPoint(x: x, y: min(max(anchor.y, work.minY), max(work.maxY - 80, work.minY)))
    }
}

/// Where an island let go at `island` hangs from: a side first, then the top or the bottom; else where it is (`dropped`).
func islandDropped(_ island: CGRect, work: CGRect) -> CGPoint {
    let center = CGPoint(x: island.midX.rounded(), y: island.midY.rounded())
    if island.minX - work.minX < IslandSize.snap { return CGPoint(x: work.minX, y: center.y) }
    if work.maxX - island.maxX < IslandSize.snap { return CGPoint(x: work.maxX - 1, y: center.y) }
    if island.minY - work.minY < IslandSize.snap { return CGPoint(x: center.x, y: work.minY) }
    if work.maxY - island.maxY < IslandSize.snap { return CGPoint(x: center.x, y: work.maxY - 1) }
    return CGPoint(x: center.x, y: island.minY.rounded())
}

/// On the top edge of a screen with a camera notch the anchor goes to its middle (`to_notch`).
func islandToNotch(_ anchor: CGPoint, work: CGRect, notch: CGRect?) -> CGPoint {
    if let notch, islandDockOf(anchor, work: work) == .top { return CGPoint(x: notch.midX, y: work.minY) }
    return anchor
}

/// Where Ajustes › Lugar da ilha puts it (`place_anchor`).
func islandPlaceAnchor(_ place: String, anchor: CGPoint, work: CGRect) -> CGPoint {
    let dock = islandDockOf(anchor, work: work)
    let y = dock == .top || dock == .bottom ? work.midY.rounded() : anchor.y
    switch place {
    case "bottom": return CGPoint(x: work.midX.rounded(), y: work.maxY - 1)
    case "left": return CGPoint(x: work.minX, y: y)
    case "right": return CGPoint(x: work.maxX - 1, y: y)
    default: return CGPoint(x: work.midX.rounded(), y: work.minY)
    }
}

// MARK: screens (macos.rs)

struct IslandScreen {
    /// The usable part: less the IslandDock, with the menu bar (the island hangs from the very top).
    var work: CGRect
    var notch: CGRect?
    var screen: NSScreen
}

@MainActor enum IslandScreens {
    static var mainHeight: CGFloat { NSScreen.screens.first?.frame.height ?? 0 }

    static func all() -> [IslandScreen] {
        let height = mainHeight
        return NSScreen.screens.map { screen in
            let frame = screen.frame, visible = screen.visibleFrame
            let top = height - frame.maxY
            let work = CGRect(x: visible.minX, y: top, width: visible.width, height: height - visible.minY - top)
            let (left, right) = (screen.auxiliaryTopLeftArea ?? .zero, screen.auxiliaryTopRightArea ?? .zero)
            var notch: CGRect?
            let tall = min(left.height, right.height)
            if left.width > 0, right.width > 0, tall > 0, left.width + right.width < frame.width {
                notch = CGRect(x: frame.minX + left.width, y: top, width: frame.width - left.width - right.width, height: tall)
            }
            return IslandScreen(work: work, notch: notch, screen: screen)
        }
    }

    /// The screen `point` is on, else the nearest (`chat::mini::work_at`).
    static func at(_ point: CGPoint) -> IslandScreen? {
        all().min(by: { distance($0.work, point) < distance($1.work, point) })
    }

    private static func distance(_ rect: CGRect, _ point: CGPoint) -> CGFloat {
        let dx = max(rect.minX - point.x, 0, point.x - rect.maxX), dy = max(rect.minY - point.y, 0, point.y - rect.maxY)
        return dx * dx + dy * dy
    }

    /// The cursor in Rust's coordinates.
    static var cursor: CGPoint {
        let at = NSEvent.mouseLocation
        return CGPoint(x: at.x, y: mainHeight - at.y)
    }

    /// A rect in Rust's coordinates as AppKit's (origin at the main screen's bottom left).
    static func appKit(_ rect: CGRect) -> NSRect {
        NSRect(x: rect.minX, y: mainHeight - rect.maxY, width: rect.width, height: rect.height)
    }
}

// MARK: the clock (fsm.rs)

enum IslandStage { case hidden, petit, home, greeting }

/// When the island opens and closes, by the clock: the pointer peeks (compact), a click opens it, open it folds a while
/// after the pointer left ("Fechar auto"), compact it hides after `compact_after`.
struct IslandMachine {
    static let greetingStays = 0.6
    static let greetingHeld = 10.0
    static let hoverOpens = 0.5
    static let hoverCloses = 0.6

    private(set) var stage = IslandStage.hidden
    private var due: Double?
    private var hoverDue: Double?
    private var byHover = false
    private var inside = false
    private var held = false
    private var closeAfter = 15.0
    var compactAfter = 60.0
    private var openOnHover = false

    mutating func launch() { clear(); stage = .greeting }

    mutating func step(_ time: Double, inside: Bool, held: Bool, closeAfter: Double, openOnHover: Bool) {
        if closeAfter != self.closeAfter {
            self.closeAfter = closeAfter
            if stage == .home, due != nil, !byHover { due = time + closeAfter }
        }
        self.openOnHover = openOnHover
        if held != self.held {
            self.held = held
            if held { clear() } else if !self.inside { rest(time) }
        }
        if inside != self.inside {
            self.inside = inside
            if inside { entered(time) } else { left(time) }
        }
        if let hoverDue, time >= hoverDue {
            self.hoverDue = nil
            if stage == .petit, !self.held, self.openOnHover { enter(.home, time); byHover = true }
        }
        if let due, time >= due, !self.held {
            byHover = false
            switch stage {
            case .petit: enter(.hidden, time)
            case .home, .greeting: enter(.petit, time)
            case .hidden: self.due = nil
            }
        }
    }

    mutating func click(_ time: Double) {
        if stage == .hidden || stage == .petit { byHover = false; enter(.home, time) }
    }

    mutating func interacted(_ time: Double) {
        if byHover, stage == .home, due != nil { due = time + closeAfter }
        byHover = false
    }

    mutating func greeted(_ time: Double) {
        if stage == .greeting, due == nil { due = time + Self.greetingStays }
    }

    mutating func reveal(_ time: Double) { if stage == .hidden { enter(.petit, time) } }
    mutating func forceHome(_ time: Double) { byHover = false; enter(.home, time) }
    mutating func forcePetit(_ time: Double) { byHover = false; enter(.petit, time) }

    /// How much of the open island's wait is left, in its last 60% (10 s at most): the bar under it.
    func countdown(_ time: Double) -> Double? {
        guard let due, stage == .home, !held else { return nil }
        let window = min(closeAfter * 0.6, 10), left = due - time
        return window > 0 && left < window ? min(max(left / window, 0), 1) : nil
    }

    private mutating func clear() { due = nil; hoverDue = nil }

    private mutating func enter(_ stage: IslandStage, _ time: Double) {
        clear()
        self.stage = stage
        if !inside { rest(time) }
    }

    private mutating func rest(_ time: Double) {
        if held { return }
        switch stage {
        case .petit: due = compactAfter > 0 ? time + compactAfter : nil
        case .home: due = time + (byHover ? Self.hoverCloses : closeAfter)
        case .hidden, .greeting: break
        }
    }

    private mutating func entered(_ time: Double) {
        switch stage {
        case .hidden: enter(.petit, time); restOpens(time)
        case .petit: due = nil; restOpens(time)
        case .home: due = nil
        case .greeting: due = time + Self.greetingHeld
        }
    }

    private mutating func restOpens(_ time: Double) { if openOnHover, !held { hoverDue = time + Self.hoverOpens } }

    private mutating func left(_ time: Double) {
        hoverDue = nil
        if stage == .greeting { enter(.petit, time) } else { rest(time) }
    }
}

// MARK: the compact island's minis (minis.rs) and the song's place (pill.rs)

/// The cells of the first `count` agents in the 2×2 grid `minisSide` from the right; on an island as short as a notch
/// they shrink to fit, 2 from the top and the bottom.
func islandMiniCells(_ island: CGRect, count: Int) -> [CGRect] {
    let count = min(count, 4), gap: CGFloat = 2
    let body = min(14, (island.height - 4 - gap) / 2)
    let corner = CGPoint(x: island.maxX - IslandSize.minisSide - body - gap / 2, y: island.midY - body - gap / 2)
    return (0..<count).map { index in
        let (column, row): (CGFloat, CGFloat) = count == 1 ? (1, 0.5) : count == 2 ? (CGFloat(index), 0.5) : (CGFloat(index % 2), CGFloat(index / 2))
        return CGRect(x: corner.x + column * (body + gap), y: corner.y + row * (body + gap), width: body, height: body)
    }
}

/// Standing up: row by row, the grid at its bottom (`upright_cells`).
func islandUprightCells(_ island: CGRect, count: Int) -> [CGRect] {
    let corner = CGPoint(x: island.midX - 15, y: island.maxY - 24 - 15)
    return (0..<min(count, 4)).map { CGRect(x: corner.x + CGFloat($0 % 2) * 16, y: corner.y + CGFloat($0 / 2) * 16, width: 14, height: 14) }
}

enum IslandPill {
    static let cover: CGFloat = 22
    static let bars = 10
    static let bar: CGFloat = 1.5
    static let barGap: CGFloat = 3
    static let barsTall: CGFloat = 16
    static let barsRoom = CGFloat(bars) * bar + CGFloat(bars - 1) * barGap
    static let pairGap: CGFloat = 10
    static let wizard: CGFloat = 30

    /// Where the cover and the bars start on the compact island (`spread`): between the wizard and the minis; around a
    /// notch the cover in the left wing after the wizard and the bars in the right one before the minis.
    static func spread(_ rect: CGRect, minis: CGRect?, notch: CGFloat) -> (CGFloat, CGFloat) {
        let pair = cover + pairGap + barsRoom
        let edge = IslandSize.compactSide - wizard / 2
        if notch > 0 {
            let barsEnd = minis.map { $0.minX - pairGap } ?? rect.maxX - edge
            return (rect.minX + edge + wizard + pairGap, barsEnd - barsRoom)
        }
        let start: CGFloat
        if let grid = minis { start = (rect.minX + edge + wizard + grid.minX) / 2 - pair / 2 } else { start = rect.maxX - edge - pair }
        return (start, start + cover + pairGap)
    }
}
