//! Bandeja de ações e palco central (o aparelho desenhado com as teclas).
use super::{checks, is_blank, key_view, widgets, App, TAB_GAP};
use crate::config::schema::{ButtonAction, ButtonConfig};
use crate::editor::action_ui::Targets;
use crate::editor::library::Preset;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::{radius, space, text};
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{
    self, pos2, vec2, Align, Color32, Frame, Layout, Margin, Rect, RichText, Rounding, Sense,
    Stroke, Ui,
};

const TRAY_CARD: egui::Vec2 = egui::Vec2::new(104.0, 64.0);
const TRAY_MARGIN: (f32, f32) = (18.0, 10.0);
const TRAY_ICON_SIZE: f32 = 24.0;
const TRAY_HOVER_TINT: f32 = 0.10;
const SEARCH_WIDTH: f32 = 200.0;
const STAGE_MARGIN: f32 = 16.0;
const STAGE_FOOTER_HEIGHT: f32 = 30.0;
const GUIDE_HEIGHT: f32 = 120.0;
const OFFLINE_BANNER_HEIGHT: f32 = 60.0;
const KEY_MIN: f32 = 48.0;
// A imagem da tecla tem o tamanho do display (100 px); acima de ~1,4x ela fica borrada.
const KEY_MAX: f32 = 140.0;
const DEVICE_MAX_WIDTH: f32 = 780.0;
const DEVICE_PAD: f32 = 24.0;
const DEVICE_GAP: f32 = 12.0;
const BEZEL_INSET: f32 = 8.0;
const SCREEN_RADIUS: f32 = 16.0;
const LED_SIZE: egui::Vec2 = egui::Vec2::new(34.0, 3.0);
const LED_TOP: f32 = 9.0;
const LED_HALO: f32 = 3.0;
const LED_HALO_RADIUS: f32 = 4.0;
const LED_RADIUS: f32 = 2.0;

/// O que está sendo arrastado: uma ação da bandeja ou o conteúdo de uma tecla.
#[derive(Clone, Copy)]
pub(crate) enum Dragged {
    Preset(usize),
    Key(usize),
}

/// Resultado de um quadro do palco: soltou uma ação numa tecla, ou trocou duas teclas.
#[derive(Default)]
struct DeviceEvents {
    dropped: Option<(usize, usize)>,
    swapped: Option<(usize, usize)>,
}

/// Lado da tecla que cabe no espaço livre (entre `KEY_MIN` e `KEY_MAX`).
fn fit_key_size(avail_w: f32, avail_h: f32, rows: usize, cols: usize) -> f32 {
    let (r, c) = (rows as f32, cols as f32);
    let by_width = (avail_w.min(DEVICE_MAX_WIDTH) - 2.0 * DEVICE_PAD - (c - 1.0) * DEVICE_GAP) / c;
    let by_height = (avail_h - 2.0 * DEVICE_PAD - (r - 1.0) * DEVICE_GAP) / r;
    by_width.min(by_height).clamp(KEY_MIN, KEY_MAX)
}

fn device_size(key: f32, rows: usize, cols: usize) -> egui::Vec2 {
    let (r, c) = (rows as f32, cols as f32);
    vec2(
        2.0 * DEVICE_PAD + c * key + (c - 1.0) * DEVICE_GAP,
        2.0 * DEVICE_PAD + r * key + (r - 1.0) * DEVICE_GAP,
    )
}

/// Nome que o leitor de tela anuncia: posição, o que a tecla mostra, se precisa de ajuste e se
/// está selecionada.
fn key_widget_info(
    index: usize,
    b: &ButtonConfig,
    selected: bool,
    problem: Option<&str>,
) -> egui::WidgetInfo {
    let shown = &b.current_appearance().text;
    let what = if is_blank(b) {
        t("vazia")
    } else if shown.is_empty() {
        t("sem texto")
    } else {
        shown
    };
    let mut name = tf("Tecla {}, {}", &[&(index + 1), &what]);
    if let Some(problem) = problem {
        name = format!("{name}. {}", t(problem));
    }
    egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, name)
}

/// Corpo do aparelho: moldura, tela, aro interno e o LED de conexão.
fn paint_device_body(p: &egui::Painter, rect: Rect, pal: &Palette, accent: Color32, on: bool) {
    let screen = rect.shrink(BEZEL_INSET);
    let outer = Rounding::same(radius::DEVICE);
    let inner = Rounding::same(SCREEN_RADIUS);
    p.rect_filled(rect, outer, pal.bezel);
    p.rect_filled(screen, inner, pal.device);
    p.rect_stroke(
        screen,
        inner,
        Stroke::new(1.0_f32, Color32::from_black_alpha(160)),
    );
    p.rect_stroke(
        rect,
        outer,
        Stroke::new(1.5_f32, accent.gamma_multiply(0.4)),
    );
    let led = if on { accent } else { pal.bad };
    let led_rect = Rect::from_center_size(pos2(rect.center().x, rect.top() + LED_TOP), LED_SIZE);
    p.rect_filled(
        led_rect.expand(LED_HALO),
        Rounding::same(LED_HALO_RADIUS),
        led.gamma_multiply(0.25),
    );
    p.rect_filled(led_rect, Rounding::same(LED_RADIUS), led);
}

