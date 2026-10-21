//! Painéis do inspetor (Aparência, Ação, Estados) e o editor de ações.
use crate::config::schema::*;
use crate::picker::{app_buttons, AppPicker};
use crate::theme::Palette;
use crate::tokens::text;
use crate::{c32, engine, icons, library, textlist, widgets};
use eframe::egui::{self, vec2, Color32, RichText, Rounding, Sense, Stroke};
const IMAGE_FIELD_RESERVED: f32 = 172.0;
const SWATCH_SIZE: f32 = 26.0;
const SWATCH_RADIUS: f32 = 8.0;
const SWATCH_GAP: f32 = 8.0;
const SLIDER_VALUE_RESERVED: f32 = 72.0; // caixa numérica à direita do slider
const SLIDER_MIN_WIDTH: f32 = 100.0;

/// Estica a trilha do próximo slider até a borda do painel (a padrão tem só 100 px).
fn fill_slider_width(ui: &mut egui::Ui) {
    ui.spacing_mut().slider_width =
        (ui.available_width() - SLIDER_VALUE_RESERVED).max(SLIDER_MIN_WIDTH);
}

fn pick_icon() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("Imagem (PNG, JPG)", &["png", "jpg", "jpeg"])
        .pick_file()
        .map(|p| p.display().to_string())
}

/// Aparência inicial do estado "ligado": a mesma, com o fundo mais claro.
fn lit_variant(a: &ButtonAppearance) -> ButtonAppearance {
    let bg = a.background;
    let mut lit = a.clone();
    lit.background = RGBAColor {
        r: bg.r.saturating_add(80),
        g: bg.g.saturating_add(80),
        b: bg.b.saturating_add(80),
        a: bg.a,
    };
    lit
}

fn color_picker(ui: &mut egui::Ui, label: &str, c: &mut RGBAColor) -> bool {
    let mut changed = false;
    ui.label(label);
    let mut rgb = [c.r, c.g, c.b];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        (c.r, c.g, c.b) = (rgb[0], rgb[1], rgb[2]);
        changed = true;
    }
    changed
}

pub fn appearance_pane(
    ui: &mut egui::Ui,
    b: &mut ButtonConfig,
    pal: &Palette,
    accent: Color32,
) -> bool {
    let mut changed = false;
    widgets::section(ui, "Rótulo", pal);
    changed |= ui
        .add(
            egui::TextEdit::singleline(&mut b.appearance.text)
                .hint_text(widgets::hint("Texto na tecla (ex.: Mudo)", pal))
                .desired_width(ui.available_width()),
        )
        .changed();
    widgets::section(ui, "Cor do botão", pal);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(SWATCH_GAP, SWATCH_GAP);
        for sw in library::SWATCHES {
            let color = RGBAColor {
                r: sw[0],
                g: sw[1],
                b: sw[2],
                a: 255,
            };
            let on = b.appearance.background == color;
            if widgets::swatch(ui, SWATCH_SIZE, SWATCH_RADIUS, c32(color), on, pal.fg).clicked() {
                b.appearance.background = color;
                changed = true;
            }
        }
    });
    ui.horizontal(|ui| {
        changed |= color_picker(ui, "Outra", &mut b.appearance.background);
        ui.add_space(SWATCH_GAP);
        changed |= color_picker(ui, "Texto", &mut b.appearance.text_color);
    });
    widgets::section(ui, "Ícone", pal);
    let ink = crate::theme::ink(accent, pal);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        let none_on = b.appearance.icon.is_none();
        let options = std::iter::once(("", "Sem ícone")).chain(icons::ICONS);
        for (id, name) in options {
            let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
            let on = if id.is_empty() {
                none_on
            } else {
                b.appearance.icon.as_deref() == Some(id)
            };
            let p = ui.painter();
            p.rect_filled(rect, Rounding::same(8.0), pal.field);
            p.rect_stroke(
                rect,
                Rounding::same(8.0),
                Stroke::new(
                    if on { 2.0_f32 } else { 1.0_f32 },
                    if on { ink } else { pal.control },
                ),
            );
            if id.is_empty() {
                p.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "–",
                    egui::FontId::monospace(16.0),
                    pal.muted,
                );
            } else {
                widgets::paint_icon(p, rect.center(), 20.0, id, if on { ink } else { pal.fg });
            }
            if resp.on_hover_text(name).clicked() {
                b.appearance.icon = (!id.is_empty()).then(|| id.to_string());
                if !id.is_empty() {
                    b.appearance.icon_path = None;
                }
                changed = true;
            }
        }
    });
    widgets::section(ui, "Imagem (PNG ou JPG)", pal);
    let mut path = b.appearance.icon_path.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        let mut edited = ui
            .add(
                egui::TextEdit::singleline(&mut path)
                    .desired_width((ui.available_width() - IMAGE_FIELD_RESERVED).max(60.0)),
            )
            .changed();
        if ui.button("Escolher").clicked() {
            if let Some(p) = pick_icon() {
                path = p;
                edited = true;
            }
        }
        if ui.button("Limpar").clicked() {
            path.clear();
            edited = true;
        }
        if edited {
            b.appearance.icon_path = (!path.is_empty()).then(|| path.clone());
            if !path.is_empty() {
                b.appearance.icon = None;
            }
            changed = true;
        }
    });
    widgets::section(ui, "Tamanho da fonte", pal);
    fill_slider_width(ui);
    changed |= ui
        .add(egui::Slider::new(
            &mut b.appearance.font_size,
            8.0_f32..=48.0,
        ))
        .changed();
    changed
}

