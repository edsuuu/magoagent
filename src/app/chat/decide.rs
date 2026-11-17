//! What a chat's agent asks the person to decide, as Claude Code shows it: its AskUserQuestion (the questions one
//! at a time, a tab each, the options numbered with what each means, a line for another answer, and Enviar), or
//! a permission it needs (Permitir / Negar). The chat's `claude -p` waits on the app's MCP tool `ask_person` (its
//! `--permission-prompt-tool`, chat/tools.rs) until the answer goes back over the bridge.

use crate::app::chat::list::{AMBER, Look, bold, pulse_dot};
use crate::app::maestro::outside::{ASK_PREFIX, Source};
use crate::prelude::*;
use std::sync::mpsc::Sender;

/// The tool Claude Code asks through: the app's MCP server's `ask_person`.
pub(crate) const PROMPT_TOOL: &str = "mcp__magoagent__ask_person";

/// One question of an AskUserQuestion.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Question {
    pub(crate) text: String,
    pub(crate) header: String,
    pub(crate) multiple: bool,
    /// (label, what it means)
    pub(crate) options: Vec<(String, String)>,
}

/// The questions of an AskUserQuestion's input; none for any other tool.
pub(crate) fn questions(input: &Value) -> Vec<Question> {
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    input["questions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|question| Question {
            text: text(&question["question"]),
            header: text(&question["header"]),
            multiple: question["multiSelect"] == true,
            options: question["options"].as_array().into_iter().flatten().map(|option| (text(&option["label"]), text(&option["description"]))).collect(),
        })
        .collect()
}

/// A decision the agent waits for.
pub(crate) struct PersonAsk {
    pub(crate) chat: String,
    pub(crate) tool: String,
    pub(crate) input: Value,
    reply: Sender<Result<String, String>>,
    pub(crate) questions: Vec<Question>,
    /// Per question: the options picked and another answer typed.
    pub(crate) picked: Vec<Vec<usize>>,
    pub(crate) other: Vec<String>,
    /// The question shown.
    pub(crate) step: usize,
    /// Of a Claude Code session outside the app (maestro/outside.rs): where from, and the hook that waits.
    pub(crate) source: Option<Source>,
    /// When the card was first drawn: its buttons wait `CLICK_GUARD` after that.
    pub(crate) shown: Option<f64>,
}

/// Clicks in the first moments of a new card are ignored: it appears under a pointer busy in another window (600 ms).
const CLICK_GUARD: f64 = 0.6;

impl PersonAsk {
    pub(crate) fn new(chat: String, request: &Value, reply: Sender<Result<String, String>>) -> PersonAsk {
        let input = request["input"].clone();
        let questions = questions(&input);
        let count = questions.len();
        PersonAsk {
            chat,
            tool: request["tool_name"].as_str().unwrap_or_default().to_string(),
            input,
            reply,
            questions,
            picked: vec![Vec::new(); count],
            other: vec![String::new(); count],
            step: 0,
            source: None,
            shown: None,
        }
    }

    /// Each question's answer as a hook takes it (events.rs `question_answer`): the one label picked, or the list of
    /// them for a multiple choice; anything else (nothing, two for one) fails there and the terminal asks.
    fn hook_answers(&self) -> serde_json::Map<String, Value> {
        let mut answers = serde_json::Map::new();
        for (index, question) in self.questions.iter().enumerate() {
            let labels: Vec<Value> =
                self.picked[index].iter().filter_map(|option| question.options.get(*option)).map(|(label, _)| label.clone().into()).collect();
            let answer = if question.multiple {
                Value::Array(labels)
            } else if labels.len() == 1 {
                labels[0].clone()
            } else {
                Value::Null
            };
            answers.insert(question.text.clone(), answer);
        }
        answers
    }

    /// Each question's answer as Claude Code takes it: the labels picked (joined by ", ") and what was typed.
    pub(crate) fn answers(&self) -> serde_json::Map<String, Value> {
        let mut answers = serde_json::Map::new();
        for (index, question) in self.questions.iter().enumerate() {
            let mut parts: Vec<String> = self.picked[index].iter().filter_map(|option| question.options.get(*option).map(|(label, _)| label.clone())).collect();
            if !self.other[index].trim().is_empty() {
                parts.push(self.other[index].trim().to_string());
            }
            answers.insert(question.text.clone(), parts.join(", ").into());
        }
        answers
    }

