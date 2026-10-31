//! O que o editor lembra entre aberturas e que não é configuração do aparelho: as ações usadas
//! por último (seção "Recentes" da biblioteca) e quantas vezes o editor abriu (as dicas de atalho
//! somem depois das primeiras). Fica em `editor.json`, ao lado da config, e um arquivo ilegível só
//! faz o editor começar do zero.
use crate::config::schema::AppConfig;
use serde::{Deserialize, Serialize};

const FILE: &str = "editor.json";
/// Quantas ações recentes a biblioteca mostra.
pub(super) const MAX_RECENT: usize = 6;
/// Até esta abertura a barra de atalhos aparece embaixo do aparelho; depois, só o "?".
pub(super) const HINT_OPENS: u32 = 3;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Memory {
    pub opens: u32,
    /// Nome (em português, a chave do preset) das ações aplicadas, a mais nova primeiro.
    pub recent: Vec<String>,
}

impl Memory {
    fn path() -> Option<std::path::PathBuf> {
        AppConfig::path().map(|p| p.with_file_name(FILE))
    }

    /// Lê a memória e conta esta abertura.
    pub fn open() -> Self {
        let mut m: Self = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        m.opens = m.opens.saturating_add(1);
        m.save();
        m
    }

    /// Põe a ação no topo dos recentes (sem repetir) e grava.
    pub fn used(&mut self, preset: &str) {
        self.recent.retain(|r| r != preset);
        self.recent.insert(0, preset.to_string());
        self.recent.truncate(MAX_RECENT);
        self.save();
    }

    pub fn shows_hints(&self) -> bool {
        self.opens <= HINT_OPENS
    }

    fn save(&self) {
        let Some(path) = Self::path() else { return };
        let Ok(json) = serde_json::to_string_pretty(self) else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, json) {
            log::warn!("memória do editor não gravada: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_actions_go_to_the_top_without_repeating_and_stay_few() {
        let mut m = Memory::default();
        for p in ["A", "B", "C", "A", "D", "E", "F", "G"] {
            m.used(p);
        }
        assert_eq!(m.recent, ["G", "F", "E", "D", "A", "C"]);
    }

    #[test]
    fn hints_show_only_in_the_first_openings() {
        let mut m = Memory {
            opens: HINT_OPENS,
            ..Memory::default()
        };
        assert!(m.shows_hints());
        m.opens += 1;
        assert!(!m.shows_hints());
    }
}
