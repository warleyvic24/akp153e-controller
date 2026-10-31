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
        service_running: true,
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

#[tokio::test]
async fn a_long_press_with_its_own_action_does_not_flip_a_two_state_key() {
    let (tx, _rx) = channel();
    let exec = Executor::new(tokio::runtime::Handle::current());
    let mut sh = two_page_state(
        ButtonAction::None,
        Some(ButtonAction::SwitchPage { page: 2 }),
    );
    sh.cfg.page_mut().buttons[0].toggled_appearance = Some(ButtonConfig::blank().appearance);
    trigger(&mut sh, &tx, &exec, 0, Duration::from_secs(1));
    assert_eq!(sh.cfg.profiles[0].active_page, 1, "a ação longa rodou");
    assert!(!sh.cfg.profiles[0].pages[0].buttons[0].toggle_state);
    // Sem ação longa, segurar ainda conta como toque e alterna.
    sh.cfg.profiles[0].active_page = 0;
    sh.cfg.page_mut().buttons[0].long_press_action = None;
    trigger(&mut sh, &tx, &exec, 0, Duration::from_secs(1));
    assert!(sh.cfg.page().buttons[0].toggle_state);
}

#[tokio::test]
async fn a_key_press_waits_for_a_drawing_in_progress() {
    let (tx, rx) = channel();
    let exec = Executor::new(tokio::runtime::Handle::current());
    let state = Arc::new(Mutex::new(two_page_state(
        ButtonAction::SwitchPage { page: 2 },
        None,
    )));
    let (pressed_tx, pressed_rx) = channel();
    let pressed = std::thread::spawn({
        let (state, tx) = (state.clone(), tx.clone());
        move || {
            // Espera o desenho abaixo já estar em andamento.
            pressed_rx.recv().unwrap();
            press(&state, &tx, &exec, 0, Duration::ZERO)
        }
    });
    {
        // Um desenho em andamento já soltou o estado, mas ainda segura a ordem dos quadros.
        let _drawing = DRAW_ORDER.lock().unwrap();
        pressed_tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            lock(&state).cfg.profiles[0].active_page,
            0,
            "o aperto não pode entrar no meio do desenho"
        );
    }
    assert_eq!(pressed.join().unwrap(), None);
    assert_eq!(lock(&state).cfg.profiles[0].active_page, 1);
    assert_eq!(rx.try_iter().count(), 15);
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
