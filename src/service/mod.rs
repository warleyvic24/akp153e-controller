//! Serviço em segundo plano: aparelho USB, ações, bandeja do sistema e porta de controle
//! para o editor. Não cria janela, então não consome CPU quando está ocioso.
//!
//! Este módulo só monta as threads (`run`) e cuida do reinício após pânico; cada parte vive no
//! seu arquivo: estado e teclas em `runtime.rs`, porta de controle em `control.rs` (formato em
//! `protocol.rs`), OBS em `obs_runtime.rs`, mixer em `mixer_runtime.rs`, perfil automático em
//! `auto_profile.rs`, descanso da tela em `sleep.rs` e autoteste em `selftest.rs`.
mod auto_profile;
mod control;
pub(crate) mod mixer_runtime;
mod obs_runtime;
mod protocol;
pub(crate) mod runtime;
mod selftest;
mod sleep;

use crate::config::schema::AppConfig;
use crate::engine::actions::Executor;
use crate::platform::shell;
use crate::usb;
use std::net::TcpListener;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

/// Quedas seguidas por pânico antes de desistir de reiniciar (o contador viaja na variável `RESTART_ENV`).
const MAX_QUICK_RESTARTS: u32 = 3;
const RESTART_ENV: &str = "AKP_RESTARTS";
/// Depois deste tempo de pé, uma queda nova conta como a primeira.
const STABLE_AFTER: Duration = Duration::from_secs(60);
/// A cópia reiniciada espera a antiga soltar a porta de controle.
const LOCK_RETRY_FOR: Duration = Duration::from_secs(5);
const LOCK_RETRY_STEP: Duration = Duration::from_millis(250);

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
    let Ok(exe) = crate::platform::current_exe() else {
        return;
    };
    let spawned = crate::platform::hide_console(
        Command::new(exe)
            .arg("--background")
            .env(RESTART_ENV, count.to_string()),
    )
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
    crate::telemetry::spawn_named("stable-timer", || {
        std::thread::sleep(STABLE_AFTER);
        STABLE.store(true, Ordering::SeqCst);
    });
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        restarts = restarts_so_far(),
        "serviço iniciado"
    );
    let rt = match crate::engine::tokio_runtime(2) {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("tokio: {e}; serviço não iniciado");
            return;
        }
    };
    let (obs_tx, obs_rx) = channel();
    let exec = Executor::new(rt.handle().clone()).with_obs(obs_tx.clone());
    let (event_tx, event_rx) = channel();
    let (cmd_tx, cmd_rx) = channel();
    // O brilho precisa estar definido antes da thread USB inicializar o aparelho.
    shell::migrate_autostart();
    let cfg = AppConfig::load();
    usb::set_brightness(cfg.ui.brightness);
    crate::strings::set_language(cfg.ui.language);
    usb::spawn(event_tx, cmd_rx);
    let state = runtime::new_state(cfg, usb::ICON_SIZE as u32);
    runtime::spawn(state.clone(), cmd_tx.clone(), event_rx, exec);
    runtime::live::spawn(state.clone(), Some(cmd_tx.clone()));
    let (focus_tx, focus_rx) = channel();
    crate::platform::focus::spawn_watch(focus_tx);
    auto_profile::spawn(state.clone(), cmd_tx.clone(), focus_rx);
    obs_runtime::spawn(state.clone(), cmd_tx.clone(), obs_rx);
    control::spawn(listener, state, cmd_tx, obs_tx);

    let _tray = shell::create_tray();
    if open_editor {
        shell::open_editor();
    }
    shell::message_loop();
    drop(rt);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarts_stop_after_three_quick_crashes_and_reset_when_stable() {
        assert_eq!(next_restart_count(0, false), Some(1));
        assert_eq!(next_restart_count(2, false), Some(3));
        assert_eq!(next_restart_count(3, false), None);
        assert_eq!(next_restart_count(3, true), Some(1));
    }
}
