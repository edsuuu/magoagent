//! "Abrir o Claude": a janela em que uma sessão de fora roda (o terminal, o editor, o Claude Desktop), para trazê-la
//! para a frente. O relay é neto do Claude Code, que roda num shell, que roda no terminal ou editor dono da janela:
//! enquanto o relay espera a resposta da ponte ele está vivo, e subir a partir dele acha o processo dessa janela. O que
//! se acha fica por sessão, só em memória, e é consultado quando o botão é clicado. Onde nada se acha (um console
//! clássico, cuja janela é de um conhost que não é ancestral) não há o que trazer.
// ponytail: achar e trazer a janela só existe no Windows (EnumWindows, SetForegroundWindow); no Linux e no macOS nenhuma
// sessão guarda janela e `focus` devolve false. Evoluir: _NET_ACTIVE_WINDOW no X11 e NSRunningApplication no macOS

use crate::app::chat::direct::folder_label;
use crate::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;

/// Nunca a janela de uma sessão: o shell da área de trabalho e os serviços no topo de toda árvore de processos. Chegar
/// a um deles quer dizer que a janela do terminal não era ancestral.
const TREE_TOPS: [&str; 10] =
    ["explorer.exe", "services.exe", "wininit.exe", "winlogon.exe", "svchost.exe", "smss.exe", "csrss.exe", "system", "sihost.exe", "userinit.exe"];
/// Até onde se sobe: uma sessão fica a poucos níveis da sua janela.
const MAX_DEPTH: usize = 16;
/// Quantas sessões são lembradas; passando disso a mais antiga sai.
const MAX_SESSIONS: usize = 64;

/// Sessão → (processo dono da janela, pasta da sessão). Processo 0: procurada e sem janela, para não procurar de novo a
/// cada evento.
static SESSIONS: Mutex<Vec<(String, u32, String)>> = Mutex::new(Vec::new());

fn sessions() -> std::sync::MutexGuard<'static, Vec<(String, u32, String)>> {
    SESSIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Os ancestrais de `start` na tabela de processos (processo → (pai, nome)), o mais próximo primeiro, parando antes do
/// topo da árvore. `start`, o relay, não entra. Um laço nos pais (um número de processo reusado) encerra a subida.
fn ancestors(table: &HashMap<u32, (u32, String)>, start: u32) -> Vec<u32> {
    let mut found = Vec::new();
    let mut current = start;
    while found.len() < MAX_DEPTH {
        let Some(&(parent, _)) = table.get(&current) else { break };
        let Some((_, name)) = table.get(&parent) else { break };
        if parent == 0 || parent == start || found.contains(&parent) || TREE_TOPS.contains(&name.to_lowercase().as_str()) {
            break;
        }
        found.push(parent);
        current = parent;
    }
    found
}

/// O ancestral mais próximo que tem janela na tela (o terminal ou o editor); 0 quando nenhum tem.
fn window_owner(above: &[u32], windows: &[(isize, u32, String)]) -> u32 {
    above.iter().copied().find(|process| windows.iter().any(|(_, owner, _)| owner == process)).unwrap_or(0)
}

/// Das janelas de um processo, (janela, título), a que vai para a frente: a que leva a pasta da sessão no título (um
/// editor com vários projetos abertos), senão a primeira.
#[allow(dead_code)]
fn pick_window<'a, Window>(windows: &'a [(Window, String)], folder: &str) -> Option<&'a Window> {
    let folder = folder.to_lowercase();
    let named = windows.iter().find(|(_, title)| !folder.is_empty() && title.to_lowercase().contains(&folder));
    named.or(windows.first()).map(|(window, _)| window)
}

fn remember(session: &str, owner: u32, folder: &str) {
    let mut sessions = sessions();
    sessions.retain(|(known, ..)| known != session);
    sessions.push((session.to_string(), owner, folder.to_string()));
    let excess = sessions.len().saturating_sub(MAX_SESSIONS);
    sessions.drain(..excess);
}

/// O dono da janela da sessão e a pasta dela, quando uma janela foi achada.
fn lookup(session: &str) -> Option<(u32, String)> {
    sessions().iter().find(|(known, owner, _)| known == session && *owner != 0).map(|(_, owner, folder)| (*owner, folder.clone()))
}

