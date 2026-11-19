import SwiftUI

/// A browser's card below its title strip, as egui paints it (src/app/browser/paint.rs and chrome.rs), in the card's own
/// points (the canvas scales it): the dark body, the bar on top, the note where the page goes. The page lies over this, in
/// `PageHostView`.
struct PortalBody: View {
    let id: String

    var body: some View {
        let card = PortalsTopic.current?.cards[id]
        let unloaded = card?.unloaded == true
        ZStack(alignment: .top) {
            VStack(spacing: 4) {
                Text(unloaded ? "Descarregado" : "Carregando…").font(.app(13))
                if unloaded {
                    Text("Parado para economizar recursos").font(.app(11.5))
                }
            }
            .foregroundStyle(PortalBar.muted)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.top, card?.hidden == false ? PortalBar.height : 0)
            .background(PortalBar.ground)
            .allowsHitTesting(false)
            if let card, card.bar {
                PortalBar(id: id, card: card)
            }
        }
    }
}

/// The bar over the page: back, forward and reload (× while loading) in one pill, the address (a field on a click) and
/// Design, which egui keeps in the bar over the canvas.
struct PortalBar: View {
    static let height: CGFloat = 40
    static let ground = Color(white: 0x24 / 255)
    static let light = Color(white: 0xd0 / 255)
    static let muted = Color(white: 0x8a / 255)
    static let pill = Color.white.opacity(15 / 255)

    let id: String
    let card: PortalCard
    @State private var editing = false
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 6) {
            HStack(spacing: 2) {
                BarButton(tip: "Voltar") { verb("back") } label: { Image(systemName: "arrow.left") }
                BarButton(tip: "Avançar") { verb("forward") } label: { Image(systemName: "arrow.right") }
                BarButton(tip: card.loading ? "Parar" : "Recarregar") { verb(card.loading ? "stop" : "reload") } label: {
                    if card.loading {
                        Text("×").font(.app(14))
                    } else {
                        Image(systemName: "arrow.clockwise").font(.system(size: 10, weight: .semibold))
                    }
                }
            }
            .padding(.horizontal, 4)
            .frame(height: 28)
            .background(Self.pill, in: Capsule())
            address
            BarButton(tip: card.design ? "Sair do Design" : "Design", on: card.design) { verb("design") } label: {
                Image(systemName: "paintbrush.pointed")
            }
        }
        .padding(.horizontal, 8)
        .frame(height: Self.height)
        .background(Self.ground)
    }

    /// The address in a pill with a lens; a click turns it into a field with all of it selected.
    private var address: some View {
        ZStack(alignment: .leading) {
            Image(systemName: "magnifyingglass").font(.system(size: 10, weight: .medium)).foregroundStyle(Self.muted).frame(width: 32)
            Group {
                if editing {
                    TextField("Endereço", text: $text, prompt: Text("Digite uma URL…").foregroundStyle(Self.muted))
                        .textFieldStyle(.plain)
                        .foregroundStyle(Self.light)
                        .focused($focused)
                        .onAppear { focused = true }
                        .onSubmit {
                            verb("navigate", ["url": text])
                            editing = false
                        }
                        .onExitCommand { editing = false }
                        .onChange(of: focused) { _, now in
                            if !now { editing = false }
                        }
                } else {
                    Text(card.address.isEmpty ? "Digite uma URL…" : card.address)
                        .foregroundStyle(card.address.isEmpty ? Self.muted : Self.light)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
            }
            .font(.custom("Hack", size: 12))
            .padding(.leading, 28)
            .padding(.trailing, 10)
        }
        .frame(maxWidth: .infinity, minHeight: 28, maxHeight: 28, alignment: .leading)
        .background(Self.pill, in: Capsule())
        .contentShape(Capsule())
        .onTapGesture {
            guard !editing else { return }
            text = card.url
            editing = true
        }
    }

    private func verb(_ name: String, _ args: [String: Any] = [:]) {
        browserVerb(name, args.merging(["portal": id]) { mine, _ in mine })
    }
}

/// One of the bar's round buttons: 24 points, a faint disc under the pointer, the accent while `on`.
private struct BarButton<Label: View>: View {
    let tip: String
    var on = false
    let action: () -> Void
    @ViewBuilder let label: Label
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            label
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(on ? canvasPrimary : PortalBar.light)
                .frame(width: 24, height: 24)
                .background(Circle().fill(Color.white.opacity(hovering ? 26 / 255 : 0)))
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .help(tip)
        .accessibilityLabel(tip)
        .onHover { hovering = $0 }
    }
}
