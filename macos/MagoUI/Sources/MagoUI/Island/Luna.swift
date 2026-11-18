// Luna (Lume), the island's wizard: a magic flame with two dark eyes, ported from `src/app/mascot/mago/` (draw.rs and
// motion.rs): the flame that licks and sways, the eyes that follow the pointer over the whole screen and blink alone,
// the faces of each state, the hop, the sparks, the Zz, the sweat, the badge, the pokes (annoyed, then dizzy) and the
// dance to the song. Drawn in a `Canvas` every display frame (`TimelineView(.animation)`: 120 Hz on ProMotion).

import SwiftUI

enum LunaState: String {
    case idle, working, needsYou = "needs_you", asking, done, error, sleeping, listening, speaking, waving
}

enum LunaEye { case round, wide, focus, happy, sleepy, cross, spiral }

/// The face and posture of a state (`LunaLook::of`).
struct LunaLook {
    var eye = LunaEye.round
    var mouthCurve: Double = 0.03
    var mouthOpen: Double = 0
    var badge: Character?
    var tilt: Double = 0
    var sway: (Double, Double) = (0.12, 1.6)
    var tint: (Color, Double) = (Luna.red, 0)

    static func of(_ state: LunaState) -> LunaLook {
        var look = LunaLook()
        switch state {
        case .idle: break
        case .working: look.eye = .focus; look.mouthCurve = 0; look.badge = "…"; look.sway = (0.9, 5)
        case .needsYou: look.eye = .wide; look.mouthCurve = 0; look.badge = "!"; look.sway = (0.35, 4)
        case .asking: look.eye = .wide; look.mouthCurve = -0.006; look.badge = "?"; look.tilt = 0.12
        case .done: look.eye = .happy; look.mouthCurve = 0.055; look.sway = (0.5, 6)
        case .error: look.eye = .cross; look.mouthCurve = -0.035; look.sway = (0, 1); look.tint = (Luna.red, 0.3)
        case .sleeping: look.eye = .sleepy; look.mouthCurve = 0; look.tilt = 0.07; look.sway = (0, 1); look.tint = (Luna.night, 0.22)
        case .listening: look.eye = .wide; look.mouthCurve = 0.015; look.sway = (0.2, 2.2)
        case .speaking: look.mouthCurve = 0; look.mouthOpen = 0.03
        case .waving: look.eye = .happy; look.mouthCurve = 0.05; look.sway = (0.4, 5)
        }
        return look
    }
}

struct LunaTween {
    var value: Double
    var target: Double
    init(_ value: Double) { self.value = value; target = value }
    mutating func step(_ delta: Double, _ rate: Double) {
        if !value.isFinite { value = target.isFinite ? target : 0 }
        value += (target - value) * (1 - exp(-rate * delta))
    }
}

struct LunaParticle {
    enum Kind { case spark, zzz, sweat }
    var at: CGPoint
    var velocity: CGVector
    var age: Double = 0
    var life: Double
    var kind: Kind
    var spin: Double
}

/// The motion of one wizard (`MagoMotion`): properties chasing targets, the sway's spring, the hop, the blinks, the
/// gaze, the particles and the pokes.
final class LunaMotion {
    static let bodyCenter = 0.2, bodyHalf = (0.3, 0.27)
    static let flameTip = -0.4

    private var last: Double?
    private(set) var state = LunaState.idle
    private var since = 0.0
    var squashX = LunaTween(1), squashY = LunaTween(1), tilt = LunaTween(0), lookX = LunaTween(0), lookY = LunaTween(0), open = LunaTween(1)
    var badgeScale = LunaTween(0), tintAmount = LunaTween(0), mouthOpen = LunaTween(0)
    var tintColor = Luna.red
    var sway = 0.0, swayVelocity = 0.0, hop = 0.0, hopVelocity = 0.0
    private var nextHop = 0.0, blinkAt = 1.5, blinkUntil = 0.0, shakeUntil = 0.0
    private enum Reaction { case none, annoyed, dizzy }
    private var reaction = Reaction.none, reactionUntil = 0.0
    private var pokes = 0, lastPoke = -Double.infinity
    private(set) var pokesTotal = 0
    private var sparkBudget = 0.0
    private var seed: UInt32
    var particles: [LunaParticle] = []
    var look = LunaLook()
    var dance: (LunaEye, Double, Double)?
    var breath = 0.0, talk = 1.0, spin = 0.0
    var badge: Character?
    private var glanceAt = CGVector.zero, glanceUntil = 0.0

