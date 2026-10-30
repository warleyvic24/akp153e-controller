use crate::config::schema::{ButtonConfig, RGBAColor};
use ab_glyph::{point, Font, FontArc, PxScale, ScaleFont};
use image::{imageops, Rgba, RgbaImage};
use std::sync::OnceLock;

/// Fontes do Windows primeiro; depois as mais comuns nas distribuições Linux.
const FONT_FILES: [&str; 6] = [
    "segoeui.ttf",
    "arial.ttf",
    "tahoma.ttf",
    "DejaVuSans.ttf",
    "LiberationSans-Regular.ttf",
    "NotoSans-Regular.ttf",
];
const BOLD_FONT_FILES: [&str; 5] = [
    "segoeuib.ttf",
    "arialbd.ttf",
    "DejaVuSans-Bold.ttf",
    "LiberationSans-Bold.ttf",
    "NotoSans-Bold.ttf",
];

/// Pastas onde procurar as fontes.
#[cfg(windows)]
fn font_dirs() -> Vec<std::path::PathBuf> {
    // Mesmo que o sistema não esteja em `C:`.
    let windows = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
    vec![std::path::Path::new(&windows).join("Fonts")]
}

#[cfg(not(windows))]
fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = [
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/truetype/liberation",
        "/usr/share/fonts/truetype/noto",
        "/usr/share/fonts/dejavu",
        "/usr/share/fonts/liberation",
        "/usr/share/fonts/noto",
        "/usr/share/fonts/TTF",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .collect();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(std::path::Path::new(&home).join(".local/share/fonts"));
    }
    dirs
}

const MIN_FONT_PX: f32 = 6.0;
const JPEG_QUALITY: u8 = 90;
const ICON_STEP_PX: f32 = 0.35;

/// A primeira fonte da lista que existir; no Linux, se nenhuma estiver nas pastas conhecidas, a
/// que o `fc-match` escolher para `fallback` (cada distribuição guarda as fontes num lugar).
fn load_font(
    files: &[&str],
    #[cfg_attr(windows, allow(unused_variables))] fallback: &str,
) -> Option<FontArc> {
    let read = |path: std::path::PathBuf| {
        std::fs::read(path)
            .ok()
            .and_then(|d| FontArc::try_from_vec(d).ok())
    };
    let dirs = font_dirs();
    let found = files
        .iter()
        .find_map(|f| dirs.iter().find_map(|dir| read(dir.join(f))));
    #[cfg(not(windows))]
    let found = found.or_else(|| read(crate::platform::fc_match_file(fallback)?));
    found
}

fn font() -> Option<&'static FontArc> {
    static FONT: OnceLock<Option<FontArc>> = OnceLock::new();
    FONT.get_or_init(|| {
        load_font(&FONT_FILES, "sans-serif").or_else(|| {
            log::warn!("nenhuma fonte do sistema encontrada; texto não será renderizado");
            None
        })
    })
    .as_ref()
}

/// Fonte em negrito; sem ela, cai na normal.
pub(super) fn bold_font() -> Option<&'static FontArc> {
    static FONT: OnceLock<Option<FontArc>> = OnceLock::new();
    FONT.get_or_init(|| load_font(&BOLD_FONT_FILES, "sans-serif:bold"))
        .as_ref()
        .or_else(font)
}

pub(super) fn blend(dst: &mut Rgba<u8>, src: RGBAColor, coverage: f32) {
    let a = coverage * src.a as f32 / 255.0;
    for (d, s) in dst.0.iter_mut().zip([src.r, src.g, src.b]) {
        *d = (*d as f32 * (1.0 - a) + s as f32 * a).round() as u8;
    }
}

pub(super) fn text_width(font: &FontArc, scale: PxScale, text: &str) -> f32 {
    let s = font.as_scaled(scale);
    text.chars().map(|c| s.h_advance(s.glyph_id(c))).sum()
}

