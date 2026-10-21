//! Bandeja de ações e palco central (o aparelho desenhado com as teclas).
use crate::config::schema::ButtonConfig;
use crate::theme::{self, Palette};
use crate::tokens::{radius, text};
use crate::{key_view, runtime, widgets, App, TAB_GAP};
use eframe::egui::{
    self, pos2, vec2, Align, Color32, Frame, Layout, Margin, Rect, RichText, Rounding, Sense,
    Stroke,
};

const TRAY_CARD: egui::Vec2 = egui::Vec2::new(104.0, 64.0);
const STAGE_FOOTER_HEIGHT: f32 = 30.0;
const OFFLINE_BANNER_HEIGHT: f32 = 60.0;
const KEY_MIN: f32 = 48.0;
const KEY_MAX: f32 = 110.0;
const DEVICE_MAX_WIDTH: f32 = 640.0;
const DEVICE_PAD: f32 = 24.0;
const DEVICE_GAP: f32 = 12.0;
/// Luz ambiente atrás do aparelho: camadas finas e sobrepostas (sem degraus visíveis), que não
/// passam da margem do painel (16 px) para não serem cortadas.
const AMBIENT_STEPS: u8 = 8;
const AMBIENT_STEP_GROW: f32 = 2.0;
const AMBIENT_STEP_ALPHA: f32 = 0.014;
const TRAY_HOVER_TINT: f32 = 0.10;