    init(seed: UInt32) { self.seed = seed | 1 }

    private func random() -> Double {
        seed ^= seed << 13
        seed ^= seed >> 17
        seed ^= seed << 5
        return Double(seed % 10_000) / 10_000
    }

    /// A mini one never follows the pointer: it glances around, somewhere new every 0.5 to 2 s.
    func glance(_ time: Double) -> CGVector {
        if time >= glanceUntil {
            glanceAt = CGVector(dx: -0.88 + random() * 1.76, dy: -0.45 + random())
            glanceUntil = time + 0.5 + random() * 1.5
        }
        return glanceAt
    }

    private func jump(_ strength: Double) {
        if hop <= 0 { hopVelocity = strength; swayVelocity += 4 }
    }

    /// A click: annoyed at first, dizzy if it insists (4 within a short while).
    func poke(_ time: Double) {
        if time - lastPoke > 1.5 { pokes = 0 }
        pokes += 1
        pokesTotal += 1
        lastPoke = time
        if pokes >= 4 {
            (reaction, reactionUntil, shakeUntil) = (.dizzy, time + 2.4, time + 2.4)
        } else {
            (reaction, reactionUntil, shakeUntil) = (.annoyed, time + 0.8, time + 0.5)
            jump(1.4)
        }
    }

    func dizzy(_ time: Double) -> Bool { reaction == .dizzy && time < reactionUntil }
    var lively: Bool { state == .working || state == .speaking }

    private func emit(_ kind: LunaParticle.Kind) {
        guard particles.count < 40 else { return }
        let (a, b) = (random(), random())
        switch kind {
        case .spark:
            let angle = a * 2 * .pi, speed = 0.18 + b * 0.3
            particles.append(LunaParticle(at: CGPoint(x: sway * 0.04, y: Self.flameTip), velocity: CGVector(dx: cos(angle) * speed, dy: (sin(angle) - 0.4) * speed),
                                      life: 0.7 + b * 0.4, kind: kind, spin: a))
        case .zzz: particles.append(LunaParticle(at: CGPoint(x: -0.28, y: -0.25), velocity: CGVector(dx: 0.05, dy: -0.1), life: 2.2, kind: kind, spin: a))
        case .sweat: particles.append(LunaParticle(at: CGPoint(x: 0.3, y: 0.1), velocity: CGVector(dx: 0.04, dy: 0.05), life: 1, kind: kind, spin: a))
        }
    }

    private func enter(_ state: LunaState, _ time: Double) {
        self.state = state
        since = time
        switch state {
        case .done: jump(2.4); for _ in 0..<14 { emit(.spark) }
        case .error: shakeUntil = time + 0.8; emit(.sweat)
        case .needsYou: nextHop = time + 0.15
        default: break
        }
    }

    private func currentLook() -> LunaLook {
        var look = LunaLook.of(state)
        switch reaction {
        case .annoyed: look.eye = .focus; look.mouthCurve = -0.03; look.badge = nil
        case .dizzy: look.eye = .spiral; look.mouthCurve = 0; look.mouthOpen = 0.03; look.badge = nil
        case .none:
            if let (eye, curve, open) = dance { look.eye = eye; look.mouthCurve = curve; look.mouthOpen = open }
        }
        return look
    }

