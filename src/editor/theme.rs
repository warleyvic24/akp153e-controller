//! Aparência da interface: paleta escura/clara, cor de destaque do usuário, fontes do Windows e estilo dos widgets.
use crate::config::schema::{Theme, UiSettings};
use crate::editor::omarchy;
use crate::editor::tokens::{
    elevation, motion, radius, space, text, COMPONENT_MIN_CONTRAST, TARGET_MIN,
};
#[cfg(test)]
use crate::editor::tokens::{TEXT_AAA_CONTRAST, TEXT_MIN_CONTRAST};
use eframe::egui::{
    self, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};

/// Cores de destaque sugeridas (o usuário também pode escolher qualquer outra).
/// Paleta moderna inspirada em design systems contemporâneos (Linear, Vercel, Stripe).
pub const ACCENTS: [(&str, [u8; 3]); 8] = [
    ("Índigo", [99, 102, 241]),    // Modern indigo - professional
    ("Esmeralda", [16, 185, 129]), // Fresh emerald - success
    ("Rosa", [236, 72, 153]),      // Modern pink - creative
    ("Âmbar", [245, 158, 11]),     // Warm amber - warning
    ("Ciano", [6, 182, 212]),      // Cyan - tech
    ("Violeta", [168, 85, 247]),   // Purple - creative
    ("Laranja", [249, 115, 22]),   // Orange - energy
    ("Vermelho", [239, 68, 68]),   // Red - danger
];

const HOVER_TINT: f32 = 0.12; // Subtler hover
const CONTRAST_STEPS: u8 = 20;
pub const DEVICE_FG: [u8; 3] = [176, 184, 210];
const RAIL_TINT: f32 = 0.18; // Softer rail
/// Base do hover quando o tema vem do Omarchy: um passo da superfície rumo ao texto.
const OMARCHY_HOVER_BASE_TINT: f32 = 0.08;
/// Base do hover no tema escuro: neutra, um passo acima da superfície elevada.
const DARK_HOVER_BASE_TINT: f32 = 0.06;
/// Largura da barra de rolagem flutuante (fina em repouso, cresce ao apontar).
const SCROLL_BAR_WIDTH: f32 = 6.0;
/// Espera antes de uma dica aparecer: o bastante para não piscar ao cruzar a tela com o mouse.
const TOOLTIP_DELAY: f32 = 0.35;
/// Ao passar de um controle a outro, a dica seguinte aparece sem nova espera durante este tempo.
const TOOLTIP_GRACE: f32 = 0.4;
pub const DISPLAY_FAMILY: &str = "display";

/// Superfícies e textos de um tema (o aparelho e as teclas ficam escuros nos dois).
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub raise: Color32,
    pub field: Color32,
    pub line: Color32,
    pub fg: Color32,
    pub muted: Color32,
    pub device: Color32,
    pub bezel: Color32,
    pub keyoff: Color32,
    pub ok: Color32,
    pub bad: Color32,
    /// Borda de controles interativos (campos, botões): derivada de `line` até dar 3:1.
    pub control: Color32,
    /// Texto e símbolos sobre as teclas, que são escuras nos dois temas.
    pub device_fg: Color32,
}

pub fn palette(theme: Theme) -> Palette {
    with_control(base_palette(theme))
}

/// Preenche `control`, a borda dos controles, derivada de `line` até dar 3:1 sobre as superfícies.
pub fn with_control(mut p: Palette) -> Palette {
    let surfaces = [p.raise, p.panel];
    let sequential = ensure_contrast(
        ensure_contrast(p.line, p.raise, COMPONENT_MIN_CONTRAST),
        p.panel,
        COMPONENT_MIN_CONTRAST,
    );
    // A correção em sequência pode deixar uma das superfícies por um triz abaixo do mínimo; nesse
    // caso se refaz de uma vez só para as duas.
    p.control = if surfaces
        .iter()
        .all(|s| contrast(sequential, *s) >= COMPONENT_MIN_CONTRAST)
    {
        sequential
    } else {
        ensure_contrast_on(p.line, &surfaces, COMPONENT_MIN_CONTRAST)
    };
    p
}

