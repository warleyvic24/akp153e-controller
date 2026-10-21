//! Peças de desenho da interface: ícones, teclas do aparelho, pílulas, abas e rótulos de seção.
use crate::theme::Palette;
use crate::tokens::{radius, space, text, TARGET_MIN, TEXT_MIN_CONTRAST};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Response, RichText, Rounding,
    Stroke, Ui,
};

const KEY_RADIUS: f32 = radius::KEY;
const PILL_ACTIVE_TINT: f32 = 0.22;
const FOCUS_RING_GAP: f32 = 2.0;
const FOCUS_RING_RADIUS: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OUTER: Color32 = Color32::from_rgb(20, 24, 36);
const KEY_NUMBER_ALPHA: u8 = 130; // 4,5:1 sobre o preto da tecla
const SHINE_ALPHA: u8 = 5; // pré-multiplicado: soma poucos níveis de cor (from_white_alpha clareia demais)
const MIN_LABEL_WIDTH: f32 = 60.0;
const HUD_CORNER: f32 = 8.0;
const EMPTY_HINT_MIN_WIDTH: f32 = 96.0; // a dica em 12 px não cabe em teclas menores
const CHIP_FILL: f32 = 0.14;
const CHIP_STROKE: f32 = 0.45;
const SWATCH_HOVER_GROW: f32 = 1.5;
const SWATCH_RING_GAP: f32 = 3.0;
const GLOW_LAYERS: [(f32, f32, f32); 3] = [(7.0, 5.0, 0.10), (4.5, 3.5, 0.22), (2.0, 2.0, 0.40)];

/// O que uma tecla do aparelho mostra (fundo, ícone vetorial e rótulo).
pub struct KeyView<'a> {
    pub number: usize,
    pub bg: Color32,
    pub icon: Option<&'a str>,
    pub label: &'a str,
    pub text: Color32,
    pub off: bool,
    pub tex: Option<egui::TextureId>,
}

pub fn paint_icon(p: &Painter, center: Pos2, size: f32, name: &str, color: Color32) {
    let scale = size / 24.0;
    let origin = center - vec2(size / 2.0, size / 2.0);
    let stroke = Stroke::new((scale * 1.9).max(1.2), color);
    for line in crate::icons::strokes(name) {
        let points: Vec<Pos2> = line
            .iter()
            .map(|(x, y)| origin + vec2(x * scale, y * scale))
            .collect();
        p.add(egui::Shape::line(points, stroke));
    }
}

/// Halo neon: contornos translúcidos sobrepostos (o egui não tem blur).
pub fn glow(p: &Painter, rect: Rect, round: f32, color: Color32) {
    for (grow, width, strength) in GLOW_LAYERS {
        p.rect_stroke(
            rect.expand(grow),
            Rounding::same(round + grow),
            Stroke::new(width, color.gamma_multiply(strength)),
        );
    }
}

/// Cantos em "L" no estilo HUD.
fn hud_corners(p: &Painter, rect: Rect, color: Color32) {
    let (r, l) = (rect.shrink(5.0), HUD_CORNER);
    let stroke = Stroke::new(1.2_f32, color);
    for (corner, dx, dy) in [
        (r.left_top(), 1.0, 1.0),
        (r.right_top(), -1.0, 1.0),
        (r.left_bottom(), 1.0, -1.0),
        (r.right_bottom(), -1.0, -1.0),
    ] {
        p.line_segment([corner, corner + vec2(dx * l, 0.0)], stroke);
        p.line_segment([corner, corner + vec2(0.0, dy * l)], stroke);
    }
}

fn lighten(c: Color32, amount: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(amount),
        c.g().saturating_add(amount),
        c.b().saturating_add(amount),
    )
}

