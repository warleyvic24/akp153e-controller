use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ButtonAction {
    None,
    Hotkey {
        keys: Vec<String>,
    },
    LaunchApp {
        path: String,
        #[serde(default)]
        args: Vec<String>,
    },
    HttpWebhook {
        url: String,
        #[serde(default = "get")]
        method: String,
        #[serde(default)]
        body: Option<String>,
    },
    MediaKey {
        key: String,
    },
    SwitchPage {
        page: u64,
    },
    SwitchProfile {
        profile: u64,
    },
    /// Mostra no aparelho o mixer de volume dos apps com som.
    OpenMixer,
    /// Digita o texto como se fosse no teclado.
    TypeText {
        text: String,
    },
    /// Várias ações em sequência, na ordem da lista (a "multi-ação" do Stream Deck).
    MultiAction {
        steps: Vec<ButtonAction>,
    },
    /// Pausa entre os passos de uma multi-ação; sozinha não faz nada.
    Delay {
        ms: u64,
    },
    /// Próxima página do perfil (depois da última volta à primeira).
    NextPage,
    /// Página anterior do perfil (antes da primeira vai à última).
    PreviousPage,
    /// Roda uma linha de comando no shell do sistema (`sh -c` no Linux, `cmd /C` no Windows).
    RunCommand {
        command: String,
    },
    /// Muda o brilho do aparelho em `delta` pontos (de 5 a 100 %).
    Brightness {
        delta: i32,
    },
    /// Tecla viva: mostra a hora e a data, redesenhada a cada minuto. Tocar não faz nada.
    Clock,
}
fn get() -> String {
    "GET".into()
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
            },
            action: ButtonAction::None,
            long_press_action: None,
            long_press_threshold_ms: 500,
            toggle_state: false,
            toggled_appearance: None,
            toggled_action: None,
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    #[serde(default)]
    pub id: u64,
    pub name: String,
    pub rows: usize,
    pub cols: usize,
    pub buttons: Vec<ButtonConfig>,
}
impl Page {
    pub fn new(name: &str, rows: usize, cols: usize) -> Self {
        Self {
            id: 0,
            name: name.into(),
            rows,
            cols,
            buttons: (0..rows * cols).map(|_| ButtonConfig::blank()).collect(),
        }
    }
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    /// Segue o tema ativo do Omarchy (cores, destaque e modo); sem Omarchy, equivale ao escuro.
    Omarchy,
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
}

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

/// Permissão do `config.json` no Linux: leitura e escrita só para o dono.
#[cfg(unix)]
const CONFIG_FILE_MODE: u32 = 0o600;
/// Arquivo vazio, ao lado da config, que serve de trava entre editor e serviço (`AppConfig::locked`).
const LOCK_FILE: &str = "config.lock";

/// Versão 2: páginas e perfis têm identificador estável e as ações apontam para ele.
pub const SCHEMA_VERSION: u32 = 2;
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
}

/// v1 -> v2 de uma ação: posição de página/perfil vira id (0 = destino que não existe mais).
/// Entra nos passos de uma multi-ação, que também guardavam a posição.
fn retarget_by_index(a: &mut ButtonAction, page_ids: &[u64], profile_ids: &[u64]) {
    match a {
        ButtonAction::SwitchPage { page } => {
            *page = page_ids.get(*page as usize).copied().unwrap_or(0)
        }
        ButtonAction::SwitchProfile { profile } => {
            *profile = profile_ids.get(*profile as usize).copied().unwrap_or(0)
        }
        ButtonAction::MultiAction { steps } => {
            for step in steps {
                retarget_by_index(step, page_ids, profile_ids);
            }
        }
        _ => {}
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
        }
    }
}

impl AppConfig {
    pub fn path() -> Option<PathBuf> {
        // Os testes criam `App`s que gravam ao serem descartados: nunca no arquivo real do usuário.
        if cfg!(test) {
            let dir = std::env::temp_dir().join(format!("akp153e-test-{}", std::process::id()));
            return Some(dir.join("config.json"));
        }
        // Só em debug: permite testar a interface (inclusive editando e excluindo) numa cópia da
        // config, sem tocar no arquivo real. O release não tem este desvio.
        #[cfg(debug_assertions)]
        if let Some(dir) = std::env::var_os("AKP153E_CONFIG_DIR") {
            return Some(PathBuf::from(dir).join("config.json"));
        }
        directories::BaseDirs::new().map(|d| {
            d.config_dir()
                .join("AJazz")
                .join("AKP153E")
                .join("config.json")
        })
    }
    /// A config do disco, ou `None` se faltar, não puder ser lida agora ou for inválida.
    /// Quem vai regravar o arquivo usa esta: `load()` cai na config padrão e a gravaria por cima.
    pub fn load_existing() -> Option<Self> {
        let text = std::fs::read_to_string(Self::path()?).ok()?;
        serde_json::from_str::<Self>(&text)
            .ok()
            .map(Self::sanitized)
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(c) => c.sanitized(),
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
                pg.rows = pg.rows.clamp(1, 3);
                pg.cols = pg.cols.clamp(1, 5);
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
        if self.schema_version < SCHEMA_VERSION {
            self.migrate_index_targets();
            self.schema_version = SCHEMA_VERSION;
        }
        self
    }

    /// v1 -> v2: ações que apontavam para a posição passam a apontar para o id.
    fn migrate_index_targets(&mut self) {
        let profile_ids: Vec<u64> = self.profiles.iter().map(|p| p.id).collect();
        for p in &mut self.profiles {
            let page_ids: Vec<u64> = p.pages.iter().map(|pg| pg.id).collect();
            for b in p.pages.iter_mut().flat_map(|pg| pg.buttons.iter_mut()) {
                for a in std::iter::once(&mut b.action)
                    .chain(b.long_press_action.as_mut())
                    .chain(b.toggled_action.as_mut())
                {
                    retarget_by_index(a, &page_ids, &profile_ids);
                }
            }
        }
    }
    pub fn save(&self) -> std::io::Result<()> {
        let p = Self::path().ok_or_else(|| std::io::Error::other("sem diretório de config"))?;
        self.save_to(&p)
    }

    /// Grava em arquivo temporário e renomeia, para uma queda no meio nunca corromper o original.
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
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

#[cfg(test)]
mod tests;
