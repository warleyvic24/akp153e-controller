//! Peças de desenho da interface: ícones, teclas do aparelho, pílulas, abas e rótulos de seção.
use crate::editor::theme::Palette;
use crate::editor::tokens::{
    elevation, motion, radius, space, text, TARGET_MIN, TEXT_MIN_CONTRAST,
};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response,
    RichText, Sense, Stroke, Ui,
};

const KEY_RADIUS: f32 = radius::KEY;
const PILL_ACTIVE_TINT: f32 = 0.22;
const ICON_BUTTON: f32 = 32.0;
const ICON_GLYPH: f32 = 17.0;
/// Quanto o ícone em repouso puxa do texto apagado rumo ao texto (0 = apagado, 1 = texto).
const ICON_IDLE_INK: f32 = 0.55;
/// Superfície de hover/pressionado: um passo da superfície elevada rumo ao texto.
const HOVER_TINT: f32 = 0.08;
const PRESSED_TINT: f32 = 0.14;
const DISABLED_ALPHA: f32 = 0.6;
const BUTTON_HEIGHT: f32 = 32.0;
const BUTTON_PAD_X: f32 = 14.0;
const BUTTON_ICON: f32 = 16.0;
const BUTTON_SHEEN_ALPHA: u8 = 40;
const DANGER_HOVER_ALPHA: f32 = 0.14;
const BRAND_MARK: f32 = 26.0;
const BRAND_GLYPH: f32 = 16.0;
const BRAND_SHEEN_ALPHA: u8 = 22;
const CHIP_DOT: f32 = 3.0;
/// Véu de hover sobre controles cujo fundo é fixo (pílula, segmento): tinta do texto, bem leve.
const HOVER_VEIL: f32 = 0.07;
/// O botão afunda este tanto (px) enquanto pressionado: retorno tátil sem mudar o layout.
const PRESS_INSET: f32 = 0.75;
/// Halo de foco dos campos: anel na cor de destaque por dentro e uma auréola difusa por fora.
const FIELD_FOCUS_STROKE: f32 = 1.5;
const FIELD_HALO_WIDTH: f32 = 3.0;
const FIELD_HALO_ALPHA: f32 = 0.28;
/// A caixa marcada cresce de 70% a 100% ao marcar (com o preenchimento surgindo junto).
const CHECK_POP_FROM: f32 = 0.7;
const MENU_ROW: f32 = 30.0;
const SEGMENT_ON_TINT: f32 = 0.07;
const FOLD_CHEVRON: f32 = 15.0;
const CHECK_BOX: f32 = 18.0;
const CHECK_RADIUS: f32 = 5.0;
const CHECK_GLYPH: f32 = 14.0;
const MENU_ICON: f32 = 15.0;
const MENU_PAD_X: f32 = 8.0;
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
    /// Se deve mostrar o rótulo de texto na tecla.
    pub show_text: bool,
}

/// Nome Lucide de cada ícone de interface que o editor pede pelo nome curto antigo.
pub const UI_ICONS: [(&str, &str); 32] = [
    ("play", "play"),
    ("next", "skip-forward"),
    ("prev", "skip-back"),
    ("volup", "volume-2"),
    ("voldown", "volume-1"),
    ("mute", "volume-x"),
    ("copy", "copy"),
    ("paste", "clipboard-paste"),
    ("calc", "calculator"),
    ("page", "file-text"),
    ("globe", "globe"),
    ("bolt", "zap"),
    ("lock", "lock"),
    ("folder", "folder"),
    ("app", "app-window"),
    ("user", "user"),
    ("undo", "undo-2"),
    ("redo", "redo-2"),
    ("settings", "settings"),
    ("trash", "trash"),
    ("search", "search"),
    ("grip", "grip-vertical"),
    ("more", "ellipsis"),
    ("close", "x"),
    ("plus", "plus"),
    ("keyboard", "keyboard"),
    ("ban", "ban"),
    ("music", "disc-3"),
    ("mixer", "sliders-horizontal"),
    ("chevron", "chevron-down"),
    ("image", "image"),
    ("keyforge", "keyforge"),
];
/// Maior textura de ícone de interface (px físicos); acima disso o ícone é decoração de palco.
const UI_ICON_MAX_PX: f32 = 128.0;

/// Id Lucide (`lucide:nome`) de um ícone de interface: o nome curto antigo, um id já completo
/// ou um nome Lucide solto.
pub(crate) fn lucide_id(name: &str) -> String {
    use crate::graphics::icon_library::PREFIX;
    if name.starts_with(PREFIX) {
        return name.to_string();
    }
    let lucide = UI_ICONS
        .iter()
        .find(|(short, _)| *short == name)
        .map_or(name, |(_, l)| *l);
    format!("{PREFIX}{lucide}")
}

/// Texturas brancas dos ícones de interface, por id e tamanho em pixels físicos. A cor vem do
/// tingimento na hora de desenhar, então trocar tema ou destaque não cria texturas novas.
type UiIconCache =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(String, u32), egui::TextureHandle>>>;

