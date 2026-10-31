use super::*;

/// As tabelas fixas de antes da Fase 1 (src/usb/mod.rs): o arquivo do AKP153E tem de dar o mesmo.
const OLD_IDS: [(u16, u16); 3] = [(0x0300, 0x3010), (0x33C3, 0x6677), (0x6602, 0x1001)];
const OLD_OPENDECK_TO_DEVICE: [u8; 18] =
    [12, 9, 6, 3, 0, 15, 13, 10, 7, 4, 1, 16, 14, 11, 8, 5, 2, 17];

#[test]
fn every_built_in_file_is_valid() {
    assert_eq!(catalog().len(), BUILT_IN.len());
}

#[test]
fn the_known_ids_are_the_same_as_before() {
    let mut ids = known_ids();
    ids.sort();
    let mut old = OLD_IDS.to_vec();
    old.sort();
    assert_eq!(ids, old);
}

#[test]
fn the_akp153e_file_gives_the_old_key_map_and_side_screen() {
    for (vid, pid) in OLD_IDS {
        let m = find(vid, pid).unwrap();
        for i in 0..15 {
            let old = OLD_OPENDECK_TO_DEVICE[(i / 5) * 6 + i % 5];
            assert_eq!(m.key_hw(i), Some(old), "{} tecla {i}", m.id);
            assert_eq!(m.key_of_hw(old), Some(i));
        }
        let side: Vec<u8> = m.strip().unwrap().slots.iter().map(|s| s.hw).collect();
        assert_eq!(side, [15, 16, 17]);
        assert!(side.iter().all(|&hw| m.key_of_hw(hw).is_none()));
    }
    let v3 = find(0x0300, 0x3010).unwrap();
    assert_eq!(
        (
            v3.image.px,
            v3.protocol.packets.clone(),
            v3.protocol.release_events
        ),
        (100, vec![1024, 512], true)
    );
    let v1 = find(0x33C3, 0x6677).unwrap();
    assert_eq!(
        (
            v1.image.px,
            v1.protocol.packets.clone(),
            v1.protocol.release_events
        ),
        (85, vec![512], false)
    );
    assert_eq!(v3.status, Status::Verified);
    assert_eq!(v1.status, Status::Experimental);
}

fn broken(change: impl Fn(&mut serde_json::Value)) -> Result<DeviceModel, String> {
    let mut v: serde_json::Value = serde_json::from_str(BUILT_IN[0]).unwrap();
    change(&mut v);
    DeviceModel::parse(&v.to_string())
}

#[test]
fn bad_files_are_refused_with_a_reason() {
    assert!(broken(|_| {}).is_ok());
    assert!(
        broken(|v| v["controls"][1]["hw"] = 12.into()).is_err(),
        "hw repetido"
    );
    assert!(
        broken(|v| v["controls"][1]["x"] = 0.into()).is_err(),
        "sobreposição"
    );
    assert!(
        broken(|v| v["controls"][0]["hw"] = 255.into()).is_err(),
        "hw reservado"
    );
    assert!(broken(|v| v["protocol"]["packets"] = serde_json::json!([4096])).is_err());
    assert!(broken(|v| v["image"]["px"] = 9000.into()).is_err());
    assert!(broken(|v| v["usb"][0]["vid"] = "zz".into()).is_err());
    assert!(
        broken(|v| v["command"] = "LOG".into()).is_err(),
        "campo desconhecido"
    );
    assert!(DeviceModel::parse(&" ".repeat(70_000)).is_err());
}