    /// Moves everything to `time`; `gaze` is where the eyes look (-1 to 1 each axis), none for a slow wander.
    func advance(_ state: LunaState, _ time: Double, gaze: CGVector?) {
        let first = last == nil
        let delta = last.map { min(max(time - $0, 0), 0.1) } ?? 0
        last = time
        if first { blinkAt = time + 1 + random() * 3 }
        if state != self.state || first { enter(state, time) }
        if time >= reactionUntil, reaction != .none { reaction = .none }
        look = currentLook()
        spin += delta * 9
        let gaze = gaze.map { CGVector(dx: min(max($0.dx, -1), 1), dy: min(max($0.dy, -1), 1)) }
            ?? CGVector(dx: sin(time * 0.45) * 0.5, dy: cos(time * 0.31) * 0.2)
        lookX.target = gaze.dx
        lookY.target = gaze.dy
        if time >= blinkAt { blinkUntil = time + 0.12; blinkAt = time + 2.4 + random() * 3 }
        open.target = time < blinkUntil ? 0.08 : 1
        tilt.target = look.tilt
        if time < shakeUntil { tilt.target += reaction == .dizzy ? sin(time * 6) * 0.12 : sin(time * 38) * 0.07 }
        badge = look.badge
        badgeScale.target = look.badge == nil ? 0 : 1
        mouthOpen.target = look.mouthOpen
        talk = self.state == .speaking ? 0.5 + 0.5 * sin(time * 11) * cos(time * 3.1) : 1
        if look.tint.1 > 0 { tintColor = look.tint.0 }
        tintAmount.target = look.tint.1
        let swayTarget = self.state == .sleeping || self.state == .error ? 0.7 : look.sway.0 * sin(time * look.sway.1)
        breath = 0.022 * sin(time * (self.state == .working ? 4 : 2))
        if first {
            tilt.value = tilt.target; lookX.value = lookX.target; lookY.value = lookY.target
            badgeScale.value = badgeScale.target; tintAmount.value = tintAmount.target; mouthOpen.value = mouthOpen.target
            return
        }
        squashX.target = 1
        squashY.target = 1
        squashX.step(delta, 14); squashY.step(delta, 14); tilt.step(delta, 12)
        lookX.step(delta, 5.99); lookY.step(delta, 5.99); open.step(delta, 40)
        badgeScale.step(delta, 14); tintAmount.step(delta, 5); mouthOpen.step(delta, 12)
        swayVelocity += (-70 * (sway - swayTarget) - 7 * swayVelocity) * delta
        sway = min(max(sway + swayVelocity * delta, -1.2), 1.2)
        if self.state == .needsYou, time >= nextHop { jump(1.5); nextHop = time + 1.7 }
        if hop > 0 || hopVelocity > 0 {
            hopVelocity -= 14 * delta
            hop += hopVelocity * delta
            if hop <= 0 {
                hop = 0
                if hopVelocity < -0.5 { squashY.value = 0.84; squashX.value = 1.1; swayVelocity -= 3 }
                hopVelocity = 0
            }
        }
        var rate = 0.0
        var kind = LunaParticle.Kind.spark
        switch self.state {
        case .done where time - since < 0.9: rate = 20
        case .working: rate = 3
        case .sleeping: (kind, rate) = (.zzz, 0.8)
        case .error: (kind, rate) = (.sweat, 0.5)
        default: break
        }
        sparkBudget += rate * delta
        while sparkBudget >= 1 { sparkBudget -= 1; emit(kind) }
        for index in particles.indices {
            particles[index].age += delta
            switch particles[index].kind {
            case .spark:
                particles[index].velocity.dx *= 1 - 1.5 * delta
                particles[index].velocity.dy = particles[index].velocity.dy * (1 - 1.5 * delta) + 0.35 * delta
            case .zzz: particles[index].velocity.dx = 0.05 * sin(particles[index].age * 3 + particles[index].spin * 6)
            case .sweat: particles[index].velocity.dy += 0.4 * delta
            }
            particles[index].at.x += particles[index].velocity.dx * delta
            particles[index].at.y += particles[index].velocity.dy * delta
        }
        particles.removeAll { $0.age >= $0.life }
    }

