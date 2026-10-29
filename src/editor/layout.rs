//! Quais painéis cabem na largura da janela. Num gerenciador lado a lado (Hyprland, telas
//! ultralargas divididas em várias janelas) o editor pode ganhar uma faixa estreita: a biblioteca
//! vira um painel que se abre pelo botão "Ações" no lugar do inspetor, e o inspetor encolhe.

/// Largura da biblioteca de ações (coluna da esquerda).
pub(crate) const LIBRARY_WIDTH: f32 = 264.0;
/// Largura do inspetor quando há espaço.
pub(crate) const INSPECTOR_WIDTH: f32 = 340.0;
/// Menor inspetor que ainda mostra os campos sem cortar.
const INSPECTOR_MIN: f32 = 260.0;
/// Palco com cinco teclas no tamanho mínimo, moldura e margens.
const STAGE_MIN: f32 = 380.0;
/// Barra de cima: abaixo disto o logotipo e o rótulo "Perfil" saem para caber o resto.
const TOP_BAR_FULL: f32 = 860.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Panels {
    /// Biblioteca e inspetor não cabem juntos: um de cada vez, com o botão "Ações".
    pub compact: bool,
    pub library: bool,
    pub inspector: bool,
    pub inspector_width: f32,
    /// Barra de cima completa (logotipo e rótulo "Perfil").
    pub full_top_bar: bool,
}

/// Painéis para a largura `width` (em pontos); `library_open` só vale no modo compacto.
pub(crate) fn panels(width: f32, library_open: bool) -> Panels {
    let compact = width < LIBRARY_WIDTH + INSPECTOR_WIDTH + STAGE_MIN;
    let full_top_bar = width >= TOP_BAR_FULL;
    if !compact {
        return Panels {
            compact,
            library: true,
            inspector: true,
            inspector_width: INSPECTOR_WIDTH,
            full_top_bar,
        };
    }
    Panels {
        compact,
        library: library_open,
        inspector: !library_open,
        inspector_width: (width - STAGE_MIN).clamp(INSPECTOR_MIN, INSPECTOR_WIDTH),
        full_top_bar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_window_shows_everything() {
        let p = panels(1500.0, false);
        assert!(!p.compact && p.library && p.inspector && p.full_top_bar);
        assert_eq!(p.inspector_width, INSPECTOR_WIDTH);
    }

    #[test]
    fn a_narrow_window_shows_one_side_panel_at_a_time() {
        let closed = panels(749.0, false);
        assert!(closed.compact && !closed.library && closed.inspector);
        assert!(!closed.full_top_bar);
        let open = panels(749.0, true);
        assert!(open.library && !open.inspector);
    }

    #[test]
    fn the_inspector_shrinks_but_not_below_its_minimum() {
        assert_eq!(panels(700.0, false).inspector_width, 700.0 - STAGE_MIN);
        assert_eq!(panels(500.0, false).inspector_width, INSPECTOR_MIN);
        assert_eq!(panels(900.0, false).inspector_width, INSPECTOR_WIDTH);
    }

    #[test]
    fn side_panels_and_the_stage_fit_exactly_at_the_breakpoint() {
        let edge = LIBRARY_WIDTH + INSPECTOR_WIDTH + STAGE_MIN;
        assert!(panels(edge - 1.0, false).compact);
        assert!(!panels(edge, false).compact);
    }
}
