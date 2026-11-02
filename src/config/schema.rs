//! Esquema do `config.json`: perfis, páginas, teclas e preferências, com a leitura, a gravação
//! atômica, a trava entre processos e o saneamento/migração de versões antigas.
//!
//! As ações das teclas ficam em `action.rs` (reexportadas aqui); migrações em `migrate.rs`.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::schema::migrate::migrate_control_ids;

pub use action::*;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RGBAColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonAppearance {
    pub background: RGBAColor,
    pub text_color: RGBAColor,
    pub text: String,
    pub font: String,
    pub font_size: f32,
    pub icon_path: Option<String>,
    /// Ícone vetorial: embutido (ver `icons.rs`) ou da biblioteca (`lucide:nome`); tem
    /// prioridade sobre `icon_path`.
    #[serde(default)]
    pub icon: Option<String>,
    /// Onde fica o rótulo. Ausente em configs antigas: `Auto` (embaixo do ícone, ou no meio).
    #[serde(default)]
    pub text_position: TextPosition,
    /// Fundo em degradê suave (mais claro em cima), em vez de cor chapada.
    #[serde(default)]
    pub gradient: bool,
    /// Se deve mostrar o rótulo de texto na tecla.
    #[serde(default = "default_true")]
    pub show_text: bool,
}

fn default_true() -> bool {
    true
}

/// Posição do rótulo na tecla; o ícone ocupa o espaço que sobra.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextPosition {
    /// Embaixo quando há ícone; no meio quando é só texto.
    #[default]
    Auto,
    Top,
    /// No meio, por cima do ícone.
    Middle,
    Bottom,
}

/// Onde achar o obs-websocket. No OBS: Ferramentas > Configurações do servidor WebSocket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObsSettings {
    pub host: String,
    pub port: u16,
    /// Senha do servidor WebSocket (vazia se a autenticação estiver desligada no OBS).
    #[serde(default)]
    pub password: String,
}

impl Default for ObsSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 4455,
            password: String::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonConfig {
    pub appearance: ButtonAppearance,
    pub action: ButtonAction,
    #[serde(default)]
    pub long_press_action: Option<ButtonAction>,
    #[serde(default = "threshold")]
    pub long_press_threshold_ms: u64,
    #[serde(default)]
    pub toggle_state: bool,
    #[serde(default)]
    pub toggled_appearance: Option<ButtonAppearance>,
    /// Ação do segundo toque de uma tecla de dois estados (ex.: ligar/desligar a live).
    #[serde(default)]
    pub toggled_action: Option<ButtonAction>,
    /// Ação de clique duplo: dois toques rápidos dentro da janela executam esta ação.
    #[serde(default)]
    pub double_tap_action: Option<ButtonAction>,
    /// Janela máxima entre dois toques para contar como clique duplo (ms).
    #[serde(default = "threshold")]
    pub double_tap_window_ms: u64,
    /// Id do controle do modelo do aparelho (ex.: \\\"k0\\\", \\\"k1\\\", ..., \\\"side\\\").
    /// Ausente em configs v3: preenchido por índice em `sanitized`.
    #[serde(default)]
    pub control_id: Option<String>,
}
fn threshold() -> u64 {
    500
}

/// Fundo padrão de um botão novo: preto (no display LCD, preto = pixel apagado).
pub const BLANK_BACKGROUND: RGBAColor = RGBAColor {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};
/// Fundo padrão das versões anteriores; é migrado para preto ao carregar.
const LEGACY_BLANK_BACKGROUND: RGBAColor = RGBAColor {
    r: 30,
    g: 30,
    b: 30,
    a: 255,
};

