import AppKit
import SwiftUI

extension Color {
    /// A color that follows the window's appearance: `dark` under Dark Aqua, `light` otherwise.
    static func themed(dark: Int, light: Int, darkAlpha: Double = 1, lightAlpha: Double = 1) -> Color {
        func rgb(_ hex: Int, _ alpha: Double) -> NSColor {
            NSColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: alpha)
        }
        return Color(nsColor: NSColor(name: nil) { appearance in
            appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? rgb(dark, darkAlpha) : rgb(light, lightAlpha)
        })
    }

    /// The ink on glass (`glass_ink`): nearly white in the dark, nearly black in the light.
    static let glassInk = themed(dark: 0xf5f5f7, light: 0x1d1d1f)
    /// The accent on glass (`glass_accent`): the tool in use, the open panel.
    static let glassAccent = themed(dark: 0x0a84ff, light: 0x007aff)
}

/// The glass of the bars (`paint_glass`): the bars' ground fading down it (96% at the top, 42% at the foot) over the blur,
/// a hair of light around it, and the lift only in the light theme.
struct Glass: ViewModifier {
    var radius: CGFloat? = nil
    var strong = false
    private static let ground = Color.themed(dark: 0x2c2c2e, light: 0xffffff)
    private static let rim = Color.themed(dark: 0xffffff, light: 0x000000, darkAlpha: 0x24 / 255, lightAlpha: 14 / 255)
    private static let lift = Color.themed(dark: 0x000000, light: 0x000000, darkAlpha: 0, lightAlpha: 0x1a / 255)
    private static let far = Color.themed(dark: 0x000000, light: 0x000000, darkAlpha: 0, lightAlpha: 0x14 / 255)

    func body(content: Content) -> some View {
        let shape = radius.map { AnyShape(RoundedRectangle(cornerRadius: $0, style: .continuous)) } ?? AnyShape(Capsule())
        let fade = strong
            ? Gradient(stops: [.init(color: Self.ground.opacity(0.78), location: 0), .init(color: Self.ground.opacity(0.78), location: 1)])
            : Gradient(stops: [
                .init(color: Self.ground.opacity(0.96), location: 0), .init(color: Self.ground.opacity(0.90), location: 0.48),
                .init(color: Self.ground.opacity(0.74), location: 0.76), .init(color: Self.ground.opacity(0.42), location: 1),
            ])
        content
            .background(LinearGradient(gradient: fade, startPoint: .top, endPoint: .bottom), in: shape)
            .background(.ultraThinMaterial, in: shape)
            .overlay(shape.stroke(Self.rim, lineWidth: 1))
            .shadow(color: Self.lift, radius: 1, y: 1)
            .shadow(color: Self.far, radius: 10, y: 6)
    }
}

extension View {
    func glass(radius: CGFloat? = nil, strong: Bool = false) -> some View { modifier(Glass(radius: radius, strong: strong)) }
}

/// A round button on glass (`glass_button_sized`): a faint disc under the pointer, its icon in the accent while `on`, a swell
/// when pressed.
struct GlassButtonStyle: ButtonStyle {
    var on = false
    var size: CGFloat = 30

    func makeBody(configuration: Configuration) -> some View {
        GlassLabel(configuration: configuration, on: on, size: size)
    }
}

private struct GlassLabel: View {
    let configuration: ButtonStyleConfiguration
    let on: Bool
    let size: CGFloat
    @State private var hovering = false
    @Environment(\.isEnabled) private var enabled

    var body: some View {
        configuration.label
            .foregroundStyle(on ? Color.glassAccent : Color.glassInk.opacity(0.85))
            .frame(width: size, height: size)
            .background(Circle().fill(Color.glassInk.opacity(hovering && enabled ? 0.10 : 0)))
            .contentShape(Circle())
            .scaleEffect(configuration.isPressed ? 1.12 : 1)
            .opacity(enabled ? 1 : 0.4)
            .animation(.spring(response: 0.25, dampingFraction: 0.6), value: configuration.isPressed)
            .onHover { hovering = $0 }
    }
}

/// An icon button of the bars: the icon at 1.05 of its 16 points in a 30 button, and in proportion in the others.
struct GlassButton: View {
    var icon: MagoIcon
    var tip: String
    var on = false
    var size: CGFloat = 30
    /// The icon's size against its 16 points, when not the button's own.
    var scale: CGFloat? = nil
    var badge: String? = nil
    var action: () -> Void

    var body: some View {
        Button(action: action) { IconView(icon: icon, scale: scale ?? 1.05 * size / 30) }
            .buttonStyle(GlassButtonStyle(on: on, size: size))
            .overlay(alignment: .bottomTrailing) {
                // its key in small at the corner, as the toolbar's
                if let badge, !badge.isEmpty {
                    Text(badge).font(.app(8)).foregroundStyle(Color.glassInk.opacity(0.7)).offset(x: 1, y: 2).allowsHitTesting(false)
                }
            }
            .help(tip)
            .accessibilityLabel(tip)
    }
}

/// The hair between two groups of buttons on glass.
struct GlassDivider: View {
    var body: some View { Rectangle().fill(Color.glassInk.opacity(0.2)).frame(width: 1, height: 20) }
}

/// A row of glass: its things 5 apart, 6.5 in from the ends.
struct GlassRow<Content: View>: View {
    var height: CGFloat = 40
    @ViewBuilder var content: Content
    var body: some View {
        HStack(spacing: 5) { content }
            .padding(.horizontal, 6.5)
            .frame(height: height)
            .glass()
    }
}

/// The sidebar's backdrop: the system's own sidebar material, which sees through to the desktop.
struct SidebarBackground: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = .sidebar
        view.blendingMode = .behindWindow
        view.state = .followsWindowActiveState
        return view
    }

    func updateNSView(_ view: NSVisualEffectView, context: Context) {}
}
