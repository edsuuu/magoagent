//! macOS with SwiftUI drawing the island (`native::enabled("island")`): the egui island steps aside and this sends Swift
//! what it shows (topic "island", built from the same `island_view` the egui one paints; "island.live" for what moves
//! every frame) and does what Swift asks (`ui.island.*`). Swift keeps the open/compact/closed clock (`fsm.rs` ported);
//! what the app forces (`force_home`, `force_petit`, `reveal`, from an ask, a turn that ended, the MCP, a shortcut) lands on
//! `island.machine`, which nothing steps here: it is read as an inbox each frame and handed to Swift as an event.

use super::paint::header_tabs;
use super::{Line, Mood, Tab, done, feed, fsm, main_state, mood, sounds};
use crate::app::chat::decide::Question;
use crate::app::integrations::ITEM_PREFIX;
use crate::app::maestro::outside::{Badge, PILL_PREFIX};
use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::ChatItem;
use std::cell::RefCell;
use std::sync::atomic::Ordering;

/// What the island shows, for Swift.
#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    /// Off (turned off, alerts paused, the welcome on): no island.
    pub(crate) on: bool,
    pub(crate) mood: &'static str,
    pub(crate) count: usize,
    /// The island's own wizard (`main_state`).
    pub(crate) state: &'static str,
    pub(crate) celebrating: bool,
    /// An agent finished since the island was last open: Luna wears a check (`done.rs`).
    pub(crate) finished: bool,
    pub(crate) lines: Vec<LineOut>,
    /// Início's list (ids, in order) and how many are hidden.
    pub(crate) feed: Vec<String>,
    pub(crate) hidden: usize,
    pub(crate) tabs: Vec<TabOut>,
    pub(crate) tab: &'static str,
    pub(crate) settings: bool,
    pub(crate) focus: Option<String>,
    /// An ask holds it open on its card (not folded by "Depois").
    pub(crate) pinned: bool,
    pub(crate) ask: Option<AskOut>,
    pub(crate) activity: Option<ActivityOut>,
    pub(crate) glimpse: Vec<Said>,
    pub(crate) chat: Option<ChatOut>,
    pub(crate) voice: bool,
    pub(crate) voice_state: &'static str,
    pub(crate) voice_note: String,
    pub(crate) sound: bool,
    pub(crate) volume: f32,
    pub(crate) close_after: u8,
    pub(crate) compact_after: u16,
    pub(crate) color: String,
    pub(crate) claude_hooks: bool,
    pub(crate) song: Option<SongOut>,
    pub(crate) luna_dances: bool,
    pub(crate) luna_color: String,
    pub(crate) github: Option<GithubOut>,
    pub(crate) projects: Vec<String>,
    /// Where it hangs from, as saved (screen points from the main screen's top left); none: the first place.
    pub(crate) at: Option<[f32; 2]>,
    pub(crate) talk_size: Option<[f32; 2]>,
    /// What the app forced since (`seq` grows by one each): the stage ("home", "petit", "greeting"; "reveal": peek only if closed) and a place asked
    /// in Ajustes or by the MCP ("top", "bottom", "left", "right").
    pub(crate) seq: u64,
    pub(crate) forced: Option<&'static str>,
    pub(crate) place: Option<&'static str>,
}

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LineOut {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) detail: String,
    /// "working", "active", "waiting", "idle", "ended".
    pub(crate) state: &'static str,
    pub(crate) mago: &'static str,
    pub(crate) color: String,
    pub(crate) chat: bool,
    pub(crate) canvas: bool,
    pub(crate) badge: Option<&'static str>,
    pub(crate) kind: String,
    pub(crate) at: i64,
    pub(crate) asleep: bool,
    /// The page it lives on (`Tab::of`) and whether Claude Code's page shows it too.
    pub(crate) tab: &'static str,
    pub(crate) claude: bool,
    pub(crate) outside: bool,
    pub(crate) tool: bool,
    /// An integration's card buttons: (label, action).
    pub(crate) chips: Vec<(String, String)>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabOut {
    pub(crate) tab: &'static str,
    pub(crate) tip: &'static str,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AskOut {
    pub(crate) chat: String,
    pub(crate) name: Option<String>,
    pub(crate) color: Option<String>,
    pub(crate) approval: bool,
    pub(crate) doing: String,
    pub(crate) summary: String,
    pub(crate) details: String,
    pub(crate) outside: bool,
    pub(crate) queued: usize,
    pub(crate) step: usize,
    pub(crate) questions: Vec<QuestionOut>,
    pub(crate) picked: Vec<Vec<usize>>,
    pub(crate) other: Vec<String>,
    /// The permission opened to read the whole request ("ver tudo").
    pub(crate) open: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionOut {
    pub(crate) text: String,
    pub(crate) header: String,
    pub(crate) multiple: bool,
    pub(crate) options: Vec<(String, String)>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivityOut {
    /// "working", "editing", "done", "failed".
    pub(crate) phase: &'static str,
    pub(crate) verb: String,
    pub(crate) target: String,
    pub(crate) file: Option<String>,
    pub(crate) place: (usize, usize),
    pub(crate) moves: Vec<Step>,
    pub(crate) patch: Vec<String>,
    pub(crate) patch_line: usize,
    pub(crate) diff: Option<(usize, usize)>,
    pub(crate) output: Vec<String>,
    pub(crate) headline: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatOut {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) working: bool,
    /// A session outside the app shown instead (its pill's id).
    pub(crate) session: Option<String>,
    /// What was said, the last ones: (the person, the words).
    pub(crate) said: Vec<Said>,
}

/// A line of text: something said (`person`: by the person) or a line of the file read (`number`).
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Said {
    pub(crate) person: bool,
    pub(crate) number: usize,
    pub(crate) text: String,
}

/// A tool of the turn: its name, running, failed.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Step {
    pub(crate) verb: String,
    pub(crate) running: bool,
    pub(crate) failed: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SongOut {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) cover_url: String,
    pub(crate) playing: bool,
    pub(crate) shuffle: Option<bool>,
    pub(crate) repeat: Option<String>,
    pub(crate) volume: Option<f32>,
    pub(crate) position_ms: Option<u64>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) read_at_ms: i64,
    pub(crate) liked: Option<bool>,
    pub(crate) device: String,
    pub(crate) track_uri: String,
    pub(crate) tint: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubOut {
    pub(crate) login: String,
    pub(crate) error: Option<String>,
    pub(crate) prs: Vec<PullOut>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullOut {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) page: String,
    pub(crate) draft: bool,
    /// "pending", "success", "failure", "unknown"; "open", "merged", "closed".
    pub(crate) ci: &'static str,
    pub(crate) state: &'static str,
    pub(crate) review: &'static str,
    pub(crate) head_ref: String,
    pub(crate) base_ref: String,
    pub(crate) updated_at: i64,
    pub(crate) conflict: bool,
    pub(crate) needs_you: bool,
    pub(crate) mine: bool,
    pub(crate) failing: Vec<String>,
}

/// What moves every frame, sent on its own ("island.live") so the rest is not sent again for it.
#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Live {
    pub(crate) mic: f32,
    pub(crate) bpm: f32,
    pub(crate) phase: f32,
    /// The beats counted so far (`phase` is its fraction): the dance's moves longer than a beat.
    pub(crate) beat: f64,
    pub(crate) energy: f32,
    pub(crate) bass: f32,
    pub(crate) dance: String,
    pub(crate) bands: Option<Vec<f32>>,
}

