use crate::config::schema::ButtonAction;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Executa ações assíncronas. SwitchPage/SwitchProfile são tratadas pelo estado da UI.
#[derive(Clone)]
pub struct Executor {
    rt: tokio::runtime::Handle,
    http: reqwest::Client,
}

impl Executor {
    pub fn new(rt: tokio::runtime::Handle) -> Self {
        Self {
            rt,
            http: reqwest::Client::new(),
        }
    }

    pub fn run(&self, action: ButtonAction) {
        match action {
            ButtonAction::Hotkey { keys } => {
                self.rt.spawn_blocking(move || {
                    if let Err(e) = hotkey(&keys) {
                        log::error!("hotkey: {e}")
                    }
                });
            }
            ButtonAction::MediaKey { key } => {
                self.rt.spawn_blocking(move || {
                    if let Err(e) = hotkey(&[key]) {
                        log::error!("media: {e}")
                    }
                });
            }
            ButtonAction::LaunchApp { path, args } => {
                self.rt.spawn_blocking(move || {
                    // Sem console: o serviço não tem janela, e um programa de console abriria uma do nada.
                    // Programas com janela própria não são afetados.
                    let spawned = std::process::Command::new(&path)
                        .args(&args)
                        .creation_flags(CREATE_NO_WINDOW)
                        .spawn();
                    if let Err(e) = spawned {
                        log::error!("launch {path}: {e}")
                    }
                });
            }
            ButtonAction::HttpWebhook { url, method, body } => {
                let Some(m) = webhook_method(&method) else {
                    log::error!("webhook {}: método inválido {method:?}", loggable_url(&url));
                    return;
                };
                let c = self.http.clone();
                self.rt.spawn(async move {
                    let shown = loggable_url(&url);
                    let mut r = c
                        .request(m, &url)
                        .timeout(std::time::Duration::from_secs(10));
                    if let Some(b) = body {
                        r = r.body(b);
                    }
                    match r.send().await {
                        Ok(x) => log::info!("webhook {shown}: {}", x.status()),
                        Err(e) => log::error!("webhook {shown}: {}", e.without_url()),
                    }
                });
            }
            ButtonAction::None
            | ButtonAction::SwitchPage { .. }
            | ButtonAction::SwitchProfile { .. }
            | ButtonAction::OpenMixer => {}
        }
    }
}

/// Só métodos usuais; qualquer outro texto é recusado em vez de virar GET sem aviso.
pub const WEBHOOK_METHODS: [&str; 6] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"];

fn webhook_method(name: &str) -> Option<reqwest::Method> {
    match name.trim().to_uppercase().as_str() {
        "GET" => Some(reqwest::Method::GET),
        "POST" => Some(reqwest::Method::POST),
        "PUT" => Some(reqwest::Method::PUT),
        "PATCH" => Some(reqwest::Method::PATCH),
        "DELETE" => Some(reqwest::Method::DELETE),
        "HEAD" => Some(reqwest::Method::HEAD),
        _ => None,
    }
}

/// URL sem credenciais, query e fragmento: tokens costumam ir nesses campos e o log é texto puro.
fn loggable_url(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(u) => format!(
            "{}://{}{}",
            u.scheme(),
            u.host_str().unwrap_or("?"),
            u.path()
        ),
        Err(_) => "<url inválida>".to_string(),
    }
}

fn parse_key(s: &str) -> Option<Key> {
    let l = s.to_lowercase();
    Some(match l.as_str() {
        "ctrl" | "control" => Key::Control,
        "shift" => Key::Shift,
        "alt" => Key::Alt,
        "meta" | "win" | "super" => Key::Meta,
        "enter" => Key::Return,
        "esc" | "escape" => Key::Escape,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "playpause" => Key::MediaPlayPause,
        "next" => Key::MediaNextTrack,
        "prev" => Key::MediaPrevTrack,
        "volup" => Key::VolumeUp,
        "voldown" => Key::VolumeDown,
        "mute" => Key::VolumeMute,
        _ => {
            let mut c = l.chars();
            match (c.next(), c.next()) {
                (Some(ch), None) => Key::Unicode(ch),
                _ => return None,
            }
        }
    })
}

pub fn is_valid_key(name: &str) -> bool {
    parse_key(name).is_some()
}