/// `c`, clareada ou escurecida só o necessário para ter o contraste `min` sobre todas as
/// superfícies ao mesmo tempo (corrigir uma de cada vez pode desfazer a anterior). Se nem preto
/// nem branco chegam lá, devolve o que mais se aproxima.
pub fn ensure_contrast_on(c: Color32, surfaces: &[Color32], min: f32) -> Color32 {
    const STEPS: u8 = 40;
    let worst = |x: Color32| {
        surfaces
            .iter()
            .map(|s| contrast(x, *s))
            .fold(f32::MAX, f32::min)
    };
    if worst(c) >= min {
        return c;
    }
    let distance = |x: Color32| {
        [(x.r(), c.r()), (x.g(), c.g()), (x.b(), c.b())]
            .iter()
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).pow(2))
            .sum::<i32>()
    };
    [Color32::WHITE, Color32::BLACK]
        .into_iter()
        .filter_map(|target| {
            (1..=STEPS)
                .map(|i| mix(c, target, f32::from(i) / f32::from(STEPS)))
                .find(|x| worst(*x) >= min)
        })
        .min_by_key(|x| distance(*x))
        .unwrap_or_else(|| {
            if worst(Color32::WHITE) >= worst(Color32::BLACK) {
                Color32::WHITE
            } else {
                Color32::BLACK
            }
        })
}

/// O tema é escuro? Nos temas fixos é óbvio; o do Omarchy decide pelo fundo (sem Omarchy, escuro).
pub fn is_dark(theme: Theme) -> bool {
    match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::HighContrast => true,
        Theme::Omarchy => omarchy::current().is_none_or(|t| t.dark),
    }
}

fn base_palette(theme: Theme) -> Palette {
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    let device_fg = c(DEVICE_FG[0], DEVICE_FG[1], DEVICE_FG[2]);
    match theme {
        Theme::Omarchy => match omarchy::current() {
            Some(t) => t.palette,
            None => base_palette(Theme::Dark),
        },
        Theme::Dark => Palette {
            // Preto de verdade (OLED), com superfícies neutras em degraus curtos: a hierarquia
            // vem da elevação e dos filetes, não de um tom azulado.
            bg: c(0, 0, 0),          // Palco: preto puro, o aparelho "flutua" nele
            panel: c(0, 0, 0),       // Barras e painéis: também pretos, separados por filetes
            raise: c(18, 18, 22),    // Cartões, menus, modais, botões
            field: c(11, 11, 14),    // Campos: um degrau acima do preto, com borda de controle
            line: c(38, 38, 45),     // Filetes: discretos no preto (a borda de controle tem 3:1)
            fg: c(244, 244, 246),    // Texto
            muted: c(163, 163, 175), // Texto secundário (≥ 7:1, AAA, em todas as superfícies)
            device: c(14, 14, 17),   // Corpo do aparelho
            bezel: c(40, 40, 46),    // Moldura do aparelho
            keyoff: c(0, 0, 0),      // Tecla apagada
            ok: c(52, 211, 153),     // Esmeralda
            bad: c(252, 129, 129),   // Vermelho suave (7:1 no preto e no elevado)
            control: Color32::TRANSPARENT,
            device_fg,
        },
        Theme::Light => Palette {
            bg: c(243, 245, 249),    // Palco: um degrau abaixo dos painéis
            panel: c(255, 255, 255), // Painéis brancos
            raise: c(250, 251, 253), // Cartões e menus: distintos do painel
            field: c(246, 248, 251), // Campos
            line: c(226, 232, 240),  // Filetes
            fg: c(15, 23, 42),       // Slate 900
            muted: c(64, 76, 96),    // Slate 650: ≥ 7:1 (AAA) também sobre o palco
            device: c(15, 23, 42),   // Deep device surface
            bezel: c(71, 85, 105),   // Slate 600
            keyoff: c(0, 0, 0),
            ok: c(4, 84, 62),    // Esmeralda escura: 7:1 sobre o branco e o palco
            bad: c(150, 18, 32), // Vermelho escuro: 7:1 sobre o branco e o palco
            control: Color32::TRANSPARENT,
            device_fg,
        },
        // Alto contraste: preto puro, texto branco, filetes claros e bordas grossas (ver `apply`).
        Theme::HighContrast => Palette {
            bg: c(0, 0, 0),
            panel: c(0, 0, 0),
            raise: c(10, 10, 10),
            field: c(0, 0, 0),
            line: c(200, 200, 200),
            fg: c(255, 255, 255),
            muted: c(230, 230, 230),
            device: c(0, 0, 0),
            bezel: c(128, 128, 128),
            keyoff: c(0, 0, 0),
            ok: c(80, 255, 160),
            bad: c(255, 120, 120),
            control: Color32::TRANSPARENT,
            device_fg: c(255, 255, 255),
        },
    }
}

