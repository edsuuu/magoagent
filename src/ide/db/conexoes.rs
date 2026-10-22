//! Saved database connections (`connections.json`, never the password) and where the password
//! lives per retention: the system vault only for `Sempre`, process memory for the in-between.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Motor { #[default] Sqlite, Mysql, Mariadb, Postgres, Mongo, Redis }

impl Motor {
    pub(super) fn dialect(self) -> super::Dialect { match self { Self::Sqlite => super::Dialect::Sqlite, Self::Mysql | Self::Mariadb => super::Dialect::Mysql, Self::Postgres => super::Dialect::Postgres, Self::Mongo => super::Dialect::Mongo, Self::Redis => super::Dialect::Redis } }
    /// The engine's default port; SQLite is a file, so 0.
    pub(super) fn porta(self) -> u16 {
        match self { Motor::Sqlite => 0, Motor::Mysql | Motor::Mariadb => 3306, Motor::Postgres => 5432, Motor::Mongo => 27017, Motor::Redis => 6379 }
    }
    pub(super) fn nome(self) -> &'static str {
        match self { Motor::Sqlite => "SQLite", Motor::Mysql => "MySQL", Motor::Mariadb => "MariaDB", Motor::Postgres => "PostgreSQL", Motor::Mongo => "MongoDB", Motor::Redis => "Redis" }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Autenticacao { #[default] #[serde(rename = "user-pass")] UsuarioSenha, #[serde(rename = "no-auth")] Sem }

/// The IDE's "Save:" choice for a password.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Retencao { Nunca, AteReiniciar, Sessao, #[default] Sempre }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Conexao {
    pub(super) id: String,
    pub(super) nome: String,
    pub(super) comentario: String,
    pub(super) motor: Motor,
    pub(super) host: String,
    pub(super) tls: bool,
    pub(super) porta: u16,
    pub(super) banco: String,
    pub(super) usuario: String,
    pub(super) autenticacao: Autenticacao,
    #[serde(default = "auth_database")]
    pub(super) banco_autenticacao: String,
    pub(super) somente_leitura: bool,
    pub(super) auto_commit: bool,
    pub(super) retencao: Retencao,
    /// The database file, for SQLite.
    pub(super) arquivo: String,
}

fn auth_database() -> String { "admin".into() }

impl Conexao {
    pub(super) fn sqlite(arquivo: &Path) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            nome: arquivo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            arquivo: arquivo.to_string_lossy().into_owned(),
            auto_commit: true,
            ..Self::default()
        }
    }
}

/// Adds the SQLite file as a connection unless one already points at it; true when added.
pub(super) fn garantir_sqlite(lista: &mut Vec<Conexao>, arquivo: &Path) -> bool {
    let caminho = arquivo.to_string_lossy();
    if lista.iter().any(|c| c.motor == Motor::Sqlite && c.arquivo == caminho) { return false; }
    lista.push(Conexao::sqlite(arquivo));
    true
}

pub(super) fn carregar() -> Vec<Conexao> {
    crate::file::connections_path().map(|p| carregar_de(&p)).unwrap_or_default()
}

pub(super) fn salvar(lista: &[Conexao]) -> Result<(), String> {
    let path = crate::file::connections_path().ok_or("sem pasta de dados")?;
    salvar_em(&path, lista)
}

fn carregar_de(path: &Path) -> Vec<Conexao> {
    let Ok(data) = std::fs::read_to_string(path) else { return Vec::new() };
    serde_json::from_str(&data).unwrap_or_else(|_| {
        // keep an unreadable file aside so the next save does not erase it
        let _ = std::fs::rename(path, path.with_extension("json.bad"));
        Vec::new()
    })
}

fn salvar_em(path: &Path, lista: &[Conexao]) -> Result<(), String> {
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(lista).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, json).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| e.to_string())
}

mod cofre {
    use super::*;

    pub(in super::super) trait Cofre {
        fn guardar(&self, id: &str, senha: &str) -> Result<(), String>;
        fn ler(&self, id: &str) -> Option<String>;
        fn apagar(&self, id: &str) -> Result<(), String>;
    }

    /// The system vault (Keychain, Credential Manager, Secret Service): item `db:{id}` of `MagoAgent`.
    #[cfg(not(test))]
    #[derive(Default)]
    pub(in super::super) struct Sistema;

    #[cfg(not(test))]
    impl Sistema {
        fn item(id: &str) -> Result<keyring::Entry, String> {
            keyring::Entry::new("MagoAgent", &format!("db:{id}")).map_err(|e| e.to_string())
        }
    }

    #[cfg(not(test))]
    impl Cofre for Sistema {
        fn guardar(&self, id: &str, senha: &str) -> Result<(), String> {
            Self::item(id)?.set_password(senha).map_err(|e| e.to_string())
        }
        fn ler(&self, id: &str) -> Option<String> { Self::item(id).ok()?.get_password().ok() }
        fn apagar(&self, id: &str) -> Result<(), String> {
            match Self::item(id)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }

    /// Passwords kept in this process only; also the stand-in vault in tests.
    #[derive(Default)]
    pub(in super::super) struct Memoria(Mutex<HashMap<String, String>>);

    impl Cofre for Memoria {
        fn guardar(&self, id: &str, senha: &str) -> Result<(), String> {
            self.0.lock().map_err(|e| e.to_string())?.insert(id.into(), senha.into());
            Ok(())
        }
        fn ler(&self, id: &str) -> Option<String> { self.0.lock().ok()?.get(id).cloned() }
        fn apagar(&self, id: &str) -> Result<(), String> {
            self.0.lock().map_err(|e| e.to_string())?.remove(id);
            Ok(())
        }
    }

