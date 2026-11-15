//! The app's own housekeeping: files, update check, memory use, the MAGOAGENT_SCRIPT driver.

pub(crate) mod files;
pub(crate) mod memory;
pub(crate) mod run_at_login;
pub(crate) mod script;
pub(crate) mod tray;
pub(crate) mod download;
pub(crate) mod update_flow;
pub(crate) mod updater;
pub(crate) mod updates;
