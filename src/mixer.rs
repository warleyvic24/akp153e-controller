//! Página de mixer de volume no aparelho. Cada coluna é um app: em cima o ícone com nome e porcentagem
//! (apertar muta), no meio volume + e embaixo volume − (segurar repete). A tecla de sair/mais fica
//! onde o mixer foi aberto, ou numa coluna livre quando essa já tem app.
//! Esta parte é pura (sem áudio nem USB), por isso é testada sem o aparelho.
use crate::apps::{extract_icon, InstalledApp};
use crate::audio::{self, AudioSession};
use crate::config::schema::RGBAColor;
use crate::strings::t;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
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
/// Segurando + ou −, o volume passa a repetir depois deste tempo, a cada intervalo.
pub const REPEAT_DELAY: Duration = Duration::from_millis(400);
pub const REPEAT_INTERVAL: Duration = Duration::from_millis(150);

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

/// Cada mixer aberto recebe um número próprio, que vai nos bits altos da versão: uma leitura feita
/// para um mixer que já fechou nunca é aceita por outro aberto depois.
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);
const EPOCH_SHIFT: u32 = 32;

type IconCache = Mutex<HashMap<String, Option<String>>>;

/// Ícones já procurados por programa (inclusive os que falharam): extrair chama o PowerShell,
/// então cada programa é tentado uma única vez.
fn icon_cache() -> &'static IconCache {
    static CACHE: OnceLock<IconCache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// `None` = nunca tentado; `Some(None)` = tentado e sem ícone.
fn cached_icon(path: &str) -> Option<Option<String>> {
    let cache = icon_cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.get(path).cloned()
}

impl MixerData {
    /// Lê os apps do Windows usando só ícones já extraídos (rápido).
    pub fn read() -> Self {
        let apps = audio::list_sessions()
            .into_iter()
            .map(|session| {
                let icon_path = session.exe_path.as_deref().and_then(cached_icon).flatten();
                MixerApp { session, icon_path }
            })
            .collect();
        Self { apps }
    }

    /// Extrai o ícone dos apps ainda não tentados (lento: PowerShell). Devolve se achou algum.
    pub fn fill_missing_icons(&mut self) -> bool {
        let mut found = false;
        for app in &mut self.apps {
            let Some(path) = app.session.exe_path.clone() else {
                continue;
            };
            if cached_icon(&path).is_some() {
                continue;
            }
            let icon = extract_icon(&InstalledApp::exe(&app.session.name, &path));
            found |= icon.is_some();
            icon_cache()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(path, icon.clone());
            app.icon_path = icon;
        }
        found
    }
}

/// O que fazer no sistema depois de um aperto (o estado da página já foi atualizado).
#[derive(Clone, Debug, PartialEq)]
pub enum MixerEffect {
    None,
    Close,
    /// `unmute`: o app estava no mudo e mexer no volume também o tira dele.
    SetVolume {
        pids: Vec<u32>,
        volume: f32,
        unmute: bool,
    },
    SetMute {
        pids: Vec<u32>,
        muted: bool,
    },
}

