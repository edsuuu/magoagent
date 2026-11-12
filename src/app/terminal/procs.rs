//! The terminal card's processes: the button at the footer's right opens a list of what runs under
//! its shell (name, PID, memory) with a button to stop each one.

use crate::app::system::memory::process_memory;
use crate::app::ui::menu_look::{MenuIcon, menu_colors, paint_menu_icon_sized};
use crate::prelude::*;
use std::collections::HashMap;

/// How many processes the list shows: the rest is no use to read in a popover.
const MAX_ROWS: usize = 200;
const REFRESH_SECONDS: f64 = 3.0;
const POPOVER_WIDTH: f32 = 260.0;

/// A process under a terminal's shell.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Proc {
    pub(crate) pid: u32,
    pub(crate) name: String,
    /// Its memory in bytes, when the system says (macOS does not yet).
    pub(crate) bytes: Option<u64>,
}

/// The popover of one terminal.
pub(crate) struct TermProcs {
    pub(crate) id: String,
    rows: Vec<Proc>,
    read_at: f64,
    /// The canvas's zoom when it opened: a zoom later leaves it over nothing of its terminal, so it closes.
    zoom: f32,
}

/// Everything that descends from `root` (not `root` itself), from a table of pid → (parent, name),
/// by pid.
pub(crate) fn descendants(table: &HashMap<u32, (u32, String)>, root: u32) -> Vec<(u32, String)> {
    let mut found: Vec<u32> = Vec::new();
    let mut todo = vec![root];
    while let Some(parent) = todo.pop() {
        for (&pid, (up, _)) in table {
            if *up == parent && pid != parent && pid != root && !found.contains(&pid) {
                found.push(pid);
                todo.push(pid);
            }
        }
    }
    found.sort_unstable();
    found.into_iter().map(|pid| (pid, table[&pid].1.clone())).collect()
}

/// The processes under the shell `root`, with their memory.
pub(crate) fn processes_of(root: u32) -> Vec<Proc> {
    descendants(&crate::transcript::process::table(), root)
        .into_iter()
        .take(MAX_ROWS)
        .map(|(pid, name)| Proc { pid, name, bytes: process_memory(pid) })
        .collect()
}

/// Ends process `pid`, with no window of its own.
pub(crate) fn stop_process(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok_and(|o| o.status.success())
    }
    #[cfg(not(windows))]
    std::process::Command::new("kill").args(["-TERM", &pid.to_string()]).output().is_ok_and(|o| o.status.success())
}

pub(crate) fn memory_text(bytes: Option<u64>) -> String {
    match bytes {
        None => "—".into(),
        Some(b) if b < 1 << 20 => "<1 MB".into(),
        Some(b) => format!("{} MB", (b as f64 / 1048576.0).round()),
    }
}

/// The button at the footer's right (`footer` is the card's folder strip).
pub(crate) fn procs_button(footer: Rect, zoom: f32) -> Rect {
    Rect::from_center_size(Pos2::new(footer.right() - 8.125 * zoom - 9.0 * zoom, footer.center().y), Vec2::new(18.0, 16.0) * zoom)
}

impl DrawingApp {
    /// The processes under terminal `id`'s shell (none while it has none).
    pub(crate) fn terminal_processes(&self, id: &str) -> Vec<Proc> {
        match self.terminals.get(id) {
            Some(Ok(session)) if !session.exited() => processes_of(session.shell_pid),
            _ => Vec::new(),
        }
    }

    /// Stops `pid` if it runs under terminal `id`'s shell; never any other process.
    pub(crate) fn stop_terminal_process(&mut self, id: &str, pid: u32) -> Result<(), String> {
        if !self.terminal_processes(id).iter().any(|p| p.pid == pid) {
            return Err(format!("O processo {pid} não roda neste terminal."));
        }
        if stop_process(pid) { Ok(()) } else { Err(format!("Não foi possível parar o processo {pid}.")) }
    }

    pub(crate) fn toggle_procs(&mut self, id: &str) {
        self.term_ui.procs = match self.term_ui.procs.take() {
            Some(p) if p.id == id => None,
            _ => Some(TermProcs { id: id.into(), rows: Vec::new(), read_at: f64::MIN, zoom: self.zoom }),
        };
    }

