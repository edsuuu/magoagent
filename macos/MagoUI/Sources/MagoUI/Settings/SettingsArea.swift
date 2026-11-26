import AppKit
import SwiftUI

/// Ajustes in SwiftUI. `SettingsArea.start()` registers the view; the shell opens its window (`MagoViews.settings`).
@MainActor public enum SettingsArea {
    public static func start() {
        MagoViews.settings = { AnyView(SettingsView()) }
    }
}

/// `ui.settings.<verb>` to the Rust side.
@MainActor func settingsSend(_ verb: String, _ args: [String: Any] = [:]) {
    MagoStore.shared.send("ui.settings." + verb, args)
}

@MainActor func setSetting(_ id: String, _ value: Any) {
    settingsSend("set", ["id": id, "value": value])
}

@MainActor func pressSetting(_ button: SettingsButton, value: String = "") {
    if let text = button.copy {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }
    settingsSend("press", ["id": button.id, "arg": button.arg ?? "", "value": value])
}
