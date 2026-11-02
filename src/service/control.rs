//! Porta de controle do serviço (`127.0.0.1:47153`): recebe um pedido por conexão do editor ou
//! da linha de comando, despacha e responde em JSON (formato em `protocol.rs`).
//!
//! Qualquer processo local conecta na porta, então toda entrada é limitada em tamanho, tempo e
//! valores antes de chegar ao aparelho.
use super::protocol::{
    http_route, ControlRequest, ControlResponse, HttpRoute, RequestEnvelope, ResponseEnvelope,
};
use super::runtime::{self, SharedState};
use super::selftest;
use crate::config::schema::AppConfig;
use crate::obs::ObsCall;
use crate::platform::shell;
use crate::usb::{self, UsbCmd};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Tempo total para o cliente mandar a linha do pedido.
const CONTROL_DEADLINE: Duration = Duration::from_millis(500);
/// Maior linha de pedido aceita (bytes, sem o `\n`).
const MAX_COMMAND_BYTES: usize = 256;
/// Conexões atendidas ao mesmo tempo; as demais são descartadas.
const MAX_CONNECTIONS: usize = 8;
/// Cabeçalhos HTTP lidos (e descartados) antes de responder; o resto fecha a conexão.
const MAX_HTTP_HEADER_BYTES: usize = 8 * 1024;
/// Tamanhos aceitos por `calib`/`pattern` (px), checados antes de alocar a imagem.
const DIAGNOSTIC_PX: std::ops::RangeInclusive<u32> = 16..=256;

/// Canais e estado que o despacho de um pedido usa.
#[derive(Clone, Copy)]
struct Ports<'a> {
    state: &'a SharedState,
    usb: &'a Sender<UsbCmd>,
    obs: &'a Sender<ObsCall>,
}

/// Atende a porta numa thread, com uma thread por conexão e um teto de conexões: um cliente
/// lento não trava os demais pedidos.
pub(super) fn spawn(
    listener: TcpListener,
    state: SharedState,
    usb: Sender<UsbCmd>,
    obs: Sender<ObsCall>,
) {
    crate::telemetry::spawn_tracked("control", move || {
        let active = Arc::new(AtomicUsize::new(0));
        for conn in listener.incoming().flatten() {
            if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                active.fetch_sub(1, Ordering::SeqCst);
                log::warn!("conexões de controle demais; descartada");
                continue;
            }
            let (state, usb, obs, active) =
                (state.clone(), usb.clone(), obs.clone(), active.clone());
            let active_on_fail = active.clone();
            let spawned = crate::telemetry::spawn_named("control-conn", move || {
                let ports = Ports {
                    state: &state,
                    usb: &usb,
                    obs: &obs,
                };
                handle(conn, &ports);
                active.fetch_sub(1, Ordering::SeqCst);
            });
            if spawned.is_none() {
                active_on_fail.fetch_sub(1, Ordering::SeqCst);
            }
        }
    });
}

/// Lê uma linha de pedido com tamanho e tempo totais limitados: um cliente que mande 1 byte por
/// vez não prende a thread nem a memória. `None` em excesso, prazo estourado ou UTF-8 inválido.
fn read_command(conn: &mut TcpStream) -> Option<String> {
    let deadline = Instant::now() + CONTROL_DEADLINE;
    let mut line = Vec::with_capacity(32);
    let mut byte = [0u8; 1];
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        conn.set_read_timeout(Some(left)).ok()?;
        match conn.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) if line.len() < MAX_COMMAND_BYTES => line.push(byte[0]),
            _ => return None,
        }
    }
    String::from_utf8(line).ok()
}

/// Atende uma conexão. Sempre escreve uma resposta (mesmo de erro), para o cliente não ver EOF.
fn handle(mut conn: TcpStream, ports: &Ports) {
    // A porta é TCP local: outro usuário da mesma máquina também conecta.
    if !crate::platform::same_user(&conn) {
        log::warn!("conexão de controle de outro usuário recusada");
        return;
    }
    let line = read_command(&mut conn);
    if let Some(route) = line.as_deref().and_then(http_route) {
        return respond_http(conn, route, ports);
    }
    let response = match line {
        None => ResponseEnvelope::error("comando inválido ou muito longo"),
        Some(line) => match RequestEnvelope::parse(&line) {
            Ok(env) => ResponseEnvelope::new(dispatch(env.request, ports)),
            Err(e) => {
                log::warn!("comando inválido: {e}");
                ResponseEnvelope::error(e.to_string())
            }
        },
    };
    let _ = writeln!(conn, "{}", response.to_json());
}

