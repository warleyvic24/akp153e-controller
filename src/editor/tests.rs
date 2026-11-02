//! Testes do `App` do editor: salvar, desfazer, ícones de programas e acessibilidade.
use super::*;
use crate::platform::apps;
#[cfg(windows)]
use picker::apply_app;
use picker::IconDone;
use std::sync::mpsc::channel;

const NOTEPAD: &str = r"C:\Windows\System32\notepad.exe";
#[cfg(windows)]
// O PowerShell que extrai o ícone demora a abrir no CI: com 20 s o teste já estourou o prazo.
const ICON_WAIT: Duration = Duration::from_secs(60);

fn test_app() -> (App, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("tokio");
    let exec = Executor::new(rt.handle().clone());
    let sh = runtime::new_state(AppConfig::default(), usb::ICON_SIZE as u32);
    let (tx, _rx) = channel::<UsbCmd>();
    (App::new(sh, tx, exec), rt)
}

/// O .exe do Bloco de Notas só existe no Windows.
#[cfg(windows)]
#[test]
fn choosing_an_app_sets_action_and_label_without_waiting_for_the_icon() {
    let (app, _rt) = test_app();
    let sh = app.sh.clone();
    let mut guard = runtime::lock(&sh);
    let g: &mut runtime::Shared = &mut guard;
    let at = (
        g.cfg.active_profile,
        g.cfg.profiles[g.cfg.active_profile].active_page,
        0,
    );
    let notepad = apps::InstalledApp::exe("Bloco de Notas", NOTEPAD);

    apply_app(
        &mut g.cfg.page_mut().buttons[0],
        &notepad,
        at,
        &app.picker.jobs,
    );
    let b = &g.cfg.page().buttons[0];
    assert_eq!(
        b.action,
        ButtonAction::LaunchApp {
            path: NOTEPAD.into(),
            args: vec![]
        }
    );
    assert_eq!(b.appearance.text, "Bloco de Notas");
    assert_eq!(b.appearance.background, BLANK_BACKGROUND);
}

/// Um quadro do egui sem janela (as texturas que ele pediria para enviar são descartadas).
fn frame(ctx: &egui::Context, run: impl FnMut(&egui::Context)) {
    ctx.run_pass(egui::RawInput::default(), run)
        .textures_delta
        .clear();
}

/// Extrai o ícone de verdade com o PowerShell. Fora do CI: no runner do GitHub a extração às vezes
/// não termina (2026-10-31), e isso não é defeito do programa. `cargo test real_icon -- --ignored`.
#[cfg(windows)]
#[test]
#[ignore]
fn real_icon_extraction_draws_the_icon_on_the_key() {
    let (mut app, _rt) = test_app();
    let sh = app.sh.clone();
    let mut guard = runtime::lock(&sh);
    let g: &mut runtime::Shared = &mut guard;
    let at = (
        g.cfg.active_profile,
        g.cfg.profiles[g.cfg.active_profile].active_page,
        0,
    );
    let notepad = apps::InstalledApp::exe("Bloco de Notas", NOTEPAD);
    apply_app(
        &mut g.cfg.page_mut().buttons[0],
        &notepad,
        at,
        &app.picker.jobs,
    );
    let start = Instant::now();
    while g.cfg.page().buttons[0].appearance.icon_path.is_none() && start.elapsed() < ICON_WAIT {
        std::thread::sleep(Duration::from_millis(100));
        app.apply_finished_icons(g);
    }
    let icon = g.cfg.page().buttons[0]
        .appearance
        .icon_path
        .clone()
        .expect("o ícone deveria ter sido extraído");
    assert!(std::path::Path::new(&icon).is_file());
    let rendered = crate::graphics::renderer::render_rgba(&g.cfg.page().buttons[0], 95, 95);
    let first = *rendered.get_pixel(0, 0);
    assert!(
        rendered.pixels().any(|p| *p != first),
        "o render deveria conter o ícone"
    );
    let _ = std::fs::remove_file(icon);
}

#[test]
fn dropping_the_app_preset_opens_the_picker_for_that_button() {
    let (mut app, _rt) = test_app();
    let sh = app.sh.clone();
    let mut guard = runtime::lock(&sh);
    let g: &mut runtime::Shared = &mut guard;
    let preset = app
        .presets
        .iter()
        .position(super::library::Preset::opens_app_picker)
        .expect("deve existir o preset Aplicativo...");
    app.apply_preset(g, 3, preset);
    assert!(app.picker.open, "o seletor deveria abrir sozinho");
    assert_eq!(app.picker.at, (0, 0, 3));
    let b = &g.cfg.page().buttons[3];
    assert!(b.appearance.text.is_empty());
    assert_eq!(b.appearance.background, BLANK_BACKGROUND);
    assert!(matches!(&b.action, ButtonAction::LaunchApp { path, .. } if path.is_empty()));
}