/// Desenha uma tecla do aparelho (tela acesa com brilho no alto, ou vazia).
pub fn paint_key(
    p: &Painter,
    rect: Rect,
    k: &KeyView,
    hovered: bool,
    selected: bool,
    pal: &Palette,
    accent: Color32,
) {
    let round = Rounding::same(KEY_RADIUS);
    if k.off {
        p.rect_filled(rect, round, pal.keyoff);
        p.rect_stroke(
            rect,
            round,
            Stroke::new(1.0_f32, Color32::from_white_alpha(14)),
        );
        hud_corners(
            p,
            rect,
            accent.gamma_multiply(if hovered { 0.9 } else { 0.28 }),
        );
        // O "+" é a única pista de que a tecla aceita uma ação: precisa de contraste, não de brilho.
        let plus = if hovered || selected {
            accent
        } else {
            pal.device_fg
        };
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "+",
            FontId::monospace(22.0),
            plus,
        );
        if selected && rect.width() >= EMPTY_HINT_MIN_WIDTH {
            p.text(
                rect.center() + vec2(0.0, 20.0),
                Align2::CENTER_CENTER,
                "Arraste uma ação",
                FontId::proportional(text::CAPTION),
                pal.device_fg,
            );
        }
    } else if let Some(tex) = k.tex {
        let mut shape = egui::epaint::RectShape::filled(rect, round, Color32::WHITE);
        shape.fill_texture_id = tex;
        shape.uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        p.add(shape);
        if hovered {
            p.rect_stroke(
                rect,
                round,
                Stroke::new(1.5_f32, accent.gamma_multiply(0.7)),
            );
        }
    } else {
        p.rect_filled(rect, round, if hovered { lighten(k.bg, 16) } else { k.bg });
        let top = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
        p.rect_filled(
            top,
            Rounding {
                nw: KEY_RADIUS,
                ne: KEY_RADIUS,
                sw: 0.0,
                se: 0.0,
            },
            Color32::from_rgba_premultiplied(SHINE_ALPHA, SHINE_ALPHA, SHINE_ALPHA, SHINE_ALPHA),
        );
    }
    if rect.width() >= 56.0 {
        p.text(
            rect.min + vec2(7.0, 5.0),
            Align2::LEFT_TOP,
            k.number,
            FontId::monospace(text::CAPTION),
            Color32::from_white_alpha(KEY_NUMBER_ALPHA),
        );
    }
    if !k.off && k.tex.is_none() {
        paint_key_content(p, rect, k);
    }
    if selected {
        glow(p, rect, KEY_RADIUS, accent);
        p.rect_stroke(rect, round, Stroke::new(2.5_f32, accent));
    }
}

fn paint_key_content(p: &Painter, rect: Rect, k: &KeyView) {
    let (cx, has_label) = (rect.center().x, !k.label.is_empty());
    let icon = k.icon.filter(|n| !crate::icons::strokes(n).is_empty());
    if let Some(name) = icon {
        let size = rect.width() * 0.34;
        let cy = if has_label {
            rect.center().y - size * 0.18
        } else {
            rect.center().y
        };
        paint_icon(p, pos2(cx, cy), size, name, k.text);
    }
    if has_label && rect.width() >= MIN_LABEL_WIDTH {
        let font = FontId::proportional((rect.width() * 0.13).clamp(10.0, 13.0));
        let (pos, anchor) = if icon.is_some() {
            (pos2(cx, rect.max.y - 8.0), Align2::CENTER_BOTTOM)
        } else {
            (rect.center(), Align2::CENTER_CENTER)
        };
        p.with_clip_rect(rect)
            .text(pos, anchor, k.label, font, k.text);
    }
}

/// Opção selecionável arredondada. A ativa leva contorno e véu na cor de destaque, e o texto
/// continua na cor normal: o preenchimento sólido é reservado à ação primária da tela ("Testar").
/// Expõe o estado ativo aos leitores de tela (`selected`), que cor sozinha não transmite.
pub fn pill(ui: &mut Ui, text: &str, active: bool, accent: Color32, pal: &Palette) -> Response {
    let ink = crate::theme::ink(accent, pal);
    let (fg, fill, stroke) = if active {
        (
            pal.fg,
            ink.gamma_multiply(PILL_ACTIVE_TINT),
            Stroke::new(1.5_f32, ink),
        )
    } else {
        (
            pal.muted,
            Color32::TRANSPARENT,
            Stroke::new(1.0_f32, pal.control),
        )
    };
    let r = ui.add(
        egui::Button::new(RichText::new(text).size(text::BODY - 1.0).color(fg))
            .fill(fill)
            .stroke(stroke)
            .selected(active)
            .rounding(Rounding::same(radius::PILL))
            .min_size(vec2(0.0, TARGET_MIN)),
    );
    r.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, text)
    });
    r
}

/// Aba com sublinhado na cor de destaque (2 px, 3:1 no mínimo sobre o fundo).
pub fn tab(ui: &mut Ui, text: &str, on: bool, accent: Color32, pal: &Palette) -> Response {
    let color = if on { pal.fg } else { pal.muted };
    let r = ui.add(egui::Button::new(RichText::new(text).color(color)).frame(false));
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, text));
    if on {
        let y = r.rect.bottom() + 1.0;
        let span = [pos2(r.rect.left() + 6.0, y), pos2(r.rect.right() - 6.0, y)];
        ui.painter()
            .line_segment(span, Stroke::new(2.0_f32, crate::theme::ink(accent, pal)));
    }
    r
}

