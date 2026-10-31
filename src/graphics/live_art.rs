//! Teclas vivas: volume geral (Vol+, Vol−, Mudo), o que está tocando (Play/Pause) e o uso do
//! sistema (tecla "Monitor" e o modo "Sistema" da tela lateral). Sem dado ao vivo, a tecla sai
//! como uma tecla comum (`renderer::render_rgba`).
use super::renderer::{
    background, bold_font, draw_glyphs, draw_line, fill_round_rect, font, render_rgba, text_width,
};
use crate::config::schema::{ButtonAction, ButtonConfig, Metric, RGBAColor, TextPosition};
use crate::platform::live::{Live, NowPlaying, SystemStats, Volume, Wants};
use crate::strings::t;
use ab_glyph::{Font, PxScale, ScaleFont};
use image::{imageops, RgbaImage};

const VOLUME_KEYS: [&str; 3] = ["volup", "voldown", "mute"];
const PLAY_KEY: &str = "playpause";

const GREEN: RGBAColor = rgb(34, 197, 94);
const AMBER: RGBAColor = rgb(245, 158, 11);
const RED: RGBAColor = rgb(239, 68, 68);
/// Fundo do Mudo ligado.
const MUTED_BG: RGBAColor = rgb(185, 28, 28);
const WHITE: RGBAColor = rgb(255, 255, 255);
const SOFT_WHITE: RGBAColor = rgb(205, 210, 220);
/// Abaixo disto a barra é verde; abaixo de `HOT`, âmbar; acima, vermelha.
const WARM: u8 = 60;
const HOT: u8 = 85;
/// Opacidade do trilho da barra (a parte vazia), sobre a cor do texto.
const TRACK_ALPHA: u8 = 60;
const MIN_PX: f32 = 7.0;

const fn rgb(r: u8, g: u8, b: u8) -> RGBAColor {
    RGBAColor { r, g, b, a: 255 }
}

/// Que dados ao vivo esta tecla usa.
pub fn wants(button: &ButtonConfig) -> Wants {
    match &button.action {
        ButtonAction::MediaKey { key } => Wants {
            volume: VOLUME_KEYS.contains(&key.as_str()),
            playing: key == PLAY_KEY,
            system: false,
        },
        ButtonAction::Monitor { .. } => Wants {
            system: true,
            ..Wants::default()
        },
        _ => Wants::default(),
    }
}

/// A tecla com o dado ao vivo; sem ele, igual a uma tecla comum.
pub fn render_live(button: &ButtonConfig, width: u32, height: u32, live: &Live) -> RgbaImage {
    match &button.action {
        ButtonAction::MediaKey { key } if VOLUME_KEYS.contains(&key.as_str()) => {
            match live.volume {
                Some(v) => volume(button, key == "mute", v, (width, height)),
                None => render_rgba(button, width, height),
            }
        }
        ButtonAction::MediaKey { key } if key == PLAY_KEY => match &live.playing {
            Some(p) => now_playing(button, p, (width, height)),
            None => render_rgba(button, width, height),
        },
        ButtonAction::Monitor { metric } => monitor(button, *metric, live.system, (width, height)),
        _ => render_rgba(button, width, height),
    }
}

/// Cor da barra para um uso de 0 a 100.
fn level_color(percent: u8) -> RGBAColor {
    if percent < WARM {
        GREEN
    } else if percent < HOT {
        AMBER
    } else {
        RED
    }
}

/// Barra de nível (x, y, largura, altura) com trilho; `fraction` de 0 a 1.
fn bar(
    img: &mut RgbaImage,
    (x, y, w, h): (f32, f32, f32, f32),
    fraction: f32,
    fill: RGBAColor,
    track: RGBAColor,
) {
    let r = h / 2.0;
    let rect = |w: f32| {
        (
            x.round() as i64,
            y.round() as i64,
            w.round() as i64,
            h.round() as i64,
        )
    };
    fill_round_rect(
        img,
        rect(w),
        r,
        RGBAColor {
            a: TRACK_ALPHA,
            ..track
        },
    );
    let filled = (w * fraction.clamp(0.0, 1.0)).max(if fraction > 0.0 { h } else { 0.0 });
    if filled > 0.0 {
        fill_round_rect(img, rect(filled), r, fill);
    }
}

