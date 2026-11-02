//! Integração com o sistema: instância única, controle entre serviço e editor (TCP local),
//! bandeja, laço de eventos e início automático. Windows e Linux diferem só nos trechos com `cfg`.
use super::hide_console;
use crate::error::ShellError;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command;
use std::time::Duration;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const WINDOW_TITLE: &str = "Keyforge";
/// Texto da opção "iniciar com o sistema" (passa por `strings::t`).
#[cfg(windows)]
pub const AUTOSTART_LABEL: &str = "Iniciar com o Windows";
#[cfg(not(windows))]
pub const AUTOSTART_LABEL: &str = "Iniciar com o sistema";
/// Identificador da janela no Wayland (`app_id`); igual ao nome do `.desktop` e do binário.
pub const APP_ID: &str = "keyforge";
const SERVICE_ADDR: &str = "127.0.0.1:47153";
const EDITOR_ADDR: &str = "127.0.0.1:47154";
const IPC_TIMEOUT: Duration = Duration::from_millis(400);
#[cfg(windows)]
const AUTOSTART_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const AUTOSTART_VALUE: &str = "Keyforge";
/// Nome do início automático antes de o programa se chamar Keyforge.
#[cfg(windows)]
const LEGACY_AUTOSTART_VALUE: &str = "AKP153E";
#[cfg(not(windows))]
const AUTOSTART_FILE: &str = "keyforge.desktop";
#[cfg(not(windows))]
const LEGACY_AUTOSTART_FILE: &str = "akp153e.desktop";
const TRAY_ICON_PX: u32 = 32;
#[cfg(windows)]
const SW_SHOW: i32 = 5;
#[cfg(windows)]
const SW_RESTORE: i32 = 9;

#[cfg(windows)]
mod ffi {
    #[repr(C)]
    pub struct Msg {
        pub hwnd: isize,
        pub message: u32,
        pub wparam: usize,
        pub lparam: isize,
        pub time: u32,
        pub pt_x: i32,
        pub pt_y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn FindWindowW(class: *const u16, title: *const u16) -> isize;
        pub fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        pub fn SetForegroundWindow(hwnd: isize) -> i32;
        pub fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        pub fn TranslateMessage(msg: *const Msg) -> i32;
        pub fn DispatchMessageW(msg: *const Msg) -> isize;
    }
}

/// Traz a janela do editor (já aberta) para a frente.
#[cfg(not(windows))]
pub fn show_window() {
    // Não há API de janelas portável entre X11 e Wayland. No Hyprland (Omarchy) o `hyprctl` foca
    // a janela pelo título; nos demais, o `wmctrl` (X11). Sem nenhum dos dois o editor fica onde
    // está (não é erro).
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        // O Hyprland com config em Lua (0.56+) recusa a sintaxe antiga com código de saída
        // diferente de zero; então tenta a antiga e, se falhar, a nova.
        let focused = hyprctl_dispatch(&hyprland_legacy_focus());
        if !focused {
            hyprctl_dispatch(&hyprland_lua_focus());
        }
    } else {
        let _ = Command::new("wmctrl").args(["-a", WINDOW_TITLE]).status();
    }
}

