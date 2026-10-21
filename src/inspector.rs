//! Painel do botão selecionado (inspetor), ações rápidas, banner offline e janela de apps instalados.
use crate::action_ui::{PaneCtx, Targets};
use crate::config::schema::*;
use crate::panes::{action_pane, appearance_pane, states_pane};
use crate::picker::apply_app;
use crate::theme::Palette;
use crate::{apps, is_blank, key_view, runtime, shell, theme, tokens, widgets, App, TAB_GAP};
use eframe::egui::{self, vec2, Color32, Frame, Margin, RichText, Rounding, Sense, Stroke};
use std::time::Duration;

const INSPECTOR_WIDTH: f32 = 300.0;
const QUICK_BUTTON_PAD_X: f32 = 9.0;
const APP_ROW_HEIGHT: f32 = 36.0;
const APP_BADGE: f32 = 24.0;
const APP_ROW_HOVER_TINT: f32 = 0.10;
const TAB_NAMES: [&str; 3] = ["Aparência", "Ação", "Alternar"];

/// Linha da lista de apps: inicial colorida, nome e o caminho ao passar o mouse.
fn app_row(ui: &mut egui::Ui, app: &apps::InstalledApp, pal: &Palette) -> egui::Response {
    let size = vec2(ui.available_width(), APP_ROW_HEIGHT);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, app.name.as_str())
    });
    let p = ui.painter();
    if resp.hovered() {
        let tint = theme::mix(pal.raise, pal.fg, APP_ROW_HOVER_TINT);
        p.rect_filled(rect, Rounding::same(tokens::radius::CONTROL), tint);
    }
    let seed: usize = app.name.bytes().map(usize::from).sum();
    let [r, g, b] = theme::ACCENTS[seed % theme::ACCENTS.len()].1;
    let badge_color = Color32::from_rgb(r, g, b);
    let center = egui::pos2(
        rect.left() + tokens::space::SM + APP_BADGE / 2.0,
        rect.center().y,
    );
    p.circle_filled(center, APP_BADGE / 2.0, badge_color);
    let initial = app
        .name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    p.text(
        center,
        egui::Align2::CENTER_CENTER,
        initial,
        egui::FontId::proportional(tokens::text::BODY),
        theme::readable_on(badge_color),
    );
    let name_at = egui::pos2(
        center.x + APP_BADGE / 2.0 + tokens::space::SM,
        rect.center().y,
    );
    p.with_clip_rect(rect).text(
        name_at,
        egui::Align2::LEFT_CENTER,
        &app.name,
        egui::FontId::proportional(tokens::text::BODY),
        pal.fg,
    );
    resp.on_hover_text(&app.target)
}