/// Barra no pé da tecla, em fração do tamanho.
const KEY_BAR_TOP: f32 = 0.86;
const KEY_BAR_HEIGHT: f32 = 0.06;
const KEY_BAR_INSET: f32 = 0.16;

/// Vol+/Vol−/Mudo: a porcentagem como rótulo (em cima), o ícone no meio e a barra no pé; o Mudo
/// ligado fica vermelho e diz "MUDO".
fn volume(button: &ButtonConfig, mute_key: bool, v: Volume, (w, h): (u32, u32)) -> RgbaImage {
    let muted_now = mute_key && v.muted;
    let mut shown = button.clone();
    let ap = shown.current_appearance_mut();
    if muted_now {
        ap.background = MUTED_BG;
        ap.text = t("MUDO").to_string();
    } else if ap.text.trim().is_empty() {
        ap.text = format!("{}%", v.percent);
    }
    if matches!(ap.text_position, TextPosition::Auto | TextPosition::Bottom) {
        ap.text_position = TextPosition::Top;
    }
    let color = ap.text_color;
    let mut img = render_rgba(&shown, w, h);
    let (wf, hf) = (w as f32, h as f32);
    let fill = if v.muted { RED } else { GREEN };
    bar(
        &mut img,
        (
            wf * KEY_BAR_INSET,
            hf * KEY_BAR_TOP,
            wf * (1.0 - 2.0 * KEY_BAR_INSET),
            hf * KEY_BAR_HEIGHT,
        ),
        f32::from(v.percent.min(100)) / 100.0,
        fill,
        color,
    );
    img
}

/// Quanto a capa escurece para o texto ficar legível.
const ART_DIM: f32 = 0.45;
const ART_DIM_PAUSED: f32 = 0.65;
const TITLE_PX: f32 = 0.17;
const TITLE_TOP: f32 = 0.56;
const ARTIST_PX: f32 = 0.13;
const ARTIST_TOP: f32 = 0.77;
const GLYPH_SIZE: f32 = 0.16;
const GLYPH_MARGIN: f32 = 0.09;

/// Play/Pause: a capa (escurecida) cobrindo a tecla, o estado no canto e título e artista embaixo.
fn now_playing(button: &ButtonConfig, p: &NowPlaying, (w, h): (u32, u32)) -> RgbaImage {
    let ap = button.current_appearance();
    let art = p.art.as_deref().and_then(|path| image::open(path).ok());
    let mut img = match art {
        Some(art) => {
            let mut img = art
                .resize_to_fill(w, h, imageops::FilterType::Triangle)
                .to_rgba8();
            let dim = if p.playing { ART_DIM } else { ART_DIM_PAUSED };
            shade(&mut img, dim);
            img
        }
        None => background(ap.background, ap.gradient, w, h),
    };
    let (wf, hf) = (w as f32, h as f32);
    let size = hf * GLYPH_SIZE;
    let (gx, gy) = (wf - wf * GLYPH_MARGIN - size, hf * GLYPH_MARGIN);
    if p.playing {
        triangle(&mut img, (gx, gy, size), GREEN);
    } else {
        let bar_w = size * 0.34;
        for x in [gx, gx + size - bar_w] {
            fill_round_rect(
                &mut img,
                (
                    x.round() as i64,
                    gy as i64,
                    bar_w.round() as i64,
                    size.round() as i64,
                ),
                1.0,
                SOFT_WHITE,
            );
        }
    }
    let (Some(regular), Some(bold)) = (font(), bold_font()) else {
        return img;
    };
    let title = if p.title.is_empty() {
        t("Tocando")
    } else {
        &p.title
    };
    draw_line(
        &mut img,
        bold,
        title,
        (hf * TITLE_PX, MIN_PX),
        WHITE,
        hf * TITLE_TOP,
    );
    if !p.artist.is_empty() {
        draw_line(
            &mut img,
            regular,
            &p.artist,
            (hf * ARTIST_PX, MIN_PX),
            SOFT_WHITE,
            hf * ARTIST_TOP,
        );
    }
    img
}