/// Roda `hyprctl dispatch <args>` sem sujar o log; `true` se o hyprctl aceitou.
#[cfg(not(windows))]
fn hyprctl_dispatch(args: &[String]) -> bool {
    use std::process::Stdio;
    Command::new("hyprctl")
        .arg("dispatch")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Seletor do Hyprland: regex do título inteiro, ancorado.
#[cfg(not(windows))]
fn hyprland_title_selector() -> String {
    format!("title:^{WINDOW_TITLE}$")
}

/// Foco na sintaxe do Hyprland com config hyprlang (até 0.55).
#[cfg(not(windows))]
fn hyprland_legacy_focus() -> [String; 2] {
    ["focuswindow".to_string(), hyprland_title_selector()]
}

/// Foco na sintaxe do Hyprland com config em Lua (0.56+).
#[cfg(not(windows))]
fn hyprland_lua_focus() -> [String; 1] {
    [format!(
        "hl.dsp.focus({{window = \"{}\"}})",
        hyprland_title_selector()
    )]
}

/// Bloqueia rodando o laço de eventos do GTK (necessário para o ícone da bandeja). Sem uso de CPU.
#[cfg(not(windows))]
pub fn message_loop() {
    gtk::main();
}

/// Traz a janela do editor (já aberta) para a frente.
#[cfg(windows)]
pub fn show_window() {
    let title: Vec<u16> = WINDOW_TITLE.encode_utf16().chain([0]).collect();
    // SAFETY: `title` é UTF-16 terminado em zero e vive durante a chamada; classe nula é permitida.
    let hwnd = unsafe { ffi::FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd != 0 {
        // SAFETY: `hwnd` veio de FindWindowW; as funções só usam o identificador.
        unsafe {
            ffi::ShowWindow(hwnd, SW_SHOW);
            ffi::ShowWindow(hwnd, SW_RESTORE);
            ffi::SetForegroundWindow(hwnd);
        }
    }
}

/// Bloqueia processando mensagens do Windows (necessário para o ícone da bandeja). Sem uso de CPU.
#[cfg(windows)]
pub fn message_loop() {
    let mut msg = ffi::Msg {
        hwnd: 0,
        message: 0,
        wparam: 0,
        lparam: 0,
        time: 0,
        pt_x: 0,
        pt_y: 0,
    };
    // SAFETY: `msg` é uma estrutura MSG válida (mesmo layout do SDK: sem o campo `lPrivate`,
    // que só existe no Mac) e exclusiva desta thread durante o laço.
    while unsafe { ffi::GetMessageW(&mut msg, 0, 0, 0) } > 0 {
        // SAFETY: `msg` acabou de ser preenchida por `GetMessageW` e continua válida nas duas chamadas.
        unsafe {
            ffi::TranslateMessage(&msg);
            ffi::DispatchMessageW(&msg);
        }
    }
}

/// `Some` se esta for a primeira instância do serviço.
pub fn acquire_service_lock() -> Option<TcpListener> {
    TcpListener::bind(SERVICE_ADDR).ok()
}

/// `Some` se não houver outro editor aberto.
pub fn acquire_editor_lock() -> Option<TcpListener> {
    TcpListener::bind(EDITOR_ADDR).ok()
}

/// Envia um comando ao serviço (JSON versionado) e devolve a resposta parseada, se houver.
/// Serviço fora do ar é normal (editor aberto antes dele): o motivo só vai para o log de depuração.
pub fn send(command: &str) -> Option<String> {
    try_send(SERVICE_ADDR, command)
        .inspect_err(|e| log::debug!("comando {command:?}: {e}"))
        .ok()
}

/// `send` com o motivo da falha, para um endereço qualquer (os testes usam uma porta livre).
fn try_send(addr: &str, command: &str) -> Result<String, ShellError> {
    // Constrói o envelope JSON v1. Comandos de texto legado (ex.: "status", "brightness 50")
    // vão como o tipo "text" com o texto original no campo "line"; o serviço os
    // reconhece pelo legacy_to_request.
    let request = serde_json::json!({
        "version": 1,
        "cmd": "text",
        "line": command
    });
    let payload = format!("{}\n", request);

    let addr: SocketAddr = addr.parse().map_err(|e| {
        ShellError::Connect(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    })?;
    let mut stream = TcpStream::connect_timeout(&addr, IPC_TIMEOUT).map_err(ShellError::Connect)?;
    stream.set_read_timeout(Some(IPC_TIMEOUT))?;
    stream.write_all(payload.as_bytes())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;

    // Parseia resposta JSON e extrai campo relevante
    let json: serde_json::Value = serde_json::from_str(reply.trim())?;
    extract_response_text(&json).ok_or(ShellError::EmptyReply)
}

/// Extrai texto legível da resposta JSON para compatibilidade com código existente.
fn extract_response_text(json: &serde_json::Value) -> Option<String> {
    match json.get("resp")?.as_str()? {
        "ok" => Some("ok".into()),
        "error" => json
            .get("message")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "obs-info" => {
            // Extrai o ObsInfo interno (remove version e resp)
            let mut obj = json.as_object()?.clone();
            obj.remove("version");
            obj.remove("resp");
            serde_json::to_string(&obj).ok()
        }
        "apps" => json
            .get("apps")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "status" => {
            let connected = json.get("connected").and_then(|v| v.as_bool())?;
            let icon_px = json.get("icon_px").and_then(|v| v.as_u64())?;
            Some(format!("{} {}", connected as u8, icon_px))
        }
        "selftest" => {
            let ok = json.get("ok").and_then(|v| v.as_bool())?;
            let msg = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
            Some(if ok { "ok".into() } else { msg.to_string() })
        }
        _ => Some("ok".into()),
    }
}

/// Abre o editor como processo separado (se já existir um, ele mesmo se traz para a frente).
pub fn open_editor() {
    spawn_self("--editor", "abrir o editor");
}

/// Inicia o serviço em segundo plano (se já houver um, a cópia nova encerra sozinha).
pub fn start_service() {
    spawn_self("--background", "iniciar o serviço");
}

/// Roda este mesmo programa com `mode` sem esperar por ele.
fn spawn_self(mode: &str, what: &str) {
    let Ok(exe) = super::current_exe() else {
        return;
    };
    if let Err(e) = super::spawn_detached(hide_console(Command::new(exe).arg(mode))) {
        log::error!("não foi possível {what}: {e}");
    }
}

/// Caminho absoluto do `reg.exe`: um nome solto seria procurado primeiro na pasta do programa.
#[cfg(windows)]
fn reg_exe() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    std::path::Path::new(&root).join("System32").join("reg.exe")
}

#[cfg(windows)]
fn reg(args: &[&str]) -> std::io::Result<std::process::ExitStatus> {
    hide_console(Command::new(reg_exe()).args(args)).status()
}

/// Atalho de início automático do Linux (padrão XDG: `~/.config/autostart`).
#[cfg(not(windows))]
fn autostart_path() -> Option<std::path::PathBuf> {
    autostart_dir().map(|d| d.join(AUTOSTART_FILE))
}

#[cfg(not(windows))]
fn autostart_dir() -> Option<std::path::PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
    Some(config.join("autostart"))
}

/// Quem ligou o início automático antes da troca de nome continua com ele ligado: a entrada antiga
/// (que aponta para o binário antigo) sai e a nova entra no lugar.
#[cfg(not(windows))]
pub fn migrate_autostart() {
    let Some(legacy) = autostart_dir().map(|d| d.join(LEGACY_AUTOSTART_FILE)) else {
        return;
    };
    if legacy.is_file() {
        set_autostart(true);
        if let Err(e) = std::fs::remove_file(&legacy) {
            log::warn!("início automático antigo não removido: {e}");
        }
    }
}

/// Texto do atalho `.desktop`; aspas no caminho do programa para suportar espaços.
/// O `Exec` tem duas camadas de escape: dentro das aspas, `"`, `` ` ``, `$` e `\` levam uma barra;
/// depois o valor inteiro dobra as barras (escape de string do formato).
#[cfg(not(windows))]
fn autostart_entry(exe: &std::path::Path) -> String {
    let quoted: String = exe
        .display()
        .to_string()
        .chars()
        .flat_map(|c| match c {
            '"' | '`' | '$' | '\\' => vec!['\\', c],
            _ => vec![c],
        })
        .collect();
    let path = quoted.replace('\\', "\\\\");
    format!(
        "[Desktop Entry]\nType=Application\nName=Keyforge\nExec=\"{path}\" --background\nX-GNOME-Autostart-enabled=true\n"
    )
}

#[cfg(not(windows))]
pub fn autostart_enabled() -> bool {
    autostart_path().is_some_and(|p| p.is_file())
}

#[cfg(not(windows))]
pub fn set_autostart(on: bool) {
    let Some(path) = autostart_path() else {
        log::error!("não foi possível alterar o início automático: sem pasta de configuração");
        return;
    };
    let result = if on {
        super::current_exe().and_then(|exe| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, autostart_entry(&exe))
        })
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    };
    if let Err(e) = result {
        log::error!("não foi possível alterar o início automático: {e}");
    }
}

