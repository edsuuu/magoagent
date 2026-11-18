//! STUB: replaced by the foundation
#![allow(dead_code)]
use serde::Serialize;

/// Whether `area` is drawn by SwiftUI: `MAGOAGENT_SWIFTUI=all` or a comma list of areas.
pub(crate) fn enabled(area: &str) -> bool {
    std::env::var("MAGOAGENT_SWIFTUI").is_ok_and(|list| list.split(',').any(|a| a.trim() == "all" || a.trim() == area))
}

/// JSON to Swift, only when it changed.
pub(crate) fn publish<T: Serialize>(topic: &str, value: &T) {
    let _ = (topic, serde_json::to_string(value));
}
