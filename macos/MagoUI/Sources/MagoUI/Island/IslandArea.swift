// The island's window and clock on macOS: a borderless panel over everything (every Space, over full-screen apps) that
// lets the mouse through outside the island. Swift owns when it opens and folds (`IslandMachine`, the port of fsm.rs);
// Rust sends what it shows (topic "island") and what it forced (`seq`/`forced`), and plays the sounds.

import AppKit
import Observation
import SwiftUI

@MainActor public enum IslandArea {
    private static var model: IslandModel?

    /// Opens the island (once).
    public static func start() {
        guard model == nil else { return }
        let model = IslandModel()
        let level = NSWindow.Level(rawValue: NSWindow.Level.mainMenu.rawValue + 1)
        let window = MagoWindow.make(
            title: "Ilha", frame: NSRect(origin: .zero, size: IslandSize.room), borderless: true, transparent: true, level: level,
            content: IslandView(model: model))
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        window.hasShadow = false
        window.isMovable = false
        window.ignoresMouseEvents = true
        model.window = window
        Self.model = model
        model.begin()
    }
}

@MainActor @Observable final class IslandModel {
    static let greetingEnds = 4.65

    var snapshot = IslandSnapshot()
    /// What moves every frame (the mic, the groove): read only inside the views that redraw every frame.
    @ObservationIgnored var live = IslandLive()
    var shape = IslandShape.closed
    var greeting = false
    /// The island in its window (SwiftUI's points, y down), its corner, the dock and the notch it grows from.
    var rect = CGRect(x: 320, y: 0, width: IslandSize.closedW, height: IslandSize.closedH)
    var corner = IslandSize.closedCorner
    var dock = IslandDock.top
    var notch: CGSize?
    var room = IslandSize.room
    var countdown: Double?
    /// A field of the island has the keyboard: it does not fold.
    var typing = false
    /// The full request of a permission open ("ver tudo") and the question card's height.
    var askHeight = IslandSize.questionH
    /// When the ask on the card arrived: clicks are ignored for 600 ms after (`PersonAsk::guarded`).
    var askSince = 0.0

    @ObservationIgnored weak var window: NSWindow?
    /// The window's top left in Rust's screen points, for the eyes that follow the pointer.
    @ObservationIgnored var origin = CGPoint.zero
    @ObservationIgnored private var machine = IslandMachine()
    @ObservationIgnored private var anchor: CGPoint?
    @ObservationIgnored private var work = CGRect(x: 0, y: 0, width: 1440, height: 900)
    @ObservationIgnored private var notchRect: CGRect?
    @ObservationIgnored private var seq: UInt64?
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var keys: Any?
    @ObservationIgnored private var started = 0.0
    @ObservationIgnored private var sentShape: IslandShape?
    @ObservationIgnored private var drag: (mouse: CGPoint, origin: CGPoint)?
    @ObservationIgnored private var ticks = 0
    @ObservationIgnored private var askChat: String?

    var now: Double { CACurrentMediaTime() }
    var pinned: Bool { snapshot.ask != nil && snapshot.pinned }

