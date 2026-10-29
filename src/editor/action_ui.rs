//! Editor da ação de um botão: escolha do tipo (atalho, app, web...) e os campos de cada tipo.
use super::{textlist, widgets};
use crate::config::schema::*;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::{radius, text};
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

/// Destinos possíveis das ações de navegação (id estável + nome exibido).
pub struct Targets {
    pages: Vec<(u64, String)>,
    profiles: Vec<(u64, String)>,
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
        Self { pages, profiles }
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

const ACTION_KINDS: [(&str, &str); 12] = [
    ("none", "Nenhuma"),
    ("hotkey", "Atalho"),
    ("launch-app", "App"),
    ("http-webhook", "Web"),
    ("media-key", "Mídia"),
    ("type-text", "Texto"),
    ("multi-action", "Multi"),
    ("switch-page", "Página"),
    ("next-page", "Pág. +"),
    ("previous-page", "Pág. −"),
    ("switch-profile", "Perfil"),
    ("open-mixer", "Mixer"),
];

/// Tipos que podem ser passo de uma multi-ação (navegação não cabe: o serviço a ignora lá dentro).
const STEP_KINDS: [(&str, &str); 6] = [
    ("hotkey", "Atalho"),
    ("type-text", "Texto"),
    ("launch-app", "App"),
    ("http-webhook", "Web"),
    ("media-key", "Mídia"),
    ("delay", "Pausa"),
];

const DEFAULT_STEP_DELAY_MS: u64 = 500;
const STEP_BUTTON: f32 = 24.0;
const TYPE_TEXT_ROWS: usize = 3;

const MEDIA_KEYS: [(&str, &str); 6] = [
    ("playpause", "Play/Pause"),
    ("next", "Próxima"),
    ("prev", "Anterior"),
    ("volup", "Vol +"),
    ("voldown", "Vol −"),
    ("mute", "Mudo"),
];

const BODY_ROWS: usize = 3;
const KIND_COLUMNS: usize = 4;
const KIND_GAP: f32 = 6.0;
const KIND_TILE_HEIGHT: f32 = 54.0;
const KIND_ICON: f32 = 18.0;
const KIND_ICON_TOP: f32 = 19.0;
const KIND_LABEL_BOTTOM: f32 = 13.0;
const KIND_ON_TINT: f32 = 0.12;
const KIND_HOVER_TINT: f32 = 0.08;

/// Explica a ação "Mixer": o que aparece no aparelho e como sair.
const MIXER_HINT: &str = "Mostra no aparelho os apps com som: ícone com a porcentagem em cima, + no meio e − embaixo. Esta tecla vira SAIR (segure para sair se houver mais de 5 apps).";

fn action_name(a: &ButtonAction) -> &'static str {
    match a {
        ButtonAction::None => "none",
        ButtonAction::Hotkey { .. } => "hotkey",
        ButtonAction::LaunchApp { .. } => "launch-app",
        ButtonAction::HttpWebhook { .. } => "http-webhook",
        ButtonAction::MediaKey { .. } => "media-key",
        ButtonAction::SwitchPage { .. } => "switch-page",
        ButtonAction::SwitchProfile { .. } => "switch-profile",
        ButtonAction::OpenMixer => "open-mixer",
        ButtonAction::TypeText { .. } => "type-text",
        ButtonAction::MultiAction { .. } => "multi-action",
        ButtonAction::Delay { .. } => "delay",
        ButtonAction::NextPage => "next-page",
        ButtonAction::PreviousPage => "previous-page",
    }
}

fn default_action(kind: &str) -> ButtonAction {
    match kind {
        "hotkey" => ButtonAction::Hotkey { keys: vec![] },
        "launch-app" => ButtonAction::LaunchApp {
            path: String::new(),
            args: vec![],
        },
        "http-webhook" => ButtonAction::HttpWebhook {
            url: String::new(),
            method: "GET".into(),
            body: None,
        },
        "media-key" => ButtonAction::MediaKey {
            key: "playpause".into(),
        },
        "switch-page" => ButtonAction::SwitchPage { page: 0 },
        "switch-profile" => ButtonAction::SwitchProfile { profile: 0 },
        "open-mixer" => ButtonAction::OpenMixer,
        "type-text" => ButtonAction::TypeText {
            text: String::new(),
        },
        "multi-action" => ButtonAction::MultiAction { steps: vec![] },
        "delay" => ButtonAction::Delay {
            ms: DEFAULT_STEP_DELAY_MS,
        },
        "next-page" => ButtonAction::NextPage,
        "previous-page" => ButtonAction::PreviousPage,
        _ => ButtonAction::None,
    }
}

