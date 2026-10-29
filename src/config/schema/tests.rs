use super::*;

#[test]
fn roundtrips_default_config() {
    let c = AppConfig::default();
    let back: AppConfig = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(back.page().buttons.len(), 15);
}

#[test]
fn sanitize_repairs_broken_config() {
    let broken = AppConfig {
        schema_version: SCHEMA_VERSION,
        profiles: vec![Profile {
            id: 0,
            name: "x".into(),
            pages: vec![],
            active_page: 9,
            auto_app: None,
        }],
        active_profile: 7,
        ui: UiSettings::default(),
    };
    assert_eq!(broken.sanitized().page().buttons.len(), 15);
    assert_eq!(
        AppConfig {
            schema_version: SCHEMA_VERSION,
            profiles: vec![],
            active_profile: 0,
            ui: UiSettings::default()
        }
        .sanitized()
        .page()
        .rows,
        3
    );
}

#[test]
fn migrates_legacy_index_targets_to_ids() {
    let mut cfg = AppConfig {
        schema_version: 1,
        ..Default::default()
    };
    cfg.profiles[0].pages[0].id = 0;
    cfg.profiles[0].id = 0;
    cfg.profiles[0].pages.push(Page::new("Page 2", 3, 5));
    cfg.profiles[0].pages[0].buttons[0].action = ButtonAction::SwitchPage { page: 1 };
    let cfg = cfg.sanitized();
    let p = &cfg.profiles[0];
    assert_eq!(cfg.schema_version, SCHEMA_VERSION);
    assert_eq!((p.pages[0].id, p.pages[1].id), (1, 2));
    assert_eq!(
        p.pages[0].buttons[0].action,
        ButtonAction::SwitchPage { page: 2 }
    );
}

#[test]
fn ids_survive_page_removal() {
    let mut cfg = AppConfig::default();
    cfg.profiles[0].add_page("B");
    cfg.profiles[0].add_page("C");
    cfg.profiles[0].pages.remove(1);
    cfg.profiles[0].add_page("D");
    let ids: Vec<u64> = cfg.profiles[0].pages.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![1, 3, 4]);
    assert_eq!(cfg.profiles[0].page_index(3), Some(1));
}

