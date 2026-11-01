//! The password vault (☰ › Ir › Cofre…): the accounts the agents log in with without ever seeing
//! a secret. The entries (site, user, kind) live in `vault.json` beside the drawing; each
//! secret goes to the system's keychain (Keychain, Credential Manager, Secret Service) as the
//! item `<id>` of `MagoAgent`, so the MCP server (same user) reads it back for `vault_fill`.

use crate::prelude::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct VaultEntry {
    pub(crate) id: String,
    /// A domain or an app's name, as in `github.com`.
    pub(crate) site: String,
    /// The e-mail or user name.
    pub(crate) user: String,
    #[serde(default)]
    pub(crate) kind: VaultKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) note: String,
}

/// How the account is entered: a password of its own, or through Google/Apple (only the e-mail
/// to pick, the provider's button does the rest).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum VaultKind {
    #[default]
    Senha,
    Google,
    Apple,
}

impl VaultKind {
    pub(crate) const ALL: [VaultKind; 3] = [VaultKind::Senha, VaultKind::Google, VaultKind::Apple];
    pub(crate) const LABELS: [&str; 3] = ["Senha", "Google", "Apple"];

    /// The provider that signs the person in, when it is not a password.
    pub(crate) fn provider(self) -> Option<&'static str> {
        match self {
            VaultKind::Senha => None,
            VaultKind::Google => Some("Google"),
            VaultKind::Apple => Some("Apple"),
        }
    }

    /// What the agents see: `senha`, `google`, `apple`.
    pub(crate) fn name(self) -> &'static str {
        match self {
            VaultKind::Senha => "senha",
            VaultKind::Google => "google",
            VaultKind::Apple => "apple",
        }
    }
}

impl VaultEntry {
    pub(crate) fn new() -> Self {
        Self { id: uuid::Uuid::new_v4().to_string(), ..Default::default() }
    }

    /// Of `site`: the same domain or name, ignoring case and a `www.`-like prefix on either side.
    pub(crate) fn is_of(&self, site: &str) -> bool {
        let (a, b) = (self.site.trim().to_lowercase(), site.trim().to_lowercase());
        !b.is_empty() && (a == b || a.ends_with(&format!(".{b}")) || b.ends_with(&format!(".{a}")))
    }
}

pub(crate) fn load_vault() -> Vec<VaultEntry> {
    vault_path().map(|p| read_vault(&p)).unwrap_or_default()
}

pub(crate) fn read_vault(path: &std::path::Path) -> Vec<VaultEntry> {
    fs::read_to_string(path).ok().and_then(|data| serde_json::from_str(&data).ok()).unwrap_or_default()
}

pub(crate) fn save_vault(entries: &[VaultEntry]) -> Result<(), String> {
    let path = vault_path().ok_or("pasta de dados do usuário não encontrada")?;
    write_vault(&path, entries).map_err(|e| e.to_string())
}

pub(crate) fn write_vault(path: &std::path::Path, entries: &[VaultEntry]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    write_atomic(path, serde_json::to_string_pretty(entries).map_err(std::io::Error::other)?)
}

/// The keychain item of an entry. The system's store, unless the tests put the mock one in first.
fn item(id: &str) -> Result<keyring_core::Entry, String> {
    if keyring_core::get_default_store().is_none() {
        keyring::Entry::store_status().as_ref().map_err(|e| e.to_string())?;
    }
    keyring_core::Entry::new("MagoAgent", id).map_err(|e| match e {
        keyring_core::Error::NoDefaultStore => "O cofre do sistema não está disponível".to_string(),
        e => e.to_string(),
    })
}

pub(crate) fn set_secret(id: &str, secret: &str) -> Result<(), String> {
    item(id)?.set_password(secret).map_err(|e| e.to_string())
}

pub(crate) fn take_secret(id: &str) -> Option<String> {
    item(id).ok()?.get_password().ok()
}

