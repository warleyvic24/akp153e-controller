//! Tema do Omarchy: lê o `colors.toml` do tema ativo e o converte numa paleta da interface.
//! A paleta sai sempre dentro dos contrastes WCAG do projeto, qualquer que seja o tema (escuro,
//! claro ou com cores fracas): o que falta de contraste é corrigido, não repassado.
use super::theme::{ensure_contrast, mix, readable_on, with_control, Palette, DEVICE_FG};
use crate::editor::tokens::TEXT_MIN_CONTRAST;
use crate::platform::omarchy as source;
use eframe::egui::Color32;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// De quanto em quanto tempo se confere se o Omarchy trocou de tema.
const POLL_EVERY: Duration = Duration::from_secs(2);
const FALLBACK_ACCENT: Color32 = Color32::from_rgb(0, 229, 255);
/// Quanto cada superfície derivada se afasta do fundo do tema.
const DARK_PAGE_SHADE: f32 = 0.35;
const LIGHT_PAGE_SHADE: f32 = 0.06;
const RAISE_TINT: f32 = 0.05;
const FIELD_SHADE: f32 = 0.2;
const LIGHT_FIELD_SHADE: f32 = 0.03;
const LINE_TINT: f32 = 0.18;
const MUTED_TINT: f32 = 0.6;
const DEVICE_SHADE: f32 = 0.4;
const BEZEL_TINT: f32 = 0.12;
/// Aparelho escuro nos dois modos, como nos temas do próprio programa.
const LIGHT_DEVICE: Color32 = Color32::from_rgb(20, 23, 29);

type Colors = HashMap<String, Color32>;

/// Paleta, destaque e modo (escuro ou claro) de um tema do Omarchy.
#[derive(Clone, Copy)]
pub struct OmarchyTheme {
    pub palette: Palette,
    pub accent: Color32,
    pub dark: bool,
}

/// Lê as linhas `chave = "#rrggbb"` do `colors.toml`. Aceita aspas simples ou duplas, `#` ou `0x`
/// no valor e comentários; linhas que não são cores são ignoradas.
pub fn parse_colors(text: &str) -> Colors {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with('[') {
                return None;
            }
            let (key, rest) = line.split_once('=')?;
            let value = value_token(rest.trim());
            Some((key.trim().to_string(), parse_hex(value)?))
        })
        .collect()
}

/// O valor sem aspas e sem o comentário que vem depois.
fn value_token(rest: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = rest.strip_prefix(quote) {
            return inner.split(quote).next().unwrap_or_default();
        }
    }
    rest.split_whitespace().next().unwrap_or_default()
}

