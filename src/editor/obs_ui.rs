//! Editor das teclas do OBS e a seção do OBS nas Preferências. A lista de cenas e entradas vem do
//! serviço (que está conectado ao OBS), pelo comando `obs-info`.
use super::action_ui::PaneCtx;
use super::{tokens, widgets};
use crate::config::schema::{ObsCommand, ObsSettings};
use crate::editor::theme::Palette;
use crate::platform::shell;
use crate::strings::{t, tf};
use eframe::egui::{self, RichText};
use std::time::{Duration, Instant};

/// De quanto em quanto tempo o editor aberto numa tecla do OBS pergunta o estado ao serviço.
const INFO_REFRESH: Duration = Duration::from_secs(2);
const PORT_WIDTH: f32 = 70.0;

/// Nome exibido de cada comando (passa por `t`).
pub(crate) fn command_label(command: ObsCommand) -> &'static str {
    match command {
        ObsCommand::Scene => "Trocar cena",
        ObsCommand::Stream => "Transmitir",
        ObsCommand::Record => "Gravar",
        ObsCommand::RecordPause => "Pausar gravação",
        ObsCommand::Mute => "Mudo",
        ObsCommand::VirtualCam => "Câmera virtual",
        ObsCommand::ReplayBuffer => "Buffer de replay",
        ObsCommand::ReplaySave => "Salvar replay",
    }
}

/// Ícone sugerido para a tecla de cada comando.
pub(crate) fn command_icon(command: ObsCommand) -> &'static str {
    match command {
        ObsCommand::Scene => "lucide:clapperboard",
        ObsCommand::Stream => "lucide:radio",
        ObsCommand::Record => "lucide:circle-dot",
        ObsCommand::RecordPause => "lucide:pause",
        ObsCommand::Mute => "lucide:mic-off",
        ObsCommand::VirtualCam => "lucide:webcam",
        ObsCommand::ReplayBuffer => "lucide:rotate-ccw-clock",
        ObsCommand::ReplaySave => "lucide:save",
    }
}

/// Como o serviço respondeu a `obs-info`.
#[derive(Clone, Copy, Default, PartialEq)]
enum Service {
    #[default]
    Stopped,
    /// Respondeu, mas sem o JSON: é uma versão antiga, sem o OBS.
    Outdated,
    Ready,
}

/// O que o serviço contou do OBS na última consulta.
#[derive(Clone, Default)]
struct Info {
    asked: Option<Instant>,
    service: Service,
    connected: bool,
    scenes: Vec<String>,
    inputs: Vec<String>,
}

