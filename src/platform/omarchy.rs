//! Onde o Omarchy (Arch + Hyprland) guarda o tema ativo. Só lê arquivos; quem interpreta as cores
//! é `editor::omarchy`. No Windows nada disto existe e `read_colors` devolve `None`.
use std::path::{Path, PathBuf};

const COLORS_FILE: &str = "colors.toml";

/// O texto do `colors.toml` do tema ativo, se houver um.
pub fn read_colors() -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    candidate_dirs(state.as_deref(), home.as_deref())
        .into_iter()
        .find_map(|dir| std::fs::read_to_string(dir.join(COLORS_FILE)).ok())
}

/// Pastas do tema ativo, da mais nova para a mais antiga: o Omarchy atual guarda em
/// `~/.local/state/omarchy/current/theme`; versões anteriores, em `~/.config/omarchy/current/theme`.
fn candidate_dirs(state_home: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(state) = state_home {
        dirs.push(state.join("omarchy/current/theme"));
    }
    if let Some(home) = home {
        dirs.push(home.join(".local/state/omarchy/current/theme"));
        dirs.push(home.join(".config/omarchy/current/theme"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_directory_comes_before_the_old_config_one() {
        let dirs = candidate_dirs(Some(Path::new("/s")), Some(Path::new("/h")));
        assert_eq!(dirs[0], Path::new("/s/omarchy/current/theme"));
        assert_eq!(dirs[1], Path::new("/h/.local/state/omarchy/current/theme"));
        assert_eq!(dirs[2], Path::new("/h/.config/omarchy/current/theme"));
    }

    #[test]
    fn without_a_home_there_is_nowhere_to_look() {
        assert!(candidate_dirs(None, None).is_empty());
    }
}
