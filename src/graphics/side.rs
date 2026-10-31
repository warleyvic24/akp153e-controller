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
use crate::platform::live::Live;
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
    pub alerts: SideAlerts,
}

/// O que é urgente e passa na frente de tudo no modo "Tocando agora": no ar, gravando, mic mudo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SideAlerts {
    pub live: bool,
    pub recording: bool,
    pub mic_muted: bool,
}

const ALERT_HEIGHT: u32 = 26;
const ALERT_GAP: u32 = 2;
const ALERT_TEXT_PX: f32 = 13.0;
const ALERT_RED: RGBAColor = RGBAColor {
    r: 200,
    g: 30,
    b: 40,
    a: 255,
};

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
pub fn render(side: &SideScreen, info: &SideInfo, live: &Live) -> Option<RgbaImage> {
    let mut img = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([0, 0, 0, 255]));
    match side {
        SideScreen::Off => return None,
        SideScreen::PageName => page_name(&mut img, info),
        SideScreen::Clock => clock(&mut img),
        SideScreen::System => super::live_art::side_system(&mut img, live.system, MUTED),
        // Inteligente: a música enquanto toca (ou pausou há pouco); fora isso, o relógio.
        SideScreen::NowPlaying => {
            match live.playing.as_ref().filter(|p| p.worth_showing()) {
                Some(p) => super::live_art::side_now_playing(&mut img, Some(p), MUTED),
                None => clock(&mut img),
            }
            alerts(&mut img, info.alerts);
        }
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

/// Faixas vermelhas no alto, por cima do resto: o que não pode passar despercebido numa live.
fn alerts(img: &mut RgbaImage, a: SideAlerts) {
    let Some(bold) = bold_font() else { return };
    let labels = [
        (a.live, "AO VIVO"),
        (a.recording, "GRAVANDO"),
        (a.mic_muted, "MIC MUDO"),
    ];
    let mut top = 0;
    for (_, label) in labels.iter().filter(|(on, _)| *on) {
        let band = RgbaImage::from_pixel(
            WIDTH,
            ALERT_HEIGHT,
            Rgba([ALERT_RED.r, ALERT_RED.g, ALERT_RED.b, 255]),
        );
        imageops::overlay(img, &band, 0, i64::from(top));
        let text_top = top as f32 + (ALERT_HEIGHT as f32 - ALERT_TEXT_PX) / 2.0 - 2.0;
        draw_line(
            img,
            bold,
            crate::strings::t(label),
            (ALERT_TEXT_PX, MIN_TEXT_PX),
            WHITE,
            text_top,
        );
        top += ALERT_HEIGHT + ALERT_GAP;
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
            alerts: SideAlerts::default(),
        }
    }

    #[test]
    fn off_draws_nothing_and_the_other_modes_fill_the_strip() {
        assert!(render(&SideScreen::Off, &info(), &Live::default()).is_none());
        for mode in [
            SideScreen::PageName,
            SideScreen::Clock,
            SideScreen::System,
            SideScreen::NowPlaying,
        ] {
            let img = render(&mode, &info(), &Live::default()).unwrap();
            assert_eq!(img.dimensions(), (WIDTH, HEIGHT));
            if font().is_some() {
                assert!(img.pixels().any(|p| p[0] > 128), "{mode:?} desenhou algo");
            }
        }
    }

    #[test]
    fn the_current_page_dot_uses_the_accent() {
        let img = render(&SideScreen::PageName, &info(), &Live::default()).unwrap();
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
        let img = render(&side, &info(), &Live::default()).unwrap();
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

    #[test]
    fn now_playing_turns_into_the_clock_without_music_or_after_a_long_pause() {
        let song = |playing, idle| Live {
            playing: Some(crate::platform::live::NowPlaying {
                title: "Música".into(),
                artist: String::new(),
                playing,
                art: None,
                idle,
                progress: None,
            }),
            ..Live::default()
        };
        let now = |live: &Live| render(&SideScreen::NowPlaying, &info(), live).unwrap();
        let clock = render(&SideScreen::Clock, &info(), &Live::default()).unwrap();
        assert_eq!(now(&Live::default()), clock, "sem música");
        assert_eq!(now(&song(false, true)), clock, "em pausa há muito tempo");
        if font().is_some() {
            assert_ne!(now(&song(true, false)), clock, "tocando");
            assert_ne!(now(&song(false, false)), clock, "pausa curta");
        }
    }

    #[test]
    fn the_progress_bar_shows_how_much_has_played() {
        let at = |progress| {
            let live = Live {
                playing: Some(crate::platform::live::NowPlaying {
                    title: "Música".into(),
                    artist: String::new(),
                    playing: true,
                    art: None,
                    idle: false,
                    progress,
                }),
                ..Live::default()
            };
            let img = render(&SideScreen::NowPlaying, &info(), &live).unwrap();
            // Pixels verdes da barra (o ▶ também é verde, mas é igual nos dois casos).
            img.pixels().filter(|p| p[1] > 150 && p[0] < 100).count()
        };
        assert!(at(Some((50, 60))) > at(Some((5, 60))));
        assert!(at(Some((5, 60))) > at(None));
    }

    #[test]
    fn alerts_go_on_top_in_red_only_when_something_is_on() {
        let mut with = info();
        let quiet = render(&SideScreen::NowPlaying, &with, &Live::default()).unwrap();
        with.alerts = SideAlerts {
            live: true,
            recording: false,
            mic_muted: true,
        };
        let loud = render(&SideScreen::NowPlaying, &with, &Live::default()).unwrap();
        let top = |img: &RgbaImage| *img.get_pixel(1, 1);
        assert_eq!(top(&loud)[0], ALERT_RED.r, "faixa vermelha no alto");
        assert_ne!(top(&quiet), top(&loud));
        // A segunda faixa (mic mudo) vem logo abaixo da primeira.
        assert_eq!(
            loud.get_pixel(1, ALERT_HEIGHT + ALERT_GAP + 1)[0],
            ALERT_RED.r
        );
        // Nos outros modos, nada de alerta.
        let clock = render(&SideScreen::Clock, &with, &Live::default()).unwrap();
        assert_ne!(top(&clock)[0], ALERT_RED.r);
    }

    /// Grava a faixa de cada modo em PNG para olhar: `AKP_SIDE_OUT=/pasta cargo test side_previews -- --ignored`.
    #[test]
    #[ignore = "gera arquivos para inspeção visual"]
    fn side_previews() {
        let dir = std::path::PathBuf::from(std::env::var("AKP_SIDE_OUT").unwrap());
        let mut loud = info();
        loud.alerts = SideAlerts {
            live: true,
            recording: true,
            mic_muted: true,
        };
        render(&SideScreen::NowPlaying, &loud, &Live::default())
            .unwrap()
            .save(dir.join("alertas.png"))
            .unwrap();
        for (name, mode) in [
            ("pagina", SideScreen::PageName),
            ("relogio", SideScreen::Clock),
            ("sistema", SideScreen::System),
            ("tocando", SideScreen::NowPlaying),
        ] {
            render(&mode, &info(), &Live::default())
                .unwrap()
                .save(dir.join(format!("{name}.png")))
                .unwrap();
        }
    }

    /// Como o aparelho deve ficar com uma config: as 15 teclas (com os dados ao vivo deste PC) e a
    /// tela lateral da página ("nome da página" se apagada), e ao lado a tecla 1 apertada. `AKP_PREVIEW_CONFIG=config.json AKP_PREVIEW_OUT=x.png
    /// cargo test device_preview -- --ignored`.
    #[test]
    #[ignore = "gera uma imagem para mostrar"]
    fn device_preview() {
        use crate::config::schema::AppConfig;
        use crate::graphics::live_art::render_live;
        use crate::graphics::renderer::pressed;
        use crate::platform::live::{Sampler, Wants};
        let text = std::fs::read_to_string(std::env::var("AKP_PREVIEW_CONFIG").unwrap()).unwrap();
        let cfg = serde_json::from_str::<AppConfig>(&text)
            .unwrap()
            .sanitized();
        let (key, gap, pad) = (100u32, 14u32, 24u32);
        let (w, h) = (
            2 * pad + 5 * (key + gap) + WIDTH,
            2 * pad + 3 * key + 2 * gap,
        );
        let mut img = RgbaImage::from_pixel(w + key + 3 * pad, h, Rgba([28, 30, 38, 255]));
        let page = cfg.page();
        // Duas consultas: a CPU é a diferença entre elas.
        let all = Wants {
            volume: true,
            playing: true,
            system: true,
            mic: true,
        };
        let mut sampler = Sampler::default();
        sampler.sample(all);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let live = sampler.sample(all);
        for (i, b) in page.buttons.iter().enumerate() {
            let x = pad + (i as u32 % 5) * (key + gap);
            let y = pad + (i as u32 / 5) * (key + gap);
            imageops::overlay(
                &mut img,
                &render_live(b, key, key, &live),
                x.into(),
                y.into(),
            );
        }
        let profile = &cfg.profiles[cfg.active_profile];
        let side_info = SideInfo {
            profile: profile.name.clone(),
            page: page.name.clone(),
            page_index: profile.active_page,
            page_count: profile.pages.len(),
            accent: crate::config::schema::RGBAColor {
                r: 0,
                g: 229,
                b: 255,
                a: 255,
            },
            alerts: SideAlerts::default(),
        };
        let mode = match &page.side {
            SideScreen::Off => SideScreen::PageName,
            other => other.clone(),
        };
        let strip = render(&mode, &side_info, &live).unwrap();
        let strip = imageops::resize(&strip, WIDTH, h - 2 * pad, imageops::FilterType::Triangle);
        imageops::overlay(&mut img, &strip, (pad + 5 * (key + gap)).into(), pad.into());
        let down = pressed(&render_live(&page.buttons[0], key, key, &live));
        imageops::overlay(&mut img, &down, (w + 2 * pad).into(), pad.into());
        img.save(std::env::var("AKP_PREVIEW_OUT").unwrap()).unwrap();
    }
}