    struct Eye { var at: CGPoint; var flat: CGSize }

    /// The two eyes as the gaze turns them on the curved face; none for one turned away.
    func eyes() -> [Eye?] {
        let (yaw, pitch) = (lookX.value * 0.62, -lookY.value * 0.5)
        return [-1.0, 1.0].map { side in
            let (eyeYaw, eyePitch) = (side * 0.37 + yaw, -0.12 + pitch)
            let (across, up) = (cos(eyeYaw), cos(eyePitch))
            guard across * up > 0.04 else { return nil }
            return Eye(at: CGPoint(x: sin(eyeYaw) * up * Self.bodyHalf.0, y: Self.bodyCenter - sin(eyePitch) * Self.bodyHalf.1),
                       flat: CGSize(width: max(across, 0.18), height: max(up, 0.18)))
        }
    }

    func mouth() -> Eye? {
        let (yaw, pitch) = (lookX.value * 0.62, -0.42 - lookY.value * 0.5)
        let (across, up) = (cos(yaw), cos(pitch))
        guard across * up > 0.04 else { return nil }
        return Eye(at: CGPoint(x: sin(yaw) * up * Self.bodyHalf.0, y: Self.bodyCenter - sin(pitch) * Self.bodyHalf.1),
                   flat: CGSize(width: max(across, 0.18), height: max(up, 0.18)))
    }
}

/// Figure units to the canvas: squashed, turned about the pivot, moved by `shift` (`draw.rs::LunaPen`).
struct LunaPen {
    var origin: CGPoint
    var scale: Double
    var squash = CGSize(width: 1, height: 1)
    var turn = 0.0
    var shift = CGVector.zero

    func at(_ x: Double, _ y: Double) -> CGPoint {
        let (sx, dy) = (x * squash.width, (y - Luna.pivot) * squash.height)
        let (s, c) = (sin(turn), cos(turn))
        return CGPoint(x: origin.x + (sx * c - dy * s + shift.dx) * scale, y: origin.y + (sx * s + dy * c + Luna.pivot + shift.dy) * scale)
    }
}

@MainActor enum Luna {
    static let lume = Color(red: 0x38 / 255, green: 0xbd / 255, blue: 0xf8 / 255)
    static let gold = Color(red: 1, green: 0xca / 255, blue: 0x4a / 255)
    static let ink = Color(red: 0x17 / 255, green: 0x14 / 255, blue: 0x2b / 255)
    static let miniInk = Color(red: 0x10 / 255, green: 0x13 / 255, blue: 0x1a / 255)
    nonisolated static let red = Color(red: 0xe8 / 255, green: 0x6a / 255, blue: 0x7e / 255)
    nonisolated static let night = Color(red: 0x3a / 255, green: 0x3f / 255, blue: 0x7a / 255)
    static let amber = Color(red: 1, green: 0xb0 / 255, blue: 0x20 / 255)
    static let sweat = Color(red: 0x8f / 255, green: 0xd3 / 255, blue: 1)
    nonisolated static let pivot = 0.47
    static let middle = (LunaMotion.flameTip + 0.47) / 2
    static let fit = 1.05, miniFit = 1.08

    /// One motion per wizard on screen, by its id.
    static var motions: [String: LunaMotion] = [:]

    static func motion(_ id: String) -> LunaMotion {
        if let motion = motions[id] { return motion }
        let motion = LunaMotion(seed: UInt32(truncatingIfNeeded: id.hashValue))
        motions[id] = motion
        return motion
    }

