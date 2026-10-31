//! Os tipos de ação do editor (o que aparece na grade e nos passos de uma multi-ação). Antes eram
//! textos ("launch-app"); com o `enum`, uma ação nova no `ButtonAction` que não for tratada aqui
//! não compila.
use crate::config::schema::{ButtonAction, Metric, ObsCommand};

/// Passo padrão da tecla de brilho (e o que o editor oferece de cara).
pub(super) const BRIGHTNESS_STEP: i32 = 10;
/// Pausa padrão de um passo "Pausa" novo.
const DEFAULT_STEP_DELAY_MS: u64 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionKind {
    None,
    Hotkey,
    LaunchApp,
    OpenUrl,
    Webhook,
    Media,
    TypeText,
    Multi,
    SwitchPage,
    NextPage,
    PreviousPage,
    SwitchProfile,
    Mixer,
    Command,
    Brightness,
    Clock,
    Monitor,
    Obs,
    Sound,
    Delay,
}

impl ActionKind {
    /// Na grade de tipos, nesta ordem (a "Pausa" só existe dentro de uma multi-ação).
    pub const PICKER: [Self; 19] = [
        Self::None,
        Self::Hotkey,
        Self::LaunchApp,
        Self::OpenUrl,
        Self::Media,
        Self::TypeText,
        Self::Multi,
        Self::SwitchPage,
        Self::NextPage,
        Self::PreviousPage,
        Self::SwitchProfile,
        Self::Mixer,
        Self::Command,
        Self::Brightness,
        Self::Clock,
        Self::Monitor,
        Self::Obs,
        Self::Sound,
        Self::Webhook,
    ];

    /// O que pode ser passo de uma multi-ação (navegação não cabe: o serviço a ignora lá dentro).
    pub const STEPS: [Self; 10] = [
        Self::Hotkey,
        Self::Sound,
        Self::Obs,
        Self::Command,
        Self::TypeText,
        Self::LaunchApp,
        Self::OpenUrl,
        Self::Webhook,
        Self::Media,
        Self::Delay,
    ];

    pub fn of(a: &ButtonAction) -> Self {
        match a {
            ButtonAction::None => Self::None,
            ButtonAction::Hotkey { .. } => Self::Hotkey,
            ButtonAction::LaunchApp { .. } => Self::LaunchApp,
            ButtonAction::OpenUrl { .. } => Self::OpenUrl,
            ButtonAction::HttpWebhook { .. } => Self::Webhook,
            ButtonAction::MediaKey { .. } => Self::Media,
            ButtonAction::SwitchPage { .. } => Self::SwitchPage,
            ButtonAction::SwitchProfile { .. } => Self::SwitchProfile,
            ButtonAction::OpenMixer => Self::Mixer,
            ButtonAction::TypeText { .. } => Self::TypeText,
            ButtonAction::MultiAction { .. } => Self::Multi,
            ButtonAction::Delay { .. } => Self::Delay,
            ButtonAction::NextPage => Self::NextPage,
            ButtonAction::PreviousPage => Self::PreviousPage,
            ButtonAction::RunCommand { .. } => Self::Command,
            ButtonAction::Brightness { .. } => Self::Brightness,
            ButtonAction::Clock => Self::Clock,
            ButtonAction::Monitor { .. } => Self::Monitor,
            ButtonAction::Obs { .. } => Self::Obs,
            ButtonAction::PlaySound { .. } => Self::Sound,
        }
    }

    /// Nome curto (passa por `t`).
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Nenhuma",
            Self::Hotkey => "Atalho",
            Self::LaunchApp => "App",
            Self::OpenUrl => "Site",
            Self::Webhook => "Webhook",
            Self::Media => "Mídia",
            Self::TypeText => "Texto",
            Self::Multi => "Multi",
            Self::SwitchPage => "Página",
            Self::NextPage => "Pág. +",
            Self::PreviousPage => "Pág. −",
            Self::SwitchProfile => "Perfil",
            Self::Mixer => "Mixer",
            Self::Command => "Comando",
            Self::Brightness => "Brilho",
            Self::Clock => "Relógio",
            Self::Monitor => "Monitor",
            Self::Obs => "OBS",
            Self::Sound => "Som",
            Self::Delay => "Pausa",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::None => "ban",
            Self::Hotkey | Self::TypeText => "keyboard",
            Self::LaunchApp => "app",
            Self::OpenUrl => "globe",
            Self::Webhook => "lucide:webhook",
            Self::Media => "music",
            Self::SwitchPage => "page",
            Self::SwitchProfile => "user",
            Self::Mixer => "mixer",
            Self::Multi => "plus",
            Self::NextPage => "next",
            Self::PreviousPage => "prev",
            Self::Command => "lucide:terminal",
            Self::Brightness => "lucide:sun",
            Self::Clock => "lucide:clock",
            Self::Monitor => "lucide:activity",
            Self::Obs => "lucide:clapperboard",
            Self::Sound => "lucide:audio-lines",
            Self::Delay => "lucide:hourglass",
        }
    }

    /// Ação nova deste tipo, com os valores padrão.
    pub fn default_action(self) -> ButtonAction {
        match self {
            Self::None => ButtonAction::None,
            Self::Hotkey => ButtonAction::Hotkey { keys: vec![] },
            Self::LaunchApp => ButtonAction::LaunchApp {
                path: String::new(),
                args: vec![],
            },
            Self::OpenUrl => ButtonAction::OpenUrl {
                url: "https://".into(),
            },
            Self::Webhook => ButtonAction::HttpWebhook {
                url: String::new(),
                method: "POST".into(),
                body: None,
            },
            Self::Media => ButtonAction::MediaKey {
                key: "playpause".into(),
            },
            Self::SwitchPage => ButtonAction::SwitchPage { page: 0 },
            Self::SwitchProfile => ButtonAction::SwitchProfile { profile: 0 },
            Self::Mixer => ButtonAction::OpenMixer,
            Self::TypeText => ButtonAction::TypeText {
                text: String::new(),
            },
            Self::Multi => ButtonAction::MultiAction { steps: vec![] },
            Self::Delay => ButtonAction::Delay {
                ms: DEFAULT_STEP_DELAY_MS,
            },
            Self::NextPage => ButtonAction::NextPage,
            Self::PreviousPage => ButtonAction::PreviousPage,
            Self::Command => ButtonAction::RunCommand {
                command: String::new(),
            },
            Self::Brightness => ButtonAction::Brightness {
                delta: BRIGHTNESS_STEP,
            },
            Self::Clock => ButtonAction::Clock,
            Self::Monitor => ButtonAction::Monitor {
                metric: Metric::Overview,
            },
            Self::Sound => ButtonAction::PlaySound {
                path: String::new(),
            },
            Self::Obs => ButtonAction::Obs {
                command: ObsCommand::Record,
                target: String::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_default_action() {
        for kind in ActionKind::PICKER.iter().chain(&ActionKind::STEPS) {
            assert_eq!(ActionKind::of(&kind.default_action()), *kind, "{kind:?}");
        }
    }

    #[test]
    fn the_picker_lists_each_kind_once_and_steps_never_navigate() {
        let mut seen = ActionKind::PICKER.to_vec();
        seen.dedup();
        assert_eq!(seen.len(), ActionKind::PICKER.len());
        assert!(!ActionKind::STEPS.iter().any(|k| matches!(
            k,
            ActionKind::SwitchPage
                | ActionKind::NextPage
                | ActionKind::PreviousPage
                | ActionKind::SwitchProfile
                | ActionKind::Multi
        )));
    }
}
