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
/// Menor janela: o layout compacto (`editor::layout`) cabe nela. Um gerenciador lado a lado pode
/// dar ainda menos, mas o tamanho pedido não deve ser maior que a faixa que a janela costuma ganhar.
const MIN_WINDOW: [f32; 2] = [640.0, 480.0];

/// Liga o log estruturado (JSON com rotação de 10 MiB x 5 arquivos, ver `telemetry`) na pasta da
/// config. Antes, migra a pasta antiga: o log cria a nova, e a cópia só acontece se ela não existe.
pub(crate) fn init_logging(mode: crate::telemetry::Mode) {
    let migrated = AppConfig::migrate_legacy_dir();
    let dir = AppConfig::path().and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    crate::telemetry::init(mode, dir.as_deref());
    if let Some(note) = migrated {
        log::warn!("{note}");
    }
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
    let rt = match crate::engine::tokio_runtime(1) {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("--run-action: tokio: {e}");
            std::process::exit(1);
        }
    };
    // Fora do serviço não há tecla de origem: os bancos de som vêm da config do disco.
    Executor::new(rt.handle().clone()).run(action, &AppConfig::load());
    std::thread::sleep(ACTION_CLI_GRACE); // dá tempo de a tarefa assíncrona terminar
}

/// `--selftest`: pede ao serviço o autoteste guiado no aparelho e diz o que fazer.
pub(crate) fn selftest_cli() {
    let reply = shell::send("selftest");
    println!("{}", crate::editor::selftest_reply_text(reply.as_deref()));
    if reply.as_deref() != Some("ok") {
        std::process::exit(1);
    }
}

/// Editor: janela comum, sem hardware próprio; conversa com o serviço pela rede local.
pub(crate) fn run_editor() -> eframe::Result {
    // Só em debug, com `KEYFORGE_CONFIG_DIR`: um segundo editor sobre a cópia da config pode abrir ao
    // lado do editor real (para conferir a interface), porque não grava no mesmo arquivo.
    let preview_copy =
        cfg!(debug_assertions) && crate::config::schema::dev_var("CONFIG_DIR").is_some();
    let editor_lock = shell::acquire_editor_lock();
    if editor_lock.is_none() && !preview_copy {
        shell::show_window();
        return Ok(());
    }
    let rt =
        crate::engine::tokio_runtime(1).map_err(|e| eframe::Error::AppCreation(Box::new(e)))?;
    let exec = Executor::new(rt.handle().clone());
    let first_run = !AppConfig::path().is_some_and(|p| p.exists());
    let mut cfg = AppConfig::load();
    // Primeira vez dentro do Omarchy: a interface já nasce com as cores do tema dele.
    if first_run && crate::editor::omarchy::current().is_some() {
        cfg.ui.theme = Theme::Omarchy;
    }
    // Só em debug: `KEYFORGE_PREVIEW_THEME=light` mostra o tema sem mexer no arquivo (nada é gravado
    // enquanto o usuário não editar algo). Serve para conferir o visual nos dois temas.
    #[cfg(debug_assertions)]
    match crate::config::schema::dev_var("PREVIEW_THEME")
        .and_then(|v| v.into_string().ok())
        .as_deref()
    {
        Some("light") => cfg.ui.theme = Theme::Light,
        Some("dark") => cfg.ui.theme = Theme::Dark,
        Some("omarchy") => cfg.ui.theme = Theme::Omarchy,
        Some("contrast") => cfg.ui.theme = Theme::HighContrast,
        _ => {}
    }
    let sh = runtime::new_state(cfg, usb::ICON_SIZE as u32);
    runtime::spawn_status_poll(sh.clone());
    runtime::live::spawn(sh.clone(), None);
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
            .with_min_inner_size(MIN_WINDOW)
            .with_title(shell::WINDOW_TITLE)
            // Wayland (Hyprland): o app_id permite regras de janela e casa com o `.desktop`.
            .with_app_id(shell::APP_ID),
        ..Default::default()
    };
    let r = eframe::run_native(
        "Keyforge",
        opts,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(sh, tx, exec)))
        }),
    );
    drop(rt);
    r
}
