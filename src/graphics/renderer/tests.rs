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

#[test]
fn a_busy_key_still_fits_in_three_packets() {
    // Ruído colorido: o pior caso para o JPEG (sem nada a comprimir).
    let mut img = RgbaImage::new(100, 100);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let v = (x
            .wrapping_mul(2_654_435_761)
            .wrapping_add(y.wrapping_mul(40_503))) as u8;
        *p = Rgba([v, v.rotate_left(3), v.rotate_left(5), 255]);
    }
    let jpeg = encode_frame(&img);
    assert!(jpeg.len() <= MAX_JPEG_BYTES, "{} bytes", jpeg.len());
    assert!(image::load_from_memory(&jpeg).is_ok());
}
