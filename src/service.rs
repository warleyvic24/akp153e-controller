//! Serviço em segundo plano: aparelho USB, ações, bandeja do sistema e servidor de controle
//! para o editor. Não cria janela, então não consome CPU quando está ocioso.
use crate::config::schema::AppConfig;
use crate::engine::actions::Executor;
use crate::runtime::{self, SharedState};
use crate::shell;
use crate::usb::{self, UsbCmd};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

const CONTROL_DEADLINE: Duration = Duration::from_millis(500);
const MAX_COMMAND_BYTES: usize = 256;
const MAX_CONNECTIONS: usize = 8;
const DIAGNOSTIC_PX: std::ops::RangeInclusive<u32> = 16..=256;

/// Quedas seguidas por pânico antes de desistir de reiniciar (o contador viaja na variável `RESTART_ENV`).
const MAX_QUICK_RESTARTS: u32 = 3;
const RESTART_ENV: &str = "AKP_RESTARTS";
/// Depois deste tempo de pé, uma queda nova conta como a primeira.
const STABLE_AFTER: Duration = Duration::from_secs(60);
/// A cópia reiniciada espera a antiga soltar a porta de controle.
const LOCK_RETRY_FOR: Duration = Duration::from_secs(5);
const LOCK_RETRY_STEP: Duration = Duration::from_millis(250);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static STABLE: AtomicBool = AtomicBool::new(false);

