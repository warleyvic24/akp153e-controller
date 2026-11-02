//! Métricas Prometheus (crate `metrics` + renderizador do `metrics-exporter-prometheus`).
//!
//! Só o serviço instala o gravador ([`install`]); sem ele as macros não fazem nada. O texto sai em
//! `metrics` na porta de controle e em `GET /metrics` (formato de exposição do Prometheus).
//! Rótulos de baixa cardinalidade apenas (tipo de quadro, tipo de erro, motivo).
use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use std::time::Duration;

pub const FRAMES_SENT: &str = "keyforge_frames_sent_total";
pub const RENDER_TIME_MS: &str = "keyforge_render_time_ms";
pub const USB_ERRORS: &str = "keyforge_usb_errors_total";
pub const RECONNECTS: &str = "keyforge_reconnects_total";
pub const COMMANDS_QUEUED: &str = "keyforge_commands_queued_total";
pub const COMMANDS_BATCH: &str = "keyforge_commands_last_batch";
pub const LOG_EVENTS: &str = "keyforge_log_events_total";
pub const CONTROL_REQUESTS: &str = "keyforge_control_requests_total";
pub const UPTIME: &str = "keyforge_uptime_seconds";
pub const CONNECTED: &str = "keyforge_device_connected";

/// Faixas do histograma de desenho (ms): uma tecla leva ~1-10 ms; a lateral, mais.
const RENDER_BUCKETS_MS: &[f64] = &[0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 250.0, 500.0];

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

fn builder() -> PrometheusBuilder {
    let base = PrometheusBuilder::new();
    match base.set_buckets_for_metric(Matcher::Full(RENDER_TIME_MS.to_string()), RENDER_BUCKETS_MS)
    {
        Ok(b) => b,
        // Só falha com lista de faixas vazia; sem faixas o histograma vira resumo.
        Err(_) => PrometheusBuilder::new(),
    }
}

/// Instala o gravador global (uma vez por processo). Falha vai ao log e as métricas ficam mudas.
pub fn install() {
    if HANDLE.get().is_some() {
        return;
    }
    match builder().install_recorder() {
        Ok(handle) => {
            let _ = HANDLE.set(handle);
            describe();
        }
        Err(e) => tracing::warn!(error = %e, "métricas desligadas"),
    }
}

fn describe() {
    describe_counter!(
        FRAMES_SENT,
        "Quadros entregues ao aparelho (tecla ou lateral)."
    );
    describe_histogram!(
        RENDER_TIME_MS,
        metrics::Unit::Milliseconds,
        "Tempo para desenhar e codificar um quadro."
    );
    describe_counter!(
        USB_ERRORS,
        "Falhas de USB (abrir ou conversar com o aparelho)."
    );
    describe_counter!(RECONNECTS, "Reconexões com o aparelho.");
    describe_counter!(COMMANDS_QUEUED, "Comandos tirados da fila da thread USB.");
    describe_gauge!(COMMANDS_BATCH, "Comandos no último lote da thread USB.");
    describe_counter!(LOG_EVENTS, "Eventos de log de aviso e erro.");
    describe_counter!(CONTROL_REQUESTS, "Pedidos atendidos na porta de controle.");
    describe_gauge!(UPTIME, metrics::Unit::Seconds, "Tempo de pé do processo.");
    describe_gauge!(CONNECTED, "1 com o aparelho conectado.");
}

/// Texto no formato de exposição do Prometheus; vazio sem [`install`].
pub fn render(connected: bool) -> String {
    let Some(handle) = HANDLE.get() else {
        return String::new();
    };
    gauge!(UPTIME).set(super::health::snapshot(connected).uptime_ms as f64 / 1000.0);
    gauge!(CONNECTED).set(f64::from(u8::from(connected)));
    handle.run_upkeep();
    handle.render()
}

/// Um quadro (`kind` = `key` ou `side`) chegou ao aparelho.
pub fn frame_sent(kind: &'static str) {
    counter!(FRAMES_SENT, "kind" => kind).increment(1);
    super::health::frame_sent();
}

/// Tempo de desenho de um quadro (`kind` = `key`, `pressed`, `mixer` ou `side`).
pub fn render_time(kind: &'static str, took: Duration) {
    histogram!(RENDER_TIME_MS, "kind" => kind).record(took.as_secs_f64() * 1000.0);
}

/// Falha de USB (`kind` = `open` ou `io`).
pub fn usb_error(kind: &'static str) {
    counter!(USB_ERRORS, "kind" => kind).increment(1);
}

/// Reconexão (`reason` = `lost` depois de cair, `requested` a pedido do serviço).
pub fn reconnect(reason: &'static str) {
    counter!(RECONNECTS, "reason" => reason).increment(1);
}

/// `n` comandos tirados da fila da thread USB num lote.
pub fn commands_queued(n: usize) {
    if n > 0 {
        counter!(COMMANDS_QUEUED).increment(n as u64);
    }
    gauge!(COMMANDS_BATCH).set(n as f64);
}

/// Um evento de log de nível `level` (`warn` ou `error`).
pub fn log_event(level: &'static str) {
    counter!(LOG_EVENTS, "level" => level).increment(1);
}

/// Um pedido `cmd` atendido na porta de controle (nomes fixos do protocolo).
pub fn control_request(cmd: &'static str) {
    counter!(CONTROL_REQUESTS, "cmd" => cmd).increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_and_histogram_render_in_prometheus_format() {
        let recorder = builder().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            frame_sent("key");
            frame_sent("key");
            render_time("key", Duration::from_millis(3));
            usb_error("io");
            reconnect("lost");
            commands_queued(4);
        });
        let text = handle.render();
        assert!(
            text.contains("keyforge_frames_sent_total{kind=\"key\"} 2"),
            "{text}"
        );
        assert!(
            text.contains("keyforge_render_time_ms_bucket{kind=\"key\",le=\"5\"} 1"),
            "{text}"
        );
        assert!(
            text.contains("keyforge_usb_errors_total{kind=\"io\"} 1"),
            "{text}"
        );
        assert!(
            text.contains("keyforge_reconnects_total{reason=\"lost\"} 1"),
            "{text}"
        );
        assert!(text.contains("keyforge_commands_queued_total 4"), "{text}");
    }

    #[test]
    fn without_a_recorder_metrics_are_silent() {
        // Testes não instalam o gravador global: as chamadas não podem falhar nem travar.
        frame_sent("side");
        usb_error("open");
    }
}