    /// The popover over terminal `id`'s footer button: above it, its right edge on the button's.
    pub(crate) fn procs_ui(&mut self, ctx: &Context, button: Rect) {
        let Some(id) = self.term_ui.procs.as_ref().map(|p| p.id.clone()) else { return };
        if self.term_ui.procs.as_ref().is_some_and(|p| p.zoom != self.zoom) {
            self.term_ui.procs = None;
            return;
        }
        if self.now - self.term_ui.procs.as_ref().map_or(0.0, |p| p.read_at) >= REFRESH_SECONDS {
            let rows = self.terminal_processes(&id);
            if let Some(p) = &mut self.term_ui.procs {
                (p.rows, p.read_at) = (rows, self.now);
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(REFRESH_SECONDS));
        let colors = menu_colors(self.dark());
        let rows = self.term_ui.procs.as_ref().map(|p| p.rows.clone()).unwrap_or_default();
        let total: u64 = rows.iter().filter_map(|r| r.bytes).sum();
        let mut stop = None;
        let area = egui::Area::new(egui::Id::unique("terminal_procs"))
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(Pos2::new(button.right(), button.top() - 6.5))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new().fill(colors.bg).corner_radius(6).stroke(Stroke::new(1.0, colors.border)).shadow(egui::epaint::Shadow::NONE).show(ui, |ui| {
                    ui.set_width(POPOVER_WIDTH);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let (head, _) = ui.allocate_exact_size(Vec2::new(POPOVER_WIDTH, 28.0), Sense::hover());
                    ui.painter().text(
                        head.left_center() + Vec2::new(12.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        "Processos",
                        FontId::proportional(12.0),
                        colors.text,
                    );
                    if !rows.is_empty() {
                        let count = format!("{} · {} MB", rows.len(), (total as f64 / 1048576.0).round());
                        ui.painter().text(head.right_center() - Vec2::new(12.0, 0.0), egui::Align2::RIGHT_CENTER, count, FontId::monospace(10.0), colors.muted);
                    }
                    ui.painter().hline(head.x_range(), head.bottom(), Stroke::new(1.0, colors.border));
                    if rows.is_empty() {
                        let (empty, _) = ui.allocate_exact_size(Vec2::new(POPOVER_WIDTH, 46.0), Sense::hover());
                        ui.painter().text(empty.center(), egui::Align2::CENTER_CENTER, "Nenhum processo filho", FontId::proportional(11.0), colors.muted);
                    }
                    egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
                        for (k, row) in rows.iter().enumerate() {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(POPOVER_WIDTH, 38.0), Sense::hover());
                            if k > 0 {
                                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, colors.border));
                            }
                            ui.painter().text(r.left_top() + Vec2::new(12.0, 7.0), egui::Align2::LEFT_TOP, &row.name, FontId::proportional(11.0), colors.text);
                            let detail = format!("PID {}   {}", row.pid, memory_text(row.bytes));
                            ui.painter().text(r.left_top() + Vec2::new(12.0, 21.0), egui::Align2::LEFT_TOP, detail, FontId::monospace(10.0), colors.muted);
                            let x = Rect::from_center_size(Pos2::new(r.right() - 22.0, r.center().y), Vec2::splat(20.0));
                            let response = ui.interact(x, egui::Id::unique(("stop_process", row.pid)), Sense::click());
                            if response.hovered() {
                                ui.painter().circle_filled(x.center(), 10.0, colors.hover);
                            }
                            let c = x.center();
                            let s = Stroke::new(1.3, if response.hovered() { colors.text } else { colors.muted });
                            ui.painter().add(Shape::line(vec![c + Vec2::new(-3.0, -3.0), c + Vec2::new(3.0, 3.0)], s));
                            ui.painter().add(Shape::line(vec![c + Vec2::new(-3.0, 3.0), c + Vec2::new(3.0, -3.0)], s));
                            if response.on_hover_cursor(CursorIcon::PointingHand).tip(&format!("Parar {} (PID {})", row.name, row.pid)).clicked() {
                                stop = Some(row.pid);
                            }
                        }
                    });
                });
            });
        if let Some(pid) = stop {
            match self.stop_terminal_process(&id, pid) {
                Ok(()) => {
                    if let Some(p) = &mut self.term_ui.procs {
                        p.read_at = self.now - REFRESH_SECONDS + 0.5;
                    }
                }
                Err(why) => self.toast(why),
            }
        }
        // a press away from it (and from its button, which toggles it) or Esc closes it
        let away = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !button.contains(p)));
        if away || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.term_ui.procs = None;
        }
    }
}

