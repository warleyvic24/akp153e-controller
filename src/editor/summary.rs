//! Resumo em uma linha do que a tecla faz ("Abre: Teams", "Atalho: Ctrl + M"...), para o inspetor.
use crate::config::schema::ButtonAction;
use crate::editor::action_ui::{media_label, Targets};
use crate::strings::{t, tf};

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
        ButtonAction::TypeText { text } if text.is_empty() => t("Digita um texto").to_string(),
        ButtonAction::TypeText { text } => tf("Digita: {}", &[&preview(text)]),
        ButtonAction::Delay { ms } => tf("Espera {} ms", &[ms]),
        ButtonAction::MultiAction { steps } if steps.is_empty() => {
            t("Multi-ação sem passos").to_string()
        }
        ButtonAction::MultiAction { steps } => tf("Multi-ação: {} passos", &[&steps.len()]),
        ButtonAction::NextPage => t("Vai para a próxima página").to_string(),
        ButtonAction::PreviousPage => t("Volta à página anterior").to_string(),
        ButtonAction::RunCommand { command } if command.trim().is_empty() => {
            t("Comando vazio").to_string()
        }
        ButtonAction::RunCommand { command } => tf("Roda: {}", &[&preview(command)]),
        ButtonAction::Brightness { delta } if *delta >= 0 => tf("Brilho +{}%", &[delta]),
        ButtonAction::Brightness { delta } => tf("Brilho {}%", &[delta]),
        ButtonAction::Clock => t("Relógio (hora e data)").to_string(),
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
    let file = path.rsplit(['/', '\\']).next().unwrap_or_default();
    let stem = match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file,
    };
    (!stem.is_empty()).then(|| stem.to_string())
}

const PREVIEW_CHARS: usize = 24;

/// Começo do texto digitado, em uma linha; o texto pode ser uma senha, então não vai inteiro.
fn preview(text: &str) -> String {
    let one_line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut shown: String = one_line.chars().take(PREVIEW_CHARS).collect();
    if one_line.chars().count() > PREVIEW_CHARS {
        shown.push('…');
    }
    shown
}

/// "ctrl", "shift", "m" vira "Ctrl + Shift + M".
pub(crate) fn chord(keys: &[String]) -> String {
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
    fn typed_text_is_shortened_and_kept_on_one_line() {
        let long = "uma senha muito longa\nem duas linhas";
        let text = summary(ButtonAction::TypeText { text: long.into() }).unwrap();
        assert!(!text.contains('\n'), "{text:?}");
        assert!(text.ends_with('…'), "{text:?}");
        assert!(!text.contains("duas linhas"), "{text:?}");
        let short = summary(ButtonAction::TypeText { text: "oi".into() }).unwrap();
        assert!(short.ends_with("oi") && !short.contains('…'), "{short}");
    }

    #[test]
    fn multi_action_and_delay_say_what_they_do() {
        let steps = vec![ButtonAction::NextPage, ButtonAction::Delay { ms: 10 }];
        let multi = summary(ButtonAction::MultiAction { steps }).unwrap();
        assert!(multi.contains('2'), "{multi}");
        let delay = summary(ButtonAction::Delay { ms: 750 }).unwrap();
        assert!(delay.contains("750"), "{delay}");
        assert!(summary(ButtonAction::NextPage).is_some());
        assert!(summary(ButtonAction::PreviousPage).is_some());
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
