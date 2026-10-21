//! Lista de programas instalados e extração de ícones em segundo plano.
use crate::apps;
use crate::config::schema::{ButtonAction, ButtonConfig, BLANK_BACKGROUND};
use crate::strings::t;
use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Lista de programas instalados: a busca roda em segundo plano na primeira abertura.
#[derive(Default)]
pub struct AppPicker {
    pub open: bool,
    pub apps: Option<Vec<apps::InstalledApp>>,
    pub scan: Option<Receiver<Vec<apps::InstalledApp>>>,
    pub query: String,
    pub at: ButtonSlot,
    pub jobs: IconJobs,
}

/// Posição de um botão: (perfil, página, tecla).
pub type ButtonSlot = (usize, usize, usize);

/// Ícone extraído em segundo plano, a aplicar ao botão que ainda abre o mesmo programa.
pub struct IconDone {
    pub at: ButtonSlot,
    pub action: ButtonAction,
    pub icon: String,
}

pub struct IconJobs {
    pub tx: Sender<IconDone>,
    pub rx: Receiver<IconDone>,
}

impl Default for IconJobs {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx }
    }
}

impl AppPicker {
    pub fn show(&mut self) {
        self.open = true;
        if self.apps.is_some() || self.scan.is_some() {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(apps::scan());
        });
        self.scan = Some(rx);
    }

    pub fn poll(&mut self) {
        let Some(rx) = &self.scan else { return };
        if let Ok(list) = rx.try_recv() {
            self.apps = Some(list);
            self.scan = None;
        }
    }
}

/// Aponta o botão para o programa e busca o ícone dele em segundo plano (sem pedir caminho ao usuário).
/// O botão ganha o nome do programa como rótulo e fundo preto; o ícone do programa chega depois.
pub fn apply_app(b: &mut ButtonConfig, app: &apps::InstalledApp, at: ButtonSlot, jobs: &IconJobs) {
    let (path, args) = app.launch();
    b.action = ButtonAction::LaunchApp { path, args };
    b.appearance.text = app.name.clone();
    b.appearance.background = BLANK_BACKGROUND;
    b.appearance.icon = None;
    b.appearance.icon_path = None;
    let (tx, app, action) = (jobs.tx.clone(), app.clone(), b.action.clone());
    std::thread::spawn(move || {
        if let Some(icon) = apps::extract_icon(&app) {
            let _ = tx.send(IconDone { at, action, icon });
        }
    });
}

pub fn app_buttons(ui: &mut egui::Ui, b: &mut ButtonConfig, picker: &mut AppPicker) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        if ui.button(t("Escolher app instalado")).clicked() {
            picker.show();
        }
        if ui.button(t("Procurar .exe")).clicked() {
            let file = rfd::FileDialog::new()
                .add_filter("Programa", &["exe"])
                .pick_file();
            if let Some(p) = file {
                let name = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                apply_app(
                    b,
                    &apps::InstalledApp::exe(&name, &p.display().to_string()),
                    picker.at,
                    &picker.jobs,
                );
                changed = true;
            }
        }
    });
    changed
}
