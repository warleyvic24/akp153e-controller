//! Saúde do serviço (`health` na porta de controle, `GET /health` por HTTP): tempo de pé, último
//! erro do log, threads vivas e o último quadro enviado ao aparelho.
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Uma thread que dá sinal (`beat`) e passa disto sem dar é considerada travada.
pub const STALE_AFTER: Duration = Duration::from_secs(30);
/// Maior mensagem de erro guardada (o resto é cortado).
const MAX_ERROR_CHARS: usize = 512;

static START: OnceLock<Instant> = OnceLock::new();
static LAST_FRAME_MS: AtomicU64 = AtomicU64::new(0);
static LAST_ERROR: Mutex<Option<LastError>> = Mutex::new(None);
static THREADS: Mutex<Vec<Arc<Slot>>> = Mutex::new(Vec::new());

thread_local! {
    static CURRENT: RefCell<Option<Arc<Slot>>> = const { RefCell::new(None) };
}

/// Estado de uma thread registrada.
struct Slot {
    name: &'static str,
    alive: AtomicBool,
    /// Último sinal (ms desde o início do processo); 0 = nunca deu sinal.
    last_beat: AtomicU64,
}

/// Marca a thread como morta ao sair (normal ou por pânico em debug).
pub struct AliveGuard(Arc<Slot>);

impl Drop for AliveGuard {
    fn drop(&mut self) {
        self.0.alive.store(false, Ordering::SeqCst);
        CURRENT.with(|c| c.borrow_mut().take());
    }
}

/// Marca o início do processo (o `uptime` conta daqui).
pub fn mark_start() {
    START.get_or_init(Instant::now);
}

fn since_start() -> Duration {
    START.get_or_init(Instant::now).elapsed()
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Registra a thread atual com `name`. Um registro antigo com o mesmo nome (thread reiniciada)
/// é substituído.
pub fn register(name: &'static str) -> AliveGuard {
    let slot = Arc::new(Slot {
        name,
        alive: AtomicBool::new(true),
        last_beat: AtomicU64::new(0),
    });
    {
        let mut threads = crate::sync::lock(&THREADS);
        threads.retain(|s| s.name != name);
        threads.push(slot.clone());
    }
    CURRENT.with(|c| *c.borrow_mut() = Some(slot.clone()));
    AliveGuard(slot)
}

/// Sinal de vida da thread atual (sem trava; nada se ela não foi registrada).
pub fn beat() {
    let now = since_start().as_millis() as u64;
    CURRENT.with(|c| {
        if let Some(slot) = c.borrow().as_ref() {
            slot.last_beat.store(now.max(1), Ordering::Relaxed);
        }
    });
}

/// Um quadro chegou ao aparelho agora.
pub fn frame_sent() {
    LAST_FRAME_MS.store(unix_ms(), Ordering::Relaxed);
}

/// Guarda o último erro do log (chamado pela camada do `tracing`).
pub fn record_error(target: &str, message: &str) {
    let message: String = message.chars().take(MAX_ERROR_CHARS).collect();
    *crate::sync::lock(&LAST_ERROR) = Some(LastError {
        at_ms: unix_ms(),
        target: target.to_string(),
        message,
    });
}

/// Último erro registrado no log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastError {
    /// Instante (ms desde a época Unix).
    pub at_ms: u64,
    pub target: String,
    pub message: String,
}

/// Estado de uma thread na resposta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadHealth {
    pub name: String,
    pub alive: bool,
    /// Há quanto tempo deu sinal; `None` para threads que só esperam (não dão sinal).
    pub last_beat_age_ms: Option<u64>,
    /// Deu sinal antes, mas não há mais de [`STALE_AFTER`].
    pub stale: bool,
}

/// Resposta de `/health`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// `ok` ou `degraded` (alguma thread morta ou travada).
    pub status: String,
    pub uptime_ms: u64,
    pub connected: bool,
    pub last_error: Option<LastError>,
    /// Último quadro enviado ao aparelho (ms desde a época Unix).
    pub last_frame_ms: Option<u64>,
    pub last_frame_age_ms: Option<u64>,
    pub threads_alive: usize,
    pub threads: Vec<ThreadHealth>,
}

impl Health {
    pub fn is_ok(&self) -> bool {
        self.status == "ok"
    }
}

/// Retrato da saúde agora; `connected` vem do estado do serviço.
pub fn snapshot(connected: bool) -> Health {
    let now = since_start();
    let now_ms = now.as_millis() as u64;
    let threads: Vec<ThreadHealth> = crate::sync::lock(&THREADS)
        .iter()
        .map(|s| {
            let beat = s.last_beat.load(Ordering::Relaxed);
            let age = (beat > 0).then(|| now_ms.saturating_sub(beat));
            ThreadHealth {
                name: s.name.to_string(),
                alive: s.alive.load(Ordering::SeqCst),
                last_beat_age_ms: age,
                stale: age.is_some_and(|a| a > STALE_AFTER.as_millis() as u64),
            }
        })
        .collect();
    let healthy = threads.iter().all(|t| t.alive && !t.stale);
    let last_frame = LAST_FRAME_MS.load(Ordering::Relaxed);
    let last_frame_ms = (last_frame > 0).then_some(last_frame);
    Health {
        status: if healthy { "ok" } else { "degraded" }.to_string(),
        uptime_ms: now_ms,
        connected,
        last_error: crate::sync::lock(&LAST_ERROR).clone(),
        last_frame_ms,
        last_frame_age_ms: last_frame_ms.map(|t| unix_ms().saturating_sub(t)),
        threads_alive: threads.iter().filter(|t| t.alive).count(),
        threads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread_named<'a>(h: &'a Health, name: &str) -> Option<&'a ThreadHealth> {
        h.threads.iter().find(|t| t.name == name)
    }

    #[test]
    fn a_registered_thread_is_alive_until_it_ends() {
        let (go, wait) = std::sync::mpsc::channel::<()>();
        let (ready_tx, ready) = std::sync::mpsc::channel::<()>();
        let worker = std::thread::spawn(move || {
            let _alive = register("health-test-worker");
            beat();
            ready_tx.send(()).unwrap();
            wait.recv().unwrap();
        });
        ready.recv().unwrap();
        let h = snapshot(true);
        let t = thread_named(&h, "health-test-worker").expect("registrada");
        assert!(t.alive && !t.stale);
        assert!(t.last_beat_age_ms.is_some());
        go.send(()).unwrap();
        worker.join().unwrap();
        let h = snapshot(true);
        assert!(!thread_named(&h, "health-test-worker").unwrap().alive);
        assert_eq!(h.status, "degraded");
    }

    #[test]
    fn errors_and_frames_show_up_in_the_snapshot() {
        record_error("keyforge::teste", &"x".repeat(2000));
        frame_sent();
        let h = snapshot(false);
        let e = h.last_error.clone().expect("erro guardado");
        assert_eq!(e.target, "keyforge::teste");
        assert_eq!(e.message.chars().count(), MAX_ERROR_CHARS);
        assert!(h.last_frame_ms.is_some());
        assert!(h.last_frame_age_ms.unwrap() < 60_000);
        let json = serde_json::to_value(&h).unwrap();
        for key in ["uptime_ms", "last_error", "threads_alive", "last_frame_ms"] {
            assert!(json.get(key).is_some(), "{key} no JSON");
        }
    }
}