/// Destaque do alto contraste: amarelo, 15:1 sobre o preto (a escolha do usuário volta ao sair).
pub const HIGH_CONTRAST_ACCENT: Color32 = Color32::from_rgb(255, 214, 0);
/// Espessura das bordas de controle no alto contraste (no normal, 1 px).
const HIGH_CONTRAST_STROKE: f32 = 2.0;

/// A cor de destaque: a escolhida pelo usuário, ou a do tema quando ele vem do Omarchy.
pub fn accent_color(ui: &UiSettings) -> Color32 {
    if ui.theme == Theme::HighContrast {
        return HIGH_CONTRAST_ACCENT;
    }
    if ui.theme == Theme::Omarchy {
        if let Some(t) = omarchy::current() {
            return t.accent;
        }
    }
    Color32::from_rgb(ui.accent[0], ui.accent[1], ui.accent[2])
}

/// Luminância relativa WCAG (0 a 1); a transparência é ignorada.
pub fn luminance(c: Color32) -> f32 {
    let channel = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// Razão de contraste WCAG entre duas cores (1 a 21).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Clareia ou escurece `fg` (rumo ao branco ou ao preto, o que afastar de `bg`) só o necessário
/// para atingir `min`. Se já atinge, devolve `fg` intacta: o tema escuro não muda de aparência.
pub fn ensure_contrast(fg: Color32, bg: Color32, min: f32) -> Color32 {
    if contrast(fg, bg) >= min {
        return fg;
    }
    let target = if luminance(bg) > 0.5 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    (1..=CONTRAST_STEPS)
        .map(|i| mix(fg, target, f32::from(i) / f32::from(CONTRAST_STEPS)))
        .find(|c| contrast(*c, bg) >= min)
        .unwrap_or(target)
}

/// A cor de destaque usada como traço, ícone ou texto: a mesma, mas nunca abaixo de 3:1 sobre as
/// superfícies do tema (no claro, ciano e verde-ácido sumiriam sobre o branco).
pub fn ink(accent: Color32, pal: &Palette) -> Color32 {
    ensure_contrast(
        ensure_contrast(accent, pal.raise, COMPONENT_MIN_CONTRAST),
        pal.panel,
        COMPONENT_MIN_CONTRAST,
    )
}

/// Branco ou preto, o que tiver maior razão de contraste sobre `bg`.
pub fn readable_on(bg: Color32) -> Color32 {
    if contrast(Color32::BLACK, bg) >= contrast(Color32::WHITE, bg) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Mistura `a` com `b`; `t` é a parte de `b` (0 a 1).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Sombra de um degrau de `tokens::elevation`; no escuro mais densa (sobre o preto, só ela não
/// some), no claro bem leve.
pub fn shadow((offset, blur): (i8, u8), dark: bool) -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, offset],
        blur,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 190 } else { 34 }),
    }
}

pub fn display_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DISPLAY_FAMILY.into()))
}

/// Fontes do Windows: Segoe UI, Consolas e Bahnschrift.
#[cfg(windows)]
fn system_fonts() -> Vec<(&'static str, std::path::PathBuf)> {
    [
        ("ui", r"C:\Windows\Fonts\segoeui.ttf"),
        ("mono", r"C:\Windows\Fonts\consola.ttf"),
        ("display", r"C:\Windows\Fonts\bahnschrift.ttf"),
        ("symbols", r"C:\Windows\Fonts\seguisym.ttf"),
    ]
    .map(|(name, path)| (name, path.into()))
    .into()
}

/// Uma fonte com os símbolos que a interface usa (▶ ▾ ↑ ↓ ● −): desde o egui 0.37 as fontes
/// embutidas não têm mais esses símbolos, e sem ela eles viram um quadrado.
#[cfg(not(windows))]
const SYMBOL_PATTERN: &str = "sans-serif:charset=25b6 25be 2191 2193 25cf 2212";