fn hotkey(keys: &[String]) -> Result<(), String> {
    let parsed: Vec<Key> = keys
        .iter()
        .map(|k| parse_key(k).ok_or(format!("tecla inválida: {k}")))
        .collect::<Result<_, _>>()?;
    let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    press_chord(&parsed, |key, direction| {
        e.key(key, direction).map_err(|e| e.to_string())
    })
}

/// Aperta as teclas em ordem e solta em ordem inversa. Se um aperto falhar, solta as já apertadas
/// antes de devolver o erro (senão Ctrl/Win ficariam presos no sistema); a soltura tenta todas.
fn press_chord(
    keys: &[Key],
    mut send: impl FnMut(Key, Direction) -> Result<(), String>,
) -> Result<(), String> {
    for (i, key) in keys.iter().enumerate() {
        if let Err(e) = send(*key, Direction::Press) {
            for held in keys[..i].iter().rev() {
                let _ = send(*held, Direction::Release);
            }
            return Err(e);
        }
    }
    let mut first_error = None;
    for key in keys.iter().rev() {
        if let Err(e) = send(*key, Direction::Release) {
            first_error.get_or_insert(e);
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_and_rejects_unknown_keys() {
        assert!(parse_key("Ctrl").is_some());
        assert!(parse_key("a").is_some());
        assert!(parse_key("playpause").is_some());
        assert!(parse_key("naoexiste").is_none());
    }

    #[test]
    fn a_failed_press_releases_the_keys_already_held() {
        let mut log = Vec::new();
        let result = press_chord(&[Key::Control, Key::Shift, Key::Unicode('s')], |k, d| {
            log.push((k, d));
            if matches!((k, d), (Key::Unicode('s'), Direction::Press)) {
                Err("falhou".to_string())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("falhou".to_string()));
        assert_eq!(
            log,
            vec![
                (Key::Control, Direction::Press),
                (Key::Shift, Direction::Press),
                (Key::Unicode('s'), Direction::Press),
                (Key::Shift, Direction::Release),
                (Key::Control, Direction::Release),
            ]
        );
    }

    #[test]
    fn a_failed_release_still_releases_every_key() {
        let mut releases = 0;
        let result = press_chord(&[Key::Control, Key::Unicode('c')], |_, d| {
            if d == Direction::Release {
                releases += 1;
                return Err("solta".to_string());
            }
            Ok(())
        });
        assert_eq!(result, Err("solta".to_string()));
        assert_eq!(releases, 2);
    }

    #[test]
    fn webhook_method_accepts_known_and_rejects_the_rest() {
        assert_eq!(webhook_method(" post "), Some(reqwest::Method::POST));
        assert_eq!(webhook_method("delete"), Some(reqwest::Method::DELETE));
        assert_eq!(webhook_method(""), None);
        assert_eq!(webhook_method("DELET"), None);
    }

    #[test]
    fn loggable_url_drops_credentials_query_and_fragment() {
        assert_eq!(
            loggable_url("https://user:pw@host.io:8443/a/b?token=x#f"),
            "https://host.io/a/b"
        );
        assert_eq!(loggable_url("não é url"), "<url inválida>");
    }

    async fn wait_for(cond: impl Fn() -> bool) -> bool {
        for _ in 0..50 {
            if cond() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        false
    }

    #[tokio::test]
    async fn webhook_sends_method_and_body() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let n = s.read(&mut buf).unwrap();
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            tx.send(String::from_utf8_lossy(&buf[..n]).into_owned())
                .unwrap();
        });
        let exec = Executor::new(tokio::runtime::Handle::current());
        exec.run(ButtonAction::HttpWebhook {
            url,
            method: "post".into(),
            body: Some("ping".into()),
        });
        let req = tokio::task::spawn_blocking(move || {
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        })
        .await
        .unwrap();
        assert!(req.starts_with("POST /hook"), "{req}");
        assert!(req.ends_with("ping"), "{req}");
    }

    #[tokio::test]
    async fn launch_app_runs_process_with_args() {
        let out = std::env::temp_dir().join(format!("akp_launch_{}.txt", std::process::id()));
        let exec = Executor::new(tokio::runtime::Handle::current());
        exec.run(ButtonAction::LaunchApp {
            path: "cmd".into(),
            args: ["/C", "echo", "ok", ">"]
                .map(String::from)
                .into_iter()
                .chain([out.display().to_string()])
                .collect(),
        });
        let probe = out.clone();
        assert!(wait_for(move || probe.exists()).await);
        let _ = std::fs::remove_file(out);
    }
}
