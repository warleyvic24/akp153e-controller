//! Peças de desenho da interface: ícones, teclas do aparelho, pílulas, abas e rótulos de seção.
use crate::editor::theme::Palette;
use crate::editor::tokens::{radius, space, text, TARGET_MIN, TEXT_MIN_CONTRAST};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response,
    RichText, Sense, Stroke, Ui,
};

const KEY_RADIUS: f32 = radius::KEY;
const PILL_ACTIVE_TINT: f32 = 0.22;
const ICON_BUTTON: f32 = 32.0;
const ICON_GLYPH: f32 = 18.0;
const SEGMENT_RADIUS: f32 = 7.0;
const KBD_RADIUS: f32 = 5.0;
const FOCUS_RING_GAP: f32 = 2.0;
const FOCUS_RING_RADIUS: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OUTER: Color32 = Color32::from_rgb(20, 24, 36);
const KEY_NUMBER_ALPHA: u8 = 130; // 4,5:1 sobre o preto da tecla
const KEY_NUMBER_MIN_WIDTH: f32 = 56.0;
const KEY_NUMBER_OFFSET: egui::Vec2 = egui::Vec2::new(7.0, 5.0);
const KEY_NUMBER_PLATE_PAD: f32 = 3.0;
const KEY_NUMBER_PLATE_ALPHA: u8 = 190;
const KEY_SELECTED_STROKE: f32 = 2.5;
const KEY_HOVER_STROKE: f32 = 1.5;
const PROBLEM_BADGE_RADIUS: f32 = 8.0;
const PROBLEM_BADGE_INSET: f32 = 4.0;
const EMPTY_RIM_ALPHA: u8 = 14;
const EMPTY_CORNERS_HOT: f32 = 0.9;
/// Brilho do "+" de uma tecla vazia em repouso (contraste ainda acima de 3:1 sobre o preto).
pub(crate) const EMPTY_PLUS_IDLE: f32 = 0.55;
const EMPTY_HINT_OFFSET: f32 = 20.0;
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
    /// Número da tecla no aparelho; `None` na miniatura do inspetor (o título já diz qual é).
    pub number: Option<usize>,
    pub bg: Color32,
    pub icon: Option<&'a str>,
    pub label: &'a str,
    pub text: Color32,
    pub off: bool,
    pub tex: Option<egui::TextureId>,
    /// A tecla precisa de ajuste para funcionar (ver `checks::button_problem`).
    pub problem: bool,
}