fn ui_icon_texture(ctx: &egui::Context, id: &str, px: u32) -> Option<egui::TextureId> {
    let cache: UiIconCache = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<UiIconCache>(egui::Id::unique("ui-icon-textures"))
            .clone()
    });
    let mut cache = crate::sync::lock(&cache);
    if let Some(tex) = cache.get(&(id.to_string(), px)) {
        return Some(tex.id());
    }
    let white = crate::config::schema::RGBAColor {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    let img = crate::graphics::icon_library::render(id, px, white)?;
    let image = egui::ColorImage::from_rgba_unmultiplied([px as usize; 2], img.as_raw());
    let handle = ctx.load_texture(
        format!("ui-icon:{id}:{px}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    let tex = handle.id();
    cache.insert((id.to_string(), px), handle);
    Some(tex)
}

/// Ícone de interface (barra, botões, menus): o Lucide de verdade, renderizado pelo `resvg` no
/// tamanho exato da tela (nítido em qualquer escala) e tingido com `color`. Se o nome não existir
/// na biblioteca, cai no desenho a traço embutido.
pub fn ui_icon(p: &Painter, center: Pos2, size: f32, name: &str, color: Color32) {
    let ppp = p.pixels_per_point();
    let px = (size * ppp).round().clamp(8.0, UI_ICON_MAX_PX);
    match ui_icon_texture(p.ctx(), &lucide_id(name), px as u32) {
        Some(tex) => {
            // Alinha à grade de pixels físicos: sem isso o traço de 2 px borra em meio pixel.
            let side = px / ppp;
            let min = center - vec2(side, side) / 2.0;
            let min = pos2((min.x * ppp).round() / ppp, (min.y * ppp).round() / ppp);
            let rect = Rect::from_min_size(min, vec2(side, side));
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            p.image(tex, rect, uv, color);
        }
        None => paint_icon(p, center, size, name, color),
    }
}

/// Ícone de interface como elemento do layout (ao lado de um texto, num título).
pub fn icon(ui: &mut Ui, name: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui_icon(ui.painter(), rect.center(), size, name, color);
    resp
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

/// Progresso (0 a 1) de um estado do controle, animado: hover, foco, marcado. O egui pede os
/// quadros extras sozinho enquanto a transição corre. Com "Reduzir movimento" (`theme::apply`
/// zera `animation_time`) o estado muda na hora.
fn ease(ctx: &egui::Context, id: egui::Id, on: bool, secs: f32) -> f32 {
    let still = ctx.global_style().animation_time <= 0.0;
    ctx.animate_bool_with_time(id, on, if still { 0.0 } else { secs })
}

/// Mistura que preserva a transparência (o `theme::mix` devolve sempre opaco): serve para surgir
/// um fundo a partir do nada.
fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

/// Véu de hover para controles com fundo fixo (o `egui::Button` com `fill` não muda ao apontar):
/// surge e some com transição, sem cobrir o texto.
fn hover_veil(ui: &Ui, resp: &Response, round: f32, pal: &Palette) {
    let t = ease(ui.ctx(), resp.id.with("veil"), resp.hovered(), motion::FAST);
    if t > 0.0 {
        ui.painter().rect_filled(
            resp.rect,
            CornerRadius::from(round),
            pal.fg.gamma_multiply(HOVER_VEIL * t),
        );
    }
}

/// Foco de campo de texto: anel na cor de destaque com auréola, no lugar do filete branco/preto
/// que o egui usa (cor de `selection.stroke`). Registra o campo para o anel de teclado não
/// desenhar um segundo contorno em volta dele.
fn field_focus(ui: &Ui, resp: &Response, pal: &Palette) {
    let focused = resp.has_focus();
    if focused {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::unique(TEXT_FOCUS_KEY), Some(resp.id)));
    }
    let t = ease(ui.ctx(), resp.id.with("focus"), focused, motion::FAST);
    if t <= 0.0 {
        return;
    }
    let ink = crate::editor::theme::ink(ui.visuals().selection.bg_fill, pal);
    let round = ui.visuals().widgets.inactive.corner_radius;
    let p = ui.painter();
    p.rect_stroke(
        resp.rect.expand(FIELD_HALO_WIDTH / 2.0),
        CornerRadius::from(radius::CONTROL + FIELD_HALO_WIDTH / 2.0),
        Stroke::new(FIELD_HALO_WIDTH, ink.gamma_multiply(FIELD_HALO_ALPHA * t)),
        egui::StrokeKind::Middle,
    );
    p.rect_stroke(
        resp.rect,
        round,
        Stroke::new(FIELD_FOCUS_STROKE, blend(pal.control, ink, t)),
        egui::StrokeKind::Inside,
    );
}

/// Chave (em `ctx.data`) do campo de texto focado no quadro: o anel de teclado o deixa em paz.
const TEXT_FOCUS_KEY: &str = "kf-text-focus";
/// Chave do modo de navegação: teclado (Tab) mostra o anel de foco; o ponteiro o esconde.
const KEYBOARD_NAV_KEY: &str = "kf-keyboard-nav";

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
    // Teclas do palco (com número) entram e saem de hover/seleção com transição; a miniatura do
    // inspetor não tem estado a animar.
    let (hot, sel) = match k.number {
        Some(n) => {
            let id = egui::Id::unique(("kf-key", n));
            (
                ease(p.ctx(), id.with("hot"), hovered, motion::FAST),
                ease(p.ctx(), id.with("sel"), selected, motion::BASE),
            )
        }
        None => (f32::from(u8::from(hovered)), f32::from(u8::from(selected))),
    };
    if k.off {
        paint_empty_key(p, rect, (hot, sel), pal, accent);
    } else if let Some(tex) = k.tex {
        paint_image_key(p, rect, tex, hot, accent);
    } else {
        paint_plain_key(p, rect, k, hot);
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
    if sel > 0.0 {
        let accent = accent.gamma_multiply(sel);
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
/// `(hot, sel)`: progresso animado de hover e de seleção (0 a 1).
fn paint_empty_key(
    p: &Painter,
    rect: Rect,
    (hot, sel): (f32, f32),
    pal: &Palette,
    accent: Color32,
) {
    let round = CornerRadius::from(KEY_RADIUS);
    p.rect_filled(rect, round, pal.keyoff);
    let rim = Stroke::new(1.0_f32, Color32::from_white_alpha(EMPTY_RIM_ALPHA));
    p.rect_stroke(rect, round, rim, egui::StrokeKind::Inside);
    // Com 15 teclas vazias, cantos e "+" acesos em todas viram ruído: em repouso só o "+",
    // mais baixo (ainda acima de 3:1 sobre o preto); os cantos surgem ao passar o mouse.
    let lit = hot.max(sel);
    if lit > 0.0 {
        hud_corners(p, rect, accent.gamma_multiply(EMPTY_CORNERS_HOT * lit));
    }
    let idle = pal.device_fg.gamma_multiply(EMPTY_PLUS_IDLE);
    let plus = blend(idle, accent, lit);
    let plus_font = FontId::monospace(text::DISPLAY);
    p.text(rect.center(), Align2::CENTER_CENTER, "+", plus_font, plus);
    if sel >= 1.0 && rect.width() >= EMPTY_HINT_MIN_WIDTH {
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
fn paint_image_key(p: &Painter, rect: Rect, tex: egui::TextureId, hot: f32, accent: Color32) {
    let round = CornerRadius::from(KEY_RADIUS);
    let mut shape = egui::epaint::RectShape::filled(rect, round, Color32::WHITE);
    shape = shape.with_texture(tex, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
    p.add(shape);
    if hot > 0.0 {
        let stroke = Stroke::new(KEY_HOVER_STROKE, accent.gamma_multiply(0.7 * hot));
        p.rect_stroke(rect, round, stroke, egui::StrokeKind::Inside);
    }
}

/// Tecla só com cor de fundo (sem textura): clareia ao passar o mouse e leva um brilho no alto.
fn paint_plain_key(p: &Painter, rect: Rect, k: &KeyView, hot: f32) {
    let fill = blend(k.bg, lighten(k.bg, 16), hot);
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
    let (cx, has_label) = (rect.center().x, !k.label.is_empty() && k.show_text);
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
    if !active {
        hover_veil(ui, &r, radius::PILL, pal);
    }
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, text));
    r
}

/// Botão quadrado só com ícone (desfazer, copiar...). `name` é o texto falado e a dica.
/// Em repouso o ícone fica um tom abaixo do texto; ao apontar ganha a cor cheia e uma superfície.
pub fn icon_button(ui: &mut Ui, icon: &str, name: &str, enabled: bool, pal: &Palette) -> Response {
    let idle = crate::editor::theme::mix(pal.muted, pal.fg, ICON_IDLE_INK);
    icon_button_inked(ui, icon, name, enabled, (idle, pal.fg), pal)
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
    icon_button_inked(ui, icon, name, enabled, (ink, ink), pal)
}

fn icon_button_inked(
    ui: &mut Ui,
    icon: &str,
    name: &str,
    enabled: bool,
    (idle, hot_ink): (Color32, Color32),
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
    let pressed = enabled && resp.is_pointer_button_down_on();
    let t = ease(ui.ctx(), resp.id.with("hot"), hot, motion::FAST);
    let round = CornerRadius::from(radius::CONTROL);
    if pressed {
        let fill = crate::editor::theme::mix(pal.raise, pal.fg, PRESSED_TINT);
        ui.painter()
            .rect_filled(rect.shrink(PRESS_INSET), round, fill);
    } else if t > 0.0 {
        let fill = crate::editor::theme::mix(pal.raise, pal.fg, HOVER_TINT);
        ui.painter()
            .rect_filled(rect, round, blend(Color32::TRANSPARENT, fill, t));
    }
    let color = if enabled {
        blend(idle, hot_ink, t)
    } else {
        pal.muted.gamma_multiply(DISABLED_ALPHA)
    };
    ui_icon(ui.painter(), rect.center(), ICON_GLYPH, icon, color);
    let resp = resp.on_hover_text(name);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Peso visual de um botão de texto.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonKind {
    /// Preenchimento sólido no destaque: a única ação principal da área.
    Primary,
    /// Contorno discreto, fundo transparente.
    Secondary,
    /// Sem contorno: ações de menu e de barra.
    Ghost,
}

/// Botão de texto do editor, com ícone Lucide opcional à esquerda. Desenhado à mão para ter
/// repouso, hover, pressionado e desligado distintos (o `egui::Button` com `fill` fixo não muda
/// ao apontar). `min_width` 0 = só o necessário para o conteúdo.
pub fn button(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
    min_width: f32,
    pal: &Palette,
) -> Response {
    use crate::editor::theme::{mix, readable_on};
    let label = crate::strings::t(label);
    let font = FontId::proportional(text::BODY);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let icon_w = if icon.is_some() {
        BUTTON_ICON + space::SM
    } else {
        0.0
    };
    let content = icon_w + galley.size().x;
    let width = (content + BUTTON_PAD_X * 2.0).max(min_width);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, BUTTON_HEIGHT), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let hot = enabled && (resp.hovered() || resp.has_focus());
    let pressed = enabled && resp.is_pointer_button_down_on();
    // Hover entra e sai com transição; o pressionado é imediato (a mão precisa sentir o clique).
    let t = ease(ui.ctx(), resp.id.with("hot"), hot, motion::FAST);
    let accent = ui.visuals().selection.bg_fill;
    let round = CornerRadius::from(radius::CONTROL);
    let (fill, ink, stroke) = match (kind, enabled) {
        (_, false) => (pal.field, pal.muted, Stroke::new(1.0_f32, pal.line)),
        (ButtonKind::Primary, true) => {
            let fill = if pressed {
                mix(accent, Color32::BLACK, PRESSED_TINT)
            } else {
                mix(accent, Color32::WHITE, HOVER_TINT * t)
            };
            (fill, readable_on(fill), Stroke::NONE)
        }
        (ButtonKind::Secondary | ButtonKind::Ghost, true) => {
            let rest = if kind == ButtonKind::Secondary {
                pal.raise
            } else {
                Color32::TRANSPARENT
            };
            let fill = if pressed {
                mix(pal.raise, pal.fg, PRESSED_TINT)
            } else {
                blend(rest, mix(pal.raise, pal.fg, HOVER_TINT), t)
            };
            let stroke = if kind == ButtonKind::Secondary {
                Stroke::new(1.0_f32, mix(pal.control, pal.fg, HOVER_TINT * t))
            } else {
                Stroke::NONE
            };
            (fill, pal.fg, stroke)
        }
    };
    let p = ui.painter();
    let body = if pressed {
        rect.shrink(PRESS_INSET)
    } else {
        rect
    };
    if kind == ButtonKind::Primary && enabled {
        let shadow = egui::epaint::Shadow {
            offset: [0, if pressed { 1 } else { 2 }],
            blur: 10,
            spread: 0,
            color: accent.gamma_multiply(0.28 + 0.17 * t),
        };
        p.add(shadow.as_shape(body, round));
    }
    p.rect(body, round, fill, stroke, egui::StrokeKind::Inside);
    if kind == ButtonKind::Primary && enabled {
        // Filete claro no alto: dá volume sem gradiente.
        let top = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.min.y + 1.0))
            .shrink2(vec2(radius::CONTROL, 0.0));
        p.rect_filled(
            top,
            CornerRadius::ZERO,
            Color32::from_white_alpha(BUTTON_SHEEN_ALPHA),
        );
    }
    let start = rect.center().x - content / 2.0;
    if let Some(name) = icon {
        ui_icon(
            p,
            pos2(start + BUTTON_ICON / 2.0, rect.center().y),
            BUTTON_ICON,
            name,
            ink,
        );
    }
    let text_pos = pos2(start + icon_w, rect.center().y - galley.size().y / 2.0);
    p.galley(text_pos, galley, ink);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Item de controle segmentado (abas do inspetor, páginas): o ativo ganha uma superfície elevada
/// com filete. `width` 0 = só o necessário para o texto.
pub fn segment(ui: &mut Ui, text: &str, on: bool, width: f32, pal: &Palette) -> Response {
    let text = crate::strings::t(text);
    let label = RichText::new(text).size(text::BODY - 1.0);
    let label = if on {
        label.color(pal.fg).strong()
    } else {
        label.color(pal.muted)
    };
    let (fill, stroke) = if on {
        (
            crate::editor::theme::mix(pal.raise, pal.fg, SEGMENT_ON_TINT),
            Stroke::new(
                1.0_f32,
                crate::editor::theme::mix(pal.line, pal.fg, SEGMENT_ON_TINT),
            ),
        )
    } else {
        (Color32::TRANSPARENT, Stroke::NONE)
    };
    let r = ui.add(
        egui::Button::new(label)
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::from(SEGMENT_RADIUS))
            .min_size(vec2(width, TARGET_MIN + 2.0)),
    );
    if !on {
        hover_veil(ui, &r, SEGMENT_RADIUS, pal);
    }
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, on, text));
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Como `segment`, para escolher uma opção entre várias (posição do rótulo): o leitor de tela o
/// anuncia como botão de opção, não como aba.
pub fn radio_segment(ui: &mut Ui, text: &str, on: bool, width: f32, pal: &Palette) -> Response {
    let r = segment(ui, text, on, width, pal);
    let name = crate::strings::t(text);
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, name));
    r
}

