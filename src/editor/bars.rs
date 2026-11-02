//! Barra do topo do editor (marca, perfil, estado, desfazer/refazer, menus) e abas de página.
use super::{settings, theme, widgets, App};
use crate::editor::theme::Palette;
use crate::editor::tokens::text::TITLE;
use crate::platform::shell;
use crate::service::runtime;
use crate::strings::{display_name, t, tf};
use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Sense};

const CONN_POPUP_WIDTH: f32 = 220.0;
const WORDMARK_GAP: f32 = 8.0;
const TOP_SEPARATOR_HEIGHT: f32 = 18.0;
const PROFILE_ICON: f32 = 14.0;
const MANAGE_MENU_WIDTH: f32 = 260.0;

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
        widgets::text_field(ui, text, "", pal, false).labelled_by(label)
    })
    .inner
}

impl App {
    /// Barra fina: marca e perfil à esquerda; estado, desfazer/refazer, menus e preferências à
    /// direita. As páginas ficam no palco, coladas ao aparelho (ver `page_tabs`).
    pub(crate) fn top_bar(
        &mut self,
        root: &mut egui::Ui,
        g: &mut runtime::Shared,
        (pal, accent): (&Palette, Color32),
        (undo, redo): (&mut bool, &mut bool),
        panels: super::layout::Panels,
    ) {
        let frame = Frame::NONE
            .fill(pal.panel)
            .stroke(egui::Stroke::new(1.0_f32, pal.line))
            .inner_margin(Margin::symmetric(16, 10));
        let shown = egui::Panel::top("top").frame(frame).show(root, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, "settings", "Preferências", true, pal).clicked() {
                    self.open_settings();
                }
                ui.add_space(WORDMARK_GAP);
                self.undo_redo(ui, undo, redo, pal);
                ui.add_space(WORDMARK_GAP);
                self.sync_chip(ui, g, pal);
                let left = Layout::left_to_right(Align::Center);
                ui.with_layout(left, |ui| {
                    // Janela estreita: o botão "Ações" abre a biblioteca no lugar do inspetor.
                    if panels.compact
                        && widgets::pill(ui, "Ações", self.library_open, accent, pal).clicked()
                    {
                        self.library_open = !self.library_open;
                    }
                    widgets::brand_mark(ui, accent);
                    if panels.full_top_bar {
                        ui.label(
                            RichText::new("Keyforge")
                                .font(theme::display_font(TITLE - 2.0))
                                .color(pal.fg)
                                .strong(),
                        );
                        ui.add_space(WORDMARK_GAP);
                        // Filete vertical entre a marca e o contexto (perfil).
                        let (sep, _) = ui.allocate_exact_size(
                            egui::vec2(1.0, TOP_SEPARATOR_HEIGHT),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(sep, 0.0, pal.line);
                        ui.add_space(WORDMARK_GAP);
                        widgets::icon(ui, "user", PROFILE_ICON, pal.muted);
                        ui.label(RichText::new(t("Perfil")).color(pal.muted));
                    }
                    self.profile_combo(ui, g, pal);
                    // Gerenciar fica junto do perfil, que é o que ele gerencia.
                    self.manage_menu(ui, g, pal);
                });
            });
        });
        let ctx = shown.response.ctx.clone();
        widgets::landmark(
            &ctx,
            shown.response.id,
            egui::accesskit::Role::Banner,
            "Keyforge",
        );
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

    /// Um selo só responde "o aparelho mostra o que estou vendo?": junta o serviço, o cabo e a
    /// gravação (que é automática: "salvo" só seria ruído). Ao clicar, reconectar e os logs.
    fn sync_chip(&mut self, ui: &mut egui::Ui, g: &runtime::Shared, pal: &Palette) {
        let (label, color) = if !g.service_running {
            ("Serviço parado", pal.bad)
        } else if !g.connected {
            ("Desconectado", pal.bad)
        } else if self.save_error.is_some() {
            ("Falha ao salvar", pal.bad)
        } else if self.unsaved() {
            ("Enviando…", pal.muted)
        } else {
            ("No aparelho", pal.ok)
        };
        let chip = widgets::status_chip(ui, label, color);
        let resp = ui
            .interact(chip.rect, ui.scope_id().with("conn_chip"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        widgets::name(&resp, "Estado do aparelho");
        let popup = ui.scope_id().with("conn_popup");
        if resp.clicked() {
            egui::Popup::toggle_id(ui.ctx(), popup);
        }
        let close = egui::PopupCloseBehavior::CloseOnClickOutside;
        egui::Popup::from_response(&resp)
            .id(popup)
            .open_memory(None)
            .close_behavior(close)
            .show(|ui| {
                ui.set_min_width(CONN_POPUP_WIDTH);
                if let Some(err) = &self.save_error {
                    ui.label(format!("{err}\n{}", t("Nova tentativa em instantes.")));
                }
                let msg = if !g.service_running {
                    t("O serviço em segundo plano não está rodando.")
                } else if g.connected {
                    t("Aparelho conectado.")
                } else {
                    t("Aparelho não encontrado. Confira o cabo USB.")
                };
                ui.label(msg);
                if !g.service_running {
                    if widgets::menu_entry(
                        ui,
                        Some("power"),
                        "Iniciar serviço",
                        "",
                        true,
                        false,
                        pal,
                    )
                    .clicked()
                    {
                        shell::start_service();
                    }
                } else if widgets::menu_entry(
                    ui,
                    Some("refresh-cw"),
                    "Reconectar",
                    "",
                    true,
                    false,
                    pal,
                )
                .clicked()
                {
                    let _ = shell::send("reconnect");
                }
                let logs = widgets::menu_entry(
                    ui,
                    Some("folder-open"),
                    "Abrir pasta de logs",
                    "",
                    true,
                    false,
                    pal,
                );
                if logs.clicked() {
                    settings::open_log_folder();
                }
            });
    }

    fn profile_combo(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let prof = g.cfg.active_profile;
        let names: Vec<String> = g
            .cfg
            .profiles
            .iter()
            .map(|p| display_name(&p.name))
            .collect();
        let selected_name = display_name(&g.cfg.profiles[prof].name);
        let mut selected = selected_name.clone();
        widgets::combo_box(
            ui,
            "profile",
            "Perfil",
            &mut selected,
            &names.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
            pal,
        );
        let new_prof = names.iter().position(|n| n == &selected).unwrap_or(prof);
        if new_prof != prof {
            g.cfg.active_profile = new_prof;
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
                    self.rename_editor(ui, g, prof, pal);
                    continue;
                }
                let page = &g.cfg.profiles[prof].pages[i];
                let active = g.cfg.profiles[prof].active_page == i;
                // Pasta não é aba: aparece só enquanto está aberta, marcada como pasta.
                if page.folder && !active {
                    continue;
                }
                let plain = display_name(&page.name);
                let name = if page.folder {
                    tf("Pasta: {}", &[&plain])
                } else {
                    plain.clone()
                };
                let resp = widgets::segment(ui, &name, active, 0.0, pal);
                if resp.clicked() {
                    g.cfg.profiles[prof].active_page = i;
                    self.selected = 0;
                    self.touch_nav();
                }
                if resp.double_clicked() {
                    self.rename = Some(Rename::new(i, &plain));
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
    fn rename_editor(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        prof: usize,
        pal: &Palette,
    ) {
        let Some(r) = self.rename.as_mut() else {
            return;
        };
        let edit = widgets::text_field(ui, &mut r.buf, "", pal, false);
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

    /// Mover e excluir a página `index`; compartilhado pelo menu de contexto e pelo "Perfil e página".
    fn page_actions(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        prof: usize,
        index: usize,
        pal: &Palette,
    ) {
        for (icon, label, delta) in [
            ("arrow-left", "Mover para a esquerda", -1),
            ("arrow-right", "Mover para a direita", 1),
        ] {
            if widgets::menu_entry(ui, Some(icon), label, "", true, false, pal).clicked() {
                if g.cfg.profiles[prof].move_page(index, delta).is_some() {
                    self.touch_nav();
                }
                ui.close();
            }
        }
        let delete = widgets::menu_entry(ui, Some("trash"), "Excluir página", "", true, true, pal);
        if delete.clicked() {
            if g.cfg.profiles[prof].remove_page(index) {
                self.selected = 0;
                self.touch_nav();
                self.toasts.push(t("Página excluída"), Some("Desfazer"));
            }
            ui.close();
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
        let rename = widgets::menu_entry(ui, Some("pencil"), "Renomear", "", true, false, pal);
        if rename.clicked() {
            self.rename = Some(Rename::new(index, name));
            ui.close();
        }
        self.page_actions(ui, g, prof, index, pal);
    }

    pub(crate) fn manage_menu(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
    ) {
        let kind = widgets::ButtonKind::Ghost;
        let open = widgets::button(ui, Some("layers"), "Gerenciar", kind, true, 0.0, pal);
        egui::Popup::menu(&open).show(|ui| {
            ui.set_min_width(MANAGE_MENU_WIDTH);
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
        self.auto_app_items(ui, g, prof, pal);
        ui.separator();
        if widgets::menu_entry(ui, Some("user-plus"), "Novo perfil", "", true, false, pal).clicked()
        {
            let name = tf("Perfil {}", &[&(g.cfg.profiles.len() + 1)]);
            g.cfg.add_profile(&name);
            self.touch();
            ui.close();
        }
        self.profile_file_items(ui, g, prof, pal);
        let delete = widgets::menu_entry(ui, Some("trash"), "Excluir perfil", "", true, true, pal);
        if delete.clicked() {
            if g.cfg.remove_profile(prof) {
                self.selected = 0;
                self.touch_nav();
                self.toasts.push(t("Perfil excluído"), Some("Desfazer"));
            }
            ui.close();
        }
    }
}