#[test]
fn save_is_atomic_and_leaves_no_temp_file() {
    let dir = std::env::temp_dir().join(format!("akp_cfg_{}", std::process::id()));
    let path = dir.join("config.json");
    AppConfig::default().save_to(&path).unwrap();
    let mut second = AppConfig::default();
    second.profiles[0].name = "novo".into();
    second.save_to(&path).unwrap();
    let back: AppConfig = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(back.profiles[0].name, "novo");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn removing_a_page_disables_actions_that_pointed_to_it() {
    let mut cfg = AppConfig::default();
    cfg.profiles[0].add_page("B");
    cfg.profiles[0].pages[0].buttons[0].action = ButtonAction::SwitchPage { page: 2 };
    cfg.profiles[0].pages[0].buttons[1].long_press_action =
        Some(ButtonAction::SwitchPage { page: 2 });
    cfg.profiles[0].pages[0].buttons[2].action = ButtonAction::SwitchPage { page: 1 };
    cfg.profiles[0].active_page = 1;
    assert!(cfg.profiles[0].remove_page(1));
    let p = &cfg.profiles[0];
    assert_eq!(p.pages[0].buttons[0].action, ButtonAction::None);
    assert_eq!(p.pages[0].buttons[1].long_press_action, None);
    assert_eq!(
        p.pages[0].buttons[2].action,
        ButtonAction::SwitchPage { page: 1 }
    );
    assert_eq!(p.active_page, 0);
}

#[test]
fn adopting_the_disk_navigation_follows_ids_not_positions() {
    let mut editor = AppConfig::default();
    editor.profiles[0].add_page("B");
    editor.profiles[0].add_page("C");
    let mut disk = editor.clone();
    disk.profiles[0].active_page = 2;
    editor.adopt_active_from(&disk);
    assert_eq!(editor.profiles[0].active_page, 2);

    // página que o editor já removeu: mantém a atual
    disk.profiles[0].pages[2].id = 99;
    editor.profiles[0].active_page = 1;
    editor.adopt_active_from(&disk);
    assert_eq!(editor.profiles[0].active_page, 1);
}

#[test]
fn backup_names_carry_a_timestamp_and_keep_the_config_directory() {
    let path = std::env::temp_dir().join("x").join("config.json");
    let path = path.as_path();
    let name = backup_path(path);
    assert_eq!(name.parent(), path.parent());
    let file = name.file_name().unwrap().to_string_lossy().into_owned();
    assert!(file.starts_with("config.json.bak-"), "{file}");
    assert_ne!(file, "config.json.bak-");
}

#[test]
fn last_page_and_last_profile_cannot_be_removed() {
    let mut cfg = AppConfig::default();
    assert!(!cfg.profiles[0].remove_page(0));
    assert!(!cfg.remove_profile(0));
}

#[test]
fn move_page_keeps_the_active_page_selected() {
    let mut cfg = AppConfig::default();
    cfg.profiles[0].add_page("B");
    cfg.profiles[0].add_page("C");
    cfg.profiles[0].active_page = 0;
    assert_eq!(cfg.profiles[0].move_page(0, 1), Some(1));
    assert_eq!(cfg.profiles[0].active_page, 1);
    assert_eq!(cfg.profiles[0].pages[1].id, 1);
    assert_eq!(cfg.profiles[0].move_page(2, 1), None);
    assert_eq!(cfg.profiles[0].move_page(0, -1), None);
}

#[test]
fn removing_a_profile_adjusts_the_active_one_and_dangling_actions() {
    let mut cfg = AppConfig::default();
    cfg.add_profile("B");
    cfg.add_profile("C");
    cfg.active_profile = 2;
    cfg.profiles[2].pages[0].buttons[0].action = ButtonAction::SwitchProfile { profile: 2 };
    cfg.profiles[2].pages[0].buttons[1].action = ButtonAction::SwitchProfile { profile: 1 };
    assert!(cfg.remove_profile(1));
    assert_eq!(cfg.active_profile, 1);
    assert_eq!(cfg.page().buttons[0].action, ButtonAction::None); // apontava para o perfil removido (id 2)
    assert_eq!(
        cfg.page().buttons[1].action,
        ButtonAction::SwitchProfile { profile: 1 }
    ); // perfil que continua existindo
}

#[test]
fn config_saved_before_brightness_existed_keeps_full_brightness() {
    let json = r#"{"theme":"light","accent":[1,2,3]}"#;
    let ui: UiSettings = serde_json::from_str(json).unwrap();
    assert_eq!(ui.brightness, crate::usb::DEFAULT_BRIGHTNESS);
    assert_eq!(ui.theme, Theme::Light);
}

#[test]
fn brightness_above_the_maximum_is_clamped_on_load() {
    let mut cfg = AppConfig::default();
    cfg.ui.brightness = 250;
    assert_eq!(cfg.sanitized().ui.brightness, crate::usb::MAX_BRIGHTNESS);
}

#[test]
fn old_config_without_ui_settings_gets_the_defaults() {
    let json = serde_json::to_string(&AppConfig::default())
        .unwrap()
        .replace(
            ",\"ui\":{\"theme\":\"dark\",\"accent\":[0,229,255],\"brightness\":100,\"language\":\"pt\"}",
            "",
        );
    assert!(!json.contains("\"ui\""));
    let cfg: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(cfg.ui, UiSettings::default());
}

#[test]
fn parses_action_with_defaults() {
    let a: ButtonAction =
        serde_json::from_str(r#"{"type":"http-webhook","url":"http://x"}"#).unwrap();
    assert_eq!(
        a,
        ButtonAction::HttpWebhook {
            url: "http://x".into(),
            method: "GET".into(),
            body: None
        }
    );
}

#[test]
fn default_long_press_is_500ms() {
    let b: ButtonConfig = serde_json::from_str(
        &serde_json::to_string(&ButtonConfig::blank())
            .unwrap()
            .replace("\"longPressThresholdMs\":500,", ""),
    )
    .unwrap();
    assert_eq!(b.long_press_threshold_ms, 500);
}

#[test]
fn new_buttons_are_black() {
    assert_eq!(
        ButtonConfig::blank().appearance.background,
        RGBAColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255
        }
    );
}

#[test]
fn legacy_gray_background_is_migrated_to_black_and_other_colors_are_kept() {
    let mut cfg = AppConfig::default();
    let red = RGBAColor {
        r: 200,
        g: 10,
        b: 10,
        a: 255,
    };
    let buttons = &mut cfg.page_mut().buttons;
    buttons[0].appearance.background = RGBAColor {
        r: 30,
        g: 30,
        b: 30,
        a: 255,
    };
    buttons[1].appearance.background = red;
    let cfg = cfg.sanitized();
    assert_eq!(
        cfg.page().buttons[0].appearance.background,
        BLANK_BACKGROUND
    );
    assert_eq!(cfg.page().buttons[1].appearance.background, red);
}