/// Nome que o leitor de tela anuncia: posição, o que a tecla mostra e se está selecionada.
fn key_widget_info(index: usize, b: &ButtonConfig, selected: bool) -> egui::WidgetInfo {
    let shown = &b.current_appearance().text;
    let what = if shown.is_empty() { "vazia" } else { shown };
    egui::WidgetInfo::selected(
        egui::WidgetType::Button,
        true,
        selected,
        format!("Tecla {}, {what}", index + 1),
    )
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
        let mut cats: Vec<&'static str> = Vec::new();
        for p in &self.presets {
            if !cats.contains(&p.category) {
                cats.push(p.category);
            }
        }
        let mut picked = None;
        let frame = Frame::none()
            .fill(pal.bg)
            .inner_margin(Margin::symmetric(18.0, 10.0));
        egui::TopBottomPanel::bottom("tray")
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, c) in cats.iter().enumerate() {
                        let on = i == self.tray_cat && self.query.is_empty();
                        if widgets::tab(ui, &c.to_uppercase(), on, accent, pal).clicked() {
                            self.tray_cat = i;
                            self.query.clear();
                        }
                        ui.add_space(TAB_GAP);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.query)
                                .hint_text(widgets::hint("Buscar ação", pal))
                                .desired_width(200.0),
                        );
                    });
                });
                let q = self.query.to_lowercase();
                let shown: Vec<usize> = self
                    .presets
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        if q.is_empty() {
                            Some(&p.category) == cats.get(self.tray_cat)
                        } else {
                            p.label.to_lowercase().contains(&q)
                        }
                    })
                    .map(|(i, _)| i)
                    .collect();
                if shown.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "Nenhuma ação encontrada para “{}”. Limpe a busca para ver todas.",
                            self.query
                        ))
                        .color(pal.muted),
                    );
                    return;
                }
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for i in shown {
                            let p = &self.presets[i];
                            let (rect, resp) =
                                ui.allocate_exact_size(TRAY_CARD, Sense::click_and_drag());
                            resp.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, p.label)
                            });
                            let ink = theme::ink(accent, pal);
                            let border = if resp.hovered() { ink } else { pal.control };
                            let painter = ui.painter();
                            let fill = if resp.hovered() {
                                theme::mix(pal.raise, accent, TRAY_HOVER_TINT)
                            } else {
                                pal.raise
                            };
                            painter.rect_filled(rect, Rounding::same(radius::CARD), fill);
                            painter.rect_stroke(
                                rect,
                                Rounding::same(radius::CARD),
                                Stroke::new(1.0_f32, border),
                            );
                            widgets::paint_icon(
                                painter,
                                pos2(rect.center().x, rect.top() + 24.0),
                                24.0,
                                p.icon,
                                ink,
                            );
                            painter.text(
                                pos2(rect.center().x, rect.bottom() - 11.0),
                                egui::Align2::CENTER_CENTER,
                                p.label,
                                egui::FontId::proportional(text::BODY - 1.0),
                                pal.fg,
                            );
                            if resp.clicked() {
                                picked = Some(i);
                            }
                            if resp.drag_started() {
                                egui::DragAndDrop::set_payload(ctx, i);
                            }
                            resp.on_hover_cursor(egui::CursorIcon::Grab);
                        }
                    });
                });
            });
        if let Some(i) = picked {
            self.apply_preset(g, self.selected, i);
        }
    }

    /// Centro: o aparelho desenhado com as teclas, e as ações rápidas do botão selecionado.
    pub(crate) fn stage(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let frame = Frame::none().fill(pal.bg).inner_margin(Margin::same(16.0));
        let mut dropped: Option<(usize, usize)> = None;
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        let reserved = STAGE_FOOTER_HEIGHT
                            + if g.connected {
                                0.0
                            } else {
                                OFFLINE_BANNER_HEIGHT
                            };
                        let avail_h = ui.available_height() - reserved;
                        if !g.connected {
                            self.offline_banner(ui, pal);
                        }
                        let (rows, cols) = (g.cfg.page().rows, g.cfg.page().cols);
                        let avail = ui.available_width().min(DEVICE_MAX_WIDTH);
                        let by_width =
                            (avail - 2.0 * DEVICE_PAD - (cols as f32 - 1.0) * DEVICE_GAP)
                                / cols as f32;
                        let by_height =
                            (avail_h - 2.0 * DEVICE_PAD - (rows as f32 - 1.0) * DEVICE_GAP)
                                / rows as f32;
                        let key = by_width.min(by_height).clamp(KEY_MIN, KEY_MAX);
                        let size = vec2(
                            2.0 * DEVICE_PAD + cols as f32 * key + (cols as f32 - 1.0) * DEVICE_GAP,
                            2.0 * DEVICE_PAD + rows as f32 * key + (rows as f32 - 1.0) * DEVICE_GAP,
                        );
                        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                        let painter = ui.painter().clone();
                        for step in 1..=AMBIENT_STEPS {
                            let grow = f32::from(step) * AMBIENT_STEP_GROW;
                            painter.rect_filled(
                                rect.expand(grow),
                                Rounding::same(22.0 + grow),
                                accent.gamma_multiply(AMBIENT_STEP_ALPHA),
                            );
                        }
                        painter.rect_filled(rect, Rounding::same(radius::DEVICE), pal.bezel);
                        painter.rect_filled(rect.shrink(8.0), Rounding::same(16.0), pal.device);
                        painter.rect_stroke(
                            rect.shrink(8.0),
                            Rounding::same(16.0),
                            Stroke::new(1.0_f32, Color32::from_black_alpha(160)),
                        );
                        painter.rect_stroke(
                            rect,
                            Rounding::same(radius::DEVICE),
                            Stroke::new(1.5_f32, accent.gamma_multiply(0.4)),
                        );
                        let led = if g.connected { accent } else { pal.bad };
                        let led_rect = Rect::from_center_size(
                            pos2(rect.center().x, rect.top() + 9.0),
                            vec2(34.0, 3.0),
                        );
                        painter.rect_filled(
                            led_rect.expand(3.0),
                            Rounding::same(4.0),
                            led.gamma_multiply(0.25),
                        );
                        painter.rect_filled(led_rect, Rounding::same(2.0), led);
                        for r in 0..rows {
                            for c in 0..cols {
                                let i = r * cols + c;
                                let origin = pos2(
                                    rect.left() + DEVICE_PAD + c as f32 * (key + DEVICE_GAP),
                                    rect.top() + DEVICE_PAD + r as f32 * (key + DEVICE_GAP),
                                );
                                let kr = Rect::from_min_size(origin, vec2(key, key));
                                let resp = ui.interact(kr, ui.id().with(("key", i)), Sense::click_and_drag());
                                let button = &g.cfg.page().buttons[i];
                                resp.widget_info(|| key_widget_info(i, button, i == self.selected));
                                // Tab e setas (navegação nativa do egui) movem o foco; a seleção o acompanha.
                                if resp.has_focus() {
                                    self.selected = i;
                                }
                                let tex = self.key_tex.get(ctx, i, button, g.icon_px);
                                widgets::paint_key(
                                    &painter,
                                    kr,
                                    &key_view(button, i + 1, tex),
                                    resp.hovered(),
                                    i == self.selected,
                                    pal,
                                    accent,
                                );
                                if resp.clicked() {
                                    self.selected = i;
                                    // O egui só dá foco por Tab; sem isto, as setas não funcionam após o clique.
                                    resp.request_focus();
                                }
                                if let Some(preset) = resp.dnd_release_payload::<usize>() {
                                    dropped = Some((i, *preset));
                                }
                            }
                        }
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(
                                "Clique num botão (ou use Tab e as setas) para editar · arraste uma ação da bandeja",
                            )
                            .monospace()
                            .size(text::CAPTION)
                            .color(pal.muted),
                        );
                    })
                });
        });
        if let Some((button, preset)) = dropped {
            self.selected = button;
            self.apply_preset(g, button, preset);
        }
    }
}
