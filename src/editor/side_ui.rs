//! Tela lateral no editor: a faixa à direita das teclas, desenhada como sai no aparelho, e a janela
//! que escolhe o que ela mostra nesta página.
use super::{theme, tokens, widgets, App};
use crate::config::schema::SideScreen;
use crate::editor::theme::Palette;
use crate::graphics::side::{self, SideInfo};
use crate::platform::live::Live;
use crate::service::runtime;
use crate::strings::{display_name, t, tf};
use eframe::egui::{self, Color32, RichText, Rounding, Sense, Stroke};

const WINDOW_WIDTH: f32 = 320.0;
const OFF_LABEL_ALPHA: f32 = 0.55;

/// Prévia da tela lateral, refeita só quando o conteúdo (ou o minuto, no relógio) muda.
#[derive(Default)]
pub(crate) struct SideTexture(Option<(SideScreen, SideInfo, u64, Live, egui::TextureHandle)>);

fn info(g: &runtime::Shared) -> SideInfo {
    let profile = &g.cfg.profiles[g.cfg.active_profile];
    let [r, gr, b] = g.cfg.ui.accent;
    SideInfo {
        profile: display_name(&profile.name),
        page: display_name(&profile.pages[profile.active_page].name),
        page_index: profile.active_page,
        page_count: profile.pages.len(),
        accent: crate::config::schema::RGBAColor {
            r,
            g: gr,
            b,
            a: 255,
        },
    }
}

impl App {
    /// Desenha a faixa em `rect` (a 6ª coluna do aparelho); clicar abre a janela da tela lateral.
    pub(crate) fn side_strip(
        &mut self,
        ui: &mut egui::Ui,
        g: &runtime::Shared,
        rect: egui::Rect,
        pal: &Palette,
        accent: Color32,
    ) {
        let resp = ui.interact(rect, ui.id().with("side-screen"), Sense::click());
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("Tela lateral"))
        });
        let p = ui.painter();
        let round = Rounding::same(tokens::radius::KEY);
        p.rect_filled(rect, round, pal.keyoff);
        let side = g.cfg.page().side.clone();
        let minute = if side == SideScreen::Clock {
            super::current_minute()
        } else {
            0
        };
        // Só o modo "Sistema" depende dos dados ao vivo.
        let live = if side == SideScreen::System {
            g.live.clone()
        } else {
            Live::default()
        };
        let info = info(g);
        let fresh =
            self.side_tex.0.as_ref().is_some_and(|(s, i, m, l, _)| {
                *s == side && *i == info && *m == minute && *l == live
            });
        if !fresh {
            self.side_tex.0 = side::render(&side, &info, &live).map(|img| {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [img.width() as usize, img.height() as usize],
                    img.as_raw(),
                );
                let tex = ui
                    .ctx()
                    .load_texture("side-screen", image, egui::TextureOptions::LINEAR);
                (side.clone(), info.clone(), minute, live.clone(), tex)
            });
        }
        match &self.side_tex.0 {
            Some((_, _, _, _, tex)) => {
                let mut shape = egui::epaint::RectShape::filled(rect, round, Color32::WHITE);
                shape.fill_texture_id = tex.id();
                shape.uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                p.add(shape);
            }
            None => {
                // Apagada: só um rótulo deitado, para quem não sabe que a faixa existe.
                let galley = p.layout_no_wrap(
                    t("Tela lateral").to_string(),
                    egui::FontId::proportional(tokens::text::CAPTION),
                    pal.device_fg.gamma_multiply(OFF_LABEL_ALPHA),
                );
                let pos = egui::pos2(
                    rect.center().x + galley.size().y / 2.0,
                    rect.center().y - galley.size().x / 2.0,
                );
                p.add(
                    egui::epaint::TextShape::new(pos, galley, Color32::WHITE)
                        .with_angle(std::f32::consts::FRAC_PI_2),
                );
            }
        }
        let hot = resp.hovered() || self.side_open;
        let stroke = if hot {
            Stroke::new(1.5_f32, accent.gamma_multiply(0.8))
        } else {
            Stroke::new(1.0_f32, Color32::from_white_alpha(14))
        };
        p.rect_stroke(rect, round, stroke);
        let resp = resp.on_hover_text(t(
            "Tela lateral: clique para escolher o que ela mostra nesta página",
        ));
        if resp.clicked() {
            self.side_open = true;
        }
    }

    /// Janela "Tela lateral": o que a faixa mostra na página ativa.
    pub(crate) fn side_window(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
    ) {
        if !self.side_open {
            return;
        }
        let accent = theme::accent_color(&g.cfg.ui);
        let mut open = true;
        let page = display_name(&g.cfg.page().name);
        let title = RichText::new(tf("Tela lateral · {}", &[&page]))
            .size(tokens::text::BODY)
            .strong();
        let mut changed = false;
        egui::Window::new(title)
            .id(egui::Id::new("side-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(WINDOW_WIDTH)
            .show(ctx, |ui| {
                let side = &mut g.cfg.page_mut().side;
                widgets::section(ui, "Mostrar", pal);
                ui.horizontal_wrapped(|ui| {
                    let modes = [
                        (SideScreen::Off, "Nada"),
                        (SideScreen::PageName, "Nome da página"),
                        (SideScreen::Clock, "Relógio"),
                        (SideScreen::System, "Sistema"),
                    ];
                    for (mode, name) in modes {
                        let on = *side == mode;
                        if widgets::pill(ui, name, on, accent, pal).clicked() && !on {
                            *side = mode;
                            changed = true;
                        }
                    }
                    let is_image = matches!(side, SideScreen::Image { .. });
                    if widgets::pill(ui, "Imagem", is_image, accent, pal).clicked() {
                        if let Some(path) = pick_side_image() {
                            *side = SideScreen::Image { path };
                            changed = true;
                        }
                    }
                });
                if let SideScreen::Image { path } = side {
                    let name = std::path::Path::new(path.as_str())
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(name).on_hover_text(path.as_str());
                        if ui.button(t("Trocar")).clicked() {
                            if let Some(new) = pick_side_image() {
                                *path = new;
                                changed = true;
                            }
                        }
                    });
                    let hint = tf("Recortada para preencher {} x {} px (em pé).", &[&side::WIDTH, &side::HEIGHT]);
                    ui.label(widgets::hint(&hint, pal));
                }
                ui.add_space(tokens::space::SM);
                let note = t("Cada página tem a sua. Experimental: o desenho da faixa foi medido num AKP153E 0300:3010 por outro projeto; se aparecer cortado ou fora do lugar, avise.");
                ui.label(RichText::new(note).size(tokens::text::CAPTION).color(pal.muted));
            });
        if changed {
            self.touch();
        }
        self.side_open = open;
    }
}

fn pick_side_image() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter(
            t("Imagem (PNG, JPG, SVG, ICO)"),
            &super::panes::IMAGE_EXTENSIONS,
        )
        .pick_file()
        .and_then(|p| super::panes::import_image(&p))
}
