use super::*;
use crate::config::schema::ButtonConfig;
use crate::graphics::renderer::{encode_frame, MAX_JPEG_BYTES};
use crate::platform::live::{NowPlaying, SystemStats, Volume};

fn media(key: &str) -> ButtonConfig {
    let mut b = ButtonConfig::blank();
    b.action = ButtonAction::MediaKey { key: key.into() };
    b
}

fn monitor_key(metric: Metric) -> ButtonConfig {
    let mut b = ButtonConfig::blank();
    b.action = ButtonAction::Monitor { metric };
    b
}

fn volume(percent: u16, muted: bool) -> Live {
    Live {
        volume: Some(Volume { percent, muted }),
        ..Live::default()
    }
}

fn stats(cpu: u8, ram: u8, temp: Option<i16>) -> Live {
    Live {
        system: Some(SystemStats { cpu, ram, temp }),
        ..Live::default()
    }
}

/// Pixels perto de `color` (para achar a barra).
fn count_near(img: &RgbaImage, color: RGBAColor) -> usize {
    img.pixels()
        .filter(|p| {
            p[0].abs_diff(color.r) < 30
                && p[1].abs_diff(color.g) < 30
                && p[2].abs_diff(color.b) < 30
        })
        .count()
}

#[test]
fn only_volume_play_and_monitor_keys_are_live() {
    assert!(wants(&media("volup")).volume);
    assert!(wants(&media("mute")).volume);
    assert!(wants(&media("playpause")).playing);
    assert!(!wants(&media("next")).any());
    assert!(wants(&monitor_key(Metric::Ram)).system);
    assert!(!wants(&ButtonConfig::blank()).any());
}

#[test]
fn without_data_a_live_key_looks_like_a_plain_key() {
    for b in [media("volup"), media("playpause")] {
        assert_eq!(
            render_live(&b, 60, 60, &Live::default()).as_raw(),
            render_rgba(&b, 60, 60).as_raw()
        );
    }
}

#[test]
fn the_volume_bar_grows_with_the_level() {
    let low = count_near(
        &render_live(&media("volup"), 100, 100, &volume(10, false)),
        GREEN,
    );
    let high = count_near(
        &render_live(&media("volup"), 100, 100, &volume(90, false)),
        GREEN,
    );
    assert!(high > low * 3, "{low} {high}");
}

#[test]
fn a_muted_mute_key_turns_red() {
    let img = render_live(&media("mute"), 100, 100, &volume(40, true));
    let corner = img.get_pixel(2, 50).0;
    assert_eq!(&corner[..3], &[MUTED_BG.r, MUTED_BG.g, MUTED_BG.b]);
    let calm = render_live(&media("mute"), 100, 100, &volume(40, false));
    assert_ne!(
        &calm.get_pixel(2, 50).0[..3],
        &[MUTED_BG.r, MUTED_BG.g, MUTED_BG.b]
    );
}

#[test]
fn a_busy_cpu_bar_is_red_and_an_idle_one_green() {
    let busy = render_live(&monitor_key(Metric::Cpu), 100, 100, &stats(95, 10, None));
    let idle = render_live(&monitor_key(Metric::Cpu), 100, 100, &stats(5, 10, None));
    assert!(count_near(&busy, RED) > count_near(&idle, RED));
    assert!(count_near(&idle, GREEN) > 0);
}

#[test]
fn readings_show_dashes_without_data() {
    assert_eq!(reading(Metric::Temp, None).value, "--");
    let no_sensor = SystemStats {
        cpu: 1,
        ram: 2,
        temp: None,
    };
    assert_eq!(reading(Metric::Temp, Some(no_sensor)).value, "--");
    assert_eq!(reading(Metric::Ram, Some(no_sensor)).value, "2%");
    let hot = SystemStats {
        temp: Some(71),
        ..no_sensor
    };
    assert_eq!(reading(Metric::Temp, Some(hot)).value, "71°");
    assert_eq!(reading(Metric::Temp, Some(hot)).level, Some(71));
}

#[test]
fn a_cover_fills_the_key_and_the_frame_fits_the_device_limit() {
    let dir = std::env::temp_dir().join(format!("akp_live_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("capa.png");
    // Capa difícil de comprimir: ruído colorido.
    let noisy = RgbaImage::from_fn(300, 300, |x, y| {
        let v = (x * 7919 + y * 104_729) as u8;
        image::Rgba([v, v.wrapping_mul(3), v.wrapping_mul(7), 255])
    });
    noisy.save(&path).unwrap();
    let live = Live {
        playing: Some(NowPlaying {
            title: "Uma música com um título bem comprido".into(),
            artist: "Artista".into(),
            playing: true,
            art: Some(path.display().to_string()),
            idle: false,
        }),
        ..Live::default()
    };
    let img = render_live(&media("playpause"), 100, 100, &live);
    assert_ne!(
        img.as_raw(),
        render_rgba(&media("playpause"), 100, 100).as_raw()
    );
    assert!(encode_frame(&img).len() <= MAX_JPEG_BYTES);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_monitor_mode_fits_the_device_limit() {
    for metric in [Metric::Overview, Metric::Cpu, Metric::Ram, Metric::Temp] {
        let img = render_live(&monitor_key(metric), 100, 100, &stats(50, 70, Some(90)));
        assert!(encode_frame(&img).len() <= MAX_JPEG_BYTES, "{metric:?}");
    }
}

#[test]
fn black_bars_around_a_cover_are_trimmed_but_a_dark_cover_is_kept() {
    // 120 x 90 com faixas pretas de 15 px em cima e embaixo (a miniatura 4:3 do YouTube).
    let img = image::RgbImage::from_fn(120, 90, |_, y| {
        if !(15..75).contains(&y) {
            image::Rgb([0, 0, 0])
        } else {
            image::Rgb([200, 80, 40])
        }
    });
    let trimmed = trim_letterbox(image::DynamicImage::ImageRgb8(img));
    assert_eq!((trimmed.width(), trimmed.height()), (120, 60));
    let dark = image::DynamicImage::ImageRgb8(image::RgbImage::new(50, 50));
    assert_eq!(trim_letterbox(dark).width(), 50);
}

#[test]
fn a_long_title_splits_at_the_space_nearest_the_middle() {
    assert_eq!(split_title("Curto"), ("Curto", None));
    assert_eq!(
        split_title("6 Hacks do Hermes Desktop que me Aposentaram"),
        ("6 Hacks do Hermes Desktop", Some("que me Aposentaram"))
    );
}

#[test]
fn a_huge_title_is_cut_at_a_whole_word() {
    assert_eq!(shorten("Curto", 40), "Curto");
    assert_eq!(
        shorten(
            "6 Hacks do Hermes Desktop que me Aposentaram o CLAUDE CODE",
            40
        ),
        "6 Hacks do Hermes Desktop que me..."
    );
}

#[test]
fn the_mic_key_turns_red_only_when_the_microphone_is_muted() {
    let key = media("micmute");
    let muted = Live {
        mic_muted: Some(true),
        ..Live::default()
    };
    let open = Live {
        mic_muted: Some(false),
        ..Live::default()
    };
    let red = render_live(&key, 100, 100, &muted);
    let normal = render_live(&key, 100, 100, &open);
    let corner = |img: &RgbaImage| *img.get_pixel(3, 50);
    assert_ne!(corner(&red), corner(&normal));
    assert!(corner(&red)[0] > corner(&red)[2], "fundo vermelho");
    // Sem leitura do sistema, a tecla fica como foi configurada.
    assert_eq!(
        render_live(&key, 100, 100, &Live::default()),
        render_rgba(&key, 100, 100)
    );
}