fn pick_target(
    ui: &mut egui::Ui,
    id: &str,
    label: egui::Id,
    value: &mut u64,
    options: &[(u64, String)],
) -> bool {
    let mut changed = false;
    let shown = options
        .iter()
        .find(|o| o.0 == *value)
        .map_or(t("(escolha)").to_string(), |o| o.1.clone());
    let combo = egui::ComboBox::from_id_salt((id, "dest"))
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (oid, name) in options {
                if ui.selectable_label(*value == *oid, name).clicked() && *value != *oid {
                    *value = *oid;
                    changed = true;
                }
            }
        });
    combo.response.labelled_by(label);
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

/// Seletor do tipo de ação; trocar o tipo recomeça com os valores padrão do novo tipo.
fn kind_picker(ui: &mut egui::Ui, cx: &PaneCtx, a: &mut ButtonAction) -> bool {
    let mut changed = false;
    let gap = KIND_GAP;
    let width = (ui.available_width() - gap * (KIND_COLUMNS - 1) as f32) / KIND_COLUMNS as f32;
    for row in ACTION_KINDS.chunks(KIND_COLUMNS) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for &(kind, label) in row {
                let on = action_name(a) == kind;
                if kind_tile(ui, kind, label, on, width, cx).clicked() && !on {
                    *a = default_action(kind);
                    changed = true;
                }
            }
        });
    }
    changed
}

/// Bloco da grade de tipos: ícone em cima, nome embaixo; o escolhido em destaque.
fn kind_tile(
    ui: &mut egui::Ui,
    kind: &str,
    label: &str,
    on: bool,
    width: f32,
    cx: &PaneCtx,
) -> egui::Response {
    let label = t(label);
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, KIND_TILE_HEIGHT), egui::Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label)
    });
    let pal = cx.pal;
    let ink = theme::ink(cx.accent, pal);
    let round = egui::Rounding::same(radius::CONTROL);
    let (fill, fg) = if on {
        (cx.accent.gamma_multiply(KIND_ON_TINT), ink)
    } else if resp.hovered() || resp.has_focus() {
        (theme::mix(pal.raise, pal.fg, KIND_HOVER_TINT), pal.fg)
    } else {
        (pal.raise, pal.fg)
    };
    let p = ui.painter();
    p.rect_filled(rect, round, fill);
    if on {
        p.rect_stroke(rect, round, egui::Stroke::new(1.5_f32, ink));
    }
    let icon_at = egui::pos2(rect.center().x, rect.top() + KIND_ICON_TOP);
    widgets::paint_icon(p, icon_at, KIND_ICON, kind_icon(kind), fg);
    p.text(
        egui::pos2(rect.center().x, rect.bottom() - KIND_LABEL_BOTTOM),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(text::CAPTION),
        fg,
    );
    resp
}

fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "hotkey" => "keyboard",
        "launch-app" => "app",
        "http-webhook" => "globe",
        "media-key" => "music",
        "switch-page" => "page",
        "switch-profile" => "user",
        "open-mixer" => "mixer",
        "type-text" => "keyboard",
        "multi-action" => "plus",
        "next-page" => "next",
        "previous-page" => "prev",
        _ => "ban",
    }
}

/// Editor completo da ação `a`. `id` separa o estado de "Ao tocar" e "Ao segurar".
pub fn action_editor(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    id: &'static str,
    a: &mut ButtonAction,
    rec: &mut Option<&'static str>,
) -> bool {
    let mut changed = kind_picker(ui, cx, a);
    changed |= match a {
        ButtonAction::Hotkey { keys } => hotkey_editor(ui, cx, id, keys, rec),
        ButtonAction::LaunchApp { path, args } => launch_editor(ui, cx.pal, id, path, args),
        ButtonAction::HttpWebhook { url, method, body } => {
            webhook_editor(ui, cx, url, method, body)
        }
        ButtonAction::MediaKey { key } => media_editor(ui, cx, key),
        ButtonAction::SwitchPage { page } => {
            let label = widgets::section(ui, "Página de destino", cx.pal);
            pick_target(ui, id, label, page, &cx.targets.pages)
        }
        ButtonAction::SwitchProfile { profile } => {
            let label = widgets::section(ui, "Perfil de destino", cx.pal);
            pick_target(ui, id, label, profile, &cx.targets.profiles)
        }
        ButtonAction::OpenMixer => {
            ui.label(widgets::hint(t(MIXER_HINT), cx.pal));
            false
        }
        ButtonAction::TypeText { text } => type_text_editor(ui, cx, text),
        ButtonAction::MultiAction { steps } => multi_editor(ui, cx, id, steps),
        ButtonAction::Delay { ms } => delay_editor(ui, cx, ms),
        ButtonAction::None | ButtonAction::NextPage | ButtonAction::PreviousPage => false,
    };
    changed
}