/// The forced stages handed to Swift so far, and when Claude Code's hooks were last looked for.
// ponytail: one island per process (a thread local), as the app has; move it into `Island` if it ever has two
#[derive(Default)]
struct Inbox {
    seq: u64,
    forced: Option<&'static str>,
    place: Option<&'static str>,
    hooks: Option<(f64, bool)>,
}

thread_local! {
    static INBOX: RefCell<Inbox> = RefCell::new(Inbox::default());
}

pub(crate) fn tab_name(tab: Tab) -> &'static str {
    match tab {
        Tab::Home => "home",
        Tab::Chats => "chats",
        Tab::Chat => "chat",
        Tab::New => "new",
        Tab::Claude => "claude",
        Tab::GitHub => "github",
        Tab::Music => "music",
    }
}

fn state_name(state: AgentState) -> &'static str {
    match state {
        AgentState::Working(_) => "working",
        AgentState::Active(_) => "active",
        AgentState::Waiting => "waiting",
        AgentState::Idle(_) => "idle",
        AgentState::Ended => "ended",
    }
}

pub(crate) fn mago_name(state: MagoState) -> &'static str {
    match state {
        MagoState::Idle => "idle",
        MagoState::Working => "working",
        MagoState::NeedsYou => "needs_you",
        MagoState::Asking => "asking",
        MagoState::Done => "done",
        MagoState::Error => "error",
        MagoState::Sleeping => "sleeping",
        MagoState::Listening => "listening",
        MagoState::Speaking => "speaking",
        MagoState::Waving => "waving",
    }
}