    /// Routes passwords by retention; session entries are cleared on disconnect.
    pub(in super::super) struct Senhas<C: Cofre> {
        pub(in super::super) cofre: C,
        pub(in super::super) memoria: Memoria,
        pub(in super::super) reinicio: Arc<Memoria>,
    }
    impl<C: Cofre + Default> Default for Senhas<C> {
        fn default() -> Self {
            static REINICIO: OnceLock<Arc<Memoria>> = OnceLock::new();
            Self { cofre: C::default(), memoria: Memoria::default(), reinicio: REINICIO.get_or_init(Default::default).clone() }
        }
    }

    impl<C: Cofre> Senhas<C> {
        pub(in super::super) fn guardar(&self, c: &Conexao, senha: &str) -> Result<(), String> {
            if c.autenticacao == Autenticacao::Sem { return Ok(()); }
            match c.retencao {
                Retencao::Nunca => Ok(()),
                Retencao::AteReiniciar => self.reinicio.guardar(&c.id, senha),
                Retencao::Sessao => self.memoria.guardar(&c.id, senha),
                Retencao::Sempre => self.cofre.guardar(&c.id, senha),
            }
        }
        pub(in super::super) fn ler(&self, c: &Conexao) -> Option<String> {
            match c.retencao {
                Retencao::Nunca => None,
                Retencao::AteReiniciar => self.reinicio.ler(&c.id),
                Retencao::Sessao => self.memoria.ler(&c.id),
                Retencao::Sempre => self.cofre.ler(&c.id),
            }
        }
        pub(in super::super) fn desconectar(&self, c: &Conexao) {
            if c.retencao == Retencao::Sessao { let _ = self.memoria.apagar(&c.id); }
        }
        /// Deleting a connection drops its password wherever it was kept.
        pub(in super::super) fn apagar(&self, c: &Conexao) -> Result<(), String> {
            self.memoria.apagar(&c.id)?;
            self.reinicio.apagar(&c.id)?;
            if c.retencao == Retencao::Sempre { self.cofre.apagar(&c.id)?; }
            Ok(())
        }
    }
}
#[allow(unused_imports)]
pub(super) use cofre::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mago-conn-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        dir.join("connections.json")
    }

    #[test]
    fn json_round_trips_without_any_password() {
        let path = temp();
        let mut lista = vec![Conexao { id: "a1".into(), nome: "local".into(), motor: Motor::Mysql, host: "127.0.0.1".into(), porta: Motor::Mysql.porta(), usuario: "root".into(), retencao: Retencao::AteReiniciar, ..Conexao::default() }];
        assert!(garantir_sqlite(&mut lista, Path::new("/proj/app.sqlite")));
        assert!(!garantir_sqlite(&mut lista, Path::new("/proj/app.sqlite")));
        salvar_em(&path, &lista).unwrap();
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.to_lowercase().contains("senha") && !json.contains("password"));
        assert_eq!(carregar_de(&path), lista);
        assert_eq!(lista[1].motor, Motor::Sqlite);
        assert_eq!(lista[1].nome, "app.sqlite");
        // a file with fields missing still loads, and one that is broken is kept aside
        std::fs::write(&path, r#"[{"id":"x","motor":"Redis"}]"#).unwrap();
        assert_eq!(carregar_de(&path)[0].motor, Motor::Redis);
        std::fs::write(&path, "{ não é json").unwrap();
        assert!(carregar_de(&path).is_empty() && path.with_extension("json.bad").exists());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn retention_picks_the_vault() {
        let senhas = Senhas::<Memoria>::default();
        let com = |retencao| Conexao { id: format!("{retencao:?}"), retencao, ..Conexao::default() };
        for retencao in [Retencao::Nunca, Retencao::AteReiniciar, Retencao::Sessao, Retencao::Sempre] {
            let c = com(retencao);
            senhas.guardar(&c, "segredo").unwrap();
            let (no_cofre, na_memoria) = (senhas.cofre.ler(&c.id).is_some(), (senhas.memoria.ler(&c.id).is_some() || senhas.reinicio.ler(&c.id).is_some()));
            let esperado = match retencao { Retencao::Nunca => (false, false), Retencao::Sempre => (true, false), _ => (false, true) };
            assert_eq!((no_cofre, na_memoria), esperado, "{retencao:?}");
            assert_eq!(senhas.ler(&c).as_deref(), (retencao != Retencao::Nunca).then_some("segredo"), "{retencao:?}");
            senhas.apagar(&c).unwrap();
            assert!(senhas.ler(&c).is_none() && senhas.cofre.ler(&c.id).is_none() && senhas.memoria.ler(&c.id).is_none() && senhas.reinicio.ler(&c.id).is_none());
        }
    }
    #[test]
    fn disconnect_forgets_only_session_and_restart_survives_an_ide() {
        let senhas = Senhas::<Memoria>::default();
        for retencao in [Retencao::Sessao, Retencao::AteReiniciar] {
            let c = Conexao { id: uuid::Uuid::new_v4().to_string(), retencao, ..Default::default() };
            senhas.guardar(&c, "segredo").unwrap();
            senhas.desconectar(&c);
            if retencao == Retencao::Sessao { assert!(senhas.ler(&c).is_none()); }
            else {
                assert_eq!(Senhas::<Memoria>::default().ler(&c).as_deref(), Some("segredo"));
                senhas.apagar(&c).unwrap();
            }
        }
    }

}