pub(crate) fn remove_secret(id: &str) -> Result<(), String> {
    match item(id)?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// The entries of a site.
pub(crate) fn accounts<'a>(entries: &'a [VaultEntry], site: &str) -> Vec<&'a VaultEntry> {
    entries.iter().filter(|e| e.is_of(site)).collect()
}

/// The one entry a login means: the site's only account, or the one `user` names among several.
pub(crate) fn pick_account<'a>(entries: &'a [VaultEntry], site: &str, user: Option<&str>) -> Result<&'a VaultEntry, String> {
    let found = accounts(entries, site);
    let users = || found.iter().map(|e| e.user.as_str()).collect::<Vec<_>>().join(", ");
    match (&found[..], user.map(str::trim).filter(|u| !u.is_empty())) {
        ([], _) => Err(format!("Nenhuma conta de {site} no cofre: peça à pessoa para cadastrar em ☰ › Cofre…")),
        (_, Some(user)) => {
            found.iter().find(|e| e.user.eq_ignore_ascii_case(user)).copied().ok_or_else(|| format!("Nenhuma conta {user} em {site}; há: {}.", users()))
        }
        ([one], None) => Ok(one),
        (_, None) => Err(format!("{site} tem mais de uma conta ({}): pergunte à pessoa qual usar e repita com `user`.", users())),
    }
}

/// Tests keep secrets in keyring's in-memory store, never in the person's keychain.
#[cfg(test)]
pub(crate) fn mock_keyring() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(site: &str, user: &str, kind: VaultKind) -> VaultEntry {
        VaultEntry { site: site.into(), user: user.into(), kind, ..VaultEntry::new() }
    }

    #[test]
    fn vault_json_round_trips_without_secrets() {
        let dir = std::env::temp_dir().join(format!("magoagent-vault-{}", std::process::id()));
        let path = dir.join("vault.json");
        let entries = vec![
            entry("github.com", "ana@exemplo.com", VaultKind::Senha),
            VaultEntry { note: "conta do trabalho".into(), ..entry("Figma", "ana@exemplo.com", VaultKind::Google) },
        ];
        write_vault(&path, &entries).unwrap();
        let json = fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"kind\": \"google\"") && !json.contains("secret"));
        assert_eq!(read_vault(&path), entries);
        assert!(read_vault(&dir.join("nada.json")).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn secrets_live_in_the_keyring_only() {
        mock_keyring();
        let e = entry("github.com", "ana@exemplo.com", VaultKind::Senha);
        assert_eq!(take_secret(&e.id), None);
        set_secret(&e.id, "s3gr3do").unwrap();
        assert_eq!(take_secret(&e.id).as_deref(), Some("s3gr3do"));
        remove_secret(&e.id).unwrap();
        assert_eq!(take_secret(&e.id), None);
        remove_secret(&e.id).unwrap();
    }

    #[test]
    fn accounts_are_picked_by_site_and_user() {
        let entries = vec![
            entry("github.com", "ana@exemplo.com", VaultKind::Senha),
            entry("GitHub.com", "trabalho@exemplo.com", VaultKind::Google),
            entry("figma.com", "ana@exemplo.com", VaultKind::Senha),
        ];
        let users = |site: &str| accounts(&entries, site).iter().map(|e| e.user.clone()).collect::<Vec<_>>();
        assert_eq!(users("github.com"), ["ana@exemplo.com", "trabalho@exemplo.com"]);
        assert_eq!(users("www.figma.com"), ["ana@exemplo.com"]);
        assert!(users("gitlab.com").is_empty());
        assert_eq!(pick_account(&entries, "figma.com", None).unwrap().user, "ana@exemplo.com");
        assert_eq!(pick_account(&entries, "github.com", Some("Trabalho@exemplo.com")).unwrap().kind, VaultKind::Google);
        assert!(pick_account(&entries, "github.com", None).unwrap_err().contains("mais de uma conta"));
        assert!(pick_account(&entries, "github.com", Some("x@y")).unwrap_err().contains("Nenhuma conta x@y"));
        assert!(pick_account(&entries, "gitlab.com", None).unwrap_err().starts_with("Nenhuma conta de gitlab.com"));
    }
}
