//! O que o laço da conexão precisa de um aparelho, qualquer que seja a família do protocolo
//! (AJAZZ/Mirabox hoje; Elgato e outras depois). Cada família implementa `Driver`; o laço em
//! `usb/mod.rs` não sabe de pacotes nem de comandos. Testes usam um aparelho falso.
use super::SideFrame;

pub type DriverResult<T> = Result<T, crate::error::UsbError>;

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
                .unwrap_or(Err(crate::error::UsbError::NotFound))
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
    use super::DriverResult;
    use crate::error::UsbError;
    use std::sync::mpsc::channel;

    fn run(dev: &mut FakeDriver, cmds: Vec<UsbCmd>) -> (DriverResult<()>, Vec<UsbEvent>) {
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
    fn an_unchanged_image_is_not_sent_again() {
        let mut dev = FakeDriver {
            keys: 15,
            has_side: true,
            ..FakeDriver::default()
        };
        let mut shown = super::super::Shown::default();
        // Página inteira redesenhada duas vezes (um "reload" sem mudança): só a 1ª vai ao aparelho.
        for _ in 0..2 {
            for key in 0..15 {
                shown.send_key(&mut dev, key, &[key as u8; 2000]).unwrap();
            }
            let strips = [vec![1; 900], vec![2; 900], vec![3; 900]];
            shown
                .send_side(&mut dev, &SideFrame::Strips(strips))
                .unwrap();
        }
        assert_eq!(dev.calls.len(), 16, "15 teclas + lateral, uma vez");
        // Mudou uma tecla: só ela vai.
        assert!(shown.send_key(&mut dev, 7, &[0xAA; 2000]).unwrap());
        assert!(!shown.send_key(&mut dev, 8, &[8; 2000]).unwrap());
        assert_eq!(dev.calls.last(), Some(&Call::Key(7, vec![0xAA; 2000])));
        // Tecla que o aparelho não tem nunca conta como mostrada.
        assert!(!shown.send_key(&mut dev, 99, &[1]).unwrap());
        assert_eq!(dev.calls.len(), 17);
        assert!(shown.send_side(&mut dev, &SideFrame::Clear).unwrap());
        assert!(!shown.send_side(&mut dev, &SideFrame::Clear).unwrap());
    }

    #[test]
    fn hashing_a_page_of_frames_is_cheap() {
        let mut dev = FakeDriver {
            keys: 15,
            ..FakeDriver::default()
        };
        let mut shown = super::super::Shown::default();
        let frame = vec![7u8; crate::graphics::renderer::MAX_JPEG_BYTES];
        let started = std::time::Instant::now();
        for _ in 0..100 {
            for key in 0..15 {
                shown.send_key(&mut dev, key, &frame).unwrap();
            }
        }
        let per_page = started.elapsed() / 100;
        assert!(
            per_page < std::time::Duration::from_millis(1),
            "comparar 15 teclas levou {per_page:?}"
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
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn a_hid_error_ends_the_connection_with_that_error() {
        let mut dev = FakeDriver {
            keys: 15,
            events: [
                Ok(Some((1, true))),
                Err(UsbError::Hid(hidapi::HidError::HidApiError {
                    message: "cabo solto".into(),
                })),
            ]
            .into(),
            ..FakeDriver::default()
        };
        let (result, events) = run(&mut dev, vec![]);
        let err = result.expect_err("o erro do aparelho encerra o laço");
        assert!(matches!(err, UsbError::Hid(_)), "{err:?}");
        assert!(err.to_string().contains("cabo solto"));
        // O aperto lido antes da queda ainda chegou ao serviço.
        assert_eq!(keys(&events), [(1, true), (1, false)]);
    }
}