#[test]
fn other_presets_do_not_open_the_picker() {
    let (mut app, _rt) = test_app();
    let sh = app.sh.clone();
    let mut guard = runtime::lock(&sh);
    let g: &mut runtime::Shared = &mut guard;
    let preset = app
        .presets
        .iter()
        .position(|p| !p.opens_app_picker())
        .unwrap();
    app.apply_preset(g, 0, preset);
    assert!(!app.picker.open);
}

#[test]
fn a_late_icon_is_dropped_when_the_button_changed() {
    let (mut app, _rt) = test_app();
    let sh = app.sh.clone();
    let mut guard = runtime::lock(&sh);
    let g: &mut runtime::Shared = &mut guard;
    let at = (
        g.cfg.active_profile,
        g.cfg.profiles[g.cfg.active_profile].active_page,
        0,
    );
    let stale = ButtonAction::LaunchApp {
        path: "outro.exe".into(),
        args: vec![],
    };
    app.picker
        .jobs
        .tx
        .send(IconDone {
            at,
            action: stale,
            icon: "x.png".into(),
        })
        .unwrap();
    app.apply_finished_icons(g);
    assert!(g.cfg.page().buttons[0].appearance.icon_path.is_none());
}

#[test]
fn picker_window_opens_and_lists_the_scanned_apps() {
    let (mut app, _rt) = test_app();
    let sh = app.sh.clone();
    let mut guard = runtime::lock(&sh);
    let g: &mut runtime::Shared = &mut guard;
    app.picker.open = true;
    app.picker.apps = Some(vec![apps::InstalledApp::exe("Bloco de Notas", NOTEPAD)]);
    let ctx = egui::Context::default();
    frame(&ctx, |ctx| app.app_picker(ctx, g));
    frame(&ctx, |ctx| app.app_picker(ctx, g));
    let shown = ctx.memory(|m| m.area_rect(egui::Id::unique(inspector::APP_PICKER_ID)));
    assert!(
        shown.is_some_and(|r| r.width() > 100.0),
        "a janela da lista deveria estar visível"
    );
    assert!(app.picker.open);
}

/// Todo controle interativo precisa de nome para o leitor de tela. Roda o editor sem janela,
/// com as três abas, os tipos de ação e as Preferências, e lista o que ficou sem nome.
#[test]
fn every_interactive_control_has_an_accessible_name() {
    use egui::accesskit::Role;
    let (mut app, _rt) = test_app();
    let sh = app.sh.clone();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    theme::install_fonts(&ctx);
    theme::apply(&ctx, &runtime::lock(&sh).cfg.ui);
    // O primeiro quadro só aplica as fontes ao egui.
    frame(&ctx, |_| {});
    let actions = [
        ButtonAction::None,
        ButtonAction::LaunchApp {
            path: String::new(),
            args: vec![],
        },
        ButtonAction::HttpWebhook {
            url: String::new(),
            method: "GET".into(),
            body: None,
        },
        ButtonAction::Hotkey { keys: vec![] },
        ButtonAction::MediaKey { key: "mute".into() },
        ButtonAction::SwitchPage { page: 0 },
    ];
    let checked = [
        Role::Button,
        Role::CheckBox,
        Role::ComboBox,
        Role::Slider,
        Role::TextInput,
        Role::MultilineTextInput,
        Role::Tab,
        Role::RadioButton,
        Role::MenuItem,
        Role::ColorWell,
    ];
    let mut unnamed: Vec<String> = Vec::new();
    for tab in 0..2 {
        for action in &actions {
            if tab != inspector::TAB_ACTION && *action != ButtonAction::None {
                continue;
            }
            app.tab = tab;
            app.settings_open = true;
            let mut guard = runtime::lock(&sh);
            let g: &mut runtime::Shared = &mut guard;
            g.cfg.page_mut().buttons[0].action = action.clone();
            let (pal, accent) = (
                theme::palette(g.cfg.ui.theme),
                theme::accent_color(&g.cfg.ui),
            );
            let (mut undo, mut redo) = (false, false);
            // Barra do modo compacto (com o botão "Ações"), mas os dois painéis desenhados.
            let compact = layout::panels(layout::INSPECTOR_WIDTH, false);
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.top_bar(ui, g, (&pal, accent), (&mut undo, &mut redo), compact);
                app.library(ui, g, &pal, accent);
                app.inspector(ui, g, &pal, accent, layout::INSPECTOR_WIDTH);
                app.stage(ui, g, &pal, accent);
                let ctx = ui.ctx().clone();
                app.settings_window(&ctx, g, &pal);
            });
            // Sem janela, as texturas novas não vão a lugar nenhum: o egui exige descartá-las.
            out.textures_delta.clear();
            let update = out.platform_output.accesskit_update.expect("accesskit");
            for (_, node) in update.nodes {
                // No egui 0.37 o texto de um rótulo vai em `value`: ele se nomeia sozinho.
                let named = [node.label(), node.value()]
                    .into_iter()
                    .flatten()
                    .any(|n| !n.trim().is_empty())
                    || !node.labelled_by().is_empty();
                // O egui 0.37 marca as áreas dos painéis (papel `Unknown`) como clicáveis;
                // não são controles nossos.
                // A barra de rolagem também é do egui (sem nome a dar): o leitor de tela rola
                // pela própria área, não por ela.
                let clickable = node.supports_action(egui::accesskit::Action::Click)
                    && node.role() != egui::accesskit::Role::Unknown
                    && node.role() != egui::accesskit::Role::ScrollBar;
                if (checked.contains(&node.role()) || clickable) && !named {
                    unnamed.push(format!("aba {tab}, {action:?}: {:?}", node.role()));
                }
            }
        }
    }
    unnamed.sort();
    unnamed.dedup();
    assert!(
        unnamed.is_empty(),
        "controles sem nome:\n{}",
        unnamed.join("\n")
    );
}