fn hex(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

fn badge_name(badge: Option<Badge>) -> Option<&'static str> {
    badge.map(|badge| match badge {
        Badge::Approval => "approval",
        Badge::Finished => "finished",
        Badge::Error => "error",
    })
}

fn line_out(line: &Line, chips: Vec<(String, String)>) -> LineOut {
    LineOut {
        id: line.id.clone(),
        name: line.name.clone(),
        detail: line.detail.clone(),
        state: state_name(line.state),
        mago: mago_name(line.mago),
        color: hex(line.color),
        chat: line.chat,
        canvas: line.canvas,
        badge: badge_name(line.badge),
        kind: line.kind.clone(),
        at: line.at,
        asleep: line.asleep,
        tab: tab_name(Tab::of(&line.id)),
        claude: Tab::Claude.shows(line),
        outside: line.id.starts_with(PILL_PREFIX),
        tool: line.id.starts_with(ITEM_PREFIX),
        chips,
    }
}

fn question_out(question: &Question) -> QuestionOut {
    QuestionOut { text: question.text.clone(), header: question.header.clone(), multiple: question.multiple, options: question.options.clone() }
}

/// The Swift name of a forced stage.
fn stage_name(stage: fsm::Stage) -> Option<&'static str> {
    match stage {
        fsm::Stage::Hidden => None,
        fsm::Stage::Petit => Some("petit"),
        fsm::Stage::Home => Some("home"),
        fsm::Stage::Greeting => Some("greeting"),
    }
}

impl DrawingApp {
    /// Every frame, with SwiftUI drawing the island: what it shows goes to Swift, and the sounds of what happened play.
    pub(crate) fn native_island(&mut self) {
        let snapshot = self.native_island_snapshot();

        crate::app::native::publish("island", &snapshot);
        let groove = self.luna_groove();

        let live = Live {
            mic: f32::from_bits(self.voice.shared.level.load(Ordering::Relaxed)).min(1.0),
            bpm: groove.as_ref().map_or(0.0, |groove| groove.bpm),
            phase: groove.as_ref().map_or(0.0, |groove| groove.phase),
            beat: groove.as_ref().map_or(0.0, |groove| groove.beat),
            energy: groove.as_ref().map_or(0.0, |groove| groove.energy),
            bass: groove.as_ref().map_or(0.0, |groove| groove.bass),
            dance: groove.as_ref().map_or(String::new(), |groove| format!("{:?}", groove.dance).to_lowercase()),
            bands: groove.and_then(|groove| groove.bands).map(|bands| bands.to_vec()),
        };

        crate::app::native::publish("island.live", &live);
    }

