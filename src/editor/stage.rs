//! Palco central: as abas de página, o aparelho desenhado com as teclas e os atalhos.
use super::device_view;
use super::{checks, is_blank, key_view, widgets, App};
use crate::config::schema::{ButtonAction, ButtonConfig};
use crate::editor::action_ui::Targets;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::{radius, space, text};
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, Frame, Margin, Rect, RichText, Sense, Stroke, Ui,
};

const STAGE_MARGIN: f32 = 16.0;
const STAGE_FOOTER_HEIGHT: f32 = 30.0;
/// Moldura da tecla de atalho, espaços entre as peças e folga do texto monoespaçado.
const HINT_CHROME: f32 = 48.0;
const PAGE_TABS_HEIGHT: f32 = 40.0;
// Título e três passos de uma linha, com margens; medido na tela (o antigo 120 cortava o fim).
const GUIDE_HEIGHT: f32 = 210.0;
const OFFLINE_BANNER_HEIGHT: f32 = 60.0;
const BADGE_STROKE: f32 = 1.0;
const BEZEL_INSET: f32 = 8.0;
const SCREEN_RADIUS: f32 = 16.0;
const LED_SIZE: egui::Vec2 = egui::Vec2::new(34.0, 3.0);
const LED_TOP: f32 = 9.0;
const LED_HALO: f32 = 3.0;
const LED_HALO_RADIUS: f32 = 4.0;
const LED_RADIUS: f32 = 2.0;
const GUIDE_ICON: f32 = 18.0;
const DEVICE_SHADOW_ALPHA: u8 = 90;
const BEZEL_SHEEN_ALPHA: u8 = 6;
const BEZEL_RIM_ALPHA: u8 = 22;

/// O que está sendo arrastado: uma ação da biblioteca ou o conteúdo de uma tecla.
#[derive(Clone, Copy)]
pub(crate) enum Dragged {
    Preset(usize),
    Key(usize),
}

/// Resultado de um quadro do palco: soltou uma ação numa tecla, ou trocou duas teclas.
#[derive(Default)]
struct DeviceEvents {
    dropped: Option<(usize, usize)>,
    swapped: Option<(usize, usize)>,
    /// Tecla sob o ponteiro (para onde vai um arquivo de imagem solto na janela).
    under_pointer: Option<usize>,
    /// Duplo clique numa tecla que leva a uma página: abrir essa página (é como se entra numa pasta).
    open_page: Option<u64>,
}

