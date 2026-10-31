//! Dados ao vivo para as teclas: volume geral, o que está tocando e uso do sistema. O serviço e o
//! editor pedem só o que a página ativa usa (`Wants`), a cada poucos segundos.
//!
//! Linux: `pactl` (volume), MPRIS pelo `busctl` do systemd (tocando agora; sem `playerctl`),
//! `/proc` e `hwmon` (CPU, RAM, temperatura). Windows: Core Audio (volume) e as APIs do sistema
//! (CPU, RAM); tocando agora e temperatura não existem lá sem dependências novas.
use std::collections::HashMap;

/// O que a página usa: só isso é consultado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wants {
    pub volume: bool,
    pub playing: bool,
    pub system: bool,
}

impl Wants {
    pub fn any(self) -> bool {
        self.volume || self.playing || self.system
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    pub volume: Option<Volume>,
    pub playing: Option<NowPlaying>,
    pub system: Option<SystemStats>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    /// 0 a 100 (pode passar de 100 com ganho extra no Linux).
    pub percent: u16,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    /// Capa já em PNG na pasta de ícones (baixada uma vez por endereço).
    pub art: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemStats {
    /// Uso da CPU desde a consulta anterior, 0 a 100.
    pub cpu: u8,
    /// Memória em uso, 0 a 100.
    pub ram: u8,
    /// Temperatura da CPU em °C, quando o sistema informa.
    pub temp: Option<i16>,
}

/// Guarda o que a próxima consulta precisa: os tempos da CPU e as capas já baixadas.
#[derive(Default)]
pub struct Sampler {
    cpu_before: Option<(u64, u64)>,
    art: HashMap<String, Option<String>>,
}

/// Maior capa baixada (as do Spotify têm ~60 KB).
#[cfg(not(windows))]
const MAX_ART_BYTES: u64 = 2 * 1024 * 1024;
#[cfg(not(windows))]
const ART_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

impl Sampler {
    pub fn sample(&mut self, wants: Wants) -> Live {
        Live {
            volume: wants.volume.then(read_volume).flatten(),
            playing: if wants.playing {
                self.now_playing()
            } else {
                None
            },
            system: wants.system.then(|| self.system()).flatten(),
        }
    }

    fn system(&mut self) -> Option<SystemStats> {
        let (busy, total) = cpu_times()?;
        let cpu = match self.cpu_before.replace((busy, total)) {
            Some((b0, t0)) if total > t0 => {
                ((busy.saturating_sub(b0)) * 100 / (total - t0)).min(100) as u8
            }
            _ => 0,
        };
        Some(SystemStats {
            cpu,
            ram: ram_percent()?,
            temp: cpu_temp(),
        })
    }

    #[cfg(windows)]
    fn now_playing(&mut self) -> Option<NowPlaying> {
        None
    }

    #[cfg(not(windows))]
    fn now_playing(&mut self) -> Option<NowPlaying> {
        let (mut playing, art_url) = mpris::current()?;
        if let Some(url) = art_url {
            let cached = self
                .art
                .entry(url.clone())
                .or_insert_with(|| fetch_art(&url));
            playing.art = cached.clone();
        }
        Some(playing)
    }
}

fn read_volume() -> Option<Volume> {
    let (level, muted) = super::audio::master_volume()?;
    Some(Volume {
        percent: (level * 100.0).round().clamp(0.0, 999.0) as u16,
        muted,
    })
}

/// A capa local (`file://`) ou baixada, convertida para o PNG da pasta de ícones.
#[cfg(not(windows))]
fn fetch_art(url: &str) -> Option<String> {
    use std::io::Read;
    let bytes = if let Some(path) = url.strip_prefix("file://") {
        std::fs::read(percent_decode(path)).ok()?
    } else if url.starts_with("https://") || url.starts_with("http://") {
        let resp = reqwest::blocking::Client::builder()
            .timeout(ART_TIMEOUT)
            .build()
            .ok()?
            .get(url)
            .send()
            .ok()?
            .error_for_status()
            .ok()?;
        let mut bytes = Vec::new();
        resp.take(MAX_ART_BYTES).read_to_end(&mut bytes).ok()?;
        bytes
    } else {
        return None;
    };
    super::save_icon(url, &bytes)
}

/// `%20` e companhia nos caminhos `file://` do MPRIS.
#[cfg(not(windows))]
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// (tempo ocupado, tempo total) da CPU desde o boot.
#[cfg(not(windows))]
fn cpu_times() -> Option<(u64, u64)> {
    parse_proc_stat(&std::fs::read_to_string("/proc/stat").ok()?)
}

#[cfg(not(windows))]
fn parse_proc_stat(text: &str) -> Option<(u64, u64)> {
    let fields: Vec<u64> = text
        .lines()
        .next()?
        .strip_prefix("cpu ")?
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    // user nice system idle iowait irq softirq steal...: ocioso = idle + iowait.
    let idle = fields.get(3)? + fields.get(4).copied().unwrap_or(0);
    let total: u64 = fields.iter().take(8).sum();
    Some((total.saturating_sub(idle), total))
}

#[cfg(not(windows))]
fn ram_percent() -> Option<u8> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

#[cfg(not(windows))]
fn parse_meminfo(text: &str) -> Option<u8> {
    let value = |key: &str| -> Option<u64> {
        text.lines()
            .find_map(|l| l.strip_prefix(key))?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    };
    let total = value("MemTotal:")?;
    let available = value("MemAvailable:")?;
    (total > 0).then(|| ((total.saturating_sub(available)) * 100 / total) as u8)
}

/// Sensores de CPU conhecidos do `hwmon` (AMD, Intel, Raspberry Pi, ACPI), em ordem de preferência.
#[cfg(not(windows))]
const CPU_SENSORS: [&str; 5] = ["k10temp", "coretemp", "zenpower", "cpu_thermal", "acpitz"];

#[cfg(not(windows))]
fn cpu_temp() -> Option<i16> {
    let sensors: Vec<(String, std::path::PathBuf)> = std::fs::read_dir("/sys/class/hwmon")
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = std::fs::read_to_string(e.path().join("name")).ok()?;
            Some((name.trim().to_string(), e.path()))
        })
        .collect();
    CPU_SENSORS.iter().find_map(|wanted| {
        let (_, dir) = sensors.iter().find(|(name, _)| name == wanted)?;
        let milli: i32 = std::fs::read_to_string(dir.join("temp1_input"))
            .ok()?
            .trim()
            .parse()
            .ok()?;
        i16::try_from(milli / 1000).ok()
    })
}

