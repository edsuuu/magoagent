//! Who is who among the agents: each one's mascot in a colour of its own, the same in every place it shows, and a name no
//! other agent on the same list shows.

use crate::prelude::*;
use std::collections::{HashMap, HashSet};

/// The colours an agent's mascot can have: hues well apart, mid tones that read on the dark and the light looks and
/// under the mini's dark eyes. Luna's blue (`LUME`) is not among them: it stays hers.
pub(crate) const AGENT_PALETTE: [u32; 9] = [0xef4444, 0xf97316, 0xeab308, 0x84cc16, 0x22c55e, 0x14b8a6, 0x818cf8, 0xa855f7, 0xec4899];

/// A colour of a program's own (Claude Desktop's orange, Hermes' purple) this near a palette's one (CIE76, in CIELAB)
/// reads as it at a mini's size: Claude Desktop's sits between the red and the orange, Hermes' between the indigo and
/// the purple, Cursor's grey near none.
const ALIKE: f32 = 40.0;

/// The colour of `id` when nobody else has it: the same from one run to the next, whatever the order of the lists.
pub(crate) fn own_color(id: &str) -> Color32 {
    rgb_hex(AGENT_PALETTE[(fnv1a(id) % AGENT_PALETTE.len() as u64) as usize])
}

/// A colour in CIELAB (D65), where distance is about how far apart two colours look.
fn lab(color: Color32) -> [f32; 3] {
    let linear = egui::Rgba::from(color);
    let (red, green, blue) = (linear.r(), linear.g(), linear.b());
    let x = (0.4124 * red + 0.3576 * green + 0.1805 * blue) / 0.950_47;
    let y = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
    let z = (0.0193 * red + 0.1192 * green + 0.9505 * blue) / 1.088_83;
    let curve = |value: f32| if value > 0.008_856 { value.cbrt() } else { 7.787 * value + 16.0 / 116.0 };
    let (x, y, z) = (curve(x), curve(y), curve(z));
    [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
}

fn distance(one: Color32, other: Color32) -> f32 {
    let (one, other) = (lab(one), lab(other));
    (0..3).map(|axis| (one[axis] - other[axis]).powi(2)).sum::<f32>().sqrt()
}

/// `taken` keeps `color` from another agent: the same colour of the palette, or a program's own that looks like it.
fn blocks(taken: Color32, color: Color32) -> bool {
    match AGENT_PALETTE.iter().any(|value| rgb_hex(*value) == taken) {
        true => taken == color,
        false => distance(taken, color) < ALIKE,
    }
}

/// `own`, or the palette's next colour after it that none of `taken` keeps (`blocks`); all taken, `own`.
pub(crate) fn free_color(own: Color32, taken: &[Color32]) -> Color32 {
    let start = AGENT_PALETTE.iter().position(|value| rgb_hex(*value) == own).unwrap_or(0);
    (0..AGENT_PALETTE.len())
        .map(|step| rgb_hex(AGENT_PALETTE[(start + step) % AGENT_PALETTE.len()]))
        .find(|color| !taken.iter().any(|other| blocks(*other, *color)))
        .unwrap_or(own)
}

/// The colours of the agents alive (`ids`): the ones `kept` had stay (an agent keeps its colour while others come and
/// go); each new one, by its id's order, gets its own or the next free one, so no two look alike while the palette has
/// room. `taken`: the colours others have already (a session outside the app in its program's), which no agent keeps.
pub(crate) fn agent_colors(ids: &[String], kept: &HashMap<String, Color32>, taken: &[Color32]) -> HashMap<String, Color32> {
    let free = |color: &Color32| !taken.iter().any(|other| blocks(*other, *color));
    let mut colors: HashMap<String, Color32> = ids.iter().filter_map(|id| Some((id.clone(), *kept.get(id).filter(|color| free(color))?))).collect();
    let mut newcomers: Vec<&String> = ids.iter().filter(|id| !colors.contains_key(*id)).collect();
    newcomers.sort();
    newcomers.dedup();
    for id in newcomers {
        let used: Vec<Color32> = taken.iter().copied().chain(colors.values().copied()).collect();
        colors.insert(id.clone(), free_color(own_color(id), &used));
    }
    colors
}

/// `names` with no two alike (any case): the first keeps its own, the next ones of the same take " 2", " 3"… one no
/// other name has.
pub(crate) fn distinct_names(names: &[String]) -> Vec<String> {
    let all: HashSet<String> = names.iter().map(|name| name.to_lowercase()).collect();
    let mut given: HashSet<String> = HashSet::new();
    names
        .iter()
        .map(|name| {
            if given.insert(name.to_lowercase()) {
                return name.clone();
            }
            let free = |candidate: &String| !all.contains(&candidate.to_lowercase()) && !given.contains(&candidate.to_lowercase());
            let distinct = (2..).map(|count| format!("{name} {count}")).find(free).unwrap_or_default();
            given.insert(distinct.to_lowercase());
            distinct
        })
        .collect()
}

/// The names of a list's `items` (its `name`), by the list's order, made `distinct_names`.
pub(crate) fn make_distinct<T>(items: &mut [T], name: impl Fn(&mut T) -> &mut String) {
    let names: Vec<String> = items.iter_mut().map(|item| name(item).clone()).collect();
    for (item, distinct) in items.iter_mut().zip(distinct_names(&names)) {
        *name(item) = distinct;
    }
}

/// A folder's name short enough for a row: a long hexadecimal run (a worktree's "agent-a47cc47699b17a639") cut to its
/// first 7, as git shortens a commit; the rest as it is.
pub(crate) fn short_name(name: &str) -> String {
    let mut short = String::new();
    let mut run = String::new();
    let flush = |short: &mut String, run: &mut String| {
        short.push_str(if run.len() >= 12 { &run[..7] } else { run });
        run.clear();
    };
    for character in name.chars() {
        if character.is_ascii_hexdigit() {
            run.push(character);
        } else {
            flush(&mut short, &mut run);
            short.push(character);
        }
    }
    flush(&mut short, &mut run);
    short
}

/// The agents' colours as last shared out (`DrawingApp::agent_color`), and the programs' own colours they were shared
/// around.
#[derive(Default)]
pub(crate) struct AgentColors {
    taken: Vec<Color32>,
    colors: HashMap<String, Color32>,
}

impl DrawingApp {
    /// The colour of agent `id`'s mascot wherever it shows (the lists, the island, its conversation and card): kept while
    /// it lives; somebody new, or a session in its program's colour, has everybody's worked out again, the ones there
    /// keeping theirs (unless that colour is now the program's).
    pub(crate) fn agent_color(&self, id: &str) -> Color32 {
        let taken = self.outside_brands();
        let cached = self.agent_colors.borrow().colors.get(id).copied().filter(|_| self.agent_colors.borrow().taken == taken);
        if let Some(color) = cached {
            return color;
        }
        let mut ids = self.outside_agent_ids();
        ids.extend(self.agent_rows().into_iter().chain(self.draft_rows()).map(|row| row.id));
        let mut colors = agent_colors(&ids, &self.agent_colors.borrow().colors, &taken);
        // one that is no agent alive (a test's, one just removed): its own, until the next newcomer
        let color = *colors.entry(id.to_string()).or_insert_with(|| own_color(id));
        *self.agent_colors.borrow_mut() = AgentColors { taken, colors };
        color
    }

    /// Chat `id`'s name as its card on the canvas says it: no other chat's (the first of the list keeps its own, as in
    /// the lists). None for what is no chat.
    pub(crate) fn chat_card_name(&self, id: &str) -> Option<String> {
        let chats: Vec<&crate::app::chat::direct::Chat> = self.chats.iter().filter(|chat| !chat.draft).collect();
        let index = chats.iter().position(|chat| chat.id == id)?;
        distinct_names(&chats.iter().map(|chat| chat.name.clone()).collect::<Vec<_>>()).into_iter().nth(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agents_colour_comes_from_its_id_and_stays_the_same() {
        assert_eq!(own_color("c0ffee"), own_color("c0ffee"), "the same from one run to the next");
        assert!(AGENT_PALETTE.iter().any(|value| rgb_hex(*value) == own_color("c0ffee")));
        // the order of the list changes nothing
        let ids: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let reversed: Vec<String> = ids.iter().rev().cloned().collect();
        assert_eq!(agent_colors(&ids, &HashMap::new(), &[]), agent_colors(&reversed, &HashMap::new(), &[]));
        // one already coloured keeps its colour when a newcomer wants the same
        let first = agent_colors(&["x".to_string()], &HashMap::new(), &[]);
        let clash = (0..200).map(|index| format!("novo-{index}")).find(|id| own_color(id) == first["x"]).unwrap();
        let both = agent_colors(&["x".to_string(), clash.clone()], &first, &[]);
        assert_eq!(both["x"], first["x"]);
        assert_ne!(both[&clash], first["x"]);
    }

    #[test]
    fn agents_alive_together_never_share_a_colour_while_the_palette_has_room() {
        let ids: Vec<String> = (0..AGENT_PALETTE.len()).map(|index| format!("agente-{index}")).collect();
        let colors = agent_colors(&ids, &HashMap::new(), &[]);
        let distinct: HashSet<[u8; 4]> = colors.values().map(|color| color.to_array()).collect();
        assert_eq!(distinct.len(), AGENT_PALETTE.len(), "{colors:?}");
        // a session outside the app in Claude Desktop's orange: no agent takes the red nor the orange by it
        let (desktop, red, orange) = (rgb_hex(0xd97757), rgb_hex(0xef4444), rgb_hex(0xf97316));
        let colors = agent_colors(&ids[..4], &HashMap::new(), &[desktop]);
        assert!(colors.values().all(|color| *color != red && *color != orange), "{colors:?}");
        assert_eq!(free_color(red, &[desktop]), rgb_hex(0xeab308), "past the red and the orange");
        assert_eq!(free_color(red, &[orange]), red, "the palette's own only keep themselves");
        // all taken: its own
        let all: Vec<Color32> = AGENT_PALETTE.iter().map(|value| rgb_hex(*value)).collect();
        assert_eq!(free_color(own_color("z"), &all), own_color("z"));
    }

    #[test]
    fn the_palette_is_far_apart_and_none_is_lunas_blue() {
        for (index, one) in AGENT_PALETTE.iter().enumerate() {
            assert!(distance(rgb_hex(*one), crate::app::mascot::mago::LUME) > ALIKE, "{one:06x}");
            for other in &AGENT_PALETTE[index + 1..] {
                assert!(distance(rgb_hex(*one), rgb_hex(*other)) > 30.0, "{one:06x} {other:06x}");
            }
        }
        // the programs' own: Claude Desktop's keeps two, Cursor's grey none
        let kept = |brand: u32| AGENT_PALETTE.iter().filter(|value| blocks(rgb_hex(brand), rgb_hex(**value))).count();
        assert_eq!((kept(0xd97757), kept(0xc084fc), kept(0xc0c4cc)), (2, 2, 0));
    }

    #[test]
    fn two_agents_never_show_the_same_name() {
        let names = ["Luna", "luna", "Luna 2", "Faísca", "Luna"].map(String::from);
        assert_eq!(distinct_names(&names), ["Luna", "luna 3", "Luna 2", "Faísca", "Luna 4"]);
        assert_eq!(distinct_names(&["a".into(), "b".into()]), ["a", "b"]);
        assert_eq!(short_name("agent-a47cc47699b17a639"), "agent-a47cc47");
        assert_eq!(short_name("magoagent"), "magoagent");
        assert_eq!(short_name("cafe-1234"), "cafe-1234", "a short run stays");
    }
}
