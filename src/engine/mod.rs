//! Execução das ações das teclas que saem do aparelho (teclado, programas, rede, som, OBS).
pub mod actions;

/// Runtime do tokio para o `Executor`. Falha só sem recursos do sistema (threads, epoll): quem
/// chama decide se encerra ou segue sem ações assíncronas, em vez de um pânico.
pub fn tokio_runtime(workers: usize) -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_runtime_runs_a_task() {
        let rt = super::tokio_runtime(1).expect("runtime");
        assert_eq!(rt.block_on(async { 40 + 2 }), 42);
    }
}