    // the flame as drawn in the design, 100 units across, its bottom middle at (50, 93)
    static let start = CGPoint(x: 50, y: 6)
    static let curves: [[CGPoint]] = [
        [CGPoint(x: 60, y: 24), CGPoint(x: 80, y: 38), CGPoint(x: 80, y: 62)],
        [CGPoint(x: 80, y: 80), CGPoint(x: 67, y: 93), CGPoint(x: 50, y: 93)],
        [CGPoint(x: 33, y: 93), CGPoint(x: 20, y: 80), CGPoint(x: 20, y: 62)],
        [CGPoint(x: 20, y: 46), CGPoint(x: 31, y: 36), CGPoint(x: 38, y: 24)],
        [CGPoint(x: 41, y: 33), CGPoint(x: 45, y: 38), CGPoint(x: 50, y: 40)],
        [CGPoint(x: 49, y: 29), CGPoint(x: 47, y: 17), CGPoint(x: 50, y: 6)],
    ]

    private static func figure(_ point: CGPoint) -> (Double, Double) { ((point.x - 50) / 100, (point.y - 93) / 100 + pivot) }

    /// The flame's path at `time` (`draw.rs::flame`): the tip and the tongue wander, the sides breathe.
    static func flame(_ pen: LunaPen, time: Double, lively: Bool, sway: Double) -> Path {
        let (speed, reach) = lively ? (1.8, 1.4) : (1.0, 1.0)
        let wave = { (rate: Double, phase: Double) in sin(time * rate * speed + phase) * reach }
        let tip = CGVector(dx: wave(3.1, 0) * 1.6 + sway * 4, dy: wave(4.3, 1) * 2.4 - (lively ? 2 : 0))
        let tongue = CGVector(dx: wave(3.7, 1) * 1.4, dy: wave(5.1, 2) * 2)
        let (right, left) = (wave(2.3, 0) * 1.2, wave(2.3, 0.8) * 1.2)
        var curves = curves
        var start = start
        curves[0][0].x += wave(3.1, 0.6) * 1.2 + tip.dx * 0.5
        for (curve, at) in [(0, 1), (0, 2), (1, 0)] { curves[curve][at].x += right }
        for (curve, at) in [(2, 1), (2, 2), (3, 0)] { curves[curve][at].x -= left }
        curves[3][2].x += tongue.dx; curves[3][2].y += tongue.dy
        curves[4][0].x += tongue.dx * 0.5; curves[4][0].y += tongue.dy * 0.5
        curves[5][1].x += tip.dx * 0.5; curves[5][1].y += tip.dy * 0.5
        curves[5][2].x += tip.dx; curves[5][2].y += tip.dy
        start.x += tip.dx; start.y += tip.dy
        let point = { (p: CGPoint) -> CGPoint in let (x, y) = figure(p); return pen.at(x, y) }
        var path = Path()
        path.move(to: point(start))
        for curve in curves { path.addCurve(to: point(curve[2]), control1: point(curve[0]), control2: point(curve[1])) }
        path.closeSubpath()
        return path
    }

    /// How much bigger and farther apart the eyes are on a small Luna (`small_eyes`).
    static func smallEyes(_ size: Double) -> (Double, Double) {
        let small = min(max((26 - size) / 12, 0), 1)
        return (1 + 0.8 * small, 1 + 0.3 * small)
    }

    static func pill(_ pen: LunaPen, _ center: (Double, Double), _ half: (Double, Double), _ flat: CGSize) -> Path {
        let a = pen.at(center.0 - half.0 * flat.width, center.1 - half.1 * flat.height)
        let b = pen.at(center.0 + half.0 * flat.width, center.1 + half.1 * flat.height)
        let rect = CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
        return Path(roundedRect: rect, cornerRadius: min(rect.width, rect.height) / 2)
    }

    static func line(_ context: inout GraphicsContext, _ pen: LunaPen, _ points: [(Double, Double)], _ width: Double, _ ink: Color) {
        var path = Path()
        path.addLines(points.map { pen.at($0.0, $0.1) })
        context.stroke(path, with: .color(ink), style: StrokeStyle(lineWidth: max(width * pen.scale, 1), lineCap: .round, lineJoin: .round))
    }

