//! Programas instalados no Linux: lê os atalhos `.desktop` (a mesma lista do menu de aplicativos).
//! O ícone vem do `Icon=` do atalho, procurado no tema de ícones (`icon_theme`).
use std::path::PathBuf;

/// Programa detectado. `target` é o comando do atalho (programa e argumentos).
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledApp {
    pub name: String,
    pub target: String,
    /// Só existe no Windows (apps da Loja); aqui é sempre `false`.
    pub uwp: bool,
    /// `Icon=` do atalho: nome no tema de ícones ou caminho absoluto.
    pub icon: Option<String>,
}

impl InstalledApp {
    pub fn exe(name: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            target: path.to_string(),
            uwp: false,
            icon: None,
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

/// Lista os programas dos atalhos `.desktop`, em ordem alfabética.
///
/// Como manda a especificação XDG, um atalho é identificado pelo nome do arquivo, e a pasta mais
/// prioritária vence, mesmo que a cópia dela esteja escondida (`Hidden`/`NoDisplay`): é assim que
/// o usuário tira um app do menu.
pub fn scan() -> Vec<InstalledApp> {
    scan_dirs(&desktop_dirs())
}

/// `scan` sobre pastas dadas, da mais prioritária para a menos.
fn scan_dirs(dirs: &[PathBuf]) -> Vec<InstalledApp> {
    let mut seen = std::collections::HashSet::new();
    let mut apps: Vec<InstalledApp> = dirs
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flat_map(|entries| {
            let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            paths
        })
        .filter(|path| path.extension().is_some_and(|ext| ext == "desktop"))
        .filter(|path| {
            path.file_name()
                .is_some_and(|id| seen.insert(id.to_os_string()))
        })
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|text| parse_desktop(&text))
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
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
    let (mut name, mut exec, mut icon) = (None, None, None);
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
            "Exec" => exec = Some(strip_field_codes(&unescape_value(value.trim()))),
            "Icon" => icon = Some(value.trim().to_string()).filter(|i| !i.is_empty()),
            "NoDisplay" | "Hidden" => hidden |= value.trim() == "true",
            _ => {}
        }
    }
    let (name, exec) = (name?, exec?);
    (is_app && !hidden && !name.is_empty() && !exec.is_empty()).then(|| InstalledApp {
        icon,
        ..InstalledApp::exe(&name, &exec)
    })
}

/// Desfaz os escapes de valor do formato `.desktop` (`\s`, `\n`, `\t`, `\r`, `\\`). É a primeira
/// das duas camadas de escape do `Exec`; a segunda (aspas) fica para `split_words`.
fn unescape_value(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Tira os códigos de campo do `Exec` (`%f`, `%U`...), que o menu preencheria com arquivos.
/// Os espaços ficam como estão: dentro de aspas eles fazem parte do argumento.
fn strip_field_codes(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // `%%` é um % literal; qualquer outro `%x` é código de campo e some.
        if chars.next() == Some('%') {
            out.push('%');
        }
    }
    out.trim().to_string()
}

/// Divide um comando em palavras respeitando aspas simples e duplas (sem expandir nada).
fn split_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            // Dentro de aspas duplas, `\"`, `\``, `\$` e `\\` valem o caractere em si.
            (Some('"'), '\\') => match chars.next() {
                Some(next @ ('"' | '`' | '$' | '\\')) => current.push(next),
                Some(next) => {
                    current.push('\\');
                    current.push(next);
                }
                None => current.push('\\'),
            },
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

/// Acha o ícone do app no tema de ícones, guarda como PNG ao lado da configuração e devolve o
/// caminho. Sem `Icon=` (programa escolhido à mão, app do mixer), tenta o nome do programa, que
/// costuma ser o nome do ícone (`firefox`, `spotify`).
pub fn extract_icon(app: &InstalledApp) -> Option<String> {
    let name = app.icon.clone().or_else(|| {
        let (program, _) = app.launch();
        let file = std::path::Path::new(&program)
            .file_name()?
            .to_str()?
            .to_string();
        Some(file)
    })?;
    let file = super::icon_theme::find(&name)?;
    let cached = super::icon_dir()?.join(super::icon_file_name(&file.display().to_string()));
    if cached.is_file() {
        return Some(cached.display().to_string());
    }
    let bytes = std::fs::read(&file).ok()?;
    let saved = super::save_icon(&file.display().to_string(), &bytes);
    if saved.is_none() {
        log::warn!(
            "ícone de {} não pôde ser lido: {}",
            app.name,
            file.display()
        );
    }
    saved
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

    /// Usa o tema de ícones de verdade (a pasta de ícones dos testes é temporária):
    /// `cargo test extracts_real_icons -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn extracts_real_icons() {
        for app in scan().iter().filter(|a| a.icon.is_some()).take(40) {
            println!("{} -> {:?}", app.name, extract_icon(app));
        }
        let by_program = InstalledApp::exe("btop", "btop");
        println!("btop (só o programa) -> {:?}", extract_icon(&by_program));
    }

    #[test]
    fn reads_the_icon_name() {
        let text =
            "[Desktop Entry]\nType=Application\nName=Chrome\nExec=chrome\nIcon=google-chrome\n";
        assert_eq!(
            parse_desktop(text).unwrap().icon.as_deref(),
            Some("google-chrome")
        );
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
    fn a_hidden_user_copy_hides_the_system_app_and_same_names_stay_apart() {
        let root = std::env::temp_dir().join(format!("akp153e-apps-{}", std::process::id()));
        let (user, system) = (root.join("user"), root.join("system"));
        for dir in [&user, &system] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let entry = |name: &str, exec: &str, extra: &str| {
            format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n{extra}")
        };
        std::fs::write(
            user.join("game.desktop"),
            entry("Game", "game", "Hidden=true"),
        )
        .unwrap();
        std::fs::write(system.join("game.desktop"), entry("Game", "game", "")).unwrap();
        std::fs::write(system.join("kitty.desktop"), entry("Terminal", "kitty", "")).unwrap();
        std::fs::write(system.join("foot.desktop"), entry("Terminal", "foot", "")).unwrap();
        let apps = scan_dirs(&[user, system]);
        let _ = std::fs::remove_dir_all(&root);
        let targets: Vec<&str> = apps.iter().map(|a| a.target.as_str()).collect();
        assert_eq!(targets, vec!["foot", "kitty"]);
    }

    #[test]
    fn launch_splits_program_and_arguments() {
        let app = InstalledApp::exe("Code", r#"/usr/bin/code --new-window "my folder""#);
        let (program, args) = app.launch();
        assert_eq!(program, "/usr/bin/code");
        assert_eq!(args, vec!["--new-window", "my folder"]);
    }

    #[test]
    fn spaces_and_escapes_inside_quotes_are_kept() {
        let text = "[Desktop Entry]\nType=Application\nName=A\nExec=\"/opt/My  App/run\" --x \"a\\\\$b \\\\\"c\\\\\"\" %U\n";
        let (program, args) = parse_desktop(text).unwrap().launch();
        assert_eq!(program, "/opt/My  App/run");
        assert_eq!(args, vec!["--x", "a$b \"c\""]);
    }

    #[test]
    fn a_value_escape_turns_into_a_space() {
        assert_eq!(unescape_value(r"a\sb\\c"), r"a b\c");
    }

    #[test]
    fn percent_percent_is_a_literal_percent() {
        assert_eq!(strip_field_codes("a %% b %f"), "a % b");
    }
}
