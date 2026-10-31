//! Pontos de partida: uma página vazia vira, num clique, uma página pronta (Streaming, Trabalho,
//! Mídia) feita com as ações da biblioteca, nas cores e ícones delas. Quem nunca usou o programa
//! sai de 15 quadrados pretos para um aparelho útil, e depois troca o que quiser.
use super::library::Preset;
use crate::config::schema::{AppConfig, ButtonConfig, Page, RGBAColor, SideScreen};

/// Uma tecla do modelo: (categoria, nome) da ação na biblioteca.
type Slot = (&'static str, &'static str);

pub(super) struct Template {
    pub name: &'static str,
    keys: &'static [Slot],
}

pub(super) const TEMPLATES: [Template; 3] = [
    Template {
        name: "Streaming",
        keys: &[
            ("OBS", "Gravar"),
            ("OBS", "Transmitir"),
            ("OBS", "Pausar gravação"),
            ("OBS", "Câmera virtual"),
            ("OBS", "Salvar replay"),
            ("Mídia", "Mic mudo"),
            ("Mídia", "Mudo"),
            ("Mídia", "Vol -"),
            ("Mídia", "Vol +"),
            ("Mídia", "Mixer"),
            ("Mídia", "Play/Pause"),
            ("Mídia", "Anterior"),
            ("Mídia", "Próxima"),
            ("Sistema", "Captura"),
            ("Aparelho", "Relógio"),
        ],
    },
    Template {
        name: "Trabalho",
        keys: &[
            ("Edição", "Copiar"),
            ("Edição", "Colar"),
            ("Edição", "Desfazer"),
            ("Edição", "Salvar"),
            ("Sistema", "Captura"),
            ("Sistema", "Bloquear"),
            ("Mídia", "Mic mudo"),
            ("Mídia", "Mudo"),
            ("Mídia", "Vol -"),
            ("Mídia", "Vol +"),
            ("Mídia", "Play/Pause"),
            ("Mídia", "Próxima"),
            ("Aparelho", "Monitor"),
            ("Aparelho", "Relógio"),
            ("Aparelho", "Brilho +"),
        ],
    },
    Template {
        name: "Mídia",
        keys: &[
            ("Mídia", "Anterior"),
            ("Mídia", "Play/Pause"),
            ("Mídia", "Próxima"),
            ("Mídia", "Mixer"),
            ("Mídia", "Tocar som"),
            ("Mídia", "Vol -"),
            ("Mídia", "Mudo"),
            ("Mídia", "Vol +"),
            ("Mídia", "Mic mudo"),
            ("Aparelho", "Relógio"),
            ("Aparelho", "Brilho −"),
            ("Aparelho", "Brilho +"),
            ("Aparelho", "Monitor"),
        ],
    },
];

/// A tecla que a ação da biblioteca daria (como ao arrastá-la para uma tecla vazia).
pub(super) fn preset_button(p: &Preset, cfg: &AppConfig) -> ButtonConfig {
    let mut b = ButtonConfig::blank();
    b.action = p.resolved_action(cfg);
    b.appearance.text = p.label.to_string();
    b.appearance.icon = Some(p.icon.to_string());
    b.appearance.background = RGBAColor {
        r: p.color[0],
        g: p.color[1],
        b: p.color[2],
        a: 255,
    };
    b
}

/// Preenche `page` com o modelo, na ordem das teclas; o que não existe neste sistema fica vazio.
/// A lateral vira a automática (música, alertas da live e relógio).
pub(super) fn fill(t: &Template, page: &mut Page, presets: &[Preset], cfg: &AppConfig) {
    for (b, (category, label)) in page.buttons.iter_mut().zip(t.keys) {
        *b = presets
            .iter()
            .find(|p| p.category == *category && p.label == *label)
            .map_or_else(ButtonConfig::blank, |p| preset_button(p, cfg));
    }
    page.side = SideScreen::NowPlaying;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::ButtonAction;

    #[test]
    fn every_template_key_exists_in_the_library() {
        let presets = super::super::library::presets();
        for t in &TEMPLATES {
            for (category, label) in t.keys {
                // Itens só do Windows somem no Linux; os demais têm de existir.
                let found = presets
                    .iter()
                    .any(|p| p.category == *category && p.label == *label);
                assert!(
                    found,
                    "{}: {category}/{label} não está na biblioteca",
                    t.name
                );
            }
        }
    }

    #[test]
    fn a_template_fills_the_page_and_turns_on_the_smart_side() {
        let cfg = AppConfig::default();
        let mut page = Page::new("P", 3, 5);
        fill(
            &TEMPLATES[0],
            &mut page,
            &super::super::library::presets(),
            &cfg,
        );
        assert!(matches!(page.buttons[0].action, ButtonAction::Obs { .. }));
        assert_eq!(page.buttons[0].appearance.text, "Gravar");
        assert_eq!(page.side, SideScreen::NowPlaying);
        assert!(page.buttons.iter().all(|b| b.action != ButtonAction::None));
    }
}