/// Linha de menu (menus suspensos e de contexto): ícone Lucide, rótulo e atalho alinhado à
/// direita, ocupando a largura do menu. `danger` pinta ícone e texto na cor de erro.
pub fn menu_entry(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    shortcut: &str,
    enabled: bool,
    danger: bool,
    pal: &Palette,
) -> Response {
    use crate::editor::theme::mix;
    let label = crate::strings::t(label);
    let ink = match (enabled, danger) {
        (false, _) => pal.muted.gamma_multiply(DISABLED_ALPHA),
        (true, true) => {
            crate::editor::theme::ensure_contrast(pal.bad, pal.raise, TEXT_MIN_CONTRAST)
        }
        (true, false) => pal.fg,
    };
    let font = FontId::proportional(text::BODY);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), Color32::PLACEHOLDER);
    let keys = (!shortcut.is_empty()).then(|| {
        ui.painter().layout_no_wrap(
            shortcut.to_string(),
            FontId::proportional(text::CAPTION),
            Color32::PLACEHOLDER,
        )
    });
    let keys_w = keys.as_ref().map_or(0.0, |k| k.size().x + space::XL);
    let content = MENU_ICON + space::MD + galley.size().x + keys_w + MENU_PAD_X * 2.0;
    let width = content.max(ui.available_width());
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, MENU_ROW), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::MenuItem, enabled, label));
    if !shortcut.is_empty() {
        // O atalho também é falado ("Copiar, Ctrl+C"), não só desenhado em cinza à direita.
        resp.ctx.accesskit_node_builder(resp.id, |node| {
            node.set_keyboard_shortcut(shortcut.to_string());
        });
    }
    let hot = enabled && (resp.hovered() || resp.has_focus());
    let t = ease(ui.ctx(), resp.id.with("hot"), hot, motion::FAST);
    let p = ui.painter();
    if t > 0.0 {
        let fill = if danger {
            pal.bad.gamma_multiply(DANGER_HOVER_ALPHA)
        } else {
            mix(pal.raise, pal.fg, HOVER_TINT)
        };
        let fill = blend(Color32::TRANSPARENT, fill, t);
        p.rect_filled(rect, CornerRadius::from(SEGMENT_RADIUS), fill);
    }
    let mut x = rect.left() + MENU_PAD_X;
    if let Some(name) = icon {
        let icon_ink = if danger || hot {
            ink
        } else {
            mix(pal.muted, pal.fg, ICON_IDLE_INK)
        };
        let icon_ink = if enabled { icon_ink } else { ink };
        ui_icon(
            p,
            pos2(x + MENU_ICON / 2.0, rect.center().y),
            MENU_ICON,
            name,
            icon_ink,
        );
    }
    x += MENU_ICON + space::MD;
    p.galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
    if let Some(k) = keys {
        let at = pos2(
            rect.right() - MENU_PAD_X - k.size().x,
            rect.center().y - k.size().y / 2.0,
        );
        p.galley(at, k, pal.muted);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Moldura dos segmentos: um trilho rebaixado com cantos arredondados.
pub fn segmented<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(pal.field)
        .stroke(Stroke::new(1.0_f32, pal.line))
        .corner_radius(CornerRadius::from(SEGMENT_RADIUS + 3.0))
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(add).inner
        })
        .inner
}

