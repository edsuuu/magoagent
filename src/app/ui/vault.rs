//! Cofre (☰ › Ir › Cofre…), a card like Ajustes: the accounts the agents log in with. The list
//! shows site, user and kind; a password is typed once, masked, and goes straight to the
//! keychain (vault.rs), never back onto the screen.

use crate::prelude::*;

/// An entry being added (`index` None) or edited, with the password typed for it.
#[derive(Default)]
pub(crate) struct VaultEdit {
    pub(crate) index: Option<usize>,
    pub(crate) entry: VaultEntry,
    pub(crate) secret: String,
    pub(crate) show: bool,
    pub(crate) error: String,
}

impl DrawingApp {
    pub(crate) fn vault_ui(&mut self, ctx: &Context) {
        let c = self.colors();
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let mut entries = self.vault.take().unwrap_or_else(load_vault);
        let mut edit = self.vault_edit.take();
        let (mut remove, mut add, mut cancel, mut save) = (None, false, false, false);
        egui::Area::new(egui::Id::unique("vault")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            egui::Frame::new()
                .fill(c.island)
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(22, 18))
                .stroke(Stroke::new(1.0, c.surface_high))
                .shadow(egui::epaint::Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(90) })
                .show(ui, |ui| {
                    ui.set_width(DIALOG_WIDTH + 40.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Cofre").size(15.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let x = square_button(ui, 26.0, false, Color32::TRANSPARENT, &c);
                            for (a, b) in [(Vec2::new(-4.5, -4.5), Vec2::new(4.5, 4.5)), (Vec2::new(-4.5, 4.5), Vec2::new(4.5, -4.5))] {
                                ui.painter().line_segment([x.rect.center() + a, x.rect.center() + b], Stroke::new(1.5, c.muted));
                            }
                            close |= x.on_hover_text("Fechar (Esc)").clicked();
                        });
                    });
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("CONTAS").size(11.0).strong().color(dialog_label(&c)));
                    ui.label(
                        egui::RichText::new("Os agentes entram com elas sem nunca ver a senha, que fica no chaveiro do sistema.")
                            .size(12.0)
                            .color(dialog_label(&c)),
                    );
                    ui.add_space(6.0);
                    group_frame(&c).inner_margin(6).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if entries.is_empty() {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Nenhuma conta ainda.").size(12.5).color(c.muted));
                            ui.add_space(6.0);
                        }
                        for (i, entry) in entries.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.set_min_height(38.0);
                                let (mark, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                                paint_icon(ui.painter(), kind_icon(entry.kind), mark.center(), c.text);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.label(egui::RichText::new(&entry.site).size(13.5).color(c.text));
                                    let how = entry.kind.provider().map_or("senha".to_string(), |p| format!("via {p}"));
                                    ui.label(egui::RichText::new(format!("{} · {how}", entry.user)).size(11.5).color(c.muted));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // ⊖, red under the pointer, as in Ajustes
                                    let (rect, button) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
                                    let tint = if button.hovered() { Color32::from_rgb(0xe0, 0x31, 0x31) } else { c.muted };
                                    ui.painter().circle_stroke(rect.center(), 7.5, Stroke::new(1.2, tint));
                                    ui.painter().hline(rect.center().x - 3.5..=rect.center().x + 3.5, rect.center().y, Stroke::new(1.4, tint));
                                    if button.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Remover").clicked() {
                                        remove = Some(i);
                                    }
                                    let editing = edit.as_ref().is_some_and(|e| e.index == Some(i));
                                    if icon_button(ui, 26.0, editing, Color32::TRANSPARENT, Icon::Edit, c.muted, &c).on_hover_text("Editar").clicked() {
                                        edit = Some(VaultEdit { index: Some(i), entry: entry.clone(), ..Default::default() });
                                    }
                                });
                            });
                        }
                    });
                    ui.add_space(10.0);
                    let button = |text: &str| {
                        egui::Button::new(egui::RichText::new(text).size(12.5).color(c.text))
                            .fill(c.surface_low)
                            .corner_radius(8)
                            .min_size(Vec2::new(0.0, 28.0))
                    };
                    add = ui.add(button("+  Adicionar conta")).clicked();
                    if let Some(e) = &mut edit {
                        ui.add_space(16.0);
                        let title = if e.index.is_some() { "EDITAR CONTA" } else { "NOVA CONTA" };
                        ui.label(egui::RichText::new(title).size(11.0).strong().color(dialog_label(&c)));
                        dialog_field(ui, "Site", &mut e.entry.site, "domínio ou app, ex.: github.com", DIALOG_WIDTH, &c);
                        dialog_field(ui, "Usuário", &mut e.entry.user, "e-mail ou usuário", DIALOG_WIDTH, &c);
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Entrar com").size(11.5).color(dialog_label(&c)));
                        ui.add_space(4.0);
                        let mut kind = VaultKind::ALL.iter().position(|k| *k == e.entry.kind).unwrap_or_default();
                        if segmented(ui, &mut kind, &VaultKind::LABELS, &c) {
                            e.entry.kind = VaultKind::ALL[kind];
                        }
                        if e.entry.kind == VaultKind::Senha {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Senha").size(11.5).color(dialog_label(&c)));
                            ui.horizontal(|ui| {
                                let hint = if e.index.is_some() { "deixe vazio para manter a atual" } else { "" };
                                ui.add(
                                    egui::TextEdit::singleline(&mut e.secret)
                                        .password(!e.show)
                                        .hint_text(hint)
                                        .desired_width(DIALOG_WIDTH - 38.0)
                                        .margin(egui::Margin::symmetric(10, 7))
                                        .background_color(c.surface_low)
                                        .font(FontId::proportional(13.5)),
                                );
                                let tip = if e.show { "Esconder" } else { "Mostrar" };
                                if icon_button(ui, 30.0, e.show, Color32::TRANSPARENT, Icon::Eye, c.muted, &c).on_hover_text(tip).clicked() {
                                    e.show = !e.show;
                                }
                            });
                        } else {
                            dialog_note(ui, "Sem senha própria: o agente escolhe este e-mail e segue pelo botão do provedor.", &c);
                        }
                        dialog_field(ui, "Observação (opcional)", &mut e.entry.note, "ex.: conta do trabalho", DIALOG_WIDTH, &c);
                        if !e.error.is_empty() {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(&e.error).size(12.0).color(Color32::from_rgb(0xe0, 0x31, 0x31)));
                        }
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let primary = egui::Button::new(egui::RichText::new("Salvar").size(12.5).color(Color32::WHITE))
                                    .fill(PRIMARY)
                                    .corner_radius(8)
                                    .min_size(Vec2::new(0.0, 28.0));
                                save = ui.add(primary).clicked() || ctx.input(|i| i.key_pressed(Key::Enter));
                                cancel = ui.add(button("Cancelar")).clicked();
                            });
                        });
                    }
                });
        });
        if let Some(i) = remove {
            let _ = remove_secret(&entries[i].id);
            entries.remove(i);
            let _ = save_vault(&entries);
            if edit.as_ref().is_some_and(|e| e.index == Some(i)) {
                edit = None;
            }
        }
        if add {
            edit = Some(VaultEdit { entry: VaultEntry::new(), ..Default::default() });
        }
        if cancel {
            edit = None;
        }
        if save && let Some(e) = &mut edit {
            match keep(&mut entries, e) {
                Ok(()) => edit = None,
                Err(error) => e.error = error,
            }
        }
        self.vault = Some(entries);
        self.vault_edit = edit;
        if close {
            self.vault_open = false;
            self.vault = None;
            self.vault_edit = None;
        }
    }
}

/// Saves what the form holds: the password into the keychain, the entry into vault.json.
fn keep(entries: &mut Vec<VaultEntry>, e: &mut VaultEdit) -> Result<(), String> {
    e.entry.site = e.entry.site.trim().to_string();
    e.entry.user = e.entry.user.trim().to_string();
    e.entry.note = e.entry.note.trim().to_string();
    if e.entry.site.is_empty() || e.entry.user.is_empty() {
        return Err("Diga o site e o usuário.".into());
    }
    if e.entry.kind == VaultKind::Senha {
        if e.secret.is_empty() && e.index.is_none() {
            return Err("Digite a senha.".into());
        }
        if !e.secret.is_empty() {
            set_secret(&e.entry.id, &e.secret)?;
        }
    } else {
        remove_secret(&e.entry.id)?;
    }
    e.secret.clear();
    match e.index {
        Some(i) => entries[i] = e.entry.clone(),
        None => entries.push(e.entry.clone()),
    }
    save_vault(entries)
}

fn kind_icon(kind: VaultKind) -> Icon {
    match kind {
        VaultKind::Senha => Icon::Lock(true),
        VaultKind::Google => Icon::Label("G"),
        VaultKind::Apple => Icon::Label("A"),
    }
}
