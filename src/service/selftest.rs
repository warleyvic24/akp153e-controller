//! Autoteste guiado do aparelho (`keyforge --selftest` ou Preferências > Testar aparelho). O
//! aparelho pede cada tecla em ordem ("APERTE"), depois uma pressão longa, e pergunta (SIM/NÃO nas
//! teclas) se o brilho mudou e se a tela lateral mostra as 3 faixas. Revela tecla trocada, soltura
//! que não chega, brilho e lateral que não respondem. Nenhuma ação roda durante o teste; ao fim o
//! resultado vai para `selftest.txt` ao lado da config e a página volta.
//!
//! A lógica (`Run`) é pura e testada; o resto só desenha e grava.
use super::runtime::{self, SharedState};
use crate::config::schema::{AppConfig, ButtonConfig, RGBAColor};
use crate::graphics::renderer::{encode_frame, render_rgba};
use crate::strings::{t, tf};
use crate::usb::{SideFrame, UsbCmd};
use image::{Rgba, RgbaImage};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Pressão longa que conta como "segurou" (folga sobre o limite de 500 ms das ações).
const HOLD_MIN: Duration = Duration::from_millis(800);
/// Brilho da pergunta "ficou escuro?": baixo o bastante para ninguém ter dúvida.
const DIM_LEVEL: u8 = 10;
/// Sem nenhum toque por este tempo, o teste desiste e devolve a página.
const IDLE_LIMIT: Duration = Duration::from_secs(120);
/// O resumo fica na tela por este tempo antes de a página voltar.
const SUMMARY_TIME: Duration = Duration::from_secs(6);
/// Tecla da resposta "NÃO": a última da primeira fileira de 5 (a de "SIM" é a primeira).
const NO_KEY: usize = 4;
const QUESTION_KEY: usize = 2;
const LABEL_FONT: f32 = 22.0;
const REPORT_FILE: &str = "selftest.txt";

const PENDING: RGBAColor = RGBAColor {
    r: 40,
    g: 40,
    b: 46,
    a: 255,
};
const TARGET: RGBAColor = RGBAColor {
    r: 30,
    g: 110,
    b: 220,
    a: 255,
};
const PASS: RGBAColor = RGBAColor {
    r: 30,
    g: 140,
    b: 70,
    a: 255,
};
const FAIL: RGBAColor = RGBAColor {
    r: 180,
    g: 40,
    b: 40,
    a: 255,
};
const SIDE_BANDS: [[u8; 3]; 3] = [[200, 50, 50], [40, 160, 70], [50, 100, 220]];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Press(usize),
    Hold,
    AskBrightness,
    AskSide,
    Done,
}

/// O que o passo atual pede ao aparelho, além de redesenhar as teclas.
#[derive(Debug, PartialEq)]
enum Effect {
    Draw,
    Brightness(u8),
    SidePattern,
    Finish,
}

struct Run {
    keys: usize,
    has_side: bool,
    step: Step,
    /// Tecla e hora do último aperto ainda não solto.
    down: Option<(usize, Instant)>,
    /// Resultado de cada tecla no passo "APERTE": `None` = ainda não, `Some(ok)`.
    pressed: Vec<Option<bool>>,
    report: Vec<String>,
    failures: usize,
    last_touch: Instant,
}

impl Run {
    fn new(keys: usize, has_side: bool, now: Instant) -> Self {
        Self {
            keys,
            has_side,
            step: Step::Press(0),
            down: None,
            pressed: vec![None; keys],
            report: Vec::new(),
            failures: 0,
            last_touch: now,
        }
    }

    fn fail(&mut self, line: String) {
        self.failures += 1;
        self.report.push(format!("FALHA: {line}"));
    }

    fn next_after_press(&self, i: usize) -> Step {
        if i + 1 < self.keys {
            Step::Press(i + 1)
        } else {
            Step::Hold
        }
    }

    /// Resposta SIM/NÃO; outra tecla é ignorada.
    fn answer(index: usize) -> Option<bool> {
        match index {
            0 => Some(true),
            NO_KEY => Some(false),
            _ => None,
        }
    }

