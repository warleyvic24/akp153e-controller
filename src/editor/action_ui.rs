//! Editor da ação de um botão: escolha do tipo (atalho, app, web...) e os campos de cada tipo.
use super::action_kind::ActionKind;
use super::{sound_ui, textlist, widgets};
use crate::config::schema::*;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::text;
use crate::engine;
use crate::strings::{display_name, t, tf};
use eframe::egui::{self, Color32, RichText};

/// O que todo editor de ação precisa do contexto da tela (em vez de passar cada item solto).
pub struct PaneCtx<'a> {
    pub ctx: &'a egui::Context,
    pub pal: &'a Palette,
    pub accent: Color32,
    pub targets: &'a Targets,
}

/// O que as ações podem referenciar na config: páginas e perfis (id estável + nome exibido) e
/// os bancos de som. Montado a cada quadro a partir da config, nunca fica desatualizado.
pub struct Targets {
    pages: Vec<(u64, String)>,
    profiles: Vec<(u64, String)>,
    sound_banks: SoundBanks,
}

impl Targets {
    pub fn from(cfg: &AppConfig) -> Self {
        let pages = cfg.profiles[cfg.active_profile]
            .pages
            .iter()
            .map(|p| (p.id, display_name(&p.name)))
            .collect();
        let profiles = cfg
            .profiles
            .iter()
            .map(|p| (p.id, display_name(&p.name)))
            .collect();
        Self {
            pages,
            profiles,
            sound_banks: cfg.sound_banks(),
        }
    }

    /// Bancos de som da config, por nome.
    pub fn sound_banks(&self) -> &SoundBanks {
        &self.sound_banks
    }

    pub fn has_page(&self, id: u64) -> bool {
        self.page_name(id).is_some()
    }

    pub fn has_profile(&self, id: u64) -> bool {
        self.profile_name(id).is_some()
    }

    pub fn page_name(&self, id: u64) -> Option<&str> {
        self.pages.iter().find(|p| p.0 == id).map(|p| p.1.as_str())
    }

    pub fn profile_name(&self, id: u64) -> Option<&str> {
        self.profiles
            .iter()
            .find(|p| p.0 == id)
            .map(|p| p.1.as_str())
    }
}

/// Nome exibido (em português, para passar por `t`) de uma tecla de mídia; o id se desconhecida.
pub(crate) fn media_label(key: &str) -> &str {
    MEDIA_KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(key, |(_, label)| label)
}

const TYPE_TEXT_ROWS: usize = 3;

const MEDIA_KEYS: [(&str, &str); 7] = [
    ("playpause", "Play/Pause"),
    ("next", "Próxima"),
    ("prev", "Anterior"),
    ("volup", "Vol +"),
    ("voldown", "Vol −"),
    ("mute", "Mudo"),
    ("micmute", "Mic mudo"),
];

const BODY_ROWS: usize = 3;
/// `id` da ação "Ao tocar" no `action_editor` (as outras são "lact", "tact" e "step").
pub(super) const MAIN_ACTION_ID: &str = "act";

/// Explica a ação "Mixer": o que aparece no aparelho e como sair.
const MIXER_HINT: &str = "Mostra no aparelho os apps com som: ícone com a porcentagem em cima, + no meio e − embaixo. Esta tecla vira SAIR (segure para sair se houver mais de 5 apps).";

fn pick_target(
    ui: &mut egui::Ui,
    pal: &Palette,
    id: &str,
    label: egui::Id,
    value: &mut u64,
    options: &[(u64, String)],
) -> bool {
    let mut changed = false;
    let mut shown = options
        .iter()
        .find(|o| o.0 == *value)
        .map_or(t("(escolha)").to_string(), |o| o.1.clone());
    let options_names: Vec<&str> = options.iter().map(|o| o.1.as_str()).collect();
    widgets::combo_box(ui, id, "Destino", &mut shown, &options_names, pal).labelled_by(label);
    // Find the selected value from the shown name
    if let Some(pos) = options_names.iter().position(|n| *n == shown) {
        if *value != options[pos].0 {
            *value = options[pos].0;
            changed = true;
        }
    }
    changed
}

enum Capture {
    Cancel,
    Keys(Vec<String>),
}

