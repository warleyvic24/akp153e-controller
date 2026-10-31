//! O que o laço da conexão precisa de um aparelho, qualquer que seja a família do protocolo
//! (AJAZZ/Mirabox hoje; Elgato e outras depois). Cada família implementa `Driver`; o laço em
//! `usb/mod.rs` não sabe de pacotes nem de comandos. Testes usam um aparelho falso.
use super::SideFrame;

pub type DriverResult<T> = Result<T, String>;

pub trait Driver {
    /// Desenha a imagem (já codificada) na tecla `key` da página; `false` se a tecla não existe.
    fn set_key(&mut self, key: usize, image: &[u8]) -> DriverResult<bool>;
    /// Desenha ou apaga a tela lateral; `false` se o aparelho não tem uma.
    fn set_side(&mut self, side: &SideFrame) -> DriverResult<bool>;
    /// Mostra o que foi desenhado desde a última vez.
    fn flush(&mut self) -> DriverResult<()>;
    fn set_brightness(&mut self, level: u8) -> DriverResult<()>;
    /// Mantém a conexão viva (alguns aparelhos se apagam sem isso).
    fn keepalive(&mut self) -> DriverResult<()>;
    /// Espera pouco por um evento: (tecla da página, apertada).
    fn poll(&mut self) -> DriverResult<Option<(usize, bool)>>;
    /// O aparelho informa a soltura; sem isso cada aperto vira aperto + soltura.
    fn reports_release(&self) -> bool;
}

/// Aparelho falso: grava o que recebeu e devolve eventos programados.
#[cfg(test)]
pub(super) mod fake {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Debug, PartialEq)]
    pub enum Call {
        Key(usize, Vec<u8>),
        Side,
        Flush,
        Brightness(u8),
    }

    #[derive(Default)]
    pub struct FakeDriver {
        pub calls: Vec<Call>,
        pub events: VecDeque<DriverResult<Option<(usize, bool)>>>,
        pub release: bool,
        pub keys: usize,
        pub has_side: bool,
    }

    impl Driver for FakeDriver {
        fn set_key(&mut self, key: usize, image: &[u8]) -> DriverResult<bool> {
            if key >= self.keys {
                return Ok(false);
            }
            self.calls.push(Call::Key(key, image.to_vec()));
            Ok(true)
        }
        fn set_side(&mut self, _: &SideFrame) -> DriverResult<bool> {
            if self.has_side {
                self.calls.push(Call::Side);
            }
            Ok(self.has_side)
        }
        fn flush(&mut self) -> DriverResult<()> {
            self.calls.push(Call::Flush);
            Ok(())
        }
        fn set_brightness(&mut self, level: u8) -> DriverResult<()> {
            self.calls.push(Call::Brightness(level));
            Ok(())
        }
        fn keepalive(&mut self) -> DriverResult<()> {
            Ok(())
        }
        fn poll(&mut self) -> DriverResult<Option<(usize, bool)>> {
            // Sem mais eventos: a conexão "cai" e o teste termina.
            self.events
                .pop_front()
                .unwrap_or_else(|| Err("fim do roteiro".into()))
        }
        fn reports_release(&self) -> bool {
            self.release
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{run_connection, SideFrame, UsbCmd, UsbEvent};
    use super::fake::{Call, FakeDriver};
    use std::sync::mpsc::channel;

    fn run(dev: &mut FakeDriver, cmds: Vec<UsbCmd>) -> (Result<(), String>, Vec<UsbEvent>) {
        let (tx, events) = channel();
        let (cmd_tx, rx) = channel();
        for c in cmds {
            cmd_tx.send(c).unwrap();
        }
        let result = run_connection(dev, &tx, &rx);
        (result, events.try_iter().collect())
    }

    fn keys(events: &[UsbEvent]) -> Vec<(usize, bool)> {
        events
            .iter()
            .filter_map(|e| match e {
                UsbEvent::Key { index, pressed } => Some((*index, *pressed)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn frames_side_and_brightness_reach_the_device_then_one_flush() {
        let mut dev = FakeDriver {
            keys: 15,
            has_side: true,
            ..FakeDriver::default()
        };
        let cmds = vec![
            UsbCmd::Frame {
                index: 3,
                data: vec![1],
            },
            UsbCmd::Frame {
                index: 99,
                data: vec![2],
            },
            UsbCmd::Side(SideFrame::Clear),
            UsbCmd::Brightness(40),
        ];
        let (result, _) = run(&mut dev, cmds);
        assert!(result.is_err(), "o roteiro acabou: conexão caiu");
        assert_eq!(
            dev.calls,
            [
                Call::Brightness(40),
                Call::Key(3, vec![1]),
                Call::Side,
                Call::Flush
            ]
        );
    }

    #[test]
    fn nothing_drawn_means_no_flush() {
        let mut dev = FakeDriver {
            keys: 6,
            ..FakeDriver::default()
        };
        let (_, _) = run(&mut dev, vec![UsbCmd::Side(SideFrame::Clear)]);
        assert!(dev.calls.is_empty(), "{:?}", dev.calls);
    }

    #[test]
    fn a_device_without_release_gets_press_and_release() {
        let mut dev = FakeDriver {
            keys: 15,
            events: [Ok(Some((4, true)))].into(),
            ..FakeDriver::default()
        };
        let (_, events) = run(&mut dev, vec![]);
        assert_eq!(keys(&events), [(4, true), (4, false)]);
    }

    #[test]
    fn a_device_with_release_passes_both_through() {
        let mut dev = FakeDriver {
            keys: 15,
            release: true,
            events: [Ok(Some((2, true))), Ok(None), Ok(Some((2, false)))].into(),
            ..FakeDriver::default()
        };
        let (_, events) = run(&mut dev, vec![]);
        assert_eq!(keys(&events), [(2, true), (2, false)]);
    }

    #[test]
    fn reconnect_ends_the_connection_cleanly() {
        let mut dev = FakeDriver::default();
        let (result, _) = run(&mut dev, vec![UsbCmd::Reconnect]);
        assert_eq!(result, Ok(()));
    }
}
