//! Painel do botão selecionado (inspetor), ações rápidas, banner offline e janela de apps instalados.
use super::{checks, is_blank, key_view, summary, theme, tokens, widgets, App};
use crate::config::schema::*;
use crate::editor::action_ui::{PaneCtx, Targets};
use crate::editor::panes::{action_pane, appearance_pane};
use crate::editor::picker::apply_app;
use crate::editor::theme::Palette;
use crate::platform::{apps, shell};
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{self, vec2, Color32, CornerRadius, Frame, Margin, RichText, Sense, Stroke};
use std::time::Duration;

const APP_ROW_HEIGHT: f32 = 36.0;
const APP_LIST_MAX_HEIGHT: f32 = 320.0;
const APP_BADGE: f32 = 24.0;
const APP_ROW_HOVER_TINT: f32 = 0.10;
const INSPECTOR_MARGIN: f32 = 14.0;
const PREVIEW_KEY: f32 = 72.0;
// O que a tecla faz vem antes de como ela parece.
const TAB_NAMES: [&str; 2] = ["Ação", "Aparência"];
pub(crate) const TAB_ACTION: usize = 0;
const TEST_HEIGHT: f32 = 32.0;
const MORE_MENU_WIDTH: f32 = 180.0;
const LIVE_UNTESTABLE: &str = "Esta tecla só mostra informação; não há o que testar";

/// Título do inspetor: o rótulo (numa linha só); sem ele, um nome curto do que a tecla faz (o
/// programa, a tecla de mídia, o atalho). `None` quando não há nada a dizer (o cabeçalho mostra
/// um marcador apagado, não um nome).
pub(crate) fn key_title(b: &ButtonConfig) -> Option<String> {
    let text = b
        .appearance
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !text.is_empty() {
        return Some(text);
    }
    match &b.action {
        ButtonAction::LaunchApp { path, .. } => summary::program_name(path),
        ButtonAction::MediaKey { key } => {
            Some(t(crate::editor::action_ui::media_label(key)).to_string())
        }
        ButtonAction::Hotkey { keys } if !keys.is_empty() => Some(summary::chord(keys)),
        ButtonAction::OpenMixer => Some(t("Mixer").to_string()),
        ButtonAction::Clock => Some(t("Relógio").to_string()),
        ButtonAction::Monitor { metric } => {
            Some(t(super::monitor_ui::metric_label(*metric)).to_string())
        }
        ButtonAction::PlaySound { path } => summary::sound_name(path),
        ButtonAction::Obs { command, target } if target.is_empty() => {
            Some(t(super::obs_ui::command_label(*command)).to_string())
        }
        ButtonAction::Obs { target, .. } => Some(target.clone()),
        ButtonAction::Brightness { delta } if *delta >= 0 => Some(t("Brilho +").to_string()),
        ButtonAction::Brightness { .. } => Some(t("Brilho −").to_string()),
        ButtonAction::NextPage => Some(t("Próxima página").to_string()),
        ButtonAction::PreviousPage => Some(t("Página anterior").to_string()),
        _ => None,
    }
}

/// Linha da lista de apps: ícone (ou a inicial colorida enquanto ele não chega), nome e o
/// caminho ao passar o mouse.
fn app_row(
    ui: &mut egui::Ui,
    app: &apps::InstalledApp,
    icon: Option<egui::TextureId>,
    pal: &Palette,
) -> egui::Response {
    let size = vec2(ui.available_width(), APP_ROW_HEIGHT);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, app.name.as_str()));
    let p = ui.painter();
    if resp.hovered() {
        let tint = theme::mix(pal.raise, pal.fg, APP_ROW_HOVER_TINT);
        p.rect_filled(rect, CornerRadius::from(tokens::radius::CONTROL), tint);
    }
    let seed: usize = app.name.bytes().map(usize::from).sum();
    let [r, g, b] = theme::ACCENTS[seed % theme::ACCENTS.len()].1;
    let badge_color = Color32::from_rgb(r, g, b);
    let center = egui::pos2(
        rect.left() + tokens::space::SM + APP_BADGE / 2.0,
        rect.center().y,
    );
    if let Some(tex) = icon {
        let r = egui::Rect::from_center_size(center, vec2(APP_BADGE, APP_BADGE));
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        p.image(tex, r, uv, Color32::WHITE);
    } else {
        paint_initial(p, center, app, badge_color);
    }
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

fn paint_initial(
    p: &egui::Painter,
    center: egui::Pos2,
    app: &apps::InstalledApp,
    badge_color: Color32,
) {
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
}

