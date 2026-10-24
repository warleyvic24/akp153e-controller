use super::*;

const ACCENT: RGBAColor = RGBAColor {
    r: 0,
    g: 229,
    b: 255,
    a: 255,
};
const TAP: Duration = Duration::from_millis(50);
const HOME: usize = 14;

fn app(n: u32, volume: f32, active: bool) -> MixerApp {
    MixerApp {
        session: AudioSession {
            key: format!("app{n}"),
            name: format!("App{n}"),
            exe_path: None,
            pids: vec![n],
            volume,
            muted: false,
            active,
        },
        icon_path: None,
    }
}

/// Entrega uma leitura do sistema ao mixer como se tivesse sido feita agora.
fn load(v: &mut MixerView, d: MixerData) -> bool {
    let version = v.version();
    v.update(d, version)
}

fn data(apps: usize) -> MixerData {
    MixerData {
        apps: (1..=apps as u32).map(|n| app(n, 0.5, true)).collect(),
    }
}

fn view_at(apps: usize, opener: usize) -> MixerView {
    let mut v = MixerView::new(opener);
    load(&mut v, data(apps));
    v
}

fn view(apps: usize) -> MixerView {
    view_at(apps, HOME)
}

fn volume_of(effect: MixerEffect) -> f32 {
    match effect {
        MixerEffect::SetVolume { volume, .. } => volume,
        other => panic!("esperava SetVolume, veio {other:?}"),
    }
}

fn keys(v: &MixerView) -> Vec<MixerKey> {
    v.keys(ACCENT)
}

#[test]
fn page_has_one_key_per_button() {
    assert_eq!(keys(&view(2)).len(), KEY_COUNT);
}

#[test]
fn top_row_shows_each_app_with_name_and_percentage() {
    let mut v = MixerView::new(HOME);
    let mut d = data(5);
    d.apps[0].session.name = "Spotify".into();
    d.apps[0].session.volume = 0.62;
    load(&mut v, d);
    let k = keys(&v);
    assert_eq!(k[0].value, "62%");
    assert!(matches!(&k[0].face, KeyFace::App { name, .. } if name == "Spotify"));
    for column in 0..COLS {
        assert!(matches!(k[column].face, KeyFace::App { .. }));
        assert_eq!(k[COLS + column].face, KeyFace::Plus);
    }
}

#[test]
fn middle_row_raises_and_bottom_row_lowers_on_press() {
    let mut v = view(1);
    assert!((volume_of(v.press_down(5)) - 0.55).abs() < 1e-6);
    assert!((volume_of(v.press_down(10)) - 0.50).abs() < 1e-6);
}

#[test]
fn volume_stops_at_the_limits_without_calling_windows() {
    let mut v = MixerView::new(HOME);
    load(
        &mut v,
        MixerData {
            apps: vec![app(1, 1.0, true), app(2, 0.0, true)],
        },
    );
    assert_eq!(v.press_down(5), MixerEffect::None);
    assert_eq!(v.press_down(11), MixerEffect::None);
    assert!(volume_of(v.press_down(6)) > 0.0);
}

#[test]
fn effects_carry_every_process_of_the_program() {
    let mut v = MixerView::new(HOME);
    let mut d = data(1);
    d.apps[0].session.pids = vec![7, 8, 9];
    load(&mut v, d);
    assert!(matches!(v.press_down(5), MixerEffect::SetVolume { pids, .. } if pids == [7, 8, 9]));
    assert!(matches!(v.press_down(0), MixerEffect::SetMute { pids, .. } if pids == [7, 8, 9]));
}

#[test]
fn app_key_toggles_mute_and_keeps_the_percentage() {
    let mut v = view(1);
    assert!(matches!(
        v.press_down(0),
        MixerEffect::SetMute { muted: true, .. }
    ));
    let tile = &keys(&v)[0];
    assert!(tile.muted);
    assert_eq!(tile.value, "50%");
    assert!(matches!(
        v.press_down(0),
        MixerEffect::SetMute { muted: false, .. }
    ));
}

#[test]
fn changing_volume_unmutes_the_app_in_the_page_and_in_the_system() {
    let mut v = view(1);
    v.press_down(0);
    let effect = v.press_down(5);
    assert!(!keys(&v)[0].muted);
    assert!(matches!(
        effect,
        MixerEffect::SetVolume { unmute: true, .. }
    ));
}

