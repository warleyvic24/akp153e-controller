//! Gravação da config fora da thread da janela.
//!
//! Antes, cada salvamento automático rodava no quadro do egui, com o estado compartilhado travado:
//! reler o arquivo, gravar com `fsync` e esperar o serviço responder ao "reload" (até 400 ms).
//! Agora o quadro só copia a config e manda para a thread de gravação; o aviso de "reload" vai
//! por outro canal, para uma thread própria, e nunca segura a próxima gravação.
use crate::config::schema::AppConfig;
use crate::platform::shell;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

/// Um pedido de gravação. `nav` diz se este editor mudou perfil/página (senão o disco manda).
pub(super) struct Job {
    pub cfg: AppConfig,
    pub nav: bool,
}

/// Resultado de uma gravação: `nav` do pedido (para refazer se falhou) e o erro, se houve.
pub(super) struct Done {
    pub nav: bool,
    pub result: Result<(), String>,
}

/// Lê o disco (sob a trava entre processos) e grava `cfg`; sem `nav`, mantém o perfil/página que
/// o arquivo já tem: o aparelho pode ter trocado de página nesse meio-tempo.
pub(super) fn write(cfg: AppConfig, nav: bool) -> std::io::Result<()> {
    let _span = tracing::info_span!("save", what = "config", nav).entered();
    crate::profile_scope!("save");
    let mut out = cfg;
    AppConfig::locked(|| {
        if !nav {
            if let Some(disk) = AppConfig::load_existing() {
                out.adopt_active_from(&disk);
            }
        }
        out.save()
    })
}

pub(super) struct Saver {
    jobs: Option<Sender<Job>>,
    done: Receiver<Done>,
    worker: Option<JoinHandle<()>>,
    /// Pedidos mandados e ainda sem resposta.
    in_flight: usize,
}

impl Saver {
    /// `notify` roda depois de cada gravação bem-sucedida (no editor: o "reload" do serviço).
    pub(super) fn start(notify: impl Fn() + Send + 'static) -> Self {
        let (jobs, inbox) = channel::<Job>();
        let (outbox, done) = channel::<Done>();
        let (reload_tx, reload_rx) = channel::<()>();
        // Avisos de "reload" em rajada viram um só.
        std::thread::spawn(move || {
            while reload_rx.recv().is_ok() {
                while reload_rx.try_recv().is_ok() {}
                notify();
            }
        });
        let worker = std::thread::spawn(move || {
            while let Ok(first) = inbox.recv() {
                // Várias edições em sequência: só a config mais nova importa; a navegação
                // vale se qualquer um dos pedidos a trazia.
                let mut skipped = 0usize;
                let mut job = first;
                for newer in inbox.try_iter() {
                    let nav = job.nav || newer.nav;
                    job = Job { nav, ..newer };
                    skipped += 1;
                }
                let nav = job.nav;
                let result = write(job.cfg, nav).map_err(|e| e.to_string());
                match &result {
                    Ok(()) => {
                        let _ = reload_tx.send(());
                    }
                    Err(e) => log::error!("salvar: {e}"),
                }
                // Um resultado por pedido, para a contagem de pendentes fechar.
                for _ in 0..skipped {
                    let _ = outbox.send(Done {
                        nav,
                        result: result.clone(),
                    });
                }
                if outbox.send(Done { nav, result }).is_err() {
                    return;
                }
            }
        });
        Self {
            jobs: Some(jobs),
            done,
            worker: Some(worker),
            in_flight: 0,
        }
    }

    /// O editor de verdade: avisa o serviço depois de gravar.
    pub(super) fn for_editor() -> Self {
        Self::start(|| {
            let _ = shell::send("reload");
        })
    }

    /// Manda gravar `cfg` sem esperar. `false` se a thread de gravação não existe mais.
    pub(super) fn submit(&mut self, cfg: AppConfig, nav: bool) -> bool {
        let sent = self
            .jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(Job { cfg, nav }).is_ok());
        if sent {
            self.in_flight += 1;
        }
        sent
    }

    /// Resultados prontos desde a última chamada.
    pub(super) fn poll(&mut self) -> Vec<Done> {
        let done: Vec<Done> = self.done.try_iter().collect();
        self.in_flight = self.in_flight.saturating_sub(done.len());
        done
    }

    /// Há gravação pedida e ainda não concluída.
    pub(super) fn busy(&self) -> bool {
        self.in_flight > 0
    }

    /// Espera as gravações pendentes terminarem (ao fechar o editor).
    pub(super) fn finish(&mut self) -> Vec<Done> {
        self.jobs = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.poll()
    }
}

impl Drop for Saver {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// Nos testes `AppConfig::path` aponta para uma pasta temporária: a config real fica intacta.
    fn saver_counting() -> (Saver, Arc<AtomicUsize>) {
        let reloads = Arc::new(AtomicUsize::new(0));
        let seen = reloads.clone();
        let saver = Saver::start(move || {
            // Simula o serviço lento a responder (o `IPC_TIMEOUT` é 400 ms).
            std::thread::sleep(Duration::from_millis(200));
            seen.fetch_add(1, Ordering::SeqCst);
        });
        (saver, reloads)
    }

    #[test]
    fn submitting_a_save_does_not_block_the_caller() {
        let (mut saver, _) = saver_counting();
        let cfg = AppConfig::default();
        let started = Instant::now();
        for _ in 0..20 {
            assert!(saver.submit(cfg.clone(), false));
        }
        let took = started.elapsed();
        assert!(
            took < Duration::from_millis(20),
            "20 pedidos levaram {took:?} na thread da janela"
        );
        assert!(saver.busy());
        let done = saver.finish();
        assert_eq!(done.len(), 20, "um resultado por pedido");
        assert!(!saver.busy());
    }

    #[test]
    fn a_save_with_fsync_takes_under_100ms() {
        let cfg = AppConfig::default();
        write(cfg.clone(), true).unwrap(); // cria a pasta e o arquivo
        let started = Instant::now();
        write(cfg, false).unwrap();
        let took = started.elapsed();
        eprintln!("gravar com fsync: {took:?}");
        assert!(took < Duration::from_millis(100), "gravar levou {took:?}");
    }
}
