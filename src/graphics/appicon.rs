//! Ícone do aplicativo (janela e bandeja): carregado do PNG gerado a partir do SVG.
use image::load_from_memory;

const ICON_PNG: &[u8] = include_bytes!("../../assets/icon-256.png");

/// Imagem `px` x `px` em RGBA; redimensionada do PNG de 256x256.
pub fn rgba(px: u32) -> Vec<u8> {
    // Só falha com um PNG corrompido no build, e o teste abaixo pega isso: fica sem ícone, sem
    // cair. (Este arquivo também entra no `build.rs`, que não tem `log`.)
    let Ok(img) = load_from_memory(ICON_PNG) else {
        return vec![0; (px * px * 4) as usize];
    };
    let resized = img.resize_exact(px, px, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    rgba.into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_matches_the_requested_size() {
        for px in [16, 32, 64] {
            assert_eq!(rgba(px).len(), (px * px * 4) as usize);
        }
    }

    #[test]
    fn corners_are_transparent_and_center_is_visible() {
        let img = rgba(64);
        // Canto deve ser transparente (alpha = 0)
        assert_eq!(img[3], 0);
        // Centro deve ter conteúdo (não totalmente transparente)
        let center_idx = ((32 * 64 + 32) * 4) as usize;
        assert_ne!(img[center_idx + 3], 0);
    }
}
