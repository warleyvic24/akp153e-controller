//! Escolha do ícone da tecla: busca na biblioteca Lucide (2000 ícones, em português ou inglês) e
//! grade com os resultados. Sem busca, mostra os ícones mais usados num controlador de live.
use super::{theme, tokens, widgets};
use crate::config::schema::{ButtonConfig, RGBAColor};
use crate::editor::theme::Palette;
use crate::graphics::{icon_library, icons};
use crate::strings::{t, tf};
use eframe::egui::{self, vec2, Color32, Rounding, Sense, Stroke};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const CELL: f32 = 40.0;
const GAP: f32 = 6.0;
const GLYPH: f32 = 20.0;
/// Textura com o dobro do tamanho exibido: nítida em telas com escala.
const TEXTURE_PX: u32 = 40;
const NO_ICON_FONT: f32 = 16.0;
/// Resultados mostrados de uma vez; a busca afunila o resto.
const MAX_RESULTS: usize = 60;

/// Texturas dos ícones da grade, por nome e cor (a cor muda com o tema).
type TextureCache = Arc<Mutex<HashMap<(String, [u8; 3]), egui::TextureHandle>>>;

pub(crate) fn texture(ctx: &egui::Context, id: &str, color: Color32) -> Option<egui::TextureId> {
    let cache: TextureCache = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<TextureCache>(egui::Id::new("icon-picker-textures"))
            .clone()
    });
    let rgb = [color.r(), color.g(), color.b()];
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(tex) = cache.get(&(id.to_string(), rgb)) {
        return Some(tex.id());
    }
    let ink = RGBAColor {
        r: rgb[0],
        g: rgb[1],
        b: rgb[2],
        a: 255,
    };
    let img = icon_library::render(id, TEXTURE_PX, ink)?;
    let image = egui::ColorImage::from_rgba_unmultiplied([TEXTURE_PX as usize; 2], img.as_raw());
    let handle = ctx.load_texture(format!("icon:{id}"), image, egui::TextureOptions::LINEAR);
    let tex = handle.id();
    cache.insert((id.to_string(), rgb), handle);
    Some(tex)
}

/// Desenha um ícone qualquer (embutido, a traço, ou da biblioteca, por textura) centrado em `center`.
pub(crate) fn paint_any(
    p: &egui::Painter,
    ctx: &egui::Context,
    center: egui::Pos2,
    size: f32,
    id: &str,
    color: Color32,
) {
    if id.starts_with(icon_library::PREFIX) {
        if let Some(tex) = texture(ctx, id, color) {
            let rect = egui::Rect::from_center_size(center, vec2(size, size));
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            p.image(tex, rect, uv, Color32::WHITE);
        }
    } else {
        widgets::paint_icon(p, center, size, id, color);
    }
}

/// Uma célula da grade. `id` vazio é "sem ícone"; ícones antigos (embutidos) são desenhados a traço.
fn cell(
    ui: &mut egui::Ui,
    id: &str,
    name: &str,
    on: bool,
    pal: &Palette,
    ink: Color32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, name));
    let p = ui.painter();
    let round = Rounding::same(tokens::radius::CONTROL);
    let fill = if resp.hovered() {
        theme::mix(pal.field, pal.fg, 0.08)
    } else {
        pal.field
    };
    p.rect_filled(rect, round, fill);
    let border = if on {
        Stroke::new(2.0_f32, ink)
    } else {
        Stroke::new(1.0_f32, pal.control)
    };
    p.rect_stroke(rect, round, border);
    let color = if on { ink } else { pal.fg };
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
        paint_any(p, ui.ctx(), rect.center(), GLYPH, id, color);
    }
    resp.on_hover_text(name)
}

/// Nome legível de um id: o nome do ícone embutido ou o nome Lucide.
fn display_name(id: &str) -> String {
    if let Some(name) = id.strip_prefix(icon_library::PREFIX) {
        return name.replace('-', " ");
    }
    icons::ICONS
        .iter()
        .find(|(i, _)| *i == id)
        .map_or_else(|| id.to_string(), |(_, n)| t(n).to_string())
}

/// Seção "Ícone" da aba Aparência; devolve `true` se o ícone mudou.
pub fn icon_section(
    ui: &mut egui::Ui,
    b: &mut ButtonConfig,
    pal: &Palette,
    accent: Color32,
) -> bool {
    let mut changed = false;
    let label = widgets::section(ui, "Ícone", pal);
    let ink = theme::ink(accent, pal);
    let query_id = egui::Id::new("icon-picker-query");
    let mut query: String = ui.data_mut(|d| d.get_temp(query_id).unwrap_or_default());
    let field = ui
        .add(
            egui::TextEdit::singleline(&mut query)
                .hint_text(widgets::hint("Buscar ícone (ex.: microfone, câmera)", pal))
                .desired_width(ui.available_width()),
        )
        .labelled_by(label);
    if field.changed() {
        ui.data_mut(|d| d.insert_temp(query_id, query.clone()));
    }
    let results = icon_library::search(&query, MAX_RESULTS);
    let current = b.appearance.icon.clone();
    ui.add_space(tokens::space::XS);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        let mut options: Vec<String> = vec![String::new()];
        // O ícone atual fica sempre à vista, mesmo fora dos resultados (ou se for um antigo).
        if let Some(cur) = current.as_ref() {
            if !results
                .iter()
                .any(|i| cur.strip_prefix(icon_library::PREFIX) == Some(i.name))
            {
                options.push(cur.clone());
            }
        }
        options.extend(
            results
                .iter()
                .map(|i| format!("{}{}", icon_library::PREFIX, i.name)),
        );
        for id in options {
            let on = if id.is_empty() {
                current.is_none()
            } else {
                current.as_deref() == Some(id.as_str())
            };
            let name = if id.is_empty() {
                t("Sem ícone").to_string()
            } else {
                display_name(&id)
            };
            if cell(ui, &id, &name, on, pal, ink).clicked() {
                b.appearance.icon = (!id.is_empty()).then(|| id.clone());
                if !id.is_empty() {
                    b.appearance.icon_path = None;
                }
                changed = true;
            }
        }
    });
    if !query.trim().is_empty() && results.is_empty() {
        let msg = tf("Nenhum ícone para “{}”. Tente em inglês.", &[&query.trim()]);
        ui.label(
            egui::RichText::new(msg)
                .size(tokens::text::CAPTION)
                .color(pal.muted),
        );
    } else if query.trim().is_empty() {
        let msg = tf(
            "{} ícones: busque por nome ou assunto.",
            &[&icon_library::all().len()],
        );
        ui.label(
            egui::RichText::new(msg)
                .size(tokens::text::CAPTION)
                .color(pal.muted),
        );
    }
    changed
}
