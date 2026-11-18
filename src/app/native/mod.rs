//! STUB: replaced by the foundation
//! The SwiftUI side of the app on macOS: which areas it draws and the snapshots it is sent.

use crate::prelude::*;

/// Whether SwiftUI draws `area` (`MAGOAGENT_SWIFTUI=all`, or a comma list of areas).
pub(crate) fn enabled(area: &str) -> bool {
    std::env::var("MAGOAGENT_SWIFTUI").is_ok_and(|value| value.split(',').any(|name| name.trim() == "all" || name.trim() == area))
}

/// Sends `value` to Swift as `topic` when it changed (the stub only serializes it).
#[allow(dead_code)]
pub(crate) fn publish<T: Serialize>(topic: &str, value: &T) {
    let _ = (topic, serde_json::to_string(value));
}
