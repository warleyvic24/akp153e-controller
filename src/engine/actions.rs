use crate::config::schema::ButtonAction;
use crate::platform::{hide_console, spawn_detached};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// Maior pausa aceita num passo `Delay`; valores maiores são cortados.
pub const MAX_DELAY_MS: u64 = 60_000;
/// Maior texto que `TypeText` digita de uma vez.
pub const MAX_TYPE_CHARS: usize = 2_000;

/// Executa ações assíncronas. Navegação (página, perfil, mixer) é tratada pelo estado do serviço;
/// dentro de uma multi-ação ela é ignorada.
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
        let this = self.clone();
        self.rt.spawn(async move { this.run_step(action).await });
    }

    /// Executa uma ação e espera terminar (menos `LaunchApp`, que só dispara o programa), para
    /// que os passos de uma multi-ação saiam na ordem.
    async fn run_step(&self, action: ButtonAction) {
        match action {
            ButtonAction::Hotkey { keys } => {
                run_blocking("hotkey", move || hotkey(&keys)).await;
            }
            ButtonAction::MediaKey { key } => {
                run_blocking("media", move || hotkey(&[key])).await;
            }
            ButtonAction::TypeText { text } => {
                run_blocking("texto", move || type_text(&text)).await;
            }
            ButtonAction::LaunchApp { path, args } => {
                let spawned = tokio::task::spawn_blocking(move || {
                    // Sem console: o serviço não tem janela, e um programa de console abriria uma do nada.
                    // Programas com janela própria não são afetados.
                    let mut cmd = std::process::Command::new(&path);
                    spawn_detached(hide_console(cmd.args(&args)))
                        .map_err(|e| format!("launch {path}: {e}"))
                })
                .await;
                if let Ok(Err(e)) = spawned {
                    log::error!("{e}");
                }
            }
            ButtonAction::HttpWebhook { url, method, body } => {
                let Some(m) = webhook_method(&method) else {
                    log::error!("webhook {}: método inválido {method:?}", loggable_url(&url));
                    return;
                };
                let shown = loggable_url(&url);
                let mut r = self
                    .http
                    .request(m, &url)
                    .timeout(std::time::Duration::from_secs(10));
                if let Some(b) = body {
                    r = r.body(b);
                }
                match r.send().await {
                    Ok(x) => log::info!("webhook {shown}: {}", x.status()),
                    Err(e) => log::error!("webhook {shown}: {}", e.without_url()),
                }
            }
            ButtonAction::MultiAction { steps } => {
                for step in steps {
                    // Box::pin: função assíncrona recursiva.
                    Box::pin(self.run_step(step)).await;
                }
            }
            ButtonAction::Delay { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(ms.min(MAX_DELAY_MS))).await;
            }
            ButtonAction::None
            | ButtonAction::SwitchPage { .. }
            | ButtonAction::SwitchProfile { .. }
            | ButtonAction::NextPage
            | ButtonAction::PreviousPage
            | ButtonAction::OpenMixer => {}
        }
    }
}

/// Roda código bloqueante (teclado) fora das threads async e registra o erro, se houver.
async fn run_blocking(
    what: &'static str,
    job: impl FnOnce() -> Result<(), String> + Send + 'static,
) {
    match tokio::task::spawn_blocking(job).await {
        Ok(Err(e)) => log::error!("{what}: {e}"),
        Ok(Ok(())) => {}
        Err(e) => log::error!("{what}: {e}"),
    }
}

fn type_text(text: &str) -> Result<(), String> {
    if text.chars().count() > MAX_TYPE_CHARS {
        return Err(format!("texto maior que {MAX_TYPE_CHARS} caracteres"));
    }
    let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    e.text(text).map_err(|e| e.to_string())
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
        "print" | "printscreen" | "prtsc" => Key::PrintScr,
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

    #[tokio::test]
    async fn multi_action_runs_steps_in_order_and_waits_on_delay() {
        let exec = Executor::new(tokio::runtime::Handle::current());
        let started = std::time::Instant::now();
        exec.run_step(ButtonAction::MultiAction {
            steps: vec![
                ButtonAction::Delay { ms: 150 },
                ButtonAction::TypeText {
                    text: "x".repeat(MAX_TYPE_CHARS + 1),
                },
                ButtonAction::NextPage,
            ],
        })
        .await;
        assert!(started.elapsed() >= std::time::Duration::from_millis(150));
    }

    #[test]
    fn type_text_refuses_oversized_text() {
        let err = type_text(&"a".repeat(MAX_TYPE_CHARS + 1)).unwrap_err();
        assert!(err.contains("maior"), "{err}");
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
        #[cfg(windows)]
        let (program, flags) = ("cmd", ["/C", "echo", "ok", ">"]);
        #[cfg(not(windows))]
        let (program, flags) = ("sh", ["-c", "echo ok >\"$0\"", ""]);
        // No Linux o caminho vai como `$0` do `sh -c`; no Windows, depois do `>`.
        let mut args: Vec<String> = flags.map(String::from).into_iter().collect();
        #[cfg(windows)]
        args.push(out.display().to_string());
        #[cfg(not(windows))]
        {
            args.pop();
            args.push(out.display().to_string());
        }
        exec.run(ButtonAction::LaunchApp {
            path: program.into(),
            args,
        });
        let probe = out.clone();
        assert!(wait_for(move || probe.exists()).await);
        let _ = std::fs::remove_file(out);
    }
}