/// Marca do Keyforge na barra do topo: quadrado arredondado no destaque com o teclado Lucide.
pub fn brand_mark(ui: &mut Ui, accent: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(BRAND_MARK, BRAND_MARK), Sense::hover());
    let p = ui.painter();
    let round = CornerRadius::from(radius::CONTROL);
    p.add(
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: 12,
            spread: 0,
            color: accent.gamma_multiply(0.35),
        }
        .as_shape(rect, round),
    );
    p.rect_filled(rect, round, accent);
    let top = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
    let r = radius::CONTROL as u8;
    p.rect_filled(
        top,
        CornerRadius {
            nw: r,
            ne: r,
            sw: 0,
            se: 0,
        },
        Color32::from_white_alpha(BRAND_SHEEN_ALPHA),
    );
    let ink = crate::editor::theme::readable_on(accent);
    ui_icon(p, rect.center(), BRAND_GLYPH, "keyboard", ink);
    resp
}

/// Rótulo de seção em caixa alta. Sem espaçamento artificial entre letras: o espaço fino que
/// simulava isso era lido letra por letra por leitores de tela e quebrava busca e cópia.
/// Devolve o id do rótulo, para o campo que ele nomeia usar `labelled_by` (leitores de tela).
/// Quadros, desde a abertura de um modal, em que o foco ainda é trazido para dentro dele.
const MODAL_FOCUS_FRAMES: u64 = 3;
/// Respiro interno das janelas modais.
const MODAL_PADDING: i8 = 20;