    /// The snapshot of this frame; what changed since the last one (a turn ended, an ask came or went, everything
    /// finished, something failed) also sounds and forces the island open, as `island_ui` does.
    pub(crate) fn native_island_snapshot(&mut self) -> Snapshot {
        let now = crate::app::system::tray::unix_seconds();
        let paused = crate::app::system::tray::alerts_paused(self.prefs.pause_alerts_until, now);

        if paused || !self.prefs.island || self.welcome.is_some() {
            self.island = super::Island::default();

            return Snapshot::default();
        }

        let time = self.now;
        let rows: Vec<AgentRow> = self.agent_rows().into_iter().filter(|row| row.state != AgentState::Ended).collect();
        let asks: Vec<String> = self.person_asks.borrow().iter().map(|ask| ask.chat.clone()).collect();
        let outside = self.outside_pills();
        let mood = mood(rows.iter().map(|row| &row.state).chain(outside.iter().map(|pill| &pill.state)));

        self.island.folded.retain(|chat| asks.contains(chat));
        let pinned = asks.iter().any(|chat| !self.island.folded.contains(chat));
        let mut tones: Vec<crate::voice::Sound> = Vec::new();

        if std::mem::take(&mut self.island.chat_asked) {
            (self.island.tab, self.island.settings) = (Tab::Chat, false);
            self.island.machine.force_home(time);
        }

        if let Some((alert, at)) = self.integrations.github.ring.take()
            && now_ms() - at < 10_000
        {
            match alert {
                crate::app::integrations::github::Alert::ReviewRequested(_) => tones.push(sounds::ASK),
                crate::app::integrations::github::Alert::CheckPassed(_) => tones.push(sounds::FINISH),
                _ => {}
            }
        }

        let music_on = self.spotify_sources() != crate::app::integrations::spotify::Sources::default();
        let github_on = self.prefs.integrations.github;
        let island = &mut self.island;

        if (island.tab == Tab::Music && !music_on) || (island.tab == Tab::GitHub && !github_on) {
            island.tab = Tab::Home;
        }

        // an agent got busy: the island peeks if it was closed (`reveal`), which only Swift knows
        let busy = matches!(mood, Mood::Needs(_) | Mood::Working(_));
        let revealed = busy && !island.was_busy;

        // the end is celebrated quietly, as `island_ui`: no chime, a check on Luna until the island is opened
        if island.was_busy && mood == Mood::Ready {
            island.celebrate_until = time + 6.0;
        }

        island.was_busy = busy;
        island.finished |= done::turn_done(&island.working, &rows);

        if done::turn_waits(&island.working, &rows) {
            island.machine.force_home(time);
        }

        island.working = rows.iter().filter(|row| matches!(row.state, AgentState::Working(_))).map(|row| row.id.clone()).collect();
        let asked_before = std::mem::replace(&mut island.asked, asks.len());

        if asks.len() < asked_before {
            tones.push(sounds::APPROVE);
        }

        if asks.len() > asked_before {
            tones.push(sounds::ASK);
        }

        let (forced, place) = (stage_name(island.machine.stage()), island.place_ask.take());
        let forced = forced.or(revealed.then_some("reveal"));

        if forced.is_some() {
            // read as an inbox: back to rest, so the next force is seen too
            island.machine = fsm::Machine::default();
        }

        let ask = self.native_ask(&asks);
        let view = self.island_view(rows, asks, mood, time);

        if view.failures > std::mem::replace(&mut self.island.failures, view.failures) {
            tones.push(sounds::ERROR);
        }

        for tone in tones {
            self.island_chime(tone, time);
        }

        let place = place.map(|place| match place {
            super::Place::Top => "top",
            super::Place::Bottom => "bottom",
            super::Place::Left => "left",
            super::Place::Right => "right",
        });

        let claude_hooks = self.native_hooks(time);

        let (seq, forced, place) = INBOX.with(|inbox| {
            let mut inbox = inbox.borrow_mut();

            if forced.is_some() || place.is_some() {
                (inbox.seq, inbox.forced, inbox.place) = (inbox.seq + 1, forced, place);
            }

            (inbox.seq, inbox.forced, inbox.place)
        });

        let items = self.integration_items();

        let chips_of = |id: &str| -> Vec<(String, String)> {
            items
                .iter()
                .find(|item| item.id == id)
                .map(|item| item.chips.iter().map(|(label, action)| (label.to_string(), action.clone())).collect())
                .unwrap_or_default()
        };

        let (feed_rows, hidden) = feed::feed_lines(&view.lines, &self.island.hidden);
        let feed = feed_rows.iter().map(|line| line.id.clone()).collect();
        let island = &self.island;
        let spotify = &self.integrations.spotify;

        Snapshot {
            on: true,
            mood: match view.mood {
                Mood::Needs(_) => "needs",
                Mood::Working(_) => "working",
                Mood::Ready => "ready",
                Mood::Empty => "empty",
            },
            count: match view.mood {
                Mood::Needs(count) | Mood::Working(count) => count,
                _ => 0,
            },
            state: mago_name(main_state(view.mood, view.celebrating)),
            celebrating: view.celebrating,
            finished: view.finished,
            feed,
            hidden,
            lines: view.lines.iter().map(|line| line_out(line, chips_of(&line.id))).collect(),
            tabs: header_tabs(&view.lines).into_iter().map(|(tab, _, _, tip)| TabOut { tab: tab_name(tab), tip }).collect(),
            tab: tab_name(island.tab),
            settings: island.settings,
            focus: view.focus.clone(),
            pinned,
            ask,
            activity: view.activity.as_ref().map(|activity| ActivityOut {
                phase: match activity.phase {
                    super::stage::Phase::Working => "working",
                    super::stage::Phase::Editing => "editing",
                    super::stage::Phase::Done => "done",
                    super::stage::Phase::Failed => "failed",
                },
                verb: activity.verb.clone(),
                target: activity.target.clone(),
                file: activity.file.clone(),
                place: activity.place,
                moves: activity.moves.iter().map(|step| Step { verb: step.verb.clone(), running: step.running, failed: step.failed }).collect(),
                patch: activity.patch.clone(),
                patch_line: activity.patch_line,
                diff: activity.diff,
                output: activity.output.clone(),
                headline: activity.headline.clone(),
            }),
            glimpse: view.glimpse.iter().map(|(number, text)| Said { person: false, number: *number, text: text.clone() }).collect(),
            chat: self.island_chat_out(view.chat_row.as_ref(), view.session.clone()),
            voice: view.voice_on,
            voice_state: match view.voice_state {
                crate::voice::VoiceState::Off => "off",
                crate::voice::VoiceState::Preparing => "preparing",
                crate::voice::VoiceState::Listening => "listening",
                crate::voice::VoiceState::Hearing => "hearing",
                crate::voice::VoiceState::Speaking => "speaking",
                crate::voice::VoiceState::Muted => "muted",
            },
            voice_note: view.voice_note.clone(),
            sound: view.sound_on,
            volume: view.volume,
            close_after: view.close_after,
            compact_after: self.prefs.island_compact_after,
            color: hex(view.color),
            claude_hooks,
            song: spotify.shown(time).map(|now| SongOut {
                title: now.track.title.clone(),
                artist: now.track.artist.clone(),
                album: now.track.album.clone(),
                cover_url: now.track.cover_url.clone(),
                playing: now.playing,
                shuffle: now.playback.shuffle,
                repeat: now.playback.repeat.map(|repeat| format!("{repeat:?}").to_lowercase()),
                volume: now.playback.volume,
                position_ms: now.playback.position_ms,
                duration_ms: now.playback.duration_ms,
                read_at_ms: now.playback.read_at_ms,
                liked: now.playback.liked,
                device: now.playback.device.clone(),
                track_uri: now.playback.track_uri.clone(),
                tint: spotify.tint.map(hex),
            }),
            luna_dances: self.prefs.integrations.luna_dances,
            luna_color: hex(self.luna_colour(crate::app::mascot::mago::LUME)),
            github: github_on.then(|| self.native_github()),
            projects: island.form.as_ref().map(|form| form.projects.clone()).unwrap_or_default(),
            at: self.prefs.island_at,
            talk_size: self.prefs.island_talk,
            seq,
            forced,
            place,
        }
    }