/// Selo na ponta direita da fileira de páginas quando o modelo ainda não foi provado por ninguém.
fn experimental_badge(ui: &Ui, row: Rect, pal: &Palette) {
    let text = t("Experimental");
    let font = egui::FontId::proportional(text::CAPTION);
    let galley = ui.painter().layout_no_wrap(text.into(), font, pal.fg);
    let pad = vec2(space::SM, space::XS);
    let size = galley.size() + 2.0 * pad;
    let badge = Rect::from_min_size(
        pos2(row.right() - size.x, row.center().y - size.y / 2.0),
        size,
    );
    // Texto e contorno na cor do texto: o contraste com o fundo já é o testado em `theme.rs`.
    ui.painter().rect_stroke(
        badge,
        CornerRadius::from(radius::PILL),
        Stroke::new(BADGE_STROKE, pal.fg),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(badge.min + pad, galley, pal.fg);
    ui.interact(badge, ui.scope_id().with("experimental"), Sense::hover())
        .on_hover_text(t("Este modelo ainda não foi provado num aparelho de verdade: teclas trocadas ou tela apagada podem acontecer."));
}

/// Nome que o leitor de tela anuncia: posição, o que a tecla mostra, se precisa de ajuste e se
/// está selecionada.
fn key_widget_info(
    index: usize,
    b: &ButtonConfig,
    selected: bool,
    problem: Option<&str>,
) -> egui::WidgetInfo {
    let shown = &b.current_appearance().text;
    let what = if is_blank(b) {
        t("vazia")
    } else if shown.is_empty() {
        t("sem texto")
    } else {
        shown
    };
    let mut name = tf("Tecla {}, {}", &[&(index + 1), &what]);
    if let Some(problem) = problem {
        name = format!("{name}. {}", t(problem));
    }
    egui::WidgetInfo::selected(egui::Role::Button, true, selected, name)
}

/// Dica ao apontar uma tecla configurada: o nome e o que ela faz (teclas só com ícone não dizem).
fn key_tooltip(b: &ButtonConfig, targets: &Targets) -> Option<String> {
    if is_blank(b) {
        return None;
    }
    let name = super::inspector::key_title(b);
    let what = super::summary::action_summary(&b.action, targets);
    match (name, what) {
        (Some(name), Some(what)) if name != what => Some(format!("{name}\n{what}")),
        (Some(one), _) | (None, Some(one)) => Some(one),
        (None, None) => None,
    }
}

/// Corpo do aparelho: moldura, tela, aro interno e o LED de conexão.
fn paint_device_body(p: &egui::Painter, rect: Rect, pal: &Palette, accent: Color32, on: bool) {
    let screen = rect.shrink(BEZEL_INSET);
    let outer = CornerRadius::from(radius::DEVICE);
    let inner = CornerRadius::from(SCREEN_RADIUS);
    // Sombra de contato (some no preto, dá chão ao aparelho no tema claro).
    p.add(
        egui::epaint::Shadow {
            offset: [0, 18],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(DEVICE_SHADOW_ALPHA),
        }
        .as_shape(rect, outer),
    );
    p.rect_filled(rect, outer, pal.bezel);
    // Metade de cima um tom mais clara: luz vinda de cima, como alumínio escovado.
    let top = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
    let r = radius::DEVICE as u8;
    p.rect_filled(
        top,
        CornerRadius {
            nw: r,
            ne: r,
            sw: 0,
            se: 0,
        },
        Color32::from_white_alpha(BEZEL_SHEEN_ALPHA),
    );
    p.rect_filled(screen, inner, pal.device);
    p.rect_stroke(
        screen,
        inner,
        Stroke::new(1.0_f32, Color32::from_black_alpha(160)),
        egui::StrokeKind::Outside,
    );
    // Filete claro na borda: separa o aparelho do fundo preto sem brilho colorido (o brilho
    // do destaque é reservado à tecla selecionada).
    p.rect_stroke(
        rect,
        outer,
        Stroke::new(1.0_f32, Color32::from_white_alpha(BEZEL_RIM_ALPHA)),
        egui::StrokeKind::Inside,
    );
    let led = if on { accent } else { pal.bad };
    let led_rect = Rect::from_center_size(pos2(rect.center().x, rect.top() + LED_TOP), LED_SIZE);
    p.rect_filled(
        led_rect.expand(LED_HALO),
        CornerRadius::from(LED_HALO_RADIUS),
        led.gamma_multiply(0.25),
    );
    p.rect_filled(led_rect, CornerRadius::from(LED_RADIUS), led);
}

const BEAUTIFY_HINT: &str =
    "Cor da categoria, fundo em degradê, rótulo e ícone nas teclas que não têm. Ctrl+Z desfaz.";

/// Deslize da grade ao trocar de página (vem do lado da página nova) e a escala da tecla apertada.
const SLIDE_TIME: f32 = 0.16;
const SLIDE_SHARE: f32 = 0.6;
const PRESS_TIME: f32 = 0.08;
const PRESSED_SCALE: f32 = 0.86;

/// Animação da troca de página no palco.
#[derive(Default)]
pub(super) struct PageSlide {
    page: Option<(usize, usize)>,
    start: Option<std::time::Instant>,
    dir: f32,
}

impl PageSlide {
    /// Deslocamento x (em teclas) e opacidade da grade neste quadro para a página `page`
    /// (perfil, página).
    fn frame(&mut self, page: (usize, usize)) -> (f32, f32) {
        if self.page != Some(page) {
            if let Some(old) = self.page {
                self.start = Some(std::time::Instant::now());
                // Mesma página em outro perfil: entra pela direita.
                self.dir = if page.0 == old.0 && page.1 < old.1 {
                    -1.0
                } else {
                    1.0
                };
            }
            self.page = Some(page);
        }
        let Some(start) = self.start else {
            return (0.0, 1.0);
        };
        let t = (start.elapsed().as_secs_f32() / SLIDE_TIME).min(1.0);
        if t >= 1.0 {
            self.start = None;
        }
        // Desacelera no fim (ease-out cúbico).
        let eased = 1.0 - (1.0 - t).powi(3);
        ((1.0 - eased) * self.dir * SLIDE_SHARE, eased)
    }
}

/// Atalhos do palco (tecla e o que faz).
const SHORTCUTS: [(&str, &str); 10] = [
    ("Del", "limpar"),
    ("Ctrl C", "copiar"),
    ("Ctrl V", "colar"),
    ("Arraste", "trocar teclas"),
    ("Ctrl Z", "desfazer"),
    ("Setas", "andar entre as teclas"),
    ("Alt Setas", "mover a tecla"),
    ("Ctrl F", "buscar ação"),
    ("F6", "voltar às teclas"),
    ("Ctrl ,", "preferências"),
];

/// Tecla vizinha de `from` na direção `dir` (um dos quatro eixos), pelos centros das teclas:
/// a mais próxima à frente, preferindo a da mesma fileira ou coluna.
pub(crate) fn neighbor(keys: &[Rect], from: usize, dir: egui::Vec2) -> Option<usize> {
    let origin = keys.get(from)?.center();
    let min_step = keys[from].width().min(keys[from].height()) / 2.0;
    keys.iter()
        .enumerate()
        .filter(|(i, _)| *i != from)
        .filter_map(|(i, r)| {
            let d = r.center() - origin;
            let along = d.x * dir.x + d.y * dir.y;
            let across = (d.x * dir.y - d.y * dir.x).abs();
            (along >= min_step).then_some((i, along + 2.0 * across))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Atalhos do palco em forma de tecla, para quem não sabe que eles existem.
fn shortcut_hints(ui: &mut Ui, pal: &Palette) {
    for (key, what) in SHORTCUTS {
        // Palco estreito: as dicas que não cabem ficam de fora em vez de sair cortadas.
        let font = egui::FontId::proportional(text::CAPTION);
        let need = ui.ctx().fonts_mut(|f| {
            let mut w = |s: &str| {
                f.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE)
                    .size()
                    .x
            };
            w(t(key)) + w(t(what))
        }) + HINT_CHROME;
        if ui.available_width() < need {
            break;
        }
        widgets::kbd(ui, t(key), pal);
        ui.label(RichText::new(t(what)).size(text::CAPTION).color(pal.muted));
        ui.add_space(space::SM);
    }
}

/// Os atalhos do palco num balão, depois que a barra de dicas some.
fn shortcuts_button(ui: &mut Ui, pal: &Palette) {
    let resp = widgets::icon_button(
        ui,
        "lucide:circle-question-mark",
        "Atalhos do teclado",
        true,
        pal,
    );
    egui::Popup::from_toggle_button_response(&resp).show(|ui| {
        egui::Grid::new("shortcuts")
            .spacing(vec2(space::MD, space::SM))
            .show(ui, |ui| {
                for (key, what) in SHORTCUTS {
                    widgets::kbd(ui, t(key), pal);
                    ui.label(t(what));
                    ui.end_row();
                }
            });
    });
}

/// Passo a passo para quem abre uma página sem nenhuma tecla configurada.
/// Devolve o ponto de partida escolhido (índice em `templates::TEMPLATES`), se algum.
fn empty_guide(ui: &mut Ui, pal: &Palette, accent: Color32, width: f32) -> Option<usize> {
    let ink = theme::ink(accent, pal);
    let mut chosen = None;
    // Da largura do aparelho e alinhado à esquerda, como as páginas, não ao centro do palco.
    ui.allocate_ui(vec2(width, 0.0), |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            widgets::card(ui, pal, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::icon(ui, "lucide:sparkles", GUIDE_ICON, ink);
                    ui.label(
                        RichText::new(t("Comece por aqui"))
                            .size(text::BODY_LG)
                            .strong(),
                    );
                });
                let steps = [
                    "Clique numa tecla do aparelho",
                    "Escolha uma ação em Ações, ou arraste-a até a tecla",
                    "Ajuste texto, cor e ícone no painel à direita",
                ];
                for (n, step) in steps.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}", n + 1)).monospace().color(ink));
                        ui.label(t(step));
                    });
                }
                ui.add_space(space::SM);
                ui.horizontal_wrapped(|ui| {
                    ui.label(t("Ou comece com uma página pronta:"));
                    for (i, template) in super::templates::TEMPLATES.iter().enumerate() {
                        if widgets::pill(ui, template.name, false, accent, pal).clicked() {
                            chosen = Some(i);
                        }
                    }
                });
            });
        });
    });
    chosen
}

