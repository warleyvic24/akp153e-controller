//! Desenho das teclas do mixer. Cada coluna de app tem um medidor de volume que atravessa as três teclas,
//! sinais de + e − grandes, e na tecla do meio o ícone, o nome e a porcentagem em negrito.
use super::renderer::{bold_font, draw_icon, draw_line};
use crate::config::schema::RGBAColor;
use crate::mixer::{KeyFace, MixerKey};
use image::{imageops, Rgba, RgbaImage};

/// Tamanho de referência: as medidas abaixo valem para uma tecla de 85 px e crescem com `px`.
const REFERENCE_PX: f32 = 85.0;
const METER_WIDTH: f32 = 6.0;
const ICON_TOP: f32 = 5.0;
const ICON_SIZE: f32 = 30.0;
const NAME_TOP: f32 = 38.0;
const NAME_SIZE: (f32, f32) = (13.0, 11.0);
const VALUE_TOP: f32 = 54.0;
const VALUE_SIZE: (f32, f32) = (22.0, 14.0);
const LABEL_TOP: f32 = 40.0;
const HINT_TOP: f32 = 60.0;
const HINT_SIZE: (f32, f32) = (11.0, 9.0);
const SIGN_ARM: f32 = 0.30;
const SIGN_THICKNESS: f32 = 0.10;

const BLACK: RGBAColor = RGBAColor {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};
const TILE: RGBAColor = RGBAColor {
    r: 16,
    g: 24,
    b: 44,
    a: 255,
};
const TILE_MUTED: RGBAColor = RGBAColor {
    r: 110,
    g: 20,
    b: 24,
    a: 255,
};
const TRACK: RGBAColor = RGBAColor {
    r: 40,
    g: 48,
    b: 70,
    a: 255,
};
const METER_MUTED: RGBAColor = RGBAColor {
    r: 120,
    g: 70,
    b: 74,
    a: 255,
};
const NAME_COLOR: RGBAColor = RGBAColor {
    r: 190,
    g: 200,
    b: 222,
    a: 255,
};
const VALUE_COLOR: RGBAColor = RGBAColor {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};
const VALUE_MUTED_COLOR: RGBAColor = RGBAColor {
    r: 255,
    g: 170,
    b: 170,
    a: 255,
};

pub fn render_mixer_key(key: &MixerKey, px: u32) -> RgbaImage {
    let scale = px as f32 / REFERENCE_PX;
    let tile = matches!(key.face, KeyFace::App { .. });
    let background = match (tile, key.muted) {
        (true, true) => TILE_MUTED,
        (true, false) => TILE,
        _ => BLACK,
    };
    let mut img = RgbaImage::from_pixel(
        px,
        px,
        Rgba([background.r, background.g, background.b, 255]),
    );
    if let Some(fill) = key.meter {
        draw_meter(&mut img, fill, key.muted, key.accent, scale);
    }
    match &key.face {
        KeyFace::Empty => {}
        KeyFace::Plus => draw_sign(&mut img, true, key.accent),
        KeyFace::Minus => draw_sign(&mut img, false, key.accent),
        KeyFace::App { name, icon_path } => {
            draw_app_icon(&mut img, icon_path.as_deref(), scale, key);
            draw_text(
                &mut img,
                name,
                NAME_TOP * scale,
                scale,
                NAME_SIZE,
                NAME_COLOR,
            );
            draw_value(&mut img, key, scale);
        }
        KeyFace::Home { more } => {
            draw_builtin(
                &mut img,
                if *more { "next" } else { "prev" },
                scale,
                key.accent,
            );
            draw_text(
                &mut img,
                &key.label,
                LABEL_TOP * scale,
                scale,
                NAME_SIZE,
                VALUE_COLOR,
            );
            draw_text(
                &mut img,
                &key.hint,
                HINT_TOP * scale,
                scale,
                HINT_SIZE,
                NAME_COLOR,
            );
        }
    }
    img
}

/// Faixa vertical na borda esquerda; `fill` (0 a 1) sobe a partir de baixo.
fn draw_meter(img: &mut RgbaImage, fill: f32, muted: bool, accent: RGBAColor, scale: f32) {
    let width = (METER_WIDTH * scale).round().max(3.0) as u32;
    let height = img.height();
    let lit_from = height - (fill.clamp(0.0, 1.0) * height as f32).round() as u32;
    let lit = if muted { METER_MUTED } else { accent };
    for y in 0..height {
        let color = if y >= lit_from { lit } else { TRACK };
        for x in 0..width {
            img.put_pixel(x, y, Rgba([color.r, color.g, color.b, 255]));
        }
    }
}

/// Sinal de + ou − feito de retângulos, centrado na parte livre da tecla (fora do medidor).
fn draw_sign(img: &mut RgbaImage, plus: bool, color: RGBAColor) {
    let side = img.width() as f32;
    let (cx, cy) = (side / 2.0 + METER_WIDTH / 2.0, side / 2.0);
    let (arm, thick) = (side * SIGN_ARM, side * SIGN_THICKNESS);
    fill_rect(
        img,
        (cx - arm, cy - thick / 2.0),
        (cx + arm, cy + thick / 2.0),
        color,
    );
    if plus {
        fill_rect(
            img,
            (cx - thick / 2.0, cy - arm),
            (cx + thick / 2.0, cy + arm),
            color,
        );
    }
}

fn fill_rect(img: &mut RgbaImage, (x0, y0): (f32, f32), (x1, y1): (f32, f32), c: RGBAColor) {
    let clamp = |v: f32, max: u32| v.round().clamp(0.0, max as f32) as u32;
    for y in clamp(y0, img.height())..clamp(y1, img.height()) {
        for x in clamp(x0, img.width())..clamp(x1, img.width()) {
            img.put_pixel(x, y, Rgba([c.r, c.g, c.b, 255]));
        }
    }
}

fn draw_app_icon(img: &mut RgbaImage, path: Option<&str>, scale: f32, key: &MixerKey) {
    let size = (ICON_SIZE * scale).round() as u32;
    let icon = path.and_then(|p| image::open(p).ok()).map(|i| {
        i.resize(size, size, imageops::FilterType::Triangle)
            .to_rgba8()
    });
    match icon {
        Some(icon) => {
            let x = (img.width().saturating_sub(icon.width()) / 2) as i64;
            imageops::overlay(img, &icon, x, (ICON_TOP * scale) as i64);
        }
        None => draw_builtin(img, "app", scale, key.accent),
    }
}

fn draw_builtin(img: &mut RgbaImage, name: &str, scale: f32, color: RGBAColor) {
    let size = ICON_SIZE * scale;
    let cx = img.width() as f32 / 2.0 + METER_WIDTH * scale / 2.0;
    draw_icon(
        img,
        name,
        cx,
        (ICON_TOP * scale) + size / 2.0,
        size * 0.8,
        color,
    );
}

fn draw_value(img: &mut RgbaImage, key: &MixerKey, scale: f32) {
    let color = if key.muted {
        VALUE_MUTED_COLOR
    } else {
        VALUE_COLOR
    };
    draw_text(img, &key.value, VALUE_TOP * scale, scale, VALUE_SIZE, color);
}

fn draw_text(
    img: &mut RgbaImage,
    text: &str,
    top: f32,
    scale: f32,
    (size, min_px): (f32, f32),
    color: RGBAColor,
) {
    if let Some(font) = bold_font() {
        draw_line(img, font, text, (size * scale, min_px * scale), color, top);
    }
}
