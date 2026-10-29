//! Mixer de volume no Linux (PulseAudio ou PipeWire com `pipewire-pulse`): lê e muda os
//! "sink-inputs" (um por fluxo de áudio de cada app) com o comando `pactl`.
//! Aqui o "pid" de cada sessão é o índice do sink-input, que é o que o `pactl` aceita para mudar
//! volume e mudo; o restante do programa só repassa esses números.
pub use super::audio_model::AudioSession;
use super::audio_model::{group, RawSession};
use serde_json::Value;
use std::process::Command;

type AudioResult<T> = Result<T, String>;

/// Valor de volume do PulseAudio que equivale a 100%.
const VOLUME_NORM: f64 = 65_536.0;

/// Programas com som tocando ou parado, um item por programa, em ordem estável de nome.
pub fn list_sessions() -> Vec<AudioSession> {
    match pactl(&["-f", "json", "list", "sink-inputs"]) {
        Ok(json) => group(parse_sink_inputs(&json)),
        Err(e) => {
            log::warn!("mixer: não foi possível listar os apps com som: {e}");
            Vec::new()
        }
    }
}

pub fn set_session_volume(ids: &[u32], volume: f32) -> AudioResult<()> {
    let percent = format!("{}%", (volume.clamp(0.0, 1.0) * 100.0).round());
    for id in ids {
        pactl(&["set-sink-input-volume", &id.to_string(), &percent])?;
    }
    Ok(())
}

pub fn set_session_mute(ids: &[u32], muted: bool) -> AudioResult<()> {
    let flag = if muted { "1" } else { "0" };
    for id in ids {
        pactl(&["set-sink-input-mute", &id.to_string(), flag])?;
    }
    Ok(())
}

fn pactl(args: &[&str]) -> AudioResult<String> {
    let out = Command::new("pactl")
        .args(args)
        .output()
        .map_err(|e| format!("pactl indisponível: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "pactl {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Lê a saída de `pactl -f json list sink-inputs`. Entradas malformadas são puladas.
fn parse_sink_inputs(json: &str) -> Vec<RawSession> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    items.iter().filter_map(parse_sink_input).collect()
}

fn parse_sink_input(item: &Value) -> Option<RawSession> {
    let id = u32::try_from(item.get("index")?.as_u64()?).ok()?;
    let props = item.get("properties");
    let prop = |key: &str| {
        props
            .and_then(|p| p.get(key))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    // O binário dá o nome estável ("firefox"); sem ele, o nome que o app se deu.
    let exe_path = prop("application.process.binary").or_else(|| prop("application.name"));
    // O volume é por canal; vale o maior (como o maior dos processos no Windows).
    let loudest = item
        .get("volume")?
        .as_object()?
        .values()
        .filter_map(|channel| channel.get("value")?.as_f64())
        .fold(0.0_f64, f64::max);
    Some(RawSession {
        pid: id,
        exe_path,
        volume: (loudest / VOLUME_NORM).clamp(0.0, 1.0) as f32,
        muted: item.get("mute").and_then(Value::as_bool).unwrap_or(false),
        active: !item.get("corked").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
      {"index": 41, "mute": false, "corked": false,
       "volume": {"front-left": {"value": 32768, "value_percent": "50%"},
                  "front-right": {"value": 49152, "value_percent": "75%"}},
       "properties": {"application.name": "Firefox", "application.process.binary": "firefox"}},
      {"index": 42, "mute": true, "corked": true,
       "volume": {"mono": {"value": 65536, "value_percent": "100%"}},
       "properties": {"application.name": "Spotify"}},
      {"index": "quebrado"}
    ]"#;

    #[test]
    fn reads_volume_mute_and_state_of_each_stream() {
        let sessions = parse_sink_inputs(SAMPLE);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].pid, 41);
        assert_eq!(sessions[0].exe_path.as_deref(), Some("firefox"));
        assert_eq!(sessions[0].volume, 0.75);
        assert!(!sessions[0].muted && sessions[0].active);
        assert_eq!(sessions[1].exe_path.as_deref(), Some("Spotify"));
        assert!(sessions[1].muted && !sessions[1].active);
    }

    #[test]
    fn garbage_gives_no_sessions() {
        assert!(parse_sink_inputs("").is_empty());
        assert!(parse_sink_inputs("lixo").is_empty());
        assert!(parse_sink_inputs("{}").is_empty());
    }

    #[test]
    fn streams_of_the_same_app_become_one_item() {
        let json = r#"[
          {"index": 1, "volume": {"m": {"value": 65536}}, "properties": {"application.process.binary": "firefox"}},
          {"index": 2, "volume": {"m": {"value": 32768}}, "properties": {"application.process.binary": "firefox"}}
        ]"#;
        let grouped = group(parse_sink_inputs(json));
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].pids, vec![1, 2]);
        assert_eq!(grouped[0].name, "Firefox");
    }
}
