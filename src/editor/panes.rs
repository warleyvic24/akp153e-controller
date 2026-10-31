//! Painéis do inspetor (Aparência, Ação, Alternar) e o editor de ações.
use super::{c32, library, widgets};
use crate::config::schema::*;
use crate::editor::action_ui::{action_editor, PaneCtx};
use crate::editor::picker::{app_buttons, AppPicker};
use crate::editor::theme::Palette;
use crate::strings::t;
use eframe::egui::{self, vec2, Color32};
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

/// Extensões de imagem aceitas para a tecla. PNG e JPG são usados como estão; SVG e ICO viram PNG
/// na pasta de ícones (o desenho da tecla só lê PNG e JPG).
pub(crate) const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "svg", "ico"];

fn pick_icon() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter(t("Imagem (PNG, JPG, SVG, ICO)"), &IMAGE_EXTENSIONS)
        .pick_file()
        .and_then(|p| import_image(&p))
}

/// Caminho da imagem a gravar na tecla, convertendo SVG e ICO para PNG; `None` se não for imagem.
pub(crate) fn import_image(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return None;
    }
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
        return Some(path.display().to_string());
    }
    let bytes = std::fs::read(path).ok()?;
    crate::platform::save_icon(&path.display().to_string(), &bytes)
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
    ui.label(t(label));
    let mut rgb = [c.r, c.g, c.b];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        (c.r, c.g, c.b) = (rgb[0], rgb[1], rgb[2]);
        changed = true;
    }
    changed
}

/// Rótulo (até 3 linhas, Enter quebra) e onde ele fica na tecla.
fn label_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    let label = widgets::section(ui, "Rótulo", pal);
    changed |= ui
        .add(
            egui::TextEdit::multiline(&mut b.appearance.text)
                .hint_text(widgets::hint(
                    "Texto na tecla (ex.: Mudo). Enter quebra a linha",
                    pal,
                ))
                .desired_rows(1)
                .desired_width(ui.available_width()),
        )
        .labelled_by(label)
        .changed();
    let positions = [
        (TextPosition::Top, "Topo"),
        (TextPosition::Middle, "Meio"),
        (TextPosition::Bottom, "Base"),
    ];
    // `Auto` aparece como a posição que ele produz (base com ícone, meio sem).
    let has_icon = b.appearance.icon.is_some() || b.appearance.icon_path.is_some();
    let shown = match b.appearance.text_position {
        TextPosition::Auto if has_icon => TextPosition::Bottom,
        TextPosition::Auto => TextPosition::Middle,
        other => other,
    };
    let width =
        (ui.available_width() - 6.0 - 2.0 * (positions.len() - 1) as f32) / positions.len() as f32;
    widgets::segmented(ui, pal, |ui| {
        for (position, name) in positions {
            if widgets::segment(ui, name, shown == position, width, pal).clicked() {
                b.appearance.text_position = position;
                changed = true;
            }
        }
    });
    changed
}

fn color_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
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
            let name = format!("#{:02X}{:02X}{:02X}", sw[0], sw[1], sw[2]);
            let swatch = widgets::swatch(
                ui,
                SWATCH_SIZE,
                SWATCH_RADIUS,
                c32(color),
                on,
                pal.fg,
                &name,
            );
            if swatch.clicked() {
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
    changed
}

fn image_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    widgets::section(ui, "Imagem", pal);
    let path = b.appearance.icon_path.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        // O nome do arquivo, não o caminho inteiro (que fica na dica): o campo cabe no painel.
        let shown = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        match shown {
            Some(name) => {
                ui.label(egui::RichText::new(name).color(pal.fg))
                    .on_hover_text(&path);
            }
            None => {
                ui.label(egui::RichText::new(t("Nenhuma")).color(pal.muted));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !path.is_empty() {
                let clear = ui.button(t("Limpar"));
                // O nome falado contém o texto visível (WCAG 2.5.3) e diz o que será limpo.
                clear.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("Limpar imagem"))
                });
                if clear.clicked() {
                    b.appearance.icon_path = None;
                    changed = true;
                }
            }
            if ui.button(t("Escolher")).clicked() {
                if let Some(p) = pick_icon() {
                    b.appearance.icon_path = Some(p);
                    b.appearance.icon = None;
                    changed = true;
                }
            }
        });
    });
    let hint = t("PNG, JPG, SVG ou ICO. Também dá para soltar o arquivo sobre a tecla.");
    ui.label(
        egui::RichText::new(hint)
            .size(crate::editor::tokens::text::CAPTION)
            .color(pal.muted),
    );
    changed
}