/// Lê e descarta os cabeçalhos HTTP (até a linha vazia, com teto de tamanho e o mesmo prazo):
/// fechar com dados por ler faz o sistema mandar RST e o cliente perder a resposta.
fn drain_http_headers(conn: &mut TcpStream) {
    let deadline = Instant::now() + CONTROL_DEADLINE;
    let (mut seen, mut tail) = (0usize, [0u8; 4]);
    let mut byte = [0u8; 1];
    while seen < MAX_HTTP_HEADER_BYTES {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            return;
        };
        if conn.set_read_timeout(Some(left)).is_err() {
            return;
        }
        match conn.read(&mut byte) {
            Ok(1) => {
                seen += 1;
                tail = [tail[1], tail[2], tail[3], byte[0]];
                // A linha do pedido já foi lida: a primeira linha vazia encerra os cabeçalhos.
                if tail[2..] == *b"\n\n" || tail == *b"\r\n\r\n" || (seen <= 2 && byte[0] == b'\n')
                {
                    return;
                }
            }
            _ => return,
        }
    }
}

/// Responde `GET /health` (JSON; 503 se degradado) e `GET /metrics` (texto do Prometheus).
fn respond_http(mut conn: TcpStream, route: HttpRoute, ports: &Ports) {
    drain_http_headers(&mut conn);
    let (status, kind, body) = match route {
        HttpRoute::Health => match dispatch(ControlRequest::Health, ports) {
            ControlResponse::Health(h) => (
                if h.is_ok() {
                    "200 OK"
                } else {
                    "503 Service Unavailable"
                },
                "application/json",
                serde_json::to_string(&h).unwrap_or_default(),
            ),
            _ => ("500 Internal Server Error", "text/plain", String::new()),
        },
        HttpRoute::Metrics => match dispatch(ControlRequest::Metrics, ports) {
            ControlResponse::Metrics { text } => {
                ("200 OK", "text/plain; version=0.0.4; charset=utf-8", text)
            }
            _ => ("500 Internal Server Error", "text/plain", String::new()),
        },
        HttpRoute::NotFound => ("404 Not Found", "text/plain", "not found\n".to_string()),
    };
    let _ = write!(
        conn,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

/// Executa um pedido já validado no formato e devolve a resposta.
fn dispatch(request: ControlRequest, ports: &Ports) -> ControlResponse {
    let Ports { state, usb, obs } = *ports;
    let cmd = request.name();
    crate::telemetry::metrics::control_request(cmd);
    let _span = tracing::info_span!("control_command", cmd).entered();
    // `status` chega do editor a cada segundo: só em `trace`, para não encher o log.
    tracing::trace!("pedido de controle");
    match request {
        ControlRequest::Show => {
            shell::open_editor();
            ControlResponse::Ok
        }
        ControlRequest::Reconnect => {
            tracing::info!("reconexão pedida pela porta de controle");
            let _ = usb.send(UsbCmd::Reconnect);
            ControlResponse::Ok
        }
        ControlRequest::Reload => {
            reload(state, usb, AppConfig::load_existing());
            ControlResponse::Ok
        }
        ControlRequest::ObsInfo => {
            let info = runtime::lock(state).obs.info_json();
            ControlResponse::ObsInfo(serde_json::from_str(&info).unwrap_or_default())
        }
        ControlRequest::ObsRun { command, target } => {
            match ObsCall::from_control(&format!("{command} {target}")) {
                Some(call) => {
                    let _ = obs.send(call);
                    ControlResponse::Ok
                }
                None => error(format!("comando do OBS desconhecido: {command}")),
            }
        }
        ControlRequest::Apps => ControlResponse::Apps {
            apps: runtime::lock(state).recent_apps.join("\t"),
        },
        ControlRequest::Brightness { level } if level <= usb::MAX_BRIGHTNESS => {
            let _ = usb.send(UsbCmd::Brightness(level));
            ControlResponse::Ok
        }
        ControlRequest::Brightness { level } => error(format!("brilho inválido: {level}")),
        ControlRequest::Selftest => {
            let reply = selftest::start(state, usb);
            ControlResponse::Selftest {
                ok: reply == "ok",
                message: reply.to_string(),
            }
        }
        ControlRequest::Status => {
            let sh = runtime::lock(state);
            ControlResponse::Status {
                connected: sh.connected,
                icon_px: sh.icon_px,
            }
        }
        ControlRequest::Health => {
            let connected = runtime::lock(state).connected;
            ControlResponse::Health(crate::telemetry::health::snapshot(connected))
        }
        ControlRequest::Metrics => {
            let connected = runtime::lock(state).connected;
            ControlResponse::Metrics {
                text: crate::telemetry::metrics::render(connected),
            }
        }
        ControlRequest::Calib { px } => diagnostic(state, usb, px, Diagnostic::Calibration),
        ControlRequest::Pattern { px } => diagnostic(state, usb, px, Diagnostic::Pattern),
        ControlRequest::Text { line } => error(format!("comando desconhecido: {line}")),
    }
}

fn error(message: String) -> ControlResponse {
    ControlResponse::Error { message }
}

/// Imagem de diagnóstico desenhada em todas as teclas.
#[derive(Clone, Copy)]
enum Diagnostic {
    Calibration,
    Pattern,
}

/// Desenha `kind` com o tamanho `px` (ou a varredura, com `None`) se ele estiver em `DIAGNOSTIC_PX`.
fn diagnostic(
    state: &SharedState,
    usb: &Sender<UsbCmd>,
    px: Option<u32>,
    kind: Diagnostic,
) -> ControlResponse {
    if let Some(px) = px.filter(|px| !DIAGNOSTIC_PX.contains(px)) {
        log::warn!("tamanho fora de {DIAGNOSTIC_PX:?}: {px}");
        return error(format!("tamanho inválido: {px} (aceito {DIAGNOSTIC_PX:?})"));
    }
    let key_count = runtime::lock(state).cfg.page().buttons.len();
    match kind {
        Diagnostic::Calibration => runtime::render_calibration(key_count, usb, px),
        Diagnostic::Pattern => runtime::render_pattern(key_count, usb, px),
    }
    runtime::forget_mixer_drawing(state);
    ControlResponse::Ok
}

/// Adota a config relida do disco e redesenha. `None` (arquivo sumido, travado num instante ou
/// inválido) mantém a atual: cair na config padrão apagaria as teclas do aparelho até o próximo
/// "reload". O backup de uma config inválida fica a cargo do editor, que usa `AppConfig::load`.
fn reload(state: &SharedState, usb: &Sender<UsbCmd>, cfg: Option<AppConfig>) {
    let Some(cfg) = cfg else {
        log::warn!("reload: config ilegível ou inválida; mantida a atual");
        return;
    };
    let brightness = cfg.ui.brightness;
    let language = cfg.ui.language;
    let brightness_changed = runtime::change_and_draw(state, usb, |sh| {
        let changed = sh.cfg.ui.brightness != brightness;
        sh.cfg = cfg;
        // Com o mixer aberto, "reload" é o que restaura a tela: reenvia as 15 teclas dele.
        if let Some(mixer) = &sh.mixer {
            mixer.invalidate();
        }
        (changed, true)
    });
    usb::set_brightness(brightness);
    // A mudança feita no editor tem de aparecer: acende a tela se ela estava em descanso.
    runtime::lock(state).sleep.wake(brightness, usb);
    // O menu da bandeja só muda de idioma quando o serviço reinicia; o resto vale já.
    crate::strings::set_language(language);
    if brightness_changed {
        let _ = usb.send(UsbCmd::Brightness(brightness));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::mpsc::channel;

    /// Abre uma conexão local; o lado do servidor é entregue a `read_command`.
    fn serve_once(client: impl FnOnce(TcpStream) + Send + 'static) -> Option<String> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || client(TcpStream::connect(addr).unwrap()));
        let (mut server, _) = listener.accept().unwrap();
        let line = read_command(&mut server);
        client.join().unwrap();
        line
    }

    /// Manda `request` à porta como um cliente faria; devolve a resposta (JSON) e o que foi ao USB.
    fn command(request: &[u8], connected: bool) -> (Value, Vec<UsbCmd>) {
        let (reply, sent) = raw(request, connected);
        let json = serde_json::from_str(reply.trim()).expect("resposta JSON válida");
        (json, sent)
    }

    /// Como `command`, devolvendo a resposta crua (HTTP).
    fn raw(request: &[u8], connected: bool) -> (String, Vec<UsbCmd>) {
        let state = runtime::new_state(AppConfig::default(), 100);
        runtime::lock(&state).connected = connected;
        let (usb, rx) = channel();
        let (obs, _obs_rx) = channel();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let request = request.to_vec();
        let client = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).unwrap();
            let _ = c.write_all(&request);
            let mut reply = String::new();
            let _ = c.read_to_string(&mut reply);
            reply
        });
        let (server, _) = listener.accept().unwrap();
        let ports = Ports {
            state: &state,
            usb: &usb,
            obs: &obs,
        };
        handle(server, &ports);
        let reply = client.join().unwrap();
        (reply, rx.try_iter().collect())
    }

    fn frames(sent: &[UsbCmd]) -> usize {
        sent.iter()
            .filter(|c| matches!(c, UsbCmd::Frame { .. }))
            .count()
    }

    #[test]
    fn a_normal_command_line_is_read() {
        let line = serve_once(|mut c| c.write_all(b"status\n").unwrap());
        assert_eq!(line.as_deref(), Some("status"));
    }

    #[test]
    fn an_oversized_line_is_rejected() {
        let line = serve_once(|mut c| c.write_all(&[b'a'; 400]).unwrap());
        assert_eq!(line, None);
    }

    #[test]
    fn a_client_dripping_bytes_hits_the_total_deadline() {
        let started = Instant::now();
        let line = serve_once(|mut c| {
            for _ in 0..8 {
                if c.write_all(b"a").is_err() {
                    break;
                }
                std::thread::sleep(CONTROL_DEADLINE / 4);
            }
        });
        assert_eq!(line, None);
        assert!(started.elapsed() < CONTROL_DEADLINE * 3);
    }

    #[test]
    fn a_line_exactly_at_the_limit_is_accepted_and_one_byte_more_is_not() {
        let at_limit = serve_once(|mut c| {
            c.write_all(&[b'a'; MAX_COMMAND_BYTES]).unwrap();
            c.write_all(b"\n").unwrap();
        });
        assert_eq!(at_limit.map(|l| l.len()), Some(MAX_COMMAND_BYTES));
        let over = serve_once(|mut c| {
            let _ = c.write_all(&[b'a'; MAX_COMMAND_BYTES + 1]);
            let _ = c.write_all(b"\n");
        });
        assert_eq!(over, None);
    }

    #[test]
    fn a_line_closed_without_newline_is_still_read() {
        let line = serve_once(|mut c| c.write_all(b"reconnect").unwrap());
        assert_eq!(line.as_deref(), Some("reconnect"));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let line = serve_once(|mut c| c.write_all(b"st\xFFtus\n").unwrap());
        assert_eq!(line, None);
    }

    #[test]
    fn status_reports_connection_and_key_size() {
        for (request, connected) in [(&b"status\n"[..], true), (b"  status  \n", false)] {
            let (json, _) = command(request, connected);
            assert_eq!(json["resp"], "status");
            assert_eq!(json["connected"], connected);
            assert_eq!(json["icon_px"], 100);
        }
    }

    #[test]
    fn json_and_text_requests_reach_the_same_handler() {
        for request in [
            &b"reconnect\n"[..],
            b"{\"version\":1,\"cmd\":\"reconnect\"}\n",
            b"{\"version\":1,\"cmd\":\"text\",\"line\":\"reconnect\"}\n",
        ] {
            let (json, sent) = command(request, false);
            assert_eq!(json["resp"], "ok", "{request:?}");
            assert!(
                matches!(sent.as_slice(), [UsbCmd::Reconnect]),
                "{request:?}"
            );
        }
    }

    #[test]
    fn unknown_malformed_or_oversized_requests_do_nothing() {
        let oversized = [b'x'; MAX_COMMAND_BYTES * 2];
        let newer = b"{\"version\":99,\"cmd\":\"reconnect\"}\n";
        for request in [
            &b"rm -rf /\n"[..],
            b"STATUS\n",
            b"\n",
            b"{not json\n",
            newer,
            &oversized,
        ] {
            let (json, sent) = command(request, true);
            assert_eq!(json["resp"], "error", "{request:?}");
            assert!(sent.is_empty(), "{request:?}");
        }
    }

    #[test]
    fn live_brightness_reaches_the_device_only_with_a_valid_level() {
        let (json, sent) = command(b"brightness 40\n", true);
        assert_eq!(json["resp"], "ok");
        assert!(matches!(sent.as_slice(), [UsbCmd::Brightness(40)]));
        for bad in [
            &b"brightness 101\n"[..],
            b"brightness -1\n",
            b"brightness x\n",
            b"{\"version\":1,\"cmd\":\"brightness\",\"level\":101}\n",
        ] {
            let (json, sent) = command(bad, true);
            assert_eq!(json["resp"], "error", "{bad:?}");
            assert!(sent.is_empty(), "{bad:?}");
        }
    }

    #[test]
    fn calibration_draws_every_key_only_with_a_valid_size() {
        let keys = AppConfig::default().page().buttons.len();
        let (_, sent) = command(b"calib 100\n", true);
        assert_eq!(frames(&sent), keys);
        let (_, sent) = command(b"pattern 64\n", true);
        assert_eq!(frames(&sent), keys);
        for bad in [
            &b"calib 9999\n"[..],
            b"calib 15\n",
            b"calib abc\n",
            b"pattern 0\n",
            b"pattern 257\n",
        ] {
            let (json, sent) = command(bad, true);
            assert_eq!(json["resp"], "error", "{bad:?}");
            assert_eq!(frames(&sent), 0, "{bad:?}");
        }
    }

    #[test]
    fn unknown_obs_commands_are_refused() {
        let (json, _) = command(b"obs-run explode\n", true);
        assert_eq!(json["resp"], "error");
        let (json, _) = command(b"obs-run record\n", true);
        assert_eq!(json["resp"], "ok");
    }

    #[test]
    fn apps_and_obs_info_answer_with_their_own_tags() {
        let (json, _) = command(b"apps\n", true);
        assert_eq!(json["resp"], "apps");
        let (json, _) = command(b"obs-info\n", true);
        assert_eq!(json["resp"], "obs-info");
    }

    #[test]
    fn an_unreadable_config_on_reload_keeps_the_current_one() {
        let mut current = AppConfig::default();
        current.page_mut().buttons[0].appearance.text = "OBS".into();
        let state = runtime::new_state(current.clone(), 100);
        let (usb, rx) = channel();
        reload(&state, &usb, None);
        assert_eq!(runtime::lock(&state).cfg, current);
        assert_eq!(rx.try_iter().count(), 0, "nada redesenhado");
        reload(&state, &usb, Some(AppConfig::default()));
        assert_eq!(runtime::lock(&state).cfg, AppConfig::default());
        assert_eq!(
            frames(&rx.try_iter().collect::<Vec<_>>()),
            current.page().buttons.len()
        );
    }

    #[test]
    fn health_answers_with_uptime_threads_and_connection() {
        let (json, sent) = command(b"health\n", true);
        assert_eq!(json["resp"], "health");
        assert_eq!(json["connected"], true);
        assert!(json["uptime_ms"].is_u64());
        assert!(json["threads_alive"].is_u64());
        assert!(json.get("last_error").is_some());
        assert!(json.get("last_frame_ms").is_some());
        assert!(sent.is_empty());
    }

    #[test]
    fn http_get_health_and_metrics_work_on_the_control_port() {
        let (reply, _) = raw(
            b"GET /health HTTP/1.1\r\nHost: x\r\nAccept: */*\r\n\r\n",
            false,
        );
        assert!(
            reply.starts_with("HTTP/1.1 200 OK") || reply.starts_with("HTTP/1.1 503"),
            "{reply}"
        );
        assert!(reply.contains("Content-Type: application/json"));
        let body = reply.split("\r\n\r\n").nth(1).unwrap();
        let json: Value = serde_json::from_str(body).unwrap();
        assert!(json["uptime_ms"].is_u64());
        assert_eq!(json["connected"], false);

        let (reply, _) = raw(b"GET /metrics HTTP/1.0\r\n\r\n", true);
        assert!(reply.starts_with("HTTP/1.1 200 OK"), "{reply}");
        assert!(reply.contains("text/plain; version=0.0.4"));

        let (reply, _) = raw(b"GET /nada HTTP/1.1\r\n\r\n", true);
        assert!(reply.starts_with("HTTP/1.1 404"), "{reply}");
    }
}
