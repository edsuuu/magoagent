//! A atualização no macOS: o .zip da release (o MagoAgent.app, baixado e conferido em download.rs) se abre ao lado do
//! app e o .app novo toma o lugar do que roda (o processo segue vivo até fechar); então o app salva, para os agentes,
//! fecha e reabre na versão nova (deb_update.rs, `reopen_updated`). Baixado pelo app (curl), o .app não leva a marca de
//! quarentena e o macOS o abre sem perguntar.

use crate::prelude::*;
use std::path::Path;

/// The .app a program at `exe` runs from (`…/X.app/Contents/MacOS/magoagent`); a build of the repo has none.
pub(crate) fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;

    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

/// The .app this program runs from, on macOS.
pub(crate) fn bundle_here() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }

    bundle_of(&std::env::current_exe().ok()?)
}

/// Opens `zip` beside `bundle` and puts its .app in the bundle's place, the old one gone; on a failure nothing changes.
pub(crate) fn install(zip: &Path, bundle: &Path) -> Result<(), String> {
    let folder = bundle.parent().ok_or("não sei onde o MagoAgent está")?;
    // beside the app: the same disk, so the swap is a rename
    let staging = folder.join(".MagoAgent-atualizando");
    let _ = fs::remove_dir_all(&staging);

    let opened = crate::editor::command("ditto").args(["-x", "-k"]).arg(zip).arg(&staging).output();

    let result = match opened {
        Ok(output) if output.status.success() => fs::read_dir(&staging)
            .ok()
            .and_then(|entries| entries.flatten().map(|entry| entry.path()).find(|path| path.extension().is_some_and(|ext| ext == "app")))
            .ok_or_else(|| "o pacote baixado não traz o MagoAgent.app".to_string())
            .and_then(|new| swap(&new, bundle, &staging.join("anterior.app"))),
        Ok(output) => Err(format!("o pacote baixado não abriu ({})", String::from_utf8_lossy(&output.stderr).trim())),
        Err(error) => Err(format!("o ditto não abriu: {error}")),
    };

    // the old app goes with the folder: a running program keeps its files open until it closes
    let _ = fs::remove_dir_all(&staging);

    result
}

/// `new` in `bundle`'s place, the old one moved to `old` first: when the new one can't go in, the old one comes back.
pub(crate) fn swap(new: &Path, bundle: &Path, old: &Path) -> Result<(), String> {
    let place = bundle.display();

    fs::rename(bundle, old).map_err(|error| format!("o app em {place} não pôde ser trocado ({error}); nada mudou"))?;

    if let Err(error) = fs::rename(new, bundle) {
        let _ = fs::rename(old, bundle);

        return Err(format!("a versão nova não pôde ir para {place} ({error}); nada mudou"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("magoagent-mac-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);

        fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// A .app with `version` written in its program.
    fn app(at: &Path, version: &str) {
        fs::create_dir_all(at.join("Contents/MacOS")).unwrap();
        fs::write(at.join("Contents/MacOS/magoagent"), version).unwrap();
    }

    fn version(bundle: &Path) -> String {
        fs::read_to_string(bundle.join("Contents/MacOS/magoagent")).unwrap()
    }

    #[test]
    fn only_a_program_inside_an_app_has_a_bundle() {
        assert_eq!(bundle_of(Path::new("/Users/eu/Desktop/MagoAgent.app/Contents/MacOS/magoagent")), Some(PathBuf::from("/Users/eu/Desktop/MagoAgent.app")));
        assert_eq!(bundle_of(Path::new("/Users/eu/magoagent/target/release/magoagent")), None, "a build of the repo");
        assert_eq!(bundle_of(Path::new("/x/MagoAgent/Contents/MacOS/magoagent")), None, "no .app");
    }

    #[test]
    fn the_new_app_takes_the_old_ones_place_and_a_failed_swap_changes_nothing() {
        let folder = folder("troca");
        let (bundle, new, old) = (folder.join("MagoAgent.app"), folder.join("novo/MagoAgent.app"), folder.join("anterior.app"));

        app(&bundle, "0.0.39");
        app(&new, "0.0.40");
        assert_eq!(swap(&new, &bundle, &old), Ok(()));
        assert_eq!(version(&bundle), "0.0.40");
        // the new one is not there: the old one comes back
        assert!(swap(&folder.join("sumiu.app"), &bundle, &folder.join("anterior2.app")).is_err());
        assert_eq!(version(&bundle), "0.0.40");
        let _ = fs::remove_dir_all(&folder);
    }

    /// The release's zip as `ditto -c -k --keepParent` makes it, installed over the app.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_releases_zip_replaces_the_app() {
        let folder = folder("zip");
        let (bundle, made) = (folder.join("MagoAgent.app"), folder.join("pacote/MagoAgent.app"));
        let zip = folder.join("MagoAgent-0.0.40-macos-arm64.zip");

        app(&bundle, "0.0.39");
        app(&made, "0.0.40");
        let zipped = std::process::Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(&made).arg(&zip).status().unwrap();

        assert!(zipped.success());
        assert_eq!(install(&zip, &bundle), Ok(()));
        assert_eq!(version(&bundle), "0.0.40");
        assert!(!folder.join(".MagoAgent-atualizando").exists(), "nothing left beside the app");
        // a file that is no zip: nothing changes
        fs::write(&zip, "isto não é um zip").unwrap();
        assert!(install(&zip, &bundle).is_err());
        assert_eq!(version(&bundle), "0.0.40");
        let _ = fs::remove_dir_all(&folder);
    }
}
