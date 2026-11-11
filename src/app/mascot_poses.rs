//! Claude Code's mascot in the chat, in every pose of its pack (assets/mascot/pack, NOTICE): what
//! it does while it works (reading, writing, a command, tests, git, a search, a subagent…), the
//! welcome of a new chat, the end of a turn (done, or asleep after a while) and the errors.

use crate::app::mascot::{CROP, Part, STEP, Square, parse};
use crate::prelude::*;
use std::sync::OnceLock;

macro_rules! pack {
    ($($name:literal),* $(,)?) => { [$(($name, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/mascot/pack/", $name, ".svg")))),*] };
}

const PACK: [(&str, &str); 75] = pack!(
    "angry-at-laptop",
    "angry-pixel-coffee",
    "angry-powerful-lightning",
    "api-success-pixel",
    "artboard-24cla",
    "artboard-24cla2",
    "artboard-24cla3",
    "artboard-25cla",
    "artboard-25cla2",
    "artboard-25cla3",
    "artboard-2cla",
    "artboard-2cla2",
    "artboard-2cla3",
    "artboard-3cla",
    "artboard-3cla2",
    "artboard-3cla3",
    "artboard-4cla",
    "artboard-4cla2",
    "artboard-4cla3",
    "baking-steaming-pie",
    "broken-heart-sad",
    "building-colorful-blocks",
    "confusion-spiral-pixel",
    "crying-sad-reaction",
    "data-blocks-pixel",
    "debugging-system-bugs",
    "developer-pair-programming-scene",
    "disconnected-plug-pixel",
    "dizzy-failed-state",
    "error-state-pixel",
    "examining-green-globe",
    "failed-process-error",
    "fast-performance-trail",
    "fine-under-pressure",
    "growing-pixel-nature",
    "happy-pixel-celebrating",
    "holding-ancient-scroll",
    "holding-green-leaf",
    "idea-lightbulb-pixel",
    "in-constant-loop",
    "in-glowing-aura",
    "infrastructure-server-stacks",
    "it-depends-sign",
    "lab-science-testing",
    "lifting-heavy-barbell",
    "love-cloud-pixel",
    "malware-detection-window",
    "managing-server-rack",
    "marking-critical-error",
    "multi-window-workflow",
    "nature-growth-mushrooms",
    "planting-small-trees",
    "processing-logic-symbols",
    "professional-documentation-audit",
    "reading-data-document",
    "refresh-sync-pixel",
    "repairing-with-tools",
    "reviewing-long-data-log",
    "rocket-launch-success",
    "secure-access-padlock",
    "server-stack-pixel",
    "sleeping-soundly",
    "stealth-ninja-security",
    "success-achievement-milestone",
    "system-error-alert",
    "system-monitoring-dashboard",
    "thinking-in-code",
    "thinking-pixel-bubble",
    "tracking-multiple-locations",
    "trending-up-pixel",
    "user-profile-settings",
    "visualizing-tree-data",
    "welcome-pixel-banner",
    "with-success-checkmark",
    "writing-software-code",
);

/// Thinking, before any tool.
const THINK: &[&str] = &[
    "thinking-pixel-bubble",
    "idea-lightbulb-pixel",
    "thinking-in-code",
    "artboard-4cla",
    "in-glowing-aura",
    "it-depends-sign",
    "artboard-24cla",
    "holding-green-leaf",
    "artboard-25cla3",
];
const READ: &[&str] = &["reading-data-document", "reviewing-long-data-log", "holding-ancient-scroll", "professional-documentation-audit"];
const WRITE: &[&str] = &["writing-software-code", "fast-performance-trail", "artboard-2cla"];
const EDIT: &[&str] = &["repairing-with-tools", "debugging-system-bugs", "artboard-4cla3"];
const SEARCH: &[&str] = &["examining-green-globe", "tracking-multiple-locations", "stealth-ninja-security", "data-blocks-pixel"];
const WEB: &[&str] = &["examining-green-globe", "api-success-pixel", "tracking-multiple-locations", "trending-up-pixel"];
const SHELL: &[&str] = &["processing-logic-symbols", "in-constant-loop", "refresh-sync-pixel", "system-monitoring-dashboard", "artboard-25cla2"];
/// A command that runs long.
const HEAVY: &[&str] = &["lifting-heavy-barbell", "fine-under-pressure", "angry-pixel-coffee"];
const TEST: &[&str] = &["lab-science-testing", "debugging-system-bugs", "malware-detection-window"];
const GIT: &[&str] = &["visualizing-tree-data", "planting-small-trees", "growing-pixel-nature", "nature-growth-mushrooms"];
const BUILD: &[&str] = &["building-colorful-blocks", "baking-steaming-pie", "rocket-launch-success"];
const SERVER: &[&str] = &["server-stack-pixel", "managing-server-rack", "infrastructure-server-stacks"];
const AGENT: &[&str] = &["multi-window-workflow", "developer-pair-programming-scene", "artboard-3cla", "artboard-25cla"];
const PLAN: &[&str] = &["professional-documentation-audit", "trending-up-pixel", "artboard-2cla"];
const MCP: &[&str] = &["api-success-pixel", "infrastructure-server-stacks", "artboard-3cla2"];
/// The app's own tools: the wizard.
const MAGO: &[&str] = &["artboard-2cla3"];
const SECURE: &[&str] = &["secure-access-padlock", "user-profile-settings", "artboard-24cla3"];
const DONE: &[&str] = &["with-success-checkmark", "success-achievement-milestone", "rocket-launch-success", "happy-pixel-celebrating", "love-cloud-pixel"];
const ASLEEP: &[&str] = &["sleeping-soundly", "artboard-24cla2", "artboard-3cla3"];
const ERROR: &[&str] = &[
    "error-state-pixel",
    "failed-process-error",
    "system-error-alert",
    "marking-critical-error",
    "dizzy-failed-state",
    "artboard-2cla2",
    "confusion-spiral-pixel",
    "crying-sad-reaction",
    "broken-heart-sad",
    "angry-at-laptop",
    "angry-powerful-lightning",
    "artboard-4cla2",
];
pub(crate) const WELCOME: &str = "welcome-pixel-banner";
pub(crate) const LOGGED_OUT: &str = "disconnected-plug-pixel";

/// The chat is Claude Code's: its mascot is the one shown.
pub(crate) fn claude(spec: &TerminalSpec) -> bool {
    spec.command.trim_start().starts_with("claude") || spec.name.to_lowercase().contains("claude")
}

/// One of `poses` for `seed`, the next one every `every` seconds of `secs`.
fn pick(poses: &'static [&'static str], seed: u64, secs: f64, every: f64) -> &'static str {
    poses[(seed.wrapping_add((secs.max(0.0) / every) as u64) % poses.len() as u64) as usize]
}

/// What it does while a tool runs (`tool`, given `what`, for `secs`), or thinks (None).
pub(crate) fn working(tool: Option<(&str, &str)>, secs: f64, seed: u64) -> &'static str {
    let Some((name, what)) = tool else { return pick(THINK, seed, secs, 5.0) };
    let command = what.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| command.contains(w));
    let poses = match name {
        "Read" | "NotebookRead" => READ,
        "Write" => WRITE,
        "Edit" | "MultiEdit" | "NotebookEdit" => EDIT,
        "Grep" | "Glob" | "LS" | "ToolSearch" => SEARCH,
        "WebFetch" | "WebSearch" => WEB,
        "Task" | "Agent" => AGENT,
        "TodoWrite" | "ExitPlanMode" => PLAN,
        "Bash" | "PowerShell" | "BashOutput" => {
            if has(&["test", "pytest", "jest", "vitest", "spec"]) {
                TEST
            } else if has(&["git "]) {
                GIT
            } else if has(&["build", "cargo check", "compile", "make", "tsc"]) {
                BUILD
            } else if has(&["docker", "kubectl", "ssh ", "deploy", "serve"]) {
                SERVER
            } else if has(&["login", "auth", "token", "chmod", "icacls"]) {
                SECURE
            } else if secs > 20.0 {
                HEAVY
            } else {
                SHELL
            }
        }
        _ if name.starts_with("mcp__magoagent") => MAGO,
        _ if name.starts_with("mcp__") => MCP,
        _ => SHELL,
    };
    pick(poses, seed, secs, 8.0)
}

