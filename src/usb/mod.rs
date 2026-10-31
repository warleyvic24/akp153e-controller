//! Transporte HID da família AJAZZ/Mirabox (interface vendor-specific, usage page 0xFFA0): comandos
//! com prefixo "CRT", imagens JPEG. O que muda de um modelo para outro (ids, pacote, tamanho da
//! imagem, ordem das teclas, tela lateral) vem do arquivo do aparelho (`crate::device`).
use crate::device::DeviceModel;
mod driver;
use driver::{Driver, DriverResult};
use hidapi::{HidApi, HidDevice, HidError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

/// Lado da imagem até o aparelho dizer o dele (`UsbEvent::Info`).
pub const ICON_SIZE: usize = 85;
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
const ACK_KEY_OFFSET: usize = 9;
const STATE_OFFSET: usize = 10;

/// O modelo aberto e o tamanho de pacote que ele aceitou.
#[derive(Clone, Copy, PartialEq)]
struct Profile {
    payload: usize,
    both_states: bool,
    icon_px: u32,
    model: &'static DeviceModel,
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (pacote {})", self.model.id, self.payload)
    }
}

/// Perfis candidatos para o modelo, em ordem de preferência (ex.: 1024 com recuo para 512).
fn profile_candidates(vid: u16, pid: u16) -> Vec<Profile> {
    let Some(model) = crate::device::find(vid, pid) else {
        return Vec::new();
    };
    // AKP_PACKET=512|1024 força um tamanho de pacote sem recompilar.
    let forced = std::env::var("AKP_PACKET")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| [512, 1024].contains(n));
    let payloads = match forced {
        Some(n) => vec![n],
        None => model.protocol.packets.clone(),
    };
    payloads
        .into_iter()
        .map(|payload| Profile {
            payload,
            both_states: model.protocol.release_events,
            icon_px: model.image.px,
            model,
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
    /// Tela lateral: as 3 faixas (de cima para baixo) ou apagar.
    Side(SideFrame),
    Reconnect,
    /// Muda o brilho (0 a 100) sem reiniciar o aparelho.
    Brightness(u8),
}

/// O que mandar à tela lateral.
#[derive(Clone, Debug, PartialEq)]
pub enum SideFrame {
    /// JPEGs das 3 faixas, de cima para baixo (a de baixo cobre parte da do meio, que cobre a de cima).
    Strips([Vec<u8>; 3]),
    /// Apaga a faixa (CLE nas 3 posições).
    Clear,
}

/// Valor de tecla do relatório (1-based) -> tecla da página; `None` se não for tecla (ex.: a
/// lateral do AKP153E, que é uma tela sem botões).
fn device_to_ui(model: &DeviceModel, d: u8) -> Option<usize> {
    model.key_of_hw(d.checked_sub(1)?)
}

/// Índices do protocolo das partes da tela lateral, de cima para baixo (vazio se não houver).
fn side_slots(model: &DeviceModel) -> Vec<u8> {
    model
        .strip()
        .map(|s| s.slots.iter().map(|slot| slot.hw).collect())
        .unwrap_or_default()
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
    let known = crate::device::known_ids();
    let mut cands: Vec<_> = api
        .device_list()
        .filter(|d| known.contains(&(d.vendor_id(), d.product_id())))
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
    let Some(key) = profile.model.key_hw(ui_index) else {
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

/// Manda a tela lateral: as faixas na ordem (cada uma cobre parte da anterior) ou o apagar.
fn send_side(dev: &HidDevice, profile: Profile, side: &SideFrame) -> Result<bool, HidError> {
    match side {
        SideFrame::Clear => {
            for key in side_slots(profile.model) {
                dev.write(&command(
                    profile.payload,
                    &[b'C', b'L', b'E', 0, 0, 0, key + 1],
                ))?;
            }
        }
        SideFrame::Strips(strips) => {
            for (key, jpeg) in side_slots(profile.model).into_iter().zip(strips) {
                let Some(packets) = image_packets(profile.payload, key, jpeg) else {
                    log::warn!("faixa da tela lateral excede 64 KiB");
                    continue;
                };
                for p in &packets {
                    dev.write(p)?;
                }
            }
            log::info!("tela lateral: 3 faixas");
        }
    }
    Ok(true)
}

/// Interpreta um relatório "ACK": (tecla da UI, pressionada) quando for tecla da grade.
/// Com `both_states` (v3) o byte de estado é obrigatório: relatório curto é ignorado, pois
/// tratá-lo como aperto deixaria a tecla "presa" até o watchdog. Sem ele (v1) só o aperto vem
/// e `pressed` não é usado.
fn parse_ack(report: &[u8], profile: Profile) -> Option<(usize, bool)> {
    let both_states = profile.both_states;
    if report.len() <= ACK_KEY_OFFSET || report[..3] != *b"ACK" {
        return None;
    }
    let pressed = match report.get(STATE_OFFSET) {
        Some(&state) => state != 0,
        None if both_states => return None,
        None => true,
    };
    device_to_ui(profile.model, report[ACK_KEY_OFFSET]).map(|i| (i, pressed))
}

fn read_key(
    dev: &HidDevice,
    buf: &mut [u8; MAX_PAYLOAD],
    profile: Profile,
) -> Result<Option<(usize, bool)>, HidError> {
    let n = dev.read_timeout(buf, READ_TIMEOUT_MS)?;
    Ok(parse_ack(&buf[..n], profile))
}

/// Teclas da grade da UI (3 x 5).
const UI_KEYS: usize = 15;

/// Comandos acumulados desde o último ciclo. Uma rajada de "reload" enfileira 15 JPEGs por vez;
/// aqui só o quadro mais recente de cada tecla vai ao aparelho.
struct Pending {
    reconnect: bool,
    brightness: Option<u8>,
    frames: [Option<Vec<u8>>; UI_KEYS],
    side: Option<SideFrame>,
}

fn collect_commands(cmds: impl Iterator<Item = UsbCmd>) -> Pending {
    let mut pending = Pending {
        reconnect: false,
        brightness: None,
        frames: std::array::from_fn(|_| None),
        side: None,
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
            UsbCmd::Side(frame) => pending.side = Some(frame),
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
            let mut driver = Mirabox {
                dev: &dev,
                profile,
                buf: [0; MAX_PAYLOAD],
            };
            if let Err(e) = run_connection(&mut driver, &tx, &rx) {
                log::warn!("conexão perdida: {e}");
            }
            let _ = tx.send(UsbEvent::Status(false));
        }
    });
}

/// Família AJAZZ/Mirabox: um aparelho aberto e o perfil que ele aceitou.
struct Mirabox<'a> {
    dev: &'a HidDevice,
    profile: Profile,
    buf: [u8; MAX_PAYLOAD],
}

