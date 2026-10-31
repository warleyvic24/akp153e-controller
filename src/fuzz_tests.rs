//! Entradas quaisquer nos leitores que recebem dados de fora (arquivo de aparelho, config, porta
//! de controle): podem ser recusadas, nunca derrubar o programa nem quebrar as invariantes.
use crate::config::schema::{AppConfig, SCHEMA_VERSION};
use crate::device::DeviceModel;
use proptest::prelude::*;

proptest! {
    #[test]
    fn any_text_as_a_device_file_is_refused_or_valid(text in ".{0,400}") {
        if let Ok(model) = DeviceModel::parse(&text) {
            prop_assert!(!model.controls.is_empty());
        }
    }

    #[test]
    fn a_config_with_odd_numbers_is_repaired_on_load(
        rows in 0usize..50,
        cols in 0usize..50,
        buttons in 0usize..40,
        active_page in 0usize..100,
        active_profile in 0usize..100,
        version in 0u32..10,
        brightness in 0u8..=255,
    ) {
        let mut cfg = serde_json::to_value(AppConfig::default()).unwrap();
        cfg["schema_version"] = version.into();
        cfg["active_profile"] = active_profile.into();
        cfg["ui"]["brightness"] = brightness.into();
        let page = &mut cfg["profiles"][0]["pages"][0];
        page["rows"] = rows.into();
        page["cols"] = cols.into();
        let blank = page["buttons"][0].clone();
        page["buttons"] = serde_json::Value::Array(vec![blank; buttons]);
        cfg["profiles"][0]["active_page"] = active_page.into();
        let cfg: AppConfig = serde_json::from_value(cfg).unwrap();
        let cfg = cfg.sanitized();
        let page = cfg.page();
        prop_assert_eq!(page.buttons.len(), page.rows * page.cols);
        prop_assert!(cfg.ui.brightness <= crate::usb::MAX_BRIGHTNESS);
        prop_assert!(cfg.schema_version >= SCHEMA_VERSION);
    }

    #[test]
    fn any_text_as_a_config_file_never_panics(text in ".{0,300}") {
        if let Ok(cfg) = serde_json::from_str::<AppConfig>(&text) {
            let cfg = cfg.sanitized();
            prop_assert!(!cfg.profiles.is_empty());
        }
    }
}