pub fn paint_icon(p: &Painter, center: Pos2, size: f32, name: &str, color: Color32) {
    let scale = size / 24.0;
    let origin = center - vec2(size / 2.0, size / 2.0);
    let stroke = Stroke::new((scale * 1.9).max(1.2), color);
    for line in crate::graphics::icons::strokes(name) {
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
            CornerRadius::from(round + grow),
            Stroke::new(width, color.gamma_multiply(strength)),
            egui::StrokeKind::Inside,
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
    if k.off {
        paint_empty_key(p, rect, hovered, selected, pal, accent);
    } else if let Some(tex) = k.tex {
        paint_image_key(p, rect, tex, hovered, accent);
    } else {
        paint_plain_key(p, rect, k, hovered);
    }
    // Numa tecla configurada o número cobriria o desenho: só aparece ao apontar, sobre uma
    // plaqueta escura (o inspetor já diz o número da selecionada).
    let number = k.number.filter(|_| k.off || hovered);
    if let Some(number) = number.filter(|_| rect.width() >= KEY_NUMBER_MIN_WIDTH) {
        let font = FontId::monospace(text::CAPTION);
        let galley = p.layout_no_wrap(
            number.to_string(),
            font,
            Color32::from_white_alpha(KEY_NUMBER_ALPHA),
        );
        let at = rect.min + KEY_NUMBER_OFFSET;
        if !k.off {
            let plate = Rect::from_min_size(at, galley.size()).expand(KEY_NUMBER_PLATE_PAD);
            p.rect_filled(
                plate,
                CornerRadius::from(KEY_NUMBER_PLATE_PAD),
                Color32::from_black_alpha(KEY_NUMBER_PLATE_ALPHA),
            );
        }
        p.galley(at, galley, Color32::WHITE);
    }
    if !k.off && k.tex.is_none() {
        paint_key_content(p, rect, k);
    }
    if selected {
        glow(p, rect, KEY_RADIUS, accent);
        p.rect_stroke(
            rect,
            CornerRadius::from(KEY_RADIUS),
            Stroke::new(KEY_SELECTED_STROKE, accent),
            egui::StrokeKind::Inside,
        );
    }
    if k.problem {
        paint_problem_badge(p, rect, pal);
    }
}

/// Selo "!" no canto superior direito: a tecla não vai funcionar como está.
fn paint_problem_badge(p: &Painter, rect: Rect, pal: &Palette) {
    let offset = PROBLEM_BADGE_INSET + PROBLEM_BADGE_RADIUS;
    let center = pos2(rect.right() - offset, rect.top() + offset);
    p.circle_filled(center, PROBLEM_BADGE_RADIUS, pal.bad);
    p.text(
        center,
        Align2::CENTER_CENTER,
        "!",
        FontId::proportional(text::CAPTION),
        crate::editor::theme::readable_on(pal.bad),
    );
}

/// Tecla sem ação: fundo preto, cantos HUD e o "+" que convida a configurá-la.
fn paint_empty_key(
    p: &Painter,
    rect: Rect,
    hovered: bool,
    selected: bool,
    pal: &Palette,
    accent: Color32,
) {
    let round = CornerRadius::from(KEY_RADIUS);
    p.rect_filled(rect, round, pal.keyoff);
    let rim = Stroke::new(1.0_f32, Color32::from_white_alpha(EMPTY_RIM_ALPHA));
    p.rect_stroke(rect, round, rim, egui::StrokeKind::Inside);
    // Com 15 teclas vazias, cantos e "+" acesos em todas viram ruído: em repouso só o "+",
    // mais baixo (ainda acima de 3:1 sobre o preto); os cantos aparecem ao passar o mouse.
    if hovered || selected {
        hud_corners(p, rect, accent.gamma_multiply(EMPTY_CORNERS_HOT));
    }
    let plus = if hovered || selected {
        accent
    } else {
        pal.device_fg.gamma_multiply(EMPTY_PLUS_IDLE)
    };
    let plus_font = FontId::monospace(text::DISPLAY);
    p.text(rect.center(), Align2::CENTER_CENTER, "+", plus_font, plus);
    if selected && rect.width() >= EMPTY_HINT_MIN_WIDTH {
        p.text(
            rect.center() + vec2(0.0, EMPTY_HINT_OFFSET),
            Align2::CENTER_CENTER,
            crate::strings::t("Arraste uma ação"),
            FontId::proportional(text::CAPTION),
            pal.device_fg,
        );
    }
}

/// Tecla com a imagem exata que vai ao aparelho (renderizada em textura).
fn paint_image_key(p: &Painter, rect: Rect, tex: egui::TextureId, hovered: bool, accent: Color32) {
    let round = CornerRadius::from(KEY_RADIUS);
    let mut shape = egui::epaint::RectShape::filled(rect, round, Color32::WHITE);
    shape = shape.with_texture(tex, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
    p.add(shape);
    if hovered {
        let stroke = Stroke::new(KEY_HOVER_STROKE, accent.gamma_multiply(0.7));
        p.rect_stroke(rect, round, stroke, egui::StrokeKind::Inside);
    }
}

/// Tecla só com cor de fundo (sem textura): clareia ao passar o mouse e leva um brilho no alto.
fn paint_plain_key(p: &Painter, rect: Rect, k: &KeyView, hovered: bool) {
    let fill = if hovered { lighten(k.bg, 16) } else { k.bg };
    p.rect_filled(rect, CornerRadius::from(KEY_RADIUS), fill);
    let top = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
    let r = KEY_RADIUS as u8;
    let shine = CornerRadius {
        nw: r,
        ne: r,
        sw: 0,
        se: 0,
    };
    let a = SHINE_ALPHA;
    p.rect_filled(top, shine, Color32::from_rgba_premultiplied(a, a, a, a));
}

fn paint_key_content(p: &Painter, rect: Rect, k: &KeyView) {
    let (cx, has_label) = (rect.center().x, !k.label.is_empty());
    let icon = k
        .icon
        .filter(|n| !crate::graphics::icons::strokes(n).is_empty());
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
    let text = crate::strings::t(text);
    let ink = crate::editor::theme::ink(accent, pal);
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
            .corner_radius(CornerRadius::from(radius::PILL))
            .min_size(vec2(0.0, TARGET_MIN)),
    );
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, text));
    r
}