fn draw_text(img: &mut RgbaImage, text: &str, size: f32, color: RGBAColor, bottom_aligned: bool) {
    let Some(font) = font() else { return };
    let (w, h) = img.dimensions();
    let mut px = size.max(MIN_FONT_PX);
    while px > MIN_FONT_PX && text_width(font, PxScale::from(px), text) > w as f32 - 4.0 {
        px -= 1.0;
    }
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);
    let x0 = (w as f32 - text_width(font, scale, text)) / 2.0;
    let line_h = scaled.ascent() - scaled.descent();
    let top = if bottom_aligned {
        h as f32 - line_h - 2.0
    } else {
        (h as f32 - line_h) / 2.0
    };
    draw_glyphs(img, font, scale, text, (x0, top + scaled.ascent()), color);
}

/// Uma linha de `text` centrada na horizontal com o topo em `top`; encolhe (até `min_px`) para caber
/// e, se mesmo assim não couber, corta com reticências.
pub(super) fn draw_line(
    img: &mut RgbaImage,
    font: &FontArc,
    text: &str,
    (size, min_px): (f32, f32),
    color: RGBAColor,
    top: f32,
) {
    let width = img.width() as f32 - 4.0;
    let mut px = size;
    while px > min_px && text_width(font, PxScale::from(px), text) > width {
        px -= 1.0;
    }
    let scale = PxScale::from(px);
    let shown = if text_width(font, scale, text) <= width {
        text.to_string()
    } else {
        (1..text.chars().count())
            .rev()
            .map(|n| format!("{}…", text.chars().take(n).collect::<String>()))
            .find(|cut| text_width(font, scale, cut) <= width)
            .unwrap_or_else(|| "…".into())
    };
    let x0 = (img.width() as f32 - text_width(font, scale, &shown)) / 2.0;
    let baseline = top + font.as_scaled(scale).ascent();
    draw_glyphs(img, font, scale, &shown, (x0, baseline), color);
}

fn draw_glyphs(
    img: &mut RgbaImage,
    font: &FontArc,
    scale: PxScale,
    text: &str,
    (x0, baseline): (f32, f32),
    color: RGBAColor,
) {
    let scaled = font.as_scaled(scale);
    let (w, h) = img.dimensions();
    let mut x = x0;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        let glyph = id.with_scale_and_position(scale, point(x, baseline));
        x += scaled.h_advance(id);
        let Some(og) = font.outline_glyph(glyph) else {
            continue;
        };
        let b = og.px_bounds();
        og.draw(|gx, gy, cov| {
            let (ix, iy) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
            if ix >= 0 && iy >= 0 && (ix as u32) < w && (iy as u32) < h {
                blend(img.get_pixel_mut(ix as u32, iy as u32), color, cov);
            }
        });
    }
}

fn stamp(mask: &mut [f32], w: u32, h: u32, x: f32, y: f32, r: f32) {
    let lo = |v: f32, max: u32| (v.floor().max(0.0) as u32).min(max - 1);
    let hi = |v: f32, max: u32| (v.ceil().max(0.0) as u32).min(max - 1);
    for py in lo(y - r - 1.0, h)..=hi(y + r + 1.0, h) {
        for px in lo(x - r - 1.0, w)..=hi(x + r + 1.0, w) {
            let dist = (px as f32 + 0.5 - x).hypot(py as f32 + 0.5 - y);
            let cell = &mut mask[(py * w + px) as usize];
            *cell = cell.max((r + 0.5 - dist).clamp(0.0, 1.0));
        }
    }
}

/// Desenha um ícone vetorial centrado em (cx, cy) com lado `size`, com bordas suavizadas.
pub(super) fn draw_icon(
    img: &mut RgbaImage,
    name: &str,
    cx: f32,
    cy: f32,
    size: f32,
    color: RGBAColor,
) {
    let (w, h) = img.dimensions();
    let scale = size / 24.0;
    let radius = (scale * 1.9).max(1.4) / 2.0;
    let (ox, oy) = (cx - size / 2.0, cy - size / 2.0);
    let mut mask = vec![0.0_f32; (w * h) as usize];
    for line in crate::graphics::icons::strokes(name) {
        for seg in line.windows(2) {
            let (a, b) = (
                (ox + seg[0].0 * scale, oy + seg[0].1 * scale),
                (ox + seg[1].0 * scale, oy + seg[1].1 * scale),
            );
            let steps = ((b.0 - a.0).hypot(b.1 - a.1) / ICON_STEP_PX)
                .ceil()
                .max(1.0) as u32;
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                stamp(
                    &mut mask,
                    w,
                    h,
                    a.0 + (b.0 - a.0) * t,
                    a.1 + (b.1 - a.1) * t,
                    radius,
                );
            }
        }
    }
    for (i, coverage) in mask.iter().enumerate().filter(|(_, c)| **c > 0.0) {
        blend(
            img.get_pixel_mut(i as u32 % w, i as u32 / w),
            color,
            *coverage,
        );
    }
}

