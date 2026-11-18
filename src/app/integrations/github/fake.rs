//! A test scene's GitHub (`MAGOAGENT_FAKE_GITHUB`, a file shaped as GitHub's answer to the pulse, its viewer with
//! the calendar's `contributionsCollection` too, next to a `MAGOAGENT_SCENE`): the island's tab is seen and clicked with
//! no token and no network. Each pull request's detail is itself with a short description, and a write is answered here.

use super::{Activity, Pulse};
use crate::prelude::*;
use std::sync::OnceLock;

/// The description every pull request of the scene has.
pub(super) const BODY: &str =
    "## Resumo\n\nPasso a passo para validar o **MSI** no Windows.\n\n## Testes\n\n- [x] O MSI instala e abre\n- [ ] Atualizar a partir da `0.0.18`";

/// The scene's answer, read once; none outside a test scene.
fn answer() -> Option<&'static Value> {
    static ANSWER: OnceLock<Option<Value>> = OnceLock::new();
    ANSWER
        .get_or_init(|| {
            let path = std::env::var_os("MAGOAGENT_SCENE").and_then(|_| std::env::var_os("MAGOAGENT_FAKE_GITHUB"))?;
            read_json(std::path::Path::new(&path))
        })
        .as_ref()
}

/// The scene's pulse.
pub(super) fn pulse() -> Option<&'static Pulse> {
    static PULSE: OnceLock<Option<Pulse>> = OnceLock::new();
    PULSE.get_or_init(|| Pulse::parse(answer()?)).as_ref()
}

/// The scene's calendar, when its answer also has the viewer's `contributionsCollection`.
pub(super) fn activity() -> Option<Activity> {
    Activity::parse(answer()?)
}