fn info(ctx: &egui::Context) -> Info {
    let id = egui::Id::new("obs-info");
    let cached: Info = ctx.data_mut(|d| d.get_temp(id).unwrap_or_default());
    if cached.asked.is_some_and(|at| at.elapsed() < INFO_REFRESH) {
        return cached;
    }
    let reply = shell::send("obs-info");
    let parsed: Option<serde_json::Value> =
        reply.as_deref().and_then(|r| serde_json::from_str(r).ok());
    let list = |key: &str| -> Vec<String> {
        parsed
            .as_ref()
            .and_then(|v| v.get(key)?.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect()
    };
    let service = match (&reply, &parsed) {
        (None, _) => Service::Stopped,
        (Some(_), None) => Service::Outdated,
        (Some(_), Some(_)) => Service::Ready,
    };
    let fresh = Info {
        asked: Some(Instant::now()),
        service,
        connected: parsed
            .as_ref()
            .and_then(|v| v.get("connected")?.as_bool())
            .unwrap_or(false),
        scenes: list("scenes"),
        inputs: list("inputs"),
    };
    ctx.data_mut(|d| d.insert_temp(id, fresh.clone()));
    fresh
}

/// Editor de uma ação do OBS; devolve `true` se algo mudou.
pub fn obs_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    command: &mut ObsCommand,
    target: &mut String,
) -> bool {
    let mut changed = false;
    let info = info(cx.ctx);
    status_line(ui, cx.pal, &info);
    widgets::section(ui, "No OBS", cx.pal);
    ui.horizontal_wrapped(|ui| {
        for option in ObsCommand::ALL {
            let on = *command == option;
            if widgets::pill(ui, command_label(option), on, cx.accent, cx.pal).clicked() && !on {
                *command = option;
                target.clear();
                changed = true;
            }
        }
    });
    if command.needs_target() {
        let (title, names, empty) = if *command == ObsCommand::Scene {
            ("Cena", &info.scenes, "Escolha a cena")
        } else {
            ("Entrada de áudio", &info.inputs, "Escolha a entrada")
        };
        let label = widgets::section(ui, title, cx.pal);
        let shown = if target.is_empty() {
            t(empty).to_string()
        } else {
            target.clone()
        };
        egui::ComboBox::from_id_salt(("obs-target", title))
            .selected_text(shown)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for name in names {
                    if ui.selectable_label(target == name, name).clicked() {
                        *target = name.clone();
                        changed = true;
                    }
                }
            })
            .response
            .labelled_by(label);
        // Sem conexão a lista fica vazia: aí dá para digitar o nome exato.
        if names.is_empty() {
            changed |= ui
                .add(
                    egui::TextEdit::singleline(target)
                        .hint_text(widgets::hint("ou digite o nome exato", cx.pal))
                        .desired_width(ui.available_width()),
                )
                .changed();
        }
    }
    let hint = t("Ligada (gravando, no ar, na cena, mudo), a tecla fica vermelha ou acesa; o segundo estado em Alternar substitui esse destaque.");
    ui.label(
        RichText::new(hint)
            .size(tokens::text::CAPTION)
            .color(cx.pal.muted),
    );
    changed
}

fn status_line(ui: &mut egui::Ui, pal: &Palette, info: &Info) {
    let (text, color) = match (info.service, info.connected) {
        (Service::Ready, true) => (t("OBS conectado").to_string(), pal.ok),
        (Service::Ready, false) => (
            t("OBS não conectado. No OBS: Ferramentas › Configurações do servidor WebSocket › Ativar. Endereço e senha em Preferências.").to_string(),
            pal.bad,
        ),
        (Service::Outdated, _) => (
            t("O serviço em execução é de uma versão anterior, sem o OBS: saia pela bandeja e abra de novo.").to_string(),
            pal.bad,
        ),
        (Service::Stopped, _) => (
            t("Serviço parado: sem ele o OBS não recebe as teclas.").to_string(),
            pal.bad,
        ),
    };
    ui.label(RichText::new(text).size(tokens::text::CAPTION).color(color));
}

/// Seção "OBS Studio" das Preferências; devolve `true` se a conexão mudou.
pub fn settings_section(ui: &mut egui::Ui, obs: &mut ObsSettings, pal: &Palette) -> bool {
    let mut changed = false;
    widgets::section(ui, "OBS Studio (WebSocket)", pal);
    ui.horizontal(|ui| {
        let host = ui.label(t("Endereço")).id;
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut obs.host)
                    .desired_width(ui.available_width() - PORT_WIDTH - 60.0),
            )
            .labelled_by(host)
            .changed();
        let port = ui.label(t("Porta")).id;
        changed |= ui
            .add(egui::DragValue::new(&mut obs.port).range(1..=u16::MAX))
            .labelled_by(port)
            .changed();
    });
    ui.horizontal(|ui| {
        let pass = ui.label(t("Senha")).id;
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut obs.password)
                    .password(true)
                    .hint_text(widgets::hint(
                        "vazia se a autenticação estiver desligada",
                        pal,
                    ))
                    .desired_width(ui.available_width()),
            )
            .labelled_by(pass)
            .changed();
    });
    let msg = tf(
        "O serviço conecta sozinho quando alguma tecla usa o OBS. Padrão: este PC, porta {}.",
        &[&ObsSettings::default().port],
    );
    ui.label(
        RichText::new(msg)
            .size(tokens::text::CAPTION)
            .color(pal.muted),
    );
    changed
}
