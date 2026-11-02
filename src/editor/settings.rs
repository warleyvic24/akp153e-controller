//! Janela de Preferências: tema, destaque, início com o Windows, brilho do aparelho e diagnóstico.
use super::{widgets, App};
use crate::config::schema::{AppConfig, Language, Theme};
use crate::editor::omarchy;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::{self, text, TARGET_MIN};
use crate::platform::shell;
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{self, Color32, RichText};

/// Seções das Preferências, na ordem da lista à esquerda.
const SECTIONS: [&str; 4] = ["Geral", "Aparelho", "Integrações", "Sobre"];
const SECTION_GENERAL: usize = 0;
const SECTION_DEVICE: usize = 1;
const SECTION_INTEGRATIONS: usize = 2;
const MODAL_WIDTH: f32 = 620.0;
const MODAL_BODY_HEIGHT: f32 = 420.0;
const NAV_WIDTH: f32 = 150.0;
const NAV_MARK: f32 = 3.0;
const NAV_ON_TINT: f32 = 0.14;
const NAV_HOVER_TINT: f32 = 0.08;
const SWATCH_GAP: f32 = 10.0;
const BRIGHTNESS_MIN: u8 = 5;
const BRIGHTNESS_MAX: u8 = crate::usb::MAX_BRIGHTNESS;
const SLIDER_VALUE_RESERVED: f32 = 72.0;

/// Abre a pasta onde ficam config e logs no gerenciador de arquivos (Explorer ou `xdg-open`).
const SELFTEST_HINT: &str = "O aparelho pede cada tecla, uma pressão longa e duas respostas SIM/NÃO. Nenhuma ação roda; ao fim, a página volta e o resultado fica em selftest.txt, na pasta da config.";

/// O que dizer depois de pedir o autoteste ao serviço (editor e `--selftest`).
pub(crate) fn selftest_reply_text(reply: Option<&str>) -> String {
    t(match reply {
        Some("ok") => "Autoteste no aparelho: aperte a tecla azul de cada vez.",
        Some("sem-aparelho") => "Autoteste: o aparelho não está conectado.",
        Some("em-andamento") => "Autoteste já em andamento no aparelho.",
        Some("aparelho-pequeno") => {
            "Autoteste: este aparelho tem teclas de menos para as respostas SIM/NÃO."
        }
        Some(_) => "Autoteste: o serviço não entendeu o pedido (versão antiga?).",
        None => "Autoteste: o serviço em segundo plano não está rodando.",
    })
    .to_string()
}

pub(crate) fn open_log_folder() {
    let opener = if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let dir = AppConfig::path().and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    match dir {
        Some(dir) => {
            let mut cmd = std::process::Command::new(opener);
            if let Err(e) = crate::platform::spawn_detached(cmd.arg(&dir)) {
                log::warn!("abrir pasta de logs: {e}");
            }
        }
        None => log::warn!("sem pasta de config para abrir"),
    }
}

impl App {
    /// Abre as Preferências, lendo agora o estado do registro (não a cada quadro).
    pub(crate) fn open_settings(&mut self) {
        self.autostart = shell::autostart_enabled();
        self.settings_open = true;
    }