/// Ver a versão do Linux: troca o valor antigo do registro pelo novo.
#[cfg(windows)]
pub fn migrate_autostart() {
    let legacy =
        reg(&["query", AUTOSTART_KEY, "/v", LEGACY_AUTOSTART_VALUE]).is_ok_and(|s| s.success());
    if legacy {
        set_autostart(true);
        let _ = reg(&["delete", AUTOSTART_KEY, "/v", LEGACY_AUTOSTART_VALUE, "/f"]);
    }
}

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    reg(&["query", AUTOSTART_KEY, "/v", AUTOSTART_VALUE])
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn set_autostart(on: bool) {
    let result = if on {
        let Ok(exe) = super::current_exe() else {
            return;
        };
        let cmd = format!("\"{}\" --background", exe.display());
        reg(&[
            "add",
            AUTOSTART_KEY,
            "/v",
            AUTOSTART_VALUE,
            "/t",
            "REG_SZ",
            "/d",
            &cmd,
            "/f",
        ])
    } else {
        reg(&["delete", AUTOSTART_KEY, "/v", AUTOSTART_VALUE, "/f"])
    };
    if !matches!(result, Ok(s) if s.success()) {
        log::error!("não foi possível alterar o início automático");
    }
}

fn tray_pixels() -> Vec<u8> {
    crate::graphics::appicon::rgba(TRAY_ICON_PX)
}

