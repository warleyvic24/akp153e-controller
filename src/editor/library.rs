use crate::config::schema::{AppConfig, ButtonAction, ObsCommand};

/// Ação pronta da biblioteca: aplicada a um botão define ação, ícone, rótulo e cor de fundo.
#[derive(Clone, Debug)]
pub struct Preset {
    pub category: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub color: [u8; 3],
    pub action: ButtonAction,
}

const MEDIA: [u8; 3] = [154, 74, 120];
const KEYS: [u8; 3] = [68, 105, 159];
const APPS: [u8; 3] = [63, 138, 106];
const WEB: [u8; 3] = [168, 120, 44];
const NAV: [u8; 3] = [111, 88, 184];
/// Teclas que mexem no próprio aparelho (brilho) ou mostram algo vivo (relógio).
const DEVICE: [u8; 3] = [52, 60, 78];
/// Teclas do OBS Studio.
const OBS: [u8; 3] = [40, 44, 58];
/// Cores oferecidas no seletor do inspetor (a última é o grafite neutro).
pub const SWATCHES: [[u8; 3]; 7] = [MEDIA, KEYS, APPS, WEB, NAV, [42, 47, 58], [0, 0, 0]];

fn hotkey(keys: &[&str]) -> ButtonAction {
    ButtonAction::Hotkey {
        keys: keys.iter().map(std::string::ToString::to_string).collect(),
    }
}

fn media(key: &str) -> ButtonAction {
    ButtonAction::MediaKey { key: key.into() }
}

fn app(path: &str) -> ButtonAction {
    ButtonAction::LaunchApp {
        path: path.into(),
        args: vec![],
    }
}

/// "Área de trabalho" é o Win+D; no Linux o Super+D de cada ambiente faz outra coisa (no Omarchy,
/// nada), então a ação não aparece.
#[cfg(not(windows))]
const WINDOWS_ONLY: [&str; 4] = [
    "Calculadora",
    "Bloco de notas",
    "Explorador",
    "Área de trabalho",
];

/// Win+Shift+S no Windows; no Linux a tecla Print, que GNOME, KDE e Omarchy ligam à captura.
fn screenshot() -> ButtonAction {
    #[cfg(windows)]
    return hotkey(&["meta", "shift", "s"]);
    #[cfg(not(windows))]
    return hotkey(&["print"]);
}

/// Win+L no Windows; no Linux o `loginctl`, que pede ao gerenciador de sessão para bloquear
/// (o Super+L do Linux não é padrão: no Omarchy, por exemplo, não bloqueia).
fn lock_screen() -> ButtonAction {
    #[cfg(windows)]
    return hotkey(&["meta", "l"]);
    #[cfg(not(windows))]
    return ButtonAction::LaunchApp {
        path: "loginctl".into(),
        args: vec!["lock-session".into()],
    };
}

/// Programa que abre uma URL no navegador padrão. No Windows, o Explorer, não o `cmd /C start`:
/// o `cmd` trata `&` da URL como separador de comandos. No Linux, o `xdg-open`.
#[cfg(windows)]
pub(crate) const URL_OPENER: &str = "explorer";
#[cfg(not(windows))]
pub(crate) const URL_OPENER: &str = "xdg-open";

fn open_url(url: &str) -> ButtonAction {
    ButtonAction::LaunchApp {
        path: URL_OPENER.into(),
        args: vec![url.into()],
    }
}

impl Preset {
    /// "Aplicativo...": ação de abrir programa ainda sem caminho; quem escolhe o programa é o usuário.
    pub fn opens_app_picker(&self) -> bool {
        matches!(&self.action, ButtonAction::LaunchApp { path, .. } if path.is_empty())
    }

    /// Presets de navegação guardam a ordem (0 = primeira); aqui viram o id real do destino.
    pub fn resolved_action(&self, cfg: &AppConfig) -> ButtonAction {
        match &self.action {
            ButtonAction::SwitchPage { page } => ButtonAction::SwitchPage {
                page: cfg.profiles[cfg.active_profile]
                    .pages
                    .get(*page as usize)
                    .map_or(0, |p| p.id),
            },
            ButtonAction::SwitchProfile { profile } => ButtonAction::SwitchProfile {
                profile: cfg.profiles.get(*profile as usize).map_or(0, |p| p.id),
            },
            other => other.clone(),
        }
    }
}

/// Preset do OBS: o nome e o ícone do comando.
fn obs(command: ObsCommand) -> Preset {
    Preset {
        category: "OBS",
        label: super::obs_ui::command_label(command),
        icon: super::obs_ui::command_icon(command),
        color: OBS,
        action: ButtonAction::Obs {
            command,
            target: String::new(),
        },
    }
}