fn restarts_so_far() -> u32 {
    std::env::var(RESTART_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Contador da próxima cópia, ou `None` se o serviço já caiu vezes demais em sequência.
fn next_restart_count(previous: u32, stable: bool) -> Option<u32> {
    let base = if stable { 0 } else { previous };
    (base < MAX_QUICK_RESTARTS).then_some(base + 1)
}

/// Chamado pelo gancho de pânico: o release aborta o processo e o serviço não tem janela,
/// então sem isto a bandeja e o aparelho ficariam mortos até o próximo login.
fn relaunch_after_panic() {
    let Some(count) = next_restart_count(restarts_so_far(), STABLE.load(Ordering::SeqCst)) else {
        log::error!("serviço caiu {MAX_QUICK_RESTARTS} vezes seguidas; sem novo reinício");
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let spawned = Command::new(exe)
        .arg("--background")
        .env(RESTART_ENV, count.to_string())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(e) = spawned {
        log::error!("não foi possível reiniciar o serviço: {e}");
    }
}

/// Primeira instância do serviço; uma cópia reiniciada espera um pouco a antiga morrer.
fn acquire_lock() -> Option<TcpListener> {
    let patient = restarts_so_far() > 0;
    let deadline = Instant::now() + LOCK_RETRY_FOR;
    loop {
        if let Some(listener) = shell::acquire_service_lock() {
            return Some(listener);
        }
        if !patient || Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(LOCK_RETRY_STEP);
    }
}

/// Executa o serviço até o usuário escolher "Sair" na bandeja.
pub fn run(open_editor: bool) {
    let Some(listener) = acquire_lock() else {
        // Porta ocupada: ou já há um serviço (responde ao "status") ou outro programa a usa.
        if shell::send("status").is_none() {
            log::error!("porta de controle ocupada por outro programa; serviço não iniciado");
        }
        if open_editor {
            let _ = shell::send("show");
        }
        return;
    };
    // O perfil de release aborta no pânico e o serviço não tem janela: sem isto a morte seria silenciosa.
    std::panic::set_hook(Box::new(|info| {
        log::error!("pânico: {info}");
        relaunch_after_panic();
    }));
    std::thread::spawn(|| {
        std::thread::sleep(STABLE_AFTER);
        STABLE.store(true, Ordering::SeqCst);
    });
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio");
    let exec = Executor::new(rt.handle().clone());
    let (event_tx, event_rx) = channel();
    let (cmd_tx, cmd_rx) = channel();
    // O brilho precisa estar definido antes da thread USB inicializar o aparelho.
    let cfg = AppConfig::load();
    usb::set_brightness(cfg.ui.brightness);
    crate::strings::set_language(cfg.ui.language);
    usb::spawn(event_tx, cmd_rx);
    let state = runtime::new_state(cfg, usb::ICON_SIZE as u32);
    runtime::spawn(state.clone(), cmd_tx.clone(), event_rx, exec);
    spawn_control_server(listener, state, cmd_tx);

    let _tray = shell::create_tray();
    if open_editor {
        shell::open_editor();
    }
    shell::message_loop();
    drop(rt);
}

/// Uma thread por conexão, com teto: um cliente lento não trava os demais comandos.
fn spawn_control_server(listener: TcpListener, state: SharedState, tx: Sender<UsbCmd>) {
    std::thread::spawn(move || {
        let active = Arc::new(AtomicUsize::new(0));
        for conn in listener.incoming().flatten() {
            if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                active.fetch_sub(1, Ordering::SeqCst);
                log::warn!("conexões de controle demais; descartada");
                continue;
            }
            let (state, tx, active) = (state.clone(), tx.clone(), active.clone());
            std::thread::spawn(move || {
                handle(conn, &state, &tx);
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
}

/// Lê uma linha de comando com tamanho e tempo totais limitados. Qualquer processo local pode
/// conectar na porta; um cliente que mande 1 byte por vez não pode prender a thread nem a memória.
fn read_command(conn: &mut TcpStream) -> Option<String> {
    let deadline = Instant::now() + CONTROL_DEADLINE;
    let mut line = Vec::with_capacity(32);
    let mut byte = [0u8; 1];
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        conn.set_read_timeout(Some(left)).ok()?;
        match conn.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) if line.len() < MAX_COMMAND_BYTES => line.push(byte[0]),
            _ => return None,
        }
    }
    String::from_utf8(line).ok()
}

/// Argumento de `calib`/`pattern`: vazio = varredura de tamanhos; senão, tamanho dentro de `DIAGNOSTIC_PX`.
/// A porta aceita qualquer processo local, então o valor é limitado antes de alocar a imagem.
fn parse_px(arg: &str) -> Result<Option<u32>, ()> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Ok(None);
    }
    match arg.parse::<u32>() {
        Ok(px) if DIAGNOSTIC_PX.contains(&px) => Ok(Some(px)),
        _ => {
            log::warn!("tamanho fora de {DIAGNOSTIC_PX:?}: {arg}");
            Err(())
        }
    }
}

fn handle(mut conn: TcpStream, state: &SharedState, tx: &Sender<UsbCmd>) {
    let Some(line) = read_command(&mut conn) else {
        return;
    };
    match line.trim() {
        "show" => shell::open_editor(),
        "reconnect" => {
            let _ = tx.send(UsbCmd::Reconnect);
        }
        "reload" => {
            let cfg = AppConfig::load();
            let brightness = cfg.ui.brightness;
            let language = cfg.ui.language;
            let (snapshot, brightness_changed) = {
                let mut sh = runtime::lock(state);
                let changed = sh.cfg.ui.brightness != brightness;
                sh.cfg = cfg;
                (runtime::Snapshot::take(&sh), changed)
            };
            usb::set_brightness(brightness);
            // O menu da bandeja só muda de idioma quando o serviço reinicia; o resto vale já.
            crate::strings::set_language(language);
            if brightness_changed {
                let _ = tx.send(UsbCmd::Brightness(brightness));
            }
            snapshot.render(tx);
        }
        "status" => {
            let sh = runtime::lock(state);
            let _ = writeln!(conn, "{} {}", u8::from(sh.connected), sh.icon_px);
        }
        other if other.starts_with("calib") => {
            let Ok(px) = parse_px(other.trim_start_matches("calib")) else {
                return;
            };
            let key_count = runtime::lock(state).cfg.page().buttons.len();
            runtime::render_calibration(key_count, tx, px);
        }
        other if other.starts_with("pattern") => {
            let Ok(px) = parse_px(other.trim_start_matches("pattern")) else {
                return;
            };
            let key_count = runtime::lock(state).cfg.page().buttons.len();
            runtime::render_pattern(key_count, tx, px);
        }
        other => log::warn!("comando desconhecido: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Abre uma conexão local; o lado do servidor é entregue a `read_command`.
    fn serve_once(client: impl FnOnce(TcpStream) + Send + 'static) -> Option<String> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || client(TcpStream::connect(addr).unwrap()));
        let (mut server, _) = listener.accept().unwrap();
        let line = read_command(&mut server);
        client.join().unwrap();
        line
    }

    #[test]
    fn restarts_stop_after_three_quick_crashes_and_reset_when_stable() {
        assert_eq!(next_restart_count(0, false), Some(1));
        assert_eq!(next_restart_count(2, false), Some(3));
        assert_eq!(next_restart_count(3, false), None);
        assert_eq!(next_restart_count(3, true), Some(1));
    }

    #[test]
    fn a_normal_command_line_is_read() {
        let line = serve_once(|mut c| c.write_all(b"status\n").unwrap());
        assert_eq!(line.as_deref(), Some("status"));
    }

    #[test]
    fn an_oversized_line_is_rejected() {
        let line = serve_once(|mut c| c.write_all(&[b'a'; 400]).unwrap());
        assert_eq!(line, None);
    }

    #[test]
    fn a_client_dripping_bytes_hits_the_total_deadline() {
        let started = Instant::now();
        let line = serve_once(|mut c| {
            for _ in 0..8 {
                if c.write_all(b"a").is_err() {
                    break;
                }
                std::thread::sleep(CONTROL_DEADLINE / 4);
            }
        });
        assert_eq!(line, None);
        assert!(started.elapsed() < CONTROL_DEADLINE * 3);
    }

    #[test]
    fn diagnostic_size_is_bounded() {
        assert_eq!(parse_px(""), Ok(None));
        assert_eq!(parse_px(" 100 "), Ok(Some(100)));
        for bad in ["0", "15", "257", "4000000000", "-1", "abc"] {
            assert_eq!(parse_px(bad), Err(()), "{bad}");
        }
    }
}
