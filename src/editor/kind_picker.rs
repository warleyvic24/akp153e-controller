//! Escolha do tipo de ação no inspetor. Tecla vazia: a grade com todos os tipos, aberta (é a
//! decisão a tomar). Tecla já configurada: uma linha só ("App ▾") que abre a mesma grade num
//! balão; assim os campos da ação, que são o que se edita depois, ficam no topo do painel.
use super::action_kind::ActionKind;
use super::action_ui::PaneCtx;
use super::{icon_picker, theme, widgets};
use crate::config::schema::ButtonAction;
use crate::editor::tokens::{radius, space, text};
use crate::strings::{t, tf};
use eframe::egui;

const KIND_COLUMNS: usize = 4;
const KIND_GAP: f32 = 6.0;
const KIND_TILE_HEIGHT: f32 = 54.0;
const KIND_ICON: f32 = 18.0;
const KIND_ICON_TOP: f32 = 19.0;
const KIND_LABEL_BOTTOM: f32 = 13.0;
const KIND_ON_TINT: f32 = 0.12;
const KIND_HOVER_TINT: f32 = 0.08;
/// Altura da linha do tipo escolhido (acima do alvo mínimo de 24 px).
const KIND_ROW_HEIGHT: f32 = 36.0;
const KIND_ROW_ICON: f32 = 16.0;

/// Tipo da ação: a grade inteira numa tecla sem ação, a linha compacta nas outras.
pub(super) fn kind_picker(
    ui: &mut egui::Ui,
    cx: &PaneCtx,
    id: &'static str,
    a: &mut ButtonAction,
) -> bool {
    if *a == ButtonAction::None {
        // "Nenhuma" já é o estado atual: na grade aberta ela só ocuparia lugar.
        return kind_grid(ui, cx, a, false);
    }
    let kind = ActionKind::of(a);
    let label = kind.label();
    let width = ui.available_width();
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, KIND_ROW_HEIGHT), egui::Sense::click());
    let popup = ui.id().with(("kind-popup", id));
    let open = ui.memory(|m| m.is_popup_open(popup));
    let spoken = tf("Tipo de ação: {}. Trocar", &[&t(label)]);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &spoken));
    let pal = cx.pal;
    let hot = resp.hovered() || resp.has_focus() || open;
    let fill = if hot {
        theme::mix(pal.raise, pal.fg, KIND_HOVER_TINT)
    } else {
        pal.raise
    };
    let p = ui.painter();
    let round = egui::Rounding::same(radius::CONTROL);
    p.rect_filled(rect, round, fill);
    if hot {
        p.rect_stroke(rect, round, egui::Stroke::new(1.0_f32, pal.line));
    }
    let ink = theme::ink(cx.accent, pal);
    let icon_at = egui::pos2(
        rect.left() + space::MD + KIND_ROW_ICON / 2.0,
        rect.center().y,
    );
    icon_picker::paint_any(p, cx.ctx, icon_at, KIND_ROW_ICON, kind.icon(), ink);
    p.text(
        egui::pos2(
            rect.left() + 2.0 * space::MD + KIND_ROW_ICON,
            rect.center().y,
        ),
        egui::Align2::LEFT_CENTER,
        t(label),
        egui::FontId::proportional(text::BODY),
        pal.fg,
    );
    let chevron = if open {
        "lucide:chevron-up"
    } else {
        "lucide:chevron-down"
    };
    let chevron_at = egui::pos2(
        rect.right() - space::MD - KIND_ROW_ICON / 2.0,
        rect.center().y,
    );
    icon_picker::paint_any(p, cx.ctx, chevron_at, KIND_ROW_ICON, chevron, pal.muted);
    p.text(
        egui::pos2(
            chevron_at.x - KIND_ROW_ICON / 2.0 - space::XS,
            rect.center().y,
        ),
        egui::Align2::RIGHT_CENTER,
        t("Trocar"),
        egui::FontId::proportional(text::CAPTION),
        pal.muted,
    );
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    let close = egui::PopupCloseBehavior::CloseOnClickOutside;
    let changed = egui::popup_below_widget(ui, popup, &resp, close, |ui| {
        ui.set_width(width - 2.0 * space::SM);
        ui.label(widgets::hint(
            t("Trocar o tipo apaga os campos desta ação."),
            pal,
        ));
        ui.add_space(space::XS);
        kind_grid(ui, cx, a, true)
    })
    .unwrap_or(false);
    if changed {
        ui.memory_mut(|m| m.close_popup());
    }
    ui.add_space(space::SM);
    changed
}

/// Grade com todos os tipos; trocar o tipo recomeça com os valores padrão do novo tipo.
/// `with_none`: inclui "Nenhuma" (no balão, é o jeito de limpar a ação).
fn kind_grid(ui: &mut egui::Ui, cx: &PaneCtx, a: &mut ButtonAction, with_none: bool) -> bool {
    let mut changed = false;
    let gap = KIND_GAP;
    let width = (ui.available_width() - gap * (KIND_COLUMNS - 1) as f32) / KIND_COLUMNS as f32;
    let kinds: Vec<ActionKind> = ActionKind::PICKER
        .into_iter()
        .filter(|k| with_none || *k != ActionKind::None)
        .collect();
    for row in kinds.chunks(KIND_COLUMNS) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for &kind in row {
                let on = ActionKind::of(a) == kind;
                if kind_tile(ui, kind, on, width, cx).clicked() && !on {
                    *a = kind.default_action();
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
    kind: ActionKind,
    on: bool,
    width: f32,
    cx: &PaneCtx,
) -> egui::Response {
    let label = t(kind.label());
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
    icon_picker::paint_any(p, cx.ctx, icon_at, KIND_ICON, kind.icon(), fg);
    p.text(
        egui::pos2(rect.center().x, rect.bottom() - KIND_LABEL_BOTTOM),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(text::CAPTION),
        fg,
    );
    resp
}
