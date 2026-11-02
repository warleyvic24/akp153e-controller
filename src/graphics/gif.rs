//! Suporte a GIF animado nas teclas.
//! Usa a crate `gif` para decodificar todos os frames com duração, e devolve o frame correto
//! conforme o tempo atual. Frames são cacheados pelo caminho do arquivo.

use gif::{Decoder, Frame};
use image::RgbaImage;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Um frame decodificado com o tempo que deve ficar na tela (ms).
#[derive(Clone, Debug)]
struct GifFrame {
    image: RgbaImage,
    delay_ms: u64,
}

/// Todo o GIF: frames em ordem e duração total (ms).
#[derive(Clone, Debug)]
struct GifData {
    frames: Vec<GifFrame>,
    total_ms: u64,
}

/// Cache de GIFs já decodificados, indexado pelo caminho do arquivo.
static GIF_CACHE: OnceLock<RwLock<HashMap<String, GifData>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<String, GifData>> {
    GIF_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Converte o buffer de um frame para RGBA. Se houver colormap, usa as cores da paleta;
/// senao, usa escala de cinza (cada byte = intensidade).
fn frame_to_rgba(frame: &Frame, colormap: Option<&[(u8, u8, u8)]>) -> Option<RgbaImage> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let mut rgba = vec![0u8; width * height * 4];

    for (i, &idx) in frame.buffer.iter().enumerate() {
        let (r, g, b) = if let Some(cm) = colormap {
            cm.get(idx as usize).copied().unwrap_or((0, 0, 0))
        } else {
            // Escala de cinza sem colormap disponível.
            (idx, idx, idx)
        };
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = 255;
    }

    RgbaImage::from_raw(width as u32, height as u32, rgba)
}

/// Decodifica um arquivo .gif e guarda os frames no cache (se ainda não estiver lá).
fn load_gif(path: &str) -> Option<GifData> {
    // Já está no cache?
    {
        let cache = cache().read().ok()?;
        if let Some(data) = cache.get(path) {
            return Some(data.clone());
        }
    }

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let decoder = Decoder::new(reader).ok()?;

    let colormap: Option<Vec<(u8, u8, u8)>> = None;

    let mut gif_frames = Vec::new();
    let mut total_ms = 0u64;

    for result in decoder {
        let frame = result.ok()?;

        let delay_ms = frame.delay as u64 * 10;
        let delay_ms = delay_ms.max(40);

        let image = frame_to_rgba(&frame, colormap.as_deref())?;
        total_ms = total_ms.saturating_add(delay_ms);
        gif_frames.push(GifFrame { image, delay_ms });
    }

    if gif_frames.is_empty() {
        return None;
    }

    let data = GifData {
        frames: gif_frames,
        total_ms,
    };
    cache().write().ok()?.insert(path.to_string(), data.clone());
    Some(data)
}

/// Devolve o frame que deve estar na tela agora (ou `now_ms` informado), e o atraso até o próximo frame (ms).
pub fn current_frame(path: &str, now_ms: Option<u64>) -> Option<(RgbaImage, u64)> {
    let data = load_gif(path)?;
    if data.frames.is_empty() {
        return None;
    }

    let now = now_ms.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    });

    let pos = now % data.total_ms;
    let mut idx = 0usize;
    let mut elapsed = 0u64;

    for (i, frame) in data.frames.iter().enumerate() {
        if pos < elapsed + frame.delay_ms {
            idx = i;
            break;
        }
        elapsed += frame.delay_ms;
    }

    let mut remaining = data.frames[idx]
        .delay_ms
        .saturating_sub(pos.saturating_sub(elapsed));
    remaining = remaining.max(40);

    Some((data.frames[idx].image.clone(), remaining))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gif_nao_existe_retorna_none() {
        assert!(current_frame("/tmp/nao_existe.gif", Some(0)).is_none());
    }
}
