//! Página de mixer de volume no aparelho. Cada coluna é um app: em cima o ícone com nome e porcentagem
//! (apertar muta), no meio volume + e embaixo volume −. A tecla que abriu o mixer vira "sair/mais".
//! Esta parte é pura (sem áudio nem USB), por isso é testada sem o aparelho.
use crate::apps::{extract_icon, InstalledApp};
use crate::audio::{self, AudioSession};
use crate::config::schema::RGBAColor;
use crate::strings::t;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const ROWS: usize = 3;
const COLS: usize = 5;
pub const KEY_COUNT: usize = ROWS * COLS;
/// Um app por coluna.
const APPS_PER_PAGE: usize = COLS;
/// Passo de cada aperto em +/−, em fração do volume (5%).
pub const VOLUME_STEP: f32 = 0.05;
/// Segurar a tecla "sair/mais" por este tempo fecha o mixer mesmo com mais apps para ver.
const EXIT_HOLD: Duration = Duration::from_millis(500);

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
        Self { apps }
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
        };
        if let Err(e) = result {
            log::warn!("mixer: {self:?} falhou: {e}");
        }
    }
}

/// O que uma tecla mostra; o desenho fica em `graphics::mixer_art`.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyFace {
    Empty,
    Plus,
    Minus,
    App {
        name: String,
        icon_path: Option<String>,
    },
    /// A tecla que abriu o mixer: "mais apps" se houver outra página, senão "sair".
    Home {
        more: bool,
    },
}

/// Uma tecla da página: o que mostrar, o texto de valor (porcentagem) e o trecho do medidor.
#[derive(Clone, Debug, PartialEq)]
pub struct MixerKey {
    pub face: KeyFace,
    /// Nome curto da tecla "sair/mais"; vazio nas demais.
    pub label: String,
    /// Dica em letras pequenas abaixo do nome (como sair com o toque longo); pode ser vazia.
    pub hint: String,
    /// Porcentagem de volume, ex.: "62%"; vazio nas teclas sem valor.
    pub value: String,
    pub muted: bool,
    /// Quanto deste trecho do medidor está cheio (0 a 1); `None` onde a coluna não tem app.
    pub meter: Option<f32>,
    pub accent: RGBAColor,
}

