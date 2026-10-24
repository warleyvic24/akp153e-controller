//! Painel do botão selecionado (inspetor), ações rápidas, banner offline e janela de apps instalados.
use super::{checks, is_blank, key_view, summary, theme, tokens, widgets, App, TAB_GAP};
use crate::config::schema::*;
use crate::editor::action_ui::{PaneCtx, Targets};
use crate::editor::panes::{action_pane, appearance_pane, states_pane};
use crate::editor::picker::apply_app;
use crate::editor::theme::Palette;
use crate::platform::{apps, shell};
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{self, vec2, Color32, Frame, Margin, RichText, Rounding, Sense, Stroke};
use std::time::Duration;

const INSPECTOR_WIDTH: f32 = 300.0;
const QUICK_BUTTON_PAD_X: f32 = 9.0;
const APP_ROW_HEIGHT: f32 = 36.0;
const APP_LIST_MAX_HEIGHT: f32 = 320.0;
const APP_BADGE: f32 = 24.0;
const APP_ROW_HOVER_TINT: f32 = 0.10;
const INSPECTOR_MARGIN: f32 = 14.0;
const PREVIEW_KEY: f32 = 56.0;
const TAB_NAMES: [&str; 3] = ["Aparência", "Ação", "Alternar"];

/// Título do inspetor: o rótulo; sem ele, o nome do programa que a tecla abre.
fn key_title(b: &ButtonConfig) -> String {
    if !b.appearance.text.is_empty() {
        return b.appearance.text.clone();
    }
    match &b.action {
        ButtonAction::LaunchApp { path, .. } => summary::program_name(path),
        _ => None,
    }
    .unwrap_or_else(|| t("Sem nome").to_string())
}

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

