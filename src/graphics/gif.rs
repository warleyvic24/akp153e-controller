//! Suporte a GIF animado nas teclas.
//!
//! O decodificador do `image` compõe cada quadro sobre o anterior (respeitando o método de
//! descarte, a paleta local/global e o deslocamento do quadro), então cada quadro guardado é a
//! imagem inteira como deve aparecer. Os quadros ficam em memória, já reduzidos ao tamanho de
//! uma tecla, num LRU com orçamento de bytes; o carimbo do arquivo (data + tamanho) invalida a
//! entrada quando o GIF é trocado no disco.
use super::cache::{ByteLru, FileStamp};
use image::codecs::gif::GifDecoder;
use image::{imageops, AnimationDecoder, RgbaImage};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maior lado guardado de cada quadro: a tecla tem ~100 px e a prévia do editor um pouco mais.
const MAX_SIDE: u32 = 144;
/// Quadros além deste número são ignorados (um GIF de minutos não vira centenas de MB).
const MAX_FRAMES: usize = 600;
/// Orçamento dos quadros decodificados de todos os GIFs juntos.
pub const GIF_BUDGET_BYTES: usize = 24 * 1024 * 1024;
/// Atraso de quadro abaixo disto vira `DEFAULT_DELAY_MS`, como fazem os navegadores.
const MIN_DELAY_MS: u64 = 20;
const DEFAULT_DELAY_MS: u64 = 100;

/// Um quadro composto com o tempo que fica na tela (ms).
#[derive(Debug)]
struct GifFrame {
    image: Arc<RgbaImage>,
    delay_ms: u64,
}

/// O GIF inteiro: quadros em ordem e duração total (ms).
#[derive(Debug)]
struct GifData {
    frames: Vec<GifFrame>,
    total_ms: u64,
}

impl GifData {
    fn bytes(&self) -> usize {
        self.frames.iter().map(|f| f.image.as_raw().len()).sum()
    }

    /// (índice do quadro em `now_ms`, ms até o próximo quadro).
    fn at(&self, now_ms: u64) -> (usize, u64) {
        let pos = now_ms % self.total_ms.max(1);
        let mut start = 0u64;
        for (i, frame) in self.frames.iter().enumerate() {
            if pos < start + frame.delay_ms {
                return (i, start + frame.delay_ms - pos);
            }
            start += frame.delay_ms;
        }
        (self.frames.len() - 1, 1)
    }
}

fn cache() -> &'static ByteLru<PathBuf, Arc<GifData>> {
    static CACHE: OnceLock<ByteLru<PathBuf, Arc<GifData>>> = OnceLock::new();
    CACHE.get_or_init(|| ByteLru::new(GIF_BUDGET_BYTES))
}

fn delay_ms(frame: &image::Frame) -> u64 {
    let (num, den) = frame.delay().numer_denom_ms();
    let ms = u64::from(num) / u64::from(den.max(1));
    if ms < MIN_DELAY_MS {
        DEFAULT_DELAY_MS
    } else {
        ms
    }
}

/// Reduz o quadro para caber em `MAX_SIDE` (mantém a proporção); quadros pequenos ficam iguais.
fn shrink(img: RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w <= MAX_SIDE && h <= MAX_SIDE {
        return img;
    }
    let scale = MAX_SIDE as f32 / w.max(h) as f32;
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    imageops::resize(&img, nw, nh, imageops::FilterType::Triangle)
}

fn decode(path: &Path) -> Option<GifData> {
    let reader = BufReader::new(File::open(path).ok()?);
    let decoder = match GifDecoder::new(reader) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("GIF inválido {}: {e}", path.display());
            return None;
        }
    };
    let mut frames = Vec::new();
    let mut total_ms = 0u64;
    for frame in decoder.into_frames().take(MAX_FRAMES) {
        let frame = match frame {
            Ok(f) => f,
            // Um GIF truncado mostra os quadros que vieram inteiros.
            Err(e) => {
                log::warn!("GIF {}: {e}", path.display());
                break;
            }
        };
        let delay_ms = delay_ms(&frame);
        total_ms = total_ms.saturating_add(delay_ms);
        frames.push(GifFrame {
            image: Arc::new(shrink(frame.into_buffer())),
            delay_ms,
        });
    }
    (!frames.is_empty()).then_some(GifData { frames, total_ms })
}

/// O GIF de `path`, do cache se o arquivo não mudou desde a última decodificação.
fn load(path: &Path) -> Option<Arc<GifData>> {
    let stamp = FileStamp::of(path)?;
    let key = path.to_path_buf();
    if let Some(data) = cache().get(&key, stamp) {
        return Some(data);
    }
    let data = Arc::new(decode(path)?);
    cache().insert(key, stamp, data.clone(), data.bytes());
    Some(data)
}

fn now_or(now_ms: Option<u64>) -> u64 {
    now_ms.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    })
}

