import AppKit
import SwiftUI

/// The glass of the bars (the reference app's `.glass`): a material with a hair of light around it.
struct Glass: ViewModifier {
    var radius: CGFloat? = nil
    func body(content: Content) -> some View {
        let shape = radius.map { AnyShape(RoundedRectangle(cornerRadius: $0, style: .continuous)) } ?? AnyShape(Capsule())
        content
            .background(.regularMaterial, in: shape)
            .overlay(shape.stroke(Color.primary.opacity(0.09), lineWidth: 1))
            .shadow(color: .black.opacity(0.10), radius: 3, y: 1)
    }
}

extension View {
    func glass(radius: CGFloat? = nil) -> some View { modifier(Glass(radius: radius)) }
}

/// A round button on glass: a faint disc under the pointer, its icon in the accent while `on`, a press that gives a little.
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

    var body: some View {
        configuration.label
            .foregroundStyle(on ? Color.accent007aff : Color.primary.opacity(0.85))
            .frame(width: size, height: size)
            .background(Circle().fill(Color.primary.opacity(hovering ? 0.10 : 0)))
            .contentShape(Circle())
            .scaleEffect(configuration.isPressed ? 0.9 : 1)
            .animation(.spring(response: 0.25, dampingFraction: 0.6), value: configuration.isPressed)
            .onHover { hovering = $0 }
    }
}

/// An icon button of the bars.
struct GlassButton: View {
    var symbol: String
    var tip: String
    var on = false
    var size: CGFloat = 30
    var badge: String? = nil
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: size * 0.5, weight: .regular))
        }
        .buttonStyle(GlassButtonStyle(on: on, size: size))
        .overlay(alignment: .bottomTrailing) {
            // its key in small at the corner, as the toolbar's
            if let badge, !badge.isEmpty {
                Text(badge).font(.system(size: 8)).foregroundStyle(.primary.opacity(0.7)).offset(x: 1, y: 2).allowsHitTesting(false)
            }
        }
        .help(tip)
    }
}

/// The hair between two groups of buttons on glass.
struct GlassDivider: View {
    var body: some View { Rectangle().fill(Color.primary.opacity(0.2)).frame(width: 1, height: 20) }
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
