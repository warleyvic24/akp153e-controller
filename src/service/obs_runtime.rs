//! Conexão do serviço com o OBS (obs-websocket 5). Só conecta se alguma tecla usa o OBS (ou se
//! uma tecla foi apertada); reconecta sozinho quando o OBS abre ou fecha. Os eventos do OBS
//! atualizam `Shared::obs` e redesenham as teclas do OBS (gravando = vermelho, cena atual acesa).
use super::runtime::{self, SharedState};
use crate::config::schema::{AppConfig, ButtonAction, ObsSettings};
use crate::obs::{self, op, ObsCall, ObsState};
use crate::usb::UsbCmd;
use serde_json::Value;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Espera do laço de leitura antes de olhar se chegou um aperto de tecla.
const POLL: Duration = Duration::from_millis(100);
/// Sem teclas do OBS, de quanto em quanto tempo conferir se alguma apareceu.
const IDLE_CHECK: Duration = Duration::from_secs(5);
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(30);
/// Prazo para o OBS responder ao Hello/Identify.
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(5);

type Socket = WebSocket<TcpStream>;

/// Alguma tecla (ou passo de multi-ação) de qualquer perfil usa o OBS?
pub fn uses_obs(cfg: &AppConfig) -> bool {
    fn any_obs(a: &ButtonAction) -> bool {
        match a {
            ButtonAction::Obs { .. } => true,
            ButtonAction::MultiAction { steps } => steps.iter().any(any_obs),
            _ => false,
        }
    }
    cfg.profiles
        .iter()
        .flat_map(|p| &p.pages)
        .flat_map(|pg| &pg.buttons)
        .flat_map(|b| {
            std::iter::once(&b.action)
                .chain(b.long_press_action.as_ref())
                .chain(b.toggled_action.as_ref())
        })
        .any(any_obs)
}