/// Cartão da bandeja: ícone e nome da ação, com tinta ao passar o mouse.
fn tray_card(ui: &mut Ui, p: &Preset, pal: &Palette, accent: Color32) -> egui::Response {
    let label = t(p.label);
    let (rect, resp) = ui.allocate_exact_size(TRAY_CARD, Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let ink = theme::ink(accent, pal);
    let hovered = resp.hovered();
    let fill = if hovered {
        theme::mix(pal.raise, accent, TRAY_HOVER_TINT)
    } else {
        pal.raise
    };
    let painter = ui.painter();
    let round = Rounding::same(radius::CARD);
    painter.rect_filled(rect, round, fill);
    let border = if hovered { ink } else { pal.control };
    painter.rect_stroke(rect, round, Stroke::new(1.0_f32, border));
    let icon_at = pos2(rect.center().x, rect.top() + TRAY_ICON_SIZE);
    widgets::paint_icon(painter, icon_at, TRAY_ICON_SIZE, p.icon, ink);
    painter.text(
        pos2(rect.center().x, rect.bottom() - 11.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(text::BODY - 1.0),
        pal.fg,
    );
    resp.on_hover_cursor(egui::CursorIcon::Grab)
}

/// Cartão final da bandeja: cria uma ação de atalho em branco para o usuário montar.
fn custom_card(ui: &mut Ui, pal: &Palette, accent: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(TRAY_CARD, Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("Ação personalizada"))
    });
    let ink = theme::ink(accent, pal);
    let border = if resp.hovered() { ink } else { pal.control };
    let painter = ui.painter();
    let round = Rounding::same(radius::CARD);
    painter.rect_stroke(rect, round, Stroke::new(1.0_f32, border));
    painter.text(
        pos2(rect.center().x, rect.top() + TRAY_ICON_SIZE),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::monospace(text::DISPLAY),
        ink,
    );
    painter.text(
        pos2(rect.center().x, rect.bottom() - 11.0),
        egui::Align2::CENTER_CENTER,
        t("Personalizada"),
        egui::FontId::proportional(text::CAPTION),
        pal.fg,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Minúsculas e sem acento, para "musica" achar "Música".
fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// A busca (já em `fold`) casa com o nome ou a categoria, em português ou no idioma da interface.
fn search_matches(folded_query: &str, texts: &[&str]) -> bool {
    texts.iter().any(|s| fold(s).contains(folded_query))
}

/// Passo a passo para quem abre uma página sem nenhuma tecla configurada.
fn empty_guide(ui: &mut Ui, pal: &Palette, accent: Color32, width: f32) {
    let ink = theme::ink(accent, pal);
    ui.set_max_width(width);
    Frame::none()
        .fill(pal.raise)
        .stroke(Stroke::new(1.0_f32, pal.control))
        .rounding(Rounding::same(radius::CARD))
        .inner_margin(Margin::same(space::MD))
        .show(ui, |ui| {
            ui.label(
                RichText::new(t("COMECE POR AQUI"))
                    .font(theme::display_font(text::CAPTION))
                    .color(pal.muted),
            );
            let steps = [
                "Clique numa tecla do aparelho",
                "Escolha uma ação na bandeja abaixo, ou arraste-a até a tecla",
                "Ajuste texto, cor e ícone no painel à direita",
            ];
            for (n, step) in steps.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}", n + 1)).monospace().color(ink));
                    ui.label(t(step));
                });
            }
        });
}

impl App {
    /// Bandeja de ações: categorias, busca e cartões que se aplicam por clique ou arrasto.
    pub(crate) fn tray(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let cats = self.tray_categories();
        let mut picked = None;
        let mut custom = false;
        let frame = Frame::none()
            .fill(pal.bg)
            .inner_margin(Margin::symmetric(TRAY_MARGIN.0, TRAY_MARGIN.1));
        egui::TopBottomPanel::bottom("tray")
            .frame(frame)
            .show(ctx, |ui| {
                self.tray_header(ui, &cats, pal, accent);
                let shown = self.visible_presets(&cats);
                if shown.is_empty() {
                    let msg = format!(
                        "Nenhuma ação encontrada para “{}”. Limpe a busca para ver todas.",
                        self.query
                    );
                    ui.label(RichText::new(msg).color(pal.muted));
                    return;
                }
                // Barra de rolagem sólida: a flutuante some e esconde que há mais cartões.
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                (picked, custom) = self.tray_cards(ui, ctx, &shown, pal, accent);
            });
        if let Some(i) = picked {
            self.apply_preset(g, self.selected, i);
        }
        if custom {
            self.apply_custom_action(g);
        }
    }

