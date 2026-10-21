//! Página de mixer de volume no aparelho. Cada app com som ocupa uma coluna (volume +, ícone com a
//! porcentagem que muda o mudo ao apertar, volume −); a última coluna tem "mais apps", o mudo geral e "voltar".
//! Esta parte é pura (sem áudio nem USB), por isso é testada sem o aparelho.
use crate::apps::{extract_icon, InstalledApp};
use crate::audio::{self, AudioSession, MasterVolume};
use crate::config::schema::{ButtonConfig, RGBAColor, BLANK_BACKGROUND};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const ROWS: usize = 3;
const COLS: usize = 5;
pub const KEY_COUNT: usize = ROWS * COLS;
/// Colunas de apps; a última coluna é de controles.
const APP_COLUMNS: usize = COLS - 1;
const CONTROL_COLUMN: usize = COLS - 1;
/// Passo de cada aperto em +/−, em fração do volume (5%).
pub const VOLUME_STEP: f32 = 0.05;

const TILE: RGBAColor = RGBAColor {
    r: 16,
    g: 24,
    b: 44,
    a: 255,
};
const MUTED: RGBAColor = RGBAColor {
    r: 130,
    g: 24,
    b: 24,
    a: 255,
};

/// Um app na grade, com o ícone do programa quando já foi extraído.
#[derive(Clone, Debug, PartialEq)]
pub struct MixerApp {
    pub session: AudioSession,
    pub icon_path: Option<String>,
}

/// O que ler do sistema para montar a página.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixerData {
    pub apps: Vec<MixerApp>,
    pub master: Option<MasterVolume>,
}

/// Ícones já procurados por programa (inclusive os que falharam): extrair chama o PowerShell,
/// então cada programa é tentado uma única vez em vez de a cada leitura do mixer.
fn icon_for(session: &AudioSession) -> Option<String> {
    static TRIED: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let path = session.exe_path.as_deref()?;
    let tried = TRIED.get_or_init(Default::default);
    if let Some(known) = tried.lock().unwrap_or_else(|e| e.into_inner()).get(path) {
        return known.clone();
    }
    let icon = extract_icon(&InstalledApp::exe(&session.name, path));
    tried
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(path.to_string(), icon.clone());
    icon
}

impl MixerData {
    /// Lê o áudio e extrai os ícones (lento: faça fora do lock do estado).
    pub fn load() -> Self {
        let apps = audio::list_sessions()
            .into_iter()
            .map(|session| {
                let icon_path = icon_for(&session);
                MixerApp { session, icon_path }
            })
            .collect();
        Self {
            apps,
            master: audio::master().ok(),
        }
    }
}

/// O que fazer no sistema depois de um aperto (o estado da página já foi atualizado).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MixerEffect {
    None,
    Close,
    /// `unmute`: o app estava no mudo e mexer no volume também o tira dele.
    SetVolume {
        pid: u32,
        volume: f32,
        unmute: bool,
    },
    SetMute {
        pid: u32,
        muted: bool,
    },
    SetMasterMute {
        muted: bool,
    },
}

