//! Aparência da interface: paleta escura/clara, cor de destaque do usuário, fontes do Windows e estilo dos widgets.
use crate::config::schema::{Theme, UiSettings};
use eframe::egui::{
    self, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
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

const LUMINANCE_THRESHOLD: f32 = 150.0;
const HOVER_TINT: f32 = 0.16;
const RAIL_TINT: f32 = 0.2;
const FONT_FILES: [(&str, &str); 3] = [
    ("ui", r"C:\Windows\Fonts\segoeui.ttf"),
    ("mono", r"C:\Windows\Fonts\consola.ttf"),
    ("display", r"C:\Windows\Fonts\bahnschrift.ttf"),
];
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
}

pub fn palette(theme: Theme) -> Palette {
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    match theme {
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
            ok: c(22, 132, 90),
            bad: c(201, 58, 71),
        },
    }
}

pub fn accent_color(ui: &UiSettings) -> Color32 {
    Color32::from_rgb(ui.accent[0], ui.accent[1], ui.accent[2])
}

/// Branco ou preto, o que ficar mais legível sobre `bg`.
pub fn readable_on(bg: Color32) -> Color32 {
    let luminance = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if luminance > LUMINANCE_THRESHOLD {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Mistura `a` com `b`; `t` é a parte de `b` (0 a 1).
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

pub fn display_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DISPLAY_FAMILY.into()))
}

/// Carrega Segoe UI, Consolas e Bahnschrift do Windows; o que faltar cai nas fontes padrão do egui.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for (name, path) in FONT_FILES {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.into(), FontData::from_owned(bytes));
        }
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
    let mut v = if ui.theme == Theme::Dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.panel;
    v.window_fill = p.raise;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.raise;
    v.override_text_color = Some(p.fg);
    v.window_rounding = egui::Rounding::same(12.0);
    v.window_stroke = Stroke::new(1.0_f32, p.line);
    let round = egui::Rounding::same(8.0);
    let base = if ui.theme == Theme::Dark {
        Color32::from_rgb(32, 37, 58)
    } else {
        Color32::from_rgb(232, 237, 245)
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
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent);
    v.selection.bg_fill = accent;
    v.selection.stroke = Stroke::new(1.0_f32, readable_on(accent));
    v.hyperlink_color = accent;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 28.0;
        s.visuals.slider_trailing_fill = true;
        s.text_styles.insert(TextStyle::Heading, display_font(22.0));
        s.text_styles
            .insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles
            .insert(TextStyle::Button, FontId::proportional(14.0));
        s.text_styles
            .insert(TextStyle::Small, FontId::proportional(11.5));
        s.text_styles
            .insert(TextStyle::Monospace, FontId::monospace(12.0));
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

    #[test]
    fn every_suggested_accent_has_readable_selection_text() {
        for (_, c) in ACCENTS {
            let bg = Color32::from_rgb(c[0], c[1], c[2]);
            let l_bg = bg.r() as f32 * 0.299 + bg.g() as f32 * 0.587 + bg.b() as f32 * 0.114;
            let l_fg = readable_on(bg).r() as f32;
            assert!((l_bg - l_fg).abs() > 60.0, "contraste baixo em {c:?}");
        }
    }

    #[test]
    fn apply_sets_base_theme_and_accent() {
        let ctx = Context::default();
        let mut s = UiSettings {
            theme: Theme::Light,
            accent: [10, 200, 30],
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