/// Rótulo de seção em caixa alta. Sem espaçamento artificial entre letras: o espaço fino que
/// simulava isso era lido letra por letra por leitores de tela e quebrava busca e cópia.
/// Devolve o id do rótulo, para o campo que ele nomeia usar `labelled_by` (leitores de tela).
pub fn section(ui: &mut Ui, text: &str, pal: &Palette) -> egui::Id {
    ui.add_space(space::XS + 2.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .font(crate::theme::display_font(text::CAPTION))
            .color(pal.muted),
    )
    .id
}

/// Etiqueta de estado com ponto e fundo na própria cor (conectado, salvo...). O texto nunca fica
/// abaixo de 4,5:1 sobre o fundo tingido; o estado vai por escrito, não só pela cor.
pub fn status_chip(ui: &mut Ui, label: &str, color: Color32) -> Response {
    let fill = crate::theme::mix(ui.visuals().panel_fill, color, CHIP_FILL);
    let ink = crate::theme::ensure_contrast(color, fill, TEXT_MIN_CONTRAST);
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(CHIP_STROKE)))
        .rounding(Rounding::same(radius::PILL))
        .inner_margin(egui::Margin::symmetric(space::MD - 2.0, 3.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("● {label}"))
                    .monospace()
                    .size(text::CAPTION)
                    .color(ink),
            )
        })
        .response
}

/// Amostra de cor clicável; a escolhida ganha um anel afastado, e o mouse em cima a destaca.
pub fn swatch(
    ui: &mut Ui,
    size: f32,
    radius: f32,
    fill: Color32,
    selected: bool,
    ring: Color32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let body = if resp.hovered() {
        rect.expand(SWATCH_HOVER_GROW)
    } else {
        rect
    };
    let p = ui.painter();
    p.rect_filled(body, Rounding::same(radius), fill);
    p.rect_stroke(
        body,
        Rounding::same(radius),
        Stroke::new(1.0_f32, Color32::from_white_alpha(28)),
    );
    if selected {
        p.rect_stroke(
            rect.expand(SWATCH_RING_GAP),
            Rounding::same(radius + SWATCH_RING_GAP),
            Stroke::new(2.0_f32, ring),
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Anel de foco do teclado em dois tons (branco por dentro, escuro por fora): aparece sobre qualquer
/// fundo, inclusive as teclas, que são escuras nos dois temas (um anel só na cor do texto sumia no
/// tema claro). Fica distinto de hover (tinta) e de seleção (brilho de destaque).
pub fn focus_ring(ctx: &egui::Context) {
    let Some(rect) = ctx
        .memory(|m| m.focused())
        .and_then(|id| ctx.read_response(id))
        .map(|r| r.rect)
    else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("focus_ring"),
    ));
    let inner = rect.expand(FOCUS_RING_GAP);
    painter.rect_stroke(
        inner.expand(FOCUS_RING_WIDTH),
        Rounding::same(FOCUS_RING_RADIUS + FOCUS_RING_WIDTH),
        Stroke::new(FOCUS_RING_WIDTH, FOCUS_RING_OUTER),
    );
    painter.rect_stroke(
        inner,
        Rounding::same(FOCUS_RING_RADIUS),
        Stroke::new(FOCUS_RING_WIDTH, Color32::WHITE),
    );
}

/// Nome falado de um controle que não tem rótulo visível ao lado (campo de busca, seletor).
pub fn name(resp: &Response, label: &str) {
    resp.ctx
        .accesskit_node_builder(resp.id, |node| node.set_name(label));
}

/// Placeholder de campo. O tema força a cor do texto (`override_text_color`), o que deixava a dica
/// idêntica a um valor digitado; com cor própria (`muted`, 4,5:1) ela se distingue sem sumir.
pub fn hint(text: &str, pal: &Palette) -> RichText {
    RichText::new(text).color(pal.muted)
}

/// Texto de tecla ao estilo "kbd".
pub fn kbd(ui: &mut Ui, label: &str, pal: &Palette) {
    egui::Frame::none()
        .fill(pal.raise)
        .stroke(Stroke::new(1.0_f32, pal.line))
        .rounding(Rounding::same(5.0))
        .inner_margin(egui::Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label.to_uppercase())
                    .monospace()
                    .size(text::CAPTION),
            )
        });
}