/// Renderiza o botão em RGBA (usado na prévia da UI e como base do frame de hardware).
pub fn render_rgba(button: &ButtonConfig, width: u32, height: u32) -> RgbaImage {
    let ap = button.current_appearance();
    let bg = ap.background;
    let mut img = RgbaImage::from_pixel(width, height, Rgba([bg.r, bg.g, bg.b, 255]));
    let mut has_icon = false;

    let builtin = ap
        .icon
        .as_deref()
        .filter(|n| !crate::graphics::icons::strokes(n).is_empty());
    if let Some(name) = builtin {
        let box_h = if ap.text.is_empty() {
            height as f32
        } else {
            height as f32 * 0.68
        };
        let size = (box_h * 0.62).min(width as f32 * 0.62);
        draw_icon(
            &mut img,
            name,
            width as f32 / 2.0,
            box_h / 2.0 + 1.0,
            size,
            ap.text_color,
        );
        has_icon = true;
    } else if let Some(path) = ap.icon_path.as_deref().filter(|p| !p.is_empty()) {
        match image::open(path) {
            Ok(icon) => {
                let box_h = if ap.text.is_empty() {
                    height
                } else {
                    height * 7 / 10
                };
                let icon = icon
                    .resize(
                        width * 8 / 10,
                        box_h * 8 / 10,
                        imageops::FilterType::Triangle,
                    )
                    .to_rgba8();
                let x = (width.saturating_sub(icon.width()) / 2) as i64;
                let y = (box_h.saturating_sub(icon.height()) / 2) as i64;
                if let Some(plate) = plate_for(&icon, bg) {
                    let pad = (icon.width().max(icon.height()) as f32 * PLATE_PAD) as i64;
                    let (w, h) = (
                        icon.width() as i64 + 2 * pad,
                        icon.height() as i64 + 2 * pad,
                    );
                    fill_rounded(&mut img, (x - pad, y - pad, w, h), plate);
                }
                imageops::overlay(&mut img, &icon, x, y);
                has_icon = true;
            }
            Err(e) => log::warn!("ícone inválido {path}: {e}"),
        }
    }
    if !ap.text.is_empty() {
        draw_text(&mut img, &ap.text, ap.font_size, ap.text_color, has_icon);
    }
    img
}

/// Contraste mínimo entre o ícone e o fundo da tecla; abaixo disto o ícone ganha uma placa.
const ICON_MIN_CONTRAST: f32 = 2.0;
/// Folga da placa em volta do ícone, em fração do lado do ícone.
const PLATE_PAD: f32 = 0.1;
/// Raio dos cantos da placa, em fração do lado dela.
const PLATE_RADIUS: f32 = 0.22;
/// Pixels com menos opacidade que isto não contam para a cor do ícone.
const OPAQUE_ALPHA: u8 = 128;

