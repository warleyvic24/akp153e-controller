//! Palco central: as abas de página, o aparelho desenhado com as teclas e os atalhos.
use super::{checks, is_blank, key_view, widgets, App};
use crate::config::schema::ButtonConfig;
use crate::editor::action_ui::Targets;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::{radius, space, text};
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{
    self, pos2, vec2, Color32, Frame, Margin, Rect, RichText, Rounding, Sense, Stroke, Ui,
};

const STAGE_MARGIN: f32 = 16.0;
const STAGE_FOOTER_HEIGHT: f32 = 30.0;
const PAGE_TABS_HEIGHT: f32 = 40.0;
// Título e três passos de uma linha, com margens; medido na tela (o antigo 120 cortava o fim).
const GUIDE_HEIGHT: f32 = 170.0;
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

/// O que está sendo arrastado: uma ação da biblioteca ou o conteúdo de uma tecla.
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

/// Atalhos do palco em forma de tecla, para quem não sabe que eles existem.
fn shortcut_hints(ui: &mut Ui, pal: &Palette) {
    let hints = [
        ("Del", "limpar"),
        ("Ctrl C", "copiar"),
        ("Ctrl V", "colar"),
        ("Arraste", "trocar teclas"),
    ];
    for (key, what) in hints {
        widgets::kbd(ui, t(key), pal);
        ui.label(RichText::new(t(what)).size(text::CAPTION).color(pal.muted));
        ui.add_space(space::SM);
    }
}

/// Passo a passo para quem abre uma página sem nenhuma tecla configurada.
fn empty_guide(ui: &mut Ui, pal: &Palette, accent: Color32, width: f32) {
    let ink = theme::ink(accent, pal);
    // Da largura do aparelho e alinhado à esquerda, como as páginas, não ao centro do palco.
    ui.allocate_ui(vec2(width, 0.0), |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            Frame::none()
                .fill(pal.raise)
                .stroke(Stroke::new(1.0_f32, pal.control))
                .rounding(Rounding::same(radius::CARD))
                .inner_margin(Margin::same(space::MD))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(t("COMECE POR AQUI"))
                            .font(theme::display_font(text::CAPTION))
                            .color(pal.muted),
                    );
                    let steps = [
                        "Clique numa tecla do aparelho",
                        "Escolha uma ação na biblioteca à esquerda, ou arraste-a até a tecla",
                        "Ajuste texto, cor e ícone no painel à direita",
                    ];
                    for (n, step) in steps.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{}", n + 1)).monospace().color(ink));
                            ui.label(t(step));
                        });
                    }
                });
        });
    });
}

impl App {
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
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) -> DeviceEvents {
        let page_blank = g.cfg.page().buttons.iter().all(is_blank);
        let banner = if g.connected {
            0.0
        } else {
            OFFLINE_BANNER_HEIGHT
        };
        let below = space::MD
            + if page_blank {
                GUIDE_HEIGHT
            } else {
                STAGE_FOOTER_HEIGHT
            };
        let above = banner + PAGE_TABS_HEIGHT + space::SM;
        let (rows, cols) = (g.cfg.page().rows, g.cfg.page().cols);
        let key = fit_key_size(
            ui.available_width(),
            ui.available_height() - above - below,
            rows,
            cols,
        );
        let device = device_size(key, rows, cols);
        // Centraliza na vertical o bloco inteiro (aviso, páginas, aparelho e o que vem abaixo).
        ui.add_space(((ui.available_height() - above - device.y - below) / 2.0).max(0.0));
        if !g.connected {
            self.offline_banner(ui, pal, g.service_running, device.x);
        }
        // As páginas pertencem ao aparelho: ficam coladas a ele, alinhadas à sua borda esquerda.
        ui.allocate_ui(vec2(device.x, PAGE_TABS_HEIGHT), |ui| {
            ui.horizontal(|ui| self.page_tabs(ui, g, pal));
        });
        ui.add_space(space::SM);
        let (rect, _) = ui.allocate_exact_size(device, Sense::hover());
        paint_device_body(ui.painter(), rect, pal, accent, g.connected);
        let events = self.draw_keys(ui, ctx, g, rect, key, pal, accent);
        ui.add_space(space::MD);
        if page_blank {
            empty_guide(ui, pal, accent, rect.width());
        } else {
            ui.allocate_ui(vec2(rect.width(), STAGE_FOOTER_HEIGHT), |ui| {
                ui.horizontal(|ui| shortcut_hints(ui, pal));
            });
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
    fn device_size_matches_the_grid() {
        let s = device_size(100.0, 3, 5);
        assert_eq!(s.x, 2.0 * DEVICE_PAD + 500.0 + 4.0 * DEVICE_GAP);
        assert_eq!(s.y, 2.0 * DEVICE_PAD + 300.0 + 2.0 * DEVICE_GAP);
    }
}