    /// The first ask waiting, as its card shows it (`ask.rs`).
    fn native_ask(&self, asks: &[String]) -> Option<AskOut> {
        let chat = asks.first()?;

        let line = {
            let all = self.person_asks.borrow();
            let source = all.iter().find(|ask| &ask.chat == chat).and_then(|ask| ask.source.as_ref().map(|source| source.pill.clone()));

            source.map_or(chat.clone(), |pill| format!("{PILL_PREFIX}{pill}"))
        };

        let color = self
            .agent_rows()
            .iter()
            .find(|row| row.id == line)
            .map(|row| hex(self.agent_color(&row.id)))
            .or_else(|| self.outside_pills().into_iter().find(|pill| pill.id == line).map(|pill| hex(pill.color)));

        let all = self.person_asks.borrow();
        let ask = all.iter().find(|ask| &ask.chat == chat)?;
        let outside = ask.source.as_ref().map(|source| source.label.clone());

        let doing = match ask.questions.len() {
            0 => format!("precisa de permissão · {}", ask.tool_label()),
            1 => "está perguntando".to_string(),
            total => format!("está perguntando ({} de {total})", ask.step.min(total - 1) + 1),
        };

        Some(AskOut {
            chat: chat.clone(),
            name: outside.clone().or_else(|| self.chats.iter().find(|item| &item.id == chat).map(|item| item.name.clone())),
            color,
            approval: ask.questions.is_empty(),
            doing,
            summary: ask.summary(),
            details: ask.details(),
            outside: outside.is_some(),
            queued: asks.len().saturating_sub(1),
            step: ask.step,
            questions: ask.questions.iter().map(question_out).collect(),
            picked: ask.picked.clone(),
            other: ask.other.clone(),
            open: ask.open,
        })
    }

    /// The conversation page: the chat shown and the last things said in it.
    fn island_chat_out(&self, row: Option<&AgentRow>, session: Option<String>) -> Option<ChatOut> {
        if let Some(session) = session {
            let name = self.outside_pills().into_iter().find(|pill| pill.id == session).map(|pill| pill.name).unwrap_or_default();

            return Some(ChatOut { id: session.clone(), name, working: false, session: Some(session), said: Vec::new() });
        }

        let row = row?;

        let said = self.transcripts.get(&row.id).map_or(Vec::new(), |transcript| {
            let said: Vec<Said> = super::log::said(&transcript.items)
                .filter_map(|item| match item {
                    ChatItem::User(text) => Some(Said { person: true, number: 0, text: text.clone() }),
                    ChatItem::Agent(text) => Some(Said { person: false, number: 0, text: text.clone() }),
                    _ => None,
                })
                .collect();

            // ponytail: the last 40 messages only; page further back if the person asks for the whole log here
            said[said.len().saturating_sub(40)..].to_vec()
        });

        Some(ChatOut { id: row.id.clone(), name: self.chat_short_name(row), working: matches!(row.state, AgentState::Working(_)), session: None, said })
    }

