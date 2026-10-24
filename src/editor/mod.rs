//! Editor: a janela egui que edita a config e avisa o serviço. Não fala com o aparelho.
mod action_ui;
mod bars;
mod checks;
mod history;
mod inspector;
mod library;
mod panes;
mod picker;
mod settings;
mod stage;
mod summary;
mod textlist;
pub(crate) mod theme;
mod toast;
mod tokens;
mod widgets;

use crate::config::schema::*;
use crate::engine::actions::Executor;
use crate::platform::shell;
use crate::service::runtime;
use crate::{graphics, strings, usb};
use eframe::egui::{self, Color32};
use picker::AppPicker;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use usb::UsbCmd;

const AUTOSAVE_DELAY: Duration = Duration::from_millis(300);
const SAVE_RETRY_DELAY: Duration = Duration::from_secs(3);
const REPAINT_EVERY: Duration = Duration::from_millis(100);
const TAB_GAP: f32 = 10.0;

pub(crate) struct App {
    sh: runtime::SharedState,
    selected: usize,
    tx: Sender<UsbCmd>,
    exec: Executor,
    dirty: bool,
    /// O usuário mudou perfil/página neste editor desde o último salvamento; senão o arquivo manda.
    nav_dirty: bool,
    /// Config ao fim do quadro anterior, base do histórico de desfazer.
    baseline: Option<AppConfig>,
    dirty_since: Option<Instant>,
    /// Último erro ao gravar a config; some quando uma gravação der certo.
    save_error: Option<String>,
    presets: Vec<library::Preset>,
    history: history::History<AppConfig>,
    edited: bool,
    applied_ui: Option<UiSettings>,
    tab: usize,
    tray_cat: usize,
    query: String,
    clipboard: Option<ButtonConfig>,
    recording: Option<&'static str>,
    key_tex: KeyTextures,
    picker: AppPicker,
    toasts: toast::Toasts,
    settings_open: bool,
    autostart: bool,
    rename: Option<bars::Rename>,
}

impl App {
    pub(crate) fn new(sh: runtime::SharedState, tx: Sender<UsbCmd>, exec: Executor) -> Self {
        Self {
            sh,
            selected: 0,
            tx,
            exec,
            dirty: false,
            nav_dirty: false,
            baseline: None,
            dirty_since: None,
            save_error: None,
            presets: library::presets(),
            history: history::History::new(),
            edited: false,
            applied_ui: None,
            tab: 0,
            tray_cat: 0,
            query: String::new(),
            clipboard: None,
            recording: None,
            key_tex: KeyTextures::default(),
            picker: AppPicker::default(),
            toasts: toast::Toasts::default(),
            settings_open: false,
            autostart: false,
            rename: None,
        }
    }

    /// Marca uma edição de conteúdo: grava, avisa o serviço e entra no histórico de desfazer.
    fn touch(&mut self) {
        self.dirty = true;
        self.edited = true;
        // O "Desfazer" de um aviso desfaz a última edição; depois de outra, ele enganaria.
        self.toasts.forget_undo();
    }

    /// Aplica um preset da biblioteca (ação, ícone, rótulo e cor) ao botão indicado.
    fn apply_preset(&mut self, g: &mut runtime::Shared, button: usize, preset: usize) {
        let Some(p) = self.presets.get(preset).cloned() else {
            return;
        };
        let action = p.resolved_action(&g.cfg);
        let slot = (
            g.cfg.active_profile,
            g.cfg.profiles[g.cfg.active_profile].active_page,
            button,
        );
        let Some(b) = g.cfg.page_mut().buttons.get_mut(button) else {
            return;
        };
        let replaced = !is_blank(b);
        b.action = action;
        if p.opens_app_picker() {
            // O programa escolhido define rótulo, fundo e ícone (ver `picker::apply_app`).
            b.appearance.text.clear();
            b.appearance.icon = Some(p.icon.to_string());
            b.appearance.icon_path = None;
            b.appearance.background = BLANK_BACKGROUND;
            self.picker.at = slot;
            self.picker.show();
            self.touch();
            self.warn_replaced(replaced, button);
            return;
        }
        b.appearance.text = p.label.to_string();
        b.appearance.icon = Some(p.icon.to_string());
        b.appearance.icon_path = None;
        b.appearance.background = RGBAColor {
            r: p.color[0],
            g: p.color[1],
            b: p.color[2],
            a: 255,
        };
        self.touch();
        self.warn_replaced(replaced, button);
    }

    /// Uma ação da bandeja caiu numa tecla já configurada: avisa e oferece desfazer.
    fn warn_replaced(&mut self, replaced: bool, button: usize) {
        if replaced {
            let msg = strings::tf("Tecla {} substituída", &[&(button + 1)]);
            self.toasts.push(msg, Some("Desfazer"));
        }
    }

