//! Transporte HID do AKP153E (interface vendor-specific, usage page 0xFFA0).
//! Relatórios de 513 bytes (report id 0x00 + 512), comandos com prefixo "CRT", imagens JPEG 85x85.
use hidapi::{HidApi, HidDevice, HidError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

pub const IDS: [(u16, u16); 3] = [(0x0300, 0x3010), (0x33C3, 0x6677), (0x6602, 0x1001)];
pub const ICON_SIZE: usize = 85;
const V3_ICON_PX: u32 = 100; // calibração no aparelho: 95 e 100 mostram as 4 bordas e a régua inteiras; acima de ~104 corta
const USAGE_PAGE: u16 = 0xFFA0;
const MAX_PAYLOAD: usize = 1024;
const HOLD_WATCHDOG: Duration = Duration::from_secs(5);
const CMD_OFFSET: usize = 6;
const READ_TIMEOUT_MS: i32 = 50;
const KEEPALIVE: Duration = Duration::from_secs(10);
pub const DEFAULT_BRIGHTNESS: u8 = 100;
pub const MAX_BRIGHTNESS: u8 = 100;
/// Brilho a usar na próxima (re)inicialização do aparelho; o editor o muda pela config (`reload`).
static BRIGHTNESS_LEVEL: AtomicU8 = AtomicU8::new(DEFAULT_BRIGHTNESS);

/// Guarda o brilho (0 a 100) para a próxima inicialização do aparelho.
pub fn set_brightness(level: u8) {
    BRIGHTNESS_LEVEL.store(level.min(MAX_BRIGHTNESS), Ordering::Relaxed);
}

fn brightness_command(payload: usize, level: u8) -> Vec<u8> {
    command(
        payload,
        &[b'L', b'I', b'G', 0, 0, level.min(MAX_BRIGHTNESS)],
    )
}
const CLEAR_ALL_KEYS: u8 = 0xFF;
const DEVICE_COLS: usize = 6;
const UI_COLS: usize = 5;
const ACK_KEY_OFFSET: usize = 9;
const STATE_OFFSET: usize = 10;
const OPENDECK_TO_DEVICE: [u8; 18] = [12, 9, 6, 3, 0, 15, 13, 10, 7, 4, 1, 16, 14, 11, 8, 5, 2, 17];
const DEVICE_TO_OPENDECK: [u8; 18] = [4, 10, 16, 3, 9, 15, 2, 8, 14, 1, 7, 13, 0, 6, 12, 5, 11, 17];

/// Parâmetros de protocolo por modelo (v1: 512 bytes, só aperto; v3: soltura reportada).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Profile {
    payload: usize,
    both_states: bool,
    icon_px: u32,
}

/// Perfis candidatos para o modelo, em ordem de preferência (v3: pacote de 1024 com recuo para 512).
fn profile_candidates(vid: u16, pid: u16) -> Vec<Profile> {
    let v3 = (vid, pid) == (0x0300, 0x3010);
    let icon_px = if v3 { V3_ICON_PX } else { ICON_SIZE as u32 };
    // AKP_PACKET=512|1024 força um tamanho de pacote sem recompilar.
    let forced = std::env::var("AKP_PACKET")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| [512, 1024].contains(n));
    let payloads = match forced {
        Some(n) => vec![n],
        None if v3 => vec![1024, 512],
        None => vec![512],
    };
    payloads
        .into_iter()
        .map(|payload| Profile {
            payload,
            both_states: v3,
            icon_px,
        })
        .collect()
}

#[derive(Debug)]
pub enum UsbEvent {
    Status(bool),
    Info { icon_px: u32 },
    Key { index: usize, pressed: bool },
}
pub enum UsbCmd {
    Frame {
        index: usize,
        data: Vec<u8>,
    },
    Reconnect,
    /// Muda o brilho (0 a 100) sem reiniciar o aparelho.
    Brightness(u8),
}

/// Índice da grade da UI (5 colunas) -> índice físico do dispositivo (0-based).
fn ui_to_device(i: usize) -> Option<u8> {
    let od = (i / UI_COLS) * DEVICE_COLS + i % UI_COLS;
    OPENDECK_TO_DEVICE.get(od).copied()
}

/// Valor de tecla do relatório (1-based) -> índice da grade da UI; `None` para a coluna lateral
/// (botões só do AKP153R; no AKP153E a lateral é uma tela sem teclas).
fn device_to_ui(d: u8) -> Option<usize> {
    let od = *DEVICE_TO_OPENDECK.get((d as usize).checked_sub(1)?)? as usize;
    (od % DEVICE_COLS < UI_COLS).then(|| (od / DEVICE_COLS) * UI_COLS + od % DEVICE_COLS)
}