/// Quanto o título de seção puxa da cor do texto (0 = apagado, 1 = texto).
const SECTION_INK: f32 = 0.45;

pub fn section(ui: &mut Ui, text: &str, pal: &Palette) -> egui::Id {
    let text = crate::strings::t(text);
    // Respiro maior acima do que abaixo: o título cola no que nomeia e se separa do grupo anterior.
    ui.add_space(space::MD);
    // Caixa normal e um tom acima do texto apagado: título de seção se lê, não se decifra.
    let id = ui
        .label(
            RichText::new(text)
                .size(text::BODY - 1.0)
                .strong()
                .color(crate::editor::theme::mix(pal.muted, pal.fg, SECTION_INK)),
        )
        .id;
    heading(ui.ctx(), id);
    id
}

/// Marca um rótulo como título (WCAG 1.3.1): o leitor de tela pula de seção em seção com `H`.
pub fn heading(ctx: &egui::Context, id: egui::Id) {
    ctx.accesskit_node_builder(id, |node| {
        node.set_role(egui::accesskit::Role::Heading);
        // O egui guarda o texto de um rótulo em `value`; um título se nomeia por `label`, e é
        // por ele que os campos ligados com `labelled_by` recebem o nome.
        if let Some(text) = node.value().map(str::to_string) {
            node.set_label(text);
            node.clear_value();
        }
    });
}

/// Dá papel e nome a uma região da janela (barra, biblioteca, inspetor, palco): o leitor de tela
/// lista essas regiões e pula direto para elas (WCAG 1.3.1 e 2.4.1).
pub fn landmark(ctx: &egui::Context, id: egui::Id, role: egui::accesskit::Role, name: &str) {
    let name = crate::strings::t(name).to_string();
    ctx.accesskit_node_builder(id, |node| {
        node.set_role(role);
        node.set_label(name);
    });
}

