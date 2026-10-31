use super::*;

fn tap(run: &mut Run, key: usize, at: Instant, held_ms: u64) -> Vec<Effect> {
    run.handle(key, true, at);
    run.handle(key, false, at + Duration::from_millis(held_ms))
}

fn through_the_keys(run: &mut Run, at: Instant) {
    for i in 0..run.keys {
        tap(run, i, at, 50);
    }
}

#[test]
fn every_key_in_order_then_hold_brightness_and_side() {
    let at = Instant::now();
    let mut run = Run::new(15, true, at);
    through_the_keys(&mut run, at);
    assert_eq!(run.step, Step::Hold);
    assert!(run.pressed.iter().all(|p| *p == Some(true)));
    assert_eq!(
        tap(&mut run, 0, at, 1000),
        vec![Effect::Brightness(DIM_LEVEL), Effect::Draw]
    );
    assert_eq!(
        tap(&mut run, 0, at, 50),
        vec![Effect::Brightness(0), Effect::SidePattern, Effect::Draw]
    );
    assert_eq!(tap(&mut run, 0, at, 50), vec![Effect::Finish, Effect::Draw]);
    assert_eq!(run.step, Step::Done);
    assert_eq!(run.failures, 0);
}

#[test]
fn a_key_arriving_with_another_number_is_a_mapping_failure() {
    let at = Instant::now();
    let mut run = Run::new(15, true, at);
    tap(&mut run, 0, at, 50);
    tap(&mut run, 7, at, 50); // pediu a 2, chegou a 8
    assert_eq!(run.pressed[1], Some(false));
    assert_eq!(run.step, Step::Press(2));
    assert_eq!(run.failures, 1);
    assert!(run
        .report
        .iter()
        .any(|l| l.contains("tecla 2") && l.contains("8")));
}

#[test]
fn a_short_hold_means_the_release_is_not_reported_in_time() {
    let at = Instant::now();
    let mut run = Run::new(15, false, at);
    through_the_keys(&mut run, at);
    tap(&mut run, 3, at, 2000); // outra tecla no passo de segurar: ignorada
    assert_eq!(run.step, Step::Hold);
    tap(&mut run, 0, at, 10);
    assert_eq!(run.failures, 1);
    // Sem tela lateral, "NÃO" no brilho já encerra.
    assert_eq!(
        tap(&mut run, NO_KEY, at, 50),
        vec![Effect::Brightness(0), Effect::Finish, Effect::Draw]
    );
    assert_eq!(run.failures, 2);
}

#[test]
fn only_yes_and_no_answer_a_question() {
    let at = Instant::now();
    let mut run = Run::new(15, true, at);
    through_the_keys(&mut run, at);
    tap(&mut run, 0, at, 1000);
    assert!(tap(&mut run, QUESTION_KEY, at, 50).is_empty());
    assert_eq!(run.step, Step::AskBrightness);
}

#[test]
fn a_release_without_its_press_is_ignored() {
    let at = Instant::now();
    let mut run = Run::new(15, true, at);
    assert!(run.handle(0, false, at).is_empty());
    run.handle(1, true, at);
    assert!(run.handle(0, false, at).is_empty());
    assert_eq!(run.step, Step::Press(0));
}

#[test]
fn the_screen_shows_the_target_key_and_the_answers() {
    let at = Instant::now();
    let mut run = Run::new(15, true, at);
    tap(&mut run, 0, at, 50);
    let view = run.keys_view();
    assert_eq!(view[0].1, PASS);
    assert_eq!(view[1].1, TARGET);
    assert_eq!(view[2].1, PENDING);
    for i in 1..15 {
        tap(&mut run, i, at, 50);
    }
    tap(&mut run, 0, at, 1000);
    let view = run.keys_view();
    assert_eq!(view[0].1, PASS);
    assert_eq!(view[NO_KEY].1, FAIL);
}

/// Prévia das telas do autoteste (grade 5 x 3 + lateral) em PNG:
/// `AKP_PREVIEW_OUT=<pasta> cargo test selftest_preview -- --ignored`.
#[test]
#[ignore]
fn selftest_preview() {
    let out = std::path::PathBuf::from(std::env::var("AKP_PREVIEW_OUT").expect("AKP_PREVIEW_OUT"));
    let at = Instant::now();
    let mut run = Run::new(15, true, at);
    let shot = |run: &Run, name: &str, side: bool| {
        let px = 100;
        let gap = 8;
        let side_w = if side {
            crate::graphics::side::WIDTH + gap
        } else {
            0
        };
        let mut img = RgbaImage::from_pixel(
            5 * (px + gap) + gap + side_w,
            3 * (px + gap) + gap,
            Rgba([20, 20, 24, 255]),
        );
        for (i, (text, bg)) in run.keys_view().into_iter().enumerate() {
            let mut b = ButtonConfig::blank();
            b.appearance.text = text;
            b.appearance.background = bg;
            b.appearance.font_size = LABEL_FONT;
            let key = render_rgba(&b, px, px);
            let (x, y) = (
                gap + (i as u32 % 5) * (px + gap),
                gap + (i as u32 / 5) * (px + gap),
            );
            image::imageops::overlay(&mut img, &key, x as i64, y as i64);
        }
        if side {
            let strip = image::imageops::resize(
                &side_pattern(),
                crate::graphics::side::WIDTH,
                3 * px + 2 * gap,
                image::imageops::FilterType::Triangle,
            );
            image::imageops::overlay(&mut img, &strip, (5 * (px + gap) + gap) as i64, gap as i64);
        }
        img.save(out.join(format!("selftest-{name}.png"))).unwrap();
    };
    for i in 0..4 {
        tap(&mut run, i, at, 50);
    }
    tap(&mut run, 9, at, 50);
    shot(&run, "1-teclas", false);
    for i in 5..15 {
        tap(&mut run, i, at, 50);
    }
    shot(&run, "2-segure", false);
    tap(&mut run, 0, at, 1000);
    shot(&run, "3-brilho", false);
    tap(&mut run, 0, at, 50);
    shot(&run, "4-lateral", true);
    tap(&mut run, 0, at, 50);
    shot(&run, "5-fim", false);
}
