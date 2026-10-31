//! Núcleo que roda independente da janela: recebe eventos do aparelho, executa ações
//! e redesenha os botões. A UI e este módulo compartilham o estado via `SharedState`.
use crate::config::schema::{AppConfig, ButtonAction, ButtonConfig, RGBAColor, SideScreen};
use crate::engine::actions::Executor;
use crate::graphics::mixer_art::render_mixer_key;
use crate::graphics::renderer::{encode_frame, render_button_frame};
use crate::graphics::side::SideInfo;
use crate::mixer::{MixerKey, MixerView};
use crate::service::mixer_runtime::{self, StepHold};
use crate::usb::{SideFrame, UsbCmd, UsbEvent};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const MAX_KEYS: usize = 64;
/// Brilho mínimo que uma tecla de brilho deixa (abaixo disso a tela parece desligada).
pub const MIN_BRIGHTNESS: u8 = 5;
const STATUS_POLL: Duration = Duration::from_secs(1);

pub struct Shared {
    pub cfg: AppConfig,
    pub connected: bool,
    /// No editor: o serviço respondeu ao último `status`. Sem ele nada chega ao aparelho.
    pub service_running: bool,
    pub icon_px: u32,
    /// Mixer de volume aberto no aparelho; enquanto existir, ele substitui a página nas teclas.
    pub mixer: Option<MixerView>,
    /// No serviço: apps que ganharam o foco há pouco (o mais novo primeiro), para o editor sugerir.
    pub recent_apps: Vec<String>,
    /// No serviço: o que se sabe do OBS (conexão, gravação, cena...), para as teclas do OBS.
    pub obs: crate::obs::ObsState,
    /// No serviço: descanso da tela (apaga após um tempo sem uso).
    pub sleep: super::sleep::SleepState,
}

pub type SharedState = Arc<Mutex<Shared>>;