fn command(payload: usize, body: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; payload + 1];
    buf[1..4].copy_from_slice(b"CRT");
    buf[CMD_OFFSET..CMD_OFFSET + body.len()].copy_from_slice(body);
    buf
}

/// Cabeçalho "BAT" seguido do JPEG em blocos do tamanho do payload; `None` se não couber em 16 bits.
fn image_packets(payload: usize, device_key: u8, jpeg: &[u8]) -> Option<Vec<Vec<u8>>> {
    let len = u16::try_from(jpeg.len()).ok()?.to_be_bytes();
    let mut out = vec![command(
        payload,
        &[b'B', b'A', b'T', 0, 0, len[0], len[1], device_key + 1],
    )];
    for chunk in jpeg.chunks(payload) {
        let mut buf = vec![0u8; payload + 1];
        buf[1..1 + chunk.len()].copy_from_slice(chunk);
        out.push(buf);
    }
    Some(out)
}

fn write_init(dev: &HidDevice, p: Profile) -> Result<(), HidError> {
    let n = p.payload;
    // CLE 0xFF apaga as imagens de todas as teclas; sem isso, restos de imagens antigas ficam nas margens.
    let clear_all = [b'C', b'L', b'E', 0, 0, 0, CLEAR_ALL_KEYS];
    for pkt in [
        command(n, b"DIS"),
        command(n, &[b'L', b'I', b'G', 0, 0, 0, 0]),
        command(n, &clear_all),
        command(n, b"STP"),
        brightness_command(n, BRIGHTNESS_LEVEL.load(Ordering::Relaxed)),
    ] {
        dev.write(&pkt)?;
    }
    Ok(())
}

fn open(api: &mut HidApi) -> Result<(HidDevice, Profile), String> {
    api.refresh_devices().map_err(|e| e.to_string())?;
    let mut cands: Vec<_> = api
        .device_list()
        .filter(|d| IDS.contains(&(d.vendor_id(), d.product_id())))
        .collect();
    cands.sort_by_key(|d| d.usage_page() != USAGE_PAGE);
    let info = cands.first().ok_or("dispositivo não encontrado")?;
    let dev = info.open_device(api).map_err(|e| e.to_string())?;
    let mut last_err = String::from("nenhum perfil de protocolo");
    for profile in profile_candidates(info.vendor_id(), info.product_id()) {
        match write_init(&dev, profile) {
            Ok(()) => return Ok((dev, profile)),
            Err(e) => {
                log::warn!("perfil {profile:?} recusado: {e}");
                last_err = e.to_string();
            }
        }
    }
    Err(last_err)
}

fn send_image(
    dev: &HidDevice,
    profile: Profile,
    ui_index: usize,
    jpeg: &[u8],
) -> Result<bool, HidError> {
    let Some(key) = ui_to_device(ui_index) else {
        return Ok(false);
    };
    let Some(packets) = image_packets(profile.payload, key, jpeg) else {
        log::warn!("imagem do botão {ui_index} excede 64 KiB");
        return Ok(false);
    };
    for p in &packets {
        dev.write(p)?;
    }
    log::info!(
        "botão {ui_index} -> tecla {key}: {} bytes em {} pacote(s) de {}",
        jpeg.len(),
        packets.len(),
        profile.payload
    );
    Ok(true)
}

/// Interpreta um relatório "ACK": (tecla da UI, pressionada) quando for tecla da grade.
/// Com `both_states` (v3) o byte de estado é obrigatório: relatório curto é ignorado, pois
/// tratá-lo como aperto deixaria a tecla "presa" até o watchdog. Sem ele (v1) só o aperto vem
/// e `pressed` não é usado.
fn parse_ack(report: &[u8], both_states: bool) -> Option<(usize, bool)> {
    if report.len() <= ACK_KEY_OFFSET || report[..3] != *b"ACK" {
        return None;
    }
    let pressed = match report.get(STATE_OFFSET) {
        Some(&state) => state != 0,
        None if both_states => return None,
        None => true,
    };
    device_to_ui(report[ACK_KEY_OFFSET]).map(|i| (i, pressed))
}

fn read_key(
    dev: &HidDevice,
    buf: &mut [u8; MAX_PAYLOAD],
    both_states: bool,
) -> Result<Option<(usize, bool)>, HidError> {
    let n = dev.read_timeout(buf, READ_TIMEOUT_MS)?;
    Ok(parse_ack(&buf[..n], both_states))
}

/// Teclas da grade da UI (3 x 5).
const UI_KEYS: usize = 15;

