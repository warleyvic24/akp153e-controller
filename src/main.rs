#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod appicon;
mod apps;
mod bars;
mod cli;
mod config;
mod engine;
mod graphics;
mod history;
mod icons;
mod library;
mod panes;
mod picker;
mod runtime;
mod service;
mod shell;
mod stage;
mod textlist;
mod theme;
mod usb;
mod widgets;

use config::schema::*;
use eframe::egui::{self, vec2, Color32, Frame, Margin, RichText, Rounding, Sense, Stroke};
use engine::actions::Executor;
use panes::{action_pane, appearance_pane, states_pane, Targets};
use picker::{apply_app, AppPicker};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use theme::Palette;
use usb::UsbCmd;

const AUTOSAVE_DELAY: Duration = Duration::from_millis(300);
const REPAINT_EVERY: Duration = Duration::from_millis(100);
const INSPECTOR_WIDTH: f32 = 300.0;
const TAB_GAP: f32 = 10.0;
const QUICK_BUTTON_PAD_X: f32 = 9.0;
const FOCUS_GLOW_RADIUS: f32 = 8.0;
const TAB_NAMES: [&str; 3] = ["Aparência", "Ação", "Estados"];

struct App {
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
}

impl App {
    fn new(sh: runtime::SharedState, tx: Sender<UsbCmd>, exec: Executor) -> Self {
        Self {
            sh,
            selected: 0,
            tx,
            exec,
            dirty: false,
            nav_dirty: false,
            baseline: None,
            dirty_since: None,
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
        }
    }

    /// Marca uma edição de conteúdo: grava, avisa o serviço e entra no histórico de desfazer.
    fn touch(&mut self) {
        self.dirty = true;
        self.edited = true;
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
                let _ = shell::send("reload");
            }
            Err(e) => log::error!("salvar: {e}"),
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

    fn app_picker(&mut self, ctx: &egui::Context, g: &mut runtime::Shared) {
        self.picker.poll();
        self.apply_finished_icons(g);
        if !self.picker.open {
            return;
        }
        let mut open = true;
        let mut chosen: Option<apps::InstalledApp> = None;
        egui::Window::new("Apps instalados")
            .open(&mut open)
            .collapsible(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.picker.query)
                        .hint_text("Buscar app")
                        .desired_width(ui.available_width()),
                );
                let Some(list) = &self.picker.apps else {
                    ui.label("Procurando programas instalados…");
                    return;
                };
                let q = self.picker.query.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for app in list
                            .iter()
                            .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
                        {
                            if ui
                                .selectable_label(false, &app.name)
                                .on_hover_text(&app.target)
                                .clicked()
                            {
                                chosen = Some(app.clone());
                            }
                        }
                        if list.is_empty() {
                            ui.label("Nenhum programa encontrado. Use “Procurar .exe”.");
                        }
                    });
            });
        if let Some(app) = chosen {
            let at = self.picker.at;
            if let Some(b) = g.cfg.page_mut().buttons.get_mut(at.2) {
                apply_app(b, &app, at, &self.picker.jobs);
            }
            self.touch();
            open = false;
        }
        self.picker.open = open;
    }

    fn offline_banner(&self, ui: &mut egui::Ui, pal: &Palette) {
        Frame::none()
            .fill(pal.bad.gamma_multiply(0.15))
            .stroke(Stroke::new(1.0_f32, pal.bad.gamma_multiply(0.5)))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Aparelho desconectado.").strong());
                    ui.label("Suas alterações ficam salvas e vão ao aparelho ao reconectar.");
                    if ui.button("Reconectar").clicked() {
                        let _ = shell::send("reconnect");
                    }
                });
            });
        ui.add_space(10.0);
    }

    fn quick_actions(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let sel = self.selected;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().button_padding.x = QUICK_BUTTON_PAD_X;
            let test = egui::Button::new(
                RichText::new("Testar")
                    .strong()
                    .color(theme::readable_on(accent)),
            )
            .fill(accent);
            let test = ui.add(test);
            widgets::glow(ui.painter(), test.rect, 8.0, accent);
            if test.clicked() {
                runtime::trigger(g, &self.tx, &self.exec, sel, Duration::ZERO);
            }
            if ui.button("Copiar").clicked() {
                self.clipboard = g.cfg.page().buttons.get(sel).cloned();
            }
            if ui
                .add_enabled(self.clipboard.is_some(), egui::Button::new("Colar"))
                .clicked()
            {
                if let (Some(c), Some(b)) = (
                    self.clipboard.clone(),
                    g.cfg.page_mut().buttons.get_mut(sel),
                ) {
                    *b = c;
                    self.touch();
                }
            }
            let clear = egui::Button::new(RichText::new("Limpar").color(pal.bad));
            if ui.add(clear).on_hover_text("Esvazia este botão").clicked() {
                if let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) {
                    *b = ButtonConfig::blank();
                    self.touch();
                }
            }
        });
    }

    /// Painel do botão selecionado, com as abas Aparência, Ação e Estados.
    fn inspector(
        &mut self,
        ctx: &egui::Context,
        g: &mut runtime::Shared,
        pal: &Palette,
        accent: Color32,
    ) {
        let frame = Frame::none()
            .fill(pal.raise)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .inner_margin(Margin::same(14.0));
        egui::SidePanel::right("insp")
            .exact_width(INSPECTOR_WIDTH)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                let sel = self.selected.min(g.cfg.page().buttons.len() - 1);
                self.selected = sel;
                let targets = Targets::from(&g.cfg);
                let icon_px = g.icon_px;
                let profile = g.cfg.active_profile;
                self.picker.at = (profile, g.cfg.profiles[profile].active_page, sel);
                self.quick_actions(ui, g, pal, accent);
                ui.add_space(4.0);
                let Some(b) = g.cfg.page_mut().buttons.get_mut(sel) else {
                    return;
                };
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                    let tex = self.key_tex.get(ctx, sel, b, icon_px);
                    widgets::paint_key(
                        ui.painter(),
                        rect,
                        &key_view(b, sel + 1, tex),
                        false,
                        false,
                        pal,
                        accent,
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!("BOTÃO {}", sel + 1))
                                .monospace()
                                .size(11.0)
                                .color(pal.muted),
                        );
                        let name = if b.appearance.text.is_empty() {
                            "Sem nome"
                        } else {
                            b.appearance.text.as_str()
                        };
                        ui.label(RichText::new(name).font(theme::display_font(20.0)));
                    });
                });
                if is_blank(b) {
                    ui.label(
                        RichText::new("Escolha uma ação na bandeja ou comece pelo rótulo.")
                            .size(12.0)
                            .color(pal.muted),
                    );
                }
                ui.horizontal(|ui| {
                    for (i, name) in TAB_NAMES.iter().enumerate() {
                        if widgets::tab(ui, name, self.tab == i, accent, pal).clicked() {
                            self.tab = i;
                        }
                        ui.add_space(TAB_GAP);
                    }
                });
                ui.separator();
                let mut changed = false;
                // Um estado de rolagem por botão e aba: trocar de um para outro volta ao topo.
                let scroll = egui::ScrollArea::vertical().id_salt(("inspector", sel, self.tab));
                scroll.show(ui, |ui| match self.tab {
                    0 => changed |= appearance_pane(ui, b, pal, accent),
                    1 => {
                        changed |= action_pane(
                            ui,
                            ctx,
                            b,
                            &targets,
                            &mut self.recording,
                            &mut self.picker,
                            pal,
                            accent,
                        )
                    }
                    _ => changed |= states_pane(ui, b, pal),
                });
                if changed {
                    self.touch();
                }
            });
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

