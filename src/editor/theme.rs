//! Aparência da interface: paleta escura/clara, cor de destaque do usuário, fontes do Windows e estilo dos widgets.
use crate::config::schema::{Theme, UiSettings};
use crate::editor::omarchy;
#[cfg(test)]
use crate::editor::tokens::TEXT_MIN_CONTRAST;
use crate::editor::tokens::{radius, space, text, COMPONENT_MIN_CONTRAST};
use eframe::egui::{
    self, vec2, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};

/// Cores de destaque sugeridas (o usuário também pode escolher qualquer outra).
pub const ACCENTS: [(&str, [u8; 3]); 6] = [
    ("Ciano", [0, 229, 255]),
    ("Magenta", [255, 43, 214]),
    ("Verde-ácido", [57, 255, 20]),
    ("Roxo", [176, 110, 255]),
    ("Laranja", [255, 140, 40]),
    ("Azul", [70, 130, 255]),
];

const HOVER_TINT: f32 = 0.16;
const SHADOW_OFFSET_Y: f32 = 8.0;
const CONTRAST_STEPS: u8 = 20;
pub const DEVICE_FG: [u8; 3] = [176, 184, 210];
const RAIL_TINT: f32 = 0.2;
/// Base do hover quando o tema vem do Omarchy: um passo da superfície rumo ao texto.
const OMARCHY_HOVER_BASE_TINT: f32 = 0.1;
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
            bg: c(7, 8, 16),
            panel: c(13, 15, 28),
            raise: c(19, 22, 40),
            field: c(10, 12, 22),
            line: c(40, 46, 84),
            fg: c(234, 238, 250),
            muted: c(140, 150, 185),
            device: c(4, 5, 12),
            bezel: c(22, 26, 48),
            keyoff: c(0, 0, 0),
            ok: c(52, 211, 153),
            bad: c(248, 113, 113),
            control: Color32::TRANSPARENT,
            device_fg,
        },
        Theme::Light => Palette {
            bg: c(233, 237, 243),
            panel: c(255, 255, 255),
            raise: c(244, 246, 250),
            field: c(241, 244, 248),
            line: c(213, 219, 229),
            fg: c(22, 27, 38),
            muted: c(95, 104, 120),
            device: c(20, 23, 29),
            bezel: c(42, 47, 58),
            keyoff: c(0, 0, 0),
            ok: c(16, 110, 74),
            bad: c(185, 40, 55),
            control: Color32::TRANSPARENT,
            device_fg,
        },
    }
}

/// A cor de destaque: a escolhida pelo usuário, ou a do tema quando ele vem do Omarchy.
pub fn accent_color(ui: &UiSettings) -> Color32 {
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
    ]
    .map(|(name, path)| (name, path.into()))
    .into()
}

/// Fontes do Linux: o `fc-match` diz qual arquivo o sistema usa para sans-serif e monospace (no
/// Omarchy, as fontes que o usuário escolheu). Sem fonte de títulos, eles usam a de texto.
#[cfg(not(windows))]
fn system_fonts() -> Vec<(&'static str, std::path::PathBuf)> {
    [("ui", "sans-serif"), ("mono", "monospace")]
        .into_iter()
        .filter_map(|(name, family)| Some((name, fc_match_file(family)?)))
        .collect()
}

/// Arquivo `.ttf` ou `.otf` da fonte da família; coleções (`.ttc`) e erros dão `None`.
#[cfg(not(windows))]
fn fc_match_file(family: &str) -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", family])
        .output()
        .ok()?;
    let path = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    (ext == "ttf" || ext == "otf").then_some(path)
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
        fonts
            .font_data
            .insert(name.into(), FontData::from_owned(bytes));
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
    v.window_rounding = egui::Rounding::same(radius::CARD);
    v.window_stroke = Stroke::new(1.0_f32, p.control);
    let shadow = egui::epaint::Shadow {
        offset: vec2(0.0, SHADOW_OFFSET_Y),
        blur: 24.0,
        spread: 0.0,
        color: Color32::from_black_alpha(if dark { 150 } else { 45 }),
    };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;
    v.menu_rounding = egui::Rounding::same(radius::CARD);
    let round = egui::Rounding::same(radius::CONTROL);
    let base = match ui.theme {
        Theme::Omarchy => mix(p.raise, p.fg, OMARCHY_HOVER_BASE_TINT),
        _ if dark => Color32::from_rgb(32, 37, 58),
        _ => Color32::from_rgb(232, 237, 245),
    };
    let hover_fill = mix(base, accent, HOVER_TINT);
    for (w, fill) in [
        (&mut v.widgets.noninteractive, p.panel),
        (&mut v.widgets.inactive, p.raise),
        (&mut v.widgets.hovered, hover_fill),
        (&mut v.widgets.active, hover_fill),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0_f32, p.line);
        w.rounding = round;
        w.fg_stroke = Stroke::new(1.0_f32, p.fg);
    }
    // A trilha do slider usa `bg_fill`; com o mesmo cinza do painel ela some.
    v.widgets.inactive.bg_fill = mix(p.raise, p.fg, RAIL_TINT);
    // Controles em repouso precisam de borda com 3:1 (WCAG 1.4.11); `line` fica para separadores.
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.control);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ink);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ink);
    v.selection.bg_fill = accent;
    v.selection.stroke = Stroke::new(1.0_f32, readable_on(accent));
    v.hyperlink_color = ink;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(space::SM, space::SM);
        s.spacing.button_padding = egui::vec2(space::MD, 6.0);
        s.spacing.interact_size.y = 28.0;
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
            .insert(TextStyle::Monospace, FontId::monospace(text::CAPTION));
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
        for t in [Theme::Dark, Theme::Light] {
            let p = palette(t);
            for (name, fg) in [
                ("fg", p.fg),
                ("muted", p.muted),
                ("ok", p.ok),
                ("bad", p.bad),
            ] {
                for (surface, bg) in [("panel", p.panel), ("raise", p.raise), ("bg", p.bg)] {
                    let ratio = contrast(fg, bg);
                    assert!(
                        ratio >= TEXT_MIN_CONTRAST,
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
        let v = ctx.style().visuals.clone();
        assert!(!v.dark_mode);
        assert_eq!(v.selection.bg_fill, Color32::from_rgb(10, 200, 30));
        s.theme = Theme::Dark;
        apply(&ctx, &s);
        assert!(ctx.style().visuals.dark_mode);
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