#[allow(clippy::too_many_arguments)]
pub fn action_pane(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    b: &mut ButtonConfig,
    targets: &Targets,
    rec: &mut Option<&'static str>,
    picker: &mut AppPicker,
    pal: &Palette,
    accent: Color32,
) -> bool {
    let mut changed = false;
    widgets::section(ui, "Ao tocar", pal);
    changed |= action_editor(ui, ctx, "act", &mut b.action, targets, rec, pal, accent);
    if matches!(b.action, ButtonAction::LaunchApp { .. }) {
        changed |= app_buttons(ui, b, picker);
    }
    widgets::section(ui, "Ao segurar (opcional)", pal);
    let mut has_long = b.long_press_action.is_some();
    if ui
        .checkbox(&mut has_long, "Ativar ação ao segurar")
        .changed()
    {
        b.long_press_action = has_long.then_some(ButtonAction::None);
        changed = true;
    }
    if let Some(a) = b.long_press_action.as_mut() {
        changed |= action_editor(ui, ctx, "lact", a, targets, rec, pal, accent);
    }
    changed
}

pub fn states_pane(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    widgets::section(ui, "Alternar entre dois estados", pal);
    let mut has_toggle = b.toggled_appearance.is_some();
    if ui
        .checkbox(&mut has_toggle, "Cada toque troca a aparência")
        .changed()
    {
        b.toggled_appearance = has_toggle.then(|| lit_variant(&b.appearance));
        b.toggle_state = false;
        changed = true;
    }
    if let Some(t) = b.toggled_appearance.as_mut() {
        ui.horizontal(|ui| {
            ui.label("Ligado:");
            changed |= ui
                .add(egui::TextEdit::singleline(&mut t.text).desired_width(120.0))
                .changed();
            changed |= color_picker(ui, "Fundo", &mut t.background);
        });
        changed |= ui
            .checkbox(&mut b.toggle_state, "Ver o estado ligado na prévia")
            .changed();
    }
    widgets::section(ui, "Segurar a partir de", pal);
    fill_slider_width(ui);
    changed |= ui
        .add(egui::Slider::new(&mut b.long_press_threshold_ms, 100..=2000).suffix(" ms"))
        .changed();
    changed
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
            .map(|p| (p.id, p.name.clone()))
            .collect();
        let profiles = cfg
            .profiles
            .iter()
            .map(|p| (p.id, p.name.clone()))
            .collect();
        Self { pages, profiles }
    }
}

fn pick_target(ui: &mut egui::Ui, id: &str, value: &mut u64, options: &[(u64, String)]) -> bool {
    let mut changed = false;
    let shown = options
        .iter()
        .find(|o| o.0 == *value)
        .map_or("(escolha)".to_string(), |o| o.1.clone());
    egui::ComboBox::from_id_salt((id, "dest"))
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (oid, name) in options {
                if ui.selectable_label(*value == *oid, name).clicked() && *value != *oid {
                    *value = *oid;
                    changed = true;
                }
            }
        });
    changed
}

const ACTION_KINDS: [(&str, &str); 7] = [
    ("none", "Nenhuma"),
    ("hotkey", "Atalho"),
    ("launch-app", "App"),
    ("http-webhook", "Web"),
    ("media-key", "Mídia"),
    ("switch-page", "Página"),
    ("switch-profile", "Perfil"),
];

