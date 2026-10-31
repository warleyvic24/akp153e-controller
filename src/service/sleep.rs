//! Descanso da tela: depois de `sleep_minutes` sem uso o aparelho apaga (brilho 0). O primeiro
//! toque só acende de novo, sem disparar a ação da tecla (como no Stream Deck); uma mudança feita
//! no editor também acende, para mostrar o resultado.
use crate::usb::UsbCmd;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct SleepState {
    last_activity: Instant,
    asleep: bool,
    /// Tecla cujo toque acordou a tela: a soltura dela também é ignorada.
    waking_key: Option<usize>,
}

impl Default for SleepState {
    fn default() -> Self {
        Self {
            last_activity: Instant::now(),
            asleep: false,
            waking_key: None,
        }
    }
}

impl SleepState {
    /// Quando a tela deve apagar; `None` se o descanso está desligado ou ela já está apagada.
    pub fn deadline(&self, minutes: u16) -> Option<Instant> {
        (minutes > 0 && !self.asleep)
            .then(|| self.last_activity + Duration::from_secs(u64::from(minutes) * 60))
    }

    /// Apaga a tela se o tempo passou; devolve `true` se apagou agora.
    pub fn sleep_if_due(&mut self, now: Instant, minutes: u16, tx: &Sender<UsbCmd>) -> bool {
        if self.deadline(minutes).is_some_and(|due| now >= due) {
            self.asleep = true;
            let _ = tx.send(UsbCmd::Brightness(0));
            return true;
        }
        false
    }

    /// Algo aconteceu (editor salvou, aparelho reconectou): acende se estava apagada e recomeça a contar.
    pub fn wake(&mut self, brightness: u8, tx: &Sender<UsbCmd>) {
        self.last_activity = Instant::now();
        if self.asleep {
            self.asleep = false;
            let _ = tx.send(UsbCmd::Brightness(brightness));
        }
    }

    /// Uma tecla foi apertada ou solta. Devolve `true` se o evento deve ser ignorado: o toque que
    /// acordou a tela (e a soltura dele) não vale como aperto.
    pub fn on_key(
        &mut self,
        index: usize,
        pressed: bool,
        brightness: u8,
        tx: &Sender<UsbCmd>,
    ) -> bool {
        self.last_activity = Instant::now();
        if !pressed && self.waking_key == Some(index) {
            self.waking_key = None;
            return true;
        }
        if pressed && self.asleep {
            self.wake(brightness, tx);
            self.waking_key = Some(index);
            return true;
        }
        false
    }

    #[cfg(test)]
    fn idle_since(&mut self, at: Instant) {
        self.last_activity = at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn brightness(sent: &[UsbCmd]) -> Vec<u8> {
        sent.iter()
            .filter_map(|c| match c {
                UsbCmd::Brightness(b) => Some(*b),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn never_sleeps_when_off() {
        let (tx, rx) = channel();
        let mut s = SleepState::default();
        assert!(s.deadline(0).is_none());
        let later = Instant::now() + Duration::from_secs(10_000);
        assert!(!s.sleep_if_due(later, 0, &tx));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn sleeps_after_the_timeout_and_the_waking_press_is_swallowed() {
        let (tx, rx) = channel();
        let mut s = SleepState::default();
        let start = Instant::now();
        s.idle_since(start);
        assert!(!s.sleep_if_due(start + Duration::from_secs(59), 1, &tx));
        assert!(s.sleep_if_due(start + Duration::from_secs(60), 1, &tx));
        assert!(s.deadline(1).is_none(), "apagada não apaga de novo");
        // O toque que acorda e a soltura dele não contam; o próximo toque conta.
        assert!(s.on_key(3, true, 80, &tx));
        assert!(s.on_key(3, false, 80, &tx));
        assert!(!s.on_key(3, true, 80, &tx));
        assert!(!s.on_key(3, false, 80, &tx));
        assert_eq!(brightness(&rx.try_iter().collect::<Vec<_>>()), [0, 80]);
    }

    #[test]
    fn activity_postpones_sleep_and_wake_restores_brightness() {
        let (tx, rx) = channel();
        let mut s = SleepState::default();
        let before = s.deadline(5).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        s.on_key(0, true, 50, &tx);
        assert!(s.deadline(5).unwrap() > before);
        s.idle_since(Instant::now() - Duration::from_secs(600));
        assert!(s.sleep_if_due(Instant::now(), 5, &tx));
        s.wake(50, &tx);
        assert_eq!(brightness(&rx.try_iter().collect::<Vec<_>>()), [0, 50]);
    }
}