fn focus_glow(ctx: &egui::Context, accent: Color32) {
    let Some(rect) = ctx
        .memory(|m| m.focused())
        .and_then(|id| ctx.read_response(id))
        .map(|r| r.rect)
    else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("focus_glow"),
    ));
    widgets::glow(&painter, rect, FOCUS_GLOW_RADIUS, accent);
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

fn key_view(b: &ButtonConfig, number: usize, tex: Option<egui::TextureId>) -> widgets::KeyView<'_> {
    let ap = b.current_appearance();
    widgets::KeyView {
        number,
        bg: c32(ap.background),
        icon: ap.icon.as_deref(),
        label: &ap.text,
        text: c32(ap.text_color),
        off: is_blank(b),
        tex,
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
            self.applied_ui = Some(g.cfg.ui.clone());
        }
        let (pal, accent) = (
            theme::palette(g.cfg.ui.theme),
            theme::accent_color(&g.cfg.ui),
        );
        // Estado do quadro anterior: só se copia a config quando ela muda (comparar não aloca).
        let before = self.baseline.take().unwrap_or_else(|| g.cfg.clone());
        let (mut want_undo, mut want_redo) = self.read_undo_keys(ctx);

        self.top_bar(ctx, g, &pal, accent);
        self.footer(ctx, &pal, &mut want_undo, &mut want_redo);
        self.tray(ctx, g, &pal, accent);
        self.inspector(ctx, g, &pal, accent);
        self.stage(ctx, g, &pal, accent);
        focus_glow(ctx, accent);
        self.app_picker(ctx, g);

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
        self.edited = false;
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--probe") {
        usb::probe();
        return Ok(());
    }
    cli::init_logging();
    let value_of = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    if has("--autostart") {
        shell::set_autostart(value_of("--autostart").is_some_and(|v| v == "on"));
        return Ok(());
    }
    if has("--run-action") {
        cli::run_action_cli(value_of("--run-action"));
        return Ok(());
    }
    if has("--editor") {
        return cli::run_editor();
    }
    service::run(!has("--background"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picker::IconDone;
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
            .position(|p| p.opens_app_picker())
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
}