impl App {
    pub(crate) fn app_picker(&mut self, ctx: &egui::Context, g: &mut runtime::Shared) {
        self.picker.poll();
        self.apply_finished_icons(g);
        if !self.picker.open {
            return;
        }
        let mut open = true;
        let pal = theme::palette(g.cfg.ui.theme);
        let mut chosen: Option<apps::InstalledApp> = None;
        egui::Window::new(
            RichText::new("Apps instalados")
                .size(tokens::text::BODY)
                .strong(),
        )
        .open(&mut open)
        .collapsible(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.picker.query)
                    .hint_text(widgets::hint("Buscar app", &pal))
                    .desired_width(ui.available_width()),
            );
            widgets::name(&search, "Buscar app");
            let Some(list) = &self.picker.apps else {
                ui.label("Procurando programas instalados…");
                return;
            };
            let q = self.picker.query.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    let mut shown = 0;
                    for app in list
                        .iter()
                        .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
                    {
                        shown += 1;
                        if app_row(ui, app, &pal).clicked() {
                            chosen = Some(app.clone());
                        }
                    }
                    if shown == 0 {
                        let msg = if list.is_empty() {
                            "Nenhum programa encontrado. Use “Procurar .exe”.".to_string()
                        } else {
                            format!("Nada encontrado para “{}”.", self.picker.query)
                        };
                        ui.label(RichText::new(msg).color(pal.muted));
                    }
                });
        });
        if let Some(app) = chosen {
            let at = self.picker.at;
            if let Some(b) = g.cfg.page_mut().buttons.get_mut(at.2) {
                apply_app(b, &app, at, &self.picker.jobs);
            }
            self.touch();
            open = false;
        }
        self.picker.open = open;
    }

    pub(crate) fn offline_banner(&self, ui: &mut egui::Ui, pal: &Palette) {
        Frame::none()
            .fill(pal.bad.gamma_multiply(0.15))
            .stroke(Stroke::new(1.0_f32, pal.bad.gamma_multiply(0.5)))
            .rounding(Rounding::same(tokens::radius::CARD))
            .inner_margin(Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Aparelho desconectado.").strong());
                    ui.label("Suas alterações ficam salvas e vão ao aparelho ao reconectar.");
                    if ui.button("Reconectar").clicked() {
                        let _ = shell::send("reconnect");
                    }
                });
            });
        ui.add_space(10.0);
    }

    fn quick_actions(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let sel = self.selected;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().button_padding.x = QUICK_BUTTON_PAD_X;
            let test = egui::Button::new(
                RichText::new("Testar")
                    .strong()
                    .color(theme::readable_on(accent)),
            )
            .fill(accent);
            let test = ui.add(test.stroke(Stroke::new(1.0_f32, theme::ink(accent, pal))));
            if test.clicked() {
                runtime::trigger(g, &self.tx, &self.exec, sel, Duration::ZERO);
                self.toasts.push(format!("Tecla {} testada", sel + 1), None);
            }
            if ui.button("Copiar").clicked() {
                self.clipboard = g.cfg.page().buttons.get(sel).cloned();
                self.toasts.push(format!("Tecla {} copiada", sel + 1), None);
            }
            if ui
                .add_enabled(self.clipboard.is_some(), egui::Button::new("Colar"))
                .clicked()
            {
                if let (Some(c), Some(b)) = (
                    self.clipboard.clone(),
                    g.cfg.page_mut().buttons.get_mut(sel),
                ) {
                    *b = c;
                    self.touch();
                    self.toasts
                        .push(format!("Colado na tecla {}", sel + 1), Some("Desfazer"));
                }
            }
            let clear = egui::Button::new(RichText::new("Limpar").color(pal.bad));
            let clear = ui.add(clear).on_hover_text("Esvazia este botão");
            clear.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Limpar botão")
            });
            if clear.clicked() {
                if let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) {
                    *b = ButtonConfig::blank();
                    self.touch();
                    self.toasts
                        .push(format!("Tecla {} limpa", sel + 1), Some("Desfazer"));
                }
            }
        });
    }

    /// Painel do botão selecionado, com as abas Aparência, Ação e Alternar.
    pub(crate) fn inspector(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let frame = Frame::none()
            .fill(pal.raise)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .inner_margin(Margin::same(14.0));
        egui::SidePanel::right("insp")
            .exact_width(INSPECTOR_WIDTH)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                let sel = self.selected.min(g.cfg.page().buttons.len() - 1);
                self.selected = sel;
                let targets = Targets::from(&g.cfg);
                let icon_px = g.icon_px;
                let profile = g.cfg.active_profile;
                self.picker.at = (profile, g.cfg.profiles[profile].active_page, sel);
                self.quick_actions(ui, g, pal, accent);
                ui.add_space(4.0);
                let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) else {
                    return;
                };
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                    let tex = self.key_tex.get(ctx, sel, b, icon_px);
                    widgets::paint_key(
                        ui.painter(),
                        rect,
                        &key_view(b, sel + 1, tex),
                        false,
                        false,
                        pal,
                        accent,
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!("BOTÃO {}", sel + 1))
                                .monospace()
                                .size(tokens::text::CAPTION)
                                .color(pal.muted),
                        );
                        let name = if b.appearance.text.is_empty() {
                            "Sem nome"
                        } else {
                            b.appearance.text.as_str()
                        };
                        ui.label(
                            RichText::new(name).font(theme::display_font(tokens::text::TITLE)),
                        );
                    });
                });
                if is_blank(b) {
                    ui.label(
                        RichText::new("Escolha uma ação na bandeja ou comece pelo rótulo.")
                            .size(12.0)
                            .color(pal.muted),
                    );
                }
                ui.horizontal(|ui| {
                    for (i, name) in TAB_NAMES.iter().enumerate() {
                        if widgets::tab(ui, name, self.tab == i, accent, pal).clicked() {
                            self.tab = i;
                        }
                        ui.add_space(TAB_GAP);
                    }
                });
                ui.separator();
                let mut changed = false;
                // Um estado de rolagem por botão e aba: trocar de um para outro volta ao topo.
                let scroll = egui::ScrollArea::vertical().id_salt(("inspector", sel, self.tab));
                scroll.show(ui, |ui| match self.tab {
                    0 => changed |= appearance_pane(ui, b, pal, accent),
                    1 => {
                        let cx = PaneCtx {
                            ctx,
                            pal,
                            accent,
                            targets: &targets,
                        };
                        changed |= action_pane(ui, &cx, b, &mut self.recording, &mut self.picker)
                    }
                    _ => changed |= states_pane(ui, b, pal),
                });
                if changed {
                    self.touch();
                }
            });
    }
}