fn key_name(k: egui::Key) -> Option<String> {
    use egui::Key::*;
    let name = match k {
        ArrowUp => "up".to_string(),
        ArrowDown => "down".to_string(),
        ArrowLeft => "left".to_string(),
        ArrowRight => "right".to_string(),
        Escape => "esc".to_string(),
        _ => k.name().to_lowercase(),
    };
    engine::actions::is_valid_key(&name).then_some(name)
}

/// Primeira combinação de teclas pressionada neste quadro (Esc sozinho cancela).
fn capture_hotkey(ctx: &egui::Context) -> Option<Capture> {
    ctx.input(|i| {
        i.events.iter().find_map(|e| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = e
            else {
                return None;
            };
            if *key == egui::Key::Escape && !modifiers.any() {
                return Some(Capture::Cancel);
            }
            let name = key_name(*key)?;
            let mut keys = Vec::new();
            for (held, modifier) in [
                (modifiers.ctrl, "ctrl"),
                (modifiers.alt, "alt"),
                (modifiers.shift, "shift"),
            ] {
                if held {
                    keys.push(modifier.to_string());
                }
            }
            keys.push(name);
            Some(Capture::Keys(keys))
        })
    })
}

/// Editor completo da ação `a`. `id` separa o estado de "Ao tocar" e "Ao segurar".
pub fn action_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    id: &'static str,
    a: &mut ButtonAction,
    rec: &mut Option<&'static str>,
) -> bool {
    let mut changed = super::kind_picker::kind_picker(ui, cx, id, a);
    changed |= match a {
        ButtonAction::Hotkey { keys } => hotkey_editor(ui, cx, id, keys, rec),
        ButtonAction::LaunchApp { path, args } => launch_editor(ui, cx.pal, id, path, args),
        ButtonAction::OpenUrl { url } => url_editor(ui, cx, url),
        ButtonAction::HttpWebhook { url, method, body } => {
            webhook_editor(ui, cx, url, method, body)
        }
        ButtonAction::MediaKey { key } => media_editor(ui, cx, key),
        ButtonAction::SwitchPage { page } => {
            let label = widgets::section(ui, "Página de destino", cx.pal);
            pick_target(ui, cx.pal, id, label, page, &cx.targets.pages)
        }
        ButtonAction::SwitchProfile { profile } => {
            let label = widgets::section(ui, "Perfil de destino", cx.pal);
            pick_target(ui, cx.pal, id, label, profile, &cx.targets.profiles)
        }
        ButtonAction::OpenMixer => {
            ui.label(widgets::hint(t(MIXER_HINT), cx.pal));
            false
        }
        ButtonAction::TypeText { text } => type_text_editor(ui, cx, text),
        ButtonAction::MultiAction { steps } => multi_editor(ui, cx, id, steps),
        ButtonAction::Delay { ms } => delay_editor(ui, cx, ms),
        ButtonAction::RunCommand { command } => command_editor(ui, cx, command),
        ButtonAction::Brightness { delta } => brightness_editor(ui, cx, delta),
        ButtonAction::Obs { command, target } => super::obs_ui::obs_editor(ui, cx, command, target),
        ButtonAction::PlaySound { path } => sound_ui::sound_file_editor(ui, cx, path),
        ButtonAction::SoundBank { name, sounds } => sound_ui::bank_editor(ui, cx, name, sounds),
        ButtonAction::PlaySoundBank {
            bank,
            index,
            volume,
        } => sound_ui::bank_sound_editor(ui, cx, bank, index, volume),
        ButtonAction::StopAllSounds => sound_ui::stop_sounds_hint(ui, cx),
        ButtonAction::Monitor { metric } => super::monitor_ui::monitor_editor(ui, cx, metric),
        ButtonAction::Clock => {
            ui.label(widgets::hint(t(CLOCK_HINT), cx.pal));
            false
        }
        ButtonAction::PressHold {
            tap,
            hold,
            threshold_ms,
        } => press_hold_editor(ui, cx, tap, hold, threshold_ms),
        ButtonAction::DoubleTap { action, window_ms } => {
            double_tap_editor(ui, cx, action, window_ms)
        }
        ButtonAction::None | ButtonAction::NextPage | ButtonAction::PreviousPage => false,
    };
    changed
}

const CLOCK_HINT: &str =
    "Mostra a hora e a data na tecla, atualizadas a cada minuto (no fuso do sistema).";
