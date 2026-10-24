//! Avisos curtos no rodapé ("Teclas trocadas · Desfazer"): dão retorno a ações que, sozinhas,
//! não mudam nada visível na tela (copiar, colar, limpar, excluir).
use crate::editor::theme::Palette;
use crate::editor::tokens::{radius, space};
use eframe::egui::{self, vec2, Align2, Frame, Margin, Stroke};
use std::time::{Duration, Instant};

const LIFETIME: Duration = Duration::from_secs(4);
const MAX_VISIBLE: usize = 3;
/// Distância da borda inferior: fica acima da bandeja de ações e do rodapé.
const BOTTOM_OFFSET: f32 = 210.0;

struct Toast {
    text: String,
    action: Option<&'static str>,
    until: Instant,
}

#[derive(Default)]
pub struct Toasts(Vec<Toast>);

impl Toasts {
    /// Mostra `text`; com `action`, o aviso ganha um botão (hoje só "Desfazer").
    /// Repetir o mesmo texto renova o aviso em vez de empilhar.
    pub fn push(&mut self, text: impl Into<String>, action: Option<&'static str>) {
        let text = text.into();
        let text = crate::strings::t(&text).to_string();
        self.0.retain(|t| t.text != text);
        self.0.push(Toast {
            text,
            action,
            until: Instant::now() + LIFETIME,
        });
        let extra = self.0.len().saturating_sub(MAX_VISIBLE);
        self.0.drain(..extra);
    }

    /// Tira os avisos que oferecem "Desfazer": a edição a que se referiam já não é a última.
    pub fn forget_undo(&mut self) {
        self.0.retain(|t| t.action.is_none());
    }

    fn prune(&mut self, now: Instant) {
        self.0.retain(|t| t.until > now);
    }

    /// Desenha os avisos vivos. Devolve `true` se o usuário clicou na ação de algum.
    pub fn show(&mut self, ctx: &egui::Context, pal: &Palette) -> bool {
        self.prune(Instant::now());
        let mut clicked: Option<usize> = None;
        egui::Area::new("toasts".into())
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -BOTTOM_OFFSET))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (i, t) in self.0.iter().enumerate() {
                    Frame::none()
                        .fill(pal.raise)
                        .stroke(Stroke::new(1.0_f32, pal.control))
                        .rounding(egui::Rounding::same(radius::CARD))
                        .shadow(ui.visuals().popup_shadow)
                        .inner_margin(Margin::symmetric(space::MD, space::SM))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(&t.text);
                                if let Some(a) = t.action {
                                    if ui.button(crate::strings::t(a)).clicked() {
                                        clicked = Some(i);
                                    }
                                }
                            });
                        });
                }
            });
        match clicked {
            Some(i) => {
                self.0.remove(i);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeating_a_message_renews_it_instead_of_stacking() {
        let mut t = Toasts::default();
        t.push("Tecla 1 copiada", None);
        t.push("Tecla 1 copiada", None);
        assert_eq!(t.0.len(), 1);
    }

    #[test]
    fn only_the_newest_messages_stay_visible() {
        let mut t = Toasts::default();
        for n in 0..5 {
            t.push(format!("aviso {n}"), None);
        }
        assert_eq!(t.0.len(), MAX_VISIBLE);
        assert_eq!(t.0.last().map(|x| x.text.as_str()), Some("aviso 4"));
    }

    #[test]
    fn a_new_edit_withdraws_older_undo_offers_but_keeps_plain_messages() {
        let mut t = Toasts::default();
        t.push("Tecla 1 copiada", None);
        t.push("Teclas trocadas", Some("Desfazer"));
        t.forget_undo();
        assert_eq!(t.0.len(), 1);
        assert!(t.0[0].action.is_none());
    }

    #[test]
    fn messages_expire() {
        let mut t = Toasts::default();
        t.push("x", Some("Desfazer"));
        t.prune(Instant::now() + LIFETIME + Duration::from_secs(1));
        assert!(t.0.is_empty());
    }
}