    /// GitHub's tab: the person's pull requests and the ones asking for their review.
    fn native_github(&self) -> GithubOut {
        use crate::app::integrations::github::{CiState, PrFilter, PrState};
        let github = &self.integrations.github;
        let Some(pulse) = github.pulse.as_ref() else { return GithubOut { login: String::new(), error: github.error.clone(), prs: Vec::new() } };

        let prs = pulse
            .listed(PrFilter::All)
            .into_iter()
            .map(|request| PullOut {
                id: request.id.clone(),
                title: request.title.clone(),
                url: request.url.clone(),
                page: request.page(),
                draft: request.draft,
                ci: match request.ci {
                    CiState::Pending => "pending",
                    CiState::Success => "success",
                    CiState::Failure => "failure",
                    CiState::Unknown => "unknown",
                },
                state: match request.state {
                    PrState::Open => "open",
                    PrState::Merged => "merged",
                    PrState::Closed => "closed",
                },
                review: request.review,
                head_ref: request.head_ref.clone(),
                base_ref: request.base_ref.clone(),
                updated_at: request.updated_at,
                conflict: request.mergeable == crate::app::integrations::github::MergeableState::Conflicting,
                needs_you: request.needs_you(),
                mine: pulse.mine.iter().any(|known| known.id == request.id),
                failing: request.failing.iter().map(|check| check.name.clone()).collect(),
            })
            .collect();

        GithubOut { login: pulse.login.clone(), error: github.error.clone(), prs }
    }

    /// Whether Claude Code's settings have the app's hooks, read again every 3 s (it is a file).
    fn native_hooks(&self, time: f64) -> bool {
        INBOX.with(|inbox| {
            let mut inbox = inbox.borrow_mut();

            match inbox.hooks {
                Some((at, installed)) if (0.0..3.0).contains(&(time - at)) => installed,
                _ => {
                    use crate::app::maestro::claude_hooks::{installed_at, settings_path};
                    let installed = settings_path().is_some_and(|path| installed_at(&path));

                    inbox.hooks = Some((time, installed));
                    installed
                }
            }
        })
    }

    /// `ui.island.<verb>` from Swift: what the egui island does inline (`island_apply`) that has no action of its own.
    pub(crate) fn native_island_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();
        let time = self.now;