impl App {
    pub(crate) fn stage(
        &mut self,
        root: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let ctx = &root.ctx().clone();
        let frame = Frame::NONE
            .fill(pal.bg)
            .inner_margin(Margin::same(STAGE_MARGIN as i8));
        let mut events = DeviceEvents::default();
        let shown = egui::CentralPanel::default().frame(frame).show(root, |ui| {
            widgets::scroll_area(ui, pal, |ui| {
                ui.vertical_centered(|ui| {
                    events = self.device_column(ui, ctx, g, pal, accent);
                })
            });
        });
        widgets::landmark(
            ctx,
            shown.response.id,
            egui::accesskit::Role::Main,
            "Aparelho",
        );
        self.apply_dropped_files(ctx, g, events.under_pointer);
        self.apply_device_events(g, events);
    }

    fn device_column(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) -> DeviceEvents {
        let page_blank = g.cfg.page().buttons.iter().all(is_blank);
        let banner = if g.connected {
            0.0
        } else {
            OFFLINE_BANNER_HEIGHT
        };
        let below = space::MD
            + if page_blank {
                GUIDE_HEIGHT
            } else {
                STAGE_FOOTER_HEIGHT
            };
        let above = banner + PAGE_TABS_HEIGHT + space::SM;
        let Some(model) = device_view::model_for(&g.cfg.page().model) else {
            return DeviceEvents::default();
        };
        let key = device_view::fit_key_size(
            model,
            ui.available_width(),
            ui.available_height() - above - below,
        );
        let layout = device_view::layout(model, key);
        let device = layout.size;
        // Centraliza na vertical o bloco inteiro (aviso, páginas, aparelho e o que vem abaixo).
        ui.add_space(((ui.available_height() - above - device.y - below) / 2.0).max(0.0));
        if !g.connected {
            self.offline_banner(ui, pal, g.service_running, device.x);
        }
        // As páginas pertencem ao aparelho: ficam coladas a ele, alinhadas à sua borda esquerda.
        let tabs = ui
            .allocate_ui(vec2(device.x, PAGE_TABS_HEIGHT), |ui| {
                ui.horizontal(|ui| self.page_tabs(ui, g, pal));
            })
            .response
            .rect;
        if model.status == crate::device::Status::Experimental {
            // A fileira encolhe ao conteúdo: o selo vai na borda direita do aparelho.
            experimental_badge(
                ui,
                Rect::from_min_size(tabs.min, vec2(device.x, tabs.height())),
                pal,
            );
        }
        ui.add_space(space::SM);
        let (rect, _) = ui.allocate_exact_size(device, Sense::hover());
        paint_device_body(ui.painter(), rect, pal, accent, g.connected);
        let at = rect.min.to_vec2();
        let keys: Vec<Rect> = layout.keys.iter().map(|k| k.translate(at)).collect();
        let events = self.draw_keys(ui, ctx, g, &keys, key, pal, accent);
        if let Some(side) = layout.strip {
            self.side_strip(ui, g, side.translate(at), pal, accent);
        }
        ui.add_space(space::MD);
        if page_blank {
            if let Some(i) = empty_guide(ui, pal, accent, rect.width()) {
                let template = &super::templates::TEMPLATES[i];
                let snapshot = g.cfg.clone();
                super::templates::fill(template, g.cfg.page_mut(), &self.presets, &snapshot);
                self.touch();
                self.toasts.push(
                    tf("Página {} pronta · Ctrl+Z desfaz", &[&t(template.name)]),
                    None,
                );
            }
        } else {
            let beautify = ui
                .allocate_ui(vec2(rect.width(), STAGE_FOOTER_HEIGHT), |ui| {
                    ui.horizontal(|ui| {
                        // O botão fica à direita; as dicas usam o que sobra à esquerda.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let clicked = widgets::button(
                                ui,
                                Some("wand-sparkles"),
                                "Organizar cores",
                                widgets::ButtonKind::Ghost,
                                true,
                                0.0,
                                pal,
                            )
                            .on_hover_text(t(BEAUTIFY_HINT))
                            .clicked();
                            let hints = self.memory.shows_hints();
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    // Nas primeiras aberturas, as dicas à vista; depois, um "?".
                                    if hints {
                                        shortcut_hints(ui, pal);
                                    } else {
                                        shortcuts_button(ui, pal);
                                    }
                                },
                            );
                            clicked
                        })
                        .inner
                    })
                    .inner
                })
                .inner;
            if beautify {
                self.beautify_page(g);
            }
        }
        events
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_keys(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        g: &runtime::Shared,
        keys: &[Rect],
        key: f32,
        pal: &Palette,
        accent: Color32,
    ) -> DeviceEvents {
        let painter = ui.painter().clone();
        let targets = Targets::from(&g.cfg);
        let mut events = DeviceEvents::default();
        let page_at = (
            g.cfg.active_profile,
            g.cfg.profiles[g.cfg.active_profile].active_page,
        );
        let reduce_motion = g.cfg.ui.reduce_motion;
        let (slide, opacity) = if reduce_motion {
            (0.0, 1.0)
        } else {
            self.page_slide.frame(page_at)
        };
        self.key_rects = keys.to_vec();
        let focus_key = std::mem::take(&mut self.focus_key);
        if opacity < 1.0 {
            ctx.request_repaint();
        }
        let mut painter = painter;
        painter.set_opacity(opacity);
        // `buttons[i]` é a tecla `i` do modelo; sobra de um lado ou do outro não é desenhada.
        for (i, &kr) in keys.iter().enumerate().take(g.cfg.page().buttons.len()) {
            let resp = ui.interact(kr, ui.scope_id().with(("key", i)), Sense::click_and_drag());
            let button = &g.cfg.page().buttons[i];
            let problem = checks::button_problem(button, &targets);
            resp.widget_info(|| key_widget_info(i, button, i == self.selected, problem));
            if focus_key && i == self.selected {
                resp.request_focus();
            }
            // Tab e setas (navegação nativa do egui) movem o foco; a seleção o acompanha.
            if resp.has_focus() {
                self.selected = i;
            }
            if resp.drag_started() && !is_blank(button) {
                egui::DragAndDrop::set_payload(ctx, Dragged::Key(i));
            }
            let file_over = ctx.input(|inp| !inp.raw.hovered_files.is_empty())
                && ctx
                    .input(|inp| inp.pointer.hover_pos())
                    .is_some_and(|p| kr.contains(p));
            if ctx
                .input(|inp| inp.pointer.hover_pos())
                .is_some_and(|p| kr.contains(p))
            {
                events.under_pointer = Some(i);
            }
            let drop_target = resp.dnd_hover_payload::<Dragged>().is_some() || file_over;
            // A prévia é a do aparelho: teclas do OBS acesas/vermelhas conforme o OBS.
            let live = crate::obs::shown(button, &self.obs_live.state);
            let tex = self.key_tex.get(ctx, i, &live, g.icon_px);
            let view = key_view(button, Some(i + 1), tex, problem.is_some());
            let hot = resp.hovered() || drop_target;
            let down = if reduce_motion {
                0.0
            } else {
                ctx.animate_bool_with_time(
                    resp.id.with("down"),
                    resp.is_pointer_button_down_on(),
                    PRESS_TIME,
                )
            };
            let shown = kr
                .translate(vec2(slide * key, 0.0))
                .shrink(key * (1.0 - PRESSED_SCALE) / 2.0 * down);
            widgets::paint_key(&painter, shown, &view, hot, i == self.selected, pal, accent);
            let resp = match (problem, key_tooltip(button, &targets)) {
                (Some(problem), _) => resp.on_hover_text(t(problem)),
                (None, Some(tip)) => resp.on_hover_text(tip),
                (None, None) => resp,
            };
            if resp.clicked() {
                self.selected = i;
                // O egui só dá foco por Tab; sem isto, as setas não funcionam após o clique.
                resp.request_focus();
            }
            if resp.double_clicked() {
                if let ButtonAction::SwitchPage { page } = button.action {
                    events.open_page = Some(page);
                }
            }
            if let Some(payload) = resp.dnd_release_payload::<Dragged>() {
                match *payload {
                    Dragged::Preset(p) => events.dropped = Some((i, p)),
                    Dragged::Key(from) if from != i => events.swapped = Some((from, i)),
                    Dragged::Key(_) => {}
                }
            }
        }
        events
    }

    /// Arquivo de imagem solto na janela: vira a imagem da tecla sob o ponteiro (ou a selecionada,
    /// quando o sistema não informa onde o arquivo caiu).
    fn apply_dropped_files(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        under: Option<usize>,
    ) {
        let files: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        let Some(path) = files.first() else {
            return;
        };
        let target = under.unwrap_or(self.selected);
        let Some(image) = super::panes::import_image(path) else {
            self.toasts
                .push(t("Solte uma imagem PNG, JPG, SVG ou ICO"), None);
            return;
        };
        let Some(b) = g.cfg.page_mut().buttons.get_mut(target) else {
            return;
        };
        b.appearance.icon_path = Some(image);
        b.appearance.icon = None;
        self.selected = target;
        self.touch();
        self.toasts
            .push(tf("Imagem na tecla {}", &[&(target + 1)]), Some("Desfazer"));
    }

    fn apply_device_events(&mut self, g: &mut runtime::Shared, events: DeviceEvents) {
        if let Some(id) = events.open_page {
            self.go_to_page(g, id);
        }
        if let Some((button, preset)) = events.dropped {
            self.selected = button;
            self.apply_preset(g, button, preset);
        }
        if let Some((from, to)) = events.swapped {
            let buttons = &mut g.cfg.page_mut().buttons;
            if from < buttons.len() && to < buttons.len() {
                buttons.swap(from, to);
                self.selected = to;
                self.touch();
                self.toasts.push("Teclas trocadas", Some("Desfazer"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grade 3 x 2 de teclas de 10 px com 2 px de folga, como no aparelho.
    fn grid() -> Vec<Rect> {
        (0..6)
            .map(|i| {
                let (col, row) = ((i % 3) as f32, (i / 3) as f32);
                Rect::from_min_size(pos2(col * 12.0, row * 12.0), vec2(10.0, 10.0))
            })
            .collect()
    }

    #[test]
    fn alt_arrows_find_the_key_next_to_the_selected_one() {
        let keys = grid();
        assert_eq!(neighbor(&keys, 0, vec2(1.0, 0.0)), Some(1));
        assert_eq!(neighbor(&keys, 1, vec2(-1.0, 0.0)), Some(0));
        assert_eq!(neighbor(&keys, 1, vec2(0.0, 1.0)), Some(4));
        assert_eq!(neighbor(&keys, 4, vec2(0.0, -1.0)), Some(1));
        // Na borda não há vizinha: nada se move (em vez de dar a volta sem aviso).
        assert_eq!(neighbor(&keys, 2, vec2(1.0, 0.0)), None);
        assert_eq!(neighbor(&keys, 0, vec2(0.0, -1.0)), None);
        assert_eq!(neighbor(&keys, 9, vec2(1.0, 0.0)), None);
    }
}