fn font_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    let label = widgets::section(ui, "Tamanho da fonte", pal);
    fill_slider_width(ui);
    changed |= ui
        .add(
            egui::Slider::new(&mut b.appearance.font_size, 8.0_f32..=48.0)
                .step_by(1.0)
                .fixed_decimals(0)
                .suffix(" px"),
        )
        .labelled_by(label)
        .changed();
    changed
}

pub fn appearance_pane(
    ui: &mut egui::Ui,
    b: &mut ButtonConfig,
    pal: &Palette,
    accent: Color32,
) -> bool {
    let mut changed = label_section(ui, b, pal);
    changed |= color_section(ui, b, pal);
    changed |= super::icon_picker::icon_section(ui, &mut b.appearance, "normal", pal, accent);
    changed |= image_section(ui, b, pal);
    changed |= font_section(ui, b, pal);
    changed
}

pub fn action_pane(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    b: &mut ButtonConfig,
    rec: &mut Option<&'static str>,
    picker: &mut AppPicker,
) -> bool {
    let pal = cx.pal;
    widgets::section(ui, "Ao tocar", pal);
    let mut changed = action_editor(ui, cx, "act", &mut b.action, rec);
    if matches!(b.action, ButtonAction::LaunchApp { .. }) {
        changed |= app_buttons(ui, b, picker);
    }
    widgets::section(ui, "Ao segurar (opcional)", pal);
    let mut has_long = b.long_press_action.is_some();
    if ui
        .checkbox(&mut has_long, t("Ativar ação ao segurar"))
        .changed()
    {
        b.long_press_action = has_long.then_some(ButtonAction::None);
        changed = true;
    }
    if let Some(a) = b.long_press_action.as_mut() {
        changed |= action_editor(ui, cx, "lact", a, rec);
        // O tempo só importa com uma ação ao segurar: fica junto dela, não escondido em outra aba.
        let label = widgets::section(ui, "Segurar a partir de", pal);
        fill_slider_width(ui);
        changed |= ui
            .add(egui::Slider::new(&mut b.long_press_threshold_ms, 100..=2000).suffix(" ms"))
            .labelled_by(label)
            .changed();
    }
    changed
}

/// Aba "Alternar": tecla de dois estados. Cada toque troca entre o normal e o ligado; o ligado
/// tem aparência própria (rótulo, cores, ícone) e, se quiser, outra ação no segundo toque.
pub fn states_pane(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    b: &mut ButtonConfig,
    rec: &mut Option<&'static str>,
) -> bool {
    let pal = cx.pal;
    let mut changed = false;
    widgets::section(ui, "Dois estados", pal);
    let mut has_toggle = b.toggled_appearance.is_some();
    if ui
        .checkbox(
            &mut has_toggle,
            t("Cada toque alterna entre normal e ligado"),
        )
        .changed()
    {
        b.toggled_appearance = has_toggle.then(|| lit_variant(&b.appearance));
        if !has_toggle {
            b.toggled_action = None;
        }
        b.toggle_state = false;
        changed = true;
    }
    let Some(lit) = b.toggled_appearance.as_mut() else {
        let hint = t("Ex.: microfone ligado/desligado, live no ar/fora do ar. Teclas do OBS já acendem sozinhas com o estado do OBS.");
        ui.label(widgets::hint(hint, pal));
        return changed;
    };
    changed |= ui
        .checkbox(&mut b.toggle_state, t("Mostrar o estado ligado na prévia"))
        .changed();
    let label = widgets::section(ui, "Rótulo ligado", pal);
    changed |= ui
        .add(
            egui::TextEdit::multiline(&mut lit.text)
                .desired_rows(1)
                .desired_width(ui.available_width()),
        )
        .labelled_by(label)
        .changed();
    ui.horizontal(|ui| {
        changed |= color_picker(ui, "Fundo", &mut lit.background);
        ui.add_space(SWATCH_GAP);
        changed |= color_picker(ui, "Texto", &mut lit.text_color);
    });
    changed |= super::icon_picker::icon_section(ui, lit, "lit", pal, cx.accent);
    widgets::section(ui, "Segundo toque", pal);
    let mut other = b.toggled_action.is_some();
    if ui
        .checkbox(&mut other, t("Fazer outra ação ao desligar"))
        .changed()
    {
        b.toggled_action = other.then_some(ButtonAction::None);
        changed = true;
    }
    match b.toggled_action.as_mut() {
        Some(a) => changed |= action_editor(ui, cx, "tact", a, rec),
        None => {
            ui.label(widgets::hint(
                t("Sem isso, os dois toques fazem a ação da aba Ação."),
                pal,
            ));
        }
    }
    changed
}