    static func eyes(_ context: inout GraphicsContext, _ pen: LunaPen, _ motion: LunaMotion, grow: Double, spread: Double, ink: Color) {
        let radius = (LunaMotion.bodyHalf.0 * LunaMotion.bodyHalf.1).squareRoot()
        let (width, height) = (radius * 0.3 * grow, radius * 0.34 * grow)
        for (index, eye) in motion.eyes().enumerated() {
            guard let eye else { continue }
            let (x, y) = (eye.at.x * spread, eye.at.y)
            let pillOf = { (wide: Double, tall: Double) -> Path in
                let (w, t) = (width * wide, max(height * tall * motion.open.value, width * wide * 0.3))
                return pill(pen, (x, y), (w / 2, t / 2), eye.flat)
            }
            let stroke = 0.026 * grow.squareRoot()
            switch motion.look.eye {
            case .round: context.fill(pillOf(1, 1), with: .color(ink))
            case .wide: context.fill(pillOf(1.16, 1.12), with: .color(ink))
            case .focus: context.fill(pillOf(1, 0.6), with: .color(ink))
            case .happy:
                line(&context, pen, (0...8).map { step in
                    let t = Double(step) / 8
                    return (x + (t - 0.5) * 0.1 * grow, y + (0.02 - 0.05 * (1 - pow(2 * t - 1, 2))) * grow)
                }, stroke, ink)
            case .sleepy: line(&context, pen, [(x - 0.04 * grow, y + 0.01), (x + 0.04 * grow, y + 0.01)], stroke, ink)
            case .cross:
                let arm = 0.035 * grow
                line(&context, pen, [(x - arm, y - arm), (x + arm, y + arm)], stroke, ink)
                line(&context, pen, [(x + arm, y - arm), (x - arm, y + arm)], stroke, ink)
            case .spiral:
                line(&context, pen, (0...20).map { step in
                    let t = Double(step) / 20, angle = motion.spin + Double(index) * 1.5 + t * 2.5 * 2 * .pi
                    return (x + cos(angle) * 0.055 * grow * t, y + sin(angle) * 0.055 * grow * t)
                }, 0.02 * grow.squareRoot(), ink)
            }
        }
    }