/// Fontes do Linux: o `fc-match` diz qual arquivo o sistema usa para sans-serif e monospace (no
/// Omarchy, as fontes que o usuário escolheu). Sem fonte de títulos, eles usam a de texto.
#[cfg(not(windows))]
fn system_fonts() -> Vec<(&'static str, std::path::PathBuf)> {
    [
        ("ui", "sans-serif"),
        ("mono", "monospace"),
        ("symbols", SYMBOL_PATTERN),
    ]
    .into_iter()
    .filter_map(|(name, family)| Some((name, crate::platform::fc_match_file(family)?)))
    .collect()
}

/// Carrega as fontes do sistema; o que faltar cai nas fontes padrão do egui.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for (name, path) in system_fonts() {
        // Só entra fonte que o leitor de fontes aceita: uma inválida derrubaria a interface.
        let Some(bytes) = std::fs::read(path)
            .ok()
            .filter(|b| ab_glyph::FontArc::try_from_vec(b.clone()).is_ok())
        else {
            continue;
        };
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
    }
    let has = |n: &str| fonts.font_data.contains_key(n);
    let (ui, mono, display) = (has("ui"), has("mono"), has("display"));
    if ui {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "ui".into());
    }
    if mono {
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "mono".into());
    }
    let mut display_stack = Vec::new();
    if display {
        display_stack.push("display".to_string());
    }
    display_stack.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts
        .families
        .insert(FontFamily::Name(DISPLAY_FAMILY.into()), display_stack);
    // Por último em toda família: só entra quando a fonte do texto não tem o símbolo.
    if has("symbols") {
        for family in fonts.families.values_mut() {
            family.push("symbols".into());
        }
    }
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &Context, ui: &UiSettings) {
    let (p, accent) = (palette(ui.theme), accent_color(ui));
    let dark = is_dark(ui.theme);
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.panel;
    v.window_fill = p.raise;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.raise;
    v.override_text_color = Some(p.fg);
    let ink = ink(accent, &p);
    v.window_corner_radius = egui::CornerRadius::from(radius::CARD);
    v.window_shadow = shadow(elevation::MODAL, dark);
    v.popup_shadow = shadow(elevation::POPUP, dark);
    v.window_stroke = Stroke::new(1.0_f32, p.line);
    v.menu_corner_radius = egui::CornerRadius::from(radius::CARD);
    v.text_cursor.stroke = Stroke::new(2.0_f32, ink);
    v.striped = false;
    let round = egui::CornerRadius::from(radius::CONTROL);
    let base = match ui.theme {
        Theme::Omarchy => mix(p.raise, p.fg, OMARCHY_HOVER_BASE_TINT),
        _ if dark => mix(p.raise, p.fg, DARK_HOVER_BASE_TINT),
        _ => Color32::from_rgb(242, 245, 250),
    };
    let hover_fill = mix(base, accent, HOVER_TINT);
    // `open` é o combo com a lista aberta: sem ele o egui pinta o cinza padrão no meio do preto.
    for (w, fill) in [
        (&mut v.widgets.noninteractive, p.panel),
        (&mut v.widgets.inactive, p.raise),
        (&mut v.widgets.hovered, hover_fill),
        (&mut v.widgets.active, hover_fill),
        (&mut v.widgets.open, hover_fill),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0_f32, p.line);
        w.corner_radius = round;
        w.fg_stroke = Stroke::new(1.0_f32, p.fg);
        w.expansion = 0.0;
    }
    // Separadores e filetes de painel: o tom mais baixo da paleta.
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.muted);
    // A trilha do slider usa `bg_fill`; com o mesmo cinza do painel ela some.
    v.widgets.inactive.bg_fill = mix(p.raise, p.fg, RAIL_TINT);
    // Controles em repouso precisam de borda com 3:1 (WCAG 1.4.11); `line` fica para separadores.
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.control);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ink);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ink);
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, ink);
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.selection.bg_fill = accent;
    v.selection.stroke = Stroke::new(1.0_f32, readable_on(accent));
    if ui.theme == Theme::HighContrast {
        // Bordas e cursor grossos: nenhum controle depende de um filete de 1 px para ser achado.
        let w = HIGH_CONTRAST_STROKE;
        v.widgets.inactive.bg_stroke = Stroke::new(w, p.control);
        v.widgets.hovered.bg_stroke = Stroke::new(w, accent);
        v.widgets.active.bg_stroke = Stroke::new(w, accent);
        v.widgets.open.bg_stroke = Stroke::new(w, accent);
        v.window_stroke = Stroke::new(w, p.line);
        v.text_cursor.stroke = Stroke::new(w + 1.0, accent);
    }
    v.window_highlight_topmost = false;
    v.hyperlink_color = ink;
    ctx.set_visuals(v);
    // Tamanho da interface (WCAG 1.4.4): texto e alvos de clique crescem juntos.
    ctx.set_zoom_factor(f32::from(ui.ui_scale.clamp(50, 300)) / 100.0);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(space::SM + 2.0, space::SM + 2.0);
        s.spacing.button_padding = egui::vec2(space::MD + 2.0, 7.0);
        s.spacing.interact_size.y = TARGET_MIN;
        s.spacing.menu_margin = egui::Margin::same(space::SM as i8);
        s.spacing.window_margin = egui::Margin::same(space::LG as i8);
        s.spacing.combo_width = 0.0;
        s.spacing.icon_width = text::BODY + 2.0;
        s.spacing.icon_width_inner = text::BODY - 4.0;
        s.spacing.slider_rail_height = 4.0;
        // A barra flutuante some quando o ponteiro sai; com movimento reduzido ou alto contraste
        // ela fica sempre à vista (achar a barra não pode depender do hover).
        let mut scroll = if ui.reduce_motion || ui.theme == Theme::HighContrast {
            egui::style::ScrollStyle::solid()
        } else {
            egui::style::ScrollStyle::floating()
        };
        scroll.bar_width = if ui.theme == Theme::HighContrast {
            SCROLL_BAR_WIDTH * 2.0
        } else {
            SCROLL_BAR_WIDTH
        };
        s.spacing.scroll = scroll;
        s.interaction.selectable_labels = false;
        s.interaction.tooltip_delay = TOOLTIP_DELAY;
        s.interaction.tooltip_grace_time = TOOLTIP_GRACE;
        // Abrir/fechar seções, rolagem e realces seguem o mesmo ritmo dos controles desenhados à mão.
        // Movimento reduzido (WCAG 2.3.3): sem animação de abrir/fechar em lugar nenhum.
        s.animation_time = if ui.reduce_motion { 0.0 } else { motion::BASE };
        s.visuals.slider_trailing_fill = true;
        s.text_styles
            .insert(TextStyle::Heading, display_font(text::DISPLAY));
        s.text_styles
            .insert(TextStyle::Body, FontId::proportional(text::BODY));
        s.text_styles
            .insert(TextStyle::Button, FontId::proportional(text::BODY));
        s.text_styles
            .insert(TextStyle::Small, FontId::proportional(text::CAPTION));
        s.text_styles
            .insert(TextStyle::Monospace, FontId::monospace(text::SMALL));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_color_contrasts_with_the_background() {
        assert_eq!(readable_on(Color32::WHITE), Color32::BLACK);
        assert_eq!(readable_on(Color32::from_rgb(250, 220, 60)), Color32::BLACK);
        assert_eq!(readable_on(Color32::from_rgb(20, 30, 90)), Color32::WHITE);
    }

    fn rgb(c: [u8; 3]) -> Color32 {
        Color32::from_rgb(c[0], c[1], c[2])
    }

    #[test]
    fn text_on_every_suggested_accent_meets_aa() {
        for (name, c) in ACCENTS {
            let bg = rgb(c);
            let ratio = contrast(readable_on(bg), bg);
            assert!(ratio >= TEXT_MIN_CONTRAST, "{name}: {ratio:.2}:1");
        }
    }

    #[test]
    fn text_on_any_accent_the_user_can_pick_meets_aa() {
        const STEP: usize = 15;
        for r in (0..=255).step_by(STEP) {
            for g in (0..=255).step_by(STEP) {
                for b in (0..=255).step_by(STEP) {
                    let bg = Color32::from_rgb(r as u8, g as u8, b as u8);
                    let ratio = contrast(readable_on(bg), bg);
                    assert!(ratio >= TEXT_MIN_CONTRAST, "{bg:?}: {ratio:.2}:1");
                }
            }
        }
    }

    #[test]
    fn accent_ink_is_visible_on_every_surface_of_both_themes() {
        for t in [Theme::Dark, Theme::Light] {
            let p = palette(t);
            for (name, c) in ACCENTS {
                let ink = ink(rgb(c), &p);
                for (surface, bg) in [("panel", p.panel), ("raise", p.raise)] {
                    let ratio = contrast(ink, bg);
                    assert!(
                        ratio >= COMPONENT_MIN_CONTRAST,
                        "{name} sobre {surface} ({t:?}): {ratio:.2}:1"
                    );
                }
            }
        }
    }

    #[test]
    fn dark_theme_accents_are_not_altered_by_ink() {
        let p = palette(Theme::Dark);
        for (_, c) in ACCENTS {
            assert_eq!(ink(rgb(c), &p), rgb(c));
        }
    }

    #[test]
    fn palettes_meet_wcag_for_text_and_controls() {
        for t in [Theme::Dark, Theme::Light, Theme::HighContrast] {
            let p = palette(t);
            for (name, fg) in [
                ("fg", p.fg),
                ("muted", p.muted),
                ("ok", p.ok),
                ("bad", p.bad),
            ] {
                for (surface, bg) in [("panel", p.panel), ("raise", p.raise), ("bg", p.bg)] {
                    let ratio = contrast(fg, bg);
                    // AAA (1.4.6): 7:1 em todo texto, inclusive o secundário e os de estado.
                    assert!(
                        ratio >= TEXT_AAA_CONTRAST,
                        "{name} sobre {surface} ({t:?}): {ratio:.2}:1"
                    );
                }
            }
            for (surface, bg) in [("panel", p.panel), ("raise", p.raise)] {
                let ratio = contrast(p.control, bg);
                assert!(
                    ratio >= COMPONENT_MIN_CONTRAST,
                    "borda de controle sobre {surface} ({t:?}): {ratio:.2}:1"
                );
            }
            let ratio = contrast(p.device_fg, p.keyoff);
            assert!(
                ratio >= TEXT_MIN_CONTRAST,
                "símbolos na tecla ({t:?}): {ratio:.2}:1"
            );
            // O "+" apagado de uma tecla vazia em repouso (sobre o preto, o premultiplicado é a cor).
            let idle = p
                .device_fg
                .gamma_multiply(crate::editor::widgets::EMPTY_PLUS_IDLE);
            let idle = Color32::from_rgb(idle.r(), idle.g(), idle.b());
            let ratio = contrast(idle, p.keyoff);
            assert!(
                ratio >= COMPONENT_MIN_CONTRAST,
                "\"+\" em repouso ({t:?}): {ratio:.2}:1"
            );
        }
    }

    #[test]
    fn ensure_contrast_keeps_good_colors_and_fixes_bad_ones() {
        let white = Color32::WHITE;
        let cyan = Color32::from_rgb(0, 229, 255);
        assert_eq!(ensure_contrast(Color32::BLACK, white, 4.5), Color32::BLACK);
        let fixed = ensure_contrast(cyan, white, COMPONENT_MIN_CONTRAST);
        assert_ne!(fixed, cyan);
        assert!(contrast(fixed, white) >= COMPONENT_MIN_CONTRAST);
    }

    #[test]
    fn apply_sets_base_theme_and_accent() {
        let ctx = Context::default();
        let mut s = UiSettings {
            theme: Theme::Light,
            accent: [10, 200, 30],
            ..UiSettings::default()
        };
        apply(&ctx, &s);
        let v = ctx.global_style().visuals.clone();
        assert!(!v.dark_mode);
        assert_eq!(v.selection.bg_fill, Color32::from_rgb(10, 200, 30));
        s.theme = Theme::Dark;
        apply(&ctx, &s);
        assert!(ctx.global_style().visuals.dark_mode);
    }

    #[test]
    fn palettes_keep_text_readable_on_panels() {
        for t in [Theme::Dark, Theme::Light] {
            let p = palette(t);
            assert_ne!(readable_on(p.panel), p.panel);
            let lum = |c: Color32| c.r() as i32 + c.g() as i32 + c.b() as i32;
            assert!((lum(p.fg) - lum(p.panel)).abs() > 300);
        }
    }
}
