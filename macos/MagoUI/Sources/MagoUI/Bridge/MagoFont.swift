// A fonte da interface do app (a Assistant, a mesma do egui), registrada pelo Rust no começo (`mago_ui_font`).
import SwiftUI

extension Font {
    /// A Assistant em `size` pontos: o texto da interface, como o egui (`ASSISTANT_FONT`, só o peso regular). Onde o egui
    /// escreve em seminegrito ele usa a fonte do sistema (`SYSTEM_SEMIBOLD`): ali, `.system(size:weight: .semibold)`.
    public static func app(_ size: CGFloat) -> Font {
        .custom("Assistant", size: size)
    }
}