/// Um evento de hook de fora chegou à ponte, com o relay (`mago_pid`) ainda vivo à espera da resposta: acha, uma vez
/// por sessão, a janela em que ela roda. Uma sessão que acabou é esquecida.
pub(crate) fn note(payload: &Value) {
    let (Some(session), Some(relay)) = (payload["session_id"].as_str().filter(|session| !session.is_empty()), payload["mago_pid"].as_u64()) else { return };
    if hook_name(payload) == "SessionEnd" {
        return sessions().retain(|(known, ..)| known != session);
    }
    if sessions().iter().any(|(known, ..)| known == session) {
        return;
    }
    let above = ancestors(&crate::transcript::process::table(), relay as u32);
    // nenhum ancestral: o relay já tinha saído, e o próximo evento da sessão tenta de novo
    if above.is_empty() {
        return;
    }
    let folder = Some(folder_label(payload["cwd"].as_str().unwrap_or_default())).filter(|folder| folder != "~").unwrap_or_default();
    remember(session, window_owner(&above, &top_windows()), &folder);
}

/// A sessão tem uma janela conhecida: "Abrir o Claude" tem o que abrir.
#[allow(dead_code)]
pub(crate) fn has_window(session: &str) -> bool {
    lookup(session).is_some()
}

/// Traz para a frente a janela em que a sessão roda; false quando não há janela conhecida ou o sistema não deixou.
pub(crate) fn focus_session_window(session: &str) -> bool {
    let Some((owner, folder)) = lookup(session) else { return false };
    let windows: Vec<(isize, String)> =
        top_windows().into_iter().filter(|(_, process, _)| *process == owner).map(|(window, _, title)| (window, title)).collect();
    pick_window(&windows, &folder).is_some_and(|window| bring_forward(*window))
}

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(isize, isize) -> i32, data: isize) -> i32;
    fn IsWindowVisible(window: isize) -> i32;
    fn GetWindow(window: isize, relation: u32) -> isize;
    fn GetWindowThreadProcessId(window: isize, process: *mut u32) -> u32;
    fn GetWindowTextW(window: isize, text: *mut u16, capacity: i32) -> i32;
    fn IsWindow(window: isize) -> i32;
    fn IsIconic(window: isize) -> i32;
    fn ShowWindow(window: isize, command: i32) -> i32;
    fn BringWindowToTop(window: isize) -> i32;
    fn SetForegroundWindow(window: isize) -> i32;
    fn AllowSetForegroundWindow(process: u32) -> i32;
    fn GetForegroundWindow() -> isize;
}

/// As janelas de topo visíveis, com título e sem dona (não são o diálogo de ninguém): (janela, processo, título).
#[cfg(windows)]
fn top_windows() -> Vec<(isize, u32, String)> {
    const GW_OWNER: u32 = 4;
    unsafe extern "system" fn collect(window: isize, data: isize) -> i32 {
        // SAFETY: `data` é o Vec que `top_windows` passou, vivo durante toda a enumeração; as chamadas só leem a janela
        unsafe {
            let list = &mut *(data as *mut Vec<(isize, u32, String)>);
            if IsWindowVisible(window) != 0 && GetWindow(window, GW_OWNER) == 0 {
                let mut process = 0u32;
                GetWindowThreadProcessId(window, &mut process);
                let mut text = [0u16; 512];
                let length = GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32);
                if length > 0 {
                    list.push((window, process, String::from_utf16_lossy(&text[..length as usize])));
                }
            }
        }
        1
    }
    let mut list: Vec<(isize, u32, String)> = Vec::new();
    // SAFETY: a enumeração é síncrona: `collect` só roda dentro desta chamada, com o Vec emprestado
    unsafe { EnumWindows(collect, &mut list as *mut Vec<(isize, u32, String)> as isize) };
    list
}

#[cfg(not(windows))]
fn top_windows() -> Vec<(isize, u32, String)> {
    Vec::new()
}

/// Restaura a janela se estiver minimizada e a traz para a frente. Não mexe no mouse nem no teclado: o clique da pessoa
/// na ilha é o que dá ao app o direito de trocar a janela da frente.
#[cfg(windows)]
fn bring_forward(window: isize) -> bool {
    const SW_RESTORE: i32 = 9;
    // SAFETY: chamadas simples da user32 sobre uma janela conferida antes; uma que sumiu só faz as chamadas falharem
    unsafe {
        if IsWindow(window) == 0 {
            return false;
        }
        if IsIconic(window) != 0 {
            ShowWindow(window, SW_RESTORE);
        }
        BringWindowToTop(window);
        AllowSetForegroundWindow(u32::MAX);
        SetForegroundWindow(window) != 0 || GetForegroundWindow() == window
    }
}

