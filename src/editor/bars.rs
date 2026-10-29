//! Barra do topo do editor (marca, perfil, estado, desfazer/refazer, menus) e abas de página.
use super::{settings, theme, widgets, App};
use crate::editor::theme::Palette;
use crate::editor::tokens::text::TITLE;
use crate::platform::shell;
use crate::service::runtime;
use crate::strings::{display_name, t, tf};
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
    /// Barra fina: marca e perfil à esquerda; estado, desfazer/refazer, menus e preferências à
    /// direita. As páginas ficam no palco, coladas ao aparelho (ver `page_tabs`).
    pub(crate) fn top_bar(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        (pal, _accent): (&Palette, Color32),
        undo: &mut bool,
        redo: &mut bool,
    ) {
        let frame = Frame::none()
            .fill(pal.panel)
            .stroke(egui::Stroke::new(1.0_f32, pal.line))
            .inner_margin(Margin::symmetric(16.0, 8.0));
        egui::TopBottomPanel::top("top")
            .frame(frame)
            .show(ctx, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, "settings", "Preferências", true, pal).clicked() {
                        self.open_settings();
                    }
                    self.manage_menu(ui, g, pal);
                    ui.add_space(WORDMARK_GAP);
                    self.undo_redo(ui, undo, redo, pal);
                    ui.add_space(WORDMARK_GAP);
                    self.save_chip(ui, pal);
                    self.connection_chip(ui, g, pal);
                    let left = Layout::left_to_right(Align::Center);
                    ui.with_layout(left, |ui| {
                        ui.label(
                            RichText::new(t("AKP153E"))
                                .font(theme::display_font(TITLE - 3.0))
                                .strong(),
                        );
                        ui.add_space(WORDMARK_GAP * 2.0);
                        ui.label(RichText::new(t("Perfil")).color(pal.muted));
                        self.profile_combo(ui, g);
                    });
                });
            });
    }

    /// Ícones em ordem visual Desfazer, Refazer (o layout à direita os desenha ao contrário).
    fn undo_redo(&self, ui: &mut egui::Ui, undo: &mut bool, redo: &mut bool, pal: &Palette) {
        *redo |= widgets::icon_button(
            ui,
            "redo",
            "Refazer (Ctrl+Shift+Z)",
            self.history.can_redo(),
            pal,
        )
        .clicked();
        *undo |= widgets::icon_button(
            ui,
            "undo",
            "Desfazer (Ctrl+Z)",
            self.history.can_undo(),
            pal,
        )
        .clicked();
    }

    /// Estado da gravação, ao lado do estado da conexão.
    fn save_chip(&self, ui: &mut egui::Ui, pal: &Palette) {
        if let Some(err) = &self.save_error {
            widgets::status_chip(ui, "Falha ao salvar", pal.bad).on_hover_text(format!(
                "{err}
{}",
                t("Nova tentativa em instantes.")
            ));
        } else if self.dirty {
            widgets::status_chip(ui, "Salvando…", pal.muted);
        } else {
            widgets::status_chip(ui, "Salvo", pal.ok);
        }
    }

    /// Chip de conexão; ao clicar, abre um menu com reconectar e a pasta de logs.
    fn connection_chip(&mut self, ui: &mut egui::Ui, g: &runtime::Shared, pal: &Palette) {
        let (label, color) = if !g.service_running {
            ("SERVIÇO PARADO", pal.bad)
        } else if g.connected {
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
            let msg = if !g.service_running {
                t("O serviço em segundo plano não está rodando.")
            } else if g.connected {
                t("Aparelho conectado.")
            } else {
                t("Aparelho não encontrado. Confira o cabo USB.")
            };
            ui.label(msg);
            if !g.service_running {
                if ui.button(t("Iniciar serviço")).clicked() {
                    shell::start_service();
                }
            } else if ui.button(t("Reconectar")).clicked() {
                let _ = shell::send("reconnect");
            }
            if ui.button(t("Abrir pasta de logs")).clicked() {
                settings::open_log_folder();
            }
        });
    }

    fn profile_combo(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared) {
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
    }

    /// Abas de página em controle segmentado, acima do aparelho. Duplo clique renomeia;
    /// clique direito abre mover/excluir.
    pub(crate) fn page_tabs(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let prof = g.cfg.active_profile;
        widgets::segmented(ui, pal, |ui| {
            let pages = g.cfg.profiles[prof].pages.len();
            for i in 0..pages {
                if self.rename.as_ref().is_some_and(|r| r.page == i) {
                    self.rename_editor(ui, g, prof);
                    continue;
                }
                let name = display_name(&g.cfg.profiles[prof].pages[i].name);
                let active = g.cfg.profiles[prof].active_page == i;
                let resp = widgets::segment(ui, &name, active, 0.0, pal);
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
            let add = widgets::icon_button(ui, "plus", "Nova página", true, pal);
            if add.clicked() {
                g.cfg.profiles[prof].add_page(&tf("Página {}", &[&(pages + 1)]));
                self.touch();
            }
        });
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
}
