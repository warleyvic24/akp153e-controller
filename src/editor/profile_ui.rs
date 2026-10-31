//! Opções do perfil no menu "Perfil e página": troca automática pelo app em foco e exportar/importar.
use super::{tokens, App};
use crate::config::profile_io;
use crate::config::schema::AppConfig;
use crate::editor::theme::Palette;
use crate::platform::shell;
use crate::service::runtime;
use crate::strings::{t, tf};
use eframe::egui::{self, RichText};
use std::time::{Duration, Instant};

/// De quanto em quanto tempo o menu aberto pergunta ao serviço os apps recentes.
const RECENT_REFRESH: Duration = Duration::from_secs(2);
const RULE_FIELD_WIDTH: f32 = 220.0;

/// Apps recentes vindos do serviço (`apps`), com a hora da última consulta.
#[derive(Default)]
pub(crate) struct RecentApps {
    asked: Option<Instant>,
    apps: Vec<String>,
}

impl RecentApps {
    fn refresh(&mut self) -> &[String] {
        if self.asked.is_none_or(|at| at.elapsed() >= RECENT_REFRESH) {
            self.asked = Some(Instant::now());
            self.apps = shell::send("apps")
                .map(|reply| {
                    reply
                        .split('\t')
                        .map(str::trim)
                        .filter(|a| !a.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
        }
        &self.apps
    }
}

/// Acrescenta `app` à regra (lista separada por vírgula), sem repetir.
fn add_to_rule(rule: &str, app: &str) -> String {
    let mut items: Vec<&str> = rule
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .collect();
    if !items.iter().any(|a| a.eq_ignore_ascii_case(app)) {
        items.push(app);
    }
    items.join(", ")
}

/// A troca automática funciona nesta sessão? (Windows sempre; Linux no Hyprland, Sway e X11.)
fn focus_supported() -> bool {
    #[cfg(windows)]
    return true;
    #[cfg(not(windows))]
    return crate::platform::focus::source().is_some();
}

impl App {
    /// Campo "ativar sozinho com este app" e as sugestões de apps que estiveram em foco.
    pub(crate) fn auto_app_items(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        prof: usize,
        pal: &Palette,
    ) {
        let label = ui
            .label(t("Ativar sozinho quando este app estiver em foco"))
            .id;
        let mut rule = g.cfg.profiles[prof].auto_app.clone().unwrap_or_default();
        let field = ui.scope(|ui| {
            ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, pal.control);
            ui.add(
                egui::TextEdit::singleline(&mut rule)
                    .hint_text(super::widgets::hint("ex.: obs, firefox", pal))
                    .desired_width(RULE_FIELD_WIDTH),
            )
            .labelled_by(label)
        });
        let mut changed = field.inner.changed();
        if !focus_supported() {
            let msg = t("Esta sessão não informa a janela ativa (GNOME e KDE no Wayland): use Hyprland, Sway, X11 ou Windows.");
            ui.label(
                RichText::new(msg)
                    .size(tokens::text::CAPTION)
                    .color(pal.bad),
            );
        } else {
            let recent = self.recent_apps.refresh().to_vec();
            if !recent.is_empty() {
                let msg = t("Apps usados há pouco (clique para incluir):");
                ui.label(
                    RichText::new(msg)
                        .size(tokens::text::CAPTION)
                        .color(pal.muted),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.set_max_width(RULE_FIELD_WIDTH + 40.0);
                    for app in recent {
                        if ui.small_button(&app).clicked() {
                            rule = add_to_rule(&rule, &app);
                            changed = true;
                        }
                    }
                });
            }
        }
        if changed {
            let trimmed = rule.trim();
            g.cfg.profiles[prof].auto_app = (!trimmed.is_empty()).then(|| trimmed.to_string());
            self.touch();
        }
    }

    /// Exportar o perfil atual para um arquivo, ou importar um como perfil novo.
    pub(crate) fn profile_file_items(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        prof: usize,
    ) {
        if ui.button(t("Exportar perfil…")).clicked() {
            ui.close_menu();
            let profile = &g.cfg.profiles[prof];
            let file = rfd::FileDialog::new()
                .set_file_name(format!("{}.{}", profile.name, profile_io::EXTENSION))
                .add_filter(t("Perfil do AKP153E"), &["json"])
                .save_file();
            if let Some(path) = file {
                let msg = match std::fs::write(&path, profile_io::export(profile)) {
                    Ok(()) => tf("Perfil exportado: {}", &[&path.display()]),
                    Err(e) => tf("Não foi possível exportar: {}", &[&e]),
                };
                self.toasts.push(msg, None);
            }
        }
        if ui.button(t("Importar perfil…")).clicked() {
            ui.close_menu();
            let file = rfd::FileDialog::new()
                .add_filter(t("Perfil do AKP153E"), &["json"])
                .pick_file();
            if let Some(path) = file {
                self.import_profile(g, &path);
            }
        }
    }

    fn import_profile(&mut self, g: &mut runtime::Shared, path: &std::path::Path) {
        let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > profile_io::MAX_FILE_BYTES);
        let text = match std::fs::read_to_string(path) {
            Ok(_) if too_big => Err(t("arquivo grande demais").to_string()),
            Ok(text) => Ok(text),
            Err(e) => Err(e.to_string()),
        };
        let icons = crate::platform::icon_dir();
        let result = text.and_then(|text| {
            let icons = icons.ok_or_else(|| t("sem pasta de configuração").to_string())?;
            let mut cfg: AppConfig = g.cfg.clone();
            let index = profile_io::import(&mut cfg, &text, &icons)?;
            Ok((cfg, index))
        });
        match result {
            Ok((cfg, index)) => {
                g.cfg = cfg;
                g.cfg.active_profile = index;
                self.selected = 0;
                self.touch_nav();
                let name = g.cfg.profiles[index].name.clone();
                self.toasts
                    .push(tf("Perfil “{}” importado", &[&name]), Some("Desfazer"));
            }
            Err(e) => self
                .toasts
                .push(tf("Não foi possível importar: {}", &[&e]), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_to_a_rule_keeps_it_tidy_and_unique() {
        assert_eq!(add_to_rule("", "obs"), "obs");
        assert_eq!(add_to_rule("obs", "firefox"), "obs, firefox");
        assert_eq!(add_to_rule(" obs ,, firefox ", "OBS"), "obs, firefox");
    }
}