fn hid(e: HidError) -> String {
    e.to_string()
}

impl Driver for Mirabox<'_> {
    fn set_key(&mut self, key: usize, image: &[u8]) -> DriverResult<bool> {
        send_image(self.dev, self.profile, key, image).map_err(hid)
    }

    fn set_side(&mut self, side: &SideFrame) -> DriverResult<bool> {
        send_side(self.dev, self.profile, side).map_err(hid)
    }

    fn flush(&mut self) -> DriverResult<()> {
        self.dev
            .write(&command(self.profile.payload, b"STP"))
            .map(drop)
            .map_err(hid)
    }

    fn set_brightness(&mut self, level: u8) -> DriverResult<()> {
        self.dev
            .write(&brightness_command(self.profile.payload, level))
            .map(drop)
            .map_err(hid)
    }

    fn keepalive(&mut self) -> DriverResult<()> {
        self.dev
            .write(&command(self.profile.payload, b"CONNECT"))
            .map(drop)
            .map_err(hid)
    }

    fn poll(&mut self) -> DriverResult<Option<(usize, bool)>> {
        read_key(self.dev, &mut self.buf, self.profile).map_err(hid)
    }

    fn reports_release(&self) -> bool {
        self.profile.both_states
    }
}

/// Laço de uma conexão: manda o que o serviço pediu e repassa os apertos. Volta ao cair a conexão
/// (erro) ou quando o serviço pede para reconectar (`Ok`).
fn run_connection(
    dev: &mut dyn Driver,
    tx: &Sender<UsbEvent>,
    rx: &Receiver<UsbCmd>,
) -> DriverResult<()> {
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
            dev.set_brightness(level)?;
        }
        for (index, data) in pending.frames.iter().enumerate() {
            if let Some(data) = data {
                flush |= dev.set_key(index, data)?;
            }
        }
        if let Some(side) = &pending.side {
            flush |= dev.set_side(side)?;
        }
        if flush {
            dev.flush()?;
        }
        if last_keepalive.elapsed() >= KEEPALIVE {
            dev.keepalive()?;
            last_keepalive = Instant::now();
        }
        expire_held(&mut down, tx);
        let Some((index, pressed)) = dev.poll()? else {
            continue;
        };
        if dev.reports_release() {
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

/// Diagnóstico para validar o protocolo no hardware real. Imprime e grava `keyforge_probe.txt`
/// (necessário porque a build de release não tem console).
pub fn probe() {
    let mut rep = String::new();
    let mut say = |l: String| {
        println!("{l}");
        rep.push_str(&l);
        rep.push('\n');
    };
    run_probe(&mut say);
    if let Err(e) = std::fs::write("keyforge_probe.txt", rep) {
        eprintln!("não gravou relatório: {e}");
    }
}

fn run_probe(say: &mut dyn FnMut(String)) {
    let mut api = match HidApi::new() {
        Ok(a) => a,
        Err(e) => return say(format!("hidapi: {e}")),
    };
    say("== Interfaces HID (* = conhecida) VID:PID usage_page/usage interface ==".into());
    let known = crate::device::known_ids();
    for d in api
        .device_list()
        .filter(|d| known.contains(&(d.vendor_id(), d.product_id())))
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
mod tests;
