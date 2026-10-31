use crate::config::schema::{ButtonConfig, RGBAColor, TextPosition};
use crate::graphics::icon_library;
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
/// Linhas que cabem numa tecla de 100 px com fonte legível.
const MAX_TEXT_LINES: usize = 3;
/// Folga entre o rótulo e a borda da tecla (topo ou base).
const TEXT_EDGE: f32 = 2.0;
/// Sombra do texto sobre ícone ou imagem.
const SHADOW: RGBAColor = RGBAColor {
    r: 0,
    g: 0,
    b: 0,
    a: 200,
};
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

/// Rótulo da tecla: até `MAX_TEXT_LINES` linhas (quebra com Enter), cada uma centrada e encolhida
/// junto com as outras até a mais larga caber; o bloco vai ao topo, ao meio ou embaixo.
fn draw_text(
    img: &mut RgbaImage,
    text: &str,
    size: f32,
    color: RGBAColor,
    place: TextPosition,
    shadow: bool,
) {
    let Some(font) = font() else { return };
    let (w, h) = img.dimensions();
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(MAX_TEXT_LINES)
        .collect();
    if lines.is_empty() {
        return;
    }
    let widest = |px: f32| {
        lines
            .iter()
            .map(|l| text_width(font, PxScale::from(px), l))
            .fold(0.0_f32, f32::max)
    };
    let mut px = size.max(MIN_FONT_PX);
    while px > MIN_FONT_PX && widest(px) > w as f32 - 4.0 {
        px -= 1.0;
    }
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);
    let line_h = scaled.ascent() - scaled.descent();
    let block_h = line_h * lines.len() as f32;
    let top = match place {
        TextPosition::Top => TEXT_EDGE,
        TextPosition::Bottom => h as f32 - block_h - TEXT_EDGE,
        _ => (h as f32 - block_h) / 2.0,
    };
    for (i, line) in lines.iter().enumerate() {
        let x0 = (w as f32 - text_width(font, scale, line)) / 2.0;
        let baseline = top + line_h * i as f32 + scaled.ascent();
        if shadow {
            draw_glyphs(img, font, scale, line, (x0 + 1.0, baseline + 1.0), SHADOW);
        }
        draw_glyphs(img, font, scale, line, (x0, baseline), color);
    }
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

/// Fração da altura da tecla que o ícone ocupa quando divide o espaço com o rótulo.
const ICON_SHARE_WITH_TEXT: f32 = 0.68;
/// Lado do ícone vetorial em relação à faixa reservada para ele.
const VECTOR_ICON_FILL: f32 = 0.62;
/// Lado da imagem (PNG/JPG) em relação à faixa reservada para ela.
const IMAGE_ICON_FILL: f32 = 0.8;

/// Faixa vertical (topo, altura) reservada ao ícone, conforme a posição do rótulo.
fn icon_band(position: TextPosition, has_text: bool, height: f32) -> (f32, f32) {
    if !has_text || position == TextPosition::Middle {
        return (0.0, height);
    }
    let band = height * ICON_SHARE_WITH_TEXT;
    match position {
        TextPosition::Top => (height - band, band),
        _ => (0.0, band),
    }
}

/// Posição efetiva do rótulo: `Auto` vira embaixo com ícone e meio sem ele.
fn text_place(position: TextPosition, has_icon: bool) -> TextPosition {
    match position {
        TextPosition::Auto if has_icon => TextPosition::Bottom,
        TextPosition::Auto => TextPosition::Middle,
        other => other,
    }
}

/// Hora (grande) e data (pequena) da tecla de relógio, em fração da altura da tecla.
const CLOCK_TIME_PX: f32 = 0.34;
const CLOCK_DATE_PX: f32 = 0.16;
const CLOCK_TIME_TOP: f32 = 0.24;
const CLOCK_DATE_TOP: f32 = 0.62;

/// Texto da tecla de relógio para `now`: ("14:05", "sáb 31/10").
pub fn clock_text(now: &jiff::Zoned) -> (String, String) {
    const WEEKDAYS: [&str; 7] = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"];
    const WEEKDAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let day = usize::from(now.weekday().to_monday_zero_offset() as u8);
    let english = crate::strings::language() == crate::config::schema::Language::En;
    let (name, date) = if english {
        (WEEKDAYS_EN[day], now.strftime("%m/%d").to_string())
    } else {
        (WEEKDAYS[day], now.strftime("%d/%m").to_string())
    };
    (now.strftime("%H:%M").to_string(), format!("{name} {date}"))
}

/// Tecla de relógio: a hora agora e a data, na cor do texto da tecla.
fn draw_clock(img: &mut RgbaImage, color: RGBAColor) {
    let Some(font) = font() else { return };
    let h = img.height() as f32;
    let (time, date) = clock_text(&jiff::Zoned::now());
    let time_px = h * CLOCK_TIME_PX;
    let date_px = h * CLOCK_DATE_PX;
    let bold = bold_font().unwrap_or(font);
    draw_line(
        img,
        bold,
        &time,
        (time_px, MIN_FONT_PX),
        color,
        h * CLOCK_TIME_TOP,
    );
    draw_line(
        img,
        font,
        &date,
        (date_px, MIN_FONT_PX),
        color,
        h * CLOCK_DATE_TOP,
    );
}

