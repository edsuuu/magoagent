//! Quick Documentation on hover, as PhpStorm shows it: the signature in the code's colours, then the
//! documentation as Markdown (the chat's renderer). The popup stays open while the pointer is on it, so
//! it can be scrolled.

use super::*;
use crate::app::chat::prose;
use crate::app::ui::UiColors;

/// The popup for the word at `at`: `anchor` is that word on screen, `rect` the popup's rectangle last frame.
pub(super) struct Popup {
    pub at: (PathBuf, usize, usize),
    pub anchor: Rect,
    pub rect: Option<Rect>,
}

/// A hover's Markdown as (the signature, the rest). The first code block is the signature; `@param` and
/// `@return` lines become the "Parâmetros" and "Retorna" sections.
pub(super) fn parts(md: &str) -> (Option<String>, String) {
    let (mut sig, mut body, mut code) = (None, vec![], vec![]);
    let (mut params, mut returns) = (vec![], vec![]);
    let mut open = None::<bool>; // inside a code block: Some(is the signature's)
    for line in md.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            match open.take() {
                Some(true) => sig = Some(code.join("\n").trim().to_string()).filter(|s: &String| !s.is_empty()),
                Some(false) => body.push("```".to_string()),
                None if sig.is_none() => open = Some(true),
                None => {
                    open = Some(false);
                    body.push(line.to_string());
                }
            }
            code.clear();
        } else if open == Some(true) {
            if !t.starts_with("<?php") {
                code.push(line.to_string());
            }
        } else if open == Some(false) {
            body.push(line.to_string());
        } else if let Some(rest) = t.trim_start_matches('_').strip_prefix("@param") {
            params.push(param_line(rest));
        } else if let Some(rest) = t.trim_start_matches('_').strip_prefix("@return") {
            returns.push(return_line(rest));
        } else if t.len() > 4 && t.starts_with("__") && t.ends_with("__") {
            body.push(format!("**{}**", &t[2..t.len() - 2])); // intelephense's title: __name__
        } else if !(t.len() >= 3 && t.chars().all(|c| matches!(c, '-' | '*' | '_'))) {
            body.push(line.to_string()); // a rule (***) is dropped
        }
    }
    for (title, items) in [("**Parâmetros**", params), ("**Retorna**", returns)] {
        if !items.is_empty() {
            if body.last().is_some_and(|l| !l.is_empty()) {
                body.push(String::new());
            }
            body.push(title.to_string());
            body.extend(items);
        }
    }
    body.dedup_by(|a, b| a.is_empty() && b.is_empty()); // a signature taken out leaves its blank lines
    (sig, body.join("\n").trim().to_string())
}

/// `@param type $name description` as a bullet: `- **$name** `type` description`. The tag may come
/// in italics and the type in code marks (intelephense's Markdown), so both are taken off first.
fn param_line(rest: &str) -> String {
    let rest = rest.trim_start_matches('_').replace(['_', '`'], "");
    let w: Vec<&str> = rest.split_whitespace().collect();
    let at = w.iter().position(|x| x.starts_with('$')).or_else(|| w.iter().position(|x| !x.starts_with('{'))).unwrap_or(w.len());
    let ty = w[..at].join(" ").replace(['{', '}'], "");
    let name = w.get(at).copied().unwrap_or("");
    let desc = w.get(at + 1..).map(|d| d.join(" ")).unwrap_or_default();
    let ty = if ty.is_empty() { String::new() } else { format!(" `{ty}`") };
    format!("- **{name}**{ty} {desc}").trim_end().to_string()
}

/// `@return type description` as a bullet: `- `type` description`.
fn return_line(rest: &str) -> String {
    let rest = rest.trim_start_matches('_').replace(['_', '`'], "");
    let (ty, desc) = rest.trim().split_once(' ').unwrap_or((rest.trim(), ""));
    format!("- `{ty}` {desc}").trim_end().to_string()
}

/// Shows the popup under `anchor` and returns its rectangle. `lang` colours the signature.
pub(super) fn show(ctx: &egui::Context, id: Id, anchor: Rect, lang: &'static highlight::Lang, md: &str) -> Rect {
    let (sig, body) = parts(md);
    let c = UiColors { island: t::ISLAND, surface_low: t::INLINE, surface_high: t::HOVER, selected: t::SELECTION, text: t::TEXT, muted: t::MUTED };
    egui::Area::new(id).order(Order::Tooltip).fixed_pos(anchor.left_bottom() + vec2(0.0, 4.0)).constrain(true).show(ctx, |ui| {
        Frame::popup(ui.style()).fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_max_width(500.0);
            egui::ScrollArea::vertical().id_salt(id.with("docs")).max_height(360.0).show(ui, |ui| {
                if let Some(sig) = &sig {
                    let mut job = highlight::layout(lang, sig, t::mono(12.5), 16.0);
                    job.wrap.max_width = 480.0;
                    ui.label(job);
                }
                if !body.is_empty() {
                    prose(ui, &body, &c, true, 480.0, false);
                }
            });
        });
    }).response.rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intelephense_markdown_keeps_its_signature_and_sections() {
        let md = "__montar__\n\nMonta a linha de um título.\n\n```php\n<?php\nfunction montar(int $largura, string $titulo): string { }\n```\n\nJunta a largura.\n\n_@param_ `int $largura` A largura em colunas\n\n_@return_ `string` A linha pronta";
        let (sig, body) = parts(md);
        assert_eq!(sig.as_deref(), Some("function montar(int $largura, string $titulo): string { }"));
        assert_eq!(body, "**montar**\n\nMonta a linha de um título.\n\nJunta a largura.\n\n**Parâmetros**\n- **$largura** `int` A largura em colunas\n\n**Retorna**\n- `string` A linha pronta");
    }

    #[test]
    fn the_signature_is_the_first_block_and_params_become_a_section() {
        let md = "```php\n<?php\nfunction montar(int $largura, string $titulo): string\n```\n***\nMonta o texto.\n\n@param int $largura A largura\n@param string $titulo\n@return string O resultado";
        let (sig, body) = parts(md);
        assert_eq!(sig.as_deref(), Some("function montar(int $largura, string $titulo): string"));
        assert_eq!(body, "Monta o texto.\n\n**Parâmetros**\n- **$largura** `int` A largura\n- **$titulo** `string`\n\n**Retorna**\n- `string` O resultado");
        assert_eq!(parts("só texto").0, None);
    }
}
