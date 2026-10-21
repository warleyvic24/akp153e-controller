//! Integração com o Windows: instância única, controle entre serviço e editor (TCP local),
//! bandeja do sistema, laço de mensagens e início automático.
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const WINDOW_TITLE: &str = "Controle AKP153E";
const SERVICE_ADDR: &str = "127.0.0.1:47153";
const EDITOR_ADDR: &str = "127.0.0.1:47154";
const IPC_TIMEOUT: Duration = Duration::from_millis(400);
const AUTOSTART_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_VALUE: &str = "AKP153E";
const TRAY_ICON_PX: u32 = 32;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SW_SHOW: i32 = 5;
const SW_RESTORE: i32 = 9;

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
    if let Err(e) = Command::new(exe)
        .arg("--editor")
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        log::error!("não foi possível abrir o editor: {e}");
    }
}

/// Caminho absoluto do `reg.exe`: um nome solto seria procurado primeiro na pasta do programa.
fn reg_exe() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    std::path::Path::new(&root).join("System32").join("reg.exe")
}

fn reg(args: &[&str]) -> std::io::Result<std::process::ExitStatus> {
    Command::new(reg_exe())
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
}

pub fn autostart_enabled() -> bool {
    reg(&["query", AUTOSTART_KEY, "/v", AUTOSTART_VALUE])
        .map(|s| s.success())
        .unwrap_or(false)
}

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
    crate::appicon::rgba(TRAY_ICON_PX)
}

/// Cria o ícone da bandeja; deve rodar na thread que executará `message_loop`.
pub fn create_tray() -> Option<TrayIcon> {
    let open = MenuItem::with_id("open", "Abrir editor", true, None);
    let auto = CheckMenuItem::with_id(
        "autostart",
        "Iniciar com o Windows",
        true,
        autostart_enabled(),
        None,
    );
    let quit = MenuItem::with_id("quit", "Sair", true, None);
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

    #[test]
    fn reg_is_resolved_to_the_system_directory() {
        assert!(reg_exe().is_file(), "{}", reg_exe().display());
    }

    #[test]
    fn missing_window_is_not_an_error() {
        show_window();
    }
}
