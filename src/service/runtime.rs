//! Núcleo que roda independente da janela: recebe eventos do aparelho, executa ações
//! e redesenha os botões. A UI e este módulo compartilham o estado via `SharedState`.
use crate::config::schema::{AppConfig, ButtonAction, ButtonConfig, RGBAColor};
use crate::engine::actions::Executor;
use crate::graphics::mixer_art::render_mixer_key;
use crate::graphics::renderer::{encode_frame, render_button_frame};
use crate::mixer::{MixerKey, MixerView};
use crate::service::mixer_runtime::{self, StepHold};
use crate::usb::{UsbCmd, UsbEvent};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const MAX_KEYS: usize = 64;
const STATUS_POLL: Duration = Duration::from_secs(1);

pub struct Shared {
    pub cfg: AppConfig,
    pub connected: bool,
    pub icon_px: u32,
    /// Mixer de volume aberto no aparelho; enquanto existir, ele substitui a página nas teclas.
    pub mixer: Option<MixerView>,
}

pub type SharedState = Arc<Mutex<Shared>>;

pub fn new_state(cfg: AppConfig, icon_px: u32) -> SharedState {
    Arc::new(Mutex::new(Shared {
        cfg,
        connected: false,
        icon_px,
        mixer: None,
    }))
}

/// Trava o estado; se outra thread entrou em pânico com ele travado, recupera o valor.
/// (Só ocorre fora do release: lá `panic = "abort"` encerra o processo antes.)
pub fn lock(state: &SharedState) -> MutexGuard<'_, Shared> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

const PATTERN_FONT: f32 = 40.0;
const CALIBRATION_SIZES: [u32; 15] = [
    80, 85, 90, 95, 96, 100, 104, 108, 112, 116, 120, 124, 126, 128, 132,
];
const SWEEP_START: u32 = 118;
const SWEEP_STEP: u32 = 2;

/// Cópia dos botões da página ativa. Desenhar e codificar JPEG é lento: com a cópia, isso roda
/// depois de soltar o lock, e o servidor de controle continua respondendo a `status`.
pub struct Snapshot {
    /// (índice da tecla, o que desenhar). Com o mixer aberto só vão as teclas que mudaram.
    frames: Vec<(usize, Frame)>,
    px: u32,
}

/// Uma tecla a desenhar: botão da configuração ou tecla do mixer (que tem desenho próprio).
enum Frame {
    Button(Box<ButtonConfig>),
    Mixer(MixerKey),
}

impl Snapshot {
    pub fn take(sh: &Shared) -> Self {
        Self {
            frames: match &sh.mixer {
                Some(mixer) => {
                    let [r, g, b] = sh.cfg.ui.accent;
                    let accent = RGBAColor { r, g, b, a: 255 };
                    mixer
                        .changed_keys(accent)
                        .into_iter()
                        .map(|(index, key)| (index, Frame::Mixer(key)))
                        .collect()
                }
                None => sh
                    .cfg
                    .page()
                    .buttons
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(index, b)| (index, Frame::Button(Box::new(b))))
                    .collect(),
            },
            px: sh.icon_px,
        }
    }

    pub fn render(&self, tx: &Sender<UsbCmd>) {
        for (index, frame) in &self.frames {
            let index = *index;
            let data = match frame {
                Frame::Button(b) => render_button_frame(b, self.px, self.px),
                Frame::Mixer(key) => encode_frame(&render_mixer_key(key, self.px)),
            };
            let _ = tx.send(UsbCmd::Frame { index, data });
        }
    }
}

/// Garante que desenhos feitos por threads diferentes cheguem ao aparelho na mesma ordem em que o
/// estado mudou. Sem isso, com redesenho parcial, um quadro antigo poderia sobrepor um novo e ficar
/// na tela, porque o que já foi "desenhado" não é reenviado.
static DRAW_ORDER: Mutex<()> = Mutex::new(());

