//! Tokens do design system (veja `docs/DESIGN.md`): escalas de espaço, raio e tipografia,
//! mais os mínimos de contraste que os testes de `theme.rs` impõem. Cores ficam em `theme::Palette`.

/// Escala de espaçamento (px). Use estes degraus em vez de números soltos.
/// Base 4px: 4/6/10/16/24/32.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 10.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
}

/// Raios de canto (px).
/// Degraus 8/12/20 + pílula: quanto maior a peça, mais redonda.
pub mod radius {
    /// Botões, campos, chips.
    pub const CONTROL: f32 = 8.0;
    /// Cards, modais, dropdowns.
    pub const CARD: f32 = 12.0;
    /// Tecla do aparelho.
    pub const KEY: f32 = 12.0;
    /// Corpo do aparelho desenhado no palco.
    pub const DEVICE: f32 = 20.0;
    /// Pílulas e chips (totalmente redondos).
    pub const PILL: f32 = 99.0;
}

/// Escala tipográfica (px). Nenhum texto abaixo de `CAPTION`.
/// Escala modular (~1,2x) de 11,5 a 24 px.
pub mod text {
    pub const CAPTION: f32 = 11.5; // Smallest - labels, hints
    pub const SMALL: f32 = 12.5; // Secondary text
    pub const BODY: f32 = 13.5; // Base body text
    pub const BODY_LG: f32 = 14.5; // Larger body
    pub const TITLE: f32 = 16.5; // Section titles
    pub const TITLE_LG: f32 = 19.0; // Large titles
    pub const DISPLAY: f32 = 24.0; // Hero display
}

/// Duração das transições (s). Curtas o bastante para nunca atrasar a mão, longas o bastante
/// para o olho acompanhar a mudança de estado em vez de ver um "pulo".
pub mod motion {
    /// Hover, pressionado, foco: resposta imediata ao ponteiro.
    pub const FAST: f32 = 0.10;
    /// Marcar/desmarcar, abrir seção, trocar de aba.
    pub const BASE: f32 = 0.16;
}

/// Elevação: sombras pretas em degraus. No preto puro a sombra quase não se vê, então a
/// elevação no tema escuro vem sobretudo do filete e do degrau de superfície; a sombra pesa no claro.
pub mod elevation {
    /// Cartões e itens erguidos no painel.
    pub const CARD: (i8, u8) = (4, 16);
    /// Menus, dicas e listas suspensas.
    pub const POPUP: (i8, u8) = (8, 24);
    /// Modais: o plano mais alto.
    pub const MODAL: (i8, u8) = (24, 56);
}

/// Alvo mínimo de clique (WCAG 2.2, critério 2.5.8).
pub const TARGET_MIN: f32 = 28.0; // Increased from 24 for better touch targets

/// WCAG 1.4.3: texto normal.
pub const TEXT_MIN_CONTRAST: f32 = 4.5;
/// WCAG 1.4.6 (AAA): contraste reforçado; as paletas fixas cumprem este em todo texto.
#[cfg(test)]
pub const TEXT_AAA_CONTRAST: f32 = 7.0;
/// WCAG 1.4.11: bordas de controles, ícones e indicadores de estado.
pub const COMPONENT_MIN_CONTRAST: f32 = 3.0;