/// Renderiza o botão em RGBA (usado na prévia da UI e como base do frame de hardware).
pub fn render_rgba(button: &ButtonConfig, width: u32, height: u32) -> RgbaImage {
    let ap = button.current_appearance();
    let bg = ap.background;
    let mut img = RgbaImage::from_pixel(width, height, Rgba([bg.r, bg.g, bg.b, 255]));
    // Tecla viva: o relógio ocupa a tecla toda (rótulo e ícone ficam de fora).
    if button.action == crate::config::schema::ButtonAction::Clock {
        draw_clock(&mut img, ap.text_color);
        return img;
    }
    let has_text = !ap.text.trim().is_empty();
    let (band_top, band_h) = icon_band(ap.text_position, has_text, height as f32);
    let center_x = width as f32 / 2.0;
    let center_y = band_top + band_h / 2.0 + 1.0;
    let vector_size = (band_h * VECTOR_ICON_FILL).min(width as f32 * VECTOR_ICON_FILL);

    let builtin = ap
        .icon
        .as_deref()
        .filter(|n| !crate::graphics::icons::strokes(n).is_empty());
    let library = ap
        .icon
        .as_deref()
        .filter(|id| id.starts_with(icon_library::PREFIX));
    let mut has_icon = false;
    if let Some(id) = library {
        let size = vector_size.round() as u32;
        if let Some(icon) = icon_library::render(id, size, ap.text_color) {
            let x = (center_x - size as f32 / 2.0).round() as i64;
            let y = (center_y - size as f32 / 2.0).round() as i64;
            imageops::overlay(&mut img, &icon, x, y);
            has_icon = true;
        }
    } else if let Some(name) = builtin {
        draw_icon(
            &mut img,
            name,
            center_x,
            center_y,
            vector_size,
            ap.text_color,
        );
        has_icon = true;
    } else if let Some(path) = ap.icon_path.as_deref().filter(|p| !p.is_empty()) {
        match image::open(path) {
            Ok(icon) => {
                let icon = icon
                    .resize(
                        (width as f32 * IMAGE_ICON_FILL) as u32,
                        (band_h * IMAGE_ICON_FILL) as u32,
                        imageops::FilterType::Triangle,
                    )
                    .to_rgba8();
                let x = (center_x - icon.width() as f32 / 2.0).round() as i64;
                let y = (band_top + (band_h - icon.height() as f32) / 2.0).round() as i64;
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
    if has_text {
        let place = text_place(ap.text_position, has_icon);
        // Texto por cima de ícone ou imagem ganha sombra para continuar legível.
        let shadow = has_icon && place == TextPosition::Middle;
        draw_text(
            &mut img,
            &ap.text,
            ap.font_size,
            ap.text_color,
            place,
            shadow,
        );
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

/// Quanto a imagem encolhe enquanto a tecla está apertada (a tecla "afunda").
const PRESSED_SCALE: f32 = 0.86;

/// A tecla apertada: a mesma imagem, menor e centrada sobre preto, como um botão que afundou.
pub fn render_pressed_rgba(button: &ButtonConfig, width: u32, height: u32) -> RgbaImage {
    let full = render_rgba(button, width, height);
    let (w, h) = (
        ((width as f32) * PRESSED_SCALE).round() as u32,
        ((height as f32) * PRESSED_SCALE).round() as u32,
    );
    let small = imageops::resize(&full, w, h, imageops::FilterType::Triangle);
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
    imageops::overlay(
        &mut img,
        &small,
        i64::from((width - w) / 2),
        i64::from((height - h) / 2),
    );
    img
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

    /// Linhas (y) da imagem com algum pixel claro (texto branco sobre o preto).
    fn inked_rows(img: &RgbaImage) -> Vec<u32> {
        (0..img.height())
            .filter(|&y| (0..img.width()).any(|x| img.get_pixel(x, y)[0] > 128))
            .collect()
    }

    fn labelled(text: &str, position: TextPosition) -> ButtonConfig {
        let mut b = ButtonConfig::blank();
        b.appearance.text = text.into();
        b.appearance.text_position = position;
        b
    }

    #[test]
    fn the_label_goes_where_it_is_told() {
        if font().is_none() {
            return; // sem fonte do sistema (CI mínimo), não há texto a medir
        }
        let top = inked_rows(&render_rgba(&labelled("Oi", TextPosition::Top), 100, 100));
        let bottom = inked_rows(&render_rgba(
            &labelled("Oi", TextPosition::Bottom),
            100,
            100,
        ));
        let middle = inked_rows(&render_rgba(
            &labelled("Oi", TextPosition::Middle),
            100,
            100,
        ));
        assert!(*top.last().unwrap() < 40, "{top:?}");
        assert!(*bottom.first().unwrap() > 60, "{bottom:?}");
        assert!(middle.first().unwrap() > top.first().unwrap());
        assert!(middle.last().unwrap() < bottom.last().unwrap());
    }

    #[test]
    fn a_label_with_two_lines_is_taller_than_one() {
        if font().is_none() {
            return;
        }
        let one = inked_rows(&render_rgba(
            &labelled("Live", TextPosition::Middle),
            100,
            100,
        ));
        let two = inked_rows(&render_rgba(
            &labelled("Live\nOBS", TextPosition::Middle),
            100,
            100,
        ));
        let span = |rows: &[u32]| rows.last().unwrap() - rows.first().unwrap();
        assert!(
            span(&two) > span(&one) + 8,
            "{} vs {}",
            span(&two),
            span(&one)
        );
    }

    #[test]
    fn the_clock_shows_hour_minute_and_weekday() {
        let at: jiff::Zoned = "2026-10-31T09:05:00-03:00[America/Sao_Paulo]"
            .parse()
            .unwrap();
        crate::strings::set_language(crate::config::schema::Language::Pt);
        assert_eq!(clock_text(&at), ("09:05".into(), "sáb 31/10".into()));
    }

    #[test]
    fn a_library_icon_is_drawn_on_the_key() {
        let mut b = ButtonConfig::blank();
        b.appearance.icon = Some("lucide:mic".into());
        assert!(!inked_rows(&render_rgba(&b, 100, 100)).is_empty());
    }

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
