use super::*;

fn akp153e() -> Profile {
    profile_candidates(0x0300, 0x3010)[0]
}

fn v1() -> Profile {
    profile_candidates(0x33C3, 0x6677)[0]
}

#[test]
fn the_side_screen_is_the_sixth_column_and_never_a_key() {
    let model = akp153e().model;
    assert_eq!(side_slots(model), [15, 16, 17]);
    for key in side_slots(model) {
        assert_eq!(
            device_to_ui(model, key + 1),
            None,
            "aperto na lateral não vira tecla"
        );
    }
}

#[test]
fn the_last_side_command_wins_and_strips_keep_their_order() {
    let strips = SideFrame::Strips([vec![1], vec![2], vec![3]]);
    let pending = collect_commands(
        [UsbCmd::Side(SideFrame::Clear), UsbCmd::Side(strips.clone())].into_iter(),
    );
    assert_eq!(pending.side, Some(strips));
}

#[test]
fn the_linux_udev_rule_covers_every_known_device() {
    let rules = include_str!("../../packaging/linux/70-keyforge.rules").to_lowercase();
    for (vid, pid) in crate::device::known_ids() {
        for subsystem in ["hidraw", "usb"] {
            let line = format!(
                "subsystem==\"{subsystem}\", attrs{{idvendor}}==\"{vid:04x}\", attrs{{idproduct}}==\"{pid:04x}\""
            );
            assert!(rules.contains(&line), "falta na regra udev: {line}");
        }
        // O dispositivo USB (não o hidraw) é quem tem `power/control`.
        let usb = format!(
            "env{{devtype}}==\"usb_device\", attr{{idvendor}}==\"{vid:04x}\", attr{{idproduct}}==\"{pid:04x}\", attr{{power/control}}=\"on\""
        );
        assert!(rules.contains(&usb), "sem autosuspend desligado: {usb}");
    }
}

#[test]
fn ui_keys_roundtrip_through_device_values() {
    let model = akp153e().model;
    for i in 0..15 {
        assert_eq!(device_to_ui(model, model.key_hw(i).unwrap() + 1), Some(i));
    }
    assert_eq!(model.key_hw(15), None);
}

#[test]
fn side_keys_and_invalid_values_are_ignored() {
    let model = akp153e().model;
    assert_eq!(device_to_ui(model, 0), None);
    assert_eq!(device_to_ui(model, 19), None);
    assert_eq!(device_to_ui(model, 16), None); // a lateral
}

#[test]
fn command_has_crt_prefix() {
    let c = command(512, b"STP");
    assert_eq!(&c[..9], &[0, b'C', b'R', b'T', 0, 0, b'S', b'T', b'P']);
    assert_eq!(c.len(), 513);
}

#[test]
fn akp153e_rev2_reports_release() {
    let v3 = profile_candidates(0x0300, 0x3010);
    assert!(v3[0].both_states);
    assert_eq!(
        (v3[0].payload, v3[1].payload, v3[0].icon_px),
        (1024, 512, 100)
    );
    let legacy = profile_candidates(0x33C3, 0x6677);
    assert!(!legacy[0].both_states);
    assert_eq!(
        (legacy.len(), legacy[0].payload, legacy[0].icon_px),
        (1, 512, 85)
    );
}

fn ack(key: u8, state: Option<u8>) -> Vec<u8> {
    let mut r = vec![0u8; ACK_KEY_OFFSET + 1];
    r[..3].copy_from_slice(b"ACK");
    r[ACK_KEY_OFFSET] = key;
    r.extend(state);
    r
}

#[test]
fn short_ack_is_ignored_on_v3_but_counts_as_press_on_v1() {
    assert_eq!(parse_ack(&ack(1, None), akp153e()), None);
    assert_eq!(parse_ack(&ack(1, None), v1()), Some((4, true)));
}

#[test]
fn ack_state_byte_gives_press_and_release() {
    assert_eq!(parse_ack(&ack(1, Some(1)), akp153e()), Some((4, true)));
    assert_eq!(parse_ack(&ack(1, Some(0)), akp153e()), Some((4, false)));
    assert_eq!(parse_ack(b"NAK\0\0\0\0\0\0\x01\x01", akp153e()), None);
}

#[test]
fn watchdog_releases_each_held_key_independently() {
    let (tx, rx) = std::sync::mpsc::channel();
    let long_ago = Instant::now() - HOLD_WATCHDOG - Duration::from_secs(1);
    let mut down = [None; UI_KEYS];
    down[2] = Some(long_ago);
    down[7] = Some(Instant::now());
    down[9] = Some(long_ago);
    expire_held(&mut down, &tx);
    let released: Vec<usize> = rx
        .try_iter()
        .map(|e| match e {
            UsbEvent::Key {
                index,
                pressed: false,
            } => index,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(released, vec![2, 9]);
    assert!(down[2].is_none() && down[7].is_some() && down[9].is_none());
}

fn frame(index: usize, tag: u8) -> UsbCmd {
    UsbCmd::Frame {
        index,
        data: vec![tag],
    }
}

#[test]
fn only_the_latest_frame_of_each_key_is_kept() {
    let cmds = vec![frame(0, 1), frame(1, 1), frame(0, 2), frame(0, 3)];
    let pending = collect_commands(cmds.into_iter());
    assert_eq!(pending.frames[0], Some(vec![3]));
    assert_eq!(pending.frames[1], Some(vec![1]));
    assert!(pending.frames[2].is_none() && !pending.reconnect);
}

#[test]
fn only_the_latest_brightness_is_kept_and_it_never_exceeds_the_maximum() {
    let pending =
        collect_commands(vec![UsbCmd::Brightness(20), UsbCmd::Brightness(60)].into_iter());
    assert_eq!(pending.brightness, Some(60));
    assert_eq!(
        brightness_command(512, 250),
        brightness_command(512, MAX_BRIGHTNESS)
    );
}

#[test]
fn reconnect_wins_and_out_of_range_keys_are_ignored() {
    let pending = collect_commands(vec![frame(99, 1), UsbCmd::Reconnect].into_iter());
    assert!(pending.reconnect);
    assert!(pending.frames.iter().all(Option::is_none));
}

#[test]
fn image_header_and_chunking() {
    let p = image_packets(512, 3, &vec![7u8; 1300]).unwrap();
    assert_eq!(p.len(), 1 + 3);
    assert_eq!(
        image_packets(1024, 0, &vec![1u8; 1300]).unwrap()[1].len(),
        1025
    );
    assert_eq!(&p[0][6..14], &[b'B', b'A', b'T', 0, 0, 0x05, 0x14, 4]);
    assert_eq!(p[3][1], 7);
    assert!(image_packets(512, 0, &vec![0u8; 70_000]).is_none());
}

/// Relatórios com bytes quaisquer (cabo ruim, clone com outro dialeto) nunca derrubam a leitura e
/// só viram tecla se o índice existir no modelo.
mod random_reports {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn any_report_is_either_ignored_or_a_real_key(report in proptest::collection::vec(any::<u8>(), 0..64)) {
            for profile in [akp153e(), v1()] {
                if let Some((key, _)) = parse_ack(&report, profile) {
                    prop_assert!(key < profile.model.keys().len());
                }
            }
        }
    }
}
