//! Tela lateral do AKP153E: a faixa à direita das 15 teclas é uma tela só, de ~82 x 400 px. O
//! aparelho a trata como as 3 "teclas" da 6ª coluna (índices 15, 16 e 17 do aparelho); cada uma
//! desenha uma faixa de 82 x 200 a partir de uma linha fixa (0, 146 e 219), e a escrita seguinte
//! cobre a anterior. Por isso desenhamos uma tela inteira e a mandamos em três faixas que se
//! sobrepõem, de cima para baixo.
//!
//! Geometria medida com imagens de régua num AKP153E rev. 2 `0300:3010` por outro projeto
//! (bodyboarder2528/opendeck-akp153e-sac, `bridge.cs`, 27-10-2026); usa o mesmo comando BAT das
//! teclas (vai à tela, não à memória flash como o `LOG`).
use super::renderer::{bold_font, draw_line, encode_frame, font};
use crate::config::schema::{RGBAColor, SideScreen};
use image::{imageops, Rgba, RgbaImage};

pub const WIDTH: u32 = 82;
/// Altura visível da faixa.
pub const HEIGHT: u32 = 400;
/// Linha onde começa cada uma das 3 faixas (de cima para baixo).
pub const SLOT_TOP: [u32; 3] = [0, 146, 219];
pub const SLOT_HEIGHT: u32 = 200;
/// A tela desenhada vai até o fim da última faixa.
const CANVAS_HEIGHT: u32 = SLOT_TOP[2] + SLOT_HEIGHT;

/// O que a página informa à tela lateral.
#[derive(Clone, Debug, PartialEq)]
pub struct SideInfo {
    pub profile: String,
    pub page: String,
    pub page_index: usize,
    pub page_count: usize,
    pub accent: RGBAColor,
}

const WHITE: RGBAColor = RGBAColor {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};
const MUTED: RGBAColor = RGBAColor {
    r: 150,
    g: 156,
    b: 170,
    a: 255,
};
const MIN_TEXT_PX: f32 = 8.0;
const DOT_RADIUS: f32 = 4.0;
const DOT_GAP: f32 = 14.0;
/// Pontos de página mostrados no máximo (mais que isso não cabe na largura da faixa).
const MAX_DOTS: usize = 8;

/// A faixa inteira (82 x 400) para o modo escolhido; `None` quando apagada.
pub fn render(side: &SideScreen, info: &SideInfo) -> Option<RgbaImage> {
    let mut img = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([0, 0, 0, 255]));
    match side {
        SideScreen::Off => return None,
        SideScreen::PageName => page_name(&mut img, info),
        SideScreen::Clock => clock(&mut img),
        SideScreen::Image { path } => {
            if let Ok(picture) = image::open(path) {
                let filled = picture
                    .resize_to_fill(WIDTH, HEIGHT, imageops::FilterType::Triangle)
                    .to_rgba8();
                imageops::overlay(&mut img, &filled, 0, 0);
            } else {
                log::warn!("tela lateral: imagem ilegível {path}");
            }
        }
    }
    Some(img)
}

/// Nome da página escrito ao longo da faixa (de cima para baixo), o perfil em cima e os pontos
/// de página embaixo. Desenha numa tela deitada (400 x 82) e gira.
fn page_name(img: &mut RgbaImage, info: &SideInfo) {
    let (Some(font), Some(bold)) = (font(), bold_font()) else {
        return;
    };
    let mut lying = RgbaImage::from_pixel(HEIGHT, WIDTH, Rgba([0, 0, 0, 255]));
    draw_line(
        &mut lying,
        font,
        &info.profile,
        (18.0, MIN_TEXT_PX),
        MUTED,
        6.0,
    );
    draw_line(
        &mut lying,
        bold,
        &info.page,
        (34.0, MIN_TEXT_PX),
        WHITE,
        28.0,
    );
    *img = imageops::rotate90(&lying);
    // Pontos de página no pé da faixa, em pé (não giram).
    let count = info.page_count.clamp(1, MAX_DOTS);
    let total = DOT_GAP * (count as f32 - 1.0);
    for i in 0..count {
        let y = HEIGHT as f32 - 20.0 - total + DOT_GAP * i as f32;
        let color = if i == info.page_index.min(MAX_DOTS - 1) {
            info.accent
        } else {
            MUTED
        };
        dot(img, WIDTH as f32 / 2.0, y, DOT_RADIUS, color);
    }
}

