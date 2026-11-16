//! GitHub's personal token: what one looks like, and Ajustes' Salvar, Remover and Testar. It lives in the system's
//! keychain and nowhere else (never `preferences.json`, a log, an address or the MCP).

use super::{Failure, Message, TOKEN_ID, USER, http, read, request, text_at};
use crate::prelude::*;

/// The token as GitHub makes them: printable ASCII with no spaces, of a sane length.
pub(crate) fn token_looks_right(token: &str) -> bool {
    (20..=255).contains(&token.len()) && token.chars().all(|letter| letter.is_ascii_graphic())
}

/// Who the token is (`GET /user`): Testar's answer.
fn fetch_login(token: &str) -> Result<String, Failure> {
    let user = read(http::send(&request(token, "GET", USER, None)))?;
    text_at(&user, "login", 60).ok_or(Failure { message: "A resposta do GitHub não tem o login.".into(), unauthorized: false })
}

impl DrawingApp {
    /// Salvar: the token into the keychain (never a file), and the polls start over with it.
    pub(crate) fn github_save_token(&mut self, token: &str) -> Result<String, String> {
        #[cfg(test)]
        mock_keyring();
        let token = token.trim();
        if !token_looks_right(token) {
            return Err("Isso não parece um token do GitHub: cole o token inteiro, sem espaços.".into());
        }
        set_secret(TOKEN_ID, token).map_err(|why| format!("O cofre do sistema recusou o token: {why}"))?;
        let github = &mut self.integrations.github;
        github.forget();
        github.token = Some(token.to_string());
        self.github_test();
        Ok("Token do GitHub guardado no cofre do sistema.".into())
    }

    /// Remover: the token out of the keychain and everything fetched with it forgotten.
    pub(crate) fn github_remove_token(&mut self) -> Result<String, String> {
        #[cfg(test)]
        mock_keyring();
        remove_secret(TOKEN_ID)?;
        let github = &mut self.integrations.github;
        github.forget();
        github.token = Some(String::new());
        Ok("Token do GitHub removido.".into())
    }

    /// Testar: asks GitHub who the token is; the answer comes as a toast.
    pub(crate) fn github_test(&mut self) -> bool {
        let github = &mut self.integrations.github;
        github.load_token();
        if !github.has_token() || github.testing {
            return false;
        }
        (github.testing, github.next_pulse) = (!cfg!(test), 0);
        if !cfg!(test) {
            github.spawn(|token| Message::Login(fetch_login(token)));
        }
        true
    }
}
