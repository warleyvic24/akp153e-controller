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
    self, pos2, vec2, Color32, CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui,
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
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    // O selo "+ Tecla N" só aparece ao apontar; quem usa teclado ou leitor de tela ouve o destino
    // na descrição, lida depois do nome (que fica igual ao texto visível, WCAG 2.5.3).
    let target = tf("Aplica na tecla {}", &[&(key + 1)]);
    resp.ctx
        .accesskit_node_builder(resp.id, |node| node.set_description(target));
    let hot = resp.hovered() || resp.has_focus();
    let painter = ui.painter();
    if hot {
        painter.rect_filled(rect, CornerRadius::from(radius::CONTROL), pal.raise);
    }
    let badge = egui::Rect::from_center_size(
        pos2(rect.left() + space::XS + BADGE / 2.0, rect.center().y),
        vec2(BADGE, BADGE),
    );
    let badge_fill = if hot { pal.panel } else { pal.raise };
    painter.rect_filled(badge, CornerRadius::from(radius::CONTROL - 1.0), badge_fill);
    super::icon_picker::paint_any(
        painter,
        ui.ctx(),
        badge.center(),
        BADGE_ICON,
        p.icon,
        pal.fg,
    );
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
        painter.rect_filled(
            chip,
            CornerRadius::from(6.0),
            accent.gamma_multiply(APPLY_TINT),
        );
        painter.galley(chip.min + APPLY_PAD, galley, ink);
    }
    // Sem alça em repouso: repetida em 25 linhas ela só faz ruído. Ao passar o mouse o cursor
    // de arrastar e o selo "+ Tecla N" já dizem o que a linha aceita.
    resp.on_hover_cursor(egui::CursorIcon::Grab)
}

impl App {
    /// Coluna da esquerda: título, busca e as ações agrupadas por categoria.
    pub(crate) fn library(
        &mut self,
        root: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let ctx = &root.ctx().clone();
        let frame = Frame::NONE
            .fill(pal.panel)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .inner_margin(Margin::same(PANEL_MARGIN as i8));
        let mut picked = Picked::default();
        let shown = egui::Panel::left("library")
            .exact_size(super::layout::LIBRARY_WIDTH)
            .resizable(false)
            .frame(frame)
            .show(root, |ui| {
                let title = ui.label(RichText::new(t("Ações")).size(text::BODY_LG).strong());
                widgets::heading(ctx, title.id);
                ui.label(
                    widgets::hint("Clique para aplicar ou arraste até uma tecla", pal)
                        .size(text::CAPTION),
                );
                ui.add_space(space::SM);
                let search = widgets::search_field(ui, &mut self.query, "Buscar ação", pal);
                // Ctrl+F leva o foco à busca (ver `App::global_shortcuts`).
                if std::mem::take(&mut self.focus_search) {
                    search.request_focus();
                }
                ui.add_space(space::XS);
                widgets::scroll_area(ui, pal, |ui| {
                    picked = self.library_list(ui, ctx, pal, accent)
                });
            });
        widgets::landmark(
            ctx,
            shown.response.id,
            egui::accesskit::Role::Navigation,
            "Biblioteca de ações",
        );
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
        if q.is_empty() && !self.memory.recent.is_empty() {
            widgets::section(ui, "Recentes", pal);
            for name in &self.memory.recent {
                let Some(i) = self.presets.iter().position(|p| p.label == name) else {
                    continue;
                };
                // Mesma linha das seções: o id muda para não colidir com o item lá embaixo.
                let resp = ui
                    .push_id(("recent", i), |ui| {
                        library_row(ui, &self.presets[i], self.selected, pal, accent)
                    })
                    .inner;
                if resp.clicked() {
                    picked.preset = Some(i);
                }
                if resp.drag_started() {
                    egui::DragAndDrop::set_payload(ctx, Dragged::Preset(i));
                }
            }
        }
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
            let custom = widgets::secondary_button(ui, "+ Ação personalizada", pal);
            picked.custom = custom.clicked();
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