fn dot(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: RGBAColor) {
    let (w, h) = img.dimensions();
    for y in (cy - r).floor().max(0.0) as u32..((cy + r).ceil() as u32).min(h) {
        for x in (cx - r).floor().max(0.0) as u32..((cx + r).ceil() as u32).min(w) {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let cover = (r + 0.5 - d).clamp(0.0, 1.0);
            if cover > 0.0 {
                super::renderer::blend(img.get_pixel_mut(x, y), color, cover);
            }
        }
    }
}

/// Hora em duas linhas grandes ("09" / "41") e a data embaixo, na largura da faixa.
fn clock(img: &mut RgbaImage) {
    let (Some(font), Some(bold)) = (font(), bold_font()) else {
        return;
    };
    let (time, date) = super::renderer::clock_text(&jiff::Zoned::now());
    let (hours, minutes) = time.split_once(':').unwrap_or((&time, ""));
    draw_line(img, bold, hours, (48.0, MIN_TEXT_PX), WHITE, 110.0);
    draw_line(img, bold, minutes, (48.0, MIN_TEXT_PX), WHITE, 170.0);
    draw_line(img, font, &date, (15.0, MIN_TEXT_PX), MUTED, 245.0);
}

/// As 3 faixas (82 x 200) a mandar, de cima para baixo, já codificadas para o aparelho.
pub fn strips(full: &RgbaImage) -> [Vec<u8>; 3] {
    let mut canvas = RgbaImage::from_pixel(WIDTH, CANVAS_HEIGHT, Rgba([0, 0, 0, 255]));
    imageops::overlay(&mut canvas, full, 0, 0);
    SLOT_TOP.map(|top| {
        let strip = imageops::crop_imm(&canvas, 0, top, WIDTH, SLOT_HEIGHT).to_image();
        encode_frame(&strip)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> SideInfo {
        SideInfo {
            profile: "Live".into(),
            page: "Cenas".into(),
            page_index: 1,
            page_count: 3,
            accent: RGBAColor {
                r: 0,
                g: 229,
                b: 255,
                a: 255,
            },
        }
    }

    #[test]
    fn off_draws_nothing_and_the_other_modes_fill_the_strip() {
        assert!(render(&SideScreen::Off, &info()).is_none());
        for mode in [SideScreen::PageName, SideScreen::Clock] {
            let img = render(&mode, &info()).unwrap();
            assert_eq!(img.dimensions(), (WIDTH, HEIGHT));
            if font().is_some() {
                assert!(img.pixels().any(|p| p[0] > 128), "{mode:?} desenhou algo");
            }
        }
    }

    #[test]
    fn the_current_page_dot_uses_the_accent() {
        let img = render(&SideScreen::PageName, &info()).unwrap();
        let cyan = img.pixels().filter(|p| p[2] > 200 && p[0] < 60).count();
        assert!(cyan > 10, "{cyan}");
    }

    #[test]
    fn an_image_is_cropped_to_fill_the_strip() {
        let dir = std::env::temp_dir().join(format!("akp_side_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("red.png");
        RgbaImage::from_pixel(300, 300, Rgba([220, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let side = SideScreen::Image {
            path: path.display().to_string(),
        };
        let img = render(&side, &info()).unwrap();
        assert_eq!(img.get_pixel(0, 0)[0], 220);
        assert_eq!(img.get_pixel(WIDTH - 1, HEIGHT - 1)[0], 220);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn three_overlapping_strips_cover_the_whole_strip_and_fit_the_limit() {
        let full = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([10, 200, 30, 255]));
        let parts = strips(&full);
        for jpeg in &parts {
            assert!(jpeg.len() <= super::super::renderer::MAX_JPEG_BYTES);
            // Girada como as teclas: 200 de largura por 82 de altura.
            let decoded = image::load_from_memory(jpeg).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (SLOT_HEIGHT, WIDTH));
        }
        assert!(
            SLOT_TOP[2] + SLOT_HEIGHT >= HEIGHT,
            "a última faixa chega ao pé"
        );
        assert!(
            SLOT_TOP.windows(2).all(|w| w[1] - w[0] < SLOT_HEIGHT),
            "sem buraco entre faixas"
        );
    }

    /// Grava a faixa de cada modo em PNG para olhar: `AKP_SIDE_OUT=/pasta cargo test side_previews -- --ignored`.
    #[test]
    #[ignore = "gera arquivos para inspeção visual"]
    fn side_previews() {
        let dir = std::path::PathBuf::from(std::env::var("AKP_SIDE_OUT").unwrap());
        for (name, mode) in [
            ("pagina", SideScreen::PageName),
            ("relogio", SideScreen::Clock),
        ] {
            render(&mode, &info())
                .unwrap()
                .save(dir.join(format!("{name}.png")))
                .unwrap();
        }
    }
}
