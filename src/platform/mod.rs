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
pub(crate) mod focus;
#[cfg(not(windows))]
mod icon_theme;
pub(crate) mod live;
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

/// A conexão local veio de um processo do mesmo usuário? A porta de controle é TCP em
/// 127.0.0.1, onde qualquer usuário da máquina conecta. No Linux o dono de cada socket está em
/// `/proc/net/tcp`; sem essa tabela (ou no Windows, que não tem uma equivalente sem API nativa)
/// a conexão é aceita, como antes.
pub(crate) fn same_user(conn: &std::net::TcpStream) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let (Ok(peer), Ok(local)) = (conn.peer_addr(), conn.local_addr()) else {
            return false;
        };
        let (Ok(table), Ok(me)) = (
            std::fs::read_to_string(PROC_NET_TCP),
            std::fs::metadata("/proc/self").map(|m| m.uid()),
        ) else {
            log::warn!("dono da conexão de controle desconhecido: {PROC_NET_TCP} ilegível");
            return true;
        };
        // Cliente que já fechou não aparece (ou aparece sem dono): não há a quem responder.
        tcp_owner(&table, peer, local) == Some(me)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = conn;
        true
    }
}

#[cfg(target_os = "linux")]
const PROC_NET_TCP: &str = "/proc/net/tcp";

/// Dono (uid) do socket que vai de `from` para `to`, segundo a tabela de `/proc/net/tcp`.
/// Endereço IPv4 em hexadecimal na ordem de bytes da máquina; porta em hexadecimal normal.
#[cfg(target_os = "linux")]
fn tcp_owner(table: &str, from: std::net::SocketAddr, to: std::net::SocketAddr) -> Option<u32> {
    const LOCAL: usize = 1;
    const REMOTE: usize = 2;
    const UID: usize = 7;
    let hex = |addr: std::net::SocketAddr| match addr {
        std::net::SocketAddr::V4(a) => Some(format!(
            "{:08X}:{:04X}",
            u32::from_ne_bytes(a.ip().octets()),
            a.port()
        )),
        std::net::SocketAddr::V6(_) => None,
    };
    let (from, to) = (hex(from)?, hex(to)?);
    table.lines().skip(1).find_map(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let matches =
            fields.get(LOCAL) == Some(&from.as_str()) && fields.get(REMOTE) == Some(&to.as_str());
        matches.then(|| fields.get(UID)?.parse().ok()).flatten()
    })
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
    fn a_local_connection_from_this_process_is_the_same_user() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let _client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        assert!(same_user(&server));
    }

    #[test]
    fn the_owner_comes_from_the_matching_socket_line() {
        let ip = u32::from_ne_bytes([127, 0, 0, 1]);
        let table = format!(
            "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
             0: {ip:08X}:B8A1 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1 1\n\
             1: {ip:08X}:D431 {ip:08X}:B8A1 01 00000000:00000000 00:00000000 00000000  1001        0 2 1\n"
        );
        let service: std::net::SocketAddr = "127.0.0.1:47265".parse().unwrap();
        let client: std::net::SocketAddr = "127.0.0.1:54321".parse().unwrap();
        assert_eq!(tcp_owner(&table, client, service), Some(1001));
        let other: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
        assert_eq!(tcp_owner(&table, other, service), None);
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