/// Lista rolável dos apps que casam com a busca; devolve o app clicado, se houver.
fn app_list(
    ui: &mut egui::Ui,
    list: &[apps::InstalledApp],
    query: &str,
    icons: &mut super::app_icons::AppIcons,
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
                // Só as linhas visíveis pedem o ícone (a lista pode ter centenas de apps).
                let visible = ui.is_rect_visible(egui::Rect::from_min_size(
                    ui.cursor().min,
                    vec2(ui.available_width(), APP_ROW_HEIGHT),
                ));
                let icon = visible.then(|| icons.get(ui.ctx(), app)).flatten();
                if app_row(ui, app, icon, pal).clicked() {
                    chosen = Some(app.clone());
                }
            }
            if shown == 0 {
                let msg = if list.is_empty() {
                    t(crate::editor::picker::NO_APPS_FOUND).to_string()
                } else {
                    tf("Nada encontrado para “{}”.", &[&query])
                };
                ui.label(RichText::new(msg).color(pal.muted));
            }
        });
    chosen
}

/// Item de menu com o atalho à direita.
fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(enabled, egui::Button::new(t(label)).shortcut_text(shortcut))
}

/// Id da janela da lista de apps (o título muda com o idioma).
pub(crate) const APP_PICKER_ID: &str = "app-picker";
const PICKER_WIDTH: f32 = 420.0;

