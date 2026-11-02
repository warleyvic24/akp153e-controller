//! Observabilidade: logs estruturados (`tracing`), métricas Prometheus, saúde (`/health`),
//! threads com nome e o perfilador opcional (feature `profiling`, puffin).
//!
//! Camada de baixo: não usa editor, serviço nem CLI (só a pasta da config, para achar o log).
pub mod health;
mod logging;
pub mod metrics;

pub use logging::{init, Mode};

use std::thread::JoinHandle;

/// Cria uma thread com nome (aparece no `top -H`, no gdb e no campo `threadName` do log).
/// `None` se o sistema recusar a thread; a falha vai para o log.
pub fn spawn_named<F, T>(name: &str, f: F) -> Option<JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(f)
        .inspect_err(
            |e| tracing::error!(thread = name, error = %e, "não foi possível criar a thread"),
        )
        .ok()
}

/// Thread de vida longa com nome, registrada na saúde: `/health` mostra se ela ainda está de pé
/// e, se ela chama [`health::beat`], há quanto tempo deu sinal.
pub fn spawn_tracked<F>(name: &'static str, f: F)
where
    F: FnOnce() + Send + 'static,
{
    spawn_named(name, move || {
        let _alive = health::register(name);
        f();
    });
}

/// Escopo do perfilador puffin; sem a feature `profiling` não gera código nenhum.
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        #[cfg(feature = "profiling")]
        puffin::profile_scope!($name);
    };
}

/// Fecha um quadro do perfilador (laço do USB no serviço, `ui` no editor). Nada sem a feature.
#[inline]
pub fn profile_frame() {
    #[cfg(feature = "profiling")]
    puffin::GlobalProfiler::lock().new_frame();
}

/// Com a feature `profiling` e `KEYFORGE_PUFFIN` definido, liga os escopos e abre o servidor do
/// puffin_viewer (no endereço da variável, ou `127.0.0.1:<porta padrão>` se ela for `1`).
#[cfg(feature = "profiling")]
fn start_profiler(default_port: u16) {
    use std::sync::OnceLock;
    static SERVER: OnceLock<puffin_http::Server> = OnceLock::new();
    let Some(var) = std::env::var_os("KEYFORGE_PUFFIN") else {
        return;
    };
    let var = var.to_string_lossy();
    let addr = if var.contains(':') {
        var.into_owned()
    } else {
        format!("127.0.0.1:{default_port}")
    };
    puffin::set_scopes_on(true);
    match puffin_http::Server::new(&addr) {
        Ok(server) => {
            let _ = SERVER.set(server);
            tracing::info!(%addr, "puffin: servidor do perfilador aberto");
        }
        Err(e) => tracing::warn!(%addr, error = %e, "puffin: servidor não abriu"),
    }
}

#[cfg(not(feature = "profiling"))]
fn start_profiler(_default_port: u16) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_threads_carry_their_name() {
        let h = spawn_named("kf-test-name", || {
            std::thread::current().name().map(str::to_string)
        })
        .expect("thread");
        assert_eq!(h.join().unwrap().as_deref(), Some("kf-test-name"));
    }

    #[test]
    fn profile_scope_compiles_away_without_the_feature() {
        crate::profile_scope!("teste");
        profile_frame();
    }
}