    /// Luna `size` tall at `center` (`draw.rs::paint`).
    static func paint(_ context: inout GraphicsContext, center: CGPoint, size: Double, motion: LunaMotion, time: Double, color: Color = lume, dance: LunaDance? = nil) {
        guard size >= 1 else { return }
        let scale = size * fit
        let lift = dance == nil ? motion.hop : 0
        let origin = CGPoint(x: center.x, y: center.y - lift * scale - middle * scale)
        var pen = LunaPen(origin: origin, scale: scale)
        if let dance {
            (pen.squash, pen.turn, pen.shift) = (dance.scale, dance.turn * .pi / 180, CGVector(dx: dance.shift.dx * box, dy: dance.shift.dy * box))
        } else {
            pen.squash = CGSize(width: motion.squashX.value * (1 - motion.breath * 0.6), height: motion.squashY.value * (1 + motion.breath))
            pen.turn = motion.tilt.value
        }
        let ground = CGPoint(x: center.x, y: origin.y + (pivot + 0.03) * scale + lift * scale)
        let shade = min(max(1 - lift * 3, 0.4), 1)
        let (rx, ry) = (0.24 * scale * shade, 0.03 * scale * shade)
        context.fill(Path(ellipseIn: CGRect(x: ground.x - rx, y: ground.y - ry, width: 2 * rx, height: 2 * ry)), with: .color(.black.opacity(28 / 255)))
        context.fill(flame(pen, time: time, lively: dance == nil && motion.lively, sway: dance == nil ? motion.sway : 0), with: .color(color))
        if motion.tintAmount.value > 0.01 {
            context.fill(flame(pen, time: time, lively: motion.lively, sway: motion.sway), with: .color(motion.tintColor.opacity(motion.tintAmount.value)))
        }
        let (grow, spread) = smallEyes(size)
        eyes(&context, pen, motion, grow: grow, spread: spread, ink: dance == nil ? ink : miniInk)
        // no mouth at rest; speaking, a small "D" under the eyes
        let open = min(motion.mouthOpen.value / 0.03, 1)
        if open > 0.01, let mouth = motion.mouth() {
            let wide = 0.055 * mouth.flat.width, deep = (0.024 + 0.04 * min(max(motion.talk, 0), 1)) * mouth.flat.height
            var path = Path()
            path.addLines((0...16).map { step in
                let angle = Double(step) / 16 * .pi
                return pen.at(mouth.at.x + wide * cos(angle), mouth.at.y + deep * sin(angle))
            })
            path.closeSubpath()
            context.fill(path, with: .color(ink.opacity(open)))
        }
        for particle in motion.particles { paintParticle(&context, pen, particle, time) }
        if let badge = motion.badge, size >= 26, motion.badgeScale.value > 0.05 {
            let radius = max(0.085 * pen.scale, 4.5) * motion.badgeScale.value * (1 + 0.05 * sin(time * 6))
            let at = pen.at(-0.34, -0.12)
            let disc = CGRect(x: at.x - radius, y: at.y - radius, width: 2 * radius, height: 2 * radius)
            context.fill(Path(ellipseIn: disc), with: .color(badge == "…" ? Color(white: 235 / 255) : amber))
            if badge == "…" {
                for dot in 0..<3 {
                    let rise = (sin(time * 5 - Double(dot) * 0.9) * 0.5 + 0.5) * radius * 0.25
                    let r = max(radius * 0.16, 0.7), x = at.x + (Double(dot) - 1) * radius * 0.5
                    context.fill(Path(ellipseIn: CGRect(x: x - r, y: at.y - rise - r, width: 2 * r, height: 2 * r)), with: .color(Color(white: 90 / 255)))
                }
            } else {
                context.draw(Text(String(badge)).font(.system(size: radius * 1.4, weight: .bold)).foregroundColor(ink), at: at)
            }
        }
    }

    static func paintParticle(_ context: inout GraphicsContext, _ pen: LunaPen, _ particle: LunaParticle, _ time: Double) {
        let fade = min(max(1 - particle.age / particle.life, 0), 1)
        let at = pen.at(particle.at.x, particle.at.y)
        switch particle.kind {
        case .spark:
            let twinkle = 0.6 + 0.4 * sin(time * 20 + particle.spin * 9)
            let outer = 0.045 * pen.scale * (0.4 + fade) * twinkle, inner = outer * 0.38
            let turn = particle.spin * 3 + particle.age * 4
            var path = Path()
            path.addLines((0..<8).map { index in
                let angle = turn + Double(index) / 8 * 2 * .pi, r = index % 2 == 0 ? outer : inner
                return CGPoint(x: at.x + cos(angle) * r, y: at.y + sin(angle) * r)
            })
            path.closeSubpath()
            context.fill(path, with: .color(gold.opacity(fade.squareRoot())))
        case .zzz:
            let alpha = min(fade * 2, 1) * min(particle.age * 3, 1), grow = 0.7 + particle.age * 0.35
            context.draw(Text("z").font(.system(size: max(0.13 * pen.scale * grow, 1))).foregroundColor(Color(red: 0xc9 / 255, green: 0xd6 / 255, blue: 1).opacity(alpha)), at: at)
        case .sweat:
            let r = 0.025 * pen.scale
            var drop = Path(ellipseIn: CGRect(x: at.x - r, y: at.y - r, width: 2 * r, height: 2 * r))
            drop.addLines([CGPoint(x: at.x - r * 0.8, y: at.y - r * 0.4), CGPoint(x: at.x + r * 0.8, y: at.y - r * 0.4), CGPoint(x: at.x, y: at.y - r * 2.2)])
            context.fill(drop, with: .color(sweat.opacity(fade)))
        }
    }