pub fn presets() -> Vec<Preset> {
    let p = |category, label, icon, color, action| Preset {
        category,
        label,
        icon,
        color,
        action,
    };
    let list = vec![
        p("Mídia", "Play/Pause", "play", MEDIA, media("playpause")),
        p("Mídia", "Próxima", "next", MEDIA, media("next")),
        p("Mídia", "Anterior", "prev", MEDIA, media("prev")),
        p("Mídia", "Vol +", "volup", MEDIA, media("volup")),
        p("Mídia", "Vol -", "voldown", MEDIA, media("voldown")),
        p("Mídia", "Mudo", "mute", MEDIA, media("mute")),
        p("Mídia", "Mixer", "mixer", MEDIA, ButtonAction::OpenMixer),
        p("Atalhos", "Copiar", "copy", KEYS, hotkey(&["ctrl", "c"])),
        p("Atalhos", "Colar", "paste", KEYS, hotkey(&["ctrl", "v"])),
        p(
            "Atalhos",
            "Recortar",
            "lucide:scissors",
            KEYS,
            hotkey(&["ctrl", "x"]),
        ),
        p("Atalhos", "Desfazer", "undo", KEYS, hotkey(&["ctrl", "z"])),
        p(
            "Atalhos",
            "Salvar",
            "lucide:save",
            KEYS,
            hotkey(&["ctrl", "s"]),
        ),
        p("Atalhos", "Captura", "lucide:camera", KEYS, screenshot()),
        p(
            "Atalhos",
            "Área de trabalho",
            "page",
            KEYS,
            hotkey(&["meta", "d"]),
        ),
        p("Atalhos", "Bloquear", "lock", KEYS, lock_screen()),
        p("Aplicativos", "Calculadora", "calc", APPS, app("calc")),
        p(
            "Aplicativos",
            "Bloco de notas",
            "page",
            APPS,
            app("notepad"),
        ),
        p("Aplicativos", "Explorador", "folder", APPS, app("explorer")),
        p("Aplicativos", "Aplicativo...", "app", APPS, app("")),
        p(
            "Aplicativos",
            "Comando",
            "lucide:terminal",
            APPS,
            ButtonAction::RunCommand {
                command: String::new(),
            },
        ),
        p("Web", "Abrir site", "globe", WEB, open_url("https://")),
        p(
            "Web",
            "Webhook",
            "lucide:webhook",
            WEB,
            ButtonAction::HttpWebhook {
                url: String::new(),
                method: "POST".into(),
                body: None,
            },
        ),
        p(
            "Navegação",
            "Página 1",
            "page",
            NAV,
            ButtonAction::SwitchPage { page: 0 },
        ),
        p(
            "Navegação",
            "Página 2",
            "page",
            NAV,
            ButtonAction::SwitchPage { page: 1 },
        ),
        p(
            "Navegação",
            "Perfil 1",
            "user",
            NAV,
            ButtonAction::SwitchProfile { profile: 0 },
        ),
        p(
            "Navegação",
            "Perfil 2",
            "user",
            NAV,
            ButtonAction::SwitchProfile { profile: 1 },
        ),
        p(
            "Aparelho",
            "Brilho +",
            "lucide:sun",
            DEVICE,
            ButtonAction::Brightness { delta: 10 },
        ),
        p(
            "Aparelho",
            "Brilho −",
            "lucide:sun-dim",
            DEVICE,
            ButtonAction::Brightness { delta: -10 },
        ),
        p(
            "Aparelho",
            "Relógio",
            "lucide:clock",
            DEVICE,
            ButtonAction::Clock,
        ),
        obs(ObsCommand::Record),
        obs(ObsCommand::Stream),
        obs(ObsCommand::Scene),
        obs(ObsCommand::Mute),
        obs(ObsCommand::RecordPause),
        obs(ObsCommand::VirtualCam),
        obs(ObsCommand::ReplaySave),
    ];
    // Calculadora, Bloco de notas e Explorador são programas do Windows (no Linux o nome muda
    // de uma distribuição para outra: o usuário escolhe em "Aplicativo..."), e Área de trabalho é o Win+D.
    #[cfg(not(windows))]
    let list = list
        .into_iter()
        .filter(|preset| !WINDOWS_ONLY.contains(&preset.label))
        .collect();
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_presets_resolve_to_real_ids() {
        let mut cfg = AppConfig::default();
        cfg.profiles[0].add_page("B");
        cfg.profiles[0].pages[1].id = 7;
        let p = presets()
            .into_iter()
            .find(|p| p.label == "Página 2")
            .unwrap();
        assert_eq!(
            p.resolved_action(&cfg),
            ButtonAction::SwitchPage { page: 7 }
        );
        let missing = presets()
            .into_iter()
            .find(|p| p.label == "Perfil 2")
            .unwrap();
        assert_eq!(
            missing.resolved_action(&cfg),
            ButtonAction::SwitchProfile { profile: 0 }
        );
    }

    /// No Linux o Super não tem significado padrão (Win+D, Win+L, Win+Shift+S são do Windows).
    #[cfg(not(windows))]
    #[test]
    fn linux_presets_do_not_send_windows_key_shortcuts() {
        for p in presets() {
            if let ButtonAction::Hotkey { keys } = &p.action {
                assert!(!keys.iter().any(|k| k == "meta"), "{}", p.label);
            }
        }
        assert!(presets().iter().all(|p| p.label != "Área de trabalho"));
    }

    #[test]
    fn every_preset_uses_a_known_icon() {
        for p in presets() {
            let known = !crate::graphics::icons::strokes(p.icon).is_empty()
                || crate::graphics::icon_library::find(p.icon).is_some();
            assert!(known, "{}: ícone {} desconhecido", p.label, p.icon);
        }
    }

    #[test]
    fn every_hotkey_and_media_preset_uses_parsable_keys() {
        for p in presets() {
            let keys = match &p.action {
                ButtonAction::Hotkey { keys } => keys.clone(),
                ButtonAction::MediaKey { key } => vec![key.clone()],
                _ => continue,
            };
            for k in keys {
                assert!(
                    crate::engine::actions::is_valid_key(&k),
                    "{}: tecla inválida {k}",
                    p.label
                );
            }
        }
    }
}