/// Escurece a imagem: `amount` uniforme, mais um pouco no pé (onde vai o texto).
fn shade(img: &mut RgbaImage, amount: f32) {
    let h = img.height().max(2) as f32;
    for (_, y, px) in img.enumerate_pixels_mut() {
        let extra = (y as f32 / (h - 1.0) - 0.4).max(0.0) * 0.5;
        let keep = (1.0 - amount - extra).clamp(0.0, 1.0);
        for c in &mut px.0[..3] {
            *c = (f32::from(*c) * keep).round() as u8;
        }
    }
}

/// Triângulo de "tocando" apontado para a direita, no quadrado (x, y, lado).
fn triangle(img: &mut RgbaImage, (x, y, size): (f32, f32, f32), color: RGBAColor) {
    let (iw, ih) = img.dimensions();
    for py in y.floor().max(0.0) as u32..((y + size).ceil() as u32).min(ih) {
        let fy = (py as f32 + 0.5 - y) / size;
        // Largura da linha: cresce até o meio e volta.
        let reach = size * (1.0 - (2.0 * fy - 1.0).abs());
        for px in x.floor().max(0.0) as u32..((x + size).ceil() as u32).min(iw) {
            let cover = (x + reach - (px as f32 + 0.5) + 0.5).clamp(0.0, 1.0);
            if cover > 0.0 && fy > 0.0 && fy < 1.0 {
                super::renderer::blend(img.get_pixel_mut(px, py), color, cover);
            }
        }
    }
}

/// Visão geral: 3 linhas, cada uma com o nome à esquerda, o valor à direita e a barra embaixo.
const ROW_TOPS: [f32; 3] = [0.08, 0.38, 0.68];
const ROW_TEXT_PX: f32 = 0.15;
const ROW_BAR_GAP: f32 = 0.17;
const ROW_BAR_HEIGHT: f32 = 0.06;
const ROW_INSET: f32 = 0.09;
/// Uma medida só: nome em cima, número grande e barra no pé.
const SINGLE_LABEL_PX: f32 = 0.15;
const SINGLE_LABEL_TOP: f32 = 0.08;
const SINGLE_VALUE_PX: f32 = 0.36;
const SINGLE_VALUE_TOP: f32 = 0.3;

/// Uma medida pronta para desenhar: nome curto, texto do valor e uso (0 a 100) para a barra.
struct Reading {
    name: &'static str,
    value: String,
    level: Option<u8>,
}

fn reading(metric: Metric, stats: Option<SystemStats>) -> Reading {
    let pct = |v: u8| (format!("{v}%"), Some(v));
    let (name, (value, level)) = match metric {
        Metric::Cpu | Metric::Overview => {
            ("CPU", stats.map_or(("--".into(), None), |s| pct(s.cpu)))
        }
        Metric::Ram => ("RAM", stats.map_or(("--".into(), None), |s| pct(s.ram))),
        Metric::Temp => (
            "TEMP",
            match stats.and_then(|s| s.temp) {
                Some(c) => (format!("{c}°"), Some(c.clamp(0, 100) as u8)),
                None => ("--".into(), None),
            },
        ),
    };
    Reading { name, value, level }
}