    /// A mini one: the flame in its agent's colour and the two eyes, bigger and farther apart, glancing around.
    static func paintMini(_ context: inout GraphicsContext, center: CGPoint, body: Double, color: Color, motion: LunaMotion, time: Double) {
        guard body >= 1 else { return }
        let breath = sin(time * 2.2), scale = body * miniFit
        let pen = LunaPen(origin: CGPoint(x: center.x, y: center.y - middle * scale), scale: scale,
                      squash: CGSize(width: 1 - breath * 0.02, height: 1 + breath * 0.04))
        context.fill(flame(pen, time: time + center.x * 0.37, lively: motion.lively, sway: motion.sway), with: .color(color))
        eyes(&context, pen, motion, grow: 1.8, spread: 1.3, ink: miniInk)
    }

    // the dance's box (40 × 48), in figure units
    static let box = (0.47 - LunaMotion.flameTip) / 47
}

/// Where the dance has the body (`dance.rs::Place`): moved (box units), turned (degrees) and squashed.
struct LunaDance {
    var shift = CGVector.zero
    var turn = 0.0
    var scale = CGSize(width: 1, height: 1)

    /// The pose `phase` (0 to 1) into the beat's move, from the preview's keyframes: Balanço sways over two beats,
    /// Passinho steps, the rest hop on every beat.
    // ponytail: Sway, Step and Hop only, no arms (Palmas, Mãos pro alto, Onda, Giro hop too); port `dance.rs` whole if missed
    static func at(dance: String, beat: Double) -> LunaDance {
        let ease = { (t: Double) in t < 0.5 ? 2 * t * t : 1 - pow(-2 * t + 2, 2) / 2 }
        func lerp(_ frames: [(Double, LunaDance)], _ progress: Double) -> LunaDance {
            let next = max(frames.firstIndex { $0.0 > progress } ?? frames.count - 1, 1)
            let (fromAt, from) = frames[next - 1], (toAt, to) = frames[next]
            let t = ease(min(max((progress - fromAt) / max(toAt - fromAt, 1e-6), 0), 1))
            return LunaDance(shift: CGVector(dx: from.shift.dx + (to.shift.dx - from.shift.dx) * t, dy: from.shift.dy + (to.shift.dy - from.shift.dy) * t),
                             turn: from.turn + (to.turn - from.turn) * t,
                             scale: CGSize(width: from.scale.width + (to.scale.width - from.scale.width) * t, height: from.scale.height + (to.scale.height - from.scale.height) * t))
        }
        let p = { (x: Double, y: Double, turn: Double, sx: Double, sy: Double) in LunaDance(shift: CGVector(dx: x, dy: y), turn: turn, scale: CGSize(width: sx, height: sy)) }
        switch dance {
        case "sway", "nap":
            return lerp([(0, p(0, 0, -7, 1, 1)), (0.5, p(0, 0, 7, 1, 1)), (1, p(0, 0, -7, 1, 1))], (beat / 2).truncatingRemainder(dividingBy: 1))
        case "step":
            return lerp([(0, p(-4, 0, -4, 1, 1)), (0.25, p(0, -3, 0, 1, 1)), (0.5, p(4, 0, 4, 1, 1)), (0.75, p(0, -3, 0, 1, 1)), (1, p(-4, 0, -4, 1, 1))],
                        (beat / 2).truncatingRemainder(dividingBy: 1))
        default:
            return lerp([(0, p(0, 0, 0, 1.05, 0.95)), (0.4, p(0, -6, 0, 0.97, 1.04)), (0.75, p(0, 0, 0, 1.02, 0.98)), (1, p(0, 0, 0, 1.05, 0.95))],
                        beat.truncatingRemainder(dividingBy: 1))
        }
    }
}

/// The gaze of a wizard at `center` toward `pointer` (`gaze_toward`).
func lunaGaze(_ center: CGPoint, _ pointer: CGPoint) -> CGVector {
    CGVector(dx: tanh((pointer.x - center.x) / 260), dy: tanh((pointer.y - center.y) / 200))
}