impl MixerEffect {
    /// Aplica a mudança no Windows; erros só vão para o log (o app pode ter fechado).
    pub fn apply(self) {
        let result = match &self {
            Self::None | Self::Close => return,
            Self::SetVolume {
                pids,
                volume,
                unmute,
            } => audio::set_session_volume(pids, *volume).and_then(|()| {
                if *unmute {
                    audio::set_session_mute(pids, false)
                } else {
                    Ok(())
                }
            }),
            Self::SetMute { pids, muted } => audio::set_session_mute(pids, *muted),
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
    /// A tecla de sair: "mais apps" se houver outra página, senão "sair".
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
    /// App aberto mas sem tocar agora: aparece esmaecido.
    pub inactive: bool,
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
            inactive: false,
            meter: None,
            accent,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MixerView {
    /// Apps na ordem em que apareceram: só somem se o programa fecha, e os novos entram no fim.
    apps: Vec<MixerApp>,
    /// Primeiro app visível (a página avança de `APPS_PER_PAGE` em `APPS_PER_PAGE`).
    first_app: usize,
    /// Tecla que abriu o mixer.
    opener: usize,
    /// Sobe a cada mudança feita pelo usuário; uma leitura do sistema feita antes dela é descartada
    /// para não desfazer o que o usuário acabou de fazer.
    version: u64,
    /// O que já foi enviado ao aparelho, para redesenhar só as teclas que mudaram.
    drawn: RefCell<Vec<Option<MixerKey>>>,
}

impl MixerView {
    pub fn new(opener: usize) -> Self {
        Self {
            apps: Vec::new(),
            first_app: 0,
            opener: opener.min(KEY_COUNT - 1),
            version: NEXT_EPOCH.fetch_add(1, Ordering::Relaxed) << EPOCH_SHIFT,
            drawn: RefCell::new(Vec::new()),
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Marca a página como alterada, para descartar leituras do sistema que começaram antes.
    pub fn touch(&mut self) {
        self.version += 1;
    }

    /// Integra uma leitura do sistema feita na versão `version`. Mantém a ordem dos apps já na
    /// tela, tira os que fecharam e acrescenta no fim os novos que estão tocando.
    /// Devolve `true` se a página mudou.
    pub fn update(&mut self, data: MixerData, version: u64) -> bool {
        if version != self.version {
            return false;
        }
        let mut merged: Vec<MixerApp> = self
            .apps
            .iter()
            .filter_map(|old| {
                data.apps
                    .iter()
                    .find(|new| new.session.key == old.session.key)
                    .cloned()
            })
            .collect();
        for new in data.apps {
            let known = merged.iter().any(|m| m.session.key == new.session.key);
            if new.session.active && !known {
                merged.push(new);
            }
        }
        if merged == self.apps {
            return false;
        }
        self.apps = merged;
        if self.first_app >= self.apps.len() {
            self.first_app = 0;
        }
        true
    }

    fn app_at(&self, column: usize) -> Option<&MixerApp> {
        self.apps.get(self.first_app + column)
    }

    fn has_more_apps(&self) -> bool {
        self.apps.len() > APPS_PER_PAGE
    }

    /// A tecla de sair: onde o mixer foi aberto; se essa coluna já tem app e todos cabem numa página,
    /// a mesma linha na última coluna livre, para não tomar a tecla de um app.
    fn home_index(&self) -> usize {
        let (row, column) = (self.opener / COLS, self.opener % COLS);
        if self.has_more_apps() || self.app_at(column).is_none() {
            return self.opener;
        }
        (0..COLS)
            .rev()
            .find(|&c| self.app_at(c).is_none())
            .map_or(self.opener, |free| row * COLS + free)
    }

    /// As 15 teclas na ordem da grade (linha por linha); `accent` é a cor de destaque da interface.
    pub fn keys(&self, accent: RGBAColor) -> Vec<MixerKey> {
        let home = self.home_index();
        (0..KEY_COUNT)
            .map(|index| {
                let key = self.key_at(index / COLS, index % COLS, accent);
                if index == home {
                    self.home_key(key.meter, accent)
                } else {
                    key
                }
            })
            .collect()
    }

    /// As teclas que mudaram desde o último desenho (e marca como desenhadas).
    pub fn changed_keys(&self, accent: RGBAColor) -> Vec<(usize, MixerKey)> {
        let mut drawn = self.drawn.borrow_mut();
        drawn.resize(KEY_COUNT, None);
        let mut changed = Vec::new();
        for (index, key) in self.keys(accent).into_iter().enumerate() {
            if drawn[index].as_ref() != Some(&key) {
                drawn[index] = Some(key.clone());
                changed.push((index, key));
            }
        }
        changed
    }

    /// Esquece o que foi desenhado: a próxima vez todas as teclas voltam a ser enviadas
    /// (o aparelho foi reconectado ou perdeu a imagem).
    pub fn invalidate(&self) {
        self.drawn.borrow_mut().clear();
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
            inactive: !session.active,
            meter: Some(meter_slice(session.volume, row)),
            ..MixerKey::plain(face, accent)
        }
    }

    /// A tecla de sair mantém o trecho do medidor da coluna onde está.
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

    /// O app e a linha (0 = ícone, 1 = +, 2 = −) da tecla `index`; `None` para a tecla de sair,
    /// colunas vazias e teclas fora da grade.
    fn locate(&self, index: usize) -> Option<(usize, usize)> {
        if index >= KEY_COUNT || index == self.home_index() {
            return None;
        }
        let slot = self.first_app + index % COLS;
        (slot < self.apps.len()).then_some((slot, index / COLS))
    }

    /// `true` se `index` é uma tecla de + ou − (as que repetem ao segurar).
    pub fn is_step_key(&self, index: usize) -> bool {
        self.locate(index).is_some_and(|(_, row)| row > 0)
    }

    /// Aperto da tecla: o ícone muta/desmuta e +/− mudam o volume na hora.
    pub fn press_down(&mut self, index: usize) -> MixerEffect {
        let Some((slot, row)) = self.locate(index) else {
            return MixerEffect::None;
        };
        self.version += 1;
        let session = &mut self.apps[slot].session;
        match row {
            0 => {
                session.muted = !session.muted;
                MixerEffect::SetMute {
                    pids: session.pids.clone(),
                    muted: session.muted,
                }
            }
            1 => step(session, VOLUME_STEP),
            _ => step(session, -VOLUME_STEP),
        }
    }

    /// Repetição de + ou − com a tecla segurada; as demais teclas não repetem.
    pub fn repeat(&mut self, index: usize) -> MixerEffect {
        match self.locate(index) {
            Some((_, row)) if row > 0 => self.press_down(index),
            _ => MixerEffect::None,
        }
    }

    /// Soltura da tecla: só a de sair age aqui (toque ou toque longo).
    pub fn release(&mut self, index: usize, held: Duration) -> MixerEffect {
        if index != self.home_index() {
            return MixerEffect::None;
        }
        if held >= EXIT_HOLD || !self.has_more_apps() {
            return MixerEffect::Close;
        }
        self.version += 1;
        let next = self.first_app + APPS_PER_PAGE;
        self.first_app = if next >= self.apps.len() { 0 } else { next };
        MixerEffect::None
    }
}

/// Soma `delta` ao volume (sem passar de 0% nem de 100%); mexer no volume tira o app do mudo.
/// Sem efeito se nada muda (já no limite e sem mudo), para não chamar o Windows à toa.
fn step(session: &mut AudioSession, delta: f32) -> MixerEffect {
    let volume = (session.volume + delta).clamp(0.0, 1.0);
    let unmute = session.muted;
    if volume == session.volume && !unmute {
        return MixerEffect::None;
    }
    session.volume = volume;
    session.muted = false;
    MixerEffect::SetVolume {
        pids: session.pids.clone(),
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

    fn app(n: u32, volume: f32, active: bool) -> MixerApp {
        MixerApp {
            session: AudioSession {
                key: format!("app{n}"),
                name: format!("App{n}"),
                exe_path: None,
                pids: vec![n],
                volume,
                muted: false,
                active,
            },
            icon_path: None,
        }
    }

    /// Entrega uma leitura do sistema ao mixer como se tivesse sido feita agora.
    fn load(v: &mut MixerView, d: MixerData) -> bool {
        let version = v.version();
        v.update(d, version)
    }

    fn data(apps: usize) -> MixerData {
        MixerData {
            apps: (1..=apps as u32).map(|n| app(n, 0.5, true)).collect(),
        }
    }

    fn view_at(apps: usize, opener: usize) -> MixerView {
        let mut v = MixerView::new(opener);
        load(&mut v, data(apps));
        v
    }

    fn view(apps: usize) -> MixerView {
        view_at(apps, HOME)
    }

    fn volume_of(effect: MixerEffect) -> f32 {
        match effect {
            MixerEffect::SetVolume { volume, .. } => volume,
            other => panic!("esperava SetVolume, veio {other:?}"),
        }
    }

    fn keys(v: &MixerView) -> Vec<MixerKey> {
        v.keys(ACCENT)
    }

    #[test]
    fn page_has_one_key_per_button() {
        assert_eq!(keys(&view(2)).len(), KEY_COUNT);
    }

    #[test]
    fn top_row_shows_each_app_with_name_and_percentage() {
        let mut v = MixerView::new(HOME);
        let mut d = data(5);
        d.apps[0].session.name = "Spotify".into();
        d.apps[0].session.volume = 0.62;
        load(&mut v, d);
        let k = keys(&v);
        assert_eq!(k[0].value, "62%");
        assert!(matches!(&k[0].face, KeyFace::App { name, .. } if name == "Spotify"));
        for column in 0..COLS {
            assert!(matches!(k[column].face, KeyFace::App { .. }));
            assert_eq!(k[COLS + column].face, KeyFace::Plus);
        }
    }

    #[test]
    fn middle_row_raises_and_bottom_row_lowers_on_press() {
        let mut v = view(1);
        assert!((volume_of(v.press_down(5)) - 0.55).abs() < 1e-6);
        assert!((volume_of(v.press_down(10)) - 0.50).abs() < 1e-6);
    }

    #[test]
    fn volume_stops_at_the_limits_without_calling_windows() {
        let mut v = MixerView::new(HOME);
        load(
            &mut v,
            MixerData {
                apps: vec![app(1, 1.0, true), app(2, 0.0, true)],
            },
        );
        assert_eq!(v.press_down(5), MixerEffect::None);
        assert_eq!(v.press_down(11), MixerEffect::None);
        assert!(volume_of(v.press_down(6)) > 0.0);
    }

    #[test]
    fn effects_carry_every_process_of_the_program() {
        let mut v = MixerView::new(HOME);
        let mut d = data(1);
        d.apps[0].session.pids = vec![7, 8, 9];
        load(&mut v, d);
        assert!(
            matches!(v.press_down(5), MixerEffect::SetVolume { pids, .. } if pids == [7, 8, 9])
        );
        assert!(matches!(v.press_down(0), MixerEffect::SetMute { pids, .. } if pids == [7, 8, 9]));
    }

    #[test]
    fn app_key_toggles_mute_and_keeps_the_percentage() {
        let mut v = view(1);
        assert!(matches!(
            v.press_down(0),
            MixerEffect::SetMute { muted: true, .. }
        ));
        let tile = &keys(&v)[0];
        assert!(tile.muted);
        assert_eq!(tile.value, "50%");
        assert!(matches!(
            v.press_down(0),
            MixerEffect::SetMute { muted: false, .. }
        ));
    }

    #[test]
    fn changing_volume_unmutes_the_app_in_the_page_and_in_the_system() {
        let mut v = view(1);
        v.press_down(0);
        let effect = v.press_down(5);
        assert!(!keys(&v)[0].muted);
        assert!(matches!(
            effect,
            MixerEffect::SetVolume { unmute: true, .. }
        ));
    }

    #[test]
    fn only_plus_and_minus_repeat_while_held() {
        let mut v = view(2);
        assert!(v.is_step_key(5) && v.is_step_key(10));
        assert!(!v.is_step_key(0) && !v.is_step_key(HOME) && !v.is_step_key(7));
        assert_eq!(v.repeat(0), MixerEffect::None);
        assert!(matches!(v.repeat(5), MixerEffect::SetVolume { .. }));
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
    fn empty_slots_do_nothing() {
        let mut v = view(1);
        for index in [1, 6, 11, 2, 7, 12, 3, 8, 13, 4, 9] {
            assert_eq!(v.press_down(index), MixerEffect::None);
        }
    }

    #[test]
    fn home_stays_on_the_opening_key_when_its_column_is_free() {
        let v = view_at(2, 7);
        assert!(matches!(keys(&v)[7].face, KeyFace::Home { more: false }));
    }

    #[test]
    fn home_moves_to_a_free_column_instead_of_taking_an_apps_key() {
        let v = view_at(3, 6);
        let k = keys(&v);
        assert!(matches!(k[9].face, KeyFace::Home { .. }));
        assert_eq!(k[6].face, KeyFace::Plus);
    }

    #[test]
    fn home_takes_the_opening_key_only_when_every_column_is_full() {
        let v = view_at(5, 6);
        assert!(matches!(keys(&v)[6].face, KeyFace::Home { .. }));
    }

    #[test]
    fn home_does_not_jump_between_pages_when_there_are_more_apps() {
        let mut v = view_at(7, 6);
        assert!(matches!(keys(&v)[6].face, KeyFace::Home { more: true }));
        v.release(6, TAP);
        assert!(matches!(keys(&v)[6].face, KeyFace::Home { more: true }));
    }

    #[test]
    fn home_key_outside_the_grid_falls_back_to_the_last_key() {
        let v = view_at(0, 40);
        assert!(matches!(keys(&v)[KEY_COUNT - 1].face, KeyFace::Home { .. }));
    }

    #[test]
    fn there_is_always_exactly_one_home_key() {
        for apps in 0..9 {
            for opener in 0..KEY_COUNT {
                let homes = keys(&view_at(apps, opener))
                    .iter()
                    .filter(|k| matches!(k.face, KeyFace::Home { .. }))
                    .count();
                assert_eq!(homes, 1, "apps={apps} opener={opener}");
            }
        }
    }

    #[test]
    fn tapping_home_exits_when_everything_fits_on_one_page() {
        assert_eq!(view(5).release(HOME, TAP), MixerEffect::Close);
        assert_eq!(view(0).release(HOME, TAP), MixerEffect::Close);
    }

    #[test]
    fn tapping_home_pages_through_apps_and_wraps() {
        let mut v = view(7);
        assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app1"));
        assert_eq!(v.release(HOME, TAP), MixerEffect::None);
        assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app6"));
        assert!(v.app_at(2).is_none());
        v.release(HOME, TAP);
        assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app1"));
    }

    #[test]
    fn holding_home_always_exits() {
        assert_eq!(view(7).release(HOME, EXIT_HOLD), MixerEffect::Close);
    }

    #[test]
    fn releasing_other_keys_does_nothing() {
        let mut v = view(2);
        assert_eq!(v.release(0, TAP), MixerEffect::None);
        assert_eq!(v.release(5, EXIT_HOLD), MixerEffect::None);
    }

    #[test]
    fn home_label_says_what_a_tap_does() {
        let more = &keys(&view(7))[HOME];
        assert_eq!(more.label, "MAIS");
        assert!(!more.hint.is_empty());
        let exit = &keys(&view(2))[HOME];
        assert_eq!(exit.label, "SAIR");
        assert!(exit.hint.is_empty());
    }

    #[test]
    fn apps_keep_their_order_when_windows_reorders_them() {
        let mut v = MixerView::new(HOME);
        load(&mut v, data(3));
        let mut reordered = data(3);
        reordered.apps.reverse();
        assert!(!load(&mut v, reordered));
        assert_eq!(v.app_at(0).map(|a| a.session.key.as_str()), Some("app1"));
    }

    #[test]
    fn a_paused_app_stays_on_screen_dimmed() {
        let mut v = MixerView::new(HOME);
        load(&mut v, data(2));
        let mut later = data(2);
        later.apps[0].session.active = false;
        assert!(load(&mut v, later));
        assert!(keys(&v)[0].inactive);
        assert!(!keys(&v)[1].inactive);
    }

    #[test]
    fn a_closed_program_leaves_and_a_new_one_joins_at_the_end() {
        let mut v = MixerView::new(HOME);
        load(&mut v, data(3));
        let mut later = data(3);
        later.apps.remove(1);
        later.apps.push(app(9, 0.5, true));
        assert!(load(&mut v, later));
        let order: Vec<_> = (0..3)
            .filter_map(|c| v.app_at(c))
            .map(|a| a.session.key.clone())
            .collect();
        assert_eq!(order, ["app1", "app3", "app9"]);
    }

    #[test]
    fn a_new_app_that_is_not_playing_does_not_join() {
        let mut v = MixerView::new(HOME);
        load(&mut v, data(1));
        let mut later = data(1);
        later.apps.push(app(2, 0.5, false));
        assert!(!load(&mut v, later));
    }

    #[test]
    fn a_reading_made_before_a_key_press_is_discarded() {
        let mut v = view(1);
        let stale_version = v.version();
        v.press_down(5);
        let mut stale = data(1);
        stale.apps[0].session.volume = 0.1;
        assert!(!v.update(stale, stale_version));
        assert_eq!(keys(&v)[0].value, "55%");
    }

    #[test]
    fn a_reading_for_a_closed_mixer_is_not_accepted_by_a_new_one() {
        let old = MixerView::new(HOME);
        let mut reopened = MixerView::new(HOME);
        assert_ne!(old.version(), reopened.version());
        assert!(!reopened.update(data(1), old.version()));
    }

    #[test]
    fn touching_the_page_discards_readings_started_before() {
        let mut v = view(1);
        let started = v.version();
        v.touch();
        assert!(!v.update(data(2), started));
        assert!(load(&mut v, data(2)));
    }

    #[test]
    fn update_fixes_the_page_when_apps_disappear() {
        let mut v = view(7);
        v.release(HOME, TAP);
        assert!(v.update(data(2), v.version()));
        assert!(v.app_at(0).is_some());
    }

    #[test]
    fn only_changed_keys_are_redrawn() {
        let mut v = view(2);
        assert_eq!(v.changed_keys(ACCENT).len(), KEY_COUNT);
        assert!(v.changed_keys(ACCENT).is_empty());
        v.press_down(5);
        let changed: Vec<usize> = v.changed_keys(ACCENT).iter().map(|(i, _)| *i).collect();
        // A tecla de baixo continua cheia no medidor, então não é reenviada.
        assert_eq!(changed, [0, 5]);
    }

    #[test]
    fn invalidate_makes_the_next_draw_full() {
        let v = view(1);
        v.changed_keys(ACCENT);
        v.invalidate();
        assert_eq!(v.changed_keys(ACCENT).len(), KEY_COUNT);
    }

    /// Gera uma imagem das 15 teclas para conferir o visual:
    /// `PREVIEW_OUT=pasta/mixer.png cargo test mixer_preview -- --ignored`.
    #[test]
    #[ignore]
    fn mixer_preview() {
        let mut v = MixerView::new(HOME);
        let mut d = data(3);
        d.apps[0].session.name = "Spotify".into();
        d.apps[0].session.volume = 0.62;
        d.apps[1].session.name = "Google Chrome".into();
        d.apps[1].session.volume = 0.3;
        d.apps[2].session.name = "Discord".into();
        d.apps[2].session.volume = 1.0;
        d.apps[2].session.active = false;
        load(&mut v, d);
        v.press_down(0);
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
