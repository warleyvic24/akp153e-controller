//! Ícone do aplicativo (janela e bandeja): grade 3 x 3 de teclas sobre fundo escuro, desenhada em código.
const BACKGROUND: [u8; 3] = [10, 12, 26];
const KEY: [u8; 3] = [0, 229, 255];
const KEY_CENTER: [u8; 3] = [255, 255, 255];
const GRID: u32 = 3;

/// Imagem `px` x `px` em RGBA; fora dos cantos arredondados fica transparente.
pub fn rgba(px: u32) -> Vec<u8> {
    let n = px as f32;
    let margin = n / 8.0;
    let gap = n / 14.0;
    let cell = (n - 2.0 * margin - gap * (GRID - 1) as f32) / GRID as f32;
    let mut out = Vec::with_capacity((px * px * 4) as usize);
    for y in 0..px {
        for x in 0..px {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let pixel = if inside_rounded(fx, fy, (0.0, 0.0, n, n), n / 5.0) {
                let color = key_at(fx, fy, margin, gap, cell).map_or(BACKGROUND, |center| {
                    if center {
                        KEY_CENTER
                    } else {
                        KEY
                    }
                });
                [color[0], color[1], color[2], 255]
            } else {
                [0, 0, 0, 0]
            };
            out.extend(pixel);
        }
    }
    out
}

/// A tecla sob o ponto: `Some(true)` se for a central.
fn key_at(x: f32, y: f32, margin: f32, gap: f32, cell: f32) -> Option<bool> {
    (0..GRID)
        .flat_map(|row| (0..GRID).map(move |col| (row, col)))
        .find(|&(row, col)| {
            let left = margin + col as f32 * (cell + gap);
            let top = margin + row as f32 * (cell + gap);
            inside_rounded(x, y, (left, top, cell, cell), cell * 0.25)
        })
        .map(|(row, col)| row == 1 && col == 1)
}

/// `rect` = (esquerda, topo, largura, altura).
fn inside_rounded(x: f32, y: f32, rect: (f32, f32, f32, f32), radius: f32) -> bool {
    let (left, top, w, h) = rect;
    let nearest_x = x.clamp(left + radius, left + w - radius);
    let nearest_y = y.clamp(top + radius, top + h - radius);
    x >= left
        && x <= left + w
        && y >= top
        && y <= top + h
        && (x - nearest_x).hypot(y - nearest_y) <= radius
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(img: &[u8], px: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * px + x) * 4) as usize;
        [img[i], img[i + 1], img[i + 2], img[i + 3]]
    }

    #[test]
    fn buffer_matches_the_requested_size() {
        for px in [16, 32, 64] {
            assert_eq!(rgba(px).len(), (px * px * 4) as usize);
        }
    }

    #[test]
    fn corners_are_transparent_and_the_center_key_is_white() {
        let img = rgba(64);
        assert_eq!(pixel(&img, 64, 0, 0)[3], 0);
        assert_eq!(pixel(&img, 64, 32, 32), [255, 255, 255, 255]);
    }

    #[test]
    fn outer_keys_use_the_accent_cyan() {
        let img = rgba(64);
        assert_eq!(pixel(&img, 64, 14, 14), [0, 229, 255, 255]);
    }
}
