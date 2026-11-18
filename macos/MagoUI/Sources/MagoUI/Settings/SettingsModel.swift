import Foundation

/// The topic "settings" (src/app/ui/settings/native): the open page as groups of rows. Keys are camelCase on the Rust
/// side, so the decoder's snake-case strategy, if any, changes nothing.
enum JSON: Decodable, Equatable {
    case bool(Bool), int(Int), double(Double), string(String), object([String: JSON]), array([JSON]), null

    init(from decoder: Decoder) throws {
        let one = try decoder.singleValueContainer()
        if one.decodeNil() { self = .null }
        else if let value = try? one.decode(Bool.self) { self = .bool(value) }
        else if let value = try? one.decode(Int.self) { self = .int(value) }
        else if let value = try? one.decode(Double.self) { self = .double(value) }
        else if let value = try? one.decode(String.self) { self = .string(value) }
        else if let value = try? one.decode([String: JSON].self) { self = .object(value) }
        else { self = .array(try one.decode([JSON].self)) }
    }

    var bool: Bool { if case .bool(let value) = self { value } else { false } }
    var int: Int { switch self { case .int(let value): value; case .double(let value): Int(value); default: 0 } }
    var string: String { if case .string(let value) = self { value } else { "" } }
    subscript(key: String) -> JSON { if case .object(let map) = self { map[key] ?? .null } else { .null } }
}

struct SettingsButton: Decodable {
    let id: String
    let label: String
    let arg: String?
    let copy: String?
    let primary: Bool
    let destructive: Bool
}

struct SettingsKeys: Decodable {
    let text: String
    let editing: Bool
    let pending: Bool
    let problem: Bool
    let global: Bool
    let enabled: Bool
    let canClear: Bool
    let changed: Bool
}

struct SettingsItem: Decodable {
    let id: String
    let kind: String
    let label: String
    let hint: String
    let value: JSON
    let options: [String]
    let min: Int
    let max: Int
    let suffix: String
    let placeholder: String
    let secret: Bool
    let tone: String
    let buttons: [SettingsButton]
    let keys: SettingsKeys?
}

struct SettingsGroup: Decodable {
    let title: String
    let items: [SettingsItem]
}

struct SettingsPreview: Decodable {
    let title: String
    let confirm: String
    let state: String
    let path: String
    let diff: [String]
}

struct SettingsSnapshot: Decodable {
    let tabs: [String]
    let tab: Int
    let title: String
    let subtitle: String
    let groups: [SettingsGroup]
    let hooksPreview: SettingsPreview?
    let wipe: [String]?
}