    pub(crate) fn settings_window(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
    ) {
        if !self.settings_open {
            return;
        }
        let ((), close) =
            widgets::modal(ctx, "settings", t("Preferências"), MODAL_WIDTH, pal, |ui| {
                ui.horizontal_top(|ui| {
                    self.settings_nav(ui, g, pal);
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_height(MODAL_BODY_HEIGHT);
                        widgets::scroll_area(ui, pal, |ui| {
                            ui.set_max_height(MODAL_BODY_HEIGHT);
                            self.settings_body(ui, g, pal)
                        });
                    });
                });
            });
        if close {
            self.settings_open = false;
        }
    }

    /// Lista das seções à esquerda do modal.
    fn settings_nav(&mut self, ui: &mut egui::Ui, g: &runtime::Shared, pal: &Palette) {
        let accent = theme::accent_color(&g.cfg.ui);
        ui.vertical(|ui| {
            ui.set_width(NAV_WIDTH);
            for (i, name) in SECTIONS.iter().enumerate() {
                let on = self.settings_tab == i;
                let name = t(name);
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(NAV_WIDTH, TARGET_MIN + tokens::space::SM),
                    egui::Sense::click(),
                );
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, on, name));
                let hot = resp.hovered() || resp.has_focus();
                let round = egui::CornerRadius::from(tokens::radius::CONTROL);
                if on {
                    ui.painter()
                        .rect_filled(rect, round, accent.gamma_multiply(NAV_ON_TINT));
                    // Barra de destaque à esquerda da seção aberta.
                    let bar = egui::Rect::from_min_size(
                        rect.left_top(),
                        egui::vec2(NAV_MARK, rect.height()),
                    );
                    ui.painter().rect_filled(bar, round, accent);
                } else if hot {
                    ui.painter().rect_filled(
                        rect,
                        round,
                        theme::mix(pal.raise, pal.fg, NAV_HOVER_TINT),
                    );
                }
                let ink = if on { theme::ink(accent, pal) } else { pal.fg };
                ui.painter().text(
                    egui::pos2(rect.left() + tokens::space::MD, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::FontId::proportional(text::BODY),
                    ink,
                );
                if resp.clicked() {
                    self.settings_tab = i;
                }
            }
        });
    }

    fn settings_body(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        match self.settings_tab {
            SECTION_GENERAL => {
                self.theme_section(ui, g, pal);
                self.accessibility_section(ui, g, pal);
                self.language_section(ui, g, pal);
                self.startup_section(ui, pal);
            }
            SECTION_DEVICE => {
                self.device_section(ui, g, pal);
                self.device_tools(ui, pal);
            }
            SECTION_INTEGRATIONS => {
                if super::obs_ui::settings_section(ui, &mut g.cfg.obs, pal) {
                    self.touch();
                }
            }
            _ => self.about_section(ui, pal),
        }
    }

    fn theme_section(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let accent = theme::accent_color(&g.cfg.ui);
        widgets::section(ui, "Tema", pal);
        ui.horizontal(|ui| {
            for (value, name) in [
                (Theme::Dark, "Escuro"),
                (Theme::Light, "Claro"),
                (Theme::Omarchy, "Omarchy"),
                (Theme::HighContrast, "Alto contraste"),
            ] {
                let on = g.cfg.ui.theme == value;
                // A opção só aparece onde há um tema do Omarchy (ou se já estiver escolhida).
                if value == Theme::Omarchy && !on && omarchy::current().is_none() {
                    continue;
                }
                if widgets::pill(ui, name, on, accent, pal).clicked() && !on {
                    g.cfg.ui.theme = value;
                    self.dirty = true;
                }
            }
        });
        widgets::section(ui, "Cor de destaque", pal);
        if g.cfg.ui.theme == Theme::HighContrast {
            ui.label(widgets::hint(
                t("No alto contraste o destaque é sempre amarelo"),
                pal,
            ));
            return;
        }
        if g.cfg.ui.theme == Theme::Omarchy && omarchy::current().is_some() {
            ui.label(widgets::hint(
                t("A cor de destaque segue o tema do Omarchy"),
                pal,
            ));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = SWATCH_GAP;
            let custom = ui.color_edit_button_srgb(&mut g.cfg.ui.accent);
            widgets::name(&custom, "Outra cor de destaque");
            if custom.changed() {
                self.dirty = true;
            }
            for (name, rgb) in theme::ACCENTS {
                let fill = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                let on = g.cfg.ui.accent == rgb;
                let size = TARGET_MIN;
                if widgets::swatch(ui, size, size / 2.0, fill, on, pal.fg, t(name))
                    .on_hover_text(name)
                    .clicked()
                {
                    g.cfg.ui.accent = rgb;
                    self.dirty = true;
                }
            }
        });
    }

    /// Tamanho da interface e movimento reduzido.
    fn accessibility_section(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let accent = theme::accent_color(&g.cfg.ui);
        widgets::section(ui, "Tamanho da interface", pal);
        ui.horizontal_wrapped(|ui| {
            for scale in crate::config::schema::UI_SCALES {
                let on = g.cfg.ui.ui_scale == scale;
                let label = format!("{scale}%");
                if widgets::pill(ui, &label, on, accent, pal).clicked() && !on {
                    g.cfg.ui.ui_scale = scale;
                    self.dirty = true;
                }
            }
        });
        ui.label(widgets::hint(
            t("Ctrl + e Ctrl − também aumentam e diminuem, até a próxima abertura."),
            pal,
        ));
        widgets::section(ui, "Movimento", pal);
        if widgets::checkbox(
            ui,
            &mut g.cfg.ui.reduce_motion,
            t("Reduzir movimento (sem deslizes nem transições)"),
        )
        .changed()
        {
            self.dirty = true;
        }
    }

    fn language_section(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let accent = theme::accent_color(&g.cfg.ui);
        widgets::section(ui, "Idioma", pal);
        ui.horizontal(|ui| {
            // Nomes dos idiomas ficam no próprio idioma, para quem não lê o atual achá-los.
            for (value, name) in [(Language::Pt, "Português"), (Language::En, "English")] {
                let on = g.cfg.ui.language == value;
                if widgets::pill(ui, name, on, accent, pal).clicked() && !on {
                    g.cfg.ui.language = value;
                    self.dirty = true;
                }
            }
        });
    }

    fn startup_section(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        widgets::section(ui, "Inicialização", pal);
        let before = self.autostart;
        widgets::checkbox(ui, &mut self.autostart, t(shell::AUTOSTART_LABEL));
        if self.autostart != before {
            let on = self.autostart;
            // O registro pode demorar: fora da thread da interface, como faz a bandeja.
            crate::telemetry::spawn_named("autostart", move || shell::set_autostart(on));
        }
    }

    fn device_section(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let brightness_label = widgets::section(ui, "Brilho do aparelho", pal);
        ui.spacing_mut().slider_width =
            (ui.available_width() - SLIDER_VALUE_RESERVED).max(TARGET_MIN * 4.0);
        let mut brightness = g.cfg.ui.brightness as f32;
        if widgets::slider(
            ui,
            &mut brightness,
            BRIGHTNESS_MIN as f32..=BRIGHTNESS_MAX as f32,
            "%",
        )
        .labelled_by(brightness_label)
        .changed()
        {
            g.cfg.ui.brightness = brightness as u8;
            self.dirty = true;
            // Ao vivo: o aparelho muda enquanto arrasta (gravar na config vem depois, sozinho).
            let _ = shell::send(&format!("brightness {}", g.cfg.ui.brightness));
        }
        let note = if g.connected {
            t("Muda no aparelho enquanto você arrasta.")
        } else {
            t("Sem aparelho conectado: vale a partir da próxima conexão.")
        };
        ui.label(RichText::new(note).size(text::CAPTION).color(pal.muted));
        widgets::section(ui, "Apagar a tela sem uso", pal);
        let name = |m: u16| {
            if m == 0 {
                t("Nunca").to_string()
            } else {
                tf("Após {} min", &[&m])
            }
        };
        let current = g.cfg.ui.sleep_minutes;
        let sleep_choices: Vec<String> = crate::config::schema::SLEEP_CHOICES
            .iter()
            .map(|m| name(*m))
            .collect();
        let sleep_choices_refs: Vec<&str> = sleep_choices.iter().map(|s| s.as_str()).collect();
        let mut sleep_selected = name(current);
        widgets::combo_box(
            ui,
            "sleep-after",
            "Apagar a tela sem uso",
            &mut sleep_selected,
            &sleep_choices_refs,
            pal,
        );
        if sleep_selected != name(current) {
            let new_val = crate::config::schema::SLEEP_CHOICES
                .iter()
                .position(|m| name(*m) == sleep_selected)
                .map(|i| i as u16)
                .unwrap_or(current);
            g.cfg.ui.sleep_minutes = new_val;
            self.dirty = true;
        }
        let hint = t("O primeiro toque só acende a tela, sem disparar a tecla.");
        ui.label(RichText::new(hint).size(text::CAPTION).color(pal.muted));
    }

    fn about_section(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        widgets::section(ui, "Versão", pal);
        ui.label(tf("Keyforge {}", &[&env!("CARGO_PKG_VERSION")]));
        widgets::section(ui, "Diagnóstico", pal);
        let kind = widgets::ButtonKind::Secondary;
        if widgets::button(
            ui,
            Some("folder-open"),
            "Abrir pasta de logs",
            kind,
            true,
            0.0,
            pal,
        )
        .clicked()
        {
            open_log_folder();
        }
    }

    /// Reconectar e o autoteste ficam com o aparelho, onde se procura por eles.
    fn device_tools(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        widgets::section(ui, "Problemas com o aparelho", pal);
        let kind = widgets::ButtonKind::Secondary;
        if widgets::button(ui, Some("refresh-cw"), "Reconectar", kind, true, 0.0, pal).clicked() {
            let _ = shell::send("reconnect");
        }
        if widgets::button(
            ui,
            Some("lucide:activity"),
            "Testar aparelho",
            kind,
            true,
            0.0,
            pal,
        )
        .on_hover_text(t(SELFTEST_HINT))
        .clicked()
        {
            let reply = shell::send("selftest");
            self.toasts
                .push(selftest_reply_text(reply.as_deref()), None);
        }
    }
}