impl ButtonConfig {
    pub fn blank() -> Self {
        Self {
            appearance: ButtonAppearance {
                background: BLANK_BACKGROUND,
                text_color: RGBAColor {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
                text: String::new(),
                font: "default".into(),
                font_size: 14.0,
                icon_path: None,
                icon: None,
                text_position: TextPosition::Auto,
                gradient: false,
                show_text: true,
            },
            action: ButtonAction::None,
            long_press_action: None,
            long_press_threshold_ms: 500,
            toggle_state: false,
            toggled_appearance: None,
            toggled_action: None,
            double_tap_action: None,
            double_tap_window_ms: 500,
            control_id: None,
        }
    }

    /// Tem dois estados: cada toque alterna entre eles.
    pub fn toggles(&self) -> bool {
        self.toggled_appearance.is_some() || self.toggled_action.is_some()
    }
    pub fn current_appearance(&self) -> &ButtonAppearance {
        match (&self.toggled_appearance, self.toggle_state) {
            (Some(a), true) => a,
            _ => &self.appearance,
        }
    }

    /// A aparência em uso agora, para mudar.
    pub fn current_appearance_mut(&mut self) -> &mut ButtonAppearance {
        match (&mut self.toggled_appearance, self.toggle_state) {
            (Some(a), true) => a,
            _ => &mut self.appearance,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    #[serde(default)]
    pub id: u64,
    pub name: String,
    pub rows: usize,
    pub cols: usize,
    /// Um botão por tecla, na ordem de leitura das teclas do modelo (`DeviceModel::keys`):
    /// `buttons[i]` é o controle `keys()[i]` (no AKP153E, `k{i}`).
    pub buttons: Vec<ButtonConfig>,
    /// Modelo de aparelho para o qual a página foi feita (id de `devices/*.json`).
    #[serde(default = "default_model")]
    pub model: String,
    /// Pasta: aberta por uma tecla, fora do "próxima página" e das abas (o editor a mostra só
    /// enquanto ela está aberta). Configs antigas não têm o campo: página comum.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub folder: bool,
    /// O que a tela lateral (controle `side` do modelo) (a faixa à direita das teclas) mostra nesta página.
    #[serde(default)]
    pub side: SideScreen,
}

/// Conteúdo da tela lateral do AKP153E (faixa de ~82 x 400 px à direita das 15 teclas).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum SideScreen {
    /// Apagada (o padrão, e o de configs antigas).
    #[default]
    Off,
    /// Nome da página e do perfil, com um ponto por página.
    PageName,
    /// Hora e data, atualizadas a cada minuto.
    Clock,
    /// CPU, RAM e temperatura ao vivo.
    System,
    /// A música tocando agora: capa, título, artista e se está tocando ou em pausa.
    NowPlaying,
    /// Uma imagem, recortada para preencher a faixa.
    Image { path: String },
}
impl Page {
    pub fn new(name: &str, rows: usize, cols: usize) -> Self {
        Self {
            id: 0,
            name: name.into(),
            rows,
            cols,
            buttons: (0..rows * cols).map(|_| ButtonConfig::blank()).collect(),
            side: SideScreen::Off,
            model: default_model(),
            folder: false,
        }
    }

    /// Divide a página para um aparelho com `keys` teclas: as que sobram viram páginas seguintes
    /// ("Nome (2)", "Nome (3)"), na mesma ordem; nenhum botão se perde. As novas ficam sem id
    /// (`sanitized` dá um). Com teclas suficientes, devolve a página como está.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Fase 4 do plano de aparelhos: chamado quando o serviço abrir um aparelho de outro tamanho"
        )
    )]
    pub fn split_for(&self, keys: usize, rows: usize, cols: usize) -> Vec<Page> {
        if keys == 0 || self.buttons.len() <= keys {
            return vec![self.clone()];
        }
        self.buttons
            .chunks(keys)
            .enumerate()
            .map(|(i, chunk)| {
                let mut page = self.clone();
                if i > 0 {
                    page.id = 0;
                    page.name = format!("{} ({})", self.name, i + 1);
                }
                page.rows = rows;
                page.cols = cols;
                page.buttons = chunk.to_vec();
                page.buttons.resize_with(keys, ButtonConfig::blank);
                page
            })
            .collect()
    }
}

/// Maior número de linhas ou colunas de uma página (8 x 8 = as 64 teclas que o serviço acompanha).
pub const MAX_GRID_SIDE: usize = 8;

/// Ícone da tecla "Voltar" de uma pasta.
pub const BACK_ICON: &str = "lucide:corner-up-left";

