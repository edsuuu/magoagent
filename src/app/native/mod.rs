//! STUB: replaced by the foundation

use crate::prelude::*;

/// `MAGOAGENT_SWIFTUI=all` or a comma list of areas.
pub(crate) fn enabled(area: &str) -> bool {
    std::env::var("MAGOAGENT_SWIFTUI").is_ok_and(|areas| areas.split(',').any(|name| name.trim() == "all" || name.trim() == area))
}

/// JSON to Swift only when changed (the stub serializes and drops it).
#[allow(dead_code)]
pub(crate) fn publish<T: Serialize>(topic: &str, value: &T) {
    let _ = (topic, serde_json::to_string(value));
}