/// Muda o estado com `change`, desenha o resultado (se `change` devolver `true` no segundo valor)
/// e devolve o primeiro valor. Mudança e desenho são feitos em bloco, na ordem de chegada.
pub fn change_and_draw<R>(
    state: &SharedState,
    tx: &Sender<UsbCmd>,
    change: impl FnOnce(&mut Shared) -> (R, bool),
) -> R {
    let _order = DRAW_ORDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (result, snapshot) = {
        let mut sh = lock(state);
        let (result, draw) = change(&mut sh);
        (result, draw.then(|| Snapshot::take(&sh)))
    };
    if let Some(snapshot) = snapshot {
        snapshot.render(tx);
    }
    result
}

/// Depois de um desenho que não passa pelo mixer (calibração, padrão de teste), a tela não é mais a
/// dele: esquece o que ele tinha desenhado para que "reload" reenvie as 15 teclas.
pub fn forget_mixer_drawing(state: &SharedState) {
    if let Some(mixer) = &lock(state).mixer {
        mixer.invalidate();
    }
}

/// Padrão de diagnóstico: cada tecla com cor própria e o número dela (revela teclas que não atualizam,
/// espelhamento e rotação). A configuração do usuário não é alterada; "reload" restaura as imagens.
/// Calibração: com `px` fixo, todas as teclas recebem o mesmo tamanho; sem ele, cada tecla recebe um
/// tamanho diferente de CALIBRATION_SIZES (na ordem da grade) para comparar numa foto só.
pub fn render_calibration(key_count: usize, tx: &Sender<UsbCmd>, px: Option<u32>) {
    for index in 0..key_count {
        let size = px.unwrap_or(CALIBRATION_SIZES[index % CALIBRATION_SIZES.len()]);
        let data = crate::graphics::renderer::encode_frame(
            &crate::graphics::renderer::calibration_rgba(size),
        );
        let _ = tx.send(UsbCmd::Frame { index, data });
    }
}