    /// Como `touch`, para edições que mudam o perfil/página ativos.
    fn touch_nav(&mut self) {
        self.nav_dirty = true;
        self.touch();
    }

    /// Grava e avisa o serviço para recarregar e redesenhar o aparelho. Sem navegação feita aqui,
    /// mantém o perfil/página que o arquivo já tem: o aparelho pode ter trocado de página nesse meio-tempo.
    fn save(&mut self, cfg: &AppConfig) {
        let mut out = cfg.clone();
        if !self.nav_dirty {
            if let Some(disk) = AppConfig::load_existing() {
                out.adopt_active_from(&disk);
            }
        }
        match out.save() {
            Ok(()) => {
                self.dirty = false;
                self.nav_dirty = false;
                self.dirty_since = None;
                self.save_error = None;
                let _ = shell::send("reload");
            }
            Err(e) => {
                log::error!("salvar: {e}");
                self.save_error = Some(e.to_string());
                // Instante no futuro: `autosave` só tenta de novo depois da espera, em vez de a cada quadro.
                self.dirty_since = Some(Instant::now() + SAVE_RETRY_DELAY);
            }
        }
    }

    /// Grava logo após a última alteração (debounce).
    fn autosave(&mut self, cfg: &AppConfig) {
        if !self.dirty {
            return;
        }
        let since = *self.dirty_since.get_or_insert_with(Instant::now);
        if since.elapsed() >= AUTOSAVE_DELAY {
            self.save(cfg);
        }
    }

    /// Desfaz/refaz a pedido, ou registra no histórico a edição feita neste quadro.
    fn track_history(
        &mut self,
        g: &mut runtime::Shared,
        before: AppConfig,
        want_undo: bool,
        want_redo: bool,
    ) {
        if want_undo || want_redo {
            let current = g.cfg.clone();
            let restored = if want_undo {
                self.history.undo(current)
            } else {
                self.history.redo(current)
            };
            if let Some(cfg) = restored {
                let ui_settings = g.cfg.ui.clone(); // tema não entra no desfazer
                g.cfg = cfg.sanitized();
                g.cfg.ui = ui_settings;
                self.selected = self.selected.min(g.cfg.page().buttons.len() - 1);
                self.nav_dirty = true;
                self.dirty = true;
            }
            self.baseline = Some(g.cfg.clone());
        } else if g.cfg != before {
            if self.edited {
                self.history.record(before, Instant::now());
            }
            self.baseline = Some(g.cfg.clone());
        } else {
            self.baseline = Some(before);
        }
    }

    fn read_undo_keys(&self, ctx: &egui::Context) -> (bool, bool) {
        let (mut undo, mut redo) = (false, false);
        if self.recording.is_none() && !ctx.wants_keyboard_input() {
            ctx.input_mut(|i| {
                let ctrl = egui::Modifiers::COMMAND;
                if i.consume_key(ctrl | egui::Modifiers::SHIFT, egui::Key::Z)
                    || i.consume_key(ctrl, egui::Key::Y)
                {
                    redo = true;
                } else if i.consume_key(ctrl, egui::Key::Z) {
                    undo = true;
                }
            });
        }
        (undo, redo)
    }

    fn apply_finished_icons(&mut self, g: &mut runtime::Shared) {
        while let Ok(done) = self.picker.jobs.rx.try_recv() {
            let (profile, page, key) = done.at;
            let button = g
                .cfg
                .profiles
                .get_mut(profile)
                .and_then(|p| p.pages.get_mut(page))
                .and_then(|pg| pg.buttons.get_mut(key));
            let Some(b) = button.filter(|b| b.action == done.action) else {
                continue;
            };
            b.appearance.icon_path = Some(done.icon);
            b.appearance.icon = None;
            self.touch();
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if !self.dirty {
            return;
        }
        let cfg = runtime::lock(&self.sh).cfg.clone();
        self.save(&cfg);
    }
}

fn c32(c: RGBAColor) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn is_blank(b: &ButtonConfig) -> bool {
    let ap = &b.appearance;
    ap.text.is_empty()
        && ap.icon.is_none()
        && ap.icon_path.is_none()
        && b.action == ButtonAction::None
        && ap.background == BLANK_BACKGROUND
}

fn key_view(
    b: &ButtonConfig,
    number: usize,
    tex: Option<egui::TextureId>,
    problem: bool,
) -> widgets::KeyView<'_> {
    let ap = b.current_appearance();
    widgets::KeyView {
        number,
        bg: c32(ap.background),
        icon: ap.icon.as_deref(),
        label: &ap.text,
        text: c32(ap.text_color),
        off: is_blank(b),
        tex,
        problem,
    }
}

