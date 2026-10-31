//! Troca automática de perfil: um perfil com `auto_app` (ex.: `obs64.exe` ou `com.obsproject.Studio`)
//! entra sozinho quando aquele app ganha o foco, e o perfil de antes volta quando o foco sai dele,
//! a menos que o usuário tenha trocado de perfil à mão nesse meio-tempo (aí a escolha dele fica).
use super::runtime::{self, SharedState};
use crate::config::schema::AppConfig;
use crate::usb::UsbCmd;
use std::sync::mpsc::{Receiver, Sender};

/// Quantos apps recentes o serviço lembra para o editor oferecer como sugestão.
pub const RECENT_APPS: usize = 8;

/// Normaliza para comparar: sem maiúsculas, espaços e o `.exe` do Windows.
fn normalize(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// A regra (um ou mais apps separados por vírgula) casa com o app em foco?
pub fn rule_matches(rule: &str, app: &str) -> bool {
    let app = normalize(app);
    !app.is_empty() && rule.split(',').map(normalize).any(|r| r == app)
}

/// Perfil (índice) cuja regra casa com o app, se houver.
fn profile_for(cfg: &AppConfig, app: &str) -> Option<usize> {
    cfg.profiles.iter().position(|p| {
        p.auto_app
            .as_deref()
            .is_some_and(|rule| rule_matches(rule, app))
    })
}

/// O que lembrar entre um foco e outro.
#[derive(Default, Debug, PartialEq)]
pub struct AutoState {
    /// Perfil (id) ativo antes da troca automática, para onde voltar.
    home: Option<u64>,
    /// Perfil (id) que a troca automática pôs; se o ativo for outro, o usuário trocou à mão.
    switched_to: Option<u64>,
}

/// Decide a troca para o app em foco e atualiza `state`; devolve o índice do perfil a ativar.
pub fn decide(cfg: &AppConfig, app: &str, state: &mut AutoState) -> Option<usize> {
    let active_id = cfg.profiles.get(cfg.active_profile)?.id;
    if let Some(target) = profile_for(cfg, app) {
        let target_id = cfg.profiles[target].id;
        if target_id == active_id {
            return None;
        }
        // Indo de um perfil automático para outro, a volta continua sendo a de antes do primeiro.
        if state.switched_to != Some(active_id) {
            state.home = Some(active_id);
        }
        state.switched_to = Some(target_id);
        return Some(target);
    }
    let came_back = state.switched_to == Some(active_id);
    let home = state.home.take();
    state.switched_to = None;
    if !came_back {
        return None;
    }
    home.and_then(|id| cfg.profile_index(id))
}

/// Guarda `app` no início da lista de recentes, sem repetir e com tamanho limitado.
fn remember(recent: &mut Vec<String>, app: &str) {
    recent.retain(|a| a != app);
    recent.insert(0, app.to_string());
    recent.truncate(RECENT_APPS);
}

/// Thread do serviço: recebe o app em foco e troca o perfil quando uma regra manda.
pub fn spawn(state: SharedState, tx: Sender<UsbCmd>, focus: Receiver<String>) {
    std::thread::spawn(move || {
        let mut auto = AutoState::default();
        for app in focus {
            let target = {
                let mut sh = runtime::lock(&state);
                remember(&mut sh.recent_apps, &app);
                // Com o mixer aberto o aparelho mostra outra coisa: trocar o perfil por baixo confundiria.
                if sh.mixer.is_some() {
                    None
                } else {
                    decide(&sh.cfg, &app, &mut auto)
                }
            };
            if let Some(index) = target {
                log::info!("perfil automático: {app}");
                runtime::switch_profile(&state, &tx, index);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AppConfig {
        let mut cfg = AppConfig::default();
        cfg.add_profile("OBS");
        cfg.add_profile("Jogo");
        cfg.profiles[1].auto_app = Some("obs64.exe, com.obsproject.Studio".into());
        cfg.profiles[2].auto_app = Some("steam".into());
        cfg
    }

    #[test]
    fn rules_ignore_case_spaces_and_the_exe_suffix() {
        assert!(rule_matches("obs64.exe", "OBS64.EXE"));
        assert!(rule_matches("obs64", "obs64.exe"));
        assert!(rule_matches(
            "firefox, com.obsproject.Studio",
            "com.obsproject.studio"
        ));
        assert!(!rule_matches("obs", "obs64.exe"));
        assert!(!rule_matches("", ""));
        assert!(!rule_matches(" , ", "x"));
    }

    #[test]
    fn focusing_the_app_switches_and_leaving_it_returns() {
        let mut cfg = config();
        let mut state = AutoState::default();
        assert_eq!(decide(&cfg, "com.obsproject.Studio", &mut state), Some(1));
        cfg.active_profile = 1;
        assert_eq!(
            decide(&cfg, "com.obsproject.Studio", &mut state),
            None,
            "já está nele"
        );
        assert_eq!(
            decide(&cfg, "firefox", &mut state),
            Some(0),
            "volta ao de antes"
        );
        cfg.active_profile = 0;
        assert_eq!(decide(&cfg, "kitty", &mut state), None);
        assert_eq!(state, AutoState::default());
    }

    #[test]
    fn hopping_between_automatic_profiles_still_returns_home() {
        let mut cfg = config();
        let mut state = AutoState::default();
        assert_eq!(decide(&cfg, "obs64.exe", &mut state), Some(1));
        cfg.active_profile = 1;
        assert_eq!(decide(&cfg, "steam", &mut state), Some(2));
        cfg.active_profile = 2;
        assert_eq!(decide(&cfg, "explorer.exe", &mut state), Some(0));
    }

    #[test]
    fn a_manual_switch_is_kept_when_the_app_loses_focus() {
        let mut cfg = config();
        let mut state = AutoState::default();
        assert_eq!(decide(&cfg, "obs64.exe", &mut state), Some(1));
        // O usuário trocou para o perfil 2 pelo aparelho enquanto o OBS estava na frente.
        cfg.active_profile = 2;
        assert_eq!(decide(&cfg, "explorer.exe", &mut state), None);
    }

    #[test]
    fn recent_apps_are_unique_newest_first_and_bounded() {
        let mut recent = Vec::new();
        for app in ["a", "b", "a", "c"] {
            remember(&mut recent, app);
        }
        assert_eq!(recent, ["c", "a", "b"]);
        for i in 0..20 {
            remember(&mut recent, &i.to_string());
        }
        assert_eq!(recent.len(), RECENT_APPS);
    }
}
