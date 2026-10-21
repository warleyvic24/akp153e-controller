//! Núcleo que roda independente da janela: recebe eventos do aparelho, executa ações
//! e redesenha os botões. A UI e este módulo compartilham o estado via `SharedState`.
use crate::config::schema::{AppConfig, ButtonAction, ButtonConfig, RGBAColor};
use crate::engine::actions::Executor;
use crate::graphics::renderer::render_button_frame;
use crate::usb::{UsbCmd, UsbEvent};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const MAX_KEYS: usize = 64;
const STATUS_POLL: Duration = Duration::from_secs(1);

pub struct Shared {
    pub cfg: AppConfig,
    pub connected: bool,
    pub icon_px: u32,
}

pub type SharedState = Arc<Mutex<Shared>>;

pub fn new_state(cfg: AppConfig, icon_px: u32) -> SharedState {
    Arc::new(Mutex::new(Shared {
        cfg,
        connected: false,
        icon_px,
    }))
}

/// Trava o estado; se outra thread entrou em pânico com ele travado, recupera o valor.
/// (Só ocorre fora do release: lá `panic = "abort"` encerra o processo antes.)
pub fn lock(state: &SharedState) -> MutexGuard<'_, Shared> {
    state.lock().unwrap_or_else(|e| e.into_inner())
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
    buttons: Vec<ButtonConfig>,
    px: u32,
}

impl Snapshot {
    pub fn take(sh: &Shared) -> Self {
        Self {
            buttons: sh.cfg.page().buttons.clone(),
            px: sh.icon_px,
        }
    }

    pub fn render(&self, tx: &Sender<UsbCmd>) {
        for (index, b) in self.buttons.iter().enumerate() {
            let data = render_button_frame(b, self.px, self.px);
            let _ = tx.send(UsbCmd::Frame { index, data });
        }
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
}

impl Followup {
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
fn dispatch(sh: &mut Shared, exec: &Executor, action: ButtonAction) -> Followup {
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
        other => {
            exec.run(other);
            return Followup::default();
        }
    }
    Followup {
        nav: Nav::capture(sh),
        snapshot: Some(Snapshot::take(sh)),
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
    let action = match (&b.long_press_action, long) {
        (Some(a), true) => a.clone(),
        _ => b.action.clone(),
    };
    let toggles = b.toggled_appearance.is_some();
    if toggles {
        sh.cfg.page_mut().buttons[index].toggle_state = !b.toggle_state;
    }
    let mut followup = dispatch(sh, exec, action);
    if toggles && followup.snapshot.is_none() {
        followup.snapshot = Some(Snapshot::take(sh));
    }
    followup
}

/// No editor: consulta o serviço a cada segundo para mostrar o estado do aparelho.
pub fn spawn_status_poll(state: SharedState) {
    std::thread::spawn(move || loop {
        let reply = crate::shell::send("status");
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
        while let Ok(event) = rx.recv() {
            match event {
                UsbEvent::Info { icon_px } => lock(&state).icon_px = icon_px,
                UsbEvent::Status(connected) => {
                    let snapshot = {
                        let mut sh = lock(&state);
                        sh.connected = connected;
                        connected.then(|| Snapshot::take(&sh))
                    };
                    if let Some(snapshot) = snapshot {
                        snapshot.render(&tx);
                    }
                }
                UsbEvent::Key { index, pressed } if index < MAX_KEYS => {
                    if pressed {
                        down[index] = Some(Instant::now());
                    } else if let Some(since) = down[index].take() {
                        let followup =
                            trigger_locked(&mut lock(&state), &exec, index, since.elapsed());
                        followup.run(&tx);
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
}
