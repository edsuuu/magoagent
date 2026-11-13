//! The editor's pure parts (indent guides, declaration spans, restyling) the file tree's code pane draws with.

use super::*;
mod helpers;
#[cfg(test)]
mod tests;
pub(crate) use helpers::*;

pub(crate) const RIGHT_MARGIN_COLUMN: usize = 120;