pub fn spawn(state: SharedState, tx: Sender<UsbCmd>, calls: Receiver<ObsCall>) {
    std::thread::spawn(move || {
        let mut retry = RETRY_MIN;
        let mut pending: Vec<ObsCall> = Vec::new();
        loop {
            let (settings, wanted) = {
                let sh = runtime::lock(&state);
                (sh.cfg.obs.clone(), uses_obs(&sh.cfg))
            };
            if !wanted && pending.is_empty() {
                match calls.recv_timeout(IDLE_CHECK) {
                    Ok(call) => pending.push(call),
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            match connect(&settings) {
                Ok(mut ws) => {
                    retry = RETRY_MIN;
                    let ended = session(&mut ws, &state, &tx, &calls, &settings, &mut pending);
                    let _ = ws.close(None);
                    set_state(&state, &tx, |s| *s = ObsState::default());
                    if let Err(e) = &ended {
                        log::info!("OBS: conexão encerrada ({e})");
                    }
                    if calls_closed(&ended) {
                        return;
                    }
                }
                Err(e) => {
                    log::debug!(
                        "OBS indisponível em {}:{}: {e}",
                        settings.host,
                        settings.port
                    );
                    if !pending.is_empty() {
                        log::warn!("OBS fechado ou WebSocket desligado: tecla ignorada");
                        pending.clear();
                    }
                    // Espera a próxima tentativa, mas um aperto de tecla tenta já.
                    match calls.recv_timeout(retry) {
                        Ok(call) => pending.push(call),
                        Err(RecvTimeoutError::Timeout) => retry = (retry * 2).min(RETRY_MAX),
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            }
        }
    });
}

fn calls_closed(ended: &Result<(), String>) -> bool {
    matches!(ended, Err(e) if e == CALLS_CLOSED)
}

const CALLS_CLOSED: &str = "serviço encerrado";

fn connect(settings: &ObsSettings) -> Result<Socket, String> {
    let addr = (settings.host.as_str(), settings.port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("endereço inválido")?;
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(HANDSHAKE_DEADLINE))
        .map_err(|e| e.to_string())?;
    let url = format!("ws://{}:{}", settings.host, settings.port);
    let (ws, _) = tungstenite::client(url, stream).map_err(|e| e.to_string())?;
    Ok(ws)
}

fn read_json(ws: &mut Socket) -> Result<Option<Value>, String> {
    match ws.read() {
        Ok(Message::Text(text)) => Ok(serde_json::from_str(text.as_str()).ok()),
        Ok(Message::Close(_)) => Err("o OBS fechou a conexão".into()),
        Ok(_) => Ok(None),
        Err(tungstenite::Error::Io(e))
            if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
        {
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    }
}

fn send(ws: &mut Socket, text: String) -> Result<(), String> {
    ws.send(Message::text(text)).map_err(|e| e.to_string())
}

/// Hello -> Identify -> Identified; depois o laço de eventos até a conexão cair.
fn session(
    ws: &mut Socket,
    state: &SharedState,
    tx: &Sender<UsbCmd>,
    calls: &Receiver<ObsCall>,
    settings: &ObsSettings,
    pending: &mut Vec<ObsCall>,
) -> Result<(), String> {
    let deadline = Instant::now() + HANDSHAKE_DEADLINE;
    let hello = loop {
        if let Some(msg) = read_json(ws)?.filter(|m| m["op"] == op::HELLO) {
            break msg;
        }
        if Instant::now() >= deadline {
            return Err("o OBS não respondeu".into());
        }
    };
    send(ws, obs::identify(&hello, &settings.password))?;
    loop {
        match read_json(ws)? {
            Some(msg) if msg["op"] == op::IDENTIFIED => break,
            _ if Instant::now() >= deadline => {
                return Err("autenticação recusada (confira a senha do WebSocket)".into())
            }
            _ => {}
        }
    }
    log::info!("OBS conectado em {}:{}", settings.host, settings.port);
    // Daqui em diante a leitura volta a cada `POLL` para atender os apertos de tecla.
    ws.get_ref()
        .set_read_timeout(Some(POLL))
        .map_err(|e| e.to_string())?;
    set_state(state, tx, |s| s.connected = true);
    for request in obs::initial_requests() {
        send(ws, request)?;
    }
    let mut next_id: u64 = 0;
    loop {
        for call in pending.drain(..) {
            next_id += 1;
            let (kind, data) = obs::request_for(call.command, &call.target);
            send(ws, obs::request(&format!("call-{next_id}"), kind, data))?;
        }
        if let Some(msg) = read_json(ws)? {
            let mut obs_state = runtime::lock(state).obs.clone();
            let (changed, follow) = match msg["op"].as_u64() {
                Some(op::EVENT) => obs::apply_event(&mut obs_state, &msg),
                Some(op::RESPONSE) => obs::apply_response(&mut obs_state, &msg),
                _ => (false, Vec::new()),
            };
            if changed {
                set_state(state, tx, |s| *s = obs_state);
            }
            for request in follow {
                send(ws, request)?;
            }
            continue;
        }
        // Nada a ler agora: um aperto de tecla, ou a config do OBS mudou (reconectar).
        match calls.try_recv() {
            Ok(call) => pending.push(call),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return Err(CALLS_CLOSED.into()),
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if runtime::lock(state).cfg.obs != *settings {
            return Ok(());
        }
    }
}

/// Muda o estado do OBS e redesenha as teclas do OBS da página, se o estado mudou.
fn set_state(state: &SharedState, tx: &Sender<UsbCmd>, change: impl FnOnce(&mut ObsState)) {
    let changed = {
        let mut sh = runtime::lock(state);
        let before = sh.obs.clone();
        change(&mut sh.obs);
        sh.obs != before
    };
    if changed {
        runtime::redraw_where(state, tx, |_, b| {
            matches!(b.action, ButtonAction::Obs { .. })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::ObsCommand;

    #[test]
    fn only_configs_with_obs_keys_connect() {
        let mut cfg = AppConfig::default();
        assert!(!uses_obs(&cfg));
        cfg.page_mut().buttons[2].action = ButtonAction::MultiAction {
            steps: vec![ButtonAction::Obs {
                command: ObsCommand::Record,
                target: String::new(),
            }],
        };
        assert!(uses_obs(&cfg));
    }

    /// Servidor WebSocket falso que fala o obs-websocket: Hello com senha, confere o Identify,
    /// responde aos pedidos iniciais e manda um evento de gravação.
    #[test]
    fn a_session_authenticates_syncs_and_follows_events() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let hello =
                r#"{"op":0,"d":{"rpcVersion":1,"authentication":{"salt":"s","challenge":"c"}}}"#;
            ws.send(Message::text(hello)).unwrap();
            let identify: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(
                identify["d"]["authentication"],
                obs::auth_response("segredo", "s", "c")
            );
            ws.send(Message::text(r#"{"op":2,"d":{"negotiatedRpcVersion":1}}"#))
                .unwrap();
            let scenes = r#"{"op":7,"d":{"requestType":"GetSceneList","requestId":"GetSceneList","requestStatus":{"result":true,"code":100},"responseData":{"currentProgramSceneName":"Jogo","scenes":[{"sceneName":"Jogo"}]}}}"#;
            ws.send(Message::text(scenes)).unwrap();
            let rec = r#"{"op":5,"d":{"eventType":"RecordStateChanged","eventIntent":64,"eventData":{"outputActive":true,"outputState":"OBS_WEBSOCKET_OUTPUT_STARTED"}}}"#;
            ws.send(Message::text(rec)).unwrap();
            // Espera um pedido de tecla (ToggleStream) e encerra.
            loop {
                let msg: Value = match ws.read() {
                    Ok(Message::Text(t)) => serde_json::from_str(t.as_str()).unwrap(),
                    Ok(_) => continue,
                    Err(_) => return false,
                };
                if msg["d"]["requestType"] == "ToggleStream" {
                    let _ = ws.close(None);
                    return true;
                }
            }
        });
        let mut cfg = AppConfig {
            obs: ObsSettings {
                host: "127.0.0.1".into(),
                port,
                password: "segredo".into(),
            },
            ..AppConfig::default()
        };
        cfg.page_mut().buttons[0].action = ButtonAction::Obs {
            command: ObsCommand::Record,
            target: String::new(),
        };
        let settings = cfg.obs.clone();
        let state = runtime::new_state(cfg, 100);
        let (tx, rx) = std::sync::mpsc::channel();
        let (call_tx, call_rx) = std::sync::mpsc::channel();
        let mut ws = connect(&settings).unwrap();
        let watcher = {
            let state = state.clone();
            std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    if runtime::lock(&state).obs.recording {
                        call_tx
                            .send(ObsCall {
                                command: ObsCommand::Stream,
                                target: String::new(),
                            })
                            .unwrap();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            })
        };
        let mut pending = Vec::new();
        let _ = session(&mut ws, &state, &tx, &call_rx, &settings, &mut pending);
        watcher.join().unwrap();
        assert!(server.join().unwrap(), "o pedido da tecla chegou ao OBS");
        let sh = runtime::lock(&state);
        assert_eq!(sh.obs.scene, "Jogo");
        assert!(sh.obs.recording);
        drop(sh);
        let frames = rx
            .try_iter()
            .filter(|c| matches!(c, UsbCmd::Frame { index: 0, .. }))
            .count();
        assert!(frames >= 1, "a tecla do OBS foi redesenhada");
    }

    /// Contra um OBS de verdade: `AKP_OBS_PORT=4466 AKP_OBS_PASSWORD=... cargo test real_obs -- --ignored --nocapture`.
    /// Troca de cena, liga e desliga a gravação e confere que o estado acompanha os eventos.
    #[test]
    #[ignore = "precisa de um OBS aberto com o WebSocket ligado"]
    fn real_obs_round_trip() {
        let port: u16 = std::env::var("AKP_OBS_PORT").unwrap().parse().unwrap();
        let mut cfg = AppConfig {
            obs: ObsSettings {
                host: "127.0.0.1".into(),
                port,
                password: std::env::var("AKP_OBS_PASSWORD").unwrap_or_default(),
            },
            ..AppConfig::default()
        };
        cfg.page_mut().buttons[0].action = ButtonAction::Obs {
            command: ObsCommand::Record,
            target: String::new(),
        };
        let settings = cfg.obs.clone();
        let state = runtime::new_state(cfg, 100);
        let (tx, _rx) = std::sync::mpsc::channel();
        let (call_tx, call_rx) = std::sync::mpsc::channel();
        let driver = {
            let state = state.clone();
            std::thread::spawn(move || {
                let wait = |what: &str, ok: &dyn Fn(&ObsState) -> bool| {
                    let deadline = Instant::now() + Duration::from_secs(10);
                    while Instant::now() < deadline {
                        if ok(&runtime::lock(&state).obs) {
                            println!("ok: {what}");
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    panic!("não aconteceu: {what} ({:?})", runtime::lock(&state).obs);
                };
                wait("conectou e leu as cenas", &|s| {
                    s.connected && !s.scenes.is_empty()
                });
                let scenes = runtime::lock(&state).obs.scenes.clone();
                println!("cenas: {scenes:?}");
                let other = scenes.last().unwrap().clone();
                let call = |command, target: &str| ObsCall {
                    command,
                    target: target.into(),
                };
                call_tx.send(call(ObsCommand::Scene, &other)).unwrap();
                wait("trocou de cena", &|s| s.scene == other);
                call_tx.send(call(ObsCommand::Record, "")).unwrap();
                wait("começou a gravar", &|s| s.recording);
                call_tx.send(call(ObsCommand::Record, "")).unwrap();
                wait("parou de gravar", &|s| !s.recording);
                drop(call_tx);
            })
        };
        let mut ws = connect(&settings).expect("conectar ao OBS");
        let mut pending = Vec::new();
        let _ = session(&mut ws, &state, &tx, &call_rx, &settings, &mut pending);
        driver.join().unwrap();
    }
}