#[test]
fn only_plus_and_minus_repeat_while_held() {
    let mut v = view(2);
    assert!(v.is_step_key(5) && v.is_step_key(10));
    assert!(!v.is_step_key(0) && !v.is_step_key(HOME) && !v.is_step_key(7));
    assert_eq!(v.repeat(0), MixerEffect::None);
    assert!(matches!(v.repeat(5), MixerEffect::SetVolume { .. }));
}

#[test]
fn meter_runs_across_the_three_keys_of_a_column() {
    assert_eq!(meter_slice(0.0, 2), 0.0);
    assert_eq!(meter_slice(1.0, 0), 1.0);
    assert_eq!(meter_slice(0.5, 2), 1.0);
    assert!((meter_slice(0.5, 1) - 0.5).abs() < 1e-6);
    assert_eq!(meter_slice(0.5, 0), 0.0);
}

#[test]
fn empty_slots_do_nothing() {
    let mut v = view(1);
    for index in [1, 6, 11, 2, 7, 12, 3, 8, 13, 4, 9] {
        assert_eq!(v.press_down(index), MixerEffect::None);
    }
}

#[test]
fn home_stays_on_the_opening_key_when_its_column_is_free() {
    let v = view_at(2, 7);
    assert!(matches!(keys(&v)[7].face, KeyFace::Home { more: false }));
}

#[test]
fn home_moves_to_a_free_column_instead_of_taking_an_apps_key() {
    let v = view_at(3, 6);
    let k = keys(&v);
    assert!(matches!(k[9].face, KeyFace::Home { .. }));
    assert_eq!(k[6].face, KeyFace::Plus);
}

#[test]
fn home_takes_the_opening_key_only_when_every_column_is_full() {
    let v = view_at(5, 6);
    assert!(matches!(keys(&v)[6].face, KeyFace::Home { .. }));
}

#[test]
fn home_does_not_jump_between_pages_when_there_are_more_apps() {
    let mut v = view_at(7, 6);
    assert!(matches!(keys(&v)[6].face, KeyFace::Home { more: true }));
    v.release(6, TAP);
    assert!(matches!(keys(&v)[6].face, KeyFace::Home { more: true }));
}

#[test]
fn home_key_outside_the_grid_falls_back_to_the_last_key() {
    let v = view_at(0, 40);
    assert!(matches!(keys(&v)[KEY_COUNT - 1].face, KeyFace::Home { .. }));
}

#[test]
fn there_is_always_exactly_one_home_key() {
    for apps in 0..9 {
        for opener in 0..KEY_COUNT {
            let homes = keys(&view_at(apps, opener))
                .iter()
                .filter(|k| matches!(k.face, KeyFace::Home { .. }))
                .count();
            assert_eq!(homes, 1, "apps={apps} opener={opener}");
        }
    }
}

#[test]
fn tapping_home_exits_when_everything_fits_on_one_page() {
    assert_eq!(view(5).release(HOME, TAP), MixerEffect::Close);
    assert_eq!(view(0).release(HOME, TAP), MixerEffect::Close);
}

#[test]
fn tapping_home_pages_through_apps_and_wraps() {
    let mut v = view(7);
    assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app1"));
    assert_eq!(v.release(HOME, TAP), MixerEffect::None);
    assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app6"));
    assert!(v.app_at(2).is_none());
    v.release(HOME, TAP);
    assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app1"));
}

#[test]
fn holding_home_always_exits() {
    assert_eq!(view(7).release(HOME, EXIT_HOLD), MixerEffect::Close);
}

#[test]
fn releasing_other_keys_does_nothing() {
    let mut v = view(2);
    assert_eq!(v.release(0, TAP), MixerEffect::None);
    assert_eq!(v.release(5, EXIT_HOLD), MixerEffect::None);
}

#[test]
fn home_label_says_what_a_tap_does() {
    let more = &keys(&view(7))[HOME];
    assert_eq!(more.label, "MAIS");
    assert!(!more.hint.is_empty());
    let exit = &keys(&view(2))[HOME];
    assert_eq!(exit.label, "SAIR");
    assert!(exit.hint.is_empty());
}

#[test]
fn apps_keep_their_order_when_windows_reorders_them() {
    let mut v = MixerView::new(HOME);
    load(&mut v, data(3));
    let mut reordered = data(3);
    reordered.apps.reverse();
    assert!(!load(&mut v, reordered));
    assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app1"));
}

