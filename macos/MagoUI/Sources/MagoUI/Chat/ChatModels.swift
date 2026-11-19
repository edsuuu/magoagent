// The chat's snapshots as Rust publishes them (src/app/chat/native/): "chats" for the list (list.rs), "chat:<id>" for one
// conversation (snapshot.rs, words.rs). Keys arrive snake_case: `MagoStore.decode` reads them as they are, so the few
// with an underscore are named in CodingKeys.
import Foundation

struct ChatsSnapshot: Decodable, Equatable {
    let open: Bool
    let current: String?
    let show: String
    let filter: String
    let counts: [Int]
    let running: Int
    let waiting: Int
    /// A chip or a search picks what shows: the teams show open, with no fold.
    let looking: Bool
    let groups: [ChatGroup]
    /// Exibição (list/display.rs): one line a row, grouped by space, the order (created, recent, name) and whose chats
    /// (all, claude, codex).
    let compact: Bool
    let bySpace: Bool
    let order: String
    let agentFilter: String

    enum CodingKeys: String, CodingKey {
        case open, current, show, filter, counts, running, waiting, looking, groups, compact, order
        case bySpace = "by_space"
        case agentFilter = "agent_filter"
    }
}

struct ChatGroup: Decodable, Equatable, Identifiable {
    let name: String
    let color: [Int]
    let entries: [ChatEntry]
    var id: String { name }
}

struct ChatEntry: Decodable, Equatable, Identifiable {
    let id: String
    let name: String
    let state: String
    /// Its mini wizard's state (LunaState's names).
    let mago: String
    let ended: Bool
    let last: String
    let ago: String
    let color: [Int]
    let chat: Bool
    let answers: [ChatAnswer]
    let team: [ChatEntry]
    let teamSummary: String?
    let teamOpen: Bool

    enum CodingKeys: String, CodingKey {
        case id, name, state, mago, ended, last, ago, color, chat, answers, team
        case teamSummary = "team_summary"
        case teamOpen = "team_open"
    }
}

/// A button that answers a row right there: ["Permitir", true] (its label, and whether it is the main one).
struct ChatAnswer: Decodable, Equatable {
    let label: String
    let main: Bool

    init(from decoder: Decoder) throws {
        var pair = try decoder.unkeyedContainer()
        label = try pair.decode(String.self)
        main = try pair.decode(Bool.self)
    }
}

struct ChatSnapshot: Decodable, Equatable {
    let id: String
    let title: String
    /// The agent's colour: its wizard over an empty conversation.
    let color: [Int]
    /// The context ring (a Claude Code chat) and the share of the window in use, nil before any reading.
    let ring: Bool
    let context: Double?
    let agent: String
    let folder: String
    let claude: Bool
    let state: String
    let status: String
    let isChat: Bool
    let fresh: Bool
    let providers: [ChatProvider]
    let earlier: Int
    let items: [ChatItem]
    let band: ChatBand?
    let ask: ChatAsk?
    let waiting: ChatWaiting?
    let queue: [String]
    let team: [String]
    let draft: String
    let draftSeq: UInt64
    let hint: String
    let slash: [SlashEntry]
    let chips: ChatChips?
    /// Buscar na conversa: the words searched ("" not searching).
    let query: String
    /// The pictures that go with the next message: [where it came from, its path].
    let attached: [[String]]
    let attachedTotal: String

    enum CodingKeys: String, CodingKey {
        case id, title, color, ring, context, agent, folder, claude, state, status, fresh, providers, earlier, items, band, ask, waiting, queue, team, draft, hint, slash, chips, query,
            attached
        case isChat = "is_chat"
        case draftSeq = "draft_seq"
        case attachedTotal = "attached_total"
    }
}

struct ChatProvider: Decodable, Equatable, Identifiable {
    let name: String
    let program: String
    let on: Bool
    var id: String { program }
}

/// A message (user, agent, system, divider, error, image) or a run of tool steps (tools).
struct ChatItem: Decodable, Equatable, Identifiable {
    let id: String
    let kind: String
    let text: String?
    let parts: [ChatPart]?
    let steps: [ChatStep]?
    let summary: String?
    let folded: Bool
    let media: [ChatPart]?
    let from: String?
    let hint: String?
}

/// A piece of the agent's words: text (paragraphs and tables), code, image, web or file.
struct ChatPart: Decodable, Equatable {
    let kind: String
    let text: String?
    let info: String?
    let detail: String?
    let opens: String?
    let paras: [ChatPara]?
}

/// A paragraph (its lines, a bubble) or a table (its rows of cells, the first the header).
struct ChatPara: Decodable, Equatable {
    let lines: [ChatLine]?
    let rows: [[[ChatRun]]]?
}

/// text, heading (level 1 to 3), quote or item (its mark; level is its indentation).
struct ChatLine: Decodable, Equatable {
    let kind: String
    let level: Int?
    let mark: String?
    let runs: [ChatRun]
}

struct ChatRun: Decodable, Equatable {
    let text: String
    let bold: Bool?
    let italic: Bool?
    let code: Bool?
    /// A file it names: a click opens it in the Árvore.
    let path: String?
    let line: Int?
}

struct ChatStep: Decodable, Equatable, Identifiable {
    let id: String
    let name: String
    let what: String
    let note: String
    let failed: Bool
    /// done, running or waiting.
    let live: String
    let ended: Bool
    let diff: [Int]?
    let output: String
    let head: String
    let files: [String]
    let inputLabel: String
    let input: String
    let log: String
    let logCut: Bool
    let error: String

    enum CodingKeys: String, CodingKey {
        case id, name, what, note, failed, live, ended, diff, output, head, files, input, log, error
        case inputLabel = "input_label"
        case logCut = "log_cut"
    }
}

struct ChatBand: Decodable, Equatable {
    let tool: String
    let what: String
    let counter: [Int]?
    let thinking: String
    let seconds: Int
}

struct ChatAsk: Decodable, Equatable {
    let head: String
    let tool: String
    let summary: String
    let outside: Bool
    let questions: [ChatQuestion]
    let step: Int
    let picked: [[Int]]
    let other: [String]
    let complete: Bool
}

struct ChatQuestion: Decodable, Equatable, Identifiable {
    let text: String
    let header: String
    let multiple: Bool
    let options: [[String]]
    var id: String { text }
}

/// A terminal stopped on a question only its screen answers.
struct ChatWaiting: Decodable, Equatable {
    let head: String
    let why: String
    let question: String
    let choices: [String]
    let marked: Int
    let screen: String
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

extension Array {
    subscript(safe index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}