#[cfg(windows)]
fn cpu_times() -> Option<(u64, u64)> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::GetSystemTimes;
    let (mut idle, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: os três ponteiros apontam para variáveis locais válidas durante a chamada.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    // O tempo de kernel já inclui o ocioso.
    let total = ticks(kernel) + ticks(user);
    Some((total.saturating_sub(ticks(idle)), total))
}

#[cfg(windows)]
fn ram_percent() -> Option<u8> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` é local, com `dwLength` preenchido como a API exige.
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    u8::try_from(status.dwMemoryLoad).ok()
}

#[cfg(windows)]
fn cpu_temp() -> Option<i16> {
    None
}

/// MPRIS (o padrão de players do Linux) lido pelo `busctl` do systemd, sem o `playerctl`.
#[cfg(not(windows))]
mod mpris {
    use super::NowPlaying;
    use serde_json::Value;
    use std::process::Command;

    const PREFIX: &str = "org.mpris.MediaPlayer2.";
    const PATH: &str = "/org/mpris/MediaPlayer2";
    const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

    fn busctl(args: &[&str]) -> Option<Value> {
        let out = Command::new("busctl")
            .args(["--user", "--json=short"])
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| serde_json::from_slice(&out.stdout).ok())
            .flatten()
    }

    /// Players no barramento (Spotify, navegador, mpv com o plugin...).
    fn players() -> Vec<String> {
        busctl(&["list", "--no-pager"])
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|e| e.get("name")?.as_str().map(str::to_string))
            .filter(|n| n.starts_with(PREFIX))
            .collect()
    }

    fn status(player: &str) -> Option<String> {
        busctl(&["get-property", player, PATH, PLAYER, "PlaybackStatus"])?
            .get("data")?
            .as_str()
            .map(str::to_string)
    }

    /// O que toca agora (prefere um player tocando a um pausado) e o endereço da capa.
    pub fn current() -> Option<(NowPlaying, Option<String>)> {
        let mut found: Vec<(String, String)> = players()
            .into_iter()
            .filter_map(|p| Some((status(&p)?, p)))
            .filter(|(s, _)| s != "Stopped")
            .collect();
        found.sort_by_key(|(s, _)| s != "Playing");
        let (state, player) = found.into_iter().next()?;
        let metadata = busctl(&["get-property", &player, PATH, PLAYER, "Metadata"])?;
        parse_metadata(&metadata, state == "Playing")
    }

    /// Lê o `a{sv}` do Metadata no JSON do busctl.
    pub fn parse_metadata(v: &Value, playing: bool) -> Option<(NowPlaying, Option<String>)> {
        let data = v.get("data")?;
        let field = |key: &str| data.get(key)?.get("data");
        let title = field("xesam:title")?.as_str()?.trim().to_string();
        let artist = field("xesam:artist")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let art = field("mpris:artUrl")
            .and_then(Value::as_str)
            .filter(|u| !u.is_empty())
            .map(str::to_string);
        (!title.is_empty()).then(|| {
            (
                NowPlaying {
                    title,
                    artist,
                    playing,
                    art: None,
                },
                art,
            )
        })
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn cpu_and_ram_come_from_proc() {
        let stat = "cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 1 2 3";
        assert_eq!(parse_proc_stat(stat), Some((150, 1000)));
        let mem = "MemTotal:       1000 kB\nMemFree: 1 kB\nMemAvailable:    250 kB\n";
        assert_eq!(parse_meminfo(mem), Some(75));
        assert_eq!(parse_meminfo("lixo"), None);
    }

    #[test]
    fn cpu_use_is_measured_between_two_samples() {
        let mut s = Sampler::default();
        let first = s.sample(Wants {
            system: true,
            ..Wants::default()
        });
        let stats = first.system.expect("o /proc existe no Linux");
        assert_eq!(
            stats.cpu, 0,
            "a primeira consulta não tem com o que comparar"
        );
        assert!(stats.ram > 0 && stats.ram <= 100);
        let nothing = s.sample(Wants::default());
        assert_eq!(nothing, Live::default(), "sem pedido, nada é consultado");
    }

    #[test]
    fn mpris_metadata_is_read_from_busctl_json() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"type":"a{sv}","data":{
                "xesam:title":{"type":"s","data":"Numb"},
                "xesam:artist":{"type":"as","data":["Linkin Park"]},
                "mpris:artUrl":{"type":"s","data":"https://i.scdn.co/image/abc"}}}"#,
        )
        .unwrap();
        let (now, art) = mpris::parse_metadata(&json, true).unwrap();
        assert_eq!(
            (now.title.as_str(), now.artist.as_str(), now.playing),
            ("Numb", "Linkin Park", true)
        );
        assert_eq!(art.as_deref(), Some("https://i.scdn.co/image/abc"));
        let empty: serde_json::Value =
            serde_json::from_str(r#"{"type":"a{sv}","data":{}}"#).unwrap();
        assert!(mpris::parse_metadata(&empty, false).is_none());
    }

    #[test]
    fn file_art_paths_are_decoded() {
        assert_eq!(
            percent_decode("/home/a/Capa%20Nova.jpg"),
            "/home/a/Capa Nova.jpg"
        );
        assert_eq!(percent_decode("100%"), "100%");
    }
}