/// Modelo das configs feitas antes da Fase 3 (só existia o AKP153E).
pub const DEFAULT_MODEL: &str = "ajazz-akp153e";
fn default_model() -> String {
    DEFAULT_MODEL.into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub id: u64,
    pub name: String,
    pub pages: Vec<Page>,
    #[serde(default)]
    pub active_page: usize,
    /// Nome do `.exe` (ex.: `obs64.exe`) que, em primeiro plano, ativa este perfil sozinho.
    #[serde(default)]
    pub auto_app: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "legacy_version")]
    pub schema_version: u32,
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub active_profile: usize,
    #[serde(default)]
    pub ui: UiSettings,
    /// Conexão com o OBS Studio; ausente em configs antigas (padrão: este PC, porta 4455).
    #[serde(default)]
    pub obs: ObsSettings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    /// Segue o tema ativo do Omarchy (cores, destaque e modo); sem Omarchy, equivale ao escuro.
    Omarchy,
    /// Alto contraste: preto, branco e amarelo, bordas grossas (WCAG AAA, 7:1 em todo texto).
    HighContrast,
}

/// Idioma da interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Pt,
    En,
}

/// Preferências do editor. Tema e destaque só afetam a janela; `brightness` (0 a 100) vai ao aparelho.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSettings {
    pub theme: Theme,
    pub accent: [u8; 3],
    /// Ausente em configs antigas: assume o brilho máximo, o que o aparelho já usava.
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    /// Ausente em configs antigas: português, o idioma que a interface sempre teve.
    #[serde(default)]
    pub language: Language,
    /// Minutos sem uso até o aparelho apagar a tela; 0 = nunca (o padrão, e o de configs antigas).
    #[serde(default)]
    pub sleep_minutes: u16,
    /// Tamanho da interface em % (100 = normal). Ausente em configs antigas: 100.
    #[serde(default = "default_ui_scale")]
    pub ui_scale: u16,
    /// Desliga deslizes e transições (troca de página, avisos, tecla apertada).
    #[serde(default)]
    pub reduce_motion: bool,
}

fn default_ui_scale() -> u16 {
    100
}

/// Tamanhos de interface oferecidos nas Preferências (%); Ctrl + / Ctrl − também ajustam.
pub const UI_SCALES: [u16; 5] = [100, 125, 150, 175, 200];

fn default_brightness() -> u8 {
    crate::usb::DEFAULT_BRIGHTNESS
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            accent: [0, 229, 255],
            brightness: default_brightness(),
            language: Language::default(),
            sleep_minutes: 0,
            ui_scale: default_ui_scale(),
            reduce_motion: false,
        }
    }
}

/// Nome do backup de uma config inválida, com data: um segundo arquivo ruim não apaga o primeiro backup.
fn backup_path(config: &std::path::Path) -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    config.with_extension(format!("json.bak-{secs}"))
}

/// Opções de descanso da tela oferecidas nas Preferências (minutos; 0 = nunca).
pub const SLEEP_CHOICES: [u16; 7] = [0, 1, 5, 10, 15, 30, 60];

/// Permissão do `config.json` no Linux: leitura e escrita só para o dono.
#[cfg(unix)]
const CONFIG_FILE_MODE: u32 = 0o600;
/// Arquivo vazio, ao lado da config, que serve de trava entre editor e serviço (`AppConfig::locked`).
const LOCK_FILE: &str = "config.lock";

/// Versão 2: páginas e perfis têm identificador estável e as ações apontam para ele.
/// Versão 3: cada página guarda o modelo de aparelho (`Page::model`) e a gravação recusa uma
/// config de versão mais nova que esta (feita por um Keyforge mais novo).
/// Versão 4: "abrir site" é a ação `OpenUrl` (antes, um `LaunchApp` do `xdg-open`/`explorer`).
pub const SCHEMA_VERSION: u32 = 5;
fn legacy_version() -> u32 {
    1
}

/// Zera ação (e ação longa) de um botão que aponte para um destino removido.
fn reset_targets(b: &mut ButtonConfig, hit: impl Fn(&ButtonAction) -> bool) {
    if hit(&b.action) {
        b.action = ButtonAction::None;
    }
    if b.long_press_action.as_ref().is_some_and(&hit) {
        b.long_press_action = None;
    }
    if b.toggled_action.as_ref().is_some_and(&hit) {
        b.toggled_action = None;
    }
    for a in [
        Some(&mut b.action),
        b.long_press_action.as_mut(),
        b.toggled_action.as_mut(),
    ]
    .into_iter()
    .flatten()
    {
        if let ButtonAction::MultiAction { steps } = a {
            steps.retain(|s| !hit(s));
        }
    }
}