        match verb {
            // the island became `shape` (Swift's clock): its sound, and closed it goes back to Início
            "shape" => {
                let shape = match text("shape").as_str() {
                    "closed" => super::Shape::Closed,
                    "compact" => super::Shape::Compact,
                    "open" => super::Shape::Open,
                    other => return Err(format!("forma desconhecida: {other}")),
                };

                let pinned = !self.person_asks.borrow().is_empty();

                let tone = match (self.island.last_shape.replace(shape), shape) {
                    (Some(super::Shape::Closed), super::Shape::Compact) => Some(sounds::PEEK),
                    (Some(super::Shape::Open), super::Shape::Compact | super::Shape::Closed) => Some(sounds::CLOSE),
                    (Some(super::Shape::Closed | super::Shape::Compact), super::Shape::Open) => Some(if pinned { sounds::ASK } else { sounds::OPEN }),
                    _ => None,
                };

                // open, what finished is in sight: the check goes
                if shape == super::Shape::Open {
                    self.island.finished = false;
                }

                if shape != super::Shape::Open {
                    let island = &mut self.island;

                    (island.tab, island.session, island.settings) = (Tab::Home, None, false);
                }

                if let Some(tone) = tone {
                    self.island_chime(tone, time);
                }
            }
            "tab" => {
                let tab = Tab::named(&text("tab")).ok_or("aba desconhecida")?;

                self.island.settings = false;
                self.island.tab = tab;

                if tab != Tab::Chat {
                    self.island.session = None;
                }

                self.island.form = if tab == Tab::New { Some(self.island.form.take().unwrap_or_else(|| self.island_form())) } else { None };

                if tab == Tab::Chat && self.chat_agent.is_none() {
                    self.chat_agent = self.chats.first().map(|chat| chat.id.clone());
                }

                self.island_chime(sounds::BLIP, time);
            }
            "settings" => self.island.settings = !self.island.settings,
            "app_settings" => {
                self.open_settings_at("Geral");
                self.raise_window = true;
                self.island.settings = false;
                self.island.machine.force_petit(time);
            }
            "focus" => {
                let id = text("id");

                self.outside_focus(&id);
                self.island.focus = Some(id);
            }
            "open" => {
                let id = text("id");

                self.island_chime(sounds::BLIP, time);
                self.island_show(&id, time);
            }
            "canvas" => self.island_on_canvas(&text("id")),
            "talk" => {
                self.island.session = None;
                self.chat_agent = Some(text("id"));
                self.island.tab = Tab::Chat;
                self.island.machine.force_home(time);
            }
            "terminal" => {
                if !self.outside_terminal(&text("id")) {
                    return Err("Não deu para trazer a janela desse terminal: responda nele.".into());
                }
            }
            // its ✕, Esc, "Depois": folded, the asks keep waiting
            "close" => {
                self.island_fold();
                (self.island.tab, self.island.form, self.island.settings) = (Tab::Home, None, false);
            }
            // the conversation pulled by an edge (`done`: let go, kept in the prefs) or double-clicked back
            "size" => {
                let size = [args["width"].as_f64().unwrap_or(0.0) as f32, args["height"].as_f64().unwrap_or(0.0) as f32];

                self.prefs.island_talk = Some(size);
            }
            "unpull" => self.prefs.island_talk = None,
            "hide" => {
                let id = text("id");
                let signature = self.island.listed.iter().find(|(listed, _)| *listed == id).map(|(_, signature)| *signature).ok_or("linha desconhecida")?;

                self.island.hidden.insert(id, signature);
            }
            // a line of an integration with no tab of its own (WhatsApp): its first button; or a card's button
            "press" => {
                let action = match args["action"].as_str() {
                    Some(action) => action.to_string(),
                    None => {
                        let id = text("id");
                        let items = self.integration_items();
                        let item = items.into_iter().find(|item| item.id == id).ok_or("integração desconhecida")?;

                        item.chips.first().map(|(_, action)| action.clone()).ok_or("sem botão")?
                    }
                };

                self.integrations.pressed.borrow_mut().push(action);
            }
            "spotify" => self.spotify_press(&text("words")),
            // where it hangs from, let go by the person (screen points from the main screen's top left)
            "place" => {
                let at = [args["x"].as_f64().unwrap_or(0.0) as f32, args["y"].as_f64().unwrap_or(0.0) as f32];

                (self.island.anchor, self.prefs.island_at) = (Some(at), Some(at));
            }
            "ask" => return self.native_island_ask(args),
            "new" => {
                let form = super::NewForm {
                    name: text("name"),
                    folder: text("folder"),
                    worktree: args["worktree"].as_bool().unwrap_or(false),
                    prompt: text("prompt"),
                    ..Default::default()
                };

                let installed = self.installed_agents();
                let message = self.island_new_chat(&form, &installed)?;

                self.prefs.island_projects = super::forms::remember_project(&form.folder, &self.prefs.island_projects);

                if let Some(id) = self.chat_agent.clone().filter(|_| !form.prompt.trim().is_empty()) {
                    self.chat_outbox.push((id, form.prompt.trim().to_string()));
                }

                (self.island.form, self.island.tab) = (None, Tab::Chat);

                return Ok(message);
            }
            "chime" => {
                let sound = match text("name").as_str() {
                    "blip" => sounds::BLIP,
                    "poke" => sounds::POKE,
                    "dizzy" => sounds::DIZZY,
                    "greet" => sounds::GREET,
                    "error" => sounds::ERROR,
                    other => return Err(format!("som desconhecido: {other}")),
                };

                self.island_chime(sound, time);
            }
            other => return Err(format!("ação desconhecida da ilha: {other}")),
        }