/// Botão quadrado só com ícone (desfazer, copiar...). `name` é o texto falado e a dica.
pub fn icon_button(ui: &mut Ui, icon: &str, name: &str, enabled: bool, pal: &Palette) -> Response {
    icon_button_tinted(ui, icon, name, enabled, pal.fg, pal)
}

/// Como `icon_button`, com a cor do ícone escolhida (ex.: vermelho para excluir).
pub fn icon_button_tinted(
    ui: &mut Ui,
    icon: &str,
    name: &str,
    enabled: bool,
    ink: Color32,
    pal: &Palette,
) -> Response {
    let name = crate::strings::t(name);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(ICON_BUTTON, ICON_BUTTON), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name));
    let hot = enabled && (resp.hovered() || resp.has_focus());
    if hot {
        ui.painter()
            .rect_filled(rect, CornerRadius::from(radius::CONTROL), pal.raise);
    }
    let color = if enabled {
        ink
    } else {
        pal.muted.gamma_multiply(0.6)
    };
    paint_icon(ui.painter(), rect.center(), ICON_GLYPH, icon, color);
    resp.on_hover_text(name)
}

/// Item de controle segmentado (abas do inspetor, páginas): o ativo ganha uma superfície.
/// `width` 0 = só o necessário para o texto.
pub fn segment(ui: &mut Ui, text: &str, on: bool, width: f32, pal: &Palette) -> Response {
    let text = crate::strings::t(text);
    let label = RichText::new(text).size(text::BODY - 1.0);
    let label = if on {
        label.color(pal.fg).strong()
    } else {
        label.color(pal.muted)
    };
    let fill = if on { pal.raise } else { Color32::TRANSPARENT };
    let r = ui.add(
        egui::Button::new(label)
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::from(SEGMENT_RADIUS))
            .min_size(vec2(width, TARGET_MIN + 6.0)),
    );
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, text));
    r
}

/// Moldura dos segmentos: um trilho escuro com cantos arredondados.
pub fn segmented<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(pal.field)
        .corner_radius(CornerRadius::from(SEGMENT_RADIUS + 3.0))
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(add).inner
        })
        .inner
}

/// Rótulo de seção em caixa alta. Sem espaçamento artificial entre letras: o espaço fino que
/// simulava isso era lido letra por letra por leitores de tela e quebrava busca e cópia.
/// Devolve o id do rótulo, para o campo que ele nomeia usar `labelled_by` (leitores de tela).
pub fn section(ui: &mut Ui, text: &str, pal: &Palette) -> egui::Id {
    let text = crate::strings::t(text);
    ui.add_space(space::XS + 2.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .font(crate::editor::theme::display_font(text::CAPTION))
            .color(pal.muted),
    )
    .id
}