/// Comandos acumulados desde o último ciclo. Uma rajada de "reload" enfileira 15 JPEGs por vez;
/// aqui só o quadro mais recente de cada tecla vai ao aparelho.
struct Pending {
    reconnect: bool,
    brightness: Option<u8>,
    frames: [Option<Vec<u8>>; UI_KEYS],
}

fn collect_commands(cmds: impl Iterator<Item = UsbCmd>) -> Pending {
    let mut pending = Pending {
        reconnect: false,
        brightness: None,
        frames: std::array::from_fn(|_| None),
    };
    for cmd in cmds {
        match cmd {
            UsbCmd::Reconnect => {
                pending.reconnect = true;
                break;
            }
            UsbCmd::Frame { index, data } => {
                if let Some(slot) = pending.frames.get_mut(index) {
                    *slot = Some(data);
                }
            }
            UsbCmd::Brightness(level) => pending.brightness = Some(level),
        }
    }
    pending
}

/// Envia o "soltou" das teclas seguradas além do limite do watchdog e as libera.
fn expire_held(down: &mut [Option<Instant>; UI_KEYS], tx: &Sender<UsbEvent>) {
    for (index, slot) in down.iter_mut().enumerate() {
        if slot.is_some_and(|since| since.elapsed() >= HOLD_WATCHDOG) {
            let _ = tx.send(UsbEvent::Key {
                index,
                pressed: false,
            });
            *slot = None;
        }
    }
}

/// Thread de hardware: abre o HID, reconecta com backoff (0,5 s a 8 s) e despacha comandos/eventos.
/// O `HidApi` da thread de hardware, criado sob demanda: uma falha na criação não encerra a thread.
fn hid_api(api: &mut Option<HidApi>) -> Result<&mut HidApi, String> {
    if api.is_none() {
        *api = Some(HidApi::new().map_err(|e| format!("hidapi: {e}"))?);
    }
    api.as_mut()
        .ok_or_else(|| "hidapi indisponível".to_string())
}

pub fn spawn(tx: Sender<UsbEvent>, rx: Receiver<UsbCmd>) {
    std::thread::spawn(move || {
        let mut api: Option<HidApi> = None;
        let mut backoff = Duration::from_millis(500);
        loop {
            // Se o HidApi não subir (ou o aparelho não estiver), tenta de novo com espera crescente.
            let opened = hid_api(&mut api).and_then(open);
            let (dev, profile) = match opened {
                Ok(d) => d,
                Err(e) => {
                    log::debug!("sem dispositivo: {e}");
                    let _ = tx.send(UsbEvent::Status(false));
                    std::thread::sleep(backoff);
                    backoff = (backoff * 2).min(Duration::from_secs(8));
                    while rx.try_recv().is_ok() {}
                    continue;
                }
            };
            backoff = Duration::from_millis(500);
            let _ = tx.send(UsbEvent::Info {
                icon_px: profile.icon_px,
            });
            let _ = tx.send(UsbEvent::Status(true));
            if run_connection(&dev, profile, &tx, &rx).is_err() {
                log::warn!("conexão perdida");
            }
            let _ = tx.send(UsbEvent::Status(false));
        }
    });
}

fn run_connection(
    dev: &HidDevice,
    profile: Profile,
    tx: &Sender<UsbEvent>,
    rx: &Receiver<UsbCmd>,
) -> Result<(), HidError> {
    let mut buf = [0u8; MAX_PAYLOAD];
    let mut last_keepalive = Instant::now();
    let mut down = [None; UI_KEYS];
    loop {
        let mut flush = false;
        let pending = collect_commands(std::iter::from_fn(|| rx.try_recv().ok()));
        if pending.reconnect {
            return Ok(());
        }
        if let Some(level) = pending.brightness {
            set_brightness(level);
            dev.write(&brightness_command(profile.payload, level))?;
        }
        for (index, data) in pending.frames.iter().enumerate() {
            if let Some(data) = data {
                flush |= send_image(dev, profile, index, data)?;
            }
        }
        if flush {
            dev.write(&command(profile.payload, b"STP"))?;
        }
        if last_keepalive.elapsed() >= KEEPALIVE {
            dev.write(&command(profile.payload, b"CONNECT"))?;
            last_keepalive = Instant::now();
        }
        expire_held(&mut down, tx);
        let Some((index, pressed)) = read_key(dev, &mut buf, profile.both_states)? else {
            continue;
        };
        if profile.both_states {
            let _ = tx.send(UsbEvent::Key { index, pressed });
            if let Some(slot) = down.get_mut(index) {
                *slot = pressed.then(Instant::now);
            }
        } else {
            // Protocolo v1 não envia soltura: gera press + release.
            let _ = tx.send(UsbEvent::Key {
                index,
                pressed: true,
            });
            let _ = tx.send(UsbEvent::Key {
                index,
                pressed: false,
            });
        }
    }
}