/// The end of a turn: done, asleep after five minutes.
pub(crate) fn resting(idle_secs: f64, seed: u64) -> &'static str {
    if idle_secs > 300.0 { pick(ASLEEP, seed, 0.0, 1.0) } else { pick(DONE, seed, 0.0, 1.0) }
}

/// An error: the plug pulled when it is the login, one of the bad days otherwise.
pub(crate) fn failing(text: &str) -> &'static str {
    if crate::app::chat::error_hint(text).is_some() {
        return LOGGED_OUT;
    }
    pick(ERROR, text.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)), 0.0, 1.0)
}

/// A pose's squares, the lone ones (sparkles, dots) told from the props it holds, and the middle
/// of the whole drawing.
struct Pose {
    squares: Vec<Square>,
    lone: Vec<bool>,
    middle: Pos2,
}

fn pose(name: &str) -> Option<&'static Pose> {
    static PARSED: OnceLock<Vec<(&'static str, Pose)>> = OnceLock::new();
    let parsed = PARSED.get_or_init(|| {
        PACK.iter()
            .map(|(name, svg)| {
                let squares = parse(svg);
                // a square of the surroundings with no neighbor of its color: a sparkle, it twinkles
                let lone = squares
                    .iter()
                    .map(|s| {
                        s.part == Part::Spark
                            && !squares
                                .iter()
                                .any(|o| o.color == s.color && o.cell != s.cell && (o.cell.0 - s.cell.0).abs() <= 1.0 && (o.cell.1 - s.cell.1).abs() <= 1.0)
                    })
                    .collect();
                let bounds = squares.iter().fold(Rect::NOTHING, |r, s| r.union(Rect::from_min_size(s.at, Vec2::splat(STEP))));
                (*name, Pose { squares, lone, middle: bounds.center() })
            })
            .collect()
    });
    parsed.iter().find(|(n, _)| *n == name).map(|(_, p)| p)
}