#[test]
fn a_paused_app_stays_on_screen_dimmed() {
    let mut v = MixerView::new(HOME);
    load(&mut v, data(2));
    let mut later = data(2);
    later.apps[0].session.active = false;
    assert!(load(&mut v, later));
    assert!(keys(&v)[0].inactive);
    assert!(!keys(&v)[1].inactive);
}

#[test]
fn a_closed_program_leaves_and_a_new_one_joins_at_the_end() {
    let mut v = MixerView::new(HOME);
    load(&mut v, data(3));
    let mut later = data(3);
    later.apps.remove(1);
    later.apps.push(app(9, 0.5, true));
    assert!(load(&mut v, later));
    let order: Vec<_> = (0..3)
        .filter_map(|c| v.app_at(c))
        .map(|a| a.session.key.clone())
        .collect();
    assert_eq!(order, ["app1", "app3", "app9"]);
}

#[test]
fn a_new_app_that_is_not_playing_does_not_join() {
    let mut v = MixerView::new(HOME);
    load(&mut v, data(1));
    let mut later = data(1);
    later.apps.push(app(2, 0.5, false));
    assert!(!load(&mut v, later));
}

#[test]
fn a_reading_made_before_a_key_press_is_discarded() {
    let mut v = view(1);
    let stale_version = v.version();
    v.press_down(5);
    let mut stale = data(1);
    stale.apps[0].session.volume = 0.1;
    assert!(!v.update(stale, stale_version));
    assert_eq!(keys(&v)[0].value, "55%");
}

#[test]
fn a_reading_for_a_closed_mixer_is_not_accepted_by_a_new_one() {
    let old = MixerView::new(HOME);
    let mut reopened = MixerView::new(HOME);
    assert_ne!(old.version(), reopened.version());
    assert!(!reopened.update(data(1), old.version()));
}

#[test]
fn touching_the_page_discards_readings_started_before() {
    let mut v = view(1);
    let started = v.version();
    v.touch();
    assert!(!v.update(data(2), started));
    assert!(load(&mut v, data(2)));
}

#[test]
fn update_fixes_the_page_when_apps_disappear() {
    let mut v = view(7);
    v.release(HOME, TAP);
    assert!(v.update(data(2), v.version()));
    assert!(v.app_at(0).is_some());
}

#[test]
fn only_changed_keys_are_redrawn() {
    let mut v = view(2);
    assert_eq!(v.changed_keys(ACCENT).len(), KEY_COUNT);
    assert!(v.changed_keys(ACCENT).is_empty());
    v.press_down(5);
    let changed: Vec<usize> = v.changed_keys(ACCENT).iter().map(|(i, _)| *i).collect();
    // A tecla de baixo continua cheia no medidor, então não é reenviada.
    assert_eq!(changed, [0, 5]);
}

#[test]
fn invalidate_makes_the_next_draw_full() {
    let v = view(1);
    v.changed_keys(ACCENT);
    v.invalidate();
    assert_eq!(v.changed_keys(ACCENT).len(), KEY_COUNT);
}

/// Gera uma imagem das 15 teclas para conferir o visual:
/// `PREVIEW_OUT=pasta/mixer.png cargo test mixer_preview -- --ignored`.
#[test]
#[ignore]
fn mixer_preview() {
    let mut v = MixerView::new(HOME);
    let mut d = data(3);
    d.apps[0].session.name = "Spotify".into();
    d.apps[0].session.volume = 0.62;
    d.apps[1].session.name = "Google Chrome".into();
    d.apps[1].session.volume = 0.3;
    d.apps[2].session.name = "Discord".into();
    d.apps[2].session.volume = 1.0;
    d.apps[2].session.active = false;
    load(&mut v, d);
    v.press_down(0);
    let px = 85u32;
    let mut sheet = image::RgbaImage::new((px + 6) * 5, (px + 6) * 3);
    for (i, key) in v.keys(ACCENT).iter().enumerate() {
        let img = crate::graphics::mixer_art::render_mixer_key(key, px);
        let (x, y) = ((i % COLS) as u32 * (px + 6), (i / COLS) as u32 * (px + 6));
        image::imageops::overlay(&mut sheet, &img, x as i64, y as i64);
    }
    sheet
        .save(std::env::var("PREVIEW_OUT").expect("defina PREVIEW_OUT"))
        .unwrap();
}
