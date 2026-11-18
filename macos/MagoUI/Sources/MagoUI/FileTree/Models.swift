import Foundation

// Mirrors of the Rust snapshots (src/app/filetree/native). Rust writes camelCase keys, so the default decoder reads them as they are.

struct FTIconRef: Codable, Hashable {
    var key: String?
    var label: String?
    var color: String?
}

struct FTRow: Codable, Identifiable, Hashable {
    var path: String
    var name: String
    var dir: Bool
    var depth: Int
    var open: Bool
    var hint: String
    var icon: FTIconRef
    var color: String?
    var letter: String?
    var letterColor: String?
    var ignored: Bool
    var problem: Bool
    var id: String { path }
}

struct FTGit: Codable, Hashable {
    var repo: Bool
    var branch: String
    var changes: Int
}

struct FTClip: Codable, Hashable {
    var count: Int
    var cut: Bool
}

struct FTSort: Codable, Hashable {
    var key: String
    var label: String
}

struct FTChange: Codable, Identifiable, Hashable {
    var path: String
    var name: String
    var dir: String
    var letter: String
    var color: String
    var icon: FTIconRef
    var id: String { path }
}

struct FTTab: Codable, Identifiable, Hashable {
    var path: String
    var name: String
    var icon: FTIconRef
    var dirty: Bool
    var preview: Bool
    var kind: String
    var id: String { path }
}

struct FTProblem: Codable, Hashable {
    var line: Int
    var col: Int
    var severity: Int
    var message: String
}

struct FTProblemFile: Codable, Identifiable, Hashable {
    var path: String
    var rel: String
    var problems: [FTProblem]
    var id: String { path }
}

/// `filetree:<id>`
struct FTState: Codable {
    var id: String
    var name: String
    var root: String
    var shown: String
    var dark: Bool
    var canBack: Bool
    var canForward: Bool
    var git: FTGit
    var filter: String
    var searching: Bool
    var treeRows: Int
    var hitsCut: Bool
    var dropped: Int
    var emptyNote: String?
    var rows: [FTRow]
    var selected: String?
    var clipboard: FTClip?
    var hideHidden: Bool
    var sort: String
    var sorts: [FTSort]
    var reversed: Bool
    var editorOpen: Bool
    var changesOpen: Bool
    var treeWidth: Double?
    var branches: [String]
    var changes: [FTChange]
    var tabs: [FTTab]
    var active: String?
    var wrap: Bool
    var findOpen: Bool
    var problemsOpen: Bool
    var problems: [FTProblemFile]
    var errorCount: Int
    var warningCount: Int
}

struct FTCompareRow: Codable, Hashable {
    var kind: String
    var leftNumber: Int?
    var rightNumber: Int?
    var left: String?
    var right: String?
    var leftWords: [[Int]]
    var rightWords: [[Int]]
    var foldKey: Int?
    var foldCount: Int?
}

struct FTCompare: Codable {
    var file: String
    var head: String
    var unified: Bool
    var collapse: Bool
    var current: Int
    var blocks: [Int]
    var removed: Int
    var added: Int
    var rows: [FTCompareRow]
}

struct FTHit: Codable, Identifiable, Hashable {
    var path: String
    var rel: String
    var name: String
    var line: Int
    var col: Int
    var len: Int
    var text: String
    var id: String { "\(path):\(line):\(col)" }
}

struct FTResults: Codable {
    var title: String
    var countNote: String
    var selected: Int
    var hits: [FTHit]
}

/// `filetree:<id>:doc`
struct FTDoc: Codable {
    var kind: String
    var path: String
    var loadRev: UInt64
    var text: String?
    var lang: String?
    var message: String?
    var pdf: Bool
    var sizeText: String?
    var diff: String?
    var compare: FTCompare?
    var results: FTResults?
}

struct FTHunk: Codable, Hashable {
    var start: Int
    var end: Int
    var kind: String
    var old: String
}

struct FTDiag: Codable, Hashable {
    var line: Int
    var col: Int
    var endLine: Int
    var endCol: Int
    var severity: Int
    var message: String
}

struct FTJump: Codable, Hashable {
    var line: Int
    var col: Int
    var seq: UInt64
    var focus: Bool
}

/// `filetree:<id>:live`
struct FTLive: Codable {
    var path: String
    var loadRev: UInt64
    var appliedRev: UInt64
    var dirty: Bool
    var external: Bool
    var hasHead: Bool
    var hunks: [FTHunk]
    var diags: [FTDiag]
    var jump: FTJump?
    var focusSeq: UInt64
    var spanRange: [Int]
    var spans: [UInt32]
}

/// `filetree:<id>:find`
struct FTFind: Codable {
    var query: String
    var `case`: Bool
    var words: Bool
    var regex: Bool
    var multiline: Bool
    var maskOn: Bool
    var mask: String
    var inDir: Bool
    var folder: String
    var selected: Int
    var searching: Bool
    var error: String?
    var countNote: String
    var hits: [FTHit]
    var focusSeq: UInt64
}

struct FTKind: Codable {
    var color: String
    var italic: Bool
}

/// `filetree:theme`
struct FTTheme: Codable {
    var kinds: [FTKind]
    var background: String
    var text: String
    var caret: String
    var caretRow: String
    var lineNumber: String
    var lineNumberCaret: String
    var selection: String
    var searchMatch: String
    var muted: String
    var link: String
    var string: String
    var comment: String
    var error: String
    var warning: String
    var added: String
    var modified: String
    var deleted: String
    var removedLine: String
    var addedLine: String
    var removedWord: String
    var addedWord: String
    var tabSelected: String
    var tabBorder: String
    var rightMargin: String
}

struct FTIconPair: Codable {
    var dark: String
    var light: String
}