/// Leitor de tela: as regiões da janela têm papel e nome (pular direto ao palco, à biblioteca,
/// ao inspetor) e as Preferências são um diálogo modal com título.
#[test]
fn regions_are_landmarks_and_preferences_is_a_named_modal_dialog() {
    use egui::accesskit::Role;
    let (mut app, _rt) = test_app();
    let sh = app.sh.clone();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    theme::install_fonts(&ctx);
    theme::apply(&ctx, &runtime::lock(&sh).cfg.ui);
    frame(&ctx, |_| {});
    app.settings_open = true;
    let mut guard = runtime::lock(&sh);
    let g: &mut runtime::Shared = &mut guard;
    let (pal, accent) = (
        theme::palette(g.cfg.ui.theme),
        theme::accent_color(&g.cfg.ui),
    );
    let (mut undo, mut redo) = (false, false);
    let wide = layout::panels(layout::INSPECTOR_WIDTH * 5.0, false);
    // Dois quadros: no primeiro o egui só mede o modal (invisível, sem nós de acessibilidade).
    let mut run = || {
        ctx.run_ui(egui::RawInput::default(), |ui| {
            app.top_bar(ui, g, (&pal, accent), (&mut undo, &mut redo), wide);
            app.library(ui, g, &pal, accent);
            app.inspector(ui, g, &pal, accent, layout::INSPECTOR_WIDTH);
            app.stage(ui, g, &pal, accent);
            let ctx = ui.ctx().clone();
            app.settings_window(&ctx, g, &pal);
        })
    };
    run().textures_delta.clear();
    let mut out = run();
    out.textures_delta.clear();
    let update = out.platform_output.accesskit_update.expect("accesskit");
    let has = |role: Role, name: &str| {
        update
            .nodes
            .iter()
            .any(|(_, n)| n.role() == role && n.label().is_some_and(|l| l == strings::t(name)))
    };
    for (role, name) in [
        (Role::Banner, "Keyforge"),
        (Role::Navigation, "Biblioteca de ações"),
        (Role::Complementary, "Tecla selecionada"),
        (Role::Main, "Aparelho"),
        (Role::Dialog, "Preferências"),
    ] {
        assert!(has(role, name), "falta {role:?} \"{name}\"");
    }
    let modal = update
        .nodes
        .iter()
        .any(|(_, n)| n.role() == Role::Dialog && n.is_modal());
    assert!(modal, "as Preferências devem ser um diálogo modal");
    let headings = update
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == Role::Heading)
        .count();
    assert!(
        headings >= 3,
        "títulos de seção como cabeçalhos: {headings}"
    );
}

/// O egui só cria o nó de acessibilidade de um controle que declara nome; um controle clicável
/// desenhado à mão sem `widget_info` nem `widgets::name` some da árvore e o teste de nomes
/// acima não o enxerga. Por isso este teste olha o código: cada `Sense::click` precisa ser
/// seguido, nas linhas seguintes, de uma declaração de nome.
#[test]
fn every_hand_drawn_clickable_declares_an_accessible_name() {
    const LOOKAHEAD: usize = 14;
    let mut missing = Vec::new();
    for (file, src) in crate::test_support::sources_without_tests() {
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let draws = line.contains("Sense::click") && !line.trim_start().starts_with("//");
            if !draws {
                continue;
            }
            let window = &lines[i..(i + LOOKAHEAD).min(lines.len())];
            let named = window
                .iter()
                .any(|l| l.contains("widget_info") || l.contains("widgets::name("));
            if !named {
                missing.push(format!("{file}:{}", i + 1));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "clicável sem nome acessível:\n{}",
        missing.join("\n")
    );
}
