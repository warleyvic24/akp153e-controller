//! Diz o que falta numa tecla para ela funcionar (atalho vazio, programa ausente, URL inválida...).
use crate::config::schema::{ButtonAction, ButtonConfig};
use crate::editor::action_ui::Targets;
use crate::engine;

/// Motivo de "Testar" ficar desligado numa tecla sem ação.
pub(crate) const NO_ACTION: &str = "Esta tecla ainda não tem ação";

/// Primeiro problema da tecla, em texto para o usuário; `None` se ela pode ser usada.
pub(crate) fn button_problem(b: &ButtonConfig, targets: &Targets) -> Option<&'static str> {
    if let Some(problem) = action_problem(&b.action, targets) {
        return Some(problem);
    }
    match &b.long_press_action {
        Some(ButtonAction::None) => Some("Escolha a ação ao segurar, ou desative-a"),
        Some(long) => action_problem(long, targets),
        None => None,
    }
}

/// Problema de uma ação. "Nenhuma" não é problema: a tecla só não faz nada.
pub(crate) fn action_problem(a: &ButtonAction, targets: &Targets) -> Option<&'static str> {
    match a {
        ButtonAction::Hotkey { keys } if keys.is_empty() => {
            Some("Atalho vazio: grave ou digite as teclas")
        }
        ButtonAction::Hotkey { keys } if !keys.iter().all(|k| engine::actions::is_valid_key(k)) => {
            Some("O atalho tem uma tecla desconhecida")
        }
        ButtonAction::Obs { command, target }
            if command.needs_target() && target.trim().is_empty() =>
        {
            Some("Escolha a cena ou a entrada do OBS")
        }
        ButtonAction::RunCommand { command } if command.trim().is_empty() => {
            Some("Digite o comando que a tecla roda")
        }
        ButtonAction::LaunchApp { path, .. } if path.trim().is_empty() => {
            Some("Escolha o programa que a tecla abre")
        }
        ButtonAction::HttpWebhook { url, .. } if !is_http_url(url) => {
            Some("A URL precisa começar com http:// ou https://")
        }
        ButtonAction::SwitchPage { page } if !targets.has_page(*page) => {
            Some("Escolha a página de destino")
        }
        ButtonAction::SwitchProfile { profile } if !targets.has_profile(*profile) => {
            Some("Escolha o perfil de destino")
        }
        _ => None,
    }
}

fn is_http_url(url: &str) -> bool {
    let url = url.trim().to_ascii_lowercase();
    ["http://", "https://"]
        .iter()
        .any(|scheme| url.len() > scheme.len() && url.starts_with(scheme))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::AppConfig;

    fn targets() -> Targets {
        Targets::from(&AppConfig::default())
    }

    fn hotkey(keys: &[&str]) -> ButtonAction {
        ButtonAction::Hotkey {
            keys: keys.iter().map(|k| k.to_string()).collect(),
        }
    }

    fn webhook(url: &str) -> ButtonAction {
        ButtonAction::HttpWebhook {
            url: url.into(),
            method: "GET".into(),
            body: None,
        }
    }

    #[test]
    fn an_empty_or_unknown_hotkey_is_a_problem() {
        let t = targets();
        assert!(action_problem(&hotkey(&[]), &t).is_some());
        assert!(action_problem(&hotkey(&["ctrl", "nada"]), &t).is_some());
        assert_eq!(action_problem(&hotkey(&["ctrl", "c"]), &t), None);
    }

    #[test]
    fn a_webhook_needs_an_http_url() {
        let t = targets();
        assert!(action_problem(&webhook(""), &t).is_some());
        assert!(action_problem(&webhook("https://"), &t).is_some());
        assert!(action_problem(&webhook("ftp://host"), &t).is_some());
        assert_eq!(action_problem(&webhook(" HTTPS://host/a "), &t), None);
    }

    #[test]
    fn a_launch_needs_a_program() {
        let t = targets();
        let empty = ButtonAction::LaunchApp {
            path: "  ".into(),
            args: vec![],
        };
        assert!(action_problem(&empty, &t).is_some());
        let notepad = ButtonAction::LaunchApp {
            path: "notepad".into(),
            args: vec![],
        };
        assert_eq!(action_problem(&notepad, &t), None);
    }

    #[test]
    fn navigation_needs_an_existing_target() {
        let cfg = AppConfig::default();
        let t = Targets::from(&cfg);
        let page = cfg.profiles[0].pages[0].id;
        let missing = u64::MAX;
        assert_eq!(action_problem(&ButtonAction::SwitchPage { page }, &t), None);
        let gone = ButtonAction::SwitchPage { page: missing };
        assert!(action_problem(&gone, &t).is_some());
        let gone = ButtonAction::SwitchProfile { profile: missing };
        assert!(action_problem(&gone, &t).is_some());
    }

    #[test]
    fn every_message_has_an_english_translation() {
        let t = targets();
        let missing = u64::MAX;
        let broken = [
            hotkey(&[]),
            hotkey(&["nada"]),
            webhook(""),
            ButtonAction::LaunchApp {
                path: String::new(),
                args: vec![],
            },
            ButtonAction::SwitchPage { page: missing },
            ButtonAction::SwitchProfile { profile: missing },
        ];
        let mut b = ButtonConfig::blank();
        b.long_press_action = Some(ButtonAction::None);
        let messages = broken
            .iter()
            .filter_map(|a| action_problem(a, &t))
            .chain(button_problem(&b, &t))
            .chain([NO_ACTION]);
        for msg in messages {
            assert!(crate::strings::has_translation(msg), "sem tradução: {msg}");
        }
    }

    #[test]
    fn no_action_is_fine_but_an_enabled_empty_long_press_is_not() {
        let t = targets();
        let mut b = ButtonConfig::blank();
        assert_eq!(button_problem(&b, &t), None);
        b.long_press_action = Some(ButtonAction::None);
        assert!(button_problem(&b, &t).is_some());
        b.long_press_action = Some(hotkey(&[]));
        assert!(button_problem(&b, &t).is_some());
        b.long_press_action = Some(hotkey(&["f5"]));
        assert_eq!(button_problem(&b, &t), None);
    }
}