fn type_text_editor(ui: &mut egui::Ui, cx: &PaneCtx, text: &mut String) -> bool {
    let label = widgets::section(ui, "Texto a digitar", cx.pal);
    ui.add(
        egui::TextEdit::multiline(text)
            .desired_width(ui.available_width())
            .desired_rows(TYPE_TEXT_ROWS),
    )
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
            let field_id = ui.id().with("step-keys");
            textlist::list_field(
                ui,
                field_id,
                label,
                keys,
                textlist::parse_keys,
                textlist::join_keys,
            )
        }
        ButtonAction::LaunchApp { path, args } => launch_editor(ui, cx.pal, "step", path, args),
        ButtonAction::HttpWebhook { url, method, body } => {
            webhook_editor(ui, cx, url, method, body)
        }
        ButtonAction::MediaKey { key } => media_editor(ui, cx, key),
        ButtonAction::TypeText { text } => type_text_editor(ui, cx, text),
        ButtonAction::Delay { ms } => delay_editor(ui, cx, ms),
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
                if step_button(ui, "↑", "Subir passo", i > 0).clicked() {
                    up = Some(i);
                }
                if step_button(ui, "↓", "Descer passo", i < last).clicked() {
                    down = Some(i);
                }
                if widgets::icon_button(ui, "trash", t("Remover passo"), true, cx.pal).clicked() {
                    remove = Some(i);
                }
                let title = super::summary::action_summary(step, cx.targets)
                    .unwrap_or_else(|| t(action_label(step)).to_string());
                ui.label(format!("{}. {title}", i + 1));
            });
            egui::CollapsingHeader::new(t("Editar"))
                .id_salt("step-body")
                .show(ui, |ui| changed |= step_editor(ui, cx, step));
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
        for (kind, label) in STEP_KINDS {
            if widgets::pill(ui, t(label), false, cx.accent, cx.pal).clicked() {
                steps.push(default_action(kind));
                changed = true;
            }
        }
    });
    changed
}

fn step_button(ui: &mut egui::Ui, glyph: &str, name: &str, enabled: bool) -> egui::Response {
    let button = egui::Button::new(glyph).min_size(egui::vec2(STEP_BUTTON, STEP_BUTTON));
    let resp = ui.add_enabled(enabled, button);
    let name = t(name);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    resp
}

/// Nome do tipo (para passar por `t`) quando o passo ainda não tem o que resumir.
fn action_label(a: &ButtonAction) -> &'static str {
    let kind = action_name(a);
    STEP_KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or("Nenhuma", |(_, label)| label)
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
    ui.label(
        RichText::new(t("Digitar (ex: meta+d)"))
            .size(text::CAPTION)
            .color(pal.muted),
    );
    let field_id = ui.id().with((id, "keys"));
    changed |= textlist::list_field(
        ui,
        field_id,
        keys_label,
        keys,
        textlist::parse_keys,
        textlist::join_keys,
    );
    let invalid: Vec<&str> = keys
        .iter()
        .map(String::as_str)
        .filter(|k| !engine::actions::is_valid_key(k))
        .collect();
    if !invalid.is_empty() {
        let msg = tf("Tecla desconhecida: {}", &[&invalid.join(", ")]);
        ui.label(RichText::new(msg).size(text::CAPTION).color(pal.bad));
    }
    changed
}

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
        ui.label(
            RichText::new(t("● Pressione as teclas… (Esc cancela)"))
                .strong()
                .color(theme::ink(cx.accent, cx.pal)),
        );
        match capture_hotkey(cx.ctx) {
            Some(Capture::Keys(k)) => {
                *keys = k;
                *rec = None;
                changed = true;
            }
            Some(Capture::Cancel) => *rec = None,
            None => {}
        }
        return changed;
    }
    ui.horizontal(|ui| {
        if ui.button(t("● Gravar atalho")).clicked() {
            *rec = Some(id);
        }
        let clear = ui.button(t("Limpar"));
        clear.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("Limpar atalho"))
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
    let path_label = widgets::section(ui, "Programa (caminho ou nome)", pal);
    let mut changed = ui
        .add(egui::TextEdit::singleline(path).desired_width(ui.available_width()))
        .labelled_by(path_label)
        .changed();
    let args_label = widgets::section(ui, "Argumentos", pal);
    let field_id = ui.id().with((id, "args"));
    changed |= textlist::list_field(
        ui,
        field_id,
        args_label,
        args,
        textlist::parse_args,
        textlist::join_args,
    );
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
    let mut changed = ui
        .add(egui::TextEdit::singleline(url).desired_width(ui.available_width()))
        .labelled_by(url_label)
        .changed();
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
    let edit = egui::TextEdit::multiline(&mut s)
        .desired_width(ui.available_width())
        .desired_rows(BODY_ROWS);
    if ui.add(edit).labelled_by(body_label).changed() {
        *body = (!s.is_empty()).then_some(s);
        changed = true;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_kind_round_trips_through_its_default() {
        for (kind, _) in ACTION_KINDS {
            assert_eq!(action_name(&default_action(kind)), kind);
        }
    }
}
