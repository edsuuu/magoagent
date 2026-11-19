import SwiftUI

/// The plans' use at the sidebar's foot (`usage/foot.rs`): a line per agent with the bar of its 5-hour session, amber from 80%;
/// a click opens the card with every window it read and the Maestro's ceiling.
struct UsageFoot: View {
    let side: SidebarState

    var body: some View {
        VStack(spacing: 2) {
            ForEach(side.usage) { line in
                Button { appAction("sidebar", ["action": side.usageOpen ? "usage_close" : "usage_open"]) } label: {
                    // the text centred 11 down, the bar from 21 to 25 (`usage_foot`)
                    VStack(spacing: 0) {
                        HStack {
                            Text(line.title).font(.app(12)).foregroundStyle(Tones.text).lineLimit(1)
                            Spacer()
                            Text(percent(line.used)).font(.app(12)).foregroundStyle(Tones.muted)
                        }
                        .frame(height: 22)
                        UsageBar(used: line.used, ground: Tones.active).padding(.top, -1)
                    }
                    .padding(.horizontal, 8).frame(height: 32, alignment: .top)
                }
                .buttonStyle(RowStyle(current: side.usageOpen, radius: 7))
                .help(side.usageOpen ? "" : "Uso dos agentes")
            }
        }
        // LINE 34 a line (32 and the gap), and 8 around them
        .padding(.horizontal, 5).padding(.top, 4).padding(.bottom, 6)
        .popover(isPresented: Binding(get: { side.usageOpen }, set: { if !$0 { appAction("sidebar", ["action": "usage_close"]) } }), arrowEdge: .top) {
            UsageCard(side: side)
        }
    }
}

func percent(_ used: Double) -> String { "\(Int((used * 100).rounded()))%" }

/// A bar `used` (0 to 1) full: the accent, amber near the end.
struct UsageBar: View {
    let used: Double
    let ground: Color

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2).fill(ground)
                RoundedRectangle(cornerRadius: 2).fill(used >= 0.8 ? Color.amber : Color.glassAccent)
                    .frame(width: proxy.size.width * min(max(used, 0), 1))
            }
        }
        .frame(height: 4)
    }
}

/// The card: each agent's windows and when they renew, the Maestro's ceiling, Atualizar and the way to Ajustes.
private struct UsageCard: View {
    let side: SidebarState

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("Uso dos agentes").font(.app(13)).foregroundStyle(MenuLook.text)
                Spacer()
                Button { appAction("usage_refresh") } label: { Image(systemName: "arrow.clockwise").font(.system(size: 11)) }
                    .buttonStyle(.plain).foregroundStyle(MenuLook.text).help("Atualizar")
            }
            .frame(height: 22)
            ForEach(side.usage) { line in
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(line.agent).font(.app(13)).foregroundStyle(MenuLook.text)
                        Spacer()
                        if let plan = line.plan { Text(plan).font(.app(12)).foregroundStyle(MenuLook.muted) }
                    }
                    ForEach(line.meters, id: \.name) { meter in
                        HStack(spacing: 4) {
                            Text(meter.name).font(.app(12)).foregroundStyle(MenuLook.text).lineLimit(1).frame(width: 68, alignment: .leading)
                            UsageBar(used: meter.used, ground: MenuLook.hover.opacity(2))
                            Text(percent(meter.used)).font(.app(12)).foregroundStyle(MenuLook.muted).frame(width: 38, alignment: .trailing)
                        }
                        .frame(height: 16)
                    }
                    if let note = line.note { Text(note).font(.app(12)).foregroundStyle(MenuLook.muted).fixedSize(horizontal: false, vertical: true) }
                }
                .padding(.top, 6)
            }
            Text(side.maestro).font(.app(12)).foregroundStyle(MenuLook.muted).fixedSize(horizontal: false, vertical: true).padding(.top, 6)
            Button("Configurações…") {
                appAction("sidebar", ["action": "usage_close"])
                shellVerb("settings", ["tab": "Agentes"])
            }
            .buttonStyle(.plain).font(.app(12)).foregroundStyle(Color.glassAccent)
        }
        .padding(10).frame(width: 240)
        .background(MenuLook.ground)
    }
}
