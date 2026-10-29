//! Integração com o sistema: instância única, controle entre serviço e editor (TCP local),
//! bandeja, laço de eventos e início automático. Windows e Linux diferem só nos trechos com `cfg`.
use super::hide_console;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command;
use std::time::Duration;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const WINDOW_TITLE: &str = "Controle AKP153E";
const SERVICE_ADDR: &str = "127.0.0.1:47153";
const EDITOR_ADDR: &str = "127.0.0.1:47154";
const IPC_TIMEOUT: Duration = Duration::from_millis(400);
#[cfg(windows)]
const AUTOSTART_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const AUTOSTART_VALUE: &str = "AKP153E";
#[cfg(not(windows))]
const AUTOSTART_FILE: &str = "akp153e.desktop";
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
    // Sem API de janelas portável entre X11 e Wayland: usa o `wmctrl` se existir; senão o editor
    // fica onde está (não é erro).
    let _ = Command::new("wmctrl").args(["-a", WINDOW_TITLE]).status();
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

/// Envia um comando ao serviço e devolve a resposta (uma linha), se houver.
pub fn send(command: &str) -> Option<String> {
    let addr: SocketAddr = SERVICE_ADDR.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&addr, IPC_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(IPC_TIMEOUT)).ok()?;
    stream.write_all(format!("{command}\n").as_bytes()).ok()?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).ok()?;
    Some(reply.trim().to_string())
}

/// Abre o editor como processo separado (se já existir um, ele mesmo se traz para a frente).
pub fn open_editor() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if let Err(e) = hide_console(Command::new(exe).arg("--editor")).spawn() {
        log::error!("não foi possível abrir o editor: {e}");
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
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
    Some(config.join("autostart").join(AUTOSTART_FILE))
}

/// Texto do atalho `.desktop`; aspas no caminho do programa para suportar espaços.
#[cfg(not(windows))]
fn autostart_entry(exe: &std::path::Path) -> String {
    let path = exe
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nName=AKP153E\nExec=\"{path}\" --background\nX-GNOME-Autostart-enabled=true\n"
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
        std::env::current_exe().and_then(|exe| {
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

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    reg(&["query", AUTOSTART_KEY, "/v", AUTOSTART_VALUE])
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn set_autostart(on: bool) {
    let result = if on {
        let Ok(exe) = std::env::current_exe() else {
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
    #[cfg(windows)]
    let autostart_label = "Iniciar com o Windows";
    #[cfg(not(windows))]
    let autostart_label = "Iniciar com o sistema";
    let auto = CheckMenuItem::with_id(
        "autostart",
        crate::strings::t(autostart_label),
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
            std::thread::spawn(|| set_autostart(!autostart_enabled()));
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
        let entry = autostart_entry(std::path::Path::new("/opt/meu app/akp153e"));
        assert!(
            entry.contains("Exec=\"/opt/meu app/akp153e\" --background"),
            "{entry}"
        );
        assert!(entry.starts_with("[Desktop Entry]"));
    }

    #[test]
    fn missing_window_is_not_an_error() {
        show_window();
    }
}
