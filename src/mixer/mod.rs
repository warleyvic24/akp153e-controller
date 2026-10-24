//! Página de mixer de volume no aparelho. Cada coluna é um app: em cima o ícone com nome e porcentagem
//! (apertar muta), no meio volume + e embaixo volume − (segurar repete). A tecla de sair/mais fica
//! onde o mixer foi aberto, ou numa coluna livre quando essa já tem app.
//! Esta parte é pura (sem áudio nem USB), por isso é testada sem o aparelho.
use crate::config::schema::RGBAColor;
use crate::platform::apps::{extract_icon, InstalledApp};
use crate::platform::audio::{self, AudioSession};
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
    let cache = icon_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
                .unwrap_or_else(std::sync::PoisonError::into_inner)
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
mod tests;
