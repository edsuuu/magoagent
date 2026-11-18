// The chat's snapshots as Rust publishes them (src/app/chat/native.rs): "chats" for the list, "chat:<id>" for one
// conversation. Keys arrive snake_case: the few with an underscore are named in CodingKeys.
import Foundation

struct ChatsSnapshot: Decodable, Equatable {
    let open: Bool
    let current: String?
    let show: String
    let filter: String
    let counts: [Int]
    let running: Int
    let waiting: Int
    let groups: [ChatGroup]
}

struct ChatGroup: Decodable, Equatable, Identifiable {
    let name: String
    let entries: [ChatEntry]
    var id: String { name }
}

struct ChatEntry: Decodable, Equatable, Identifiable {
    let id: String
    let name: String
    let state: String
    let last: String
    let ago: String
    let color: [Int]
    let chat: Bool
}

struct ChatSnapshot: Decodable, Equatable {
    let id: String
    let title: String
    let agent: String
    let folder: String
    let state: String
    let status: String
    let isChat: Bool
    let fresh: Bool
    let providers: [ChatProvider]
    let earlier: Int
    let items: [ChatItem]
    let band: ChatBand?
    let ask: ChatAsk?
    let queue: [String]
    let draft: String
    let draftSeq: UInt64
    let hint: String
    let slash: [SlashEntry]
    let chips: ChatChips?

    enum CodingKeys: String, CodingKey {
        case id, title, agent, folder, state, status, fresh, providers, earlier, items, band, ask, queue, draft, hint, slash, chips
        case isChat = "is_chat"
        case draftSeq = "draft_seq"
    }
}

struct ChatProvider: Decodable, Equatable, Identifiable {
    let name: String
    let program: String
    let on: Bool
    var id: String { program }
}

struct ChatItem: Decodable, Equatable, Identifiable {
    let id: String
    let kind: String
    let text: String?
    let parts: [ChatPart]?
    let steps: [ChatStep]?
    let summary: String?
    let folded: Bool
    let from: String?
    let hint: String?
}

struct ChatPart: Decodable, Equatable {
    let kind: String
    let text: String
    let info: String?
}

struct ChatStep: Decodable, Equatable, Identifiable {
    let id: String
    let name: String
    let what: String
    let note: String
    let failed: Bool
    let running: Bool
    let diff: [Int]?
    let input: String
    let output: String
    let error: String
}

struct ChatBand: Decodable, Equatable {
    let tool: String
    let what: String
    let counter: [Int]?
    let thinking: String
    let seconds: Int
}

struct ChatAsk: Decodable, Equatable {
    let tool: String
    let questions: [ChatQuestion]
}

struct ChatQuestion: Decodable, Equatable, Identifiable {
    let text: String
    let header: String
    let multiple: Bool
    let options: [[String]]
    var id: String { text }
}

struct SlashEntry: Decodable, Equatable, Identifiable {
    let name: String
    let args: String
    let desc: String
    var id: String { name }
}

struct ChatChips: Decodable, Equatable {
    let folder: String
    let folderPath: String
    let folderLocked: Bool
    let noFolder: String
    let recent: [[String]]
    let mode: Int
    let modes: [String]
    let tuning: String
    let model: String
    let models: [ChatChoice]
    let more: [ChatChoice]
    let effort: String
    let efforts: [ChatChoice]

    enum CodingKeys: String, CodingKey {
        case folder, recent, mode, modes, tuning, model, models, more, effort, efforts
        case folderPath = "folder_path"
        case folderLocked = "folder_locked"
        case noFolder = "no_folder"
    }
}

struct ChatChoice: Decodable, Equatable, Identifiable {
    let id: String
    let name: String
    let note: String
}