/// Luminância relativa (WCAG) de uma cor sRGB.
fn luminance(r: u8, g: u8, b: u8) -> f32 {
    let channel = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast(a: f32, b: f32) -> f32 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Um ícone de app ou site que some no fundo da tecla (o logo escuro do GitHub na tecla preta)
/// ganha uma placa: clara se o ícone for escuro, escura se for claro. `None` se já se destaca.
fn plate_for(icon: &RgbaImage, bg: RGBAColor) -> Option<RGBAColor> {
    let opaque: Vec<f32> = icon
        .pixels()
        .filter(|p| p.0[3] >= OPAQUE_ALPHA)
        .map(|p| luminance(p.0[0], p.0[1], p.0[2]))
        .collect();
    if opaque.is_empty() {
        return None;
    }
    let icon_lum = opaque.iter().sum::<f32>() / opaque.len() as f32;
    if contrast(icon_lum, luminance(bg.r, bg.g, bg.b)) >= ICON_MIN_CONTRAST {
        return None;
    }
    let (white, black) = (contrast(icon_lum, 1.0), contrast(icon_lum, 0.0));
    let shade = if white >= black { 255 } else { 0 };
    Some(RGBAColor {
        r: shade,
        g: shade,
        b: shade,
        a: 255,
    })
}

/// Retângulo de cantos arredondados (x, y, largura, altura), cortado nas bordas da imagem.
fn fill_rounded(img: &mut RgbaImage, (x, y, w, h): (i64, i64, i64, i64), color: RGBAColor) {
    let r = (w.min(h) as f32 * PLATE_RADIUS).max(1.0);
    for py in y.max(0)..(y + h).min(img.height() as i64) {
        for px in x.max(0)..(x + w).min(img.width() as i64) {
            // Distância até o retângulo encolhido pelo raio: dentro dele, ou a até `r` dele.
            let dx = (x as f32 + r - (px as f32 + 0.5)).max(px as f32 + 0.5 - (x + w) as f32 + r);
            let dy = (y as f32 + r - (py as f32 + 0.5)).max(py as f32 + 0.5 - (y + h) as f32 + r);
            let outside = dx.max(0.0).hypot(dy.max(0.0));
            let coverage = (r - outside + 0.5).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend(img.get_pixel_mut(px as u32, py as u32), color, coverage);
            }
        }
    }
}

/// Gera o JPEG enviado ao display: rotação e espelhamento exigidos pelo hardware (rot90 + espelho H/V).
pub fn render_button_frame(button: &ButtonConfig, width: u32, height: u32) -> Vec<u8> {
    encode_frame(&render_rgba(button, width, height))
}

/// Imagem de calibração: borda colorida por lado, régua de 10 em 10 px e cruz central.
/// Mostra no aparelho quanto de cada lado aparece (deslocamento e recorte por tecla).
pub fn calibration_rgba(size: u32) -> RgbaImage {
    const EDGE: u32 = 4;
    const TICK: u32 = 10;
    const TICK_LEN: u32 = 8;
    let white = Rgba([255, 255, 255, 255]);
    let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 255]));
    let last = size - 1;
    for i in 0..size {
        for d in 0..EDGE {
            img.put_pixel(i, d, Rgba([255, 0, 0, 255]));
            img.put_pixel(i, last - d, Rgba([0, 255, 0, 255]));
            img.put_pixel(d, i, Rgba([0, 90, 255, 255]));
            img.put_pixel(last - d, i, Rgba([255, 220, 0, 255]));
        }
    }
    for i in EDGE..size - EDGE {
        img.put_pixel(i, size / 2, white);
        img.put_pixel(size / 2, i, white);
        if i % TICK == 0 {
            for d in EDGE..EDGE + TICK_LEN {
                img.put_pixel(i, d, white);
                img.put_pixel(d, i, white);
            }
        }
    }
    img
}

