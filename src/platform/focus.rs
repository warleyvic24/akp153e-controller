//! App em primeiro plano, para a troca automática de perfil. Manda o identificador do app por
//! `tx` sempre que o foco muda: no Windows o nome do `.exe` (ex.: `obs64.exe`); no Hyprland a
//! classe da janela (ex.: `com.obsproject.Studio`, `firefox`). Em outras sessões do Linux (X11,
//! GNOME, KDE) não há fonte confiável sem dependências novas: a troca automática fica desligada.
use std::sync::mpsc::Sender;
use std::time::Duration;

/// Intervalo da consulta no Windows (lá não há evento de foco sem um gancho de janela).
#[cfg(windows)]
const POLL: Duration = Duration::from_millis(750);
/// Espera antes de reconectar ao Hyprland (o compositor reiniciou ou o socket caiu).
#[cfg(not(windows))]
const RECONNECT: Duration = Duration::from_secs(3);

/// Inicia a thread que observa o foco; não faz nada onde o foco não pode ser observado.
pub fn spawn_watch(tx: Sender<String>) {
    std::thread::spawn(move || watch(tx));
}

#[cfg(windows)]
fn watch(tx: Sender<String>) {
    let mut last = String::new();
    loop {
        if let Some(app) = foreground_exe().filter(|a| *a != last) {
            last = app.clone();
            if tx.send(app).is_err() {
                return;
            }
        }
        std::thread::sleep(POLL);
    }
}

/// Nome do `.exe` da janela em primeiro plano.
#[cfg(windows)]
fn foreground_exe() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    // SAFETY: as duas chamadas só leem o identificador da janela e o número do processo dela;
    // `pid` vive durante a chamada.
    let pid = unsafe {
        let window = GetForegroundWindow();
        if window.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, Some(&mut pid));
        pid
    };
    let path = super::audio::process_path(pid)?;
    std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
}

#[cfg(not(windows))]
fn watch(tx: Sender<String>) {
    let Some(signature) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") else {
        log::info!("troca automática de perfil: só no Hyprland e no Windows");
        return;
    };
    loop {
        if let Some(app) = hyprland_active_class() {
            if tx.send(app).is_err() {
                return;
            }
        }
        match hyprland_events(&signature, &tx) {
            Err(Stop) => return,
            Ok(()) => std::thread::sleep(RECONNECT),
        }
    }
}

/// O canal do serviço fechou: a thread termina.
#[cfg(not(windows))]
struct Stop;

/// Classe da janela ativa agora (`hyprctl activewindow -j`), para o estado inicial.
#[cfg(not(windows))]
fn hyprland_active_class() -> Option<String> {
    let out = std::process::Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    json.get("class")?
        .as_str()
        .filter(|c| !c.is_empty())
        .map(str::to_string)
}

/// Lê os eventos do Hyprland (socket2) até a conexão cair; cada `activewindow>>` vira um envio.
#[cfg(not(windows))]
fn hyprland_events(signature: &std::ffi::OsStr, tx: &Sender<String>) -> Result<(), Stop> {
    use std::io::BufRead;
    let Some(stream) = hyprland_socket_paths(signature)
        .into_iter()
        .find_map(|p| std::os::unix::net::UnixStream::connect(p).ok())
    else {
        return Ok(());
    };
    for line in std::io::BufReader::new(stream).lines() {
        let Ok(line) = line else {
            return Ok(());
        };
        if let Some(class) = active_window_class(&line) {
            tx.send(class.to_string()).map_err(|_| Stop)?;
        }
    }
    Ok(())
}

/// Onde fica o socket de eventos: o Hyprland atual usa `$XDG_RUNTIME_DIR/hypr`; os antigos, `/tmp/hypr`.
#[cfg(not(windows))]
fn hyprland_socket_paths(signature: &std::ffi::OsStr) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        paths.push(std::path::Path::new(&runtime).join("hypr").join(signature));
    }
    paths.push(std::path::Path::new("/tmp/hypr").join(signature));
    paths
        .into_iter()
        .map(|dir| dir.join(".socket2.sock"))
        .collect()
}

/// Classe da linha `activewindow>>classe,título`; `None` para outros eventos ou sem janela.
#[cfg(not(windows))]
fn active_window_class(line: &str) -> Option<&str> {
    let (class, _title) = line.strip_prefix("activewindow>>")?.split_once(',')?;
    (!class.is_empty()).then_some(class)
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn only_active_window_events_carry_a_class() {
        assert_eq!(
            active_window_class("activewindow>>com.obsproject.Studio,OBS 31 - Cenas"),
            Some("com.obsproject.Studio")
        );
        assert_eq!(
            active_window_class("activewindow>>firefox,a, b, c"),
            Some("firefox")
        );
        assert_eq!(active_window_class("activewindow>>,"), None);
        assert_eq!(active_window_class("activewindowv2>>55d1a0"), None);
        assert_eq!(active_window_class("workspace>>2"), None);
    }
}
