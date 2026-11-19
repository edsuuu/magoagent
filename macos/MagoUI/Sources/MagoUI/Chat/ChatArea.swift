// The chat in SwiftUI: `MagoViews.chat` is one conversation by its id (the chat's card on the canvas), `MagoViews.chats`
// the Chats screen (the list beside the conversation open). All the logic is Rust's (src/app/chat/native/): what is
// clicked goes as an action of the MCP layer or as `ui.chat.<verb>`.
import SwiftUI

@MainActor public enum ChatArea {
    public static func start() {
        MagoViews.chat = { id in AnyView(ChatView(id: id)) }
        MagoViews.chats = { AnyView(ChatsScreen()) }
    }
}

/// The Chats screen (chat/mod.rs `chat_screen`): the list 320 wide at the left, 16 from the edges, the conversation open
/// beside it; « folds the list and » brings it back.
struct ChatsScreen: View {
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look
    @State private var folded = false

    var body: some View {
        let list = store.decode(ChatsSnapshot.self, topic: "chats")
        HStack(alignment: .top, spacing: 16) {
            if !folded {
                ChatList(list: list, folded: $folded)
                    .frame(width: 320)
                    .transition(.move(edge: .leading).combined(with: .opacity))
            }
            Group {
                if let current = list?.current {
                    ChatView(id: current)
                } else {
                    Text("Nenhuma conversa").font(.app(14)).foregroundStyle(look.muted).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            // folded, the header starts past the window's buttons, with » to bring the list back
            .padding(.top, folded ? 40 : 0)
            .overlay(alignment: .topLeading) {
                if folded {
                    Button { withAnimation(.smooth(duration: 0.25)) { folded = false } } label: {
                        Image(systemName: "chevron.right.2").font(.system(size: 14, weight: .medium)).foregroundStyle(look.soft).frame(width: 28, height: 28)
                    }
                    .buttonStyle(HoverGround(radius: 7, color: look.hover))
                    .help("Expandir lista (⌘B)")
                    .padding(.leading, 170)
                    .padding(.top, 6)
                }
            }
        }
        .padding(.leading, 16)
        .padding(.trailing, 16)
        .padding(.top, 12)
        .padding(.bottom, 16)
        .background(look.bg)
    }
}