/// The pose `name`, its middle at `center`, the mascot as big as in a `width`-wide frame (the
/// same body size in every pose); it breathes, taps its feet (`busy`), blinks and its sparkles
/// twinkle.
pub(crate) fn paint_pose(p: &Painter, name: &str, center: Pos2, width: f32, t: f64, busy: bool, alpha: f32) {
    let Some(pose) = pose(name) else { return };
    let s = width / CROP.width();
    let wave = |period: f64, phase: f64| ((t / period + phase) * std::f64::consts::TAU).sin() as f32;
    let rise = if busy { 0.18 * wave(0.9, 0.0).abs() } else { 0.1 * wave(3.0, 0.0) };
    let legs = if busy { [0.3 * wave(0.45, 0.0).max(0.0), 0.3 * wave(0.45, 0.5).max(0.0)] } else { [0.0; 2] };
    let blink = (t / 3.4).fract() < 0.13 / 3.4;
    let side = super::mascot::SIDE * s;
    for (q, lone) in pose.squares.iter().zip(&pose.lone) {
        let mut at = q.at + Vec2::splat(super::mascot::SIDE / 2.0);
        let mut size = Vec2::splat(side);
        let mut color = q.color;
        match q.part {
            Part::Leg(leg) => at.y -= legs[leg % 2] * STEP,
            Part::Spark if *lone => {
                let beat = (t * 3.0 + (q.cell.0 * 0.9 + q.cell.1 * 1.7) as f64).sin() as f32;
                color = color.gamma_multiply(0.55 + 0.45 * beat);
                at.y -= (rise + 0.15 * ((t * 2.0) as f32 + q.cell.0).sin()) * STEP;
            }
            part => {
                at.y -= rise * STEP;
                if part == Part::Eye && blink {
                    size.y *= 0.25;
                }
            }
        }
        let at = center + (at - pose.middle) * s;
        p.rect_filled(Rect::from_center_size(at, size), 0.0, color.gamma_multiply(alpha));
    }
    p.ctx().request_repaint_after(std::time::Duration::from_millis(40));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pose_of_the_pack_is_read_and_has_a_place() {
        let lists = [THINK, READ, WRITE, EDIT, SEARCH, WEB, SHELL, HEAVY, TEST, GIT, BUILD, SERVER, AGENT, PLAN, MCP, MAGO, SECURE, DONE, ASLEEP, ERROR];
        for (name, _) in PACK {
            let pose = pose(name).unwrap();
            assert!(pose.squares.len() > 60, "{name}: read whole");
            let used = lists.iter().any(|l| l.contains(&name)) || [WELCOME, LOGGED_OUT].contains(&name);
            assert!(used, "{name} shows up somewhere");
        }
        let files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/mascot/pack")).unwrap().count();
        assert_eq!(files, PACK.len(), "every file of the pack is listed");
    }

    #[test]
    fn the_pose_follows_what_it_does() {
        assert!(READ.contains(&working(Some(("Read", "src/main.rs")), 1.0, 0)));
        assert!(TEST.contains(&working(Some(("Bash", "cargo test")), 1.0, 3)));
        assert!(GIT.contains(&working(Some(("Bash", "git status")), 1.0, 3)));
        assert!(HEAVY.contains(&working(Some(("Bash", "ls -R")), 30.0, 3)));
        assert!(SHELL.contains(&working(Some(("Bash", "ls")), 1.0, 3)));
        assert_eq!(working(Some(("mcp__magoagent__app", "")), 1.0, 9), "artboard-2cla3");
        assert!(THINK.contains(&working(None, 0.0, 1)));
        assert_ne!(working(None, 0.0, 1), working(None, 5.1, 1), "the thinking turns over");
        assert_eq!(failing("Failed to authenticate: OAuth expired"), LOGGED_OUT);
        assert!(ERROR.contains(&failing("Erro da API: 529")));
        assert!(ASLEEP.contains(&resting(600.0, 2)) && DONE.contains(&resting(10.0, 2)));
    }
}
