//! STUB: replaced by the foundation
use serde::Serialize;

/// `MAGOAGENT_SWIFTUI=all` or a comma list of areas.
pub(crate) fn enabled(area: &str) -> bool {
    std::env::var("MAGOAGENT_SWIFTUI").is_ok_and(|value| value.split(',').any(|name| name.trim() == "all" || name.trim() == area))
}

/// JSON to Swift only when changed (the stub drops it).
pub(crate) fn publish<T: Serialize>(_topic: &str, value: &T) {
    let _ = serde_json::to_string(value);
}