/// Aplica a rotação do hardware e codifica em JPEG.
pub fn encode_frame(img: &RgbaImage) -> Vec<u8> {
    // rot90 seguido de espelho horizontal e vertical (= rot180) equivale a rot270.
    let img = imageops::rotate270(img);
    let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    match enc.encode(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    ) {
        Ok(()) => out,
        Err(e) => {
            log::error!("falha ao codificar JPEG: {e}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(r: u8, g: u8, b: u8) -> RgbaImage {
        RgbaImage::from_pixel(10, 10, Rgba([r, g, b, 255]))
    }

    const BLACK: RGBAColor = RGBAColor {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    #[test]
    fn a_dark_icon_on_a_black_key_gets_a_light_plate() {
        let github_dark = solid(0x24, 0x29, 0x2f);
        assert_eq!(plate_for(&github_dark, BLACK).map(|c| c.r), Some(255));
        let youtube_red = solid(0xff, 0x00, 0x33);
        assert!(plate_for(&youtube_red, BLACK).is_none());
        let white_bg = RGBAColor {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        assert_eq!(
            plate_for(&solid(250, 250, 250), white_bg).map(|c| c.r),
            Some(0)
        );
        let invisible = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        assert!(plate_for(&invisible, BLACK).is_none());
    }

    #[test]
    fn the_plate_is_rounded_and_clipped_to_the_image() {
        let mut img = solid(0, 0, 0);
        let white = RGBAColor {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        fill_rounded(&mut img, (-2, 1, 10, 8), white);
        assert_eq!(img.get_pixel(4, 4).0[0], 255, "o meio fica cheio");
        assert_eq!(img.get_pixel(0, 0).0[0], 0, "fora da placa");
        assert!(img.get_pixel(7, 1).0[0] < 255, "o canto é arredondado");
    }

    #[test]
    fn frame_is_a_valid_jpeg_under_64k() {
        let f = render_button_frame(&ButtonConfig::blank(), 85, 85);
        assert_eq!(&f[..2], &[0xFF, 0xD8]);
        assert!(f.len() < 65_535);
        let img = image::load_from_memory(&f).unwrap();
        assert_eq!((img.width(), img.height()), (85, 85));
    }

    fn blank_pixels() -> RgbaImage {
        render_rgba(&ButtonConfig::blank(), 40, 40)
    }

    #[test]
    fn text_changes_pixels_and_stays_inside_the_frame() {
        let mut b = ButtonConfig::blank();
        b.appearance.text = "Play".into();
        let with_text = render_rgba(&b, 40, 40);
        assert_eq!(with_text.dimensions(), (40, 40));
        assert_ne!(with_text.as_raw(), blank_pixels().as_raw());
    }

    #[test]
    fn long_text_is_shrunk_to_fit() {
        let mut b = ButtonConfig::blank();
        b.appearance.text = "Texto muito comprido para caber".into();
        b.appearance.font_size = 40.0;
        let img = render_rgba(&b, 40, 40);
        assert_ne!(img.as_raw(), blank_pixels().as_raw());
    }

    #[test]
    fn icon_is_drawn_and_missing_icon_is_ignored() {
        let path = std::env::temp_dir().join(format!("akp_icon_{}.png", std::process::id()));
        RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let mut b = ButtonConfig::blank();
        b.appearance.icon_path = Some(path.display().to_string());
        let center = render_rgba(&b, 40, 40).get_pixel(20, 20).0;
        let _ = std::fs::remove_file(&path);
        assert_eq!(center[0], 255);
        b.appearance.icon_path = Some("nao_existe.png".into());
        assert_eq!(render_rgba(&b, 40, 40).as_raw(), blank_pixels().as_raw());
    }

    #[test]
    fn toggled_state_uses_the_toggled_appearance() {
        let mut b = ButtonConfig::blank();
        let mut lit = b.appearance.clone();
        lit.background = RGBAColor {
            r: 9,
            g: 99,
            b: 199,
            a: 255,
        };
        b.toggled_appearance = Some(lit);
        b.toggle_state = true;
        assert_eq!(render_rgba(&b, 10, 10).get_pixel(1, 1).0, [9, 99, 199, 255]);
    }

    #[test]
    fn builtin_icon_is_drawn_in_the_text_color() {
        let mut b = ButtonConfig::blank();
        b.appearance.background = RGBAColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        b.appearance.text_color = RGBAColor {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        b.appearance.icon = Some("play".into());
        let lit = render_rgba(&b, 60, 60)
            .pixels()
            .filter(|p| p.0[0] > 128)
            .count();
        assert!(lit > 40, "poucos pixels acesos: {lit}");
        b.appearance.icon = Some("nao-existe".into());
        assert_eq!(
            render_rgba(&b, 60, 60)
                .pixels()
                .filter(|p| p.0[0] > 128)
                .count(),
            0
        );
    }

    #[test]
    fn icon_and_text_fit_together_in_a_small_frame() {
        let mut b = ButtonConfig::blank();
        b.appearance.icon = Some("volup".into());
        b.appearance.text = "Vol +".into();
        assert_eq!(render_button_frame(&b, 85, 85)[..2], [0xFF, 0xD8]);
    }

    #[test]
    fn background_color_is_rendered() {
        let mut b = ButtonConfig::blank();
        b.appearance.background = RGBAColor {
            r: 200,
            g: 10,
            b: 10,
            a: 255,
        };
        let px = render_rgba(&b, 10, 10).get_pixel(5, 5).0;
        assert_eq!(px, [200, 10, 10, 255]);
    }
}
