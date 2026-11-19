import SwiftUI

/// The app's menus (`ui/menu_look.rs`): a box in the popover's ground with a hairline border, rows 30 tall with a 15-point
/// icon, the text at 13.
enum MenuLook {
    static let ground = Color.themed(dark: 0x2c2c2e, light: 0xffffff)
    static let border = Color.themed(dark: 0x545458, light: 0x000000, darkAlpha: 0x80 / 255, lightAlpha: 0x1f / 255)
    static let text = Color.themed(dark: 0xf5f5f7, light: 0x1d1d1f)
    static let muted = Color.themed(dark: 0xebebf5, light: 0x3c3c43, darkAlpha: 0x99 / 255, lightAlpha: 0x99 / 255)
    static let hover = Color.themed(dark: 0xffffff, light: 0x000000, darkAlpha: 0x1a / 255, lightAlpha: 0x14 / 255)
}

/// A row of a menu: its icon, its label, in the accent while `current`.
struct MenuRow: View {
    let icon: MagoIcon?
    let label: String
    var current = false
    var danger = false
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let icon { IconView(icon: icon, scale: 15.0 / 16).frame(width: 15) }
                Text(label).font(.app(13)).lineLimit(1)
                Spacer(minLength: 0)
            }
            .foregroundStyle(current ? Color.glassAccent : danger ? Color.red : MenuLook.text)
            .padding(.horizontal, 8).frame(height: 30)
            .background(RoundedRectangle(cornerRadius: 4).fill(hovering ? MenuLook.hover : .clear))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
    }
}

extension View {
    /// The menu's box around rows.
    func menuBox(width: CGFloat) -> some View {
        VStack(spacing: 1) { self }
            .padding(3.25).frame(width: width)
            .background(MenuLook.ground)
    }
}