    /// Every question has an answer.
    pub(crate) fn complete(&self) -> bool {
        (0..self.questions.len()).all(|index| !self.picked[index].is_empty() || !self.other[index].trim().is_empty())
    }

    /// The option `option` of the question shown, as buttons: ticked or not in a multiple choice, otherwise
    /// the answer, on to the next question. True when that answered the last one.
    pub(crate) fn pick(&mut self, option: usize) -> bool {
        let step = self.step.min(self.questions.len().saturating_sub(1));
        let Some(question) = self.questions.get(step) else { return false };
        if option >= question.options.len() {
            return false;
        }
        let picked = &mut self.picked[step];
        if question.multiple {
            if picked.contains(&option) {
                picked.retain(|chosen| *chosen != option)
            } else {
                picked.push(option)
            }
            return false;
        }
        *picked = vec![option];
        self.next()
    }

    /// On past the question shown (a multiple choice's OK): true when it was the last.
    pub(crate) fn next(&mut self) -> bool {
        if self.step + 1 < self.questions.len() {
            self.step += 1;
            return false;
        }
        self.complete()
    }

    /// The card showed up less than `CLICK_GUARD` ago (from its first draw): its clicks don't count yet.
    pub(crate) fn guarded(&mut self, time: f64) -> bool {
        time - *self.shown.get_or_insert(time) < CLICK_GUARD
    }

    /// What goes back to Claude Code: allowed (with the answers, for questions) or denied.
    pub(crate) fn decision(&self, allow: bool) -> String {
        if !allow {
            return deny("A pessoa recusou.");
        }
        let mut input = self.input.clone();
        if !self.questions.is_empty()
            && let Some(object) = input.as_object_mut()
        {
            object.insert("answers".into(), Value::Object(self.answers()));
        }
        json!({ "behavior": "allow", "updatedInput": input }).to_string()
    }

    /// Answers the agent; the ask is done.
    pub(crate) fn send(self, allow: bool) {
        let text = match &self.source {
            None => self.decision(allow),
            Some(source) if !allow => refusal(&source.event, "Negado pela pessoa no MagoAgent."),
            Some(source) if self.questions.is_empty() => {
                if source.event == "PermissionRequest" {
                    permission_answer(true, "")
                } else {
                    String::new()
                }
            }
            Some(source) => question_answer(&source.event, &self.input, &self.hook_answers()),
        };
        let _ = self.reply.send(Ok(text));
    }

    /// "Responder no terminal": nothing is printed and the CLI asks there, as if the app were not.
    pub(crate) fn send_terminal(self) {
        let _ = self.reply.send(Ok(String::new()));
    }

    /// What a permission is about, in one line: the command, the file or the address.
    pub(crate) fn summary(&self) -> String {
        let field = |key: &str| self.input[key].as_str().map(str::to_string);
        field("command").or_else(|| field("file_path")).or_else(|| field("url")).or_else(|| field("path")).unwrap_or_else(|| {
            let text = self.input.to_string();
            text.chars().take(200).collect()
        })
    }
}

/// A refusal as Claude Code takes it.
pub(crate) fn deny(message: &str) -> String {
    json!({ "behavior": "deny", "message": message }).to_string()
}

impl DrawingApp {
    /// The bridge brought a decision a chat's agent waits for (`ask_person`).
    pub(crate) fn ask_person(&mut self, from: &str, request: &Value, reply: Sender<Result<String, String>>) {
        if !self.is_chat(from) {
            let _ = reply.send(Ok(deny("Só um chat do app pergunta à pessoa por aqui.")));
            return;
        }
        // one at a time per chat: a new one means the agent gave the old one up
        self.person_asks.get_mut().retain(|ask| ask.chat != from);
        let ask = PersonAsk::new(from.to_string(), request, reply);
        let name = self.chats.iter().find(|chat| chat.id == from).map(|chat| chat.name.clone()).unwrap_or_default();
        self.toast(if ask.questions.is_empty() { format!("{name} pede permissão") } else { format!("{name} tem uma pergunta para você") });
        self.person_asks.get_mut().push(ask);
    }

