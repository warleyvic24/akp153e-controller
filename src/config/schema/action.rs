//! Ações das teclas (`ButtonAction`) e os tipos que só elas usam. Formato gravado no
//! `config.json` (`type` em kebab-case): mudar um nome aqui exige migração em `migrate.rs`.
use serde::{Deserialize, Serialize};

/// O que a tecla "Monitor" mostra.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Metric {
    /// CPU, RAM e temperatura juntas.
    #[default]
    Overview,
    Cpu,
    Ram,
    Temp,
}

/// O que uma tecla faz ao ser tocada. Navegação, mixer e brilho são tratados pelo serviço
/// (`service::runtime`); o resto vai ao executor (`engine::actions`).
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
    /// Abre um endereço (http ou https) no navegador padrão.
    OpenUrl {
        url: String,
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
    /// Toca um arquivo de som (soundboard), sem esperar terminar.
    PlaySound {
        path: String,
    },
    /// Banco de sons: nome + lista de caminhos (para organizar por categoria).
    SoundBank {
        name: String,
        #[serde(default)]
        sounds: Vec<String>,
    },
    /// Toca um som de um banco com volume (0.0 a 1.0).
    PlaySoundBank {
        bank: String,
        #[serde(default)]
        index: usize,
        #[serde(default = "default_volume")]
        volume: f32,
    },
    /// Para todos os sons tocados pelo soundboard (botão pânico).
    StopAllSounds,
    /// Muda o brilho do aparelho em `delta` pontos (de 5 a 100 %).
    Brightness {
        delta: i32,
    },
    /// Tecla viva: mostra a hora e a data, redesenhada a cada minuto. Tocar não faz nada.
    Clock,
    /// Tecla viva com o uso do sistema (CPU, RAM, temperatura), atualizada a cada poucos segundos.
    Monitor {
        #[serde(default)]
        metric: Metric,
    },
    /// Comando no OBS Studio (obs-websocket). `target` é a cena ou a entrada, quando o comando usa.
    Obs {
        command: ObsCommand,
        #[serde(default)]
        target: String,
    },
    /// Pressionar/soltar rápido executa `tap`; segurar além do limiar executa `hold`.
    PressHold {
        tap: Box<ButtonAction>,
        hold: Box<ButtonAction>,
        #[serde(default = "default_hold_threshold_ms")]
        threshold_ms: u64,
    },
    /// Dois toques rápidos dentro da janela executam `action`.
    DoubleTap {
        action: Box<ButtonAction>,
        #[serde(default = "default_double_tap_window_ms")]
        window_ms: u64,
    },
}

/// Tecla de mídia "Mic mudo": não é uma tecla do teclado; o serviço muda o microfone padrão.
pub const MIC_MUTE_KEY: &str = "micmute";

/// O que uma tecla do OBS faz.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObsCommand {
    /// Vai para a cena `target`.
    Scene,
    /// Liga ou desliga a transmissão.
    Stream,
    /// Liga ou desliga a gravação.
    Record,
    /// Pausa ou retoma a gravação.
    RecordPause,
    /// Muda ou desmuda a entrada de áudio `target`.
    Mute,
    /// Liga ou desliga a câmera virtual.
    VirtualCam,
    /// Liga ou desliga o buffer de replay.
    ReplayBuffer,
    /// Salva o replay.
    ReplaySave,
}

impl ObsCommand {
    pub const ALL: [ObsCommand; 8] = [
        ObsCommand::Scene,
        ObsCommand::Stream,
        ObsCommand::Record,
        ObsCommand::RecordPause,
        ObsCommand::Mute,
        ObsCommand::VirtualCam,
        ObsCommand::ReplayBuffer,
        ObsCommand::ReplaySave,
    ];

    /// O comando age sobre uma cena ou entrada (e precisa de `target`).
    pub fn needs_target(self) -> bool {
        matches!(self, ObsCommand::Scene | ObsCommand::Mute)
    }
}

/// Método padrão do webhook (configs antigas sem o campo).
fn get() -> String {
    "GET".into()
}

impl ButtonAction {
    /// A ação (ou um passo/gesto dentro dela) toca um som de banco, e precisa de `sound_banks`.
    pub fn plays_sound_bank(&self) -> bool {
        match self {
            ButtonAction::PlaySoundBank { .. } => true,
            ButtonAction::MultiAction { steps } => steps.iter().any(Self::plays_sound_bank),
            ButtonAction::PressHold { tap, hold, .. } => {
                tap.plays_sound_bank() || hold.plays_sound_bank()
            }
            ButtonAction::DoubleTap { action, .. } => action.plays_sound_bank(),
            _ => false,
        }
    }
}

/// Limiar padrão para diferenciar toque curto de pressionamento longo: 500 ms.
fn default_hold_threshold_ms() -> u64 {
    500
}

/// Janela padrão para detecção de clique duplo: 300 ms.
fn default_double_tap_window_ms() -> u64 {
    300
}

/// Volume padrão para sons (1.0 = volume máximo).
fn default_volume() -> f32 {
    1.0
}
