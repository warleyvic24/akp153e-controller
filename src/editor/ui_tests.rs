//! Testes da interface clicando de verdade (egui_kittest, sem janela): o que antes só se conferia
//! à mão no editor aberto.
use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

struct Under {
    app: App,
    _rt: tokio::runtime::Runtime,
}

fn under_test() -> Under {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("tokio");
    let exec = Executor::new(rt.handle().clone());
    let sh = runtime::new_state(AppConfig::default(), usb::ICON_SIZE as u32);
    let (tx, _rx) = std::sync::mpsc::channel::<UsbCmd>();
    Under {
        app: App::new(sh, tx, exec),
        _rt: rt,
    }
}

/// O editor inteiro numa janela larga (biblioteca, palco e inspetor visíveis).
fn harness(under: Under) -> Harness<'static, Under> {
    let mut fonts_ready = false;
    Harness::builder()
        .with_size(egui::vec2(1600.0, 1000.0))
        .build_ui_state(
            move |ui, u: &mut Under| {
                // As fontes do editor valem a partir do quadro seguinte: o primeiro só as instala.
                if !fonts_ready {
                    theme::install_fonts(ui.ctx());
                    theme::apply(ui.ctx(), &runtime::lock(&u.app.sh).cfg.ui);
                    fonts_ready = true;
                    return;
                }
                let sh = u.app.sh.clone();
                let mut guard = runtime::lock(&sh);
                let g: &mut runtime::Shared = &mut guard;
                let (pal, accent) = (
                    theme::palette(g.cfg.ui.theme),
                    theme::accent_color(&g.cfg.ui),
                );
                let panels = layout::panels(1600.0, true);
                let (mut undo, mut redo) = (false, false);
                u.app
                    .top_bar(ui, g, (&pal, accent), (&mut undo, &mut redo), panels);
                u.app.library(ui, g, &pal, accent);
                u.app.inspector(ui, g, &pal, accent, panels.inspector_width);
                u.app.stage(ui, g, &pal, accent);
            },
            under,
        )
}

fn first_key(h: &Harness<'_, Under>) -> ButtonAction {
    runtime::lock(&h.state().app.sh).cfg.page().buttons[0]
        .action
        .clone()
}

#[test]
fn the_kind_row_opens_the_grid_and_a_new_type_replaces_the_action() {
    let mut u = under_test();
    runtime::lock(&u.app.sh).cfg.page_mut().buttons[0].action = ButtonAction::Hotkey {
        keys: vec!["ctrl".into(), "m".into()],
    };
    u.app.selected = 0;
    let mut h = harness(u);
    h.run();
    h.get_by_label_contains("Tipo de ação: Atalho").click();
    h.run();
    // O balão traz a grade completa, com "Nenhuma" para limpar.
    h.get_by_label("Nenhuma");
    h.get_by_label("Som").click();
    h.run();
    assert_eq!(
        first_key(&h),
        ButtonAction::PlaySound {
            path: String::new()
        }
    );
    // Escolher fecha o balão.
    assert!(h.query_by_label("Nenhuma").is_none());
}

#[test]
fn an_empty_key_shows_the_open_grid_without_none() {
    let mut u = under_test();
    u.app.selected = 0;
    let mut h = harness(u);
    h.run();
    assert!(h.query_by_label("Nenhuma").is_none());
    // "Relógio" também está na biblioteca, à esquerda: o da grade é o do inspetor, à direita.
    h.get_all_by_label("Relógio")
        .max_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("Relógio na grade")
        .click();
    h.run();
    assert_eq!(first_key(&h), ButtonAction::Clock);
}

#[test]
fn the_more_menu_clears_the_key_and_the_two_folds_start_closed() {
    let mut u = under_test();
    runtime::lock(&u.app.sh).cfg.page_mut().buttons[0].action = ButtonAction::Clock;
    u.app.selected = 0;
    let mut h = harness(u);
    h.run();
    // Sem ação ao segurar nem dois estados: as seções começam fechadas, dizendo "desligado".
    h.get_by_label_contains("Ao segurar · desligado");
    h.get_by_label_contains("Dois estados · desligado");
    h.get_by_label("Mais ações da tecla").click();
    h.run();
    h.get_by_label_contains("Limpar tecla").click();
    h.run();
    assert_eq!(first_key(&h), ButtonAction::None);
}