    fn handle(&mut self, index: usize, pressed: bool, now: Instant) -> Vec<Effect> {
        self.last_touch = now;
        if pressed {
            self.down = Some((index, now));
            return Vec::new();
        }
        let Some((down_index, since)) = self.down.take() else {
            return Vec::new();
        };
        if down_index != index {
            return Vec::new();
        }
        let held = now.saturating_duration_since(since);
        match self.step {
            Step::Press(want) => {
                if index == want {
                    self.pressed[want] = Some(true);
                    self.report.push(format!("ok: tecla {} chegou", want + 1));
                } else {
                    // O aparelho mandou outro número: o mapa de teclas do modelo está errado.
                    self.pressed[want] = Some(false);
                    self.fail(format!(
                        "tecla {} apertada chegou como {}",
                        want + 1,
                        index + 1
                    ));
                }
                self.step = self.next_after_press(want);
                vec![Effect::Draw]
            }
            Step::Hold if index == 0 => {
                let ms = held.as_millis();
                if held >= HOLD_MIN {
                    self.report.push(format!("ok: pressão longa de {ms} ms"));
                } else {
                    self.fail(format!(
                        "pressão longa durou {ms} ms (a soltura chegou cedo ou não é reportada)"
                    ));
                }
                self.step = Step::AskBrightness;
                vec![Effect::Brightness(DIM_LEVEL), Effect::Draw]
            }
            Step::AskBrightness => match Self::answer(index) {
                Some(yes) => {
                    if yes {
                        self.report.push("ok: brilho mudou".into());
                    } else {
                        self.fail("brilho não mudou".into());
                    }
                    // `Brightness(0)` = devolver o brilho da config (quem executa sabe qual é).
                    let mut effects = vec![Effect::Brightness(0)];
                    if self.has_side {
                        self.step = Step::AskSide;
                        effects.push(Effect::SidePattern);
                    } else {
                        self.step = Step::Done;
                        effects.push(Effect::Finish);
                    }
                    effects.push(Effect::Draw);
                    effects
                }
                None => Vec::new(),
            },
            Step::AskSide => match Self::answer(index) {
                Some(yes) => {
                    if yes {
                        self.report.push("ok: tela lateral com as 3 faixas".into());
                    } else {
                        self.fail("tela lateral não mostrou as 3 faixas".into());
                    }
                    self.step = Step::Done;
                    vec![Effect::Finish, Effect::Draw]
                }
                None => Vec::new(),
            },
            Step::Hold | Step::Done => Vec::new(),
        }
    }

    /// Texto e cor de cada tecla no passo atual.
    fn keys_view(&self) -> Vec<(String, RGBAColor)> {
        (0..self.keys)
            .map(|i| match self.step {
                Step::Press(want) if i == want => (tf("{}\nAPERTE", &[&(i + 1)]), TARGET),
                Step::Press(_) | Step::Done => match self.pressed[i] {
                    Some(true) => (format!("{}\nOK", i + 1), PASS),
                    Some(false) => (format!("{}\n{}", i + 1, t("ERRO")), FAIL),
                    None => ((i + 1).to_string(), PENDING),
                },
                Step::Hold if i == 0 => (t("SEGURE\n1 s").to_string(), TARGET),
                Step::AskBrightness | Step::AskSide if i == 0 => (t("SIM").to_string(), PASS),
                Step::AskBrightness | Step::AskSide if i == NO_KEY => (t("NÃO").to_string(), FAIL),
                Step::AskBrightness if i == QUESTION_KEY => {
                    (t("Ficou\nescuro?").to_string(), PENDING)
                }
                Step::AskSide if i == QUESTION_KEY => (t("Lateral\n3 cores?").to_string(), PENDING),
                _ => (String::new(), PENDING),
            })
            .collect()
    }
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);

fn run_lock() -> std::sync::MutexGuard<'static, Option<Run>> {
    RUN.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Há um autoteste na tela: os desenhos normais esperam (`Snapshot::render`).
pub fn running() -> bool {
    run_lock().is_some()
}

/// Começa o autoteste; devolve o que responder a quem pediu.
pub fn start(state: &SharedState, tx: &Sender<UsbCmd>) -> &'static str {
    let (connected, keys, has_side) = {
        let sh = runtime::lock(state);
        let model = crate::device::catalog()
            .iter()
            .find(|m| m.id == sh.cfg.page().model);
        let keys = model.map_or(sh.cfg.page().buttons.len(), |m| m.keys().len());
        (
            sh.connected,
            keys,
            model.is_some_and(|m| m.strip().is_some()),
        )
    };
    if !connected {
        return "sem-aparelho";
    }
    if keys <= NO_KEY {
        return "aparelho-pequeno";
    }
    {
        let mut run = run_lock();
        if run.is_some() {
            return "em-andamento";
        }
        *run = Some(Run::new(keys, has_side, Instant::now()));
    }
    log::info!("autoteste do aparelho iniciado ({keys} teclas)");
    draw(state, tx);
    spawn_watchdog(state.clone(), tx.clone());
    "ok"
}

