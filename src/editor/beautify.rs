//! "Embelezar página": dá à página inteira um visual coerente de uma vez. Cada tecla configurada
//! ganha a cor da sua categoria (a mesma da biblioteca), fundo em degradê, um rótulo curto quando
//! não tem nenhum e um ícone quando falta. O usuário desfaz com Ctrl+Z ou pelo aviso.
use super::{inspector, is_blank, library, App};
use crate::config::schema::{ButtonAction, ButtonConfig, Page, RGBAColor};
use crate::graphics::live_art;
use crate::service::runtime;
use crate::strings::tf;

impl App {
    /// Botão "Embelezar página": aplica à página ativa e oferece desfazer.
    pub(super) fn beautify_page(&mut self, g: &mut runtime::Shared) {
        let changed = beautify(g.cfg.page_mut(), &self.presets);
        if changed == 0 {
            self.toasts
                .push(crate::strings::t("A página já está embelezada"), None);
            return;
        }
        self.touch();
        self.toasts
            .push(tf("{} teclas embelezadas", &[&changed]), Some("Desfazer"));
    }
}

/// Embeleza `page`; devolve quantas teclas mudaram.
pub(super) fn beautify(page: &mut Page, presets: &[library::Preset]) -> usize {
    page.buttons
        .iter_mut()
        .filter(|b| !is_blank(b))
        .map(|b| {
            let before = b.clone();
            beautify_key(b, presets);
            usize::from(*b != before)
        })
        .sum()
}

fn beautify_key(b: &mut ButtonConfig, presets: &[library::Preset]) {
    if let Some([r, g, bl]) = library::category_color(&b.action) {
        b.appearance.background = RGBAColor {
            r,
            g,
            b: bl,
            a: 255,
        };
    }
    b.appearance.gradient = true;
    // Teclas que desenham o próprio conteúdo (relógio, monitor, volume e música ao vivo) não
    // ganham rótulo nem ícone: o dado ao vivo é o rótulo.
    let draws_itself = matches!(b.action, ButtonAction::Clock | ButtonAction::Monitor { .. })
        || live_art::wants(b).any();
    if draws_itself {
        return;
    }
    let has_image = b.appearance.icon.is_some() || b.appearance.icon_path.is_some();
    if !has_image {
        if let Some(p) = presets.iter().find(|p| p.action == b.action) {
            b.appearance.icon = Some(p.icon.to_string());
        }
    }
    if b.appearance.text.trim().is_empty() {
        if let Some(title) = inspector::key_title(b) {
            b.appearance.text = title;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{Metric, BLANK_BACKGROUND};

    fn page_with(actions: Vec<ButtonAction>) -> Page {
        let mut page = Page::new("P", 3, 5);
        for (b, a) in page.buttons.iter_mut().zip(actions) {
            b.action = a;
        }
        page
    }

    #[test]
    fn configured_keys_get_their_category_color_and_a_gradient() {
        let mut page = page_with(vec![
            ButtonAction::MediaKey { key: "next".into() },
            ButtonAction::Hotkey {
                keys: vec!["ctrl".into(), "s".into()],
            },
        ]);
        let changed = beautify(&mut page, &library::presets());
        assert_eq!(changed, 2);
        let media = library::category_color(&page.buttons[0].action).unwrap();
        assert_eq!(page.buttons[0].appearance.background.r, media[0]);
        assert!(page.buttons[0].appearance.gradient);
        assert_ne!(
            page.buttons[0].appearance.background,
            page.buttons[1].appearance.background
        );
    }

    #[test]
    fn empty_keys_stay_untouched() {
        let mut page = page_with(vec![]);
        assert_eq!(beautify(&mut page, &library::presets()), 0);
        assert!(page.buttons.iter().all(|b| !b.appearance.gradient));
        assert_eq!(page.buttons[0].appearance.background, BLANK_BACKGROUND);
    }

    #[test]
    fn a_key_without_label_or_icon_gets_both() {
        let mut page = page_with(vec![ButtonAction::MediaKey { key: "next".into() }]);
        beautify(&mut page, &library::presets());
        let ap = &page.buttons[0].appearance;
        assert!(ap.icon.is_some());
        assert!(!ap.text.is_empty());
    }

    #[test]
    fn what_the_user_wrote_is_kept() {
        let mut page = page_with(vec![ButtonAction::MediaKey { key: "next".into() }]);
        page.buttons[0].appearance.text = "Pular".into();
        page.buttons[0].appearance.icon_path = Some("/x.png".into());
        beautify(&mut page, &library::presets());
        let ap = &page.buttons[0].appearance;
        assert_eq!(ap.text, "Pular");
        assert_eq!(ap.icon, None);
    }

    #[test]
    fn live_keys_get_no_label() {
        let mut page = page_with(vec![
            ButtonAction::MediaKey {
                key: "volup".into(),
            },
            ButtonAction::Monitor {
                metric: Metric::Cpu,
            },
        ]);
        beautify(&mut page, &library::presets());
        assert!(page.buttons[0].appearance.text.is_empty());
        assert!(page.buttons[1].appearance.text.is_empty());
        assert!(page.buttons[1].appearance.gradient);
    }

    #[test]
    fn beautifying_twice_changes_nothing_more() {
        let mut page = page_with(vec![ButtonAction::NextPage, ButtonAction::OpenMixer]);
        beautify(&mut page, &library::presets());
        assert_eq!(beautify(&mut page, &library::presets()), 0);
    }
}
