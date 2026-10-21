use crate::config::schema::{AppConfig, ButtonAction};

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
/// Cores oferecidas no seletor do inspetor (a última é o grafite neutro).
pub const SWATCHES: [[u8; 3]; 7] = [MEDIA, KEYS, APPS, WEB, NAV, [42, 47, 58], [0, 0, 0]];

fn hotkey(keys: &[&str]) -> ButtonAction {
    ButtonAction::Hotkey {
        keys: keys.iter().map(|k| k.to_string()).collect(),
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

/// Pelo Explorer, não pelo `cmd /C start`: o `cmd` trata `&` da URL como separador de comandos.
fn open_url(url: &str) -> ButtonAction {
    ButtonAction::LaunchApp {
        path: "explorer".into(),
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

pub fn presets() -> Vec<Preset> {
    let p = |category, label, icon, color, action| Preset {
        category,
        label,
        icon,
        color,
        action,
    };
    vec![
        p("Mídia", "Play/Pause", "play", MEDIA, media("playpause")),
        p("Mídia", "Próxima", "next", MEDIA, media("next")),
        p("Mídia", "Anterior", "prev", MEDIA, media("prev")),
        p("Mídia", "Vol +", "volup", MEDIA, media("volup")),
        p("Mídia", "Vol -", "voldown", MEDIA, media("voldown")),
        p("Mídia", "Mudo", "mute", MEDIA, media("mute")),
        p("Mídia", "Mixer", "volup", MEDIA, ButtonAction::OpenMixer),
        p("Atalhos", "Copiar", "copy", KEYS, hotkey(&["ctrl", "c"])),
        p("Atalhos", "Colar", "paste", KEYS, hotkey(&["ctrl", "v"])),
        p("Atalhos", "Recortar", "bolt", KEYS, hotkey(&["ctrl", "x"])),
        p("Atalhos", "Desfazer", "bolt", KEYS, hotkey(&["ctrl", "z"])),
        p("Atalhos", "Salvar", "bolt", KEYS, hotkey(&["ctrl", "s"])),
        p(
            "Atalhos",
            "Captura",
            "bolt",
            KEYS,
            hotkey(&["meta", "shift", "s"]),
        ),
        p(
            "Atalhos",
            "Área de trabalho",
            "page",
            KEYS,
            hotkey(&["meta", "d"]),
        ),
        p("Atalhos", "Bloquear", "lock", KEYS, hotkey(&["meta", "l"])),
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
        p("Web", "Abrir site", "globe", WEB, open_url("https://")),
        p(
            "Web",
            "Webhook",
            "bolt",
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
    ]
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

    #[test]
    fn every_preset_uses_a_known_icon() {
        for p in presets() {
            assert!(
                !crate::icons::strokes(p.icon).is_empty(),
                "{}: ícone {} desconhecido",
                p.label,
                p.icon
            );
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
