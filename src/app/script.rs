//! A scripted mouse and keyboard, to check the interface while the screen is locked (macOS then
//! blocks outside input but the app keeps drawing). Steps, separated by `;` or new lines, in
//! window points; MAGOAGENT_SCRIPT holds them or the path of a file with them:
//!   move x y · click x y · rclick x y · dclick x y · down x y · up x y · drag x1 y1 x2 y2
//!   type texto · key J cmd shift alt · wait ms · shot /caminho.png · size w h · minimize · quit
//! They go in through raw input, the same path as a real mouse. `shot` saves what egui drew;
//! native web pages are not in it (capture the window from outside for those). A `wait` lets the
//! app idle as it would, so MAGOAGENT_PERF (`Perf`) measures it.

use crate::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct Script {
    steps: VecDeque<String>,
    until: f64,
    /// A `shot` waiting for its pixels.
    shooting: Arc<AtomicBool>,
    /// The path of the last `shot`, so the app also saves the native pages next to it (macOS).
    #[cfg(target_os = "macos")]
    pages: Option<String>,
}

/// MAGOAGENT_PERF=<file>: once a second, the frames painted, the CPU they took (mean and worst,
/// in ms) and what asked for them (`file:line`), appended to the file as a tab-separated line.
#[derive(Default)]
pub(crate) struct Perf {
    pub(crate) path: std::path::PathBuf,
    since: f64,
    frames: u32,
    cpu_ms: f32,
    worst_ms: f32,
    causes: std::collections::BTreeSet<String>,
}

impl Perf {
    pub(crate) fn from_env() -> Option<Perf> {
        Some(Perf { path: std::env::var_os("MAGOAGENT_PERF")?.into(), ..Default::default() })
    }

    /// One frame at `now`: what the last one cost (eframe's `cpu_usage`) and who asked for it.
    pub(crate) fn frame(&mut self, now: f64, cpu: Option<f32>, causes: &[egui::RepaintCause]) {
        let ms = cpu.unwrap_or(0.0) * 1000.0;
        self.frames += 1;
        self.cpu_ms += ms;
        self.worst_ms = self.worst_ms.max(ms);
        self.causes.extend(causes.iter().map(|c| format!("{}:{}", c.file, c.line)));
        if now - self.since < 1.0 {
            return;
        }
        let head = if self.since == 0.0 { "tempo\tquadros\tcpu_med_ms\tcpu_max_ms\tcausas\n" } else { "" };
        let causes = std::mem::take(&mut self.causes).into_iter().collect::<Vec<_>>().join(", ");
        let line = format!("{head}{now:.1}\t{}\t{:.2}\t{:.2}\t{causes}\n", self.frames, self.cpu_ms / self.frames as f32, self.worst_ms);
        use std::io::Write;
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&self.path).and_then(|mut f| f.write_all(line.as_bytes()));
        (self.since, self.frames, self.cpu_ms, self.worst_ms) = (now, 0, 0.0, 0.0);
    }
}

impl Script {
    pub(crate) fn from_env() -> Option<Script> {
        let text = std::env::var("MAGOAGENT_SCRIPT").ok()?;
        Some(Script::new(&std::fs::read_to_string(&text).unwrap_or(text)))
    }

    /// The path of a `shot` that asked for the native pages too, once.
    #[cfg(target_os = "macos")]
    pub(crate) fn take_pages(&mut self) -> Option<String> {
        self.pages.take()
    }