impl Profile {
    /// Remove a página; recusa se for a última. Ações que apontavam para ela são desativadas.
    pub fn remove_page(&mut self, index: usize) -> bool {
        if self.pages.len() <= 1 || index >= self.pages.len() {
            return false;
        }
        let id = self.pages.remove(index).id;
        for b in self.pages.iter_mut().flat_map(|p| p.buttons.iter_mut()) {
            reset_targets(
                b,
                |a| matches!(a, ButtonAction::SwitchPage { page } if *page == id),
            );
        }
        self.active_page = self.active_page.min(self.pages.len() - 1);
        true
    }

    /// Troca a página de lugar com a vizinha (`delta` = -1 ou 1); devolve a nova posição.
    pub fn move_page(&mut self, index: usize, delta: isize) -> Option<usize> {
        let to = index
            .checked_add_signed(delta)
            .filter(|&t| t < self.pages.len() && index < self.pages.len())?;
        self.pages.swap(index, to);
        if self.active_page == index {
            self.active_page = to
        } else if self.active_page == to {
            self.active_page = index
        }
        Some(to)
    }

    pub fn page_index(&self, id: u64) -> Option<usize> {
        self.pages.iter().position(|p| p.id == id)
    }

    pub fn add_page(&mut self, name: &str) {
        let id = self.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let mut page = Page::new(name, 3, 5);
        page.id = id;
        self.pages.push(page);
    }

    /// Cria uma pasta que volta para a página `parent` e devolve o id dela. A tecla "Voltar" já
    /// nasce no primeiro lugar (o canto de cima, à esquerda, como no app da Elgato); é um botão
    /// comum, então dá para mudar de lugar ou de cara.
    pub fn add_folder(&mut self, name: &str, parent: u64, back_label: &str) -> u64 {
        self.add_page(name);
        let folder = self.pages.last_mut().expect("acabou de entrar");
        folder.folder = true;
        let back = &mut folder.buttons[0];
        back.action = ButtonAction::SwitchPage { page: parent };
        back.appearance.text = back_label.into();
        back.appearance.icon = Some(BACK_ICON.into());
        folder.id
    }

    /// Próxima (ou anterior) página que não é pasta, em volta; a atual se não houver outra.
    pub fn step_page(&self, forward: bool) -> usize {
        let count = self.pages.len();
        let current = self.active_page.min(count.saturating_sub(1));
        (1..count)
            .map(|step| {
                if forward {
                    (current + step) % count
                } else {
                    (current + count - step) % count
                }
            })
            .find(|&i| !self.pages[i].folder)
            .unwrap_or(current)
    }
}

/// Dá um id novo (maior que todos) a cada item que ainda não tem.
fn assign_ids<'a>(ids: impl Iterator<Item = &'a mut u64>) {
    let mut all: Vec<&mut u64> = ids.collect();
    let first_free = all.iter().map(|i| **i).max().unwrap_or(0) + 1;
    for (id, new_id) in all.iter_mut().filter(|i| ***i == 0).zip(first_free..) {
        **id = new_id;
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut page = Page::new("Page 1", 3, 5);
        page.id = 1;
        Self {
            schema_version: SCHEMA_VERSION,
            profiles: vec![Profile {
                id: 1,
                name: "Default".into(),
                pages: vec![page],
                active_page: 0,
                auto_app: None,
            }],
            active_profile: 0,
            ui: UiSettings::default(),
            obs: ObsSettings::default(),
        }
    }
}

/// Nome da pasta da config (`~/.config/Keyforge`, `%APPDATA%\\Keyforge`).
pub const APP_DIR: &str = "Keyforge";

/// Variável de ambiente de desenvolvimento: `KEYFORGE_<nome>`, ou o nome antigo `AKP153E_<nome>`.
pub fn dev_var(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(format!("KEYFORGE_{name}"))
        .or_else(|| std::env::var_os(format!("AKP153E_{name}")))
}