/// (índice do quadro, quadro, ms até o próximo) em `now_ms` (ou agora). Sem cópia da imagem.
pub fn frame_at(path: &str, now_ms: Option<u64>) -> Option<(usize, Arc<RgbaImage>, u64)> {
    let data = load(Path::new(path))?;
    let (index, remaining) = data.at(now_or(now_ms));
    Some((index, data.frames[index].image.clone(), remaining))
}

/// API mantida para quem precisa de uma cópia própria; o renderizador usa `frame_at`.
#[cfg_attr(not(test), allow(dead_code))]
/// Devolve o frame que deve estar na tela agora (ou `now_ms` informado), e o atraso até o próximo frame (ms).
pub fn current_frame(path: &str, now_ms: Option<u64>) -> Option<(RgbaImage, u64)> {
    frame_at(path, now_ms).map(|(_, img, remaining)| ((*img).clone(), remaining))
}

/// Bytes ocupados pelos quadros guardados.
#[cfg(test)]
pub fn cached_bytes() -> usize {
    cache().bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, Rgba};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keyforge-gif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// Grava um GIF com um quadro de cor chapada por item de `colors`, cada um com `delay` ms.
    fn write_gif(path: &Path, size: u32, colors: &[[u8; 3]], delay: u32) {
        let file = File::create(path).unwrap();
        let mut enc = GifEncoder::new(file);
        for c in colors {
            let img = RgbaImage::from_pixel(size, size, Rgba([c[0], c[1], c[2], 255]));
            enc.encode_frame(Frame::from_parts(
                img,
                0,
                0,
                Delay::from_numer_denom_ms(delay, 1),
            ))
            .unwrap();
        }
    }

    fn close(a: [u8; 4], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 8)
    }

    #[test]
    fn gif_nao_existe_retorna_none() {
        assert!(current_frame("/tmp/nao_existe.gif", Some(0)).is_none());
    }

    #[test]
    fn colors_come_from_the_palette_not_grayscale() {
        let path = scratch("palette.gif");
        write_gif(&path, 8, &[[255, 0, 0], [0, 0, 255]], 100);
        let p = path.to_str().unwrap();
        let (red, remaining) = current_frame(p, Some(0)).unwrap();
        assert!(
            close(red.get_pixel(4, 4).0, [255, 0, 0]),
            "{:?}",
            red.get_pixel(4, 4)
        );
        assert_eq!(remaining, 100);
        let (blue, _) = current_frame(p, Some(150)).unwrap();
        assert!(
            close(blue.get_pixel(4, 4).0, [0, 0, 255]),
            "{:?}",
            blue.get_pixel(4, 4)
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn frames_are_decoded_once_and_shared() {
        let path = scratch("shared.gif");
        write_gif(&path, 8, &[[0, 255, 0], [9, 9, 9]], 50);
        let p = path.to_str().unwrap();
        let (_, a, _) = frame_at(p, Some(0)).unwrap();
        let (_, b, _) = frame_at(p, Some(10)).unwrap();
        assert!(
            Arc::ptr_eq(&a, &b),
            "o mesmo quadro, sem decodificar de novo"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_replaced_gif_is_decoded_again() {
        let path = scratch("replaced.gif");
        write_gif(&path, 8, &[[255, 0, 0]], 100);
        let p = path.to_str().unwrap();
        assert!(close(
            current_frame(p, Some(0)).unwrap().0.get_pixel(0, 0).0,
            [255, 0, 0]
        ));
        write_gif(&path, 10, &[[0, 255, 0], [0, 255, 0]], 100);
        let (img, _) = current_frame(p, Some(0)).unwrap();
        assert_eq!(img.width(), 10);
        assert!(close(img.get_pixel(0, 0).0, [0, 255, 0]));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn big_gifs_are_shrunk_to_key_size() {
        let path = scratch("big.gif");
        write_gif(&path, 400, &[[1, 2, 3]], 100);
        let (img, _) = current_frame(path.to_str().unwrap(), Some(0)).unwrap();
        assert_eq!(img.dimensions(), (MAX_SIDE, MAX_SIDE));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn tiny_delays_become_the_browser_default() {
        let path = scratch("fast.gif");
        write_gif(&path, 4, &[[1, 1, 1], [2, 2, 2]], 0);
        let (_, _, remaining) = frame_at(path.to_str().unwrap(), Some(0)).unwrap();
        assert_eq!(remaining, DEFAULT_DELAY_MS);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn frame_lookup_wraps_around_the_loop() {
        let data = GifData {
            frames: (0..3)
                .map(|_| GifFrame {
                    image: Arc::new(RgbaImage::new(1, 1)),
                    delay_ms: 100,
                })
                .collect(),
            total_ms: 300,
        };
        assert_eq!(data.at(0), (0, 100));
        assert_eq!(data.at(150), (1, 50));
        assert_eq!(data.at(299), (2, 1));
        assert_eq!(data.at(300), (0, 100));
    }
}
