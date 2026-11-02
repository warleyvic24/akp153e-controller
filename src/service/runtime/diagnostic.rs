//! Imagens de diagnóstico desenhadas em todas as teclas (comandos `calib` e `pattern` da porta
//! de controle). Não mexem na config do usuário: o próximo "reload" devolve a página.
use crate::config::schema::{ButtonConfig, RGBAColor};
use crate::graphics::renderer::{calibration_rgba, encode_frame, render_button_frame};
use crate::usb::UsbCmd;
use std::sync::mpsc::Sender;

/// Tamanho da fonte do número da tecla no padrão de teste.
const PATTERN_FONT: f32 = 40.0;
/// Tamanhos (px) da varredura de calibração, um por tecla na ordem da grade.
const CALIBRATION_SIZES: [u32; 15] = [
    80, 85, 90, 95, 96, 100, 104, 108, 112, 116, 120, 124, 126, 128, 132,
];
/// Varredura do padrão de teste: primeira tecla em `SWEEP_START` px, cada seguinte `SWEEP_STEP` a mais.
const SWEEP_START: u32 = 118;
const SWEEP_STEP: u32 = 2;

/// Padrão de diagnóstico: cada tecla com cor própria e o número dela (revela teclas que não atualizam,
/// espelhamento e rotação). A configuração do usuário não é alterada; "reload" restaura as imagens.
/// Calibração: com `px` fixo, todas as teclas recebem o mesmo tamanho; sem ele, cada tecla recebe um
/// tamanho diferente de CALIBRATION_SIZES (na ordem da grade) para comparar numa foto só.
pub fn render_calibration(key_count: usize, tx: &Sender<UsbCmd>, px: Option<u32>) {
    for index in 0..key_count {
        let size = px.unwrap_or(CALIBRATION_SIZES[index % CALIBRATION_SIZES.len()]);
        let data = encode_frame(&calibration_rgba(size));
        let _ = tx.send(UsbCmd::Frame { index, data });
    }
}

/// Padrão de teste: cada tecla com cor própria e o número dela (com `px`) ou o tamanho usado
/// (na varredura, sem `px`).
pub fn render_pattern(key_count: usize, tx: &Sender<UsbCmd>, px: Option<u32>) {
    const HUES: [[u8; 3]; 6] = [
        [200, 40, 40],
        [40, 160, 60],
        [50, 90, 210],
        [200, 160, 30],
        [150, 60, 190],
        [30, 160, 170],
    ];
    for index in 0..key_count {
        let mut b = ButtonConfig::blank();
        let [r, g, bl] = HUES[index % HUES.len()];
        b.appearance.background = RGBAColor {
            r,
            g,
            b: bl,
            a: 255,
        };
        b.appearance.text_color = RGBAColor {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let size = px.unwrap_or(SWEEP_START + SWEEP_STEP * index as u32);
        b.appearance.text = if px.is_some() {
            (index + 1).to_string()
        } else {
            size.to_string()
        };
        b.appearance.font_size = PATTERN_FONT;
        let data = render_button_frame(&b, size, size);
        let _ = tx.send(UsbCmd::Frame { index, data });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn sent_indexes(draw: impl FnOnce(&Sender<UsbCmd>)) -> Vec<usize> {
        let (tx, rx) = channel();
        draw(&tx);
        rx.try_iter()
            .map(|cmd| match cmd {
                UsbCmd::Frame { index, .. } => index,
                _ => panic!("só quadros de tecla"),
            })
            .collect()
    }

    #[test]
    fn every_key_gets_one_frame_in_order() {
        let all: Vec<usize> = (0..15).collect();
        assert_eq!(sent_indexes(|tx| render_calibration(15, tx, None)), all);
        assert_eq!(
            sent_indexes(|tx| render_calibration(15, tx, Some(100))),
            all
        );
        assert_eq!(sent_indexes(|tx| render_pattern(15, tx, None)), all);
        assert_eq!(sent_indexes(|tx| render_pattern(15, tx, Some(64))), all);
    }
}