    /// The chat waits on the person.
    pub(crate) fn person_asked(&self, chat: &str) -> bool {
        self.person_asks.borrow().iter().any(|ask| ask.chat == chat)
    }

    /// Sends the decision of `chat`'s ask.
    pub(crate) fn answer_person(&self, chat: &str, allow: bool) {
        let mut asks = self.person_asks.borrow_mut();
        if let Some(index) = asks.iter().position(|ask| ask.chat == chat) {
            asks.remove(index).send(allow);
        }
    }

    /// The MCP's `chat_decide`: the Maestro answers for the person (`answers`: question → answer; `allow` false refuses).
    pub(crate) fn act_chat_decide(&mut self, args: &Value) -> Result<String, String> {
        let wanted = args["chat"].as_str().unwrap_or_default().trim().to_lowercase();
        let chat = self
            .person_asks
            .borrow()
            .iter()
            .map(|ask| ask.chat.clone())
            .find(|id| wanted.is_empty() || *id == wanted || self.chats.iter().any(|chat| chat.id == *id && chat.name.to_lowercase() == wanted))
            .ok_or("Nenhum chat esperando uma decisão.")?;
        let allow = args["allow"].as_bool().unwrap_or(true);
        if let Some(ask) = self.person_asks.get_mut().iter_mut().find(|ask| ask.chat == chat) {
            for (index, question) in ask.questions.clone().iter().enumerate() {
                if let Some(answer) = args["answers"][&question.text].as_str() {
                    match question.options.iter().position(|(label, _)| label == answer) {
                        Some(option) => ask.picked[index] = vec![option],
                        None => ask.other[index] = answer.to_string(),
                    }
                }
            }
            if allow && !ask.complete() {
                let missing: Vec<String> = ask.questions.iter().map(|question| question.text.clone()).collect();
                return Err(format!("Responda todas as perguntas em `answers`: {}", missing.join(" | ")));
            }
        }
        self.answer_person(&chat, allow);
        Ok(if allow { "Respondido.".into() } else { "Recusado.".into() })
    }