#[cfg(windows)]
const COMMAND_HINT: &str =
    "Roda no cmd (ex.: start ms-settings:). Sem janela; a saída não aparece.";
#[cfg(not(windows))]
const COMMAND_HINT: &str =
    "Roda no sh (ex.: playerctl play-pause, notify-send oi). A saída não aparece.";

/// Uma ação aninhada (o toque ou o segurar de `PressHold`, a ação de `DoubleTap`) num bloco
/// recolhível com o editor de passo.
fn nested_action(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    id: &'static str,
    title: &str,
    action: &mut ButtonAction,
) -> bool {
    let mut changed = false;
    ui.push_id(id, |ui| {
        widgets::fold(ui, id, title, None, false, cx.pal, |ui| {
            changed = step_editor(ui, cx, action);
        });
    });
    changed
}

/// `PressHold`: tocar faz uma ação, segurar além do limiar faz outra.
fn press_hold_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    tap: &mut ButtonAction,
    hold: &mut ButtonAction,
    threshold_ms: &mut u64,
) -> bool {
    widgets::section(ui, "Pressione / segure", cx.pal);
    let mut changed = nested_action(ui, cx, "presshold-tap", "Ao tocar (tap)", tap);
    changed |= nested_action(ui, cx, "presshold-hold", "Ao segurar (hold)", hold);
    widgets::section(ui, "Limiar de segurar (ms)", cx.pal);
    changed |= widgets::numeric_field(ui, threshold_ms, "Limiar de segurar (ms)", cx.pal, 80.0);
    changed
}

/// `DoubleTap`: dois toques dentro da janela fazem a ação.
fn double_tap_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    action: &mut ButtonAction,
    window_ms: &mut u64,
) -> bool {
    widgets::section(ui, "Clique duplo", cx.pal);
    let mut changed = nested_action(ui, cx, "doubletap", "Ação do clique duplo", action);
    widgets::section(ui, "Janela máxima (ms)", cx.pal);
    changed |= widgets::numeric_field(ui, window_ms, "Janela máxima (ms)", cx.pal, 80.0);
    changed
}

fn command_editor(ui: &mut egui::Ui, cx: &PaneCtx, command: &mut String) -> bool {
    let label = widgets::section(ui, "Linha de comando", cx.pal);
    let changed = widgets::text_area(ui, command, "", cx.pal, 2)
        .labelled_by(label)
        .changed();
    ui.label(widgets::hint(t(COMMAND_HINT), cx.pal));
    changed
}

/// Aumentar ou diminuir, e de quanto.
fn brightness_editor(ui: &mut egui::Ui, cx: &PaneCtx, delta: &mut i32) -> bool {
    let mut changed = false;
    widgets::section(ui, "Cada toque", cx.pal);
    let up = *delta >= 0;
    ui.horizontal(|ui| {
        if widgets::pill(ui, "Mais claro", up, cx.accent, cx.pal).clicked() && !up {
            *delta = delta.abs();
            changed = true;
        }
        if widgets::pill(ui, "Mais escuro", !up, cx.accent, cx.pal).clicked() && up {
            *delta = -delta.abs();
            changed = true;
        }
    });
    let mut step = delta.unsigned_abs() as f32;
    step = step.clamp(1.0, 50.0);
    let label = widgets::section(ui, "Passo (%)", cx.pal);
    if widgets::slider(ui, &mut step, 1.0..=50.0, " %")
        .labelled_by(label)
        .changed()
    {
        *delta = if up { step as i32 } else { -(step as i32) };
        changed = true;
    }
    changed
}

fn type_text_editor(ui: &mut egui::Ui, cx: &PaneCtx, text: &mut String) -> bool {
    let label = widgets::section(ui, "Texto a digitar", cx.pal);
    widgets::text_area(ui, text, "", cx.pal, TYPE_TEXT_ROWS as u32)
        .labelled_by(label)
        .changed()
}

fn delay_editor(ui: &mut egui::Ui, cx: &PaneCtx, ms: &mut u64) -> bool {
    let label = widgets::section(ui, "Pausa (ms)", cx.pal);
    ui.add(egui::DragValue::new(ms).range(0..=engine::actions::MAX_DELAY_MS))
        .labelled_by(label)
        .changed()
}

