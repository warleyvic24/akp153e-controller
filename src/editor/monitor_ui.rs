//! Editor da tecla "Monitor" (uso do sistema ao vivo): qual medida ela mostra.
use super::action_ui::PaneCtx;
use super::widgets;
use crate::config::schema::Metric;
use crate::strings::t;
use eframe::egui;

const METRICS: [Metric; 4] = [Metric::Overview, Metric::Cpu, Metric::Ram, Metric::Temp];

#[cfg(windows)]
const MONITOR_HINT: &str =
    "Atualiza a cada 2 segundos. No Windows não há leitura de temperatura: aparece \"--\".";
#[cfg(not(windows))]
const MONITOR_HINT: &str =
    "Atualiza a cada 2 segundos. Barra verde, âmbar acima de 60% e vermelha acima de 85%.";

/// Nome exibido (em português, para passar por `t`) de cada medida.
pub(crate) fn metric_label(metric: Metric) -> &'static str {
    match metric {
        Metric::Overview => "Sistema",
        Metric::Cpu => "CPU",
        Metric::Ram => "RAM",
        Metric::Temp => "Temperatura",
    }
}

pub(super) fn monitor_editor(ui: &mut egui::Ui, cx: &PaneCtx, metric: &mut Metric) -> bool {
    widgets::section(ui, "Mostrar", cx.pal);
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for m in METRICS {
            if widgets::pill(ui, metric_label(m), *metric == m, cx.accent, cx.pal).clicked() {
                *metric = m;
                changed = true;
            }
        }
    });
    ui.label(widgets::hint(t(MONITOR_HINT), cx.pal));
    changed
}
