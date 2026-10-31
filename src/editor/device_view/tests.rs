use super::*;

fn akp153e() -> &'static DeviceModel {
    model_for("ajazz-akp153e").unwrap()
}

/// Um aparelho de 6 teclas (2 x 3) sem tela lateral: o arquivo do AKP153E com outros controles.
fn six_keys() -> DeviceModel {
    let mut file: serde_json::Value =
        serde_json::from_str(include_str!("../../../devices/ajazz/akp153e.json")).unwrap();
    file["id"] = "six".into();
    file["status"] = "experimental".into();
    file["controls"] = (0..6)
        .map(|i| serde_json::json!({"id": format!("k{i}"), "kind": "key", "x": i % 3, "y": i / 3, "hw": i}))
        .collect();
    DeviceModel::parse(&file.to_string()).unwrap()
}

#[test]
fn keys_shrink_to_fit_a_short_stage_but_never_below_the_minimum() {
    // Com sobra de altura, a largura máxima do aparelho (780 px) é que limita a tecla.
    let by_width = (DEVICE_MAX_WIDTH - 2.0 * DEVICE_PAD - 5.0 * DEVICE_GAP) / (5.0 + SIDE_RATIO);
    assert_eq!(fit_key_size(akp153e(), 900.0, 2000.0), by_width);
    assert_eq!(fit_key_size(&six_keys(), 900.0, 2000.0), KEY_MAX);
    let mid = fit_key_size(akp153e(), 900.0, 400.0);
    assert!(mid > KEY_MIN && mid < KEY_MAX, "{mid}");
    assert_eq!(fit_key_size(akp153e(), 900.0, 50.0), KEY_MIN);
}

#[test]
fn the_device_is_never_wider_than_its_cap() {
    let key = fit_key_size(akp153e(), 5000.0, 5000.0);
    assert!(layout(akp153e(), key).size.x <= DEVICE_MAX_WIDTH + 1.0);
}

#[test]
fn the_akp153e_is_drawn_as_before() {
    let l = layout(akp153e(), 100.0);
    assert_eq!(
        l.size,
        vec2(
            2.0 * DEVICE_PAD + 500.0 + 5.0 * DEVICE_GAP + 100.0 * SIDE_RATIO,
            2.0 * DEVICE_PAD + 300.0 + 2.0 * DEVICE_GAP
        )
    );
    assert_eq!(l.keys.len(), 15);
    // Ordem de leitura: a tecla 7 é a 2ª fileira, 3ª coluna.
    assert_eq!(
        l.keys[7].min,
        pos2(DEVICE_PAD + 2.0 * 112.0, DEVICE_PAD + 112.0)
    );
    let strip = l.strip.unwrap();
    assert_eq!(strip.min, pos2(DEVICE_PAD + 5.0 * 112.0, DEVICE_PAD));
    assert_eq!(strip.size(), vec2(100.0 * SIDE_RATIO, 324.0));
}

#[test]
fn a_six_key_device_has_its_own_grid_and_no_side_screen() {
    let l = layout(&six_keys(), 100.0);
    assert_eq!(l.keys.len(), 6);
    assert!(l.strip.is_none());
    assert_eq!(
        l.size,
        vec2(
            2.0 * DEVICE_PAD + 300.0 + 2.0 * DEVICE_GAP,
            2.0 * DEVICE_PAD + 200.0 + DEVICE_GAP
        )
    );
}