    /// The card over the box: the questions (or the permission) and how to answer. Drops an ask whose chat no
    /// longer runs.
    pub(crate) fn decide_card(&self, ui: &mut egui::Ui, chat: &str, look: &Look, time: f64) {
        let mut asks = self.person_asks.borrow_mut();
        if !self.chat_runs.contains_key(chat) && !chat.starts_with(ASK_PREFIX) {
            asks.retain(|ask| ask.chat != chat);
        }
        let Some(index) = asks.iter().position(|ask| ask.chat == chat) else { return };
        let name = self.chats.iter().find(|item| item.id == chat).map(|item| item.name.clone()).unwrap_or_default();
        let hex = |value: u32| Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
        let (fill, edge, title) = if look.dark { (hex(0x2a2213), hex(0x7a5a22), hex(0xf6c46f)) } else { (hex(0xfdf3e1), hex(0xe6b866), hex(0x8a5a00)) };
        let (well, well_edge) = if look.dark { (hex(0x17130b), hex(0x3d2f14)) } else { (hex(0xfffaf0), hex(0xefd3a0)) };
        let mut decided: Option<bool> = None;
        let mut terminal = false;
        let ask = &mut asks[index];
        let outside = ask.source.as_ref().map(|source| (source.label.clone(), source.session.clone()));
        let name = outside.as_ref().map_or(name, |(label, _)| label.clone());
        let guarded = ask.guarded(time);
        let before = (ask.picked.clone(), ask.other.clone(), ask.step);
        egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, edge)).corner_radius(12).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
            pulse_dot(ui.painter(), Pos2::new(rect.left() + 5.0, rect.center().y), 4.5, AMBER, 2.0, time);
            let head = match (outside.is_some(), ask.questions.is_empty()) {
                (true, true) => format!("{name} precisa de permissão: {}", ask.tool),
                (true, false) => format!("{name} está perguntando"),
                (false, true) => format!("{name} pede permissão: {}", ask.tool),
                (false, false) => format!("{name} precisa de você"),
            };
            bold(ui.painter(), Pos2::new(rect.left() + 20.0, rect.top()), &head, 14.0, title);
            if ask.questions.is_empty() {
                // a permission: what it is about, Permitir / Negar
                let summary = ask.summary();
                egui::Frame::new().fill(well).stroke(Stroke::new(1.0, well_edge)).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(
                    ui,
                    |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(egui::RichText::new(summary).monospace().size(13.0).color(look.text)).wrap());
                    },
                );
                ui.horizontal(|ui| {
                    if pill(ui, "Permitir", true, look, edge).clicked() {
                        decided = Some(true);
                    }
                    if pill(ui, "Negar", false, look, edge).clicked() {
                        decided = Some(false);
                    }
                    if outside.is_some() && pill(ui, "Responder no terminal", false, look, edge).clicked() {
                        terminal = true;
                    }
                });
                return;
            }
            // the questions' tabs, as Claude Code's: their headers, the one shown lit, ✓ on the answered
            if ask.questions.len() > 1 {
                ui.horizontal(|ui| {
                    for step in 0..ask.questions.len() {
                        let answered = !ask.picked[step].is_empty() || !ask.other[step].trim().is_empty();
                        let header = &ask.questions[step].header;
                        let label = format!("{}{}", if answered { "✓ " } else { "" }, if header.is_empty() { format!("{}", step + 1) } else { header.clone() });
                        if pill(ui, &label, step == ask.step, look, edge).clicked() {
                            ask.step = step;
                        }
                    }
                });
            }
            let step = ask.step.min(ask.questions.len() - 1);
            let question = ask.questions[step].clone();
            ui.add(egui::Label::new(egui::RichText::new(&question.text).size(14.5).color(look.text)).wrap());
            if question.multiple {
                ui.label(egui::RichText::new("Escolha uma ou mais").size(12.0).color(look.muted));
            }
            for (option, (label, meaning)) in question.options.iter().enumerate() {
                let on = ask.picked[step].contains(&option);
                let height = if meaning.is_empty() { 34.0 } else { 50.0 };
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                let painter = ui.painter();
                let ground = if response.hovered() { AMBER.gamma_multiply(0.22) } else { well };
                painter.rect(rect, 8, ground, Stroke::new(1.0, if on { AMBER } else { well_edge }), StrokeKind::Inside);
                let mark = if question.multiple { if on { "☑" } else { "☐" } } else { "" };
                let number = format!("{}{}. {label}", if mark.is_empty() { String::new() } else { format!("{mark} ") }, option + 1);
                painter.text(Pos2::new(rect.left() + 12.0, rect.top() + 17.0), egui::Align2::LEFT_CENTER, number, FontId::proportional(14.0), look.text);
                if !meaning.is_empty() {
                    let galley = crate::app::chat::list::clipped(painter, meaning.clone(), FontId::proportional(12.5), look.muted, rect.width() - 24.0);
                    painter.galley(Pos2::new(rect.left() + 12.0, rect.top() + 28.0), galley, look.muted);
                }
                if response.clicked() {
                    let picked = &mut ask.picked[step];
                    if question.multiple {
                        if on { picked.retain(|chosen| *chosen != option) } else { picked.push(option) }
                    } else {
                        *picked = vec![option];
                        // one answer: on to the next question, or send when it was the last
                        if step + 1 < ask.questions.len() {
                            ask.step = step + 1;
                        } else if ask.complete() {
                            decided = Some(true);
                        }
                    }
                }
            }
            // another answer, written (a hook takes only the options' labels)
            if outside.is_none() {
                let other = &mut ask.other[step];
                egui::Frame::new().fill(well).stroke(Stroke::new(1.0, well_edge)).corner_radius(8).inner_margin(egui::Margin::symmetric(10, 6)).show(
                    ui,
                    |ui| {
                        let field = egui::TextEdit::singleline(other)
                            .hint_text("Outra resposta…")
                            .frame(egui::Frame::NONE)
                            .text_color(look.text)
                            .desired_width(f32::INFINITY);
                        ui.add(field);
                    },
                );
            }
            ui.horizontal(|ui| {
                let last = step + 1 == ask.questions.len();
                if step > 0 && pill(ui, "Voltar", false, look, edge).clicked() {
                    ask.step = step - 1;
                }
                if !last && pill(ui, "Próxima", true, look, edge).clicked() {
                    ask.step = step + 1;
                }
                if last && ask.complete() && pill(ui, "Enviar", true, look, edge).clicked() {
                    decided = Some(true);
                }
                if pill(ui, "Recusar", false, look, edge).clicked() {
                    decided = Some(false);
                }
                if outside.is_some() && pill(ui, "Responder no terminal", false, look, edge).clicked() {
                    terminal = true;
                }
            });
        });
        if guarded {
            (ask.picked, ask.other, ask.step) = before;
            (decided, terminal) = (None, false);
        }
        if terminal {
            if let Some((_, session)) = outside {
                self.outside.declined.borrow_mut().push((session, now_ms()));
            }
            asks.remove(index).send_terminal();
        } else if let Some(allow) = decided {
            asks.remove(index).send(allow);
        }
    }
}

