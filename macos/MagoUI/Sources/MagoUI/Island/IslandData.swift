// What Rust publishes for the island (`src/app/island/native.rs`): topic "island" and, for what moves every frame,
// "island.live". The keys are camelCase on both sides.

import Foundation

struct IslandSnapshot: Decodable, Equatable {
    var on = false
    var mood = "empty"
    var count = 0
    var state = "idle"
    var celebrating = false
    var finished = false
    var lines: [IslandLine] = []
    var feed: [String] = []
    var hidden = 0
    var tabs: [IslandTab] = []
    var tab = "home"
    var settings = false
    var focus: String?
    var pinned = false
    var ask: IslandAsk?
    var activity: IslandActivity?
    var glimpse: [IslandSaid] = []
    var chat: IslandChat?
    var voice = false
    /// The call is on in the code (`voice::CALL_ON`): off, the header has no Chat por voz, only one still on to end.
    var callOn = false
    var voiceState = "off"
    var voiceNote = ""
    var sound = true
    var volume: Double = 0.1
    var closeAfter = 15
    var compactAfter = 60
    var color = "#000000"
    var claudeHooks = false
    var song: IslandSong?
    var lunaDances = false
    var lunaColor = "#38bdf8"
    var github: IslandGithub?
    var projects: [String] = []
    var at: [Double]?
    var talkSize: [Double]?
    var seq: UInt64 = 0
    var forced: String?
    var place: String?
}

struct IslandLine: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
    var detail: String
    var state: String
    var mago: String
    var color: String
    var chat: Bool
    var canvas: Bool
    var badge: String?
    var kind: String
    var at: Int64
    var asleep: Bool
    var tab: String
    var claude: Bool
    var outside: Bool
    var tool: Bool
    var chips: [[String]]

    /// Who needs the person first, then what failed, what works, what just ended, the rest (`feed.rs::rank`).
    var rank: Int {
        if badge == "approval" || state == "waiting" { return 0 }
        if badge == "error" || mago == "error" { return 1 }
        if state == "working" { return 2 }
        if badge == "finished" || mago == "done" { return 3 }
        return 4
    }
}

struct IslandTab: Decodable, Equatable {
    var tab: String
    var tip: String
}

struct IslandAsk: Decodable, Equatable {
    var chat: String
    var name: String?
    var color: String?
    var approval: Bool
    var doing: String
    var summary: String
    var details: String
    var outside: Bool
    var queued: Int
    var step: Int
    var questions: [IslandQuestion]
    var picked: [[Int]]
    var other: [String]
    var open = false
}

struct IslandQuestion: Decodable, Equatable {
    var text: String
    var header: String
    var multiple: Bool
    var options: [[String]]
}

struct IslandActivity: Decodable, Equatable {
    var phase: String
    var verb: String
    var target: String
    var file: String?
    var place: [Int]
    var moves: [IslandStep]
    var patch: [String]
    var patchLine: Int
    var diff: [Int]?
    var output: [String]
    var headline: String?
}

struct IslandStep: Decodable, Equatable {
    var verb: String
    var running: Bool
    var failed: Bool
}

struct IslandSaid: Decodable, Equatable {
    var person: Bool
    var number: Int
    var text: String
}

struct IslandChat: Decodable, Equatable {
    var id: String
    var name: String
    var working: Bool
    var session: String?
    var said: [IslandSaid]
    /// A chat's own chips (folder, permission, model and effort), the big chat's; none for a terminal.
    var chips: ChatChips?
}

struct IslandSong: Decodable, Equatable {
    var title: String
    var artist: String
    var album: String
    var coverUrl: String
    var playing: Bool
    var shuffle: Bool?
    var `repeat`: String?
    var volume: Double?
    var positionMs: UInt64?
    var durationMs: UInt64?
    var readAtMs: Int64
    var liked: Bool?
    var device: String
    var trackUri: String
    var tint: String?
    /// The bars' two colours, left to right (`Spotify::colours`); none until the cover comes.
    var colours: [String]?

    /// Where in the song it is now: what the player said, gone on while it plays (`Now::position_at`).
    func position(now: Date = Date()) -> Double? {
        guard let position = positionMs else { return nil }
        let gone = playing && readAtMs > 0 ? max(0, now.timeIntervalSince1970 * 1000 - Double(readAtMs)) : 0
        let at = Double(position) + gone
        return durationMs.map { min(at, Double($0)) } ?? at
    }
}

struct IslandGithub: Decodable, Equatable {
    var login: String
    var error: String?
    var prs: [IslandPull]
}

struct IslandPull: Decodable, Equatable, Identifiable {
    var id: String
    var title: String
    var url: String
    var page: String
    var draft: Bool
    var ci: String
    var state: String
    var review: String
    var headRef: String
    var baseRef: String
    var updatedAt: Int64
    var conflict: Bool
    var needsYou: Bool
    var mine: Bool
    var failing: [String]
}

struct IslandLive: Decodable, Equatable {
    var mic: Double = 0
    var bpm: Double = 0
    var phase: Double = 0
    var beat: Double = 0
    var energy: Double = 0
    var bass: Double = 0
    var dance = ""
    var bands: [Double]?
}