    /// Cartões rolantes da bandeja. Devolve a ação clicada e se "Personalizada" foi clicada.
    fn tray_cards(
        &self,
        ui: &mut Ui,
        ctx: &egui::Context,
        shown: &[usize],
        pal: &Palette,
        accent: Color32,
    ) -> (Option<usize>, bool) {
        let (mut picked, mut custom) = (None, false);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for &i in shown {
                    let resp = tray_card(ui, &self.presets[i], pal, accent);
                    if resp.clicked() {
                        picked = Some(i);
                    }
                    if resp.drag_started() {
                        egui::DragAndDrop::set_payload(ctx, Dragged::Preset(i));
                    }
                }
                if self.query.is_empty() {
                    custom = custom_card(ui, pal, accent).clicked();
                }
            });
        });
        (picked, custom)
    }

    fn tray_categories(&self) -> Vec<&'static str> {
        let mut cats: Vec<&'static str> = Vec::new();
        for p in &self.presets {
            if !cats.contains(&p.category) {
                cats.push(p.category);
            }
        }
        cats
    }

    fn tray_header(&mut self, ui: &mut Ui, cats: &[&'static str], pal: &Palette, accent: Color32) {
        ui.horizontal(|ui| {
            for (i, c) in cats.iter().enumerate() {
                let on = i == self.tray_cat && self.query.is_empty();
                if widgets::tab(ui, &t(c).to_uppercase(), on, accent, pal).clicked() {
                    self.tray_cat = i;
                    self.query.clear();
                }
                ui.add_space(TAB_GAP);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(widgets::hint("Buscar ação", pal))
                        .desired_width(SEARCH_WIDTH),
                );
                widgets::name(&search, "Buscar ação");
            });
        });
    }

    /// Índices das ações a mostrar: a categoria ativa, ou o resultado da busca.
    fn visible_presets(&self, cats: &[&'static str]) -> Vec<usize> {
        let q = fold(self.query.trim());
        self.presets
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                if q.is_empty() {
                    Some(&p.category) == cats.get(self.tray_cat)
                } else {
                    search_matches(&q, &[p.label, t(p.label), p.category, t(p.category)])
                }
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// "Ação personalizada": a tecla selecionada vira um atalho vazio e o inspetor abre na aba Ação.
    fn apply_custom_action(&mut self, g: &mut runtime::Shared) {
        let Some(b) = g.cfg.page_mut().buttons.get_mut(self.selected) else {
            return;
        };
        b.action = ButtonAction::Hotkey { keys: vec![] };
        self.tab = 1;
        self.touch();
    }

    /// Centro: o aparelho desenhado com as teclas, e o guia ou a dica logo abaixo.
    pub(crate) fn stage(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let frame = Frame::none()
            .fill(pal.bg)
            .inner_margin(Margin::same(STAGE_MARGIN));
        let mut events = DeviceEvents::default();
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        events = self.device_column(ui, ctx, g, pal, accent);
                    })
                });
        });
        self.apply_device_events(g, events);
    }

    fn device_column(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        g: &runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) -> DeviceEvents {
        let page_blank = g.cfg.page().buttons.iter().all(is_blank);
        let mut reserved = STAGE_FOOTER_HEIGHT;
        if !g.connected {
            reserved += OFFLINE_BANNER_HEIGHT;
        }
        if page_blank {
            reserved += GUIDE_HEIGHT;
        }
        let avail_h = ui.available_height() - reserved;
        if !g.connected {
            self.offline_banner(ui, pal);
        }
        let (rows, cols) = (g.cfg.page().rows, g.cfg.page().cols);
        let key = fit_key_size(ui.available_width(), avail_h, rows, cols);
        let (rect, _) = ui.allocate_exact_size(device_size(key, rows, cols), Sense::hover());
        paint_device_body(ui.painter(), rect, pal, accent, g.connected);
        let events = self.draw_keys(ui, ctx, g, rect, key, pal, accent);
        ui.add_space(space::SM - 2.0);
        if page_blank {
            empty_guide(ui, pal, accent, rect.width());
        } else {
            let hint = "Clique para editar · arraste uma ação da bandeja · arraste entre teclas para trocar";
            ui.label(
                RichText::new(t(hint))
                    .monospace()
                    .size(text::CAPTION)
                    .color(pal.muted),
            );
        }
        events
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_keys(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        g: &runtime::Shared,
        rect: Rect,
        key: f32,
        pal: &Palette,
        accent: Color32,
    ) -> DeviceEvents {
        let painter = ui.painter().clone();
        let cols = g.cfg.page().cols;
        let targets = Targets::from(&g.cfg);
        let mut events = DeviceEvents::default();
        for i in 0..g.cfg.page().rows * cols {
            let origin = pos2(
                rect.left() + DEVICE_PAD + (i % cols) as f32 * (key + DEVICE_GAP),
                rect.top() + DEVICE_PAD + (i / cols) as f32 * (key + DEVICE_GAP),
            );
            let kr = Rect::from_min_size(origin, vec2(key, key));
            let resp = ui.interact(kr, ui.id().with(("key", i)), Sense::click_and_drag());
            let button = &g.cfg.page().buttons[i];
            let problem = checks::button_problem(button, &targets);
            resp.widget_info(|| key_widget_info(i, button, i == self.selected, problem));
            // Tab e setas (navegação nativa do egui) movem o foco; a seleção o acompanha.
            if resp.has_focus() {
                self.selected = i;
            }
            if resp.drag_started() && !is_blank(button) {
                egui::DragAndDrop::set_payload(ctx, Dragged::Key(i));
            }
            let drop_target = resp.dnd_hover_payload::<Dragged>().is_some();
            let tex = self.key_tex.get(ctx, i, button, g.icon_px);
            let view = key_view(button, i + 1, tex, problem.is_some());
            let hot = resp.hovered() || drop_target;
            widgets::paint_key(&painter, kr, &view, hot, i == self.selected, pal, accent);
            let resp = match problem {
                Some(problem) => resp.on_hover_text(t(problem)),
                None => resp,
            };
            if resp.clicked() {
                self.selected = i;
                // O egui só dá foco por Tab; sem isto, as setas não funcionam após o clique.
                resp.request_focus();
            }
            if let Some(payload) = resp.dnd_release_payload::<Dragged>() {
                match *payload {
                    Dragged::Preset(p) => events.dropped = Some((i, p)),
                    Dragged::Key(from) if from != i => events.swapped = Some((from, i)),
                    Dragged::Key(_) => {}
                }
            }
        }
        events
    }

    fn apply_device_events(&mut self, g: &mut runtime::Shared, events: DeviceEvents) {
        if let Some((button, preset)) = events.dropped {
            self.selected = button;
            self.apply_preset(g, button, preset);
        }
        if let Some((from, to)) = events.swapped {
            let buttons = &mut g.cfg.page_mut().buttons;
            if from < buttons.len() && to < buttons.len() {
                buttons.swap(from, to);
                self.selected = to;
                self.touch();
                self.toasts.push("Teclas trocadas", Some("Desfazer"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_shrink_to_fit_a_short_stage_but_never_below_the_minimum() {
        // Com sobra de altura, a largura máxima do aparelho (780 px) é que limita a tecla.
        let by_width = (DEVICE_MAX_WIDTH - 2.0 * DEVICE_PAD - 4.0 * DEVICE_GAP) / 5.0;
        assert_eq!(fit_key_size(900.0, 2000.0, 3, 5), by_width);
        assert_eq!(fit_key_size(900.0, 2000.0, 3, 2), KEY_MAX);
        let mid = fit_key_size(900.0, 400.0, 3, 5);
        assert!(mid > KEY_MIN && mid < KEY_MAX, "{mid}");
        assert_eq!(fit_key_size(900.0, 50.0, 3, 5), KEY_MIN);
    }

    #[test]
    fn the_device_is_never_wider_than_its_cap() {
        let key = fit_key_size(5000.0, 5000.0, 3, 5);
        assert!(device_size(key, 3, 5).x <= DEVICE_MAX_WIDTH + 2.0 * DEVICE_PAD);
    }

    #[test]
    fn search_ignores_case_and_accents_and_checks_every_text() {
        assert!(search_matches(&fold("MUSICA"), &["Música"]));
        assert!(search_matches(&fold("ção"), &["Ação"]));
        assert!(search_matches(&fold("mute"), &["Mudo", "Mute"]));
        assert!(!search_matches(&fold("zzz"), &["Mudo", "Mute"]));
    }

    #[test]
    fn device_size_matches_the_grid() {
        let s = device_size(100.0, 3, 5);
        assert_eq!(s.x, 2.0 * DEVICE_PAD + 500.0 + 4.0 * DEVICE_GAP);
        assert_eq!(s.y, 2.0 * DEVICE_PAD + 300.0 + 2.0 * DEVICE_GAP);
    }
}
