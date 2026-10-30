//! Integração com o sistema (Windows e Linux). Todo `unsafe` do projeto fica aqui, sempre com `// SAFETY:`.
//! `apps` e `audio` têm uma implementação por sistema, com a mesma interface; `shell` é um arquivo só,
//! com `cfg` nos pontos que diferem (janela, bandeja, início automático).
use std::process::Command;

#[cfg(windows)]
pub(crate) mod apps;
#[cfg(not(windows))]
#[path = "apps_linux.rs"]
pub(crate) mod apps;

#[cfg(windows)]
pub(crate) mod audio;
#[cfg(not(windows))]
#[path = "audio_linux.rs"]
pub(crate) mod audio;

mod audio_model;
#[cfg(not(windows))]
mod icon_theme;
pub(crate) mod omarchy;

/// Lado do PNG guardado para ícones de apps e sites (a tecla tem 100 px; sobra para o mixer).
pub(crate) const ICON_PX: u32 = 256;

/// Pasta dos ícones extraídos, ao lado da configuração.
pub(crate) fn icon_dir() -> Option<std::path::PathBuf> {
    crate::config::schema::AppConfig::path().and_then(|p| p.parent().map(|d| d.join("icons")))
}

/// Nome do PNG de um ícone: derivado da origem (caminho do programa, nome do ícone ou site).
pub fn icon_file_name(source: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.to_lowercase().hash(&mut hasher);
    format!("{:016x}.png", hasher.finish())
}

/// Converte um ícone (PNG, ICO, JPEG ou SVG) num PNG quadrado de `ICON_PX`, centralizado e com
/// fundo transparente. `None` se os bytes não forem uma imagem que dê para ler.
pub(crate) fn icon_image(bytes: &[u8]) -> Option<image::RgbaImage> {
    let picture = if looks_like_svg(bytes) {
        svg_image(bytes)?
    } else {
        image::load_from_memory(bytes).ok()?.to_rgba8()
    };
    let fitted = image::DynamicImage::ImageRgba8(picture)
        .resize(ICON_PX, ICON_PX, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut square = image::RgbaImage::new(ICON_PX, ICON_PX);
    let x = i64::from((ICON_PX - fitted.width()) / 2);
    let y = i64::from((ICON_PX - fitted.height()) / 2);
    image::imageops::overlay(&mut square, &fitted, x, y);
    Some(square)
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
    head.contains("<svg")
}

/// Desenha o SVG em `ICON_PX`, mantendo a proporção.
fn svg_image(bytes: &[u8]) -> Option<image::RgbaImage> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = ICON_PX as f32 / size.width().max(size.height());
    let (w, h) = (
        (size.width() * scale).ceil().max(1.0) as u32,
        (size.height() * scale).ceil().max(1.0) as u32,
    );
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // O tiny-skia guarda cores pré-multiplicadas; o PNG quer as normais.
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(w, h, pixels)
}

/// Guarda o ícone de `source` como PNG na pasta de ícones e devolve o caminho.
pub(crate) fn save_icon(source: &str, bytes: &[u8]) -> Option<String> {
    let dir = icon_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let out = dir.join(icon_file_name(source));
    icon_image(bytes)?.save(&out).ok()?;
    Some(out.display().to_string())
}
pub(crate) mod shell;

/// No Windows, impede que um programa de console abra uma janela própria (o serviço não tem
/// janela). Nos outros sistemas não faz nada.
pub(crate) fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW)
    }
    #[cfg(not(windows))]
    {
        cmd
    }
}

/// Caminho deste executável. No Linux, se o pacote foi atualizado com o programa rodando, o
/// kernel informa "/usr/bin/akp153e (deleted)"; o arquivo novo está no mesmo caminho sem o sufixo.
pub(crate) fn current_exe() -> std::io::Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(strip_deleted(exe))
}

const DELETED_SUFFIX: &str = " (deleted)";

fn strip_deleted(exe: std::path::PathBuf) -> std::path::PathBuf {
    match exe.to_str().and_then(|s| s.strip_suffix(DELETED_SUFFIX)) {
        Some(path) if cfg!(target_os = "linux") => path.into(),
        _ => exe,
    }
}

/// Arquivo `.ttf` ou `.otf` que o `fc-match` escolhe para o padrão (ex.: "sans-serif:bold");
/// coleções (`.ttc`) e erros dão `None`.
#[cfg(not(windows))]
pub(crate) fn fc_match_file(pattern: &str) -> Option<std::path::PathBuf> {
    let out = Command::new("fc-match")
        .args(["-f", "%{file}", pattern])
        .output()
        .ok()?;
    let path = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    (ext == "ttf" || ext == "otf").then_some(path)
}

/// Inicia um programa sem esperar por ele. Uma thread o recolhe ao terminar: no Linux, um filho
/// que sai e nunca recebe `wait` fica como zumbi enquanto o processo pai viver (o serviço vive a
/// sessão inteira). Devolve o pid.
pub(crate) fn spawn_detached(cmd: &mut Command) -> std::io::Result<u32> {
    let mut child = cmd.spawn()?;
    let pid = child.id();
    std::thread::spawn(move || child.wait());
    Ok(pid)
}

// Só há testes do Linux aqui; no Windows o módulo inteiro some (senão o `use` fica sem uso).
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// Lê o estado ("Z" = zumbi) de um processo em /proc.
    fn proc_state(pid: u32) -> Option<char> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // O nome vai entre parênteses e pode ter espaços; o estado vem logo depois do ")".
        stat.rsplit_once(')')?.1.trim_start().chars().next()
    }

    #[test]
    fn a_replaced_binary_is_found_at_its_original_path() {
        let deleted = std::path::PathBuf::from("/usr/bin/akp153e (deleted)");
        assert_eq!(
            strip_deleted(deleted),
            std::path::Path::new("/usr/bin/akp153e")
        );
        let normal = std::path::PathBuf::from("/usr/bin/akp153e");
        assert_eq!(strip_deleted(normal.clone()), normal);
    }

    #[test]
    fn a_detached_child_that_exits_is_reaped() {
        let pid = spawn_detached(&mut Command::new("true")).expect("true");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while proc_state(pid).is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(proc_state(pid), None, "o filho {pid} ficou como zumbi");
    }
}