/// Região viva: o leitor de tela lê o texto dela quando muda, sem tirar o foco de onde está
/// (avisos, gravação de atalho, erros). WCAG 4.1.3.
pub fn live_region(ctx: &egui::Context, id: egui::Id) {
    ctx.accesskit_node_builder(id, |node| {
        node.set_live(egui::accesskit::Live::Polite);
    });
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
        .inner_margin(egui::Margin::symmetric(space::MD as i8, 4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = space::SM;
            ui.horizontal(|ui| {
                let (dot, _) =
                    ui.allocate_exact_size(vec2(CHIP_DOT * 2.0, CHIP_DOT * 2.0), Sense::hover());
                let p = ui.painter();
                p.circle_filled(dot.center(), CHIP_DOT + 2.5, ink.gamma_multiply(0.18));
                p.circle_filled(dot.center(), CHIP_DOT, ink);
                let text = ui.label(
                    RichText::new(label)
                        .size(text::CAPTION + 0.5)
                        .strong()
                        .color(ink),
                );
                // Mudou de "No aparelho" para "Desconectado": quem não vê a tela também fica sabendo.
                live_region(ui.ctx(), text.id);
                text
            })
            .inner
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
    let t = ease(ui.ctx(), resp.id.with("hot"), resp.hovered(), motion::FAST);
    let body = rect.expand(SWATCH_HOVER_GROW * t);
    let p = ui.painter();
    p.rect_filled(body, CornerRadius::from(radius), fill);
    p.rect_stroke(
        body,
        CornerRadius::from(radius),
        Stroke::new(1.0_f32, Color32::from_white_alpha(28)),
        egui::StrokeKind::Inside,
    );
    let on = ease(ui.ctx(), resp.id.with("on"), selected, motion::BASE);
    if on > 0.0 {
        p.rect_stroke(
            rect.expand(SWATCH_RING_GAP),
            CornerRadius::from(radius + SWATCH_RING_GAP),
            Stroke::new(2.0_f32, ring.gamma_multiply(on)),
            egui::StrokeKind::Inside,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Anel de foco do teclado em dois tons (branco por dentro, escuro por fora): aparece sobre qualquer
/// fundo, inclusive as teclas, que são escuras nos dois temas (um anel só na cor do texto sumia no
/// tema claro). Fica distinto de hover (tinta) e de seleção (brilho de destaque).
///
/// Como o `:focus-visible` da web: aparece quando se navega pelo teclado (Tab, setas) e some ao
/// usar o ponteiro, para um clique de mouse não deixar um contorno branco no botão. Campos de
/// texto têm o próprio halo (`field_focus`) e não ganham um segundo anel.
pub fn focus_ring(ctx: &egui::Context) {
    let (keys, pointer) = ctx.input(|i| {
        let nav = [
            egui::Key::Tab,
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
        ];
        (
            nav.iter().any(|k| i.key_pressed(*k)),
            i.pointer.any_pressed(),
        )
    });
    let nav_key = egui::Id::unique(KEYBOARD_NAV_KEY);
    let text_key = egui::Id::unique(TEXT_FOCUS_KEY);
    let keyboard = ctx.data_mut(|d| {
        let mode = d.get_temp_mut_or(nav_key, false);
        if keys {
            *mode = true;
        } else if pointer {
            *mode = false;
        }
        *mode
    });
    let focused = ctx.memory(eframe::egui::Memory::focused);
    let text_field = ctx
        .data_mut(|d| d.remove_temp::<Option<egui::Id>>(text_key))
        .flatten();
    let Some((id, rect)) = focused
        .filter(|id| keyboard && text_field != Some(*id))
        .and_then(|id| ctx.read_response(id).map(|r| (id, r.rect)))
    else {
        return;
    };
    // Surge com transição a cada controle novo que recebe o foco.
    let t = ease(ctx, id.with("focus-ring"), true, motion::FAST);
    if t <= 0.0 {
        return;
    }
    // Acima de tudo (inclusive modais e menus): chamado no fim do quadro, depois das janelas.
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::unique("focus_ring"),
    ));
    let inner = rect.expand(FOCUS_RING_GAP);
    painter.rect_stroke(
        inner.expand(FOCUS_RING_WIDTH),
        CornerRadius::from(FOCUS_RING_RADIUS + FOCUS_RING_WIDTH),
        Stroke::new(FOCUS_RING_WIDTH, FOCUS_RING_OUTER.gamma_multiply(t)),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        inner,
        CornerRadius::from(FOCUS_RING_RADIUS),
        Stroke::new(FOCUS_RING_WIDTH, Color32::WHITE.gamma_multiply(t)),
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
    field_focus(ui, &resp, pal);
    // A lupa acende junto com o campo focado.
    let lens = if resp.has_focus() { pal.fg } else { pal.muted };
    ui_icon(ui.painter(), center, ICON, "search", lens);
    name(&resp, hint_text);
    resp
}

/// Seção recolhível do inspetor: título, e quando fechada um resumo do que tem dentro (para não
/// esconder o que a tecla faz). `on` diz se a seção está em uso: só então começa aberta.
pub fn fold(
    ui: &mut Ui,
    id: &str,
    title: &str,
    summary: Option<String>,
    on: bool,
    pal: &Palette,
    body: impl FnOnce(&mut Ui),
) {
    let title = crate::strings::t(title);
    let header = match summary {
        Some(s) => RichText::new(format!("{title} · {s}")),
        None => RichText::new(title),
    };
    ui.add_space(space::XS);
    let chevron = pal.muted;
    egui::CollapsingHeader::new(header.strong().color(pal.fg))
        .id_salt(id)
        .default_open(on)
        .icon(move |ui, openness, resp| {
            // Durante a abertura as duas setas se fundem: a seta parece girar em vez de pular.
            for (name, alpha) in [
                ("chevron-right", 1.0 - openness),
                ("chevron-down", openness),
            ] {
                if alpha > 0.0 {
                    ui_icon(
                        ui.painter(),
                        resp.rect.center(),
                        FOLD_CHEVRON,
                        name,
                        chevron.gamma_multiply(alpha),
                    );
                }
            }
        })
        .show(ui, body);
}

/// Janela modal do editor: no centro, com o fundo escurecido, título e X. Esc, o X ou um clique
/// fora pedem para fechar (o segundo valor). `title` já vem traduzido.
pub fn modal<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    pal: &Palette,
    body: impl FnOnce(&mut Ui) -> R,
) -> (R, bool) {
    let frame = egui::Frame::NONE
        .fill(pal.panel)
        .stroke(Stroke::new(1.0_f32, pal.line))
        .corner_radius(CornerRadius::from(radius::CARD + 4.0))
        .inner_margin(egui::Margin::same(MODAL_PADDING))
        .shadow(crate::editor::theme::shadow(
            elevation::MODAL,
            ctx.global_style().visuals.dark_mode,
        ));
    let mut close = false;
    let modal_id = egui::Id::unique(id);
    // Foco do teclado ao abrir: entra no modal (no X, se o corpo não pedir outro) e, ao fechar,
    // volta ao controle que o abriu (WCAG 2.4.3).
    let opener_key = modal_id.with("opener");
    let first_frame = ctx
        .data(|d| d.get_temp::<Option<egui::Id>>(opener_key))
        .is_none();
    let opened_key = modal_id.with("opened-frame");
    if first_frame {
        let opener = ctx.memory(egui::Memory::focused);
        let now = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| {
            d.insert_temp(opener_key, opener);
            d.insert_temp(opened_key, now);
        });
    }
    // Os primeiros quadros do modal: o egui primeiro mede a janela (invisível) e um pedido de foco
    // nesse quadro pode se perder.
    let just_opened = ctx.cumulative_frame_nr()
        < ctx.data(|d| d.get_temp::<u64>(opened_key)).unwrap_or(0) + MODAL_FOCUS_FRAMES;
    let shown = egui::Modal::new(modal_id).frame(frame).show(ctx, |ui| {
        ui.set_width(width);
        ui.horizontal(|ui| {
            let title = ui.label(
                RichText::new(title)
                    .font(crate::editor::theme::display_font(text::TITLE_LG))
                    .color(pal.fg),
            );
            heading(ui.ctx(), title.id);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let x = icon_button(ui, "close", "Fechar", true, pal);
                // Foco ainda fora (no botão que abriu, atrás do fundo escuro) ou em lugar nenhum:
                // traz para o X. Se o corpo pedir foco (a busca), o pedido dele vence.
                let focused = ui.ctx().memory(egui::Memory::focused);
                let opener = ui
                    .ctx()
                    .data(|d| d.get_temp::<Option<egui::Id>>(opener_key))
                    .flatten();
                if just_opened && (focused.is_none() || focused == opener) {
                    x.request_focus();
                }
                close = x.clicked();
            });
        });
        ui.add_space(space::SM);
        ui.painter().hline(
            ui.min_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0_f32, pal.line),
        );
        ui.add_space(space::LG);
        body(ui)
    });
    let close = close || shown.should_close();
    // Diálogo modal com o título como nome: o leitor de tela anuncia "Preferências, diálogo".
    ctx.accesskit_node_builder(shown.response.id, |node| {
        node.set_role(egui::accesskit::Role::Dialog);
        node.set_modal();
        node.set_label(title.to_string());
    });
    if close {
        let opener = ctx.data_mut(|d| d.remove_temp::<Option<egui::Id>>(opener_key));
        if let Some(Some(opener)) = opener {
            ctx.memory_mut(|m| m.request_focus(opener));
        }
    }
    (shown.inner, close)
}

