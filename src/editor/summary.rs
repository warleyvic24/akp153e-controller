//! Resumo em uma linha do que a tecla faz ("Abre: Teams", "Atalho: Ctrl + M"...), para o inspetor.
use crate::config::schema::ButtonAction;
use crate::editor::action_ui::{media_label, Targets};
use crate::strings::{t, tf};
use std::path::Path;

/// Uma linha para o usuário; `None` quando a tecla não tem ação.
pub(crate) fn action_summary(a: &ButtonAction, targets: &Targets) -> Option<String> {
    let text = match a {
        ButtonAction::None => return None,
        ButtonAction::Hotkey { keys } if keys.is_empty() => t("Atalho vazio").to_string(),
        ButtonAction::Hotkey { keys } => tf("Atalho: {}", &[&chord(keys)]),
        ButtonAction::LaunchApp { path, .. } => match program_name(path) {
            Some(name) => tf("Abre: {}", &[&name]),
            None => t("Abre um programa").to_string(),
        },
        ButtonAction::HttpWebhook { url, method, .. } => match host(url) {
            Some(host) => tf("Web: {} {}", &[&method.to_uppercase(), &host]),
            None => t("Chama uma URL").to_string(),
        },
        ButtonAction::MediaKey { key } => tf("Mídia: {}", &[&t(media_label(key))]),
        ButtonAction::SwitchPage { page } => match targets.page_name(*page) {
            Some(name) => tf("Vai para a página {}", &[&name]),
            None => t("Troca de página").to_string(),
        },
        ButtonAction::SwitchProfile { profile } => match targets.profile_name(*profile) {
            Some(name) => tf("Vai para o perfil {}", &[&name]),
            None => t("Troca de perfil").to_string(),
        },
        ButtonAction::OpenMixer => t("Abre o mixer de volume").to_string(),
    };
    Some(text)
}

/// Nome curto do programa: "C:\...\Teams.exe" vira "Teams"; um app da Loja
/// ("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App") vira "WindowsCalculator".
pub(crate) fn program_name(path: &str) -> Option<String> {
    let path = path.trim().trim_matches('"');
    if path.is_empty() {
        return None;
    }
    if let Some((family, _)) = path.split_once('!') {
        let package = family.split('_').next().unwrap_or(family);
        return package.rsplit('.').next().map(str::to_string);
    }
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
}

/// "ctrl", "shift", "m" vira "Ctrl + Shift + M".
fn chord(keys: &[String]) -> String {
    keys.iter()
        .map(|k| {
            let mut chars = k.chars();
            chars.next().map_or(String::new(), |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Só o servidor da URL: sem usuário, senha, caminho ou consulta (podem ter token).
fn host(url: &str) -> Option<String> {
    let rest = url.trim().split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    (!host.is_empty()).then(|| host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::AppConfig;

    fn summary(a: ButtonAction) -> Option<String> {
        action_summary(&a, &Targets::from(&AppConfig::default()))
    }

    #[test]
    fn no_action_has_no_summary() {
        assert_eq!(summary(ButtonAction::None), None);
    }

    #[test]
    fn a_hotkey_reads_like_a_chord() {
        let keys = ["ctrl", "shift", "m"].map(String::from).to_vec();
        let text = summary(ButtonAction::Hotkey { keys }).unwrap();
        assert!(text.ends_with("Ctrl + Shift + M"), "{text}");
    }

    #[test]
    fn program_names_are_short() {
        assert_eq!(
            program_name(r#""C:\Program Files\Teams\Teams.exe""#).as_deref(),
            Some("Teams")
        );
        assert_eq!(program_name("notepad").as_deref(), Some("notepad"));
        let store = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";
        assert_eq!(program_name(store).as_deref(), Some("WindowsCalculator"));
        assert_eq!(program_name("  "), None);
    }

    #[test]
    fn a_webhook_shows_only_method_and_host() {
        let a = ButtonAction::HttpWebhook {
            url: "https://user:secret@hooks.io:8443/a/b?token=x".into(),
            method: "post".into(),
            body: None,
        };
        let text = summary(a).unwrap();
        assert!(text.ends_with("POST hooks.io:8443"), "{text}");
        assert!(
            !text.contains("secret") && !text.contains("token"),
            "{text}"
        );
    }

    #[test]
    fn navigation_names_the_target() {
        let cfg = AppConfig::default();
        let page = &cfg.profiles[0].pages[0];
        let text = action_summary(
            &ButtonAction::SwitchPage { page: page.id },
            &Targets::from(&cfg),
        )
        .unwrap();
        assert!(
            text.contains(&crate::strings::display_name(&page.name)),
            "{text}"
        );
    }
}