/// Etiqueta de estado com ponto e fundo na própria cor (conectado, salvo...). O texto nunca fica
/// abaixo de 4,5:1 sobre o fundo tingido; o estado vai por escrito, não só pela cor.
pub fn status_chip(ui: &mut Ui, label: &str, color: Color32) -> Response {
    let label = crate::strings::t(label);
    let fill = crate::editor::theme::mix(ui.visuals().panel_fill, color, CHIP_FILL);
    let ink = crate::editor::theme::ensure_contrast(color, fill, TEXT_MIN_CONTRAST);
    egui::Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(CHIP_STROKE)))
        .corner_radius(CornerRadius::from(radius::PILL))
        .inner_margin(egui::Margin::symmetric((space::MD - 2.0) as i8, 3))
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
    name: &str,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, name));
    let body = if resp.hovered() {
        rect.expand(SWATCH_HOVER_GROW)
    } else {
        rect
    };
    let p = ui.painter();
    p.rect_filled(body, CornerRadius::from(radius), fill);
    p.rect_stroke(
        body,
        CornerRadius::from(radius),
        Stroke::new(1.0_f32, Color32::from_white_alpha(28)),
        egui::StrokeKind::Inside,
    );
    if selected {
        p.rect_stroke(
            rect.expand(SWATCH_RING_GAP),
            CornerRadius::from(radius + SWATCH_RING_GAP),
            Stroke::new(2.0_f32, ring),
            egui::StrokeKind::Inside,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Anel de foco do teclado em dois tons (branco por dentro, escuro por fora): aparece sobre qualquer
/// fundo, inclusive as teclas, que são escuras nos dois temas (um anel só na cor do texto sumia no
/// tema claro). Fica distinto de hover (tinta) e de seleção (brilho de destaque).
pub fn focus_ring(ctx: &egui::Context) {
    let Some(rect) = ctx
        .memory(eframe::egui::Memory::focused)
        .and_then(|id| ctx.read_response(id))
        .map(|r| r.rect)
    else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("focus_ring"),
    ));
    let inner = rect.expand(FOCUS_RING_GAP);
    painter.rect_stroke(
        inner.expand(FOCUS_RING_WIDTH),
        CornerRadius::from(FOCUS_RING_RADIUS + FOCUS_RING_WIDTH),
        Stroke::new(FOCUS_RING_WIDTH, FOCUS_RING_OUTER),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        inner,
        CornerRadius::from(FOCUS_RING_RADIUS),
        Stroke::new(FOCUS_RING_WIDTH, Color32::WHITE),
        egui::StrokeKind::Inside,
    );
}

/// Nome falado de um controle que não tem rótulo visível ao lado (campo de busca, seletor).
pub fn name(resp: &Response, label: &str) {
    resp.ctx
        .accesskit_node_builder(resp.id, |node| node.set_label(crate::strings::t(label)));
}

/// Placeholder de campo. O tema força a cor do texto (`override_text_color`), o que deixava a dica
/// idêntica a um valor digitado; com cor própria (`muted`, 4,5:1) ela se distingue sem sumir.
/// Campo de busca: lupa à esquerda, dica apagada e o nome falado igual à dica.
pub fn search_field(ui: &mut Ui, query: &mut String, hint_text: &str, pal: &Palette) -> Response {
    const ICON: f32 = 14.0;
    const LEFT_PAD: i8 = 26;
    let resp = ui.add(
        egui::TextEdit::singleline(query)
            .hint_text(hint(hint_text, pal))
            .margin(egui::Margin {
                left: LEFT_PAD,
                right: 6,
                top: 4,
                bottom: 4,
            })
            .desired_width(ui.available_width()),
    );
    // No egui 0.37 o retângulo da resposta é o campo inteiro, com a margem: a lupa vai no meio dela.
    let center = pos2(
        resp.rect.left() + f32::from(LEFT_PAD) / 2.0 + 1.0,
        resp.rect.center().y,
    );
    paint_icon(ui.painter(), center, ICON, "search", pal.muted);
    name(&resp, hint_text);
    resp
}

pub fn hint(text: &str, pal: &Palette) -> RichText {
    RichText::new(crate::strings::t(text)).color(pal.muted)
}

/// Texto de tecla ao estilo "kbd".
pub fn kbd(ui: &mut Ui, label: &str, pal: &Palette) {
    egui::Frame::NONE
        .fill(pal.raise)
        .stroke(Stroke::new(1.0_f32, pal.line))
        .corner_radius(CornerRadius::from(KBD_RADIUS))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label.to_uppercase())
                    .monospace()
                    .size(text::CAPTION),
            )
        });
}
