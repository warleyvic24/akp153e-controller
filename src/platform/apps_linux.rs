//! Programas instalados no Linux: lê os atalhos `.desktop` (a mesma lista do menu de aplicativos).
//! Ícones não são extraídos; a tecla mostra o nome do app.
use std::path::{Path, PathBuf};

/// Programa detectado. `target` é o comando do atalho (programa e argumentos).
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledApp {
    pub name: String,
    pub target: String,
    /// Só existe no Windows (apps da Loja); aqui é sempre `false`.
    pub uwp: bool,
}

impl InstalledApp {
    pub fn exe(name: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            target: path.to_string(),
            uwp: false,
        }
    }

    /// Programa e argumentos que a ação de abrir app deve usar.
    pub fn launch(&self) -> (String, Vec<String>) {
        let mut words = split_words(&self.target);
        if words.is_empty() {
            return (self.target.clone(), Vec::new());
        }
        let program = words.remove(0);
        (program, words)
    }
}

/// Lista os programas dos atalhos `.desktop`, em ordem alfabética e sem repetir nome.
pub fn scan() -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = desktop_dirs()
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "desktop"))
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|text| parse_desktop(&text))
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    apps
}

fn desktop_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    let mut dirs: Vec<PathBuf> = data_home.into_iter().collect();
    dirs.extend(
        data_dirs
            .split(':')
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    );
    // Flatpak e Snap guardam seus atalhos fora das pastas padrão.
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    dirs.extend(home.map(|h| h.join(".local/share/flatpak/exports/share")));
    dirs.push(PathBuf::from("/var/lib/snapd/desktop"));
    dirs.into_iter().map(|d| d.join("applications")).collect()
}

/// Lê um arquivo `.desktop`. `None` para o que não é um programa visível no menu.
pub fn parse_desktop(text: &str) -> Option<InstalledApp> {
    let mut in_entry = false;
    let (mut name, mut exec) = (None, None);
    let (mut is_app, mut hidden) = (false, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Type" => is_app = value.trim() == "Application",
            "Name" => name = Some(value.trim().to_string()),
            "Exec" => exec = Some(strip_field_codes(value.trim())),
            "NoDisplay" | "Hidden" => hidden |= value.trim() == "true",
            _ => {}
        }
    }
    let (name, exec) = (name?, exec?);
    (is_app && !hidden && !name.is_empty() && !exec.is_empty())
        .then(|| InstalledApp::exe(&name, &exec))
}

/// Tira os códigos de campo do `Exec` (`%f`, `%U`...), que o menu preencheria com arquivos.
fn strip_field_codes(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Divide um comando em palavras respeitando aspas simples e duplas (sem expandir nada).
fn split_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in command.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, c) => current.push(c),
        }
    }
    if started || !current.is_empty() {
        words.push(current);
    }
    words
}

/// No Linux os ícones dos apps não são extraídos.
pub fn extract_icon(_app: &InstalledApp) -> Option<String> {
    None
}

/// Nome do arquivo de ícone de um programa; mantido por compatibilidade com o Windows.
#[allow(dead_code)]
pub fn icon_file_name(target: &str) -> String {
    Path::new(target).file_name().map_or_else(
        || "app.png".to_string(),
        |n| format!("{}.png", n.to_string_lossy()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "[Desktop Entry]\nType=Application\nName=Firefox\nName[pt_BR]=Navegador\nExec=firefox %u\n\n[Desktop Action new-window]\nName=Nova janela\nExec=firefox --new-window\n";

    #[test]
    fn reads_name_and_command_without_field_codes() {
        let app = parse_desktop(FIREFOX).unwrap();
        assert_eq!(app, InstalledApp::exe("Firefox", "firefox"));
    }

    #[test]
    fn hidden_and_non_application_entries_are_skipped() {
        assert!(
            parse_desktop("[Desktop Entry]\nType=Application\nName=A\nExec=a\nNoDisplay=true")
                .is_none()
        );
        assert!(parse_desktop("[Desktop Entry]\nType=Link\nName=A\nExec=a").is_none());
        assert!(parse_desktop("[Desktop Entry]\nType=Application\nName=A").is_none());
    }

    #[test]
    fn launch_splits_program_and_arguments() {
        let app = InstalledApp::exe("Code", r#"/usr/bin/code --new-window "my folder""#);
        let (program, args) = app.launch();
        assert_eq!(program, "/usr/bin/code");
        assert_eq!(args, vec!["--new-window", "my folder"]);
    }

    #[test]
    fn percent_percent_is_a_literal_percent() {
        assert_eq!(strip_field_codes("a %% b %f"), "a % b");
    }
}