impl AppConfig {
    pub fn path() -> Option<PathBuf> {
        // Os testes criam `App`s que gravam ao serem descartados: nunca no arquivo real do usuário.
        if cfg!(test) {
            let dir = std::env::temp_dir().join(format!("keyforge-test-{}", std::process::id()));
            return Some(dir.join("config.json"));
        }
        // Só em debug: permite testar a interface (inclusive editando e excluindo) numa cópia da
        // config, sem tocar no arquivo real. O release não tem este desvio.
        #[cfg(debug_assertions)]
        if let Some(dir) = dev_var("CONFIG_DIR") {
            return Some(PathBuf::from(dir).join("config.json"));
        }
        directories::BaseDirs::new().map(|d| d.config_dir().join(APP_DIR).join("config.json"))
    }
}

impl AppConfig {
    /// A config do disco, ou `None` se faltar, não puder ser lida agora ou for inválida.
    /// Quem vai regravar o arquivo usa esta: `load()` cai na config padrão e a gravaria por cima.
    pub fn load_existing() -> Option<Self> {
        let path = Self::path()?;
        let text = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str::<Self>(&text)
            .ok()
            .map(|c| c.backed_up(&path, &text).sanitized())
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(c) => c.backed_up(&path, &text).sanitized(),
            Err(e) => {
                let backup = backup_path(&path);
                log::warn!("config inválida ({e}); backup em {}", backup.display());
                if let Err(copy_error) = std::fs::copy(&path, &backup) {
                    log::error!("backup da config inválida falhou: {copy_error}");
                }
                Self::default()
            }
        }
    }

    /// Garante invariantes (>=1 perfil/página, grade completa, ids e índices válidos) e migra versões antigas.
    pub fn sanitized(mut self) -> Self {
        self.ui.brightness = self.ui.brightness.min(crate::usb::MAX_BRIGHTNESS);
        if self.profiles.is_empty() {
            return Self::default();
        }
        for p in &mut self.profiles {
            if p.pages.is_empty() {
                p.pages.push(Page::new("Page 1", 3, 5));
            }
            for pg in &mut p.pages {
                // Até 8 x 8 (o Stream Deck XL tem 8 x 4); a grade real vem do modelo da página.
                pg.rows = pg.rows.clamp(1, MAX_GRID_SIDE);
                pg.cols = pg.cols.clamp(1, MAX_GRID_SIDE);
                pg.buttons
                    .resize_with(pg.rows * pg.cols, ButtonConfig::blank);
                for b in pg
                    .buttons
                    .iter_mut()
                    .filter(|b| b.appearance.background == LEGACY_BLANK_BACKGROUND)
                {
                    b.appearance.background = BLANK_BACKGROUND;
                }
            }
            assign_ids(p.pages.iter_mut().map(|pg| &mut pg.id));
            p.active_page = p.active_page.min(p.pages.len() - 1);
        }
        assign_ids(self.profiles.iter_mut().map(|p| &mut p.id));
        self.active_profile = self.active_profile.min(self.profiles.len() - 1);
        // v2 -> v3 não muda dados: `model` ausente já vira o AKP153E ao ler.
        if self.schema_version < 2 {
            self.migrate_index_targets();
        }
        if self.schema_version < 4 {
            self.migrate_open_url();
        }
        if self.schema_version < 5 {
            migrate_control_ids(&mut self);
        }
        // Mais nova que este programa: fica como está, e `save_to` se recusa a gravar.
        self.schema_version = self.schema_version.max(SCHEMA_VERSION);
        self
    }

    pub fn save(&self) -> std::io::Result<()> {
        let p = Self::path().ok_or_else(|| std::io::Error::other("sem diretório de config"))?;
        self.save_to(&p)
    }

    /// Grava em arquivo temporário e renomeia, para uma queda no meio nunca corromper o original.
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        if self.schema_version > SCHEMA_VERSION {
            return Err(std::io::Error::other(format!(
                "a config é da versão {} e este Keyforge só conhece até a {SCHEMA_VERSION}: atualize o programa",
                self.schema_version
            )));
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        // Só o dono lê: a config guarda URLs de webhook (com tokens) e caminhos de programas.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, CONFIG_FILE_MODE);
        let written = options.open(&tmp).and_then(|mut f| {
            use std::io::Write;
            f.write_all(json.as_bytes())?;
            f.sync_all() // sem isto, uma queda de energia após o rename pode deixar o arquivo vazio
        });
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    /// Roda `job` (reler, mudar e gravar o arquivo) com a trava entre processos `config.lock`.
    /// Editor e serviço regravam o mesmo arquivo; sem a trava, um podia ler antes de o outro
    /// gravar e desfazer a mudança dele (ex.: a troca de página feita no aparelho). Sem a trava
    /// (pasta inacessível, sistema de arquivos sem suporte) `job` roda assim mesmo.
    pub fn locked<R>(job: impl FnOnce() -> R) -> R {
        let lock = Self::path()
            .map(|p| p.with_file_name(LOCK_FILE))
            .and_then(|p| {
                std::fs::create_dir_all(p.parent()?).ok()?;
                std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(p)
                    .ok()
            });
        match &lock {
            Some(file) => {
                if let Err(e) = file.lock() {
                    log::warn!("trava da config: {e}");
                }
            }
            None => log::warn!("trava da config indisponível"),
        }
        // A trava é solta quando `lock` sai de escopo (o arquivo é fechado).
        job()
    }

    /// Adota o perfil e a página ativos de `disk` (achados por id), se ainda existirem aqui.
    /// Serve para o editor não desfazer uma troca de página feita pelo aparelho.
    pub fn adopt_active_from(&mut self, disk: &AppConfig) {
        let Some(source) = disk.profiles.get(disk.active_profile) else {
            return;
        };
        let Some(index) = self.profile_index(source.id) else {
            return;
        };
        self.active_profile = index;
        let page_id = source.pages.get(source.active_page).map(|p| p.id);
        if let Some(page) = page_id.and_then(|id| self.profiles[index].page_index(id)) {
            self.profiles[index].active_page = page;
        }
    }

    pub fn profile_index(&self, id: u64) -> Option<usize> {
        self.profiles.iter().position(|p| p.id == id)
    }

    /// Remove o perfil; recusa se for o último. Ações que apontavam para ele são desativadas.
    pub fn remove_profile(&mut self, index: usize) -> bool {
        if self.profiles.len() <= 1 || index >= self.profiles.len() {
            return false;
        }
        let id = self.profiles.remove(index).id;
        for b in self
            .profiles
            .iter_mut()
            .flat_map(|p| p.pages.iter_mut())
            .flat_map(|pg| pg.buttons.iter_mut())
        {
            reset_targets(
                b,
                |a| matches!(a, ButtonAction::SwitchProfile { profile } if *profile == id),
            );
        }
        if index < self.active_profile {
            self.active_profile -= 1;
        }
        self.active_profile = self.active_profile.min(self.profiles.len() - 1);
        true
    }

    pub fn add_profile(&mut self, name: &str) {
        let id = self.profiles.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let mut page = Page::new("Page 1", 3, 5);
        page.id = 1;
        self.profiles.push(Profile {
            id,
            name: name.into(),
            pages: vec![page],
            active_page: 0,
            auto_app: None,
        });
    }

    pub fn page(&self) -> &Page {
        let p = &self.profiles[self.active_profile.min(self.profiles.len() - 1)];
        &p.pages[p.active_page.min(p.pages.len() - 1)]
    }
    pub fn page_mut(&mut self) -> &mut Page {
        let i = self.active_profile.min(self.profiles.len() - 1);
        let p = &mut self.profiles[i];
        let j = p.active_page.min(p.pages.len() - 1);
        &mut p.pages[j]
    }
}

/// Bancos de som da config: nome do banco → arquivos, na ordem em que a tecla os lista.
pub type SoundBanks = std::collections::BTreeMap<String, Vec<String>>;

impl AppConfig {
    /// Junta os bancos (`ButtonAction::SoundBank`) de todos os perfis e páginas. Bancos sem nome
    /// ou sem sons ficam de fora; com dois bancos de mesmo nome, vale o último encontrado.
    pub fn sound_banks(&self) -> SoundBanks {
        self.profiles
            .iter()
            .flat_map(|p| &p.pages)
            .flat_map(|pg| &pg.buttons)
            .filter_map(|b| match &b.action {
                ButtonAction::SoundBank { name, sounds }
                    if !name.is_empty() && !sounds.is_empty() =>
                {
                    Some((name.clone(), sounds.clone()))
                }
                _ => None,
            })
            .collect()
    }
}

mod action;
mod migrate;
#[cfg(test)]
mod tests;
