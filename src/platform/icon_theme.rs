//! Acha o arquivo de um ícone do Linux pelo nome (`Icon=firefox` dos atalhos `.desktop`), nas
//! pastas da especificação de temas de ícones do freedesktop. `hicolor` vem primeiro: é onde os
//! programas instalam o ícone deles; os demais temas e `/usr/share/pixmaps` só cobrem o que faltar.
use std::path::{Path, PathBuf};

const FALLBACK_THEME: &str = "hicolor";
/// Abaixo disto um PNG fica borrado na tecla; o SVG ganha dele.
const SHARP_PX: u32 = 64;
const EXTENSIONS: [&str; 3] = ["png", "svg", "xpm"];

/// Caminho do melhor arquivo para `name`. Um caminho absoluto é usado como está.
pub fn find(name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let as_path = Path::new(name);
    if as_path.is_absolute() {
        return as_path.is_file().then(|| as_path.to_path_buf());
    }
    find_in(name, &base_dirs())
}

/// Pastas `icons` (e `pixmaps`) do usuário e do sistema, na ordem da especificação.
fn base_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    let mut dirs: Vec<PathBuf> = home.iter().map(|h| h.join(".icons")).collect();
    dirs.extend(data_home.map(|d| d.join("icons")));
    dirs.extend(
        data_dirs
            .split(':')
            .filter(|d| !d.is_empty())
            .map(|d| Path::new(d).join("icons")),
    );
    dirs.push(PathBuf::from("/usr/share/pixmaps"));
    dirs
}

/// Busca em `bases`: primeiro no `hicolor`, depois em qualquer outro tema, por último solto na
/// pasta (como em `/usr/share/pixmaps`).
fn find_in(name: &str, bases: &[PathBuf]) -> Option<PathBuf> {
    let in_theme = |theme_filter: &dyn Fn(&str) -> bool| {
        bases
            .iter()
            .filter_map(|base| std::fs::read_dir(base).ok())
            .flatten()
            .flatten()
            .filter(|theme| theme.file_name().to_str().is_some_and(theme_filter))
            .filter_map(|theme| best_in_theme(&theme.path(), name))
            .max_by_key(|c| c.score)
            .map(|c| c.path)
    };
    in_theme(&|t| t == FALLBACK_THEME)
        .or_else(|| in_theme(&|t| t != FALLBACK_THEME))
        .or_else(|| {
            bases.iter().find_map(|base| {
                EXTENSIONS
                    .iter()
                    .map(|ext| base.join(format!("{name}.{ext}")))
                    .find(|p| p.is_file())
            })
        })
}

struct Candidate {
    path: PathBuf,
    score: u32,
}

/// O maior PNG nítido do tema; sem ele, o SVG; sem os dois, o maior PNG que houver.
fn best_in_theme(theme: &Path, name: &str) -> Option<Candidate> {
    std::fs::read_dir(theme)
        .ok()?
        .flatten()
        .filter_map(|size_dir| {
            let size = size_dir.file_name().to_str().map(size_of_dir)?;
            ["apps", "applications"].iter().find_map(|context| {
                EXTENSIONS.iter().find_map(|ext| {
                    let path = size_dir.path().join(context).join(format!("{name}.{ext}"));
                    path.is_file().then(|| Candidate {
                        score: score(size, ext),
                        path,
                    })
                })
            })
        })
        .max_by_key(|c| c.score)
}

/// Tamanho de uma pasta de tema: "256x256" → 256, "48x48@2x" → 96, "scalable" → `None` (vetor).
fn size_of_dir(dir: &str) -> Option<u32> {
    let (dims, scale) = match dir.split_once('@') {
        Some((dims, scale)) => (dims, scale.trim_end_matches('x').parse().unwrap_or(1)),
        None => (dir, 1),
    };
    let side: u32 = dims.split('x').next()?.parse().ok()?;
    Some(side * scale)
}

/// Quanto melhor, maior: PNG nítido > SVG > PNG pequeno > XPM.
fn score(size: Option<u32>, ext: &str) -> u32 {
    const SVG: u32 = 10_000;
    match (size, ext) {
        (_, "xpm") => 1,
        (_, "svg") => SVG,
        (Some(px), _) if px >= SHARP_PX => SVG + px,
        (Some(px), _) => px,
        (None, _) => SVG,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn theme_folder_names_give_the_icon_size() {
        assert_eq!(size_of_dir("256x256"), Some(256));
        assert_eq!(size_of_dir("48x48@2x"), Some(96));
        assert_eq!(size_of_dir("scalable"), None);
    }

    #[test]
    fn picks_the_sharpest_hicolor_icon_then_svg_then_other_themes() {
        let root = std::env::temp_dir().join(format!("akp153e-icons-{}", std::process::id()));
        let icons = root.join("icons");
        touch(&icons.join("hicolor/16x16/apps/a.png"));
        touch(&icons.join("hicolor/256x256/apps/a.png"));
        touch(&icons.join("hicolor/scalable/apps/a.svg"));
        touch(&icons.join("hicolor/16x16/apps/b.png"));
        touch(&icons.join("hicolor/scalable/apps/b.svg"));
        touch(&icons.join("Yaru/256x256/apps/c.png"));
        touch(&root.join("pixmaps/d.png"));
        let bases = [icons.clone(), root.join("pixmaps")];
        let found = |n| find_in(n, &bases).map(|p| p.strip_prefix(&root).unwrap().to_path_buf());
        let result = (found("a"), found("b"), found("c"), found("d"), found("x"));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            result.0.unwrap(),
            Path::new("icons/hicolor/256x256/apps/a.png")
        );
        assert_eq!(
            result.1.unwrap(),
            Path::new("icons/hicolor/scalable/apps/b.svg")
        );
        assert_eq!(
            result.2.unwrap(),
            Path::new("icons/Yaru/256x256/apps/c.png")
        );
        assert_eq!(result.3.unwrap(), Path::new("pixmaps/d.png"));
        assert!(result.4.is_none());
    }
}
