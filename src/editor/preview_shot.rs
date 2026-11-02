//! Só em debug: `KEYFORGE_PREVIEW_SHOT=<arquivo.png>` grava a janela do editor num PNG depois de
//! alguns quadros e fecha o editor. Junto com `KEYFORGE_PREVIEW_UI` e `KEYFORGE_PREVIEW_THEME`
//! serve à regressão visual (`tools/ui-shots.sh`) sem depender do compositor para capturar a tela
//! (com o monitor desligado ou a sessão travada, o `grim` só vê a tela de bloqueio).
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Quadros antes da captura: as texturas das teclas são desenhadas numa thread e chegam depois.
const SHOT_AFTER_FRAMES: u64 = 40;

/// Pede a captura no quadro certo e fecha a janela quando ela foi gravada.
pub(super) fn tick(ctx: &egui::Context) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some(path) = std::env::var_os("KEYFORGE_PREVIEW_SHOT") else {
        return;
    };
    let done: Arc<AtomicBool> = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Arc<AtomicBool>>(egui::Id::unique("preview-shot-done"))
            .clone()
    });
    if done.load(Ordering::Acquire) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        return;
    }
    ctx.request_repaint();
    if ctx.cumulative_frame_nr() != SHOT_AFTER_FRAMES {
        return;
    }
    let repaint = ctx.clone();
    ctx.request_screenshot(move |image| {
        let [w, h] = image.size;
        let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
        match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
            Some(img) => {
                if let Err(e) = img.save(&path) {
                    log::warn!("captura do editor: {e}");
                }
            }
            None => log::warn!("captura do editor: tamanho inválido"),
        }
        done.store(true, Ordering::Release);
        repaint.request_repaint();
    });
}
