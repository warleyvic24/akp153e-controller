//! Testes de desempenho do desenho das teclas: tempos e memória dos caches.
//! Os limites valem para o build de teste (sem otimização); no release tudo é bem mais rápido.
use super::{cache, gif, renderer};
use crate::config::schema::ButtonConfig;
use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, Rgba, RgbaImage};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("keyforge-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Uma "foto" de 512 px com ruído (cara de decodificar e de redimensionar).
fn photo(path: &PathBuf, seed: u32) {
    let img = RgbaImage::from_fn(512, 512, |x, y| {
        let v = (x.wrapping_mul(31) ^ y.wrapping_mul(17) ^ seed) as u8;
        Rgba([v, v.wrapping_mul(3), v.wrapping_add(90), 255])
    });
    img.save(path).unwrap();
}

fn key_with_image(path: &std::path::Path) -> ButtonConfig {
    let mut b = ButtonConfig::blank();
    b.appearance.icon_path = Some(path.to_string_lossy().into_owned());
    b
}

#[test]
fn a_page_of_image_keys_renders_fast_once_cached() {
    let paths: Vec<PathBuf> = (0..15)
        .map(|i| {
            let p = scratch(&format!("key{i}.png"));
            photo(&p, i);
            p
        })
        .collect();
    let page: Vec<ButtonConfig> = paths.iter().map(|p| key_with_image(p)).collect();
    let cold = Instant::now();
    for b in &page {
        renderer::render_rgba(b, 100, 100);
    }
    let cold = cold.elapsed();
    let warm = Instant::now();
    const ROUNDS: u32 = 10;
    for _ in 0..ROUNDS {
        for b in &page {
            renderer::render_rgba(b, 100, 100);
        }
    }
    let per_page = warm.elapsed() / ROUNDS;
    eprintln!("página de 15 imagens: fria {cold:?}, com cache {per_page:?}");
    assert!(
        per_page < cold / 4,
        "o cache não pegou: {per_page:?} x {cold:?}"
    );
    // Meta do release: < 5 ms/página. O build de teste é 10-20x mais lento e roda junto com os
    // outros testes; o limite dele só pega a volta da decodificação a cada desenho (~700 ms).
    let limit = if cfg!(debug_assertions) { 150 } else { 5 };
    assert!(
        per_page < Duration::from_millis(limit),
        "página levou {per_page:?}"
    );
    for p in paths {
        cache::forget_image(&p);
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_image_cache_stays_within_50mb() {
    // 40 fotos de 4096 x 128 x 4 = 2 MB cada (80 MB decodificados no total).
    let paths: Vec<PathBuf> = (0..40)
        .map(|i| {
            let p = scratch(&format!("big{i}.png"));
            RgbaImage::from_pixel(4096, 128, Rgba([i as u8, 0, 0, 255]))
                .save(&p)
                .unwrap();
            p
        })
        .collect();
    for p in &paths {
        assert!(cache::image(p).is_some());
        assert!(cache::image_bytes() <= cache::IMAGE_BUDGET_BYTES);
    }
    for p in paths {
        cache::forget_image(&p);
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn gif_frames_are_not_decoded_per_draw() {
    let path = scratch("anim.gif");
    {
        let mut enc = GifEncoder::new(std::fs::File::create(&path).unwrap());
        for i in 0..30u8 {
            let img = RgbaImage::from_pixel(300, 300, Rgba([i * 8, 255 - i * 8, 40, 255]));
            enc.encode_frame(Frame::from_parts(
                img,
                0,
                0,
                Delay::from_numer_denom_ms(50, 1),
            ))
            .unwrap();
        }
    }
    let b = key_with_image(&path);
    let cold = Instant::now();
    renderer::render_rgba(&b, 100, 100);
    let cold = cold.elapsed();
    let warm = Instant::now();
    for _ in 0..30 {
        renderer::render_rgba(&b, 100, 100);
    }
    let per_draw = warm.elapsed() / 30;
    eprintln!("tecla com GIF: 1º desenho {cold:?}, seguintes {per_draw:?}");
    assert!(per_draw < cold / 4, "{per_draw:?} x {cold:?}");
    // 30 quadros reduzidos a 144 px: ~2,4 MB, longe dos 30 x 300 x 300 x 4 = 10,8 MB originais.
    assert!(gif::cached_bytes() <= gif::GIF_BUDGET_BYTES);
    assert!(gif::cached_bytes() < 30 * 300 * 300 * 4);
    let _ = std::fs::remove_file(path);
}