fn parse_hex(value: &str) -> Option<Color32> {
    let hex = value
        .trim_start_matches('#')
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// A paleta do tema; `None` se faltar fundo ou texto, que não dá para inventar.
pub fn from_colors(colors: &Colors) -> Option<OmarchyTheme> {
    let get = |keys: &[&str]| keys.iter().find_map(|k| colors.get(*k).copied());
    let background = get(&["background"])?;
    let foreground = get(&["foreground"])?;
    let dark = readable_on(background) == Color32::WHITE;
    let (toward, away) = if dark {
        (Color32::WHITE, Color32::BLACK)
    } else {
        (Color32::BLACK, Color32::WHITE)
    };
    let panel = background;
    let (bg, raise, field) = if dark {
        (
            get(&["darker_background"]).unwrap_or_else(|| mix(panel, away, DARK_PAGE_SHADE)),
            get(&["lighter_background"]).unwrap_or_else(|| mix(panel, toward, RAISE_TINT)),
            get(&["dark_background"]).unwrap_or_else(|| mix(panel, away, FIELD_SHADE)),
        )
    } else {
        (
            mix(panel, toward, LIGHT_PAGE_SHADE),
            mix(panel, toward, RAISE_TINT / 2.0),
            mix(panel, toward, LIGHT_FIELD_SHADE),
        )
    };
    let line = get(&["selection"]).unwrap_or_else(|| mix(panel, foreground, LINE_TINT));
    let surfaces = [panel, raise, bg];
    let readable = |c: Color32| {
        surfaces
            .iter()
            .fold(c, |c, s| ensure_contrast(c, *s, TEXT_MIN_CONTRAST))
    };
    let muted =
        get(&["dark_foreground", "muted"]).unwrap_or_else(|| mix(panel, foreground, MUTED_TINT));
    let device = if dark {
        mix(bg, Color32::BLACK, DEVICE_SHADE)
    } else {
        LIGHT_DEVICE
    };
    let palette = Palette {
        bg,
        panel,
        raise,
        field,
        line,
        fg: readable(foreground),
        muted: readable(muted),
        device,
        bezel: mix(device, foreground, BEZEL_TINT),
        keyoff: Color32::BLACK,
        ok: readable(get(&["green"]).unwrap_or(Color32::from_rgb(52, 211, 153))),
        bad: readable(get(&["red"]).unwrap_or(Color32::from_rgb(248, 113, 113))),
        control: Color32::TRANSPARENT,
        device_fg: Color32::from_rgb(DEVICE_FG[0], DEVICE_FG[1], DEVICE_FG[2]),
    };
    Some(OmarchyTheme {
        palette: with_control(palette),
        accent: get(&["accent", "blue"]).unwrap_or(FALLBACK_ACCENT),
        dark,
    })
}

struct Cache {
    /// Texto do `colors.toml` na última leitura.
    text: Option<String>,
    theme: Option<OmarchyTheme>,
    checked: Option<Instant>,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache {
    text: None,
    theme: None,
    checked: None,
});

fn lock() -> std::sync::MutexGuard<'static, Cache> {
    CACHE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Relê o arquivo e guarda o resultado; `true` se o tema mudou desde a leitura anterior.
fn reload(cache: &mut Cache) -> bool {
    cache.checked = Some(Instant::now());
    let text = source::read_colors();
    if text == cache.text {
        return false;
    }
    cache.theme = text.as_deref().and_then(|t| from_colors(&parse_colors(t)));
    cache.text = text;
    true
}

/// O tema ativo do Omarchy, ou `None` fora dele (ou se o arquivo não tiver as cores básicas).
pub fn current() -> Option<OmarchyTheme> {
    let mut cache = lock();
    if cache.checked.is_none() {
        reload(&mut cache);
    }
    cache.theme
}

/// Confere se o Omarchy trocou de tema (no máximo a cada `POLL_EVERY`). `true` = mudou.
pub fn refresh() -> bool {
    let mut cache = lock();
    if cache.checked.is_some_and(|at| at.elapsed() < POLL_EVERY) {
        return false;
    }
    reload(&mut cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::theme::contrast;
    use crate::editor::tokens::COMPONENT_MIN_CONTRAST;

    const TOKYO_NIGHT: &str = r##"
accent = "#7aa2f7"
selection = "#292e42"
muted = "#414868"
background = "#1a1b26"
dark_background = "#13141c"
darker_background = "#0e0e14"
lighter_background = "#24283b"
foreground = "#a9b1d6"
dark_foreground = "#565f89"
red = "#f7768e"
green = "#9ece6a"
blue = "#7aa2f7"
"##;

    fn theme(text: &str) -> OmarchyTheme {
        from_colors(&parse_colors(text)).expect("tema válido")
    }

    fn assert_accessible(t: &OmarchyTheme, what: &str) {
        let p = &t.palette;
        for (name, fg) in [
            ("fg", p.fg),
            ("muted", p.muted),
            ("ok", p.ok),
            ("bad", p.bad),
        ] {
            for (surface, bg) in [("panel", p.panel), ("raise", p.raise), ("bg", p.bg)] {
                let ratio = contrast(fg, bg);
                assert!(
                    ratio >= TEXT_MIN_CONTRAST,
                    "{what}: {name}/{surface} {ratio:.2}"
                );
            }
        }
        for (surface, bg) in [("panel", p.panel), ("raise", p.raise)] {
            let ratio = contrast(p.control, bg);
            assert!(
                ratio >= COMPONENT_MIN_CONTRAST,
                "{what}: borda/{surface} {ratio:.2}"
            );
        }
        assert!(
            contrast(p.device_fg, p.keyoff) >= TEXT_MIN_CONTRAST,
            "{what}: aparelho"
        );
    }

    #[test]
    fn parses_quotes_comments_and_hex_styles() {
        let c = parse_colors(
            "# comentário\n[cores]\na = \"#112233\"\nb = '#445566' # fim\nc = 0x778899\nd = aabbcc\ne = \"nao\"\nf=\n",
        );
        assert_eq!(c["a"], Color32::from_rgb(0x11, 0x22, 0x33));
        assert_eq!(c["b"], Color32::from_rgb(0x44, 0x55, 0x66));
        assert_eq!(c["c"], Color32::from_rgb(0x77, 0x88, 0x99));
        assert_eq!(c["d"], Color32::from_rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(c.len(), 4, "o que não é cor é ignorado");
    }

    #[test]
    fn tokyo_night_is_a_dark_theme_with_its_own_accent() {
        let t = theme(TOKYO_NIGHT);
        assert!(t.dark);
        assert_eq!(t.accent, Color32::from_rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(t.palette.panel, Color32::from_rgb(0x1a, 0x1b, 0x26));
        assert_accessible(&t, "tokyo-night");
    }

    #[test]
    fn a_white_theme_is_light_and_accessible() {
        let t = theme("background = \"#ffffff\"\nforeground = \"#000000\"\naccent = \"#0969da\"\n");
        assert!(!t.dark);
        assert_accessible(&t, "white");
    }

    #[test]
    fn weak_contrast_is_corrected_not_passed_on() {
        // Texto quase igual ao fundo, em tema escuro e em claro.
        for (bg, fg) in [
            ("#202020", "#222222"),
            ("#f0f0f0", "#eeeeee"),
            ("#777777", "#7a7a7a"),
        ] {
            let t = theme(&format!("background = \"{bg}\"\nforeground = \"{fg}\"\n"));
            assert_accessible(&t, bg);
        }
    }

    #[test]
    fn every_background_and_text_pair_ends_up_accessible() {
        const STEP: usize = 51;
        for r in (0..=255).step_by(STEP) {
            for g in (0..=255).step_by(STEP) {
                for b in (0..=255).step_by(STEP) {
                    let color = format!("#{r:02x}{g:02x}{b:02x}");
                    let flipped = format!("#{b:02x}{r:02x}{g:02x}");
                    let t = theme(&format!(
                        "background = \"{color}\"\nforeground = \"{flipped}\"\nred = \"{flipped}\"\ngreen = \"{color}\"\n"
                    ));
                    assert_accessible(&t, &color);
                }
            }
        }
    }

    #[test]
    fn without_background_or_foreground_there_is_no_theme() {
        assert!(from_colors(&parse_colors("accent = \"#ffffff\"")).is_none());
        assert!(from_colors(&parse_colors("background = \"#000000\"")).is_none());
        assert!(from_colors(&parse_colors("")).is_none());
    }

    #[test]
    fn accent_falls_back_to_blue_then_to_the_default() {
        let base = "background = \"#101010\"\nforeground = \"#eeeeee\"\n";
        assert_eq!(
            theme(&format!("{base}blue = \"#0000ff\"")).accent,
            Color32::from_rgb(0, 0, 255)
        );
        assert_eq!(theme(base).accent, FALLBACK_ACCENT);
    }
}