/// The button's icon on the card's footer: a server, in the muted color.
pub(crate) fn paint_procs_button(p: &Painter, footer: Rect, zoom: f32, color: Color32) {
    paint_menu_icon_sized(p, MenuIcon::Server, procs_button(footer, zoom).center(), color, 10.0 * zoom);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_what_descends_from_the_shell_and_not_the_shell() {
        let table: HashMap<u32, (u32, String)> = HashMap::from([
            (1, (0, "init".into())),
            (10, (1, "shell".into())),
            (11, (10, "node".into())),
            (12, (11, "esbuild".into())),
            (20, (1, "other".into())),
            (21, (20, "x".into())),
        ]);
        assert_eq!(descendants(&table, 10), vec![(11, "node".to_string()), (12, "esbuild".to_string())]);
        assert!(descendants(&table, 12).is_empty());
        // a cycle in the table (a reused pid) ends
        let cyclic: HashMap<u32, (u32, String)> = HashMap::from([(5, (6, "a".into())), (6, (5, "b".into()))]);
        assert_eq!(descendants(&cyclic, 5).len(), 1);
    }

    #[test]
    fn memory_reads_in_megabytes() {
        assert_eq!(memory_text(None), "—");
        assert_eq!(memory_text(Some(100)), "<1 MB");
        assert_eq!(memory_text(Some(5 * 1048576 + 1000)), "5 MB");
    }

    #[test]
    fn a_process_that_is_not_under_the_shell_is_never_stopped() {
        let mut app = DrawingApp::default();
        let err = app.stop_terminal_process("nobody", std::process::id()).unwrap_err();
        assert!(err.contains("não roda neste terminal"));
    }

    /// The popover is a part of its terminal on the canvas: another window enlarged over it closes
    /// it (an agent that opens a tree presses nothing, so no press away would).
    #[test]
    fn the_popover_closes_when_another_window_is_enlarged_over_its_terminal() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        let mut shell = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 400.0 });
        shell.terminal = Some(TerminalSpec::default());
        let mut ide = app.new_element(ElementKind::Rectangle { x: 700.0, y: 0.0, width: 400.0, height: 300.0 });
        ide.filetree = Some(FileTreeSpec::default());
        let (shell_id, ide_id) = (shell.id.clone(), ide.id.clone());
        app.scene.elements = vec![shell, ide];
        // no frame of the canvas ran: the enlarged frame is measured from it
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let draw = |app: &mut DrawingApp| ctx.run_ui(Default::default(), |_| app.terminal_extras(&ctx)).textures_delta.clear();
        let shown = || ctx.memory(|m| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("terminal_procs"))));
        app.toggle_procs(&shell_id);
        draw(&mut app);
        assert!(app.term_ui.procs.is_some() && shown(), "open over its terminal");
        // its own terminal enlarged keeps it
        app.set_elevated(Some(shell_id));
        draw(&mut app);
        assert!(app.term_ui.procs.is_some() && shown());
        app.set_elevated(Some(ide_id));
        draw(&mut app);
        draw(&mut app);
        assert!(app.term_ui.procs.is_none() && !shown(), "the tree went over it");
    }

    /// A zoom of the canvas leaves the popover over nothing of its terminal: it closes.
    #[test]
    fn the_popover_closes_when_the_canvas_zooms() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        let mut shell = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 400.0 });
        shell.terminal = Some(TerminalSpec::default());
        let id = shell.id.clone();
        app.scene.elements = vec![shell];
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let draw = |app: &mut DrawingApp| ctx.run_ui(Default::default(), |_| app.terminal_extras(&ctx)).textures_delta.clear();
        app.toggle_procs(&id);
        draw(&mut app);
        assert!(app.term_ui.procs.is_some());
        app.zoom *= 0.5;
        draw(&mut app);
        assert!(app.term_ui.procs.is_none());
    }

    #[test]
    fn stops_a_child_of_the_shell_and_lists_it_first() {
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 400.0 });
        e.terminal = Some(TerminalSpec::default());
        let id = e.id.clone();
        app.scene.elements.push(e);
        let (program, args): (&str, Vec<&str>) =
            if cfg!(windows) { ("cmd", vec!["/c", "ping -n 40 127.0.0.1 >nul"]) } else { ("sh", vec!["-c", "sleep 40; echo done"]) };
        let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some((program, &args)), 100).unwrap();
        app.terminals.insert(id.clone(), Ok(session));
        let mut child = None;
        for _ in 0..60 {
            child = app.terminal_processes(&id).into_iter().next();
            if child.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let child = child.expect("the shell's command shows as a process under it");
        assert!(child.pid != 0 && !child.name.is_empty());
        app.stop_terminal_process(&id, child.pid).unwrap();
        for _ in 0..60 {
            if !app.terminal_processes(&id).iter().any(|p| p.pid == child.pid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("the process is still there");
    }
}
