//! A scripted mouse and keyboard, to check the interface while the screen is locked (macOS then
//! blocks outside input but the app keeps drawing). Steps, separated by `;` or new lines, in
//! window points; MAGOAGENT_SCRIPT holds them or the path of a file with them:
//!   move x y · click x y · dclick x y · down x y · up x y · drag x1 y1 x2 y2
//!   type texto · key J cmd shift alt · wait ms · shot /caminho.png · quit
//! They go in through raw input, the same path as a real mouse. `shot` saves what egui drew;
//! native web pages are not in it (capture the window from outside for those).

use crate::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct Script {
    steps: VecDeque<String>,
    until: f64,
    /// A `shot` waiting for its pixels.
    shooting: Arc<AtomicBool>,
}

impl Script {
    pub(crate) fn from_env() -> Option<Script> {
        let text = std::env::var("MAGOAGENT_SCRIPT").ok()?;
        Some(Script::new(&std::fs::read_to_string(&text).unwrap_or(text)))
    }

    pub(crate) fn new(text: &str) -> Script {
        let steps = text.split([';', '\n']).map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')).map(String::from).collect();
        Script { steps, until: 0.0, shooting: Default::default() }
    }

    /// One step per frame, into this frame's input. Presses and releases land on separate
    /// frames, as a hand would.
    pub(crate) fn feed(&mut self, ctx: &Context, raw: &mut egui::RawInput) {
        let now = raw.time.unwrap_or(0.0);
        let shooting = self.shooting.load(Ordering::Relaxed);
        if self.steps.is_empty() && !shooting {
            return;
        }
        ctx.request_repaint(); // frames keep coming while there are steps
        if shooting || now < self.until {
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
            ["quit"] => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => eprintln!("roteiro: passo desconhecido: {step}"),
        }
    }
}
