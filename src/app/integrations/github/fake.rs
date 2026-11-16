//! A test scene's GitHub (`MAGOAGENT_FAKE_GITHUB`, a file shaped as GitHub's answer to the pulse, next to a
//! `MAGOAGENT_SCENE`): the island's tab is seen and clicked with no token and no network. Each pull request's detail is
//! itself with a short description, and a write is answered here.

use super::Pulse;
use crate::prelude::*;
use std::sync::OnceLock;

/// The description every pull request of the scene has.
pub(super) const BODY: &str =
    "## Resumo\n\nPasso a passo para validar o **MSI** no Windows.\n\n## Testes\n\n- [x] O MSI instala e abre\n- [ ] Atualizar a partir da `0.0.18`";

/// The scene's pulse, read once; none outside a test scene.
pub(super) fn pulse() -> Option<&'static Pulse> {
    static PULSE: OnceLock<Option<Pulse>> = OnceLock::new();
    PULSE
        .get_or_init(|| {
            let path = std::env::var_os("MAGOAGENT_SCENE").and_then(|_| std::env::var_os("MAGOAGENT_FAKE_GITHUB"))?;
            let root: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
            Pulse::parse(&root)
        })
        .as_ref()
}