/// Diagnóstico para validar o protocolo no hardware real. Imprime e grava `akp153e_probe.txt`
/// (necessário porque a build de release não tem console).
pub fn probe() {
    let mut rep = String::new();
    let mut say = |l: String| {
        println!("{l}");
        rep.push_str(&l);
        rep.push('\n');
    };
    run_probe(&mut say);
    if let Err(e) = std::fs::write("akp153e_probe.txt", rep) {
        eprintln!("não gravou relatório: {e}");
    }
}

fn run_probe(say: &mut dyn FnMut(String)) {
    let mut api = match HidApi::new() {
        Ok(a) => a,
        Err(e) => return say(format!("hidapi: {e}")),
    };
    say("== Interfaces HID (* = conhecida) VID:PID usage_page/usage interface ==".into());
    for d in api
        .device_list()
        .filter(|d| IDS.contains(&(d.vendor_id(), d.product_id())))
    {
        say(format!(
            "* {:04X}:{:04X} {:04X}/{:X} if{}",
            d.vendor_id(),
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.interface_number()
        ));
    }
    let (dev, profile) = match open(&mut api) {
        Ok(d) => d,
        Err(e) => return say(format!("open falhou: {e}")),
    };
    say(format!("aberto; perfil {profile:?}. Imagem de teste no botão 0 e escuta por 20 s (aperte e solte botões)..."));
    let mut cfg = crate::config::schema::ButtonConfig::blank();
    cfg.appearance.background = crate::config::schema::RGBAColor {
        r: 200,
        g: 0,
        b: 0,
        a: 255,
    };
    cfg.appearance.text = "OK".into();
    let jpeg =
        crate::graphics::renderer::render_button_frame(&cfg, profile.icon_px, profile.icon_px);
    match send_image(&dev, profile, 0, &jpeg)
        .and_then(|_| dev.write(&command(profile.payload, b"STP")).map(|_| true))
    {
        Ok(_) => say("imagem enviada (botão 0 deve ficar vermelho com 'OK')".into()),
        Err(e) => say(format!("erro ao enviar: {e}")),
    }
    let (mut buf, start) = ([0u8; MAX_PAYLOAD], Instant::now());
    while start.elapsed() < Duration::from_secs(20) {
        match dev.read_timeout(&mut buf, 200) {
            Ok(0) => {}
            Ok(n) => say(format!(
                "IN[{n}] {}",
                buf[..n.min(24)]
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
            Err(e) => return say(format!("erro de leitura: {e}")),
        }
    }
    say("fim".into());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_linux_udev_rule_covers_every_known_device() {
        let rules = include_str!("../../packaging/linux/99-akp153e.rules").to_lowercase();
        for (vid, pid) in IDS {
            for subsystem in ["hidraw", "usb"] {
                let line = format!(
                    "subsystem==\"{subsystem}\", attrs{{idvendor}}==\"{vid:04x}\", attrs{{idproduct}}==\"{pid:04x}\""
                );
                assert!(rules.contains(&line), "falta na regra udev: {line}");
            }
        }
    }

    #[test]
    fn key_maps_are_inverse_permutations() {
        for (od, &d) in OPENDECK_TO_DEVICE.iter().enumerate() {
            assert_eq!(DEVICE_TO_OPENDECK[d as usize] as usize, od);
        }
    }

    #[test]
    fn ui_keys_roundtrip_through_device_values() {
        for i in 0..15 {
            assert_eq!(device_to_ui(ui_to_device(i).unwrap() + 1), Some(i));
        }
        assert_eq!(ui_to_device(15), None);
    }

    #[test]
    fn side_keys_and_invalid_values_are_ignored() {
        assert_eq!(device_to_ui(0), None);
        assert_eq!(device_to_ui(19), None);
        assert_eq!(device_to_ui(16), None); // opendeck 5 = coluna lateral
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
        assert_eq!(parse_ack(&ack(1, None), true), None);
        assert_eq!(parse_ack(&ack(1, None), false), Some((4, true)));
    }

    #[test]
    fn ack_state_byte_gives_press_and_release() {
        assert_eq!(parse_ack(&ack(1, Some(1)), true), Some((4, true)));
        assert_eq!(parse_ack(&ack(1, Some(0)), true), Some((4, false)));
        assert_eq!(parse_ack(b"NAK\0\0\0\0\0\0\x01\x01", true), None);
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
}