pub fn render_pattern(key_count: usize, tx: &Sender<UsbCmd>, px: Option<u32>) {
    const HUES: [[u8; 3]; 6] = [
        [200, 40, 40],
        [40, 160, 60],
        [50, 90, 210],
        [200, 160, 30],
        [150, 60, 190],
        [30, 160, 170],
    ];
    for index in 0..key_count {
        let mut b = ButtonConfig::blank();
        let [r, g, bl] = HUES[index % HUES.len()];
        b.appearance.background = RGBAColor {
            r,
            g,
            b: bl,
            a: 255,
        };
        b.appearance.text_color = RGBAColor {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let size = px.unwrap_or(SWEEP_START + SWEEP_STEP * index as u32);
        b.appearance.text = if px.is_some() {
            format!("{}", index + 1)
        } else {
            size.to_string()
        };
        b.appearance.font_size = PATTERN_FONT;
        let data = render_button_frame(&b, size, size);
        let _ = tx.send(UsbCmd::Frame { index, data });
    }
}

/// Perfil e página ativos (por id), capturados com o lock para gravar depois, sem ele.
pub struct Nav {
    profile_id: u64,
    page_id: Option<u64>,
}

impl Nav {
    fn capture(sh: &Shared) -> Option<Self> {
        let profile = sh.cfg.profiles.get(sh.cfg.active_profile)?;
        Some(Self {
            profile_id: profile.id,
            page_id: profile.pages.get(profile.active_page).map(|p| p.id),
        })
    }
}

/// O que falta fazer depois de soltar o lock: gravar a navegação e redesenhar o aparelho.
#[derive(Default)]
pub struct Followup {
    nav: Option<Nav>,
    snapshot: Option<Snapshot>,
    /// A ação pediu o mixer; guarda a tecla que o abriu, que vira "sair/mais".
    open_mixer: Option<usize>,
}

impl Followup {
    /// A ação pediu o mixer: o serviço o abre depois de soltar o lock (ler o áudio é lento).
    pub fn mixer_home(&self) -> Option<usize> {
        self.open_mixer
    }

    pub fn run(self, tx: &Sender<UsbCmd>) {
        if let Some(nav) = &self.nav {
            persist(nav);
        }
        if let Some(snapshot) = &self.snapshot {
            snapshot.render(tx);
        }
    }
}

/// Mantém o arquivo em dia para que o editor abra na página/perfil atuais.
/// Relê o disco e muda só perfil/página ativos (por id): gravar a cópia em memória inteira
/// apagaria uma edição do editor cujo "reload" ainda não chegou ao serviço.
fn persist(nav: &Nav) {
    let Some(mut disk) = AppConfig::load_existing() else {
        return;
    };
    let Some(profile_index) = disk.profile_index(nav.profile_id) else {
        return;
    };
    let page_index = nav
        .page_id
        .and_then(|id| disk.profiles[profile_index].page_index(id));
    let unchanged = disk.active_profile == profile_index
        && page_index.is_none_or(|i| disk.profiles[profile_index].active_page == i);
    if unchanged {
        return;
    }
    disk.active_profile = profile_index;
    if let Some(i) = page_index {
        disk.profiles[profile_index].active_page = i;
    }
    if let Err(e) = disk.save() {
        log::error!("salvar troca de página: {e}");
    }
}

/// Executa a ação sob o lock; troca de página/perfil volta como `Followup` (gravar e redesenhar).
fn dispatch(sh: &mut Shared, exec: &Executor, index: usize, action: ButtonAction) -> Followup {
    match action {
        ButtonAction::SwitchPage { page } => {
            let i = sh.cfg.active_profile;
            let profile = &mut sh.cfg.profiles[i];
            let Some(index) = profile.page_index(page) else {
                return Followup::default();
            };
            profile.active_page = index;
        }
        ButtonAction::SwitchProfile { profile } => {
            let Some(index) = sh.cfg.profile_index(profile) else {
                return Followup::default();
            };
            sh.cfg.active_profile = index;
        }
        ButtonAction::NextPage | ButtonAction::PreviousPage => {
            let forward = matches!(action, ButtonAction::NextPage);
            let i = sh.cfg.active_profile;
            let profile = &mut sh.cfg.profiles[i];
            let count = profile.pages.len();
            if count < 2 {
                return Followup::default();
            }
            let current = profile.active_page;
            profile.active_page = if forward {
                (current + 1) % count
            } else {
                (current + count - 1) % count
            };
        }
        ButtonAction::OpenMixer => {
            return Followup {
                open_mixer: Some(index),
                ..Followup::default()
            };
        }
        other => {
            exec.run(other);
            return Followup::default();
        }
    }
    Followup {
        nav: Nav::capture(sh),
        snapshot: Some(Snapshot::take(sh)),
        open_mixer: None,
    }
}

/// Dispara e conclui na hora (botão "Testar" do editor e testes). O serviço usa `trigger_locked`
/// para soltar o lock antes de gravar e desenhar.
pub fn trigger(
    sh: &mut Shared,
    tx: &Sender<UsbCmd>,
    exec: &Executor,
    index: usize,
    held: Duration,
) {
    trigger_locked(sh, exec, index, held).run(tx);
}

/// Parte do disparo que precisa do lock; `held` decide entre ação normal e de pressão longa.
pub fn trigger_locked(sh: &mut Shared, exec: &Executor, index: usize, held: Duration) -> Followup {
    let Some(b) = sh.cfg.page().buttons.get(index).cloned() else {
        return Followup::default();
    };
    let long = held >= Duration::from_millis(b.long_press_threshold_ms);
    let toggles = b.toggles();
    // Tecla de dois estados: no segundo toque (já ligada) vale a ação do estado ligado, se houver.
    let tap = match (&b.toggled_action, b.toggle_state) {
        (Some(a), true) => a,
        _ => &b.action,
    };
    let action = match (&b.long_press_action, long) {
        (Some(a), true) => a.clone(),
        _ => tap.clone(),
    };
    if toggles {
        sh.cfg.page_mut().buttons[index].toggle_state = !b.toggle_state;
    }
    let mut followup = dispatch(sh, exec, index, action);
    if toggles && followup.snapshot.is_none() {
        followup.snapshot = Some(Snapshot::take(sh));
    }
    followup
}

/// No editor: consulta o serviço a cada segundo para mostrar o estado do aparelho.
pub fn spawn_status_poll(state: SharedState) {
    std::thread::spawn(move || loop {
        let reply = crate::platform::shell::send("status");
        let mut parts = reply.as_deref().unwrap_or("0").split_whitespace();
        let connected = parts.next() == Some("1");
        let icon_px = parts.next().and_then(|v| v.parse().ok());
        {
            let mut sh = lock(&state);
            sh.connected = connected;
            if let Some(px) = icon_px {
                sh.icon_px = px;
            }
        }
        std::thread::sleep(STATUS_POLL);
    });
}

/// Thread que consome os eventos do aparelho (vive mesmo com a janela escondida).
pub fn spawn(state: SharedState, tx: Sender<UsbCmd>, rx: Receiver<UsbEvent>, exec: Executor) {
    std::thread::spawn(move || {
        let mut down: [Option<Instant>; MAX_KEYS] = [None; MAX_KEYS];
        let mut hold: Option<StepHold> = None;
        let mut next_refresh = Instant::now() + mixer_runtime::REFRESH;
        let mut was_open = false;
        loop {
            let mixer_open = mixer_runtime::is_open(&state);
            if mixer_open != was_open {
                // Uma tecla apertada num modo e solta no outro não vale: agiria na tela errada.
                was_open = mixer_open;
                down = [None; MAX_KEYS];
                hold = None;
            }
            let event = if mixer_open {
                let wake = hold
                    .as_ref()
                    .map_or(next_refresh, |h| next_refresh.min(h.due()));
                match rx.recv_timeout(wake.saturating_duration_since(Instant::now())) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                hold = None;
                match rx.recv() {
                    Ok(event) => Some(event),
                    Err(_) => break,
                }
            };
            if mixer_open {
                let now = Instant::now();
                if now >= next_refresh {
                    mixer_runtime::reload(&state, &tx);
                    next_refresh = now + mixer_runtime::REFRESH;
                }
                if let Some(h) = hold.as_mut() {
                    mixer_runtime::repeat_if_due(&state, &tx, h, now);
                }
            }
            let Some(event) = event else { continue };
            match event {
                UsbEvent::Info { icon_px } => lock(&state).icon_px = icon_px,
                UsbEvent::Status(connected) => {
                    // Sem aparelho não chega a soltura da tecla que encerraria uma repetição.
                    hold = None;
                    down = [None; MAX_KEYS];
                    change_and_draw(&state, &tx, |sh| {
                        sh.connected = connected;
                        // Aparelho (re)conectado: o que estava desenhado se perdeu, então tudo volta.
                        if let Some(mixer) = &sh.mixer {
                            mixer.invalidate();
                        }
                        ((), connected)
                    });
                }
                UsbEvent::Key { index, pressed } if index < MAX_KEYS => {
                    if mixer_open {
                        // Com o mixer aberto +/− e o ícone agem ao apertar; só a tecla de sair espera a soltura.
                        if pressed {
                            down[index] = Some(Instant::now());
                            // Só uma tecla de + ou − nova substitui a repetição em andamento.
                            if let Some(new_hold) = mixer_runtime::key_down(&state, &tx, index) {
                                hold = Some(new_hold);
                            }
                        } else {
                            if hold.as_ref().is_some_and(|h| h.index() == index) {
                                hold = None;
                            }
                            // Soltura sem aperto registrado (aperto feito antes do mixer abrir) é ignorada.
                            if let Some(since) = down[index].take() {
                                if mixer_runtime::key_up(&state, &tx, index, since.elapsed()) {
                                    // A página mudou ou fechou: a repetição apontaria para outro app.
                                    hold = None;
                                }
                            }
                        }
                    } else if pressed {
                        down[index] = Some(Instant::now());
                    } else if let Some(since) = down[index].take() {
                        let followup =
                            trigger_locked(&mut lock(&state), &exec, index, since.elapsed());
                        let mixer_home = followup.mixer_home();
                        followup.run(&tx);
                        if let Some(opener) = mixer_home {
                            mixer_runtime::open(&state, &tx, opener);
                        }
                    }
                }
                UsbEvent::Key { .. } => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn two_page_state(action: ButtonAction, long: Option<ButtonAction>) -> Shared {
        let mut cfg = AppConfig::default();
        cfg.profiles[0].add_page("Page 2");
        let b = &mut cfg.page_mut().buttons[0];
        b.action = action;
        b.long_press_action = long;
        Shared {
            cfg,
            connected: true,
            icon_px: 85,
            mixer: None,
        }
    }

    #[tokio::test]
    async fn short_press_runs_normal_action_and_redraws() {
        let (tx, rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::SwitchPage { page: 2 }, None);
        trigger(&mut sh, &tx, &exec, 0, Duration::from_millis(50));
        assert_eq!(sh.cfg.profiles[0].active_page, 1);
        assert_eq!(rx.try_iter().count(), 15);
    }

    #[tokio::test]
    async fn long_press_uses_long_action_only_past_threshold() {
        let (tx, _rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(
            ButtonAction::None,
            Some(ButtonAction::SwitchPage { page: 2 }),
        );
        trigger(&mut sh, &tx, &exec, 0, Duration::from_millis(100));
        assert_eq!(sh.cfg.profiles[0].active_page, 0);
        trigger(&mut sh, &tx, &exec, 0, Duration::from_millis(600));
        assert_eq!(sh.cfg.profiles[0].active_page, 1);
    }

    #[tokio::test]
    async fn toggle_with_page_switch_redraws_once() {
        let (tx, rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::SwitchPage { page: 2 }, None);
        sh.cfg.page_mut().buttons[0].toggled_appearance = Some(ButtonConfig::blank().appearance);
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert_eq!(rx.try_iter().count(), 15);
    }

    #[tokio::test]
    async fn toggle_without_page_change_still_redraws() {
        let (tx, rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::SwitchPage { page: 9 }, None);
        sh.cfg.page_mut().buttons[0].toggled_appearance = Some(ButtonConfig::blank().appearance);
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert!(sh.cfg.page().buttons[0].toggle_state);
        assert_eq!(rx.try_iter().count(), 15);
    }

    #[tokio::test]
    async fn out_of_range_page_is_ignored() {
        let (tx, rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::SwitchPage { page: 9 }, None);
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert_eq!(sh.cfg.profiles[0].active_page, 0);
        assert_eq!(rx.try_iter().count(), 0);
    }

    #[tokio::test]
    async fn open_mixer_action_asks_the_service_to_open_it() {
        let (tx, rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::OpenMixer, None);
        let followup = trigger_locked(&mut sh, &exec, 0, Duration::ZERO);
        assert_eq!(followup.mixer_home(), Some(0));
        followup.run(&tx);
        assert_eq!(rx.try_iter().count(), 0);
        assert_eq!(sh.cfg.profiles[0].active_page, 0);
    }

    #[tokio::test]
    async fn next_and_previous_page_wrap_around() {
        let (tx, rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::NextPage, Some(ButtonAction::PreviousPage));
        // A tecla age na página atual, então as duas páginas precisam ter a mesma ação.
        for page in &mut sh.cfg.profiles[0].pages {
            page.buttons[0].action = ButtonAction::NextPage;
            page.buttons[0].long_press_action = Some(ButtonAction::PreviousPage);
        }
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert_eq!(sh.cfg.profiles[0].active_page, 1);
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert_eq!(
            sh.cfg.profiles[0].active_page, 0,
            "depois da última volta à primeira"
        );
        trigger(&mut sh, &tx, &exec, 0, Duration::from_secs(5));
        assert_eq!(
            sh.cfg.profiles[0].active_page, 1,
            "antes da primeira vai à última"
        );
        assert_eq!(
            rx.try_iter().count(),
            45,
            "cada troca redesenha as 15 teclas"
        );
    }

    #[tokio::test]
    async fn next_page_does_nothing_in_a_single_page_profile() {
        let (tx, rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::NextPage, None);
        sh.cfg.profiles[0].pages.truncate(1);
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert_eq!(sh.cfg.profiles[0].active_page, 0);
        assert_eq!(rx.try_iter().count(), 0);
    }

    #[tokio::test]
    async fn second_tap_of_a_two_state_key_runs_its_toggled_action() {
        let (tx, _rx) = channel();
        let exec = Executor::new(tokio::runtime::Handle::current());
        let mut sh = two_page_state(ButtonAction::None, None);
        let key = &mut sh.cfg.page_mut().buttons[0];
        key.toggled_appearance = Some(ButtonConfig::blank().appearance);
        key.toggled_action = Some(ButtonAction::SwitchPage { page: 2 });
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert_eq!(
            sh.cfg.profiles[0].active_page, 0,
            "o primeiro toque usa a ação normal"
        );
        trigger(&mut sh, &tx, &exec, 0, Duration::ZERO);
        assert_eq!(
            sh.cfg.profiles[0].active_page, 1,
            "o segundo usa a do estado ligado"
        );
    }

    fn mixer_state(opener: usize) -> SharedState {
        let state = Arc::new(Mutex::new(two_page_state(ButtonAction::None, None)));
        lock(&state).mixer = Some(MixerView::new(opener));
        state
    }

    fn drawn(state: &SharedState, tx: &Sender<UsbCmd>) {
        let snapshot = Snapshot::take(&lock(state));
        snapshot.render(tx);
    }

    #[test]
    fn mixer_draws_everything_once_then_only_what_changes() {
        let (tx, rx) = channel();
        let state = mixer_state(14);
        drawn(&state, &tx);
        assert_eq!(rx.try_iter().count(), 15);
        drawn(&state, &tx);
        assert_eq!(rx.try_iter().count(), 0);
    }

    #[test]
    fn a_reconnected_device_gets_the_whole_mixer_again() {
        let (tx, rx) = channel();
        let state = mixer_state(14);
        drawn(&state, &tx);
        rx.try_iter().count();
        lock(&state).mixer.as_ref().unwrap().invalidate();
        drawn(&state, &tx);
        assert_eq!(rx.try_iter().count(), 15);
    }

    #[test]
    fn tapping_the_exit_key_closes_the_mixer_and_restores_the_page() {
        let (tx, rx) = channel();
        let state = mixer_state(14);
        drawn(&state, &tx);
        rx.try_iter().count();
        mixer_runtime::key_up(&state, &tx, 14, Duration::ZERO);
        assert!(!mixer_runtime::is_open(&state));
        assert_eq!(rx.try_iter().count(), 15);
    }

    #[test]
    fn pressing_an_empty_column_does_not_start_a_repeat() {
        let (tx, _rx) = channel();
        let state = mixer_state(14);
        assert!(mixer_runtime::key_down(&state, &tx, 5).is_none());
    }

    #[test]
    fn holding_plus_repeats_only_after_the_delay() {
        use crate::mixer::{MixerApp, MixerData, REPEAT_DELAY};
        use crate::platform::audio::AudioSession;
        let (tx, rx) = channel();
        let state = mixer_state(14);
        let app = MixerApp {
            session: AudioSession {
                key: "teste".into(),
                name: "Teste".into(),
                exe_path: None,
                // Nenhum processo real tem este número, então o Windows não é alterado.
                pids: vec![u32::MAX],
                volume: 0.5,
                muted: false,
                active: true,
            },
            icon_path: None,
        };
        {
            let mut sh = lock(&state);
            let mixer = sh.mixer.as_mut().unwrap();
            let version = mixer.version();
            mixer.update(MixerData { apps: vec![app] }, version);
        }
        drawn(&state, &tx);
        rx.try_iter().count();
        let mut hold = mixer_runtime::key_down(&state, &tx, 5).expect("tecla + segura");
        let start = Instant::now();
        mixer_runtime::repeat_if_due(&state, &tx, &mut hold, start);
        let after_press = rx.try_iter().count();
        mixer_runtime::repeat_if_due(&state, &tx, &mut hold, start + REPEAT_DELAY);
        assert!(rx.try_iter().count() > 0 && after_press > 0);
        assert!(hold.due() > start + REPEAT_DELAY);
    }
}