/// Lista rolável dos apps que casam com a busca; devolve o app clicado, se houver.
fn app_list(
    ui: &mut egui::Ui,
    list: &[apps::InstalledApp],
    query: &str,
    pal: &Palette,
) -> Option<apps::InstalledApp> {
    let q = query.to_lowercase();
    let mut chosen = None;
    egui::ScrollArea::vertical()
        .max_height(APP_LIST_MAX_HEIGHT)
        .show(ui, |ui| {
            let mut shown = 0;
            for app in list
                .iter()
                .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
            {
                shown += 1;
                if app_row(ui, app, pal).clicked() {
                    chosen = Some(app.clone());
                }
            }
            if shown == 0 {
                let msg = if list.is_empty() {
                    t("Nenhum programa encontrado. Use “Procurar .exe”.").to_string()
                } else {
                    tf("Nada encontrado para “{}”.", &[&query])
                };
                ui.label(RichText::new(msg).color(pal.muted));
            }
        });
    chosen
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
            RichText::new(t("Apps instalados"))
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
                ui.label(t("Procurando programas instalados…"));
                return;
            };
            chosen = app_list(ui, list, &self.picker.query, &pal);
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
                    ui.label(RichText::new(t("Aparelho desconectado.")).strong());
                    ui.label(t(
                        "Suas alterações ficam salvas e vão ao aparelho ao reconectar.",
                    ));
                    if ui.button(t("Reconectar")).clicked() {
                        let _ = shell::send("reconnect");
                    }
                });
            });
        ui.add_space(tokens::space::SM + 2.0);
    }

    fn quick_actions(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        untestable: Option<&'static str>,
        pal: &Palette,
        accent: Color32,
    ) {
        let sel = self.selected;
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding.x = QUICK_BUTTON_PAD_X;
            let label = RichText::new(t("Testar"))
                .strong()
                .color(theme::readable_on(accent));
            let test = egui::Button::new(label)
                .fill(accent)
                .stroke(Stroke::new(1.0_f32, theme::ink(accent, pal)));
            let test = ui.add_enabled(untestable.is_none(), test);
            let test = match untestable {
                Some(why) => test.on_disabled_hover_text(t(why)),
                None => test.on_hover_text(t("Executa a ação agora")),
            };
            if test.clicked() {
                self.test_key(g, sel);
            }
            let copy = ui.button(t("Copiar")).on_hover_text("Ctrl+C");
            if copy.clicked() {
                self.copy_key(ui.ctx(), g, sel);
            }
            let paste = egui::Button::new(t("Colar"));
            let paste = ui
                .add_enabled(self.clipboard.is_some(), paste)
                .on_hover_text("Ctrl+V");
            if paste.clicked() {
                self.paste_key(g, sel);
            }
            // Destrutivo: no fim da linha, longe de Copiar, para não ser clicado por engano.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let clear = egui::Button::new(RichText::new(t("Limpar")).color(pal.bad));
                let clear = ui.add(clear).on_hover_text(t("Esvazia este botão (Del)"));
                clear.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("Limpar botão"))
                });
                if clear.clicked() {
                    self.clear_key(g, sel);
                }
            });
        });
    }

    fn test_key(&mut self, g: &mut runtime::Shared, sel: usize) {
        runtime::trigger(g, &self.tx, &self.exec, sel, Duration::ZERO);
        let msg = tf("Tecla {} testada", &[&(sel + 1)]);
        self.toasts.push(msg, None);
    }

    fn copy_key(&mut self, ctx: &egui::Context, g: &runtime::Shared, sel: usize) {
        self.clipboard = g.cfg.page().buttons.get(sel).cloned();
        // O Windows só entrega Ctrl+V ao app se a área de transferência tiver texto; por isso vai
        // um texto curto. A tecla em si fica só na cópia interna: URLs com token não vazam.
        let name = self
            .clipboard
            .as_ref()
            .map_or("", |b| b.appearance.text.as_str());
        ctx.copy_text(
            format!("AKP153E #{} {name}", sel + 1)
                .trim_end()
                .to_string(),
        );
        let msg = tf("Tecla {} copiada", &[&(sel + 1)]);
        self.toasts.push(msg, None);
    }

    /// Atalhos sobre a tecla selecionada: Del limpa, Ctrl+C copia, Ctrl+V cola. Ficam desligados
    /// enquanto se digita, se grava um atalho ou há uma janela aberta.
    pub(crate) fn key_shortcuts(&mut self, ctx: &egui::Context, g: &mut runtime::Shared) {
        let busy = self.recording.is_some() || self.picker.open || self.settings_open;
        if busy || ctx.wants_keyboard_input() {
            return;
        }
        let (copy, paste, clear) = ctx.input_mut(|i| {
            let copy = i.events.iter().any(|e| matches!(e, egui::Event::Copy));
            let paste = i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)));
            let clear = i.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
            (copy, paste, clear)
        });
        let sel = self.selected.min(g.cfg.page().buttons.len() - 1);
        if copy {
            self.copy_key(ctx, g, sel);
        } else if paste {
            self.paste_key(g, sel);
        } else if clear && !is_blank(&g.cfg.page().buttons[sel]) {
            self.clear_key(g, sel);
        }
    }

    fn paste_key(&mut self, g: &mut runtime::Shared, sel: usize) {
        let Some(copied) = self.clipboard.clone() else {
            return;
        };
        let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) else {
            return;
        };
        *b = copied;
        self.touch();
        let msg = tf("Colado na tecla {}", &[&(sel + 1)]);
        self.toasts.push(msg, Some("Desfazer"));
    }

    fn clear_key(&mut self, g: &mut runtime::Shared, sel: usize) {
        let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) else {
            return;
        };
        *b = ButtonConfig::blank();
        self.touch();
        let msg = tf("Tecla {} limpa", &[&(sel + 1)]);
        self.toasts.push(msg, Some("Desfazer"));
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
            .inner_margin(Margin::same(INSPECTOR_MARGIN));
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
                let Some(b) = g.cfg.page().buttons.get(sel) else {
                    return;
                };
                // Primeiro qual tecla é e o que faz; depois o que dá para fazer com ela.
                self.key_header(ui, ctx, b, sel, (icon_px, &targets), pal, accent);
                if let Some(problem) = checks::button_problem(b, &targets) {
                    let msg = RichText::new(format!("! {}", t(problem)))
                        .size(tokens::text::CAPTION)
                        .color(pal.bad);
                    ui.label(msg);
                }
                let untestable = match b.action {
                    ButtonAction::None => Some(checks::NO_ACTION),
                    _ => checks::action_problem(&b.action, &targets),
                };
                ui.add_space(tokens::space::XS);
                self.quick_actions(ui, g, untestable, pal, accent);
                ui.add_space(tokens::space::SM);
                let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) else {
                    return;
                };
                self.tab_bar(ui, accent, pal);
                ui.separator();
                let cx = PaneCtx {
                    ctx,
                    pal,
                    accent,
                    targets: &targets,
                };
                if self.tab_body(ui, &cx, b, sel) {
                    self.touch();
                }
            });
    }

    /// Miniatura da tecla, número, nome e o resumo da ação; a dica quando ainda está vazia.
    #[allow(clippy::too_many_arguments)]
    fn key_header(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        b: &ButtonConfig,
        sel: usize,
        (icon_px, targets): (u32, &Targets),
        pal: &Palette,
        accent: Color32,
    ) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(PREVIEW_KEY, PREVIEW_KEY), Sense::hover());
            let tex = self.key_tex.get(ctx, sel, b, icon_px);
            let view = key_view(b, sel + 1, tex, false);
            widgets::paint_key(ui.painter(), rect, &view, false, false, pal, accent);
            ui.vertical(|ui| {
                let number = RichText::new(tf("BOTÃO {}", &[&(sel + 1)]))
                    .monospace()
                    .size(tokens::text::CAPTION)
                    .color(pal.muted);
                ui.label(number);
                let title = theme::display_font(tokens::text::TITLE);
                ui.label(RichText::new(key_title(b)).font(title));
                if let Some(summary) = summary::action_summary(&b.action, targets) {
                    ui.label(
                        RichText::new(summary)
                            .size(tokens::text::CAPTION)
                            .color(pal.muted),
                    );
                }
            });
        });
        if is_blank(b) {
            let hint = t("Escolha uma ação na bandeja ou comece pelo rótulo.");
            let hint = RichText::new(hint)
                .size(tokens::text::CAPTION)
                .color(pal.muted);
            ui.label(hint);
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, accent: Color32, pal: &Palette) {
        ui.horizontal(|ui| {
            for (i, name) in TAB_NAMES.iter().enumerate() {
                if widgets::tab(ui, name, self.tab == i, accent, pal).clicked() {
                    self.tab = i;
                }
                ui.add_space(TAB_GAP);
            }
        });
    }

    /// Conteúdo da aba ativa; devolve `true` se algo mudou.
    fn tab_body(
        &mut self,
        ui: &mut egui::Ui,
        cx: &PaneCtx,
        b: &mut ButtonConfig,
        sel: usize,
    ) -> bool {
        let mut changed = false;
        // Um estado de rolagem por botão e aba: trocar de um para outro volta ao topo.
        let scroll = egui::ScrollArea::vertical().id_salt(("inspector", sel, self.tab));
        scroll.show(ui, |ui| match self.tab {
            0 => changed |= appearance_pane(ui, b, cx.pal, cx.accent),
            1 => changed |= action_pane(ui, cx, b, &mut self.recording, &mut self.picker),
            _ => changed |= states_pane(ui, b, cx.pal),
        });
        changed
    }
}