impl MixerKey {
    fn plain(face: KeyFace, accent: RGBAColor) -> Self {
        Self {
            face,
            label: String::new(),
            hint: String::new(),
            value: String::new(),
            muted: false,
            meter: None,
            accent,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MixerView {
    data: MixerData,
    /// Primeiro app visível (a página avança de `APPS_PER_PAGE` em `APPS_PER_PAGE`).
    first_app: usize,
    /// Tecla que abriu o mixer; ela vira "sair/mais".
    home: usize,
}

impl MixerView {
    pub fn new(data: MixerData, home: usize) -> Self {
        Self {
            data,
            first_app: 0,
            home: home.min(KEY_COUNT - 1),
        }
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
        self.data.apps.len() > APPS_PER_PAGE
    }

    /// As 15 teclas na ordem da grade (linha por linha); `accent` é a cor de destaque da interface.
    pub fn keys(&self, accent: RGBAColor) -> Vec<MixerKey> {
        (0..KEY_COUNT)
            .map(|index| {
                let key = self.key_at(index / COLS, index % COLS, accent);
                if index == self.home {
                    self.home_key(key.meter, accent)
                } else {
                    key
                }
            })
            .collect()
    }

    fn key_at(&self, row: usize, column: usize, accent: RGBAColor) -> MixerKey {
        let Some(app) = self.app_at(column) else {
            return MixerKey::plain(KeyFace::Empty, accent);
        };
        let session = &app.session;
        let face = match row {
            0 => KeyFace::App {
                name: session.name.clone(),
                icon_path: app.icon_path.clone(),
            },
            1 => KeyFace::Plus,
            _ => KeyFace::Minus,
        };
        MixerKey {
            value: if row == 0 {
                percent_text(session.volume)
            } else {
                String::new()
            },
            muted: session.muted,
            meter: Some(meter_slice(session.volume, row)),
            ..MixerKey::plain(face, accent)
        }
    }

    /// A tecla "sair/mais" mantém o trecho do medidor da coluna onde está.
    fn home_key(&self, meter: Option<f32>, accent: RGBAColor) -> MixerKey {
        let more = self.has_more_apps();
        MixerKey {
            label: t(if more { "MAIS" } else { "SAIR" }).to_string(),
            hint: if more {
                t("SEGURE: SAIR").to_string()
            } else {
                String::new()
            },
            meter,
            ..MixerKey::plain(KeyFace::Home { more }, accent)
        }
    }

    /// Trata o aperto da tecla `index` (solta após `held`), atualiza a página e diz o que mudar no sistema.
    pub fn press(&mut self, index: usize, held: Duration) -> MixerEffect {
        if index >= KEY_COUNT {
            return MixerEffect::None;
        }
        if index == self.home {
            return self.press_home(held);
        }
        let (row, column) = (index / COLS, index % COLS);
        let slot = self.first_app + column;
        let Some(app) = self.data.apps.get_mut(slot) else {
            return MixerEffect::None;
        };
        match row {
            0 => {
                app.session.muted = !app.session.muted;
                MixerEffect::SetMute {
                    pid: app.session.pid,
                    muted: app.session.muted,
                }
            }
            1 => step(&mut app.session, VOLUME_STEP),
            _ => step(&mut app.session, -VOLUME_STEP),
        }
    }

    /// Toque: próxima página de apps (ou sair, se tudo cabe numa página). Toque longo: sempre sai.
    fn press_home(&mut self, held: Duration) -> MixerEffect {
        if held >= EXIT_HOLD || !self.has_more_apps() {
            return MixerEffect::Close;
        }
        let next = self.first_app + APPS_PER_PAGE;
        self.first_app = if next >= self.data.apps.len() {
            0
        } else {
            next
        };
        MixerEffect::None
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

fn percent_text(volume: f32) -> String {
    format!("{}%", (volume * 100.0).round() as u32)
}

/// Parte do medidor da coluna que cabe na tecla da linha `row` (0 = de cima): o nível sobe de baixo
/// para cima atravessando as três teclas, como uma barra só.
fn meter_slice(volume: f32, row: usize) -> f32 {
    let keys_below = (ROWS - 1 - row) as f32;
    (volume.clamp(0.0, 1.0) * ROWS as f32 - keys_below).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENT: RGBAColor = RGBAColor {
        r: 0,
        g: 229,
        b: 255,
        a: 255,
    };
    const TAP: Duration = Duration::from_millis(50);
    const HOME: usize = 14;

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
        MixerView::new(
            MixerData {
                apps: (1..=apps as u32).map(|p| app(p, 0.5)).collect(),
            },
            HOME,
        )
    }

    fn volume_of(effect: MixerEffect) -> f32 {
        match effect {
            MixerEffect::SetVolume { volume, .. } => volume,
            other => panic!("esperava SetVolume, veio {other:?}"),
        }
    }

    #[test]
    fn page_has_one_key_per_button() {
        assert_eq!(view(2).keys(ACCENT).len(), KEY_COUNT);
    }

    #[test]
    fn top_row_shows_each_app_with_name_and_percentage() {
        let mut v = view(5);
        v.data.apps[0].session.name = "Spotify".into();
        v.data.apps[0].session.volume = 0.62;
        let keys = v.keys(ACCENT);
        assert_eq!(keys[0].value, "62%");
        assert!(matches!(&keys[0].face, KeyFace::App { name, .. } if name == "Spotify"));
        for column in 0..COLS {
            assert!(matches!(keys[column].face, KeyFace::App { .. }));
            assert_eq!(keys[COLS + column].face, KeyFace::Plus);
        }
    }

    #[test]
    fn middle_row_raises_and_bottom_row_lowers() {
        let mut v = view(1);
        assert!((volume_of(v.press(5, TAP)) - 0.55).abs() < 1e-6);
        assert!((volume_of(v.press(10, TAP)) - 0.50).abs() < 1e-6);
    }

    #[test]
    fn volume_never_leaves_zero_to_one() {
        let mut loud = view(1);
        loud.data.apps[0].session.volume = 1.0;
        assert_eq!(volume_of(loud.press(5, TAP)), 1.0);
        let mut quiet = view(1);
        quiet.data.apps[0].session.volume = 0.0;
        assert_eq!(volume_of(quiet.press(10, TAP)), 0.0);
    }

    #[test]
    fn app_key_toggles_mute_and_keeps_the_percentage() {
        let mut v = view(1);
        assert_eq!(
            v.press(0, TAP),
            MixerEffect::SetMute {
                pid: 1,
                muted: true
            }
        );
        let tile = &v.keys(ACCENT)[0];
        assert!(tile.muted);
        assert_eq!(tile.value, "50%");
        assert_eq!(
            v.press(0, TAP),
            MixerEffect::SetMute {
                pid: 1,
                muted: false
            }
        );
    }

    #[test]
    fn changing_volume_unmutes_the_app_in_the_page_and_in_the_system() {
        let mut v = view(1);
        v.press(0, TAP);
        let effect = v.press(5, TAP);
        assert!(!v.data.apps[0].session.muted);
        assert!(matches!(
            effect,
            MixerEffect::SetVolume { unmute: true, .. }
        ));
        assert!(matches!(
            v.press(5, TAP),
            MixerEffect::SetVolume { unmute: false, .. }
        ));
    }

    #[test]
    fn meter_runs_across_the_three_keys_of_a_column() {
        assert_eq!(meter_slice(0.0, 2), 0.0);
        assert_eq!(meter_slice(1.0, 0), 1.0);
        assert_eq!(meter_slice(0.5, 2), 1.0);
        assert!((meter_slice(0.5, 1) - 0.5).abs() < 1e-6);
        assert_eq!(meter_slice(0.5, 0), 0.0);
    }

    #[test]
    fn only_columns_with_an_app_have_a_meter() {
        let keys = view(1).keys(ACCENT);
        assert!(keys[0].meter.is_some() && keys[5].meter.is_some() && keys[10].meter.is_some());
        assert!(keys[1].meter.is_none() && keys[11].meter.is_none());
    }

    #[test]
    fn empty_slots_do_nothing() {
        let mut v = view(1);
        for index in [1, 6, 11, 2, 7, 12, 3, 8, 13, 4, 9] {
            assert_eq!(v.press(index, TAP), MixerEffect::None);
        }
    }

    #[test]
    fn the_key_that_opened_the_mixer_becomes_the_home_key() {
        let v = MixerView::new(MixerData::default(), 7);
        let keys = v.keys(ACCENT);
        assert!(matches!(keys[7].face, KeyFace::Home { more: false }));
        let homes = keys
            .iter()
            .filter(|k| matches!(k.face, KeyFace::Home { .. }))
            .count();
        assert_eq!(homes, 1);
    }

    #[test]
    fn home_key_outside_the_grid_falls_back_to_the_last_key() {
        let v = MixerView::new(MixerData::default(), 40);
        assert!(matches!(
            v.keys(ACCENT)[KEY_COUNT - 1].face,
            KeyFace::Home { .. }
        ));
    }

    #[test]
    fn home_key_keeps_the_meter_of_its_column() {
        assert!(view(5).keys(ACCENT)[HOME].meter.is_some());
        assert!(view(1).keys(ACCENT)[HOME].meter.is_none());
    }

    #[test]
    fn tapping_home_exits_when_everything_fits_on_one_page() {
        assert_eq!(view(5).press(HOME, TAP), MixerEffect::Close);
        assert_eq!(view(0).press(HOME, TAP), MixerEffect::Close);
    }

    #[test]
    fn tapping_home_pages_through_apps_and_wraps() {
        let mut v = view(7);
        assert_eq!(v.app_at(0).map(|a| a.session.pid), Some(1));
        assert_eq!(v.press(HOME, TAP), MixerEffect::None);
        assert_eq!(v.app_at(0).map(|a| a.session.pid), Some(6));
        assert!(v.app_at(2).is_none());
        v.press(HOME, TAP);
        assert_eq!(v.app_at(0).map(|a| a.session.pid), Some(1));
    }

    #[test]
    fn holding_home_always_exits() {
        assert_eq!(view(7).press(HOME, EXIT_HOLD), MixerEffect::Close);
    }

    #[test]
    fn home_label_says_what_a_tap_does() {
        let more = &view(7).keys(ACCENT)[HOME];
        assert_eq!(more.label, "MAIS");
        assert!(!more.hint.is_empty());
        let exit = &view(2).keys(ACCENT)[HOME];
        assert_eq!(exit.label, "SAIR");
        assert!(exit.hint.is_empty());
    }

    #[test]
    fn update_reports_changes_and_fixes_the_page() {
        let mut v = view(7);
        v.press(HOME, TAP);
        assert!(!v.update(v.data.clone()));
        assert!(v.update(view(2).data));
        assert_eq!(v.first_app, 0);
    }

    /// Gera uma imagem das 15 teclas para conferir o visual:
    /// `PREVIEW_OUT=pasta/mixer.png cargo test mixer_preview -- --ignored`.
    #[test]
    #[ignore]
    fn mixer_preview() {
        let mut v = view(3);
        v.data.apps[0].session.name = "Spotify".into();
        v.data.apps[0].session.volume = 0.62;
        v.data.apps[1].session.name = "Google Chrome".into();
        v.data.apps[1].session.volume = 0.3;
        v.data.apps[2].session.name = "Discord".into();
        v.data.apps[2].session.volume = 1.0;
        v.press(1, TAP);
        let px = 85u32;
        let mut sheet = image::RgbaImage::new((px + 6) * 5, (px + 6) * 3);
        for (i, key) in v.keys(ACCENT).iter().enumerate() {
            let img = crate::graphics::mixer_art::render_mixer_key(key, px);
            let (x, y) = ((i % COLS) as u32 * (px + 6), (i / COLS) as u32 * (px + 6));
            image::imageops::overlay(&mut sheet, &img, x as i64, y as i64);
        }
        sheet
            .save(std::env::var("PREVIEW_OUT").expect("defina PREVIEW_OUT"))
            .unwrap();
    }
}