/// Texturas das teclas: a imagem exata que vai ao display, refeita só quando o botão muda.
#[derive(Default)]
struct KeyTextures(Vec<Option<(ButtonConfig, u32, egui::TextureHandle)>>);

impl KeyTextures {
    fn get(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        b: &ButtonConfig,
        px: u32,
    ) -> Option<egui::TextureId> {
        if self.0.len() <= index {
            self.0.resize_with(index + 1, || None);
        }
        if is_blank(b) {
            return None;
        }
        let slot = &mut self.0[index];
        if slot
            .as_ref()
            .is_none_or(|(cached, cached_px, _)| cached != b || *cached_px != px)
        {
            let rgba = graphics::renderer::render_rgba(b, px, px);
            let image = egui::ColorImage::from_rgba_unmultiplied([px as usize; 2], rgba.as_raw());
            *slot = Some((
                b.clone(),
                px,
                ctx.load_texture(format!("key{index}"), image, egui::TextureOptions::LINEAR),
            ));
        }
        slot.as_ref().map(|(_, _, t)| t.id())
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let sh = self.sh.clone();
        let mut guard = runtime::lock(&sh);
        let g: &mut runtime::Shared = &mut guard;
        self.autosave(&g.cfg);
        ctx.request_repaint_after(REPAINT_EVERY);
        if self.applied_ui.as_ref() != Some(&g.cfg.ui) {
            theme::apply(ctx, &g.cfg.ui);
            strings::set_language(g.cfg.ui.language);
            self.applied_ui = Some(g.cfg.ui.clone());
        }
        let (pal, accent) = (
            theme::palette(g.cfg.ui.theme),
            theme::accent_color(&g.cfg.ui),
        );
        // Estado do quadro anterior: só se copia a config quando ela muda (comparar não aloca).
        let before = self.baseline.take().unwrap_or_else(|| g.cfg.clone());
        let (mut want_undo, mut want_redo) = self.read_undo_keys(ctx);
        self.key_shortcuts(ctx, g);

        self.top_bar(ctx, g, (&pal, accent), &mut want_undo, &mut want_redo);
        self.tray(ctx, g, &pal, accent);
        self.inspector(ctx, g, &pal, accent);
        self.stage(ctx, g, &pal, accent);
        widgets::focus_ring(ctx);
        self.app_picker(ctx, g);
        self.settings_window(ctx, g, &pal);
        want_undo |= self.toasts.show(ctx, &pal);

        self.track_history(g, before, want_undo, want_redo);
        self.edited = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::apps;
    use picker::{apply_app, IconDone};
    use std::sync::mpsc::channel;

    const NOTEPAD: &str = r"C:\Windows\System32\notepad.exe";
    const ICON_WAIT: Duration = Duration::from_secs(20);

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

    #[test]
    fn choosing_an_app_sets_action_and_extracted_icon_without_a_label() {
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

        let start = Instant::now();
        while g.cfg.page().buttons[0].appearance.icon_path.is_none() && start.elapsed() < ICON_WAIT
        {
            std::thread::sleep(Duration::from_millis(100));
            app.apply_finished_icons(g);
        }
        let icon = g.cfg.page().buttons[0]
            .appearance
            .icon_path
            .clone()
            .expect("o ícone deveria ter sido extraído");
        assert!(std::path::Path::new(&icon).is_file());
        let rendered = graphics::renderer::render_rgba(&g.cfg.page().buttons[0], 95, 95);
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
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.app_picker(ctx, g));
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.app_picker(ctx, g));
        let shown = ctx.memory(|m| m.area_rect(egui::Id::new("Apps instalados")));
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
        let _ = ctx.run(egui::RawInput::default(), |_| {});
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
        ];
        let mut unnamed: Vec<String> = Vec::new();
        for tab in 0..3 {
            for action in &actions {
                if tab != 1 && *action != ButtonAction::None {
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
                let out = ctx.run(egui::RawInput::default(), |ctx| {
                    app.top_bar(ctx, g, (&pal, accent), &mut undo, &mut redo);
                    app.tray(ctx, g, &pal, accent);
                    app.inspector(ctx, g, &pal, accent);
                    app.stage(ctx, g, &pal, accent);
                    app.settings_window(ctx, g, &pal);
                });
                let update = out.platform_output.accesskit_update.expect("accesskit");
                for (_, node) in update.nodes {
                    let named = node.name().is_some_and(|n| !n.trim().is_empty())
                        || !node.labelled_by().is_empty();
                    let clickable = node.supports_action(egui::accesskit::Action::Default);
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
}
