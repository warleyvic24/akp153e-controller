//! Janela de Preferências: tema, destaque, início com o Windows, brilho do aparelho e diagnóstico.
use super::{widgets, App};
use crate::config::schema::{AppConfig, Language, Theme};
use crate::editor::omarchy;
use crate::editor::theme::{self, Palette};
use crate::editor::tokens::{text, TARGET_MIN};
use crate::platform::shell;
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{self, Color32, RichText};

const WINDOW_WIDTH: f32 = 360.0;
const SWATCH_GAP: f32 = 10.0;
const BRIGHTNESS_MIN: u8 = 5;
const BRIGHTNESS_MAX: u8 = crate::usb::MAX_BRIGHTNESS;
const SLIDER_VALUE_RESERVED: f32 = 72.0;

/// Abre a pasta onde ficam config e logs no gerenciador de arquivos (Explorer ou `xdg-open`).
pub(crate) fn open_log_folder() {
    let opener = if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let dir = AppConfig::path().and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    match dir {
        Some(dir) => {
            if let Err(e) = std::process::Command::new(opener).arg(&dir).spawn() {
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
        let mut open = true;
        let title = RichText::new(t("Preferências")).size(text::BODY).strong();
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(WINDOW_WIDTH)
            .show(ctx, |ui| {
                self.theme_section(ui, g, pal);
                self.language_section(ui, g, pal);
                self.startup_section(ui, pal);
                self.device_section(ui, g, pal);
                self.about_section(ui, pal);
            });
        self.settings_open = open;
    }

    fn theme_section(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let accent = theme::accent_color(&g.cfg.ui);
        widgets::section(ui, "Tema", pal);
        ui.horizontal(|ui| {
            for (value, name) in [
                (Theme::Dark, "Escuro"),
                (Theme::Light, "Claro"),
                (Theme::Omarchy, "Omarchy"),
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
        if g.cfg.ui.theme == Theme::Omarchy && omarchy::current().is_some() {
            ui.label(widgets::hint(
                t("A cor de destaque segue o tema do Omarchy"),
                pal,
            ));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = SWATCH_GAP;
            if ui.color_edit_button_srgb(&mut g.cfg.ui.accent).changed() {
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
        ui.checkbox(&mut self.autostart, t(shell::AUTOSTART_LABEL));
        if self.autostart != before {
            let on = self.autostart;
            // O registro pode demorar: fora da thread da interface, como faz a bandeja.
            std::thread::spawn(move || shell::set_autostart(on));
        }
    }

    fn device_section(&mut self, ui: &mut egui::Ui, g: &mut runtime::Shared, pal: &Palette) {
        let label = widgets::section(ui, "Brilho do aparelho", pal);
        ui.spacing_mut().slider_width =
            (ui.available_width() - SLIDER_VALUE_RESERVED).max(TARGET_MIN * 4.0);
        let slider = egui::Slider::new(&mut g.cfg.ui.brightness, BRIGHTNESS_MIN..=BRIGHTNESS_MAX)
            .suffix("%");
        if ui.add(slider).labelled_by(label).changed() {
            self.dirty = true;
        }
        let note = if g.connected {
            t("Aplica no aparelho ao salvar.")
        } else {
            t("Sem aparelho conectado: vale a partir da próxima conexão.")
        };
        ui.label(RichText::new(note).size(text::CAPTION).color(pal.muted));
    }

    fn about_section(&self, ui: &mut egui::Ui, pal: &Palette) {
        widgets::section(ui, "Diagnóstico", pal);
        ui.label(tf("Versão {}", &[&env!("CARGO_PKG_VERSION")]));
        ui.horizontal(|ui| {
            if ui.button(t("Reconectar")).clicked() {
                let _ = shell::send("reconnect");
            }
            if ui.button(t("Abrir pasta de logs")).clicked() {
                open_log_folder();
            }
        });
    }
}
