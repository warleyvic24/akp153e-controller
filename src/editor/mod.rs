//! Editor: a janela egui que edita a config e avisa o serviço. Não fala com o aparelho.
mod action_kind;
mod action_ui;
mod app_icons;
mod bars;
mod beautify;
mod checks;
mod device_view;
mod favicon;
mod history;
mod icon_picker;
mod inspector;
mod key_tex;
mod kind_picker;
mod layout;
mod library;
mod library_panel;
mod memory;
mod monitor_ui;
mod obs_ui;
pub(crate) mod omarchy;
mod panes;
mod picker;
mod preview_shot;
mod profile_ui;
mod settings;
pub(crate) use settings::selftest_reply_text;
mod side_ui;
mod sound_ui;
mod stage;
mod summary;
mod templates;
mod textlist;
pub(crate) mod theme;
mod toast;
mod tokens;
#[cfg(test)]
mod ui_tests;
mod widgets;

use crate::config::schema::*;
use crate::engine::actions::Executor;
use crate::platform::shell;
use crate::service::runtime;
use crate::{strings, usb};
use eframe::egui::{self, Color32};
use picker::AppPicker;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use usb::UsbCmd;

const AUTOSAVE_DELAY: Duration = Duration::from_millis(300);
const SAVE_RETRY_DELAY: Duration = Duration::from_secs(3);
const REPAINT_EVERY: Duration = Duration::from_millis(100);

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
    query: String,
    clipboard: Option<ButtonConfig>,
    recording: Option<&'static str>,
    key_tex: key_tex::KeyTextures,
    picker: AppPicker,
    /// Recentes e número de aberturas (`editor.json`).
    memory: memory::Memory,
    app_icons: app_icons::AppIcons,
    /// Busca dos ícones dos sites das teclas "Abrir site".
    favicons: favicon::Favicons,
    toasts: toast::Toasts,
    settings_open: bool,
    /// Seção aberta nas Preferências (`settings::SECTIONS`).
    settings_tab: usize,
    /// Página pedida pelo botão "Abrir esta página" do inspetor (aplicada no fim do quadro).
    open_page: Option<u64>,
    autostart: bool,
    rename: Option<bars::Rename>,
    /// Modo compacto (janela estreita): a biblioteca aberta no lugar do inspetor.
    library_open: bool,
    /// Apps que estiveram em foco (do serviço), sugeridos na troca automática de perfil.
    recent_apps: profile_ui::RecentApps,
    /// Estado do OBS (do serviço), para as teclas do OBS aparecerem como no aparelho.
    obs_live: obs_ui::ObsLive,
    /// Janela da tela lateral aberta.
    side_open: bool,
    side_tex: side_ui::SideTexture,
    page_slide: stage::PageSlide,
    /// Ctrl+F pediu o foco na busca da biblioteca (aplicado quando ela é desenhada).
    focus_search: bool,
    /// F6 pediu o foco na tecla selecionada do palco.
    focus_key: bool,
    /// Retângulos das teclas no último quadro: Alt+setas acham a vizinha numa direção.
    key_rects: Vec<egui::Rect>,
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
            query: String::new(),
            clipboard: None,
            recording: None,
            key_tex: key_tex::KeyTextures::default(),
            picker: AppPicker::default(),
            memory: memory::Memory::open(),
            app_icons: app_icons::AppIcons::default(),
            favicons: favicon::Favicons::default(),
            toasts: toast::Toasts::default(),
            settings_open: false,
            settings_tab: 0,
            open_page: None,
            autostart: false,
            rename: None,
            library_open: false,
            recent_apps: profile_ui::RecentApps::default(),
            obs_live: obs_ui::ObsLive::default(),
            side_open: false,
            side_tex: side_ui::SideTexture::default(),
            page_slide: stage::PageSlide::default(),
            focus_search: false,
            focus_key: false,
            key_rects: Vec::new(),
        }
        .with_preview_state()
    }

    /// Só em debug: `KEYFORGE_PREVIEW_UI=key=5,tab=1,settings,picker,library` abre o editor já
    /// numa tela, para conferir (e fotografar) cada uma sem clicar. O release ignora.
    fn with_preview_state(mut self) -> Self {
        if !cfg!(debug_assertions) {
            return self;
        }
        let Some(spec) =
            crate::config::schema::dev_var("PREVIEW_UI").and_then(|v| v.into_string().ok())
        else {
            return self;
        };
        for item in spec.split(',').map(str::trim) {
            match item.split_once('=') {
                Some(("key", n)) => {
                    self.selected = n.parse::<usize>().unwrap_or(1).saturating_sub(1)
                }
                Some(("tab", n)) => self.tab = n.parse().unwrap_or(0),
                _ if item == "settings" => self.settings_open = true,
                _ if item == "picker" => self.picker.show(),
                _ if item == "library" => self.library_open = true,
                _ if item == "side" => self.side_open = true,
                _ => {}
            }
        }
        self
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
        self.memory.used(p.label);
        let mut action = p.resolved_action(&g.cfg);
        if p.creates_folder() {
            let prof = &mut g.cfg.profiles[g.cfg.active_profile];
            let parent = prof.pages[prof.active_page].id;
            let folders = prof.pages.iter().filter(|pg| pg.folder).count();
            let name = strings::tf("Pasta {}", &[&(folders + 1)]);
            let folder = prof.add_folder(&name, parent, strings::t("Voltar"));
            action = ButtonAction::SwitchPage { page: folder };
            self.toasts.push(strings::t(FOLDER_HINT), None);
        }
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
        b.appearance.background = BLANK_BACKGROUND;
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

    /// Abre a página `id` do perfil ativo no editor (e no aparelho), como entrar numa pasta.
    fn go_to_page(&mut self, g: &mut runtime::Shared, id: u64) {
        let prof = &mut g.cfg.profiles[g.cfg.active_profile];
        if let Some(index) = prof.page_index(id) {
            prof.active_page = index;
            self.selected = 0;
            self.touch_nav();
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
        // Ler e gravar sob a trava: o serviço pode estar gravando uma troca de página agora.
        let saved = AppConfig::locked(|| {
            if !self.nav_dirty {
                if let Some(disk) = AppConfig::load_existing() {
                    out.adopt_active_from(&disk);
                }
            }
            out.save()
        });
        match saved {
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
        if self.recording.is_none() && !ctx.egui_wants_keyboard_input() {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z)
                    || i.consume_key(egui::Modifiers::CTRL, egui::Key::Y)
                {
                    redo = true;
                } else if i.consume_key(egui::Modifiers::CTRL, egui::Key::Z) {
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
    number: Option<usize>,
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
        show_text: ap.show_text,
    }
}

/// Como se entra numa pasta para montá-la.
const FOLDER_HINT: &str =
    "Pasta criada. \"Abrir esta página\" (ou duplo clique na tecla) entra nela para montar as teclas.";

/// Minuto atual (para a prévia da tecla de relógio mudar junto com a do aparelho).
pub(crate) fn current_minute() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 60)
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        let sh = self.sh.clone();
        let mut guard = runtime::lock(&sh);
        let g: &mut runtime::Shared = &mut guard;
        self.autosave(&g.cfg);
        self.obs_live.refresh(&g.cfg);
        self.key_tex.live.clone_from(&g.live);
        ctx.request_repaint_after(REPAINT_EVERY);

        // O Omarchy trocou de tema: força reaplicar as cores sem esperar o usuário mexer em nada.
        if omarchy::refresh() {
            self.applied_ui = None;
        }
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
        self.global_shortcuts(ctx);

        let panels = layout::panels(ctx.content_rect().width(), self.library_open);
        let undo_redo = (&mut want_undo, &mut want_redo);
        self.top_bar(root, g, (&pal, accent), undo_redo, panels);
        if panels.library {
            self.library(root, g, &pal, accent);
        }
        if panels.inspector {
            self.inspector(root, g, &pal, accent, panels.inspector_width);
        }
        self.stage(root, g, &pal, accent);
        self.app_picker(ctx, g);
        self.settings_window(ctx, g, &pal);
        self.side_window(ctx, g, &pal);
        want_undo |= self.toasts.show(ctx, &pal, g.cfg.ui.reduce_motion);
        // Depois de todas as janelas: o anel fica por cima de modais e menus também.
        widgets::focus_ring(ctx);
        preview_shot::tick(ctx);

        self.track_history(g, before, want_undo, want_redo);
        self.edited = false;
    }
}

#[cfg(test)]
mod tests;
