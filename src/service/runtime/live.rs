//! Dados ao vivo das teclas (volume, tocando agora, uso do sistema): uma thread consulta a cada
//! `PERIOD` só o que a página ativa usa e, no serviço, redesenha só o que mudou.
use super::{lock, redraw_side, redraw_where, Shared, SharedState};
use crate::config::schema::SideScreen;
use crate::graphics::live_art;
use crate::platform::live::{Live, Sampler, Wants};
use crate::usb::UsbCmd;
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub const PERIOD: Duration = Duration::from_secs(2);
/// Depois de um toque numa tecla de mídia, espera o sistema aplicar a mudança antes de ler.
const AFTER_WAKE: Duration = Duration::from_millis(150);

/// Pedido de consulta antecipada (tecla de mídia tocada) e o sinal que acorda a thread.
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Faz a thread consultar já, em vez de esperar o fim do `PERIOD`.
pub fn wake() {
    let (flag, signal) = &WAKE;
    *flag
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
    signal.notify_all();
}

/// Dorme até `PERIOD` ou até um `wake`.
fn nap() {
    let (flag, signal) = &WAKE;
    let guard = flag
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mut woken, _) = signal
        .wait_timeout_while(guard, PERIOD, |w| !*w)
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if std::mem::take(&mut *woken) {
        drop(woken);
        std::thread::sleep(AFTER_WAKE);
    }
}

/// O que a página ativa usa (nada com o mixer aberto, que ocupa as teclas e a lateral).
pub(super) fn wants(sh: &Shared) -> Wants {
    if sh.mixer.is_some() {
        return Wants::default();
    }
    let page = sh.cfg.page();
    let mut all = page
        .buttons
        .iter()
        .map(live_art::wants)
        .fold(Wants::default(), |a, b| Wants {
            volume: a.volume || b.volume,
            playing: a.playing || b.playing,
            system: a.system || b.system,
        });
    all.system |= page.side == SideScreen::System;
    all
}

/// O que mudou de `old` para `new`, nos termos de `Wants` (para saber o que redesenhar).
fn changed(old: &Live, new: &Live) -> Wants {
    Wants {
        volume: old.volume != new.volume,
        playing: old.playing != new.playing,
        system: old.system != new.system,
    }
}

/// Thread dos dados ao vivo. Com `tx` (o serviço) redesenha no aparelho as teclas afetadas;
/// sem ele (o editor) só guarda o dado em `Shared::live`, que a prévia usa.
pub fn spawn(state: SharedState, tx: Option<Sender<UsbCmd>>) {
    std::thread::spawn(move || {
        let mut sampler = Sampler::default();
        loop {
            let wanted = wants(&lock(&state));
            if wanted.any() {
                // Consultar é lento (processos externos): fora do lock.
                let fresh = sampler.sample(wanted);
                let diff = {
                    let mut sh = lock(&state);
                    let diff = changed(&sh.live, &fresh);
                    sh.live = fresh;
                    diff
                };
                if let (Some(tx), true) = (&tx, diff.any()) {
                    redraw_where(&state, tx, |_, b| {
                        let w = live_art::wants(b);
                        (w.volume && diff.volume)
                            || (w.playing && diff.playing)
                            || (w.system && diff.system)
                    });
                    if diff.system && lock(&state).cfg.page().side == SideScreen::System {
                        redraw_side(&state, tx);
                    }
                }
            }
            nap();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{AppConfig, ButtonAction, Metric};
    use crate::platform::live::{SystemStats, Volume};

    #[test]
    fn only_what_the_page_uses_is_wanted() {
        let state = super::super::new_state(AppConfig::default(), 100);
        let mut sh = lock(&state);
        for b in &mut sh.cfg.page_mut().buttons {
            b.action = ButtonAction::None;
        }
        sh.cfg.page_mut().side = SideScreen::Off;
        assert!(!wants(&sh).any());
        sh.cfg.page_mut().buttons[0].action = ButtonAction::MediaKey { key: "mute".into() };
        assert_eq!(
            wants(&sh),
            Wants {
                volume: true,
                ..Wants::default()
            }
        );
        sh.cfg.page_mut().side = SideScreen::System;
        sh.cfg.page_mut().buttons[1].action = ButtonAction::Monitor {
            metric: Metric::Cpu,
        };
        assert!(wants(&sh).system && !wants(&sh).playing);
    }

    #[test]
    fn the_diff_names_only_what_changed() {
        let old = Live {
            volume: Some(Volume {
                percent: 40,
                muted: false,
            }),
            ..Live::default()
        };
        let mut new = old.clone();
        assert!(!changed(&old, &new).any());
        new.system = Some(SystemStats {
            cpu: 5,
            ram: 30,
            temp: None,
        });
        assert_eq!(
            changed(&old, &new),
            Wants {
                system: true,
                ..Wants::default()
            }
        );
    }
}