/// Esquece o foco guardado por `modal` quando ele fecha por outro caminho (escolher um app
/// fecha a lista sem passar pelo X): na próxima abertura o foco entra de novo no modal.
pub fn modal_closed(ctx: &egui::Context, id: &str) {
    let opener_key = egui::Id::unique(id).with("opener");
    let opener = ctx.data_mut(|d| d.remove_temp::<Option<egui::Id>>(opener_key));
    if let Some(Some(opener)) = opener {
        ctx.memory_mut(|m| m.request_focus(opener));
    }
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

/// Campo de texto padrão: altura `TARGET_MIN`, borda visível, raio `radius::CONTROL`.
/// O texto do placeholder é em `pal.muted` para não se confundir com valor digitado.
/// Quando `password` é true, oculta os caracteres (campo de senha).
pub fn text_field(
    ui: &mut Ui,
    text: &mut String,
    hint: &str,
    pal: &Palette,
    password: bool,
) -> Response {
    let mut edit = egui::TextEdit::singleline(text)
        .hint_text(crate::strings::t(hint))
        .text_color(pal.fg)
        .margin(egui::Margin::symmetric(space::SM as i8, space::XS as i8))
        .desired_width(ui.available_width());
    if password {
        edit = edit.password(true);
    }
    let resp = ui.add(edit);
    field_focus(ui, &resp, pal);
    resp
}

/// Campo de texto com largura fixa (útil para modais e painéis estreitos).
pub fn text_field_width(
    ui: &mut Ui,
    text: &mut String,
    hint: &str,
    pal: &Palette,
    password: bool,
    width: f32,
) -> Response {
    let mut edit = egui::TextEdit::singleline(text)
        .hint_text(crate::strings::t(hint))
        .text_color(pal.fg)
        .margin(egui::Margin::symmetric(space::SM as i8, space::XS as i8))
        .desired_width(width);
    if password {
        edit = edit.password(true);
    }
    let resp = ui.add(edit);
    field_focus(ui, &resp, pal);
    resp
}

/// Campo de texto de várias linhas: altura flexível, borda visível, raio `radius::CONTROL`.
pub fn text_area(ui: &mut Ui, text: &mut String, hint: &str, pal: &Palette, rows: u32) -> Response {
    let edit = egui::TextEdit::multiline(text)
        .hint_text(crate::strings::t(hint))
        .text_color(pal.fg)
        .margin(egui::Margin::symmetric(space::SM as i8, space::XS as i8))
        .desired_rows(rows as usize)
        .desired_width(ui.available_width());
    let resp = ui.add(edit);
    field_focus(ui, &resp, pal);
    resp
}

/// Campo numérico (u64): edição como string, validação, devolve `true` se mudou.
pub fn numeric_field(ui: &mut Ui, value: &mut u64, hint: &str, pal: &Palette, width: f32) -> bool {
    let mut s = value.to_string();
    let resp = ui.add(
        egui::TextEdit::singleline(&mut s)
            .hint_text(crate::strings::t(hint))
            .text_color(pal.fg)
            .margin(egui::Margin::symmetric(space::SM as i8, space::XS as i8))
            .desired_width(width),
    );
    field_focus(ui, &resp, pal);
    // Sem rótulo ao lado ligado a ele, a dica é o único nome que o leitor de tela teria.
    if !hint.is_empty() {
        name(&resp, hint);
    }
    if resp.changed() {
        if let Ok(v) = s.parse::<u64>() {
            *value = v;
            return true;
        }
    }
    false
}

/// Botão secundário: contorno na cor de controle, texto na cor normal.
pub fn secondary_button(ui: &mut Ui, text: &str, pal: &Palette) -> Response {
    button(ui, None, text, ButtonKind::Secondary, true, 0.0, pal)
}

/// Slider padrão: faixa do mínimo ao máximo, sufixo opcional.
pub fn slider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) -> Response {
    let s = egui::Slider::new(value, range);
    let s = if suffix.is_empty() {
        s
    } else {
        s.suffix(suffix)
    };
    ui.add(s)
}