/// Tecla durante o autoteste: `true` se ela foi usada por ele (a ação da tecla não roda).
pub fn on_key(state: &SharedState, tx: &Sender<UsbCmd>, index: usize, pressed: bool) -> bool {
    let effects = {
        let mut run = run_lock();
        let Some(run) = run.as_mut() else {
            return false;
        };
        run.handle(index, pressed, Instant::now())
    };
    for effect in effects {
        match effect {
            Effect::Draw => draw(state, tx),
            Effect::Brightness(0) => {
                let level = runtime::lock(state).cfg.ui.brightness;
                let _ = tx.send(UsbCmd::Brightness(level));
            }
            Effect::Brightness(level) => {
                let _ = tx.send(UsbCmd::Brightness(level));
            }
            Effect::SidePattern => {
                let _ = tx.send(UsbCmd::Side(SideFrame::Strips(
                    crate::graphics::side::strips(&side_pattern()),
                )));
            }
            Effect::Finish => finish(state, tx),
        }
    }
    true
}

fn draw(state: &SharedState, tx: &Sender<UsbCmd>) {
    let Some(view) = run_lock().as_ref().map(Run::keys_view) else {
        return;
    };
    let px = runtime::lock(state).icon_px;
    for (index, (text, background)) in view.into_iter().enumerate() {
        let mut b = ButtonConfig::blank();
        b.appearance.text = text;
        b.appearance.background = background;
        b.appearance.font_size = LABEL_FONT;
        let data = encode_frame(&render_rgba(&b, px, px));
        let _ = tx.send(UsbCmd::Frame { index, data });
    }
}

/// Três faixas de cores com o número de cada uma, no tamanho da tela lateral do AKP153E.
fn side_pattern() -> RgbaImage {
    let (w, h) = (crate::graphics::side::WIDTH, crate::graphics::side::HEIGHT);
    RgbaImage::from_fn(w, h, |_, y| {
        let [r, g, b] = SIDE_BANDS[(y * 3 / h).min(2) as usize];
        Rgba([r, g, b, 255])
    })
}

/// Grava o relatório e, depois do resumo, devolve a página.
fn finish(state: &SharedState, tx: &Sender<UsbCmd>) {
    let Some((failures, report)) = run_lock().as_ref().map(|r| (r.failures, r.report.clone()))
    else {
        return;
    };
    let mut text = format!(
        "Autoteste do aparelho, {}\n{} falha(s)\n\n",
        jiff::Zoned::now().strftime("%Y-%m-%d %H:%M"),
        failures
    );
    for line in &report {
        text.push_str(line);
        text.push('\n');
    }
    log::info!("autoteste terminado: {failures} falha(s)");
    if let Some(path) = report_path() {
        if let Err(e) = std::fs::write(&path, &text) {
            log::warn!("autoteste: não foi possível gravar {}: {e}", path.display());
        }
    }
    let state = state.clone();
    let tx = tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SUMMARY_TIME);
        restore(&state, &tx);
    });
}

fn report_path() -> Option<std::path::PathBuf> {
    AppConfig::path().map(|p| p.with_file_name(REPORT_FILE))
}

/// Encerra o teste e redesenha a página inteira (e a lateral) como estava.
fn restore(state: &SharedState, tx: &Sender<UsbCmd>) {
    if run_lock().take().is_none() {
        return;
    }
    let level = runtime::lock(state).cfg.ui.brightness;
    let _ = tx.send(UsbCmd::Brightness(level));
    runtime::forget_mixer_drawing(state);
    runtime::change_and_draw(state, tx, |_| ((), true));
}

/// Desiste se ninguém tocar no aparelho por `IDLE_LIMIT` (a página não fica presa no teste).
fn spawn_watchdog(state: SharedState, tx: Sender<UsbCmd>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let idle = match run_lock().as_ref() {
            None => return,
            Some(r) => r.step != Step::Done && r.last_touch.elapsed() >= IDLE_LIMIT,
        };
        if idle {
            log::info!(
                "autoteste abandonado (sem toque por {} s)",
                IDLE_LIMIT.as_secs()
            );
            restore(&state, &tx);
            return;
        }
    });
}

#[cfg(test)]
mod tests;
