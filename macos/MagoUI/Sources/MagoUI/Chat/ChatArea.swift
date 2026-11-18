// The chat in SwiftUI: `MagoViews.chat` is one conversation by its id, `MagoViews.chats` the Chats screen (the list beside
// the conversation open). All the logic is Rust's (src/app/chat/native.rs): what is clicked goes as an action of the MCP
// layer or as `ui.chat.<verb>`.
import SwiftUI

@MainActor public enum ChatArea {
    public static func start() {
        MagoViews.chat = { id in AnyView(ChatView(id: id)) }
        MagoViews.chats = { AnyView(ChatsScreen()) }
    }
}

/// The colour of a state: working blue, waiting amber, ready green (list/running.rs `chip`).
func stateColor(_ state: String) -> Color {
    switch state {
    case "working": .blue
    case "waiting": .orange
    default: .green
    }
}

/// The Chats screen: the list at the left, the conversation open at the right.
struct ChatsScreen: View {
    private let store = MagoStore.shared

    var body: some View {
        let list = store.decode(ChatsSnapshot.self, topic: "chats")
        HSplitView {
            ChatList(list: list)
                .frame(minWidth: 220, idealWidth: 280, maxWidth: 380)
            if let current = list?.current {
                ChatView(id: current)
                    .frame(minWidth: 360)
            } else {
                ContentUnavailableView("Nenhuma conversa", systemImage: "bubble.left.and.bubble.right", description: Text("Comece uma em Nova conversa."))
                    .frame(minWidth: 360)
            }
        }
    }
}
