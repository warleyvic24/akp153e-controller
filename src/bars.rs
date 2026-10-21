//! Barras do editor: topo (estado, perfis, páginas, menus) e rodapé (desfazer/refazer).
use crate::config::schema::*;
use crate::theme::Palette;
use crate::{runtime, theme, widgets, App};
use eframe::egui::{self, vec2, Align, Color32, Frame, Layout, Margin, RichText};

impl App {
    pub(crate) fn top_bar(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let frame = Frame::none()
            .fill(pal.panel)
            .inner_margin(Margin::symmetric(18.0, 10.0));
        egui::TopBottomPanel::top("top")
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(widgets::spaced("AKP153E"))
                            .font(theme::display_font(20.0))
                            .strong(),
                    );
                    let (text, color) = if g.connected {
                        ("CONECTADO", pal.ok)
                    } else {
                        ("DESCONECTADO", pal.bad)
                    };
                    ui.label(
                        RichText::new(format!("● {text}"))
                            .monospace()
                            .size(11.5)
                            .color(color),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.manage_menu(ui, g, pal);
                        self.appearance_menu(ui, g);
                    });
                });
                ui.horizontal_wrapped(|ui| self.profile_and_pages(ui, g, pal, accent));
            });
    }

    pub(crate) fn profile_and_pages(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let mut prof = g.cfg.active_profile;
        egui::ComboBox::from_id_salt("prof")
            .selected_text(g.cfg.profiles[prof].name.clone())
            .show_ui(ui, |ui| {
                for (i, p) in g.cfg.profiles.iter().enumerate() {
                    ui.selectable_value(&mut prof, i, &p.name);
                }
            });
        if prof != g.cfg.active_profile {
            g.cfg.active_profile = prof;
            self.selected = 0;
            self.touch_nav();
        }
        let pages = g.cfg.profiles[prof].pages.len();
        for i in 0..pages {
            let name = g.cfg.profiles[prof].pages[i].name.clone();
            if widgets::pill(
                ui,
                &name,
                g.cfg.profiles[prof].active_page == i,
                accent,
                pal,
            )
            .clicked()
            {
                g.cfg.profiles[prof].active_page = i;
                self.selected = 0;
                self.touch_nav();
            }
        }
        if widgets::pill(ui, "+", false, accent, pal)
            .on_hover_text("Nova página")
            .clicked()
        {
            g.cfg.profiles[prof].add_page(&format!("Page {}", pages + 1));
            self.touch();
        }
    }

    pub(crate) fn appearance_menu(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared) {
        ui.menu_button("Aparência", |ui| {
            for (theme, name) in [(Theme::Dark, "Escuro"), (Theme::Light, "Claro (azul)")] {
                if ui.selectable_label(g.cfg.ui.theme == theme, name).clicked() {
                    g.cfg.ui.theme = theme;
                    self.dirty = true;
                }
            }
            ui.separator();
            ui.label("Cor de destaque");
            ui.horizontal(|ui| {
                if ui.color_edit_button_srgb(&mut g.cfg.ui.accent).changed() {
                    self.dirty = true;
                }
                for (name, rgb) in theme::ACCENTS {
                    let fill = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    if ui
                        .add(
                            egui::Button::new("  ")
                                .fill(fill)
                                .min_size(vec2(22.0, 22.0)),
                        )
                        .on_hover_text(name)
                        .clicked()
                    {
                        g.cfg.ui.accent = rgb;
                        self.dirty = true;
                    }
                }
            });
        });
    }

    pub(crate) fn manage_menu(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
    ) {
        ui.menu_button("Gerenciar", |ui| {
            let prof = g.cfg.active_profile;
            let index = g.cfg.profiles[prof].active_page;
            ui.label("Nome da página");
            if ui
                .text_edit_singleline(&mut g.cfg.profiles[prof].pages[index].name)
                .changed()
            {
                self.touch();
            }
            if ui.button("Mover página para a esquerda").clicked() {
                if g.cfg.profiles[prof].move_page(index, -1).is_some() {
                    self.touch_nav();
                }
                ui.close_menu();
            }
            if ui.button("Mover página para a direita").clicked() {
                if g.cfg.profiles[prof].move_page(index, 1).is_some() {
                    self.touch_nav();
                }
                ui.close_menu();
            }
            if ui
                .button(RichText::new("Excluir página").color(pal.bad))
                .clicked()
            {
                if g.cfg.profiles[prof].remove_page(index) {
                    self.selected = 0;
                    self.touch_nav();
                }
                ui.close_menu();
            }
            ui.separator();
            ui.label("Nome do perfil");
            if ui
                .text_edit_singleline(&mut g.cfg.profiles[prof].name)
                .changed()
            {
                self.touch();
            }
            if ui.button("Novo perfil").clicked() {
                let name = format!("Perfil {}", g.cfg.profiles.len() + 1);
                g.cfg.add_profile(&name);
                self.touch();
                ui.close_menu();
            }
            if ui
                .button(RichText::new("Excluir perfil").color(pal.bad))
                .clicked()
            {
                if g.cfg.remove_profile(prof) {
                    self.selected = 0;
                    self.touch_nav();
                }
                ui.close_menu();
            }
        });
    }

    pub(crate) fn footer(
        &self,
        ctx: &egui::Context,
        pal: &Palette,
        undo: &mut bool,
        redo: &mut bool,
    ) {
        let frame = Frame::none()
            .fill(pal.bg)
            .inner_margin(Margin::symmetric(18.0, 6.0));
        let (can_undo, can_redo) = (self.history.can_undo(), self.history.can_redo());
        egui::TopBottomPanel::bottom("foot")
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    *undo |= ui
                        .add_enabled(can_undo, egui::Button::new("Desfazer"))
                        .on_hover_text("Ctrl+Z")
                        .clicked();
                    *redo |= ui
                        .add_enabled(can_redo, egui::Button::new("Refazer"))
                        .on_hover_text("Ctrl+Shift+Z")
                        .clicked();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let text = if self.dirty { "Salvando…" } else { "Salvo" };
                        ui.label(RichText::new(text).monospace().size(11.0).color(pal.muted));
                    });
                });
            });
    }
}
