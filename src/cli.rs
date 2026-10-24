//! Modos de linha de comando e o editor: log em arquivo, `--run-action` e a janela do editor.
use crate::config::schema::*;
use crate::editor::{theme, App};
use crate::engine::actions::Executor;
use crate::platform::shell;
use crate::service::runtime;
use crate::usb;
use eframe::egui;
use std::sync::mpsc::channel;
use std::time::Duration;
use usb::UsbCmd;

const WINDOW_ICON_PX: u32 = 64;
const ACTION_CLI_GRACE: Duration = Duration::from_millis(1500);
const LOG_MAX_BYTES: u64 = 1 << 20;

pub(crate) fn init_logging() {
    let file = AppConfig::path()
        .and_then(|p| p.parent().map(|d| d.join("akp153e.log")))
        .and_then(|p| {
            if std::fs::metadata(&p)
                .map(|m| m.len() > LOG_MAX_BYTES)
                .unwrap_or(false)
            {
                let _ = std::fs::remove_file(&p);
            }
            std::fs::create_dir_all(p.parent()?).ok()?;
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .ok()
        });
    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,akp153e=info"),
    );
    if let Some(f) = file {
        builder.target(env_logger::Target::Pipe(Box::new(f)));
    }
    builder.init();
}
/// Executa uma ação (JSON) e sai; serve para testes e automação. Código de saída 2 se o JSON for inválido.
pub(crate) fn run_action_cli(json: Option<&String>) {
    let parsed = json
        .ok_or_else(|| "falta o JSON da ação".to_string())
        .and_then(|j| serde_json::from_str::<ButtonAction>(j).map_err(|e| e.to_string()));
    let action = match parsed {
        Ok(a) => a,
        Err(e) => {
            log::error!("--run-action: {e}");
            std::process::exit(2);
        }
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("tokio");
    Executor::new(rt.handle().clone()).run(action);
    std::thread::sleep(ACTION_CLI_GRACE); // dá tempo de a tarefa assíncrona terminar
}

/// Editor: janela comum, sem hardware próprio; conversa com o serviço pela rede local.
pub(crate) fn run_editor() -> eframe::Result {
    let Some(_editor_lock) = shell::acquire_editor_lock() else {
        shell::show_window();
        return Ok(());
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("tokio");
    let exec = Executor::new(rt.handle().clone());
    #[allow(unused_mut)]
    let mut cfg = AppConfig::load();
    // Só em debug: `AKP153E_PREVIEW_THEME=light` mostra o tema sem mexer no arquivo (nada é gravado
    // enquanto o usuário não editar algo). Serve para conferir o visual nos dois temas.
    #[cfg(debug_assertions)]
    match std::env::var("AKP153E_PREVIEW_THEME").as_deref() {
        Ok("light") => cfg.ui.theme = Theme::Light,
        Ok("dark") => cfg.ui.theme = Theme::Dark,
        _ => {}
    }
    let sh = runtime::new_state(cfg, usb::ICON_SIZE as u32);
    runtime::spawn_status_poll(sh.clone());
    let (tx, rx) = channel::<UsbCmd>();
    drop(rx); // o editor não fala com o aparelho; quem desenha nele é o serviço
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(egui::IconData {
                rgba: crate::graphics::appicon::rgba(WINDOW_ICON_PX),
                width: WINDOW_ICON_PX,
                height: WINDOW_ICON_PX,
            })
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title(shell::WINDOW_TITLE),
        ..Default::default()
    };
    let r = eframe::run_native(
        "AKP153E",
        opts,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(sh, tx, exec)))
        }),
    );
    drop(rt);
    r
}