/// Passo de multi-ação: os mesmos campos da ação isolada, menos a gravação de atalho
/// (só um editor por vez pode gravar) e a navegação.
fn step_editor(ui: &mut egui::Ui, cx: &PaneCtx, step: &mut ButtonAction) -> bool {
    match step {
        ButtonAction::Hotkey { keys } => {
            let label = widgets::section(ui, "Atalho", cx.pal);
            let field_id = ui.scope_id().with("step-keys");
            textlist::list_field(
                ui,
                field_id,
                label,
                keys,
                textlist::parse_keys,
                textlist::join_keys,
                cx.pal,
            )
        }
        ButtonAction::LaunchApp { path, args } => launch_editor(ui, cx.pal, "step", path, args),
        ButtonAction::OpenUrl { url } => url_editor(ui, cx, url),
        ButtonAction::HttpWebhook { url, method, body } => {
            webhook_editor(ui, cx, url, method, body)
        }
        ButtonAction::MediaKey { key } => media_editor(ui, cx, key),
        ButtonAction::TypeText { text } => type_text_editor(ui, cx, text),
        ButtonAction::Delay { ms } => delay_editor(ui, cx, ms),
        ButtonAction::RunCommand { command } => command_editor(ui, cx, command),
        ButtonAction::Obs { command, target } => super::obs_ui::obs_editor(ui, cx, command, target),
        ButtonAction::PlaySound { path } => sound_ui::sound_file_editor(ui, cx, path),
        ButtonAction::PlaySoundBank {
            bank,
            index,
            volume,
        } => sound_ui::bank_sound_editor(ui, cx, bank, index, volume),
        ButtonAction::StopAllSounds => sound_ui::stop_sounds_hint(ui, cx),
        ButtonAction::PressHold {
            tap,
            hold,
            threshold_ms,
        } => press_hold_editor(ui, cx, tap, hold, threshold_ms),
        ButtonAction::DoubleTap { action, window_ms } => {
            double_tap_editor(ui, cx, action, window_ms)
        }
        _ => false,
    }
}

/// Lista de passos executados em sequência, cada um recolhível, com subir/descer/remover.
fn multi_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    id: &'static str,
    steps: &mut Vec<ButtonAction>,
) -> bool {
    let mut changed = false;
    widgets::section(ui, "Passos (em ordem)", cx.pal);
    let last = steps.len().saturating_sub(1);
    let (mut remove, mut up, mut down) = (None, None, None);
    for (i, step) in steps.iter_mut().enumerate() {
        ui.push_id((id, i), |ui| {
            ui.horizontal(|ui| {
                if step_button(ui, cx.pal, "lucide:arrow-up", "Subir passo", i > 0).clicked() {
                    up = Some(i);
                }
                if step_button(ui, cx.pal, "lucide:arrow-down", "Descer passo", i < last).clicked()
                {
                    down = Some(i);
                }
                let remove_step = widgets::icon_button_tinted(
                    ui,
                    "trash",
                    "Remover passo",
                    true,
                    cx.pal.bad,
                    cx.pal,
                );
                if remove_step.clicked() {
                    remove = Some(i);
                }
                let title = super::summary::action_summary(step, cx.targets)
                    .unwrap_or_else(|| t(ActionKind::of(step).label()).to_string());
                ui.label(format!("{}. {title}", i + 1));
            });
            widgets::fold(ui, "step-body", "Editar", None, false, cx.pal, |ui| {
                changed |= step_editor(ui, cx, step)
            });
        });
    }
    if let Some(i) = remove {
        steps.remove(i);
        changed = true;
    } else if let Some(i) = up {
        steps.swap(i, i - 1);
        changed = true;
    } else if let Some(i) = down {
        steps.swap(i, i + 1);
        changed = true;
    }
    widgets::section(ui, "Adicionar passo", cx.pal);
    ui.horizontal_wrapped(|ui| {
        for kind in ActionKind::STEPS {
            if widgets::pill(ui, t(kind.label()), false, cx.accent, cx.pal).clicked() {
                steps.push(kind.default_action());
                changed = true;
            }
        }
    });
    changed
}

fn step_button(
    ui: &mut egui::Ui,
    pal: &Palette,
    icon: &str,
    name: &str,
    enabled: bool,
) -> egui::Response {
    widgets::icon_button(ui, icon, name, enabled, pal)
}

