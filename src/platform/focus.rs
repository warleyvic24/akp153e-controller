//! App em primeiro plano, para a troca automática de perfil. Manda o identificador do app por
//! `tx` sempre que o foco muda: no Windows o nome do `.exe` (ex.: `obs64.exe`); no Linux a classe
//! da janela (ex.: `com.obsproject.Studio`, `firefox`), lida do Hyprland (socket de eventos), do
//! Sway (`swaymsg -t subscribe`) ou do X11 (`xprop`, consultado a cada `POLL`). GNOME e KDE no
//! Wayland não expõem a janela ativa a outros programas: lá a troca automática fica desligada.
use std::sync::mpsc::Sender;
use std::time::Duration;

/// Intervalo da consulta no Windows e no X11 (sem evento de foco que dê para assinar sem gancho).
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

/// De onde vem o app em foco nesta sessão do Linux.
#[cfg(not(windows))]
#[derive(Debug, PartialEq)]
pub enum Source {
    Hyprland,
    Sway,
    X11,
}

/// A sessão gráfica atual, pelas variáveis de ambiente; `None` onde não há como saber o foco.
#[cfg(not(windows))]
pub fn source() -> Option<Source> {
    source_from(|name| std::env::var_os(name).is_some_and(|v| !v.is_empty()))
}

#[cfg(not(windows))]
fn source_from(has: impl Fn(&str) -> bool) -> Option<Source> {
    if has("HYPRLAND_INSTANCE_SIGNATURE") {
        Some(Source::Hyprland)
    } else if has("SWAYSOCK") {
        Some(Source::Sway)
    } else if has("DISPLAY") && !has("WAYLAND_DISPLAY") {
        Some(Source::X11)
    } else {
        None
    }
}

#[cfg(not(windows))]
fn watch(tx: Sender<String>) {
    match source() {
        Some(Source::Hyprland) => watch_hyprland(&tx),
        Some(Source::Sway) => watch_sway(&tx),
        Some(Source::X11) => watch_x11(&tx),
        None => log::info!("troca automática de perfil: sessão sem janela ativa visível"),
    }
}

/// Sway: assina os eventos de janela e manda o app de cada `focus`; reconecta se o swaymsg sair.
#[cfg(not(windows))]
fn watch_sway(tx: &Sender<String>) {
    use std::io::BufRead;
    loop {
        let child = std::process::Command::new("swaymsg")
            .args(["-t", "subscribe", "-m", "[\"window\"]"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            log::warn!("troca automática de perfil: swaymsg indisponível");
            return;
        };
        if let Some(out) = child.stdout.take() {
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(app) = sway_focused_app(&line) {
                    if tx.send(app).is_err() {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
        }
        let _ = child.wait();
        std::thread::sleep(RECONNECT);
    }
}

/// App de um evento de janela do Sway: `app_id` (Wayland) ou a classe X11 (XWayland), só no `focus`.
#[cfg(not(windows))]
fn sway_focused_app(line: &str) -> Option<String> {
    let event: serde_json::Value = serde_json::from_str(line).ok()?;
    if event.get("change")?.as_str()? != "focus" {
        return None;
    }
    let container = event.get("container")?;
    container
        .get("app_id")
        .and_then(|v| v.as_str())
        .or_else(|| container.pointer("/window_properties/class")?.as_str())
        .filter(|a| !a.is_empty())
        .map(str::to_string)
}

/// X11: pergunta a janela ativa ao `xprop` a cada `POLL` e manda a classe quando ela muda.
#[cfg(not(windows))]
fn watch_x11(tx: &Sender<String>) {
    let mut last = String::new();
    loop {
        match x11_active_class() {
            Err(e) => {
                log::warn!("troca automática de perfil: {e}");
                return;
            }
            Ok(Some(app)) if app != last => {
                last = app.clone();
                if tx.send(app).is_err() {
                    return;
                }
            }
            Ok(_) => {}
        }
        std::thread::sleep(POLL);
    }
}

/// Classe da janela ativa no X11; `Err` se o `xprop` não existir.
#[cfg(not(windows))]
fn x11_active_class() -> Result<Option<String>, String> {
    let xprop = |args: &[&str]| {
        std::process::Command::new("xprop")
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .map_err(|e| format!("xprop indisponível: {e}"))
    };
    let root = xprop(&["-root", "_NET_ACTIVE_WINDOW"])?;
    let Some(id) = x11_window_id(&root) else {
        return Ok(None);
    };
    Ok(x11_class(&xprop(&["-id", &id, "WM_CLASS"])?))
}

/// Id da janela em `_NET_ACTIVE_WINDOW(WINDOW): window id # 0x3a00007` (0x0 = nenhuma).
#[cfg(not(windows))]
fn x11_window_id(text: &str) -> Option<String> {
    let id = text.rsplit('#').next()?.trim().split(',').next()?.trim();
    (id.starts_with("0x") && id != "0x0").then(|| id.to_string())
}

/// Classe (o segundo nome) em `WM_CLASS(STRING) = "Navigator", "firefox"`.
#[cfg(not(windows))]
fn x11_class(text: &str) -> Option<String> {
    let names: Vec<&str> = text.split('"').skip(1).step_by(2).collect();
    names
        .get(1)
        .or(names.first())
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string())
}

#[cfg(not(windows))]
fn watch_hyprland(tx: &Sender<String>) {
    let Some(signature) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") else {
        return;
    };
    loop {
        if let Some(app) = hyprland_active_class() {
            if tx.send(app).is_err() {
                return;
            }
        }
        match hyprland_events(&signature, tx) {
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
    fn the_session_picks_the_focus_source() {
        let with = |vars: &'static [&'static str]| move |name: &str| vars.contains(&name);
        assert_eq!(
            source_from(with(&["HYPRLAND_INSTANCE_SIGNATURE", "WAYLAND_DISPLAY"])),
            Some(Source::Hyprland)
        );
        assert_eq!(
            source_from(with(&["SWAYSOCK", "WAYLAND_DISPLAY"])),
            Some(Source::Sway)
        );
        assert_eq!(source_from(with(&["DISPLAY"])), Some(Source::X11));
        assert_eq!(
            source_from(with(&["DISPLAY", "WAYLAND_DISPLAY"])),
            None,
            "GNOME/KDE Wayland"
        );
        assert_eq!(source_from(with(&[])), None);
    }

    #[test]
    fn sway_focus_events_give_the_app_id_or_the_x11_class() {
        let native = r#"{"change":"focus","container":{"app_id":"foot","window_properties":null}}"#;
        assert_eq!(sway_focused_app(native).as_deref(), Some("foot"));
        let xwayland = r#"{"change":"focus","container":{"app_id":null,"window_properties":{"class":"Steam"}}}"#;
        assert_eq!(sway_focused_app(xwayland).as_deref(), Some("Steam"));
        let title = r#"{"change":"title","container":{"app_id":"foot"}}"#;
        assert_eq!(sway_focused_app(title), None);
        assert_eq!(sway_focused_app("lixo"), None);
    }

    #[test]
    fn x11_output_is_parsed() {
        assert_eq!(
            x11_window_id("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x3a00007").as_deref(),
            Some("0x3a00007")
        );
        assert_eq!(
            x11_window_id("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0"),
            None
        );
        assert_eq!(
            x11_class("WM_CLASS(STRING) = \"Navigator\", \"firefox\"").as_deref(),
            Some("firefox")
        );
        assert_eq!(
            x11_class("WM_CLASS(STRING) = \"obs\"").as_deref(),
            Some("obs")
        );
        assert_eq!(x11_class("WM_CLASS:  not found."), None);
    }

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