/// A round button of the card; `strong` is the one that answers.
fn pill(ui: &mut egui::Ui, text: &str, strong: bool, look: &Look, edge: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.5), look.text);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 24.0, 30.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let ground = match (strong, response.hovered()) {
        (true, _) => AMBER.gamma_multiply(if response.hovered() { 0.4 } else { 0.28 }),
        (false, true) => AMBER.gamma_multiply(0.18),
        (false, false) => Color32::TRANSPARENT,
    };
    ui.painter().rect(rect, 15, ground, Stroke::new(1.0, edge), StrokeKind::Inside);
    ui.painter().galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, look.text);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked() -> (PersonAsk, std::sync::mpsc::Receiver<Result<String, String>>) {
        let (reply, answers) = std::sync::mpsc::channel();
        let request = json!({ "tool_name": "AskUserQuestion", "input": { "questions": [
            { "question": "Qual cor?", "header": "Cor", "multiSelect": false, "options": [{ "label": "Azul", "description": "fria" }, { "label": "Verde" }] },
            { "question": "Quais frutas?", "header": "Frutas", "multiSelect": true, "options": [{ "label": "Maçã" }, { "label": "Uva" }] },
        ] } });
        (PersonAsk::new("chat".into(), &request, reply), answers)
    }

    #[test]
    fn the_answers_go_back_as_claude_code_takes_them() {
        let (mut ask, answers) = asked();
        assert_eq!(ask.questions.len(), 2);
        assert_eq!(ask.questions[0].options[0], ("Azul".to_string(), "fria".to_string()));
        assert!(ask.questions[1].multiple && !ask.complete());
        ask.picked[0] = vec![1];
        ask.picked[1] = vec![0, 1];
        ask.other[1] = " banana ".into();
        assert!(ask.complete());
        ask.send(true);
        let sent: Value = serde_json::from_str(&answers.recv().unwrap().unwrap()).unwrap();
        assert_eq!(sent["behavior"], "allow");
        assert_eq!(sent["updatedInput"]["answers"], json!({ "Qual cor?": "Verde", "Quais frutas?": "Maçã, Uva, banana" }));
        assert_eq!(sent["updatedInput"]["questions"][0]["header"], "Cor", "the questions stay as they came");
    }

    #[test]
    fn a_pick_answers_and_moves_on_and_a_multiple_choice_waits_for_ok() {
        let (mut ask, _answers) = asked();
        assert!(!ask.pick(5), "an option that isn't there does nothing");
        assert!(!ask.pick(1) && ask.step == 1, "one answer goes on to the next question");
        assert!(!ask.pick(0) && !ask.pick(1) && !ask.pick(0), "a multiple choice only ticks");
        assert_eq!(ask.picked[1], vec![1]);
        assert!(ask.next(), "its OK on the last question sends");
        assert!(ask.guarded(10.0) && ask.guarded(10.5) && !ask.guarded(10.7), "the first 600 ms of the card don't count");
    }

    #[test]
    fn a_permission_is_allowed_as_it_came_or_denied() {
        let (reply, answers) = std::sync::mpsc::channel();
        let ask = PersonAsk::new("chat".into(), &json!({ "tool_name": "Bash", "input": { "command": "rm -rf build" } }), reply);
        assert!(ask.questions.is_empty());
        assert_eq!(ask.summary(), "rm -rf build");
        assert_eq!(serde_json::from_str::<Value>(&ask.decision(true)).unwrap(), json!({ "behavior": "allow", "updatedInput": { "command": "rm -rf build" } }));
        ask.send(false);
        assert_eq!(serde_json::from_str::<Value>(&answers.recv().unwrap().unwrap()).unwrap()["behavior"], "deny");
    }
}
