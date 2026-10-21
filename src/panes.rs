//! Painéis do inspetor (Aparência, Ação, Alternar) e o editor de ações.
use crate::action_ui::{action_editor, PaneCtx};
use crate::config::schema::*;
use crate::picker::{app_buttons, AppPicker};
use crate::strings::t;
use crate::theme::Palette;
use crate::{c32, icons, library, widgets};
use eframe::egui::{self, vec2, Color32, Rounding, Sense, Stroke};
const IMAGE_FIELD_RESERVED: f32 = 172.0;
const SWATCH_SIZE: f32 = 26.0;
const ICON_GAP: f32 = 6.0;
const ICON_CELL: f32 = 40.0;
const ICON_GLYPH: f32 = 20.0;
const NO_ICON_FONT: f32 = 16.0;
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
        .add_filter(t("Imagem (PNG, JPG)"), &["png", "jpg", "jpeg"])
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
    ui.label(t(label));
    let mut rgb = [c.r, c.g, c.b];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        (c.r, c.g, c.b) = (rgb[0], rgb[1], rgb[2]);
        changed = true;
    }
    changed
}

fn label_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    let label = widgets::section(ui, "Rótulo", pal);
    changed |= ui
        .add(
            egui::TextEdit::singleline(&mut b.appearance.text)
                .hint_text(widgets::hint("Texto na tecla (ex.: Mudo)", pal))
                .desired_width(ui.available_width()),
        )
        .labelled_by(label)
        .changed();
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

/// Uma célula da grade de ícones. `id` vazio é "sem ícone".
fn icon_cell(
    ui: &mut egui::Ui,
    (id, name): (&str, &str),
    on: bool,
    pal: &Palette,
    ink: Color32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ICON_CELL, ICON_CELL), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, t(name)));
    let p = ui.painter();
    let round = Rounding::same(crate::tokens::radius::CONTROL);
    p.rect_filled(rect, round, pal.field);
    let width = if on { 2.0_f32 } else { 1.0_f32 };
    let border = if on { ink } else { pal.control };
    p.rect_stroke(rect, round, Stroke::new(width, border));
    if id.is_empty() {
        let font = egui::FontId::monospace(NO_ICON_FONT);
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "–",
            font,
            pal.muted,
        );
    } else {
        let color = if on { ink } else { pal.fg };
        widgets::paint_icon(p, rect.center(), ICON_GLYPH, id, color);
    }
    resp.on_hover_text(t(name))
}

fn icon_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette, accent: Color32) -> bool {
    let mut changed = false;
    widgets::section(ui, "Ícone", pal);
    let ink = crate::theme::ink(accent, pal);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(ICON_GAP, ICON_GAP);
        let none_on = b.appearance.icon.is_none();
        let options = std::iter::once(("", "Sem ícone")).chain(icons::ICONS);
        for (id, name) in options {
            let on = if id.is_empty() {
                none_on
            } else {
                b.appearance.icon.as_deref() == Some(id)
            };
            if icon_cell(ui, (id, name), on, pal, ink).clicked() {
                b.appearance.icon = (!id.is_empty()).then(|| id.to_string());
                if !id.is_empty() {
                    b.appearance.icon_path = None;
                }
                changed = true;
            }
        }
    });
    changed
}

fn image_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    let image_label = widgets::section(ui, "Imagem (PNG ou JPG)", pal);
    let mut path = b.appearance.icon_path.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        let mut edited = ui
            .add(
                egui::TextEdit::singleline(&mut path)
                    .desired_width((ui.available_width() - IMAGE_FIELD_RESERVED).max(60.0)),
            )
            .labelled_by(image_label)
            .changed();
        if ui.button(t("Escolher")).clicked() {
            if let Some(p) = pick_icon() {
                path = p;
                edited = true;
            }
        }
        let clear = ui.button(t("Limpar"));
        // O nome falado contém o texto visível (WCAG 2.5.3) e diz o que será limpo.
        clear.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("Limpar imagem"))
        });
        if clear.clicked() {
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
    changed
}

fn font_section(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    let label = widgets::section(ui, "Tamanho da fonte", pal);
    fill_slider_width(ui);
    changed |= ui
        .add(egui::Slider::new(
            &mut b.appearance.font_size,
            8.0_f32..=48.0,
        ))
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
    changed |= icon_section(ui, b, pal, accent);
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
    }
    changed
}

pub fn states_pane(ui: &mut egui::Ui, b: &mut ButtonConfig, pal: &Palette) -> bool {
    let mut changed = false;
    widgets::section(ui, "Alternar entre dois estados", pal);
    let mut has_toggle = b.toggled_appearance.is_some();
    if ui
        .checkbox(&mut has_toggle, t("Cada toque troca a aparência"))
        .changed()
    {
        b.toggled_appearance = has_toggle.then(|| lit_variant(&b.appearance));
        b.toggle_state = false;
        changed = true;
    }
    if let Some(lit) = b.toggled_appearance.as_mut() {
        ui.horizontal(|ui| {
            ui.label(t("Ligado:"));
            changed |= ui
                .add(egui::TextEdit::singleline(&mut lit.text).desired_width(120.0))
                .changed();
            changed |= color_picker(ui, "Fundo", &mut lit.background);
        });
        changed |= ui
            .checkbox(&mut b.toggle_state, t("Ver o estado ligado na prévia"))
            .changed();
    }
    let label = widgets::section(ui, "Segurar a partir de", pal);
    fill_slider_width(ui);
    changed |= ui
        .add(egui::Slider::new(&mut b.long_press_threshold_ms, 100..=2000).suffix(" ms"))
        .labelled_by(label)
        .changed();
    changed
}