#[cfg(not(windows))]
fn bring_forward(_window: isize) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(u32, u32, &str)]) -> HashMap<u32, (u32, String)> {
        entries.iter().map(|(process, parent, name)| (*process, (*parent, name.to_string()))).collect()
    }

    fn windows_of(owners: &[u32]) -> Vec<(isize, u32, String)> {
        owners.iter().map(|owner| (*owner as isize, *owner, "janela".to_string())).collect()
    }

    #[test]
    fn the_terminal_is_found_above_the_shells() {
        // Windows Terminal → pwsh → claude (node) → bash → o relay
        let table = tree(&[
            (4, 0, "System"),
            (100, 4, "explorer.exe"),
            (200, 100, "WindowsTerminal.exe"),
            (300, 200, "pwsh.exe"),
            (400, 300, "node.exe"),
            (500, 400, "bash.exe"),
            (600, 500, "magoagent-hook.exe"),
        ]);
        let above = ancestors(&table, 600);
        assert_eq!(above, [500, 400, 300, 200], "o mais próximo primeiro, sem passar do explorer");
        assert_eq!(window_owner(&above, &windows_of(&[100, 200])), 200);
        // o VS Code: o processo da janela fica acima do host do terminal
        let code = tree(&[
            (100, 1, "explorer.exe"),
            (210, 100, "Code.exe"),
            (220, 210, "Code.exe"),
            (300, 220, "powershell.exe"),
            (400, 300, "node.exe"),
            (600, 400, "magoagent-hook.exe"),
        ]);
        assert_eq!(window_owner(&ancestors(&code, 600), &windows_of(&[210])), 210);
        // um console clássico: a janela é do conhost, que não é ancestral; a subida para no explorer em vez de achar a barra de tarefas
        let classic = tree(&[(100, 1, "Explorer.EXE"), (300, 100, "cmd.exe"), (400, 300, "node.exe"), (600, 400, "magoagent-hook.exe")]);
        assert_eq!(window_owner(&ancestors(&classic, 600), &windows_of(&[100])), 0);
    }

    #[test]
    fn a_relay_already_gone_a_loop_or_a_deep_tree_end_the_walk() {
        assert!(ancestors(&tree(&[]), 600).is_empty());
        let looped = tree(&[(1, 2, "a.exe"), (2, 1, "b.exe"), (3, 1, "magoagent-hook.exe")]);
        assert_eq!(ancestors(&looped, 3), [1, 2]);
        let deep: Vec<(u32, u32, &str)> = (1..100).map(|process| (process, process + 1, "x.exe")).collect();
        assert_eq!(ancestors(&tree(&deep), 1).len(), MAX_DEPTH);
    }

    #[test]
    fn the_window_named_after_the_folder_wins() {
        let windows = vec![(1, "notas — Visual Studio Code".to_string()), (2, "app.ts — Loja — Visual Studio Code".to_string())];
        assert_eq!(pick_window(&windows, "loja"), Some(&2));
        assert_eq!(pick_window(&windows, "outra"), Some(&1));
        assert_eq!(pick_window(&windows, ""), Some(&1));
        assert_eq!(pick_window::<i32>(&[], "loja"), None);
    }

    #[test]
    fn sessions_are_remembered_once_and_forgotten_when_they_end() {
        remember("teste-janela-1", 42, "loja");
        remember("teste-janela-1", 43, "loja");
        assert_eq!(lookup("teste-janela-1"), Some((43, "loja".into())));
        assert!(has_window("teste-janela-1"));
        // sem janela achada: lembrada para não procurar de novo, sem nada a abrir
        remember("teste-janela-2", 0, "");
        assert!(!has_window("teste-janela-2") && !focus_session_window("teste-janela-2"));
        // um evento sem o processo do relay (um hook do próprio app) não procura nada; o fim da sessão a esquece
        note(&json!({ "hook_event_name": "Stop", "session_id": "teste-janela-3" }));
        assert!(!sessions().iter().any(|(known, ..)| known == "teste-janela-3"));
        note(&json!({ "hook_event_name": "SessionEnd", "session_id": "teste-janela-1", "mago_pid": 1 }));
        assert!(!has_window("teste-janela-1"));
        // um relay de verdade (este processo de teste): a sessão fica decidida, com ou sem janela
        note(&json!({ "hook_event_name": "Stop", "session_id": "teste-janela-4", "mago_pid": std::process::id(), "cwd": "C:/proj/loja" }));
        assert!(sessions().iter().any(|(known, _, folder)| known == "teste-janela-4" && folder == "loja"));
        // só as últimas ficam (um teste só: a lista é de todos)
        for count in 0..MAX_SESSIONS + 5 {
            remember(&format!("teste-janela-cheia-{count}"), 1, "");
        }
        assert!(!has_window("teste-janela-cheia-0") && has_window(&format!("teste-janela-cheia-{}", MAX_SESSIONS + 4)));
        sessions().retain(|(known, ..)| !known.starts_with("teste-janela"));
    }
}
