//! The mascots: Luna (`mago/`), each agent's colour and name (`identity.rs`), and whether an agent is Claude Code's.

pub(crate) mod identity;
pub(crate) mod mago;

use crate::prelude::*;

/// The agent is Claude Code's (its command or its name).
pub(crate) fn claude(spec: &TerminalSpec) -> bool {
    spec.command.trim_start().starts_with("claude") || spec.name.to_lowercase().contains("claude")
}