/// Cria o ícone da bandeja; deve rodar na thread que executará `message_loop`.
pub fn create_tray() -> Option<TrayIcon> {
    // O GTK precisa ser iniciado na thread que roda `message_loop`.
    #[cfg(not(windows))]
    if let Err(e) = gtk::init() {
        log::error!("bandeja: GTK indisponível: {e}");
        return None;
    }
    let open = MenuItem::with_id("open", crate::strings::t("Abrir editor"), true, None);
    let auto = CheckMenuItem::with_id(
        "autostart",
        crate::strings::t(AUTOSTART_LABEL),
        true,
        autostart_enabled(),
        None,
    );
    let quit = MenuItem::with_id("quit", crate::strings::t("Sair"), true, None);
    let menu = Menu::new();
    menu.append_items(&[&open, &auto, &PredefinedMenuItem::separator(), &quit])
        .ok()?;
    let icon = Icon::from_rgba(tray_pixels(), TRAY_ICON_PX, TRAY_ICON_PX).ok()?;
    MenuEvent::set_event_handler(Some(|e: MenuEvent| match e.id.0.as_str() {
        "open" => open_editor(),
        // `reg` roda duas vezes: fora da thread do menu, para a bandeja não travar.
        "autostart" => {
            crate::telemetry::spawn_named("autostart", || set_autostart(!autostart_enabled()));
        }
        "quit" => std::process::exit(0),
        _ => {}
    }));
    TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
        if matches!(e, TrayIconEvent::DoubleClick { .. }) {
            open_editor();
        }
    }));
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(WINDOW_TITLE)
        .with_icon(icon)
        .build()
        .map_err(|e| log::error!("bandeja: {e}"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serviço de mentira numa porta livre: responde `reply` ao primeiro pedido.
    fn fake_service(reply: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&conn).read_line(&mut request).unwrap();
            conn.write_all(reply.as_bytes()).unwrap();
        });
        addr
    }

    #[test]
    fn an_offline_service_is_a_connect_error() {
        let addr = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().to_string()
        };
        let err = try_send(&addr, "status").unwrap_err();
        assert!(matches!(err, ShellError::Connect(_)), "{err:?}");
        assert!(matches!(
            try_send("não é endereço", "status"),
            Err(ShellError::Connect(_))
        ));
    }

    #[test]
    fn a_garbled_reply_is_a_json_error() {
        let err = try_send(&fake_service("lixo\n"), "status").unwrap_err();
        assert!(matches!(err, ShellError::Json(_)), "{err:?}");
    }

    #[test]
    fn a_reply_without_text_is_reported() {
        let err = try_send(&fake_service("{\"version\":1}\n"), "status").unwrap_err();
        assert!(matches!(err, ShellError::EmptyReply), "{err:?}");
    }

    #[test]
    fn a_good_reply_comes_back_as_text() {
        let reply = try_send(&fake_service("{\"version\":1,\"resp\":\"ok\"}\n"), "reload");
        assert_eq!(reply.unwrap(), "ok");
    }

    #[test]
    fn tray_icon_buffer_matches_dimensions() {
        assert_eq!(
            tray_pixels().len(),
            (TRAY_ICON_PX * TRAY_ICON_PX * 4) as usize
        );
    }

    #[cfg(windows)]
    #[test]
    fn reg_is_resolved_to_the_system_directory() {
        assert!(reg_exe().is_file(), "{}", reg_exe().display());
    }

    #[cfg(not(windows))]
    #[test]
    fn autostart_entry_quotes_the_program_path() {
        let entry = autostart_entry(std::path::Path::new("/opt/meu app/keyforge"));
        assert!(
            entry.contains("Exec=\"/opt/meu app/keyforge\" --background"),
            "{entry}"
        );
        assert!(entry.starts_with("[Desktop Entry]"));
        let odd = autostart_entry(std::path::Path::new("/opt/a$b`c\"d/keyforge"));
        assert!(
            odd.contains(r#"Exec="/opt/a\\$b\\`c\\"d/keyforge" --background"#),
            "{odd}"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn hyprland_focus_has_both_syntaxes() {
        assert_eq!(hyprland_legacy_focus(), ["focuswindow", "title:^Keyforge$"]);
        assert_eq!(
            hyprland_lua_focus(),
            [r#"hl.dsp.focus({window = "title:^Keyforge$"})"#]
        );
    }

    // Chama o sistema de verdade: no Hyprland ou no Windows traz a janela real do editor para a
    // frente, no meio do trabalho de quem roda os testes. Rode à mão: cargo test -- --ignored
    #[test]
    #[ignore = "foca a janela real do editor"]
    fn missing_window_is_not_an_error() {
        show_window();
    }
}