    pub(crate) fn new(text: &str) -> Script {
        let steps = text.split([';', '\n']).map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')).map(String::from).collect();
        Script { steps, until: 0.0, shooting: Default::default(), #[cfg(target_os = "macos")] pages: None }
    }

    /// One step per frame, into this frame's input. Presses and releases land on separate
    /// frames, as a hand would.
    pub(crate) fn feed(&mut self, ctx: &Context, raw: &mut egui::RawInput) {
        let now = raw.time.unwrap_or(0.0);
        let shooting = self.shooting.load(Ordering::Relaxed);
        if self.steps.is_empty() && !shooting {
            return;
        }
        // frames keep coming while there are steps; a `wait` sleeps until its end
        if !shooting && now < self.until {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(self.until - now));
            return;
        }
        ctx.request_repaint();
        if shooting {
            return;
        }
        let Some(step) = self.steps.pop_front() else { return };
        let words: Vec<&str> = step.split_whitespace().collect();
        let n = |i: usize| words.get(i).and_then(|w| w.parse::<f32>().ok()).unwrap_or(0.0);
        let at = Pos2::new(n(1), n(2));
        let button = |pressed: bool, pos: Pos2| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        match words.as_slice() {
            ["move", ..] => raw.events.push(egui::Event::PointerMoved(at)),
            ["down", ..] => raw.events.extend([egui::Event::PointerMoved(at), button(true, at)]),
            ["up", ..] => raw.events.extend([egui::Event::PointerMoved(at), button(false, at)]),
            ["click", ..] => {
                raw.events.extend([egui::Event::PointerMoved(at), button(true, at)]);
                self.steps.push_front(format!("up {} {}", at.x, at.y));
            }
            ["rclick", ..] => {
                // the secondary button: pressed, then released on the next frame, as `click` does
                raw.events.extend([egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }]);
                self.steps.push_front(format!("rup {} {}", at.x, at.y));
            }
            ["rup", ..] => raw.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }),
            ["dclick", ..] => {
                raw.events.extend([egui::Event::PointerMoved(at), button(true, at)]);
                for step in [format!("up {} {}", at.x, at.y), format!("down {} {}", at.x, at.y), format!("up {} {}", at.x, at.y)].into_iter().rev() {
                    self.steps.push_front(step);
                }
            }
            ["scroll", ..] => {
                // a wheel step of dy points over (x, y): positive scrolls the content down
                raw.events.extend([
                    egui::Event::PointerMoved(at),
                    egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, n(3)), phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Wheel, modifiers: Modifiers::NONE },
                ]);
            }
            ["drag", ..] => {
                // pressed at the start, ten moves along the way, released at the end
                let to = Pos2::new(n(3), n(4));
                raw.events.extend([egui::Event::PointerMoved(at), button(true, at)]);
                self.steps.push_front(format!("up {} {}", to.x, to.y));
                for k in (1..=10).rev() {
                    let p = at.lerp(to, k as f32 / 10.0);
                    self.steps.push_front(format!("move {} {}", p.x, p.y));
                }
            }
            ["type", ..] => raw.events.push(egui::Event::Text(step["type".len()..].trim_start().to_string())),
            ["key", name, mods @ ..] => {
                let Some(key) = Key::from_name(name) else {
                    eprintln!("roteiro: tecla desconhecida: {name}");
                    return;
                };
                let mut modifiers = Modifiers::NONE;
                for m in mods {
                    match *m {
                        "cmd" => modifiers = modifiers | Modifiers::COMMAND,
                        "shift" => modifiers = modifiers | Modifiers::SHIFT,
                        "alt" => modifiers = modifiers | Modifiers::ALT,
                        "ctrl" => modifiers = modifiers | Modifiers::CTRL,
                        _ => {}
                    }
                }
                let key_event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
                raw.events.extend([key_event(true), key_event(false)]);
            }
            ["wait", ..] => self.until = now + n(1) as f64 / 1000.0,
            ["shot", path] => {
                let (path, shooting) = (path.to_string(), self.shooting.clone());
                #[cfg(target_os = "macos")]
                {
                    self.pages = Some(path.clone());
                }
                shooting.store(true, Ordering::Relaxed);
                ctx.request_screenshot(move |image| {
                    let pixels = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    let saved = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, pixels).map(|i| i.save(&path));
                    if !matches!(saved, Some(Ok(()))) {
                        eprintln!("roteiro: não salvei {path}");
                    }
                    shooting.store(false, Ordering::Relaxed);
                });
            }
            ["minimize"] => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            // below the window's minimum too, to see the narrow layouts
            ["size", ..] => {
                let size = Vec2::new(n(1) as f32, n(2) as f32);
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(size));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            }
            ["quit"] => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => eprintln!("roteiro: passo desconhecido: {step}"),
        }
    }
}
