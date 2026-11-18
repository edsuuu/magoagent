//! STUB: replaced by the foundation
pub(crate) fn enabled(area: &str) -> bool {
    std::env::var("MAGOAGENT_SWIFTUI").is_ok_and(|list| list == "all" || list.split(',').any(|each| each.trim() == area))
}

pub(crate) fn publish<T: serde::Serialize>(_topic: &str, value: &T) {
    let _ = serde_json::to_string(value);
}