pub fn new_state(cfg: AppConfig, icon_px: u32) -> SharedState {
    Arc::new(Mutex::new(Shared {
        cfg,
        connected: false,
        service_running: true,
        icon_px,
        mixer: None,
        recent_apps: Vec::new(),
        obs: crate::obs::ObsState::default(),
        sleep: super::sleep::SleepState::default(),
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
    /// Tela lateral da página (só nos desenhos da página inteira; o mixer não a usa).
    side: Option<(SideScreen, SideInfo)>,
}

/// O que a tela lateral da página ativa mostra agora e com que dados.
fn side_of(sh: &Shared) -> (SideScreen, SideInfo) {
    let profile = &sh.cfg.profiles[sh.cfg.active_profile.min(sh.cfg.profiles.len() - 1)];
    let page_index = profile.active_page.min(profile.pages.len() - 1);
    let [r, g, b] = sh.cfg.ui.accent;
    let info = SideInfo {
        profile: crate::strings::display_name(&profile.name),
        page: crate::strings::display_name(&profile.pages[page_index].name),
        page_index,
        page_count: profile.pages.len(),
        accent: RGBAColor { r, g, b, a: 255 },
    };
    (sh.cfg.page().side.clone(), info)
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
                    .enumerate()
                    .map(|(index, b)| (index, Frame::Button(Box::new(shown(sh, b)))))
                    .collect(),
            },
            px: sh.icon_px,
            side: sh.mixer.is_none().then(|| side_of(sh)),
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
        if let Some((side, info)) = &self.side {
            let frame = match crate::graphics::side::render(side, info) {
                Some(img) => SideFrame::Strips(crate::graphics::side::strips(&img)),
                None => SideFrame::Clear,
            };
            let _ = tx.send(UsbCmd::Side(frame));
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
    /// A ação mudou o brilho: vai ao aparelho e ao arquivo depois de soltar o lock.
    brightness: Option<u8>,
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
        if let Some(level) = self.brightness {
            crate::usb::set_brightness(level);
            let _ = tx.send(UsbCmd::Brightness(level));
            persist_brightness(level);
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
    AppConfig::locked(|| persist_unlocked(nav));
}

/// Grava o brilho mudado por uma tecla, para o editor e o próximo início do serviço o verem.
fn persist_brightness(level: u8) {
    AppConfig::locked(|| {
        let Some(mut disk) = AppConfig::load_existing() else {
            return;
        };
        if disk.ui.brightness != level {
            disk.ui.brightness = level;
            if let Err(e) = disk.save() {
                log::error!("salvar brilho: {e}");
            }
        }
    });
}

/// Brilho depois de somar `delta`, entre o mínimo útil e o máximo do aparelho.
pub fn stepped_brightness(current: u8, delta: i32) -> u8 {
    let level = i32::from(current) + delta;
    level.clamp(
        i32::from(MIN_BRIGHTNESS),
        i32::from(crate::usb::MAX_BRIGHTNESS),
    ) as u8
}

fn persist_unlocked(nav: &Nav) {
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

/// Ativa o perfil de índice `index` (troca automática): redesenha e grava a navegação.
pub fn switch_profile(state: &SharedState, tx: &Sender<UsbCmd>, index: usize) {
    let nav = change_and_draw(state, tx, |sh| {
        if index >= sh.cfg.profiles.len() {
            return (None, false);
        }
        sh.cfg.active_profile = index;
        (Nav::capture(sh), true)
    });
    if let Some(nav) = nav {
        persist(&nav);
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
        ButtonAction::Brightness { delta } => {
            let level = stepped_brightness(sh.cfg.ui.brightness, delta);
            sh.cfg.ui.brightness = level;
            return Followup {
                brightness: Some(level),
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
        brightness: None,
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
    let long_action = b.long_press_action.as_ref().filter(|_| long);
    // A pressão longa com ação própria não é um toque: não alterna a tecla de dois estados.
    let toggles = b.toggles() && long_action.is_none();
    // Tecla de dois estados: no segundo toque (já ligada) vale a ação do estado ligado, se houver.
    let tap = match (&b.toggled_action, b.toggle_state) {
        (Some(a), true) => a,
        _ => &b.action,
    };
    let action = long_action.unwrap_or(tap).clone();
    if toggles {
        sh.cfg.page_mut().buttons[index].toggle_state = !b.toggle_state;
    }
    let mut followup = dispatch(sh, exec, index, action);
    if toggles && followup.snapshot.is_none() {
        followup.snapshot = Some(Snapshot::take(sh));
    }
    followup
}

/// Tecla solta no aparelho: dispara a ação e desenha sob `DRAW_ORDER`, como `change_and_draw`.
/// Sem isso um "reload" simultâneo intercalaria os quadros dele com os desta troca de página, e
/// teclas ficariam com a imagem antiga. Devolve a tecla que abriu o mixer, se a ação pediu um:
/// abri-lo usa `change_and_draw`, então fica para depois de soltar `DRAW_ORDER`.
fn press(
    state: &SharedState,
    tx: &Sender<UsbCmd>,
    exec: &Executor,
    index: usize,
    held: Duration,
) -> Option<usize> {
    let _order = DRAW_ORDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let followup = trigger_locked(&mut lock(state), exec, index, held);
    let mixer_home = followup.mixer_home();
    followup.run(tx);
    mixer_home
}

/// O botão como deve aparecer agora: teclas do OBS refletem o estado do OBS (gravando, cena...).
fn shown(sh: &Shared, b: &ButtonConfig) -> ButtonConfig {
    match &b.action {
        ButtonAction::Obs { command, target } => {
            crate::obs::live_look(b, *command, sh.obs.is_active(*command, target))
        }
        _ => b.clone(),
    }
}

/// Redesenha só as teclas da página que satisfazem `pick(índice, botão)`, na ordem dos outros desenhos.
pub fn redraw_where(
    state: &SharedState,
    tx: &Sender<UsbCmd>,
    pick: impl Fn(usize, &ButtonConfig) -> bool,
) {
    let _order = DRAW_ORDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let snapshot = {
        let sh = lock(state);
        if sh.mixer.is_some() {
            return;
        }
        Snapshot {
            frames: sh
                .cfg
                .page()
                .buttons
                .iter()
                .enumerate()
                .filter(|(i, b)| pick(*i, b))
                .map(|(i, b)| (i, Frame::Button(Box::new(shown(&sh, b)))))
                .collect(),
            px: sh.icon_px,
            side: None,
        }
    };
    snapshot.render(tx);
}

/// Redesenha só a tela lateral (relógio dela na virada do minuto).
fn redraw_side(state: &SharedState, tx: &Sender<UsbCmd>) {
    let _order = DRAW_ORDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let snapshot = {
        let sh = lock(state);
        if sh.mixer.is_some() {
            return;
        }
        Snapshot {
            frames: Vec::new(),
            px: sh.icon_px,
            side: Some(side_of(&sh)),
        }
    };
    snapshot.render(tx);
}

/// Desenha a tecla `index` "afundada" enquanto o dedo está nela.
fn show_pressed(state: &SharedState, tx: &Sender<UsbCmd>, index: usize) {
    let _order = DRAW_ORDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (button, px) = {
        let sh = lock(state);
        if sh.mixer.is_some() {
            return;
        }
        let Some(b) = sh.cfg.page().buttons.get(index) else {
            return;
        };
        // Tecla vazia não reage: não há o que afundar.
        if b.action == ButtonAction::None {
            return;
        }
        (shown(&sh, b), sh.icon_px)
    };
    let data = encode_frame(&crate::graphics::renderer::render_pressed_rgba(
        &button, px, px,
    ));
    let _ = tx.send(UsbCmd::Frame { index, data });
}

/// Redesenha só a tecla `index` da página ativa.
fn redraw_key(state: &SharedState, tx: &Sender<UsbCmd>, index: usize) {
    redraw_where(state, tx, |i, _| i == index);
}

/// Folga depois da virada do minuto, para a hora lida já ser a nova.
const CLOCK_SLACK: Duration = Duration::from_millis(50);

/// Tempo até o próximo minuto cheio do relógio do sistema.
fn until_next_minute() -> Duration {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let into_minute = Duration::from_millis((since_epoch.as_millis() % 60_000) as u64);
    Duration::from_secs(60).saturating_sub(into_minute) + CLOCK_SLACK
}

/// Teclas de relógio da página ativa (nenhuma com o mixer aberto, que ocupa as teclas).
fn clock_keys(sh: &Shared) -> Vec<usize> {
    if sh.mixer.is_some() {
        return Vec::new();
    }
    sh.cfg
        .page()
        .buttons
        .iter()
        .enumerate()
        .filter(|(_, b)| b.action == ButtonAction::Clock)
        .map(|(i, _)| i)
        .collect()
}

fn has_clock(state: &SharedState) -> bool {
    let sh = lock(state);
    !clock_keys(&sh).is_empty() || side_clock(&sh)
}

/// A tela lateral da página mostra o relógio (e o mixer não está por cima)?
fn side_clock(sh: &Shared) -> bool {
    sh.mixer.is_none() && sh.cfg.page().side == SideScreen::Clock
}

/// Redesenha só as teclas de relógio (e a tela lateral, se ela for um relógio).
fn redraw_clocks(state: &SharedState, tx: &Sender<UsbCmd>) {
    redraw_where(state, tx, |_, b| b.action == ButtonAction::Clock);
    if side_clock(&lock(state)) {
        redraw_side(state, tx);
    }
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
            sh.service_running = reply.is_some();
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
            let sleep_due = {
                let sh = lock(&state);
                sh.sleep.deadline(sh.cfg.ui.sleep_minutes)
            };
            let event = if mixer_open {
                let wake = hold
                    .as_ref()
                    .map_or(next_refresh, |h| next_refresh.min(h.due()));
                let wake = sleep_due.map_or(wake, |due| wake.min(due));
                match rx.recv_timeout(wake.saturating_duration_since(Instant::now())) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                hold = None;
                // Com relógio na página, acorda na virada do minuto para redesenhá-lo; com
                // descanso ligado, na hora de apagar a tela.
                let clock = has_clock(&state).then(|| Instant::now() + until_next_minute());
                let wake = match (clock, sleep_due) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                match wake {
                    Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now()))
                    {
                        Ok(event) => Some(event),
                        Err(RecvTimeoutError::Timeout) => {
                            if clock.is_some_and(|c| Instant::now() >= c) {
                                redraw_clocks(&state, &tx);
                            }
                            None
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    },
                    None => match rx.recv() {
                        Ok(event) => Some(event),
                        Err(_) => break,
                    },
                }
            };
            {
                let mut sh = lock(&state);
                let minutes = sh.cfg.ui.sleep_minutes;
                if sh.sleep.sleep_if_due(Instant::now(), minutes, &tx) {
                    log::info!("tela em descanso após {minutes} min sem uso");
                }
            }
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
                    {
                        // Ao reconectar, o aparelho volta aceso: a contagem do descanso recomeça.
                        let mut sh = lock(&state);
                        let brightness = sh.cfg.ui.brightness;
                        sh.sleep.wake(brightness, &tx);
                    }
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
                    let swallowed = {
                        let mut sh = lock(&state);
                        let brightness = sh.cfg.ui.brightness;
                        sh.sleep.on_key(index, pressed, brightness, &tx)
                    };
                    if swallowed {
                        // O toque que acordou a tela não age; também não deixa aperto pendente.
                        down[index] = None;
                        continue;
                    }
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
                        show_pressed(&state, &tx, index);
                    } else if let Some(since) = down[index].take() {
                        let opener = press(&state, &tx, &exec, index, since.elapsed());
                        // Solta: a tecla volta ao desenho normal (o da página nova, se ela mudou).
                        redraw_key(&state, &tx, index);
                        if let Some(opener) = opener {
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
mod tests;
