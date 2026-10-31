//! Desenho do aparelho no palco a partir do arquivo do modelo (`devices/*.json`): onde fica cada
//! tecla e a tela lateral, e o tamanho da tecla que cabe no espaço livre. Só geometria (testável).
use crate::device::{ControlKind, DeviceModel};
use eframe::egui::{pos2, vec2, Rect, Vec2};

pub(super) const KEY_MIN: f32 = 48.0;
// A imagem da tecla tem o tamanho do display (100 px); acima de ~1,4x ela fica borrada.
pub(super) const KEY_MAX: f32 = 140.0;
pub(super) const DEVICE_MAX_WIDTH: f32 = 780.0;
pub(super) const DEVICE_PAD: f32 = 24.0;
pub(super) const DEVICE_GAP: f32 = 12.0;
/// Largura de uma coluna só de tela lateral em relação à tecla (82 px de faixa para ~95 px de tecla no AKP153E).
const SIDE_RATIO: f32 = 0.86;

/// O modelo da página (`Page::model`); se o arquivo sumiu, o primeiro do catálogo (o AKP153E).
pub(super) fn model_for(id: &str) -> Option<&'static DeviceModel> {
    let catalog = crate::device::catalog();
    catalog
        .iter()
        .find(|m| m.id == id)
        .or_else(|| catalog.first())
}

/// Largura de cada coluna da grade, em teclas: 1, ou `SIDE_RATIO` se a coluna só tem tela lateral.
fn column_factors(model: &DeviceModel) -> Vec<f32> {
    let cols = model.controls.iter().map(|c| c.x + c.w).max().unwrap_or(0);
    (0..cols)
        .map(|col| {
            let covering = model
                .controls
                .iter()
                .filter(|c| (c.x..c.x + c.w).contains(&col));
            let mut kinds = covering.map(|c| c.kind).peekable();
            if kinds.peek().is_some() && kinds.all(|k| k == ControlKind::Strip) {
                SIDE_RATIO
            } else {
                1.0
            }
        })
        .collect()
}

fn rows(model: &DeviceModel) -> u32 {
    model.controls.iter().map(|c| c.y + c.h).max().unwrap_or(1)
}

/// Posições dos controles com tecla de lado `key`, relativas ao canto do aparelho.
pub(super) struct Layout {
    pub size: Vec2,
    /// Na ordem dos botões da página (`DeviceModel::keys`).
    pub keys: Vec<Rect>,
    pub strip: Option<Rect>,
}

pub(super) fn layout(model: &DeviceModel, key: f32) -> Layout {
    let factors = column_factors(model);
    let mut left = vec![DEVICE_PAD];
    for f in &factors {
        left.push(left.last().copied().unwrap_or(0.0) + f * key + DEVICE_GAP);
    }
    let rect_of = |x: u32, y: u32, w: u32, h: u32| {
        let (x, y, w, h) = (x as usize, y as f32, w as usize, h as f32);
        Rect::from_min_size(
            pos2(left[x], DEVICE_PAD + y * (key + DEVICE_GAP)),
            vec2(
                left[x + w] - left[x] - DEVICE_GAP,
                h * key + (h - 1.0) * DEVICE_GAP,
            ),
        )
    };
    let r = rows(model) as f32;
    Layout {
        size: vec2(
            left.last().copied().unwrap_or(0.0) - DEVICE_GAP + DEVICE_PAD,
            2.0 * DEVICE_PAD + r * key + (r - 1.0) * DEVICE_GAP,
        ),
        keys: model
            .keys()
            .iter()
            .map(|c| rect_of(c.x, c.y, c.w, c.h))
            .collect(),
        strip: model.strip().map(|c| rect_of(c.x, c.y, c.w, c.h)),
    }
}

/// Lado da tecla que cabe no espaço livre (entre `KEY_MIN` e `KEY_MAX`).
pub(super) fn fit_key_size(model: &DeviceModel, avail_w: f32, avail_h: f32) -> f32 {
    let factors = column_factors(model);
    let (c, r) = (factors.len() as f32, rows(model) as f32);
    let by_width = (avail_w.min(DEVICE_MAX_WIDTH) - 2.0 * DEVICE_PAD - (c - 1.0) * DEVICE_GAP)
        / factors.iter().sum::<f32>();
    let by_height = (avail_h - 2.0 * DEVICE_PAD - (r - 1.0) * DEVICE_GAP) / r;
    by_width.min(by_height).clamp(KEY_MIN, KEY_MAX)
}

#[cfg(test)]
mod tests;
