//! Mixer de volume em funcionamento no serviço: abre a página, lê o áudio fora do laço de teclas e
//! trata aperto, repetição e soltura. A lógica da página (puramente de dados) fica em `mixer.rs`.
//! Nada aqui segura o lock do estado durante leitura de áudio, extração de ícone ou desenho.
use crate::mixer::{MixerData, MixerEffect, MixerView, REPEAT_DELAY, REPEAT_INTERVAL};
use crate::service::runtime::{change_and_draw, lock, SharedState};
use crate::usb::UsbCmd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Com o mixer aberto, relê os apps com som neste intervalo (apps abrem, fecham ou mudam de volume).
pub const REFRESH: Duration = Duration::from_secs(2);

/// Só uma leitura roda por vez; se uma ainda não acabou, a próxima é pulada.
static LOADING: AtomicBool = AtomicBool::new(false);

/// Libera `LOADING` ao sair da thread de leitura, mesmo que ela entre em pânico.
struct LoadingGuard;

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        LOADING.store(false, Ordering::Release);
    }
}

/// Uma tecla de + ou − segurada: desde quando e quando repetiu pela última vez.
pub struct StepHold {
    index: usize,
    since: Instant,
    last: Instant,
}

impl StepHold {
    /// Instante em que a próxima repetição vence.
    pub fn due(&self) -> Instant {
        (self.since + REPEAT_DELAY).max(self.last + REPEAT_INTERVAL)
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

pub fn is_open(state: &SharedState) -> bool {
    lock(state).mixer.is_some()
}

/// Abre o mixer na hora (só com a tecla de sair) e carrega os apps em segundo plano.
pub fn open(state: &SharedState, tx: &Sender<UsbCmd>, opener: usize) {
    change_and_draw(state, tx, |sh| {
        sh.mixer = Some(MixerView::new(opener));
        ((), true)
    });
    reload(state, tx);
}

/// Relê os apps do Windows em outra thread, sem travar o tratamento das teclas.
pub fn reload(state: &SharedState, tx: &Sender<UsbCmd>) {
    if LOADING.swap(true, Ordering::AcqRel) {
        return;
    }
    let (state, tx) = (state.clone(), tx.clone());
    std::thread::spawn(move || {
        let _release = LoadingGuard;
        load_and_publish(&state, &tx);
    });
}

/// Primeiro mostra os apps (rápido, só com ícones já conhecidos); depois extrai os ícones que
/// faltam (PowerShell, lento) e atualiza. Uma leitura feita antes de um aperto, ou antes de o mixer
/// ser fechado e aberto de novo, é descartada.
fn load_and_publish(state: &SharedState, tx: &Sender<UsbCmd>) {
    let Some(version) = lock(state).mixer.as_ref().map(MixerView::version) else {
        return;
    };
    let mut data = MixerData::read();
    publish(state, tx, data.clone(), version);
    if data.fill_missing_icons() {
        publish(state, tx, data, version);
    }
}

fn publish(state: &SharedState, tx: &Sender<UsbCmd>, data: MixerData, version: u64) {
    change_and_draw(state, tx, |sh| {
        let changed = sh.mixer.as_mut().is_some_and(|m| m.update(data, version));
        ((), changed)
    });
}

/// Aplica o efeito no Windows e marca a página como alterada de novo: uma leitura que começou entre
/// o aperto e esta mudança viu o volume antigo e não pode sobrescrever o novo.
fn apply(state: &SharedState, effect: MixerEffect) {
    if effect == MixerEffect::None {
        return;
    }
    effect.apply();
    if let Some(mixer) = lock(state).mixer.as_mut() {
        mixer.touch();
    }
}

/// Aperto de uma tecla com o mixer aberto: redesenha o que mudou e só então muda o volume no sistema.
/// Devolve o estado de "segurando" se a tecla é + ou −.
pub fn key_down(state: &SharedState, tx: &Sender<UsbCmd>, index: usize) -> Option<StepHold> {
    let (effect, is_step) = change_and_draw(state, tx, |sh| match sh.mixer.as_mut() {
        Some(mixer) => {
            let is_step = mixer.is_step_key(index);
            ((mixer.press_down(index), is_step), true)
        }
        None => ((MixerEffect::None, false), false),
    });
    apply(state, effect);
    let now = Instant::now();
    is_step.then_some(StepHold {
        index,
        since: now,
        last: now,
    })
}

/// Repete + ou − se a tecla segurada já passou do tempo de espera e do intervalo.
pub fn repeat_if_due(state: &SharedState, tx: &Sender<UsbCmd>, hold: &mut StepHold, now: Instant) {
    if now < hold.due() {
        return;
    }
    hold.last = now;
    let effect = change_and_draw(state, tx, |sh| match sh.mixer.as_mut() {
        Some(mixer) => (mixer.repeat(hold.index), true),
        None => (MixerEffect::None, false),
    });
    apply(state, effect);
}

/// Soltura de uma tecla com o mixer aberto: a de sair troca de página ou fecha o mixer
/// (a página normal volta inteira). Devolve `true` se a página mudou ou fechou.
pub fn key_up(state: &SharedState, tx: &Sender<UsbCmd>, index: usize, held: Duration) -> bool {
    let (effect, layout_changed) = change_and_draw(state, tx, |sh| {
        let Some(mixer) = sh.mixer.as_mut() else {
            return ((MixerEffect::None, false), false);
        };
        let before = mixer.version();
        let effect = mixer.release(index, held);
        let layout_changed = mixer.version() != before || effect == MixerEffect::Close;
        if effect == MixerEffect::Close {
            sh.mixer = None;
        }
        ((effect, layout_changed), true)
    });
    apply(state, effect);
    layout_changed
}