impl MixerEffect {
    /// Aplica a mudança no Windows; erros só vão para o log (o app pode ter fechado).
    pub fn apply(self) {
        let result = match self {
            Self::None | Self::Close => return,
            Self::SetVolume {
                pid,
                volume,
                unmute,
            } => audio::set_session_volume(pid, volume).and_then(|()| {
                if unmute {
                    audio::set_session_mute(pid, false)
                } else {
                    Ok(())
                }
            }),
            Self::SetMute { pid, muted } => audio::set_session_mute(pid, muted),
            Self::SetMasterMute { muted } => audio::set_master_mute(muted),
        };
        if let Err(e) = result {
            log::warn!("mixer: {self:?} falhou: {e}");
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MixerView {
    data: MixerData,
    /// Primeiro app visível (a página de apps avança de `APP_COLUMNS` em `APP_COLUMNS`).
    first_app: usize,
}

impl MixerView {
    pub fn new(data: MixerData) -> Self {
        Self { data, first_app: 0 }
    }

    /// Troca os dados lidos do sistema; devolve `true` se a página mudou e precisa ser redesenhada.
    pub fn update(&mut self, data: MixerData) -> bool {
        if data == self.data {
            return false;
        }
        self.data = data;
        if self.first_app >= self.data.apps.len() {
            self.first_app = 0;
        }
        true
    }

    fn app_at(&self, column: usize) -> Option<&MixerApp> {
        self.data.apps.get(self.first_app + column)
    }

    fn has_more_apps(&self) -> bool {
        self.data.apps.len() > APP_COLUMNS
    }

    /// As 15 teclas na ordem da grade (linha por linha).
    pub fn buttons(&self) -> Vec<ButtonConfig> {
        (0..KEY_COUNT)
            .map(|index| self.button(index / COLS, index % COLS))
            .collect()
    }

    fn button(&self, row: usize, column: usize) -> ButtonConfig {
        if column == CONTROL_COLUMN {
            return self.control_button(row);
        }
        let Some(app) = self.app_at(column) else {
            return ButtonConfig::blank();
        };
        match row {
            0 => key("volup", "", BLANK_BACKGROUND),
            1 => tile(app),
            _ => key("voldown", "", BLANK_BACKGROUND),
        }
    }

    fn control_button(&self, row: usize) -> ButtonConfig {
        match row {
            0 if self.has_more_apps() => key("next", "", BLANK_BACKGROUND),
            1 => match self.data.master {
                Some(m) => key(
                    "mute",
                    &percent_text(m.volume, m.muted),
                    background(m.muted),
                ),
                None => ButtonConfig::blank(),
            },
            2 => key("prev", "", BLANK_BACKGROUND),
            _ => ButtonConfig::blank(),
        }
    }

    /// Trata o aperto da tecla `index`, atualiza a página e diz o que mudar no sistema.
    pub fn press(&mut self, index: usize) -> MixerEffect {
        if index >= KEY_COUNT {
            return MixerEffect::None;
        }
        let (row, column) = (index / COLS, index % COLS);
        if column == CONTROL_COLUMN {
            return self.press_control(row);
        }
        let slot = self.first_app + column;
        let Some(app) = self.data.apps.get_mut(slot) else {
            return MixerEffect::None;
        };
        let pid = app.session.pid;
        match row {
            0 => step(&mut app.session, VOLUME_STEP),
            1 => {
                app.session.muted = !app.session.muted;
                MixerEffect::SetMute {
                    pid,
                    muted: app.session.muted,
                }
            }
            _ => step(&mut app.session, -VOLUME_STEP),
        }
    }

    fn press_control(&mut self, row: usize) -> MixerEffect {
        match row {
            0 if self.has_more_apps() => {
                let next = self.first_app + APP_COLUMNS;
                self.first_app = if next >= self.data.apps.len() {
                    0
                } else {
                    next
                };
                MixerEffect::None
            }
            1 => match self.data.master.as_mut() {
                Some(m) => {
                    m.muted = !m.muted;
                    MixerEffect::SetMasterMute { muted: m.muted }
                }
                None => MixerEffect::None,
            },
            2 => MixerEffect::Close,
            _ => MixerEffect::None,
        }
    }
}

/// Soma `delta` ao volume (sem passar de 0% nem de 100%); mexer no volume tira o app do mudo.
fn step(session: &mut AudioSession, delta: f32) -> MixerEffect {
    let volume = (session.volume + delta).clamp(0.0, 1.0);
    let unmute = session.muted;
    session.volume = volume;
    session.muted = false;
    MixerEffect::SetVolume {
        pid: session.pid,
        volume,
        unmute,
    }
}

fn background(muted: bool) -> RGBAColor {
    if muted {
        MUTED
    } else {
        TILE
    }
}

fn percent_text(volume: f32, muted: bool) -> String {
    if muted {
        "MUDO".into()
    } else {
        format!("{}%", (volume * 100.0).round() as u32)
    }
}

fn key(icon: &str, text: &str, background: RGBAColor) -> ButtonConfig {
    let mut b = ButtonConfig::blank();
    b.appearance.icon = Some(icon.into());
    b.appearance.text = text.into();
    b.appearance.background = background;
    b
}

fn tile(app: &MixerApp) -> ButtonConfig {
    let mut b = key(
        "app",
        &percent_text(app.session.volume, app.session.muted),
        background(app.session.muted),
    );
    if let Some(path) = &app.icon_path {
        b.appearance.icon = None;
        b.appearance.icon_path = Some(path.clone());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(pid: u32, volume: f32) -> MixerApp {
        MixerApp {
            session: AudioSession {
                pid,
                name: format!("App{pid}"),
                exe_path: None,
                volume,
                muted: false,
            },
            icon_path: None,
        }
    }

    fn view(apps: usize) -> MixerView {
        MixerView::new(MixerData {
            apps: (1..=apps as u32).map(|p| app(p, 0.5)).collect(),
            master: Some(MasterVolume {
                volume: 0.8,
                muted: false,
            }),
        })
    }

    fn volume_of(effect: MixerEffect) -> f32 {
        match effect {
            MixerEffect::SetVolume { volume, .. } => volume,
            other => panic!("esperava SetVolume, veio {other:?}"),
        }
    }

    #[test]
    fn page_has_one_button_per_key() {
        assert_eq!(view(2).buttons().len(), KEY_COUNT);
    }

    #[test]
    fn volume_keys_move_by_one_step() {
        let mut v = view(1);
        assert!((volume_of(v.press(0)) - 0.55).abs() < 1e-6);
        assert!((volume_of(v.press(10)) - 0.50).abs() < 1e-6);
    }

    #[test]
    fn volume_never_leaves_zero_to_one() {
        let mut loud = view(1);
        loud.data.apps[0].session.volume = 1.0;
        assert_eq!(volume_of(loud.press(0)), 1.0);
        let mut quiet = view(1);
        quiet.data.apps[0].session.volume = 0.0;
        assert_eq!(volume_of(quiet.press(10)), 0.0);
    }

    #[test]
    fn tile_toggles_mute_and_shows_it() {
        let mut v = view(1);
        assert_eq!(
            v.press(5),
            MixerEffect::SetMute {
                pid: 1,
                muted: true
            }
        );
        assert_eq!(v.buttons()[5].appearance.text, "MUDO");
        assert_eq!(
            v.press(5),
            MixerEffect::SetMute {
                pid: 1,
                muted: false
            }
        );
        assert_eq!(v.buttons()[5].appearance.text, "50%");
    }

    #[test]
    fn changing_volume_unmutes_the_app_in_the_page_and_in_the_system() {
        let mut v = view(1);
        v.press(5);
        let effect = v.press(0);
        assert!(!v.data.apps[0].session.muted);
        assert!(matches!(
            effect,
            MixerEffect::SetVolume { unmute: true, .. }
        ));
        assert!(matches!(
            v.press(0),
            MixerEffect::SetVolume { unmute: false, .. }
        ));
    }

    #[test]
    fn empty_slots_do_nothing() {
        let mut v = view(1);
        for index in [1, 6, 11, 2, 7, 12, 3, 8, 13] {
            assert_eq!(v.press(index), MixerEffect::None);
        }
    }

    #[test]
    fn back_key_closes_the_mixer() {
        assert_eq!(view(0).press(14), MixerEffect::Close);
    }

    #[test]
    fn master_mute_toggles() {
        let mut v = view(0);
        assert_eq!(v.press(9), MixerEffect::SetMasterMute { muted: true });
        assert_eq!(v.buttons()[9].appearance.text, "MUDO");
    }

    #[test]
    fn more_apps_key_pages_through_and_wraps() {
        let mut v = view(6);
        assert_eq!(v.app_at(0).map(|a| a.session.pid), Some(1));
        v.press(4);
        assert_eq!(v.app_at(0).map(|a| a.session.pid), Some(5));
        assert!(v.app_at(2).is_none());
        v.press(4);
        assert_eq!(v.app_at(0).map(|a| a.session.pid), Some(1));
    }

    #[test]
    fn more_apps_key_is_hidden_when_everything_fits() {
        let mut v = view(4);
        assert_eq!(v.buttons()[4], ButtonConfig::blank());
        v.press(4);
        assert_eq!(v.first_app, 0);
    }

    #[test]
    fn update_reports_changes_and_fixes_the_page() {
        let mut v = view(6);
        v.press(4);
        assert!(!v.update(v.data.clone()));
        assert!(v.update(view(2).data));
        assert_eq!(v.first_app, 0);
    }

    #[test]
    fn muted_tile_is_red_and_app_without_icon_uses_the_builtin_one() {
        let mut v = view(1);
        v.press(5);
        let tile = &v.buttons()[5];
        assert_eq!(tile.appearance.background, MUTED);
        assert_eq!(tile.appearance.icon.as_deref(), Some("app"));
    }
}