impl App {
    pub(crate) fn app_picker(&mut self, ctx: &egui::Context, g: &mut runtime::Shared) {
        self.picker.poll();
        self.favicons.poll(&g.cfg, &self.picker.jobs.tx);
        self.apply_finished_icons(g);
        if !self.picker.open {
            return;
        }
        let mut open = true;
        let pal = theme::palette(g.cfg.ui.theme);
        let mut chosen: Option<apps::InstalledApp> = None;
        let ((), close) = widgets::modal(
            ctx,
            APP_PICKER_ID,
            t("Escolha um app"),
            PICKER_WIDTH,
            &pal,
            |ui| {
                let search = widgets::search_field(ui, &mut self.picker.query, "Buscar app", &pal);
                if std::mem::take(&mut self.picker.focus_search) {
                    search.request_focus();
                }
                ui.add_space(tokens::space::SM);
                let Some(list) = &self.picker.apps else {
                    ui.label(t("Procurando programas instalados…"));
                    return;
                };
                chosen = app_list(ui, list, &self.picker.query, &mut self.app_icons, &pal);
            },
        );
        if close {
            open = false;
        }
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

    /// Aviso acima do aparelho, da largura dele. Sem serviço, nada chega ao aparelho: o botão o
    /// inicia em vez de pedir uma reconexão que ninguém atenderia.
    pub(crate) fn offline_banner(
        &self,
        ui: &mut egui::Ui,
        pal: &Palette,
        service_running: bool,
        width: f32,
    ) {
        let (title, body, button) = if service_running {
            (
                "Aparelho desconectado.",
                "Suas alterações ficam salvas e vão ao aparelho ao reconectar.",
                "Reconectar",
            )
        } else {
            (
                "Serviço parado.",
                "Ele é quem fala com o aparelho; sem ele as teclas não respondem.",
                "Iniciar serviço",
            )
        };
        ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
            Frame::NONE
                .fill(pal.bad.gamma_multiply(0.15))
                .stroke(Stroke::new(1.0_f32, pal.bad.gamma_multiply(0.5)))
                .corner_radius(CornerRadius::from(tokens::radius::CARD))
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(t(button)).clicked() {
                                if service_running {
                                    let _ = shell::send("reconnect");
                                } else {
                                    shell::start_service();
                                }
                            }
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(RichText::new(t(title)).strong());
                                        ui.label(t(body));
                                    });
                                },
                            );
                        });
                    });
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
            // Ícones à direita (o layout os desenha ao contrário); "Testar" ocupa o resto.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Copiar, colar e limpar num menu com nome e atalho: o limpar (destrutivo) sai de
                // perto do "Testar", e os ícones soltos deixam de ser adivinhação.
                let more = widgets::icon_button(ui, "more", "Mais ações da tecla", true, pal);
                egui::Popup::menu(&more).show(|ui| {
                    ui.set_min_width(MORE_MENU_WIDTH);
                    if menu_item(ui, "Copiar", "Ctrl+C", true).clicked() {
                        self.copy_key(ui.ctx(), g, sel);
                    }
                    let can_paste = self.clipboard.is_some();
                    if menu_item(ui, "Colar", "Ctrl+V", can_paste).clicked() {
                        self.paste_key(g, sel);
                    }
                    ui.separator();
                    let clear = egui::Button::new(RichText::new(t("Limpar tecla")).color(pal.bad))
                        .shortcut_text("Del");
                    if ui.add(clear).clicked() {
                        self.clear_key(g, sel);
                    }
                });
                // Sem o que testar, o botão perde o preenchimento: um primário apagado ainda
                // parece clicável. Neutro com borda deixa claro que está desligado.
                // Secundário: testar é ocasional, não a ação principal do painel.
                let (fill, ink, stroke) = if untestable.is_none() {
                    (
                        pal.raise,
                        theme::ink(accent, pal),
                        Stroke::new(1.0_f32, pal.control),
                    )
                } else {
                    (pal.field, pal.muted, Stroke::new(1.0_f32, pal.control))
                };
                let label = RichText::new(format!("▶  {}", t("Testar")))
                    .strong()
                    .color(ink);
                let test = egui::Button::new(label)
                    .fill(fill)
                    .stroke(stroke)
                    .min_size(vec2(ui.available_width(), TEST_HEIGHT));
                let test = ui.add_enabled(untestable.is_none(), test);
                let test = match untestable {
                    Some(why) => test.on_disabled_hover_text(t(why)),
                    None => test.on_hover_text(t("Executa a ação agora")),
                };
                test.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, untestable.is_none(), t("Testar"))
                });
                if test.clicked() {
                    self.test_key(g, sel);
                }
            });
        });
    }

    fn test_key(&mut self, g: &mut runtime::Shared, sel: usize) {
        let brightness = g.cfg.ui.brightness;
        runtime::trigger(g, &self.tx, &self.exec, sel, Duration::ZERO);
        // O brilho é do aparelho: muda aqui e vai a ele pelo salvamento (o "reload" do serviço).
        if g.cfg.ui.brightness != brightness {
            self.touch();
        }
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
        if busy || ctx.egui_wants_keyboard_input() {
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
        root: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
        width: f32,
    ) {
        let ctx = &root.ctx().clone();
        let frame = Frame::NONE
            .fill(pal.panel)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .inner_margin(Margin::same(INSPECTOR_MARGIN as i8));
        egui::Panel::right("insp")
            .exact_size(width)
            .resizable(false)
            .frame(frame)
            .show(root, |ui| {
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
                    ButtonAction::Clock | ButtonAction::Monitor { .. } => Some(LIVE_UNTESTABLE),
                    _ => checks::action_problem(&b.action, &targets),
                };
                ui.add_space(tokens::space::XS);
                self.quick_actions(ui, g, untestable, pal, accent);
                ui.add_space(tokens::space::SM);
                let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) else {
                    return;
                };
                self.tab_bar(ui, pal);
                ui.add_space(tokens::space::SM);
                let cx = PaneCtx {
                    ctx,
                    pal,
                    accent,
                    targets: &targets,
                };
                if self.tab_body(ui, &cx, b, sel) {
                    self.touch();
                }
                if let Some(id) = self.open_page.take() {
                    self.go_to_page(g, id);
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
            let live = crate::obs::shown(b, &self.obs_live.state);
            let tex = self.key_tex.get(ctx, sel, &live, icon_px);
            let view = key_view(b, None, tex, false);
            widgets::paint_key(ui.painter(), rect, &view, false, false, pal, accent);
            ui.vertical(|ui| {
                let number = RichText::new(tf("Tecla {}", &[&(sel + 1)]))
                    .size(tokens::text::CAPTION)
                    .color(pal.muted);
                ui.label(number);
                let title = theme::display_font(tokens::text::TITLE);
                // Sem nome é ausência, não conteúdo: apagado, para não competir com o que importa.
                match key_title(b) {
                    Some(name) => ui.label(RichText::new(name).font(title)),
                    None => ui.label(RichText::new(t("Sem nome")).font(title).color(pal.muted)),
                };
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
            let hint = t("Escolha uma ação em Ações ou dê um nome em Aparência.");
            let hint = RichText::new(hint)
                .size(tokens::text::CAPTION)
                .color(pal.muted);
            ui.label(hint);
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        // Abas do mesmo tamanho, ocupando a largura do painel (trilho de 3 px de cada lado).
        let width = (ui.available_width() - 6.0 - 2.0 * (TAB_NAMES.len() - 1) as f32)
            / TAB_NAMES.len() as f32;
        widgets::segmented(ui, pal, |ui| {
            for (i, name) in TAB_NAMES.iter().enumerate() {
                if widgets::segment(ui, name, self.tab == i, width, pal).clicked() {
                    self.tab = i;
                }
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
            TAB_ACTION => {
                changed |= action_pane(
                    ui,
                    cx,
                    b,
                    &mut self.recording,
                    &mut self.picker,
                    &mut self.open_page,
                )
            }
            _ => changed |= appearance_pane(ui, b, cx.pal, cx.accent),
        });
        changed
    }
}