fn hotkey_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    id: &'static str,
    keys: &mut Vec<String>,
    rec: &mut Option<&'static str>,
) -> bool {
    let pal = cx.pal;
    let keys_label = widgets::section(ui, "Atalho", pal);
    ui.horizontal_wrapped(|ui| {
        if keys.is_empty() {
            ui.label(RichText::new(t("(vazio)")).color(pal.muted));
        }
        for k in keys.iter() {
            widgets::kbd(ui, k, pal);
        }
    });
    let mut changed = hotkey_recorder(ui, cx, id, keys, rec);
    // Digitar o atalho é para quem precisa de uma tecla que o gravador não pega (ex.: Super).
    widgets::fold(
        ui,
        &format!("{id}-keys-advanced"),
        "Digitar o atalho",
        None,
        false,
        pal,
        |ui| {
            ui.label(
                RichText::new(t("Ex.: meta+d, ctrl+shift+m"))
                    .size(text::CAPTION)
                    .color(pal.muted),
            );
            let field_id = ui.scope_id().with((id, "keys"));
            changed |= textlist::list_field(
                ui,
                field_id,
                keys_label,
                keys,
                textlist::parse_keys,
                textlist::join_keys,
                pal,
            );
        },
    );
    let invalid: Vec<&str> = keys
        .iter()
        .map(String::as_str)
        .filter(|k| !engine::actions::is_valid_key(k))
        .collect();
    if !invalid.is_empty() {
        let msg = tf("Tecla desconhecida: {}", &[&invalid.join(", ")]);
        let shown = ui.label(RichText::new(msg).size(text::CAPTION).color(pal.bad));
        widgets::live_region(ui.ctx(), shown.id);
    }
    changed
}

/// Chave do pedido de devolver o foco ao "Gravar atalho" quando a gravação termina.
const REFOCUS_RECORDER: &str = "refocus-recorder";

/// "Gravar atalho": captura as próximas teclas pressionadas, ou limpa o atalho.
fn hotkey_recorder(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    id: &'static str,
    keys: &mut Vec<String>,
    rec: &mut Option<&'static str>,
) -> bool {
    let mut changed = false;
    if *rec == Some(id) {
        // Gravar não é erro: vermelho aqui diria "algo deu errado" a quem só digita um atalho.
        // Sem "●" de texto: o ícone Lucide é decoração, o texto diz o estado. Região viva: o leitor
        // de tela avisa que a gravação começou (o foco continua no botão).
        let ink = theme::ink(cx.accent, cx.pal);
        ui.horizontal(|ui| {
            widgets::icon(ui, "lucide:circle-dot", text::BODY, ink);
            let msg = ui.label(
                RichText::new(t("Pressione as teclas… (Esc cancela)"))
                    .strong()
                    .color(ink),
            );
            widgets::live_region(ui.ctx(), msg.id);
        });
        match capture_hotkey(cx.ctx) {
            Some(Capture::Keys(k)) => {
                *keys = k;
                *rec = None;
                changed = true;
            }
            Some(Capture::Cancel) => *rec = None,
            None => {}
        }
        if rec.is_none() {
            // O botão sumiu durante a gravação e o foco foi junto: devolve-o a ele (WCAG 2.4.3).
            cx.ctx
                .data_mut(|d| d.insert_temp(egui::Id::unique(REFOCUS_RECORDER).with(id), true));
        }
        return changed;
    }
    ui.horizontal(|ui| {
        let kind = widgets::ButtonKind::Secondary;
        let record = widgets::button(
            ui,
            Some("lucide:circle-dot"),
            "Gravar atalho",
            kind,
            true,
            0.0,
            cx.pal,
        );
        let refocus = egui::Id::unique(REFOCUS_RECORDER).with(id);
        if cx
            .ctx
            .data_mut(|d| d.remove_temp::<bool>(refocus))
            .unwrap_or(false)
        {
            record.request_focus();
        }
        if record.clicked() {
            *rec = Some(id);
        }
        let clear = widgets::button(
            ui,
            Some("eraser"),
            "Limpar",
            widgets::ButtonKind::Ghost,
            true,
            0.0,
            cx.pal,
        );
        clear.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, t("Limpar atalho"))
        });
        if clear.clicked() {
            keys.clear();
            changed = true;
        }
    });
    changed
}

