use crate::config::schema::{ButtonConfig, RGBAColor};
use ab_glyph::{point, Font, FontArc, PxScale, ScaleFont};
use image::{imageops, Rgba, RgbaImage};
use std::sync::OnceLock;

const FONT_FILES: [&str; 3] = ["segoeui.ttf", "arial.ttf", "tahoma.ttf"];

/// Pasta de fontes do Windows, mesmo que o sistema não esteja em `C:`.
fn font_dir() -> std::path::PathBuf {
    let windows = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
    std::path::Path::new(&windows).join("Fonts")
}
const MIN_FONT_PX: f32 = 6.0;
const JPEG_QUALITY: u8 = 90;
const ICON_STEP_PX: f32 = 0.35;

fn font() -> Option<&'static FontArc> {
    static FONT: OnceLock<Option<FontArc>> = OnceLock::new();
    FONT.get_or_init(|| {
        let dir = font_dir();
        FONT_FILES
            .iter()
            .find_map(|f| {
                std::fs::read(dir.join(f))
                    .ok()
                    .and_then(|d| FontArc::try_from_vec(d).ok())
            })
            .or_else(|| {
                log::warn!("nenhuma fonte do sistema encontrada; texto não será renderizado");
                None
            })
    })
    .as_ref()
}

fn blend(dst: &mut Rgba<u8>, src: RGBAColor, coverage: f32) {
    let a = coverage * src.a as f32 / 255.0;
    for (d, s) in dst.0.iter_mut().zip([src.r, src.g, src.b]) {
        *d = (*d as f32 * (1.0 - a) + s as f32 * a).round() as u8;
    }
}

fn text_width(font: &FontArc, scale: PxScale, text: &str) -> f32 {
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
    let baseline = top + scaled.ascent();
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
fn draw_icon(img: &mut RgbaImage, name: &str, cx: f32, cy: f32, size: f32, color: RGBAColor) {
    let (w, h) = img.dimensions();
    let scale = size / 24.0;
    let radius = (scale * 1.9).max(1.4) / 2.0;
    let (ox, oy) = (cx - size / 2.0, cy - size / 2.0);
    let mut mask = vec![0.0_f32; (w * h) as usize];
    for line in crate::icons::strokes(name) {
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
        .filter(|n| !crate::icons::strokes(n).is_empty());
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
