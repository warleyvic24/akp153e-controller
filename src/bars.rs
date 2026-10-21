//! Barras do editor: topo (estado, perfis, páginas, menus) e rodapé (desfazer/refazer).
use crate::strings::{display_name, t, tf};
use crate::theme::Palette;
use crate::tokens::text::TITLE;
use crate::{runtime, settings, shell, theme, widgets, App};
use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Sense};

const CONN_POPUP_WIDTH: f32 = 220.0;
const WORDMARK_GAP: f32 = 6.0;
const RENAME_WIDTH: f32 = 120.0;

/// Página em renomeação direto na barra (duplo clique ou "Renomear" no menu de contexto).
pub(crate) struct Rename {
    page: usize,
    buf: String,
    focused: bool,
}

impl Rename {
    fn new(page: usize, name: &str) -> Self {
        Self {
            page,
            buf: name.to_string(),
            focused: false,
        }
    }
}

/// Campo de texto dentro de menu. O egui tira a borda dos widgets em menus; sem ela o campo some
/// no fundo, então a borda de controle (3:1) é restaurada só para ele.
fn menu_field(
    ui: &mut egui::Ui,
    text: &mut String,
    label: egui::Id,
    pal: &Palette,
) -> egui::Response {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, pal.control);
        ui.text_edit_singleline(text).labelled_by(label)
    })
    .inner
}

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
                        RichText::new(t("AKP153E"))
                            .font(theme::display_font(TITLE))
                            .strong(),
                    );
                    ui.add_space(WORDMARK_GAP);
                    self.connection_chip(ui, g, pal);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.manage_menu(ui, g, pal);
                        if ui.button(t("Preferências")).clicked() {
                            self.open_settings();
                        }
                    });
                });
                ui.horizontal_wrapped(|ui| self.profile_and_pages(ui, g, pal, accent));
            });
    }

    /// Chip de conexão; ao clicar, abre um menu com reconectar e a pasta de logs.
    fn connection_chip(&mut self, ui: &mut egui::Ui, g: &runtime::Shared, pal: &Palette) {
        let (label, color) = if g.connected {
            ("CONECTADO", pal.ok)
        } else {
            ("DESCONECTADO", pal.bad)
        };
        let chip = widgets::status_chip(ui, label, color);
        let resp = ui
            .interact(chip.rect, ui.id().with("conn_chip"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        widgets::name(&resp, "Estado da conexão");
        let popup = ui.id().with("conn_popup");
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        let close = egui::PopupCloseBehavior::CloseOnClickOutside;
        egui::popup_below_widget(ui, popup, &resp, close, |ui| {
            ui.set_min_width(CONN_POPUP_WIDTH);
            let msg = if g.connected {
                t("Aparelho conectado.")
            } else {
                t("Aparelho não encontrado. Confira o cabo USB.")
            };
            ui.label(msg);
            if ui.button(t("Reconectar")).clicked() {
                let _ = shell::send("reconnect");
            }
            if ui.button(t("Abrir pasta de logs")).clicked() {
                settings::open_log_folder();
            }
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
        let combo = egui::ComboBox::from_id_salt("prof")
            .selected_text(display_name(&g.cfg.profiles[prof].name))
            .show_ui(ui, |ui| {
                for (i, p) in g.cfg.profiles.iter().enumerate() {
                    ui.selectable_value(&mut prof, i, display_name(&p.name));
                }
            });
        widgets::name(&combo.response, "Perfil");
        if prof != g.cfg.active_profile {
            g.cfg.active_profile = prof;
            self.selected = 0;
            self.touch_nav();
        }
        let pages = g.cfg.profiles[prof].pages.len();
        for i in 0..pages {
            if self.rename.as_ref().is_some_and(|r| r.page == i) {
                self.rename_editor(ui, g, prof);
                continue;
            }
            let name = display_name(&g.cfg.profiles[prof].pages[i].name);
            let active = g.cfg.profiles[prof].active_page == i;
            let resp = widgets::pill(ui, &name, active, accent, pal);
            if resp.clicked() {
                g.cfg.profiles[prof].active_page = i;
                self.selected = 0;
                self.touch_nav();
            }
            if resp.double_clicked() {
                self.rename = Some(Rename::new(i, &name));
            }
            resp.on_hover_text(t("Duplo clique renomeia · clique direito abre o menu"))
                .context_menu(|ui| self.page_menu(ui, g, prof, i, &name, pal));
        }
        let add = widgets::pill(ui, "+", false, accent, pal);
        widgets::name(&add, "Nova página");
        if add.on_hover_text(t("Nova página")).clicked() {
            g.cfg.profiles[prof].add_page(&tf("Página {}", &[&(pages + 1)]));
            self.touch();
        }
    }

    /// Campo no lugar da pílula: Enter ou perder o foco confirma, Esc cancela.
    fn rename_editor(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, prof: usize) {
        let Some(r) = self.rename.as_mut() else {
            return;
        };
        let edit = ui.add(egui::TextEdit::singleline(&mut r.buf).desired_width(RENAME_WIDTH));
        widgets::name(&edit, "Novo nome da página");
        if !r.focused {
            edit.request_focus();
            r.focused = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.rename = None;
            return;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) || edit.lost_focus() {
            let (page, name) = (r.page, r.buf.trim().to_string());
            self.rename = None;
            let current = &mut g.cfg.profiles[prof].pages[page].name;
            if !name.is_empty() && *current != name {
                *current = name;
                self.touch();
            }
        }
    }

    /// Mover e excluir a página `index`; compartilhado pelo menu de contexto e pelo "Gerenciar".
    fn page_actions(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        prof: usize,
        index: usize,
        pal: &Palette,
    ) {
        for (label, delta) in [("Mover para a esquerda", -1), ("Mover para a direita", 1)] {
            if ui.button(t(label)).clicked() {
                if g.cfg.profiles[prof].move_page(index, delta).is_some() {
                    self.touch_nav();
                }
                ui.close_menu();
            }
        }
        let delete = ui.button(RichText::new(t("Excluir página")).color(pal.bad));
        if delete.clicked() {
            if g.cfg.profiles[prof].remove_page(index) {
                self.selected = 0;
                self.touch_nav();
                self.toasts.push(t("Página excluída"), Some("Desfazer"));
            }
            ui.close_menu();
        }
    }

    fn page_menu(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        prof: usize,
        index: usize,
        name: &str,
        pal: &Palette,
    ) {
        if ui.button(t("Renomear")).clicked() {
            self.rename = Some(Rename::new(index, name));
            ui.close_menu();
        }
        self.page_actions(ui, g, prof, index, pal);
    }

    pub(crate) fn manage_menu(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
    ) {
        ui.menu_button(t("Gerenciar"), |ui| {
            let prof = g.cfg.active_profile;
            let index = g.cfg.profiles[prof].active_page;
            let page_label = ui.label(t("Nome da página")).id;
            let page_name = &mut g.cfg.profiles[prof].pages[index].name;
            if menu_field(ui, page_name, page_label, pal).changed() {
                self.touch();
            }
            self.page_actions(ui, g, prof, index, pal);
            ui.separator();
            self.profile_items(ui, g, prof, pal);
        });
    }

    fn profile_items(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        prof: usize,
        pal: &Palette,
    ) {
        let profile_label = ui.label(t("Nome do perfil")).id;
        if menu_field(ui, &mut g.cfg.profiles[prof].name, profile_label, pal).changed() {
            self.touch();
        }
        if ui.button(t("Novo perfil")).clicked() {
            let name = tf("Perfil {}", &[&(g.cfg.profiles.len() + 1)]);
            g.cfg.add_profile(&name);
            self.touch();
            ui.close_menu();
        }
        let delete = ui.button(RichText::new(t("Excluir perfil")).color(pal.bad));
        if delete.clicked() {
            if g.cfg.remove_profile(prof) {
                self.selected = 0;
                self.touch_nav();
                self.toasts.push(t("Perfil excluído"), Some("Desfazer"));
            }
            ui.close_menu();
        }
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
                        .add_enabled(can_undo, egui::Button::new(t("Desfazer")))
                        .on_hover_text(t("Ctrl+Z"))
                        .clicked();
                    *redo |= ui
                        .add_enabled(can_redo, egui::Button::new(t("Refazer")))
                        .on_hover_text(t("Ctrl+Shift+Z"))
                        .clicked();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(err) = &self.save_error {
                            widgets::status_chip(ui, "Falha ao salvar", pal.bad).on_hover_text(
                                format!("{err}\n{}", t("Nova tentativa em instantes.")),
                            );
                        } else if self.dirty {
                            widgets::status_chip(ui, "Salvando…", pal.muted);
                        } else {
                            widgets::status_chip(ui, "Salvo", pal.ok);
                        }
                    });
                });
            });
    }
}