/// Tecla "Monitor": as três medidas, ou uma só em destaque.
fn monitor(
    button: &ButtonConfig,
    metric: Metric,
    stats: Option<SystemStats>,
    (w, h): (u32, u32),
) -> RgbaImage {
    let ap = button.current_appearance();
    let color = ap.text_color;
    let mut img = background(ap.background, ap.gradient, w, h);
    let (Some(regular), Some(bold)) = (font(), bold_font()) else {
        return img;
    };
    let (wf, hf) = (w as f32, h as f32);
    let inset = wf * ROW_INSET;
    if metric == Metric::Overview {
        for (top, m) in ROW_TOPS
            .iter()
            .zip([Metric::Cpu, Metric::Ram, Metric::Temp])
        {
            let r = reading(m, stats);
            let y = hf * top;
            let scale = PxScale::from(hf * ROW_TEXT_PX);
            let baseline = y + regular.as_scaled(scale).ascent();
            draw_glyphs(&mut img, regular, scale, r.name, (inset, baseline), color);
            let vw = text_width(bold, scale, &r.value);
            draw_glyphs(
                &mut img,
                bold,
                scale,
                &r.value,
                (wf - inset - vw, baseline),
                color,
            );
            let fill = r.level.map_or(color, level_color);
            let frac = f32::from(r.level.unwrap_or(0)) / 100.0;
            bar(
                &mut img,
                (
                    inset,
                    y + hf * ROW_BAR_GAP,
                    wf - 2.0 * inset,
                    hf * ROW_BAR_HEIGHT,
                ),
                frac,
                fill,
                color,
            );
        }
    } else {
        let r = reading(metric, stats);
        draw_line(
            &mut img,
            regular,
            r.name,
            (hf * SINGLE_LABEL_PX, MIN_PX),
            color,
            hf * SINGLE_LABEL_TOP,
        );
        draw_line(
            &mut img,
            bold,
            &r.value,
            (hf * SINGLE_VALUE_PX, MIN_PX),
            color,
            hf * SINGLE_VALUE_TOP,
        );
        let fill = r.level.map_or(color, level_color);
        let frac = f32::from(r.level.unwrap_or(0)) / 100.0;
        bar(
            &mut img,
            (
                wf * KEY_BAR_INSET,
                hf * KEY_BAR_TOP,
                wf * (1.0 - 2.0 * KEY_BAR_INSET),
                hf * KEY_BAR_HEIGHT,
            ),
            frac,
            fill,
            color,
        );
    }
    img
}

/// Tela lateral no modo "Sistema" (82 x 400): CPU, RAM e temperatura, uma em cima da outra.
const SIDE_BLOCK_TOPS: [f32; 3] = [24.0, 154.0, 284.0];
const SIDE_LABEL_PX: f32 = 14.0;
const SIDE_VALUE_PX: f32 = 30.0;
const SIDE_VALUE_GAP: f32 = 22.0;
const SIDE_BAR_GAP: f32 = 64.0;
const SIDE_BAR_HEIGHT: f32 = 8.0;
const SIDE_INSET: f32 = 10.0;

pub fn side_system(img: &mut RgbaImage, stats: Option<SystemStats>, muted: RGBAColor) {
    let (Some(regular), Some(bold)) = (font(), bold_font()) else {
        return;
    };
    let w = img.width() as f32;
    for (top, m) in SIDE_BLOCK_TOPS
        .iter()
        .zip([Metric::Cpu, Metric::Ram, Metric::Temp])
    {
        let r = reading(m, stats);
        draw_line(img, regular, r.name, (SIDE_LABEL_PX, MIN_PX), muted, *top);
        draw_line(
            img,
            bold,
            &r.value,
            (SIDE_VALUE_PX, MIN_PX),
            WHITE,
            top + SIDE_VALUE_GAP,
        );
        let fill = r.level.map_or(muted, level_color);
        let frac = f32::from(r.level.unwrap_or(0)) / 100.0;
        bar(
            img,
            (
                SIDE_INSET,
                top + SIDE_BAR_GAP,
                w - 2.0 * SIDE_INSET,
                SIDE_BAR_HEIGHT,
            ),
            frac,
            fill,
            WHITE,
        );
    }
}

#[cfg(test)]
mod tests;
