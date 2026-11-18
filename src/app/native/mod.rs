//! STUB: replaced by the foundation
//! The contract the areas code against: which areas run in SwiftUI, and the JSON topics Swift reads.

/// `MAGOAGENT_SWIFTUI=all` or a comma list of area names.
#[allow(dead_code)]
pub(crate) fn enabled(area: &str) -> bool {
    std::env::var("MAGOAGENT_SWIFTUI").is_ok_and(|list| list.split(',').any(|name| name.trim() == "all" || name.trim() == area))
}

/// Sends `value` to Swift as topic `topic`, only when it changed.
pub(crate) fn publish<T: serde::Serialize>(topic: &str, value: &T) {
    let _ = (topic, serde_json::to_string(value));
}