fn launch_editor(
    ui: &mut egui::Ui,
    pal: &Palette,
    id: &'static str,
    path: &mut String,
    args: &mut Vec<String>,
) -> bool {
    // Na frente, só qual programa; caminho e argumentos são detalhe técnico, em "Avançado".
    let shown = super::summary::program_name(path)
        .unwrap_or_else(|| t("Nenhum programa escolhido").to_string());
    let color = if path.trim().is_empty() {
        pal.muted
    } else {
        pal.fg
    };
    ui.label(
        RichText::new(shown)
            .size(text::BODY + 1.0)
            .strong()
            .color(color),
    );
    // A ação principal põe os botões de escolher app entre o nome e o "Avançado" (`panes`).
    if id == MAIN_ACTION_ID {
        return false;
    }
    launch_advanced(ui, pal, id, path, args)
}

/// Caminho do programa e argumentos, recolhidos.
pub(super) fn launch_advanced(
    ui: &mut egui::Ui,
    pal: &Palette,
    id: &'static str,
    path: &mut String,
    args: &mut Vec<String>,
) -> bool {
    let mut changed = false;
    widgets::fold(
        ui,
        &format!("{id}-advanced"),
        "Avançado",
        None,
        false,
        pal,
        |ui| {
            let path_label = widgets::section(ui, "Programa (caminho ou nome)", pal);
            changed |= widgets::text_field(ui, path, "", pal, false)
                .labelled_by(path_label)
                .changed();
            let args_label = widgets::section(ui, "Argumentos", pal);
            let field_id = ui.scope_id().with((id, "args"));
            changed |= textlist::list_field(
                ui,
                field_id,
                args_label,
                args,
                textlist::parse_args,
                textlist::join_args,
                pal,
            );
        },
    );
    changed
}

/// Endereço do site, com o aviso no próprio campo enquanto não for um http(s) completo.
fn url_editor(ui: &mut egui::Ui, cx: &PaneCtx, url: &mut String) -> bool {
    let label = widgets::section(ui, "Endereço do site", cx.pal);
    let changed = widgets::text_field(ui, url, "https://exemplo.com", cx.pal, false)
        .labelled_by(label)
        .changed();
    if !engine::actions::is_web_url(url) {
        ui.label(widgets::hint(
            t("Comece com https:// (ou http://)."),
            cx.pal,
        ));
    }
    changed
}

fn webhook_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    url: &mut String,
    method: &mut String,
    body: &mut Option<String>,
) -> bool {
    let url_label = widgets::section(ui, "URL", cx.pal);
    let mut changed = widgets::text_field(ui, url, "", cx.pal, false)
        .labelled_by(url_label)
        .changed();
    if !engine::actions::is_web_url(url) {
        ui.label(widgets::hint(
            t("Comece com https:// (ou http://)."),
            cx.pal,
        ));
    }
    let summary = Some(method.to_uppercase());
    widgets::fold(
        ui,
        "webhook-advanced",
        "Avançado",
        summary,
        false,
        cx.pal,
        |ui| {
            widgets::section(ui, "Método", cx.pal);
            ui.horizontal_wrapped(|ui| {
                for m in engine::actions::WEBHOOK_METHODS {
                    let on = method.eq_ignore_ascii_case(m);
                    if widgets::pill(ui, m, on, cx.accent, cx.pal).clicked() {
                        *method = m.to_string();
                        changed = true;
                    }
                }
            });
            let body_label = widgets::section(ui, "Corpo da requisição", cx.pal);
            let mut s = body.clone().unwrap_or_default();
            let changed_body = widgets::text_area(ui, &mut s, "", cx.pal, BODY_ROWS as u32)
                .labelled_by(body_label)
                .changed();
            if changed_body {
                *body = (!s.is_empty()).then_some(s);
                changed = true;
            }
        },
    );
    changed
}

fn media_editor(ui: &mut egui::Ui, cx: &PaneCtx, key: &mut String) -> bool {
    widgets::section(ui, "Tecla de mídia", cx.pal);
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (k, label) in MEDIA_KEYS {
            if widgets::pill(ui, label, key == k, cx.accent, cx.pal).clicked() {
                *key = k.to_string();
                changed = true;
            }
        }
    });
    changed
}