    func begin() {
        observe()
        pull()
        // ponytail: a 120 Hz timer polls the cursor (the window passes clicks through, so it hears no hover); a
        // tracking area per shape would be lighter if this shows in Activity Monitor
        let timer = Timer(timeInterval: 1.0 / 120, repeats: true) { _ in MainActor.assumeIsolated { self.tick() } }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
        keys = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let (code, letter) = (event.keyCode, event.charactersIgnoringModifiers?.lowercased() ?? "")
            let number = event.windowNumber
            return MainActor.assumeIsolated { self.window?.windowNumber == number && self.key(code, letter) } ? nil : event
        }
    }

    /// Reads the topics again whenever the store changes.
    private func observe() {
        withObservationTracking {
            _ = MagoStore.shared.decode(IslandSnapshot.self, topic: "island")
            _ = MagoStore.shared.decode(IslandLive.self, topic: "island.live")
        } onChange: {
            Task { @MainActor in
                self.pull()
                self.observe()
            }
        }
    }

    private func pull() {
        if let snapshot = MagoStore.shared.decode(IslandSnapshot.self, topic: "island"), snapshot != self.snapshot { took(snapshot) }
        if let live = MagoStore.shared.decode(IslandLive.self, topic: "island.live"), live != self.live { self.live = live }
    }

    /// A new snapshot: what Rust forced since the last one lands on the clock.
    private func took(_ snapshot: IslandSnapshot) {
        if snapshot.ask?.chat != askChat {
            askChat = snapshot.ask?.chat
            askSince = now
        }
        self.snapshot = snapshot
        if anchor == nil, let at = snapshot.at { setAnchor(CGPoint(x: at[0], y: at[1]), send: false) }
        guard snapshot.seq != seq else { return }
        let first = seq == nil
        seq = snapshot.seq
        if first { return }
        switch snapshot.forced {
        case "home": machine.forceHome(now)
        case "petit": machine.forcePetit(now)
        case "reveal": machine.reveal(now)
        case "greeting": machine.launch(); started = now; greeting = true
        default: break
        }
        if let place = snapshot.place, let anchor = anchor ?? defaultAnchor() {
            setAnchor(islandPlaceAnchor(place, anchor: anchor, work: IslandScreens.at(anchor)?.work ?? work), send: true)
        }
    }

    private func defaultAnchor() -> CGPoint? {
        guard let main = IslandScreens.all().first else { return nil }
        return CGPoint(x: main.notch?.midX ?? main.work.midX.rounded(), y: main.work.minY)
    }

    /// Hangs the island from `at`, kept on its screen and on the notch's middle on a top edge with one.
    private func setAnchor(_ at: CGPoint, send: Bool) {
        guard let screen = IslandScreens.at(at) else { return }
        let kept = islandToNotch(islandKeepOnScreen(at, work: screen.work), work: screen.work, notch: screen.notch)
        (anchor, work, notchRect) = (kept, screen.work, screen.notch)
        if send { act("place", ["x": kept.x, "y": kept.y]) }
    }

    // MARK: the clock

    private func tick() {
        let time = now
        ticks += 1
        // the store is observed; this only catches a foundation whose store is not
        if ticks % 30 == 0 { pull() }
        guard let window else { return }
        guard snapshot.on else {
            if window.isVisible { window.orderOut(nil) }
            return
        }
        if !window.isVisible {
            window.orderFrontRegardless()
            // the first time it shows, Luna makes her entrance with the panel open
            // ponytail: the panel opening on Início is the whole entrance; port entry.rs (her drop and the sparks) if missed
            if started == 0 {
                (started, greeting) = (time, true)
                machine.launch()
                act("chime", ["name": "greet"])
            }
        }
        if anchor == nil, let first = defaultAnchor() { setAnchor(first, send: false) }
        guard let anchor else { return }
        if greeting, time - started >= Self.greetingEnds { machine.greeted(time) }
        // an ask holds it open on its card, and so does the new chat's form, until answered or folded
        let holds = pinned || snapshot.tab == "new"
        if holds, machine.stage != .home { machine.forceHome(time) }
        let cursor = IslandScreens.cursor
        let screenRect = rect.offsetBy(dx: origin.x, dy: origin.y)
        let inside = screenRect.contains(cursor)
        let pressing = NSEvent.pressedMouseButtons & 1 != 0 && (inside || drag != nil)
        machine.compactAfter = Double(snapshot.compactAfter)
        machine.step(time, inside: inside, held: pressing || typing || holds, closeAfter: Double(snapshot.closeAfter), openOnHover: false)
        greeting = machine.stage == .greeting
        let shape: IslandShape = switch machine.stage {
        case .hidden: .closed
        case .petit: .compact
        case .home, .greeting: .open
        }
        if shape != sentShape {
            sentShape = shape
            act("shape", ["shape": shape.rawValue])
        }
        let countdown = machine.countdown(time)
        if countdown != self.countdown { self.countdown = countdown }
        if drag == nil { place(shape, anchor: anchor) }
        // outside the island (and a margin for the pointer coming back) the clicks go to what is under it
        let catches = screenRect.insetBy(dx: -IslandSize.hitMargin, dy: -IslandSize.hitMargin).contains(cursor) || drag != nil
        if window.ignoresMouseEvents == catches { window.ignoresMouseEvents = !catches }
    }

    /// The window at its room on the anchor's screen and the island in it, on a spring growing and a curve shrinking.
    private func place(_ shape: IslandShape, anchor: CGPoint) {
        let dock = islandDockOf(anchor, work: work)
        let notch = dock == .top ? notchRect.flatMap { abs($0.midX - anchor.x) < 1 ? $0.size : nil } : nil
        let chat = islandTalkSize(saved: snapshot.talkSize, said: snapshot.chat?.said.count ?? 0, work: work)
        var (size, corner) = islandTarget(
            shape, tab: snapshot.tab, pinned: snapshot.ask != nil, settings: snapshot.settings,
            tall: snapshot.activity?.phase == "editing", body: askHeight - IslandSize.pageTop - 10, chat: chat)
        if dock.upright, shape != .open { size = islandUpright(shape, size, song: snapshot.song != nil) }
        if let notch { size = islandAroundNotch(shape, size, notch: notch, song: snapshot.song != nil, voice: snapshot.voice) }
        let room = islandWindowCap(chat: chat)
        let origin = islandKeptOrigin(anchor, room: room, work: work)
        let roomEven = CGSize(width: islandEven(room.width), height: islandEven(room.height))
        if origin != self.origin || roomEven != self.room {
            self.origin = origin
            self.room = roomEven
            window?.setFrame(IslandScreens.appKit(CGRect(origin: origin, size: roomEven)), display: true)
        }
        let rect = islandIn(anchor, dock: dock, origin: origin, window: roomEven, island: size)
        if dock != self.dock { self.dock = dock }
        if notch != self.notch { self.notch = notch }
        if shape != self.shape || rect != self.rect || corner != self.corner {
            let grows = rect.width * rect.height > self.rect.width * self.rect.height
            withAnimation(grows ? .spring(response: 0.5, dampingFraction: 0.72) : .timingCurve(0.45, 0, 0.2, 1, duration: 0.34)) {
                self.shape = shape
                self.rect = rect
                self.corner = corner
            }
        }
    }

    // MARK: what the person does

    /// A click on the closed or compact island opens it; on the open one it only counts as being used.
    func click() {
        if shape == .open { machine.interacted(now) } else { machine.click(now) }
    }

    /// Opened on `tab` with `id` in its card (a mini, a line of the feed).
    func open(tab: String, focus: String?) {
        if let focus { act("focus", ["id": focus]) }
        act("tab", ["tab": tab])
        machine.click(now)
        machine.interacted(now)
    }

    func fold() {
        act("close")
        machine.forcePetit(now)
    }

    /// The island moved by the press: the window follows the mouse; let go, it hangs from the nearest edge.
    func dragged() {
        guard let window else { return }
        let mouse = NSEvent.mouseLocation
        if drag == nil { drag = (mouse, window.frame.origin) }
        guard let drag else { return }
        window.setFrameOrigin(NSPoint(x: drag.origin.x + mouse.x - drag.mouse.x, y: drag.origin.y + mouse.y - drag.mouse.y))
    }

    func dropped() {
        guard let window, drag != nil else { return }
        drag = nil
        let top = IslandScreens.mainHeight - window.frame.maxY
        let island = rect.offsetBy(dx: window.frame.minX, dy: top)
        guard let screen = IslandScreens.at(CGPoint(x: island.midX, y: island.midY)) else { return }
        work = screen.work
        setAnchor(islandDropped(island, work: screen.work), send: true)
    }

    /// The island's keys while its window has the keyboard: Esc folds (or closes the settings), and the ask's own.
    private func key(_ code: UInt16, _ letter: String) -> Bool {
        guard shape == .open else { return false }
        if code == 53 {
            if snapshot.settings { act("settings") } else { fold() }
            return true
        }
        guard let ask = snapshot.ask, !typing, now - askSince > 0.6 else { return false }
        if ask.approval, letter == "y" || letter == "n" {
            act("ask", ["chat": ask.chat, "allow": letter == "y"])
            return true
        }
        if !ask.approval, let number = Int(letter), number >= 1, number <= 9 {
            act("ask", ["chat": ask.chat, "pick": number - 1])
            return true
        }
        return false
    }

    /// `ui.island.<verb>`, or any other of the app's actions by its own name.
    func act(_ verb: String, _ args: [String: Any] = [:]) {
        MagoStore.shared.send("ui.island.\(verb)", args)
    }

    func app(_ action: String, _ args: [String: Any] = [:]) {
        MagoStore.shared.send(action, args)
    }

    /// Ajustes by `set_setting` (the same as the MCP's).
    func setting(_ name: String, _ value: String) {
        app("set_setting", ["name": name, "value": value])
    }
}