/// Combo box padrão: seleção única com seta, raio `radius::CONTROL`. `id` distingue combos na
/// mesma área (sem ele, dois combos sem rótulo dividiriam o mesmo estado de aberto/fechado).
/// `name` é o que o seletor escolhe ("Perfil", "Apagar a tela sem uso"): o leitor de tela diz
/// o nome e o valor ("Perfil, Padrão, caixa de seleção"), não só o valor solto.
pub fn combo_box(
    ui: &mut Ui,
    id: &str,
    name: &str,
    selected: &mut String,
    options: &[&str],
    _pal: &Palette,
) -> Response {
    let selected_label = selected.clone();
    let shown = if selected_label.is_empty() {
        crate::strings::t("(escolha)").to_string()
    } else {
        selected_label.clone()
    };
    let before = selected.clone();
    let mut resp = egui::ComboBox::from_id_salt(id)
        .selected_text(&shown)
        .show_ui(ui, |ui| {
            for opt in options {
                let on = selected.as_str() == *opt;
                let item = ui.selectable_value(selected, opt.to_string(), crate::strings::t(opt));
                let opt = crate::strings::t(opt).to_string();
                item.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, on, &opt)
                });
            }
        })
        .response;
    if *selected != before {
        resp.mark_changed();
    }
    let name = crate::strings::t(name).to_string();
    resp.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::ComboBox, true, name.clone());
        info.current_text_value = Some(shown.clone());
        info
    });
    resp
}

/// Área de rolagem padrão: barra fina, sem excesso de preenchimento.
pub fn scroll_area<R>(_ui: &mut Ui, _pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(_ui, add)
        .inner
}

/// Caixa de seleção (checkbox): quadrado com borda de controle; marcada, preenchida no destaque
/// com o "check" do Lucide. Todo o rótulo é clicável e o alvo nunca fica abaixo de `TARGET_MIN`.
pub fn checkbox(ui: &mut Ui, checked: &mut bool, label: &str) -> Response {
    let label = crate::strings::t(label);
    let visuals = ui.visuals().clone();
    let accent = visuals.selection.bg_fill;
    let font = FontId::proportional(text::BODY);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let width = CHECK_BOX + space::SM + 2.0 + galley.size().x;
    let (rect, mut resp) =
        ui.allocate_exact_size(vec2(width, TARGET_MIN.max(galley.size().y)), Sense::click());
    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }
    let on = *checked;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, label));
    let hot = resp.hovered() || resp.has_focus();
    let box_rect = Rect::from_center_size(
        pos2(rect.left() + CHECK_BOX / 2.0, rect.center().y),
        vec2(CHECK_BOX, CHECK_BOX),
    );
    let p = ui.painter();
    let round = CornerRadius::from(CHECK_RADIUS);
    // Marcar faz a caixa "saltar": o preenchimento cresce do centro e o check surge junto.
    let t = ease(ui.ctx(), resp.id.with("on"), on, motion::BASE);
    if t < 1.0 {
        let stroke = if hot {
            visuals.widgets.hovered.bg_stroke
        } else {
            visuals.widgets.inactive.bg_stroke
        };
        p.rect(
            box_rect,
            round,
            visuals.extreme_bg_color,
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    if t > 0.0 {
        let fill = if hot {
            crate::editor::theme::mix(accent, Color32::WHITE, HOVER_TINT)
        } else {
            accent
        };
        let scale = CHECK_POP_FROM + (1.0 - CHECK_POP_FROM) * t;
        let body = Rect::from_center_size(box_rect.center(), box_rect.size() * scale);
        p.rect_filled(body, round, fill.gamma_multiply(t));
        let ink = crate::editor::theme::readable_on(fill);
        ui_icon(
            p,
            box_rect.center(),
            CHECK_GLYPH,
            "lucide:check",
            ink.gamma_multiply(t),
        );
    }
    let text_color = visuals.override_text_color.unwrap_or(visuals.text_color());
    let text_pos = pos2(
        box_rect.right() + space::SM + 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    p.galley(text_pos, galley, text_color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Filete entre grupos de um painel (conteúdo / cor / ícone...): separa blocos sem caixas
/// aninhadas, que no preto puro pesam mais do que organizam.
pub fn divider(ui: &mut Ui, pal: &Palette) {
    ui.add_space(space::SM);
    let y = ui.cursor().top();
    ui.painter()
        .hline(ui.max_rect().x_range(), y, Stroke::new(1.0_f32, pal.line));
    ui.add_space(space::XS);
}

/// Cartão com sombra suave para agrupamento visual de conteúdo relacionado.
pub fn card<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(pal.raise)
        .stroke(Stroke::new(1.0_f32, pal.line))
        .corner_radius(CornerRadius::from(radius::CARD))
        .inner_margin(egui::Margin::same(space::LG as i8 - 2))
        .shadow(crate::editor::theme::shadow(
            elevation::CARD,
            ui.visuals().dark_mode,
        ))
        .show(ui, add)
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ui_icon_maps_to_a_real_lucide_icon() {
        for (short, _) in UI_ICONS {
            let id = lucide_id(short);
            // Ou é um ícone Lucide real, ou um ícone built-in (como "keyforge") que mapeia para si mesmo
            assert!(
                crate::graphics::icon_library::find(&id).is_some() || short == "keyforge",
                "{short} -> {id}"
            );
        }
        assert_eq!(lucide_id("lucide:mic"), "lucide:mic");
        assert_eq!(lucide_id("close"), "lucide:x");
    }

    #[test]
    fn ui_icons_render_as_white_textures_once_per_size() {
        let ctx = egui::Context::default();
        let a = ui_icon_texture(&ctx, "lucide:x", 18).expect("x");
        let b = ui_icon_texture(&ctx, "lucide:x", 18).expect("x");
        assert_eq!(a, b);
        assert!(ui_icon_texture(&ctx, "lucide:nao-existe", 18).is_none());
    }
}
