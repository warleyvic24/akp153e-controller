//! Tokens do design system (veja `docs/DESIGN.md`): escalas de espaço, raio e tipografia,
//! mais os mínimos de contraste que os testes de `theme.rs` impõem. Cores ficam em `theme::Palette`.

/// Escala de espaçamento (px). Use estes degraus em vez de números soltos.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
}

/// Raios de canto (px).
pub mod radius {
    /// Botões, campos, tabelas pequenas.
    pub const CONTROL: f32 = 8.0;
    /// Cartões e janelas.
    pub const CARD: f32 = 12.0;
    /// Tecla do aparelho.
    pub const KEY: f32 = 10.0;
    /// Corpo do aparelho desenhado no palco.
    pub const DEVICE: f32 = 22.0;
    /// Pílulas e chips (totalmente redondos).
    pub const PILL: f32 = 99.0;
}

/// Escala tipográfica (px). Nenhum texto abaixo de `CAPTION`.
pub mod text {
    pub const CAPTION: f32 = 12.0;
    pub const BODY: f32 = 14.0;
    pub const TITLE: f32 = 20.0;
    pub const DISPLAY: f32 = 22.0;
}

/// Alvo mínimo de clique (WCAG 2.2, critério 2.5.8).
pub const TARGET_MIN: f32 = 24.0;

/// WCAG 1.4.3: texto normal.
pub const TEXT_MIN_CONTRAST: f32 = 4.5;
/// WCAG 1.4.11: bordas de controles, ícones e indicadores de estado.
pub const COMPONENT_MIN_CONTRAST: f32 = 3.0;
