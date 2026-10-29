//! Biblioteca de ações (coluna da esquerda): busca e lista agrupada. Cada item aplica a ação à
//! tecla selecionada com um clique ou vai até uma tecla arrastado (o clique é a alternativa ao
//! arrasto que a WCAG 2.2, critério 2.5.7, pede).
use super::stage::Dragged;
use super::{widgets, App};
use crate::config::schema::ButtonAction;
use crate::editor::library::Preset;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::{radius, space, text};
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{
    self, pos2, vec2, Color32, Frame, Margin, RichText, Rounding, Sense, Stroke, Ui,
};

const PANEL_MARGIN: f32 = 12.0;
const ROW_HEIGHT: f32 = 36.0;
const ROW_GAP: f32 = 2.0;
const BADGE: f32 = 28.0;
const BADGE_ICON: f32 = 16.0;
const APPLY_TINT: f32 = 0.14;
const APPLY_PAD: egui::Vec2 = egui::Vec2::new(8.0, 4.0);

/// O que um quadro da biblioteca pediu: aplicar um preset ou criar uma ação personalizada.
#[derive(Default)]
struct Picked {
    preset: Option<usize>,
    custom: bool,
}

/// Minúsculas e sem acento, para "musica" achar "Música".
fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// A busca (já em `fold`) casa com o nome ou a categoria, em português ou no idioma da interface.
fn search_matches(folded_query: &str, texts: &[&str]) -> bool {
    texts.iter().any(|s| fold(s).contains(folded_query))
}

/// Linha da biblioteca: selo com ícone, nome e, ao passar o mouse, o atalho "+ Tecla N".
fn library_row(
    ui: &mut Ui,
    p: &Preset,
    key: usize,
    pal: &Palette,
    accent: Color32,
) -> egui::Response {
    let label = t(p.label);
    let size = vec2(ui.available_width(), ROW_HEIGHT);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let hot = resp.hovered() || resp.has_focus();
    let painter = ui.painter();
    if hot {
        painter.rect_filled(rect, Rounding::same(radius::CONTROL), pal.raise);
    }
    let badge = egui::Rect::from_center_size(
        pos2(rect.left() + space::XS + BADGE / 2.0, rect.center().y),
        vec2(BADGE, BADGE),
    );
    let badge_fill = if hot { pal.panel } else { pal.raise };
    painter.rect_filled(badge, Rounding::same(radius::CONTROL - 1.0), badge_fill);
    widgets::paint_icon(painter, badge.center(), BADGE_ICON, p.icon, pal.fg);
    painter.text(
        pos2(badge.right() + space::MD, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(text::BODY - 1.0),
        pal.fg,
    );
    let ink = theme::ink(accent, pal);
    if hot {
        let apply = tf("+ Tecla {}", &[&(key + 1)]);
        let galley = painter.layout_no_wrap(apply, egui::FontId::proportional(text::CAPTION), ink);
        let chip = egui::Rect::from_min_size(
            pos2(
                rect.right() - space::XS - galley.size().x - 2.0 * APPLY_PAD.x,
                rect.center().y - galley.size().y / 2.0 - APPLY_PAD.y,
            ),
            galley.size() + 2.0 * APPLY_PAD,
        );
        painter.rect_filled(chip, Rounding::same(6.0), accent.gamma_multiply(APPLY_TINT));
        painter.galley(chip.min + APPLY_PAD, galley, ink);
    } else {
        let at = pos2(rect.right() - space::SM - BADGE_ICON / 2.0, rect.center().y);
        widgets::paint_icon(painter, at, BADGE_ICON, "grip", pal.muted);
    }
    resp.on_hover_cursor(egui::CursorIcon::Grab)
}

impl App {
    /// Coluna da esquerda: título, busca e as ações agrupadas por categoria.
    pub(crate) fn library(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let frame = Frame::none()
            .fill(pal.panel)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .inner_margin(Margin::same(PANEL_MARGIN));
        let mut picked = Picked::default();
        egui::SidePanel::left("library")
            .exact_width(super::layout::LIBRARY_WIDTH)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                ui.label(RichText::new(t("Ações")).size(text::BODY + 1.0).strong());
                ui.label(
                    widgets::hint("Clique para aplicar ou arraste até uma tecla", pal)
                        .size(text::CAPTION),
                );
                ui.add_space(space::SM);
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(widgets::hint("Buscar ação", pal))
                        .desired_width(ui.available_width()),
                );
                widgets::name(&search, "Buscar ação");
                ui.add_space(space::XS);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| picked = self.library_list(ui, ctx, pal, accent));
            });
        if let Some(i) = picked.preset {
            self.apply_preset(g, self.selected, i);
        }
        if picked.custom {
            self.apply_custom_action(g);
        }
        // No modo compacto a biblioteca ocupa o lugar do inspetor: escolhida a ação, ele volta
        // para mostrar o resultado.
        if picked.preset.is_some() || picked.custom {
            self.library_open = false;
        }
    }

    /// Grupos e linhas visíveis (todos, ou só os que casam com a busca).
    fn library_list(
        &self,
        ui: &mut Ui,
        ctx: &egui::Context,
        pal: &Palette,
        accent: Color32,
    ) -> Picked {
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        let mut picked = Picked::default();
        let q = fold(self.query.trim());
        let mut shown = 0;
        let mut category = "";
        for (i, p) in self.presets.iter().enumerate() {
            let texts = [p.label, t(p.label), p.category, t(p.category)];
            if !q.is_empty() && !search_matches(&q, &texts) {
                continue;
            }
            if p.category != category {
                category = p.category;
                widgets::section(ui, category, pal);
            }
            shown += 1;
            let resp = library_row(ui, p, self.selected, pal, accent);
            if resp.clicked() {
                picked.preset = Some(i);
            }
            if resp.drag_started() {
                egui::DragAndDrop::set_payload(ctx, Dragged::Preset(i));
            }
        }
        if shown == 0 {
            let msg = tf("Nada encontrado para “{}”.", &[&self.query.trim()]);
            ui.label(RichText::new(msg).color(pal.muted));
        }
        if q.is_empty() {
            widgets::section(ui, "Outras", pal);
            let custom = egui::Button::new(t("+ Ação personalizada")).frame(false);
            picked.custom = ui.add(custom).clicked();
        }
        picked
    }

    /// "Ação personalizada": a tecla selecionada vira um atalho vazio e o inspetor abre na aba Ação.
    fn apply_custom_action(&mut self, g: &mut runtime::Shared) {
        let Some(b) = g.cfg.page_mut().buttons.get_mut(self.selected) else {
            return;
        };
        b.action = ButtonAction::Hotkey { keys: vec![] };
        self.tab = super::inspector::TAB_ACTION;
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ignores_case_and_accents_and_checks_every_text() {
        assert!(search_matches(&fold("MUSICA"), &["Música"]));
        assert!(search_matches(&fold("ção"), &["Ação"]));
        assert!(search_matches(&fold("mute"), &["Mudo", "Mute"]));
        assert!(!search_matches(&fold("zzz"), &["Mudo", "Mute"]));
    }
}