fn action_name(a: &ButtonAction) -> &'static str {
    match a {
        ButtonAction::None => "none",
        ButtonAction::Hotkey { .. } => "hotkey",
        ButtonAction::LaunchApp { .. } => "launch-app",
        ButtonAction::HttpWebhook { .. } => "http-webhook",
        ButtonAction::MediaKey { .. } => "media-key",
        ButtonAction::SwitchPage { .. } => "switch-page",
        ButtonAction::SwitchProfile { .. } => "switch-profile",
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
        _ => ButtonAction::None,
    }
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
            if modifiers.ctrl {
                keys.push("ctrl".to_string());
            }
            if modifiers.alt {
                keys.push("alt".to_string());
            }
            if modifiers.shift {
                keys.push("shift".to_string());
            }
            keys.push(name);
            Some(Capture::Keys(keys))
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn action_editor(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    id: &'static str,
    a: &mut ButtonAction,
    targets: &Targets,
    rec: &mut Option<&'static str>,
    pal: &Palette,
    accent: Color32,
) -> bool {
    let mut ch = false;
    ui.horizontal_wrapped(|ui| {
        for (kind, label) in ACTION_KINDS {
            let on = action_name(a) == kind;
            if widgets::pill(ui, label, on, accent, pal).clicked() && !on {
                *a = default_action(kind);
                ch = true;
            }
        }
    });
    match a {
        ButtonAction::Hotkey { keys } => {
            widgets::section(ui, "Atalho", pal);
            ui.horizontal_wrapped(|ui| {
                if keys.is_empty() {
                    ui.label(RichText::new("(vazio)").color(pal.muted));
                }
                for k in keys.iter() {
                    widgets::kbd(ui, k, pal);
                }
            });
            if *rec == Some(id) {
                // Gravar não é erro: vermelho aqui diria "algo deu errado" a quem está só digitando um atalho.
                ui.label(
                    RichText::new("● Pressione as teclas… (Esc cancela)")
                        .strong()
                        .color(crate::theme::ink(accent, pal)),
                );
                match capture_hotkey(ctx) {
                    Some(Capture::Keys(k)) => {
                        *keys = k;
                        *rec = None;
                        ch = true;
                    }
                    Some(Capture::Cancel) => *rec = None,
                    None => {}
                }
            } else {
                ui.horizontal(|ui| {
                    if ui.button("● Gravar atalho").clicked() {
                        *rec = Some(id);
                    }
                    if ui.button("Limpar").clicked() {
                        keys.clear();
                        ch = true;
                    }
                });
            }
            ui.label(
                RichText::new("Digitar (ex: meta+d)")
                    .size(text::CAPTION)
                    .color(pal.muted),
            );
            let field_id = ui.id().with((id, "keys"));
            ch |= textlist::list_field(
                ui,
                field_id,
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
                ui.label(
                    RichText::new(format!("Tecla desconhecida: {}", invalid.join(", ")))
                        .size(text::CAPTION)
                        .color(pal.bad),
                );
            }
        }
        ButtonAction::LaunchApp { path, args } => {
            widgets::section(ui, "Programa (caminho ou nome)", pal);
            ch |= ui
                .add(egui::TextEdit::singleline(path).desired_width(ui.available_width()))
                .changed();
            widgets::section(ui, "Argumentos", pal);
            let field_id = ui.id().with((id, "args"));
            ch |= textlist::list_field(
                ui,
                field_id,
                args,
                textlist::parse_args,
                textlist::join_args,
            );
        }
        ButtonAction::HttpWebhook { url, method, body } => {
            widgets::section(ui, "URL", pal);
            ch |= ui
                .add(egui::TextEdit::singleline(url).desired_width(ui.available_width()))
                .changed();
            widgets::section(ui, "Método", pal);
            ui.horizontal_wrapped(|ui| {
                for m in engine::actions::WEBHOOK_METHODS {
                    if widgets::pill(ui, m, method.eq_ignore_ascii_case(m), accent, pal).clicked() {
                        *method = m.to_string();
                        ch = true;
                    }
                }
            });
            widgets::section(ui, "Corpo da requisição", pal);
            let mut s = body.clone().unwrap_or_default();
            if ui
                .add(
                    egui::TextEdit::multiline(&mut s)
                        .desired_width(ui.available_width())
                        .desired_rows(3),
                )
                .changed()
            {
                *body = (!s.is_empty()).then_some(s);
                ch = true;
            }
        }
        ButtonAction::MediaKey { key } => {
            widgets::section(ui, "Tecla de mídia", pal);
            ui.horizontal_wrapped(|ui| {
                for (k, label) in [
                    ("playpause", "Play/Pause"),
                    ("next", "Próxima"),
                    ("prev", "Anterior"),
                    ("volup", "Vol +"),
                    ("voldown", "Vol −"),
                    ("mute", "Mudo"),
                ] {
                    if widgets::pill(ui, label, key == k, accent, pal).clicked() {
                        *key = k.to_string();
                        ch = true;
                    }
                }
            });
        }
        ButtonAction::SwitchPage { page } => {
            widgets::section(ui, "Página de destino", pal);
            ch |= pick_target(ui, id, page, &targets.pages);
        }
        ButtonAction::SwitchProfile { profile } => {
            widgets::section(ui, "Perfil de destino", pal);
            ch |= pick_target(ui, id, profile, &targets.profiles);
        }
        ButtonAction::None => {}
    }
    ch
}