        Ok("Feito.".into())
    }

    /// The ask card's buttons (`ask.rs`): Permitir or Negar (`allow`), an option picked (`pick`), "Outro" sent (`other`), OK (`next`), the whole
    /// request opened (`open`) or "Responder no terminal" (`terminal`).
    fn native_island_ask(&mut self, args: &Value) -> Result<String, String> {
        let chat = args["chat"].as_str().unwrap_or_default();
        let mut asks = self.person_asks.borrow_mut();
        let index = asks.iter().position(|ask| ask.chat == chat).ok_or("Nenhuma pergunta desse chat.")?;

        if asks[index].guarded(self.now) {
            return Err("Espere um instante: a pergunta acabou de chegar.".into());
        }

        if let Some(allow) = args["allow"].as_bool() {
            asks.remove(index).send(allow);

            return Ok(if allow { "Permitido." } else { "Negado." }.into());
        }

        let ask = &mut asks[index];

        let send = if let Some(option) = args["pick"].as_u64() {
            ask.pick(option as usize)
        } else if let Some(words) = args["other"].as_str() {
            let step = ask.step.min(ask.questions.len().saturating_sub(1));

            if let Some(other) = ask.other.get_mut(step) {
                *other = words.to_string();
            }

            ask.answer_other()
        } else if args["next"].as_bool() == Some(true) {
            ask.next()
        } else if args["open"].as_bool() == Some(true) {
            ask.open = !ask.open;
            false
        } else if args["terminal"].as_bool() == Some(true) {
            let ask = asks.remove(index);

            if let Some(source) = &ask.source {
                self.outside.declined.borrow_mut().push((source.session.clone(), now_ms()));
            }

            ask.send_terminal();

            return Ok("Respondendo no terminal.".into());
        } else {
            return Err("diga allow, pick, other, next, open ou terminal".into());
        };

        if send {
            asks.remove(index).send(true);

            return Ok("Respondido.".into());
        }

        Ok("Feito.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: &str) -> crate::app::chat::direct::Chat {
        crate::app::chat::direct::Chat {
            id: id.into(),
            agent: "claude".into(),
            name: "Claude Code · loja".into(),
            cwd: "/p".into(),
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
            recruit: false,
            named: false,
            draft: false,
        }
    }

    #[test]
    fn the_snapshot_has_the_agents_the_tabs_and_what_the_island_is_set_to() {
        let mut app = DrawingApp::default();

        app.prefs.island = false;
        assert_eq!(app.native_island_snapshot(), Snapshot::default(), "turned off: nothing");
        app.prefs.island = true;
        app.chats = vec![chat("a")];
        let snapshot = app.native_island_snapshot();

        assert!(snapshot.on);
        let line = snapshot.lines.iter().find(|line| line.id == "a").expect("the chat is a line");

        // each agent wears its own colour (`agent_color`), the same as in the egui lists
        assert_eq!((line.tab, line.chat, line.outside, line.tool), ("chats", true, false, false));
        assert_eq!(line.color, hex(app.agent_color("a")));
        assert_eq!(snapshot.tab, "home");
        assert_eq!(snapshot.tabs.first().map(|tab| tab.tab), Some("home"));
        assert_eq!(snapshot.tabs.last().map(|tab| tab.tab), Some("new"), "the new chat always last");
        assert_eq!(snapshot.color, "#000000");
        assert!(snapshot.ask.is_none() && !snapshot.pinned);
        let json = serde_json::to_value(&snapshot).unwrap();

        assert!(json["lines"][0]["mago"].is_string() && json["closeAfter"].is_number(), "camelCase for Swift: {json}");
    }

    #[test]
    fn what_the_app_forces_reaches_swift_once_each_time() {
        let mut app = DrawingApp::default();

        app.prefs.island = true;
        let first = app.native_island_snapshot().seq;

        app.island_show_page(Tab::GitHub);
        let snapshot = app.native_island_snapshot();

        assert_eq!((snapshot.seq, snapshot.forced, snapshot.tab), (first + 1, Some("home"), "home"), "GitHub off: Início");
        assert_eq!(app.native_island_snapshot().seq, first + 1, "read once");
        // forced again while Swift has it open: seen again
        app.island_show_page(Tab::Chats);
        assert_eq!(app.native_island_snapshot().seq, first + 2);
        app.put_island(super::super::Place::Left);
        let snapshot = app.native_island_snapshot();

        assert_eq!((snapshot.seq, snapshot.place), (first + 3, Some("left")));
    }

    #[test]
    fn swift_moves_the_tabs_and_closing_goes_back_to_the_start() {
        let mut app = DrawingApp::default();

        app.prefs.island = true;
        app.native_island_act("tab", &json!({ "tab": "new" })).unwrap();
        assert_eq!((app.island.tab, app.island.form.is_some()), (Tab::New, true));
        app.native_island_act("settings", &json!({})).unwrap();
        assert!(app.island.settings);
        app.native_island_act("shape", &json!({ "shape": "open" })).unwrap();
        app.native_island_act("shape", &json!({ "shape": "compact" })).unwrap();
        assert_eq!((app.island.tab, app.island.settings), (Tab::Home, false));
        app.native_island_act("place", &json!({ "x": 735.0, "y": 0.0 })).unwrap();
        assert_eq!(app.prefs.island_at, Some([735.0, 0.0]));
        assert!(app.native_island_act("nada", &json!({})).is_err());
        assert!(app.native_island_act("tab", &json!({ "tab": "nada" })).is_err());
    }
}
