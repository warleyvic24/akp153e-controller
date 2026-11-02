//! Assinante do `tracing`: JSON no serviço (arquivo com rotação e, num terminal, stdout), texto
//! legível (`pretty`) no editor quando há terminal. Os `log::` antigos chegam pela ponte
//! `tracing-log`. Filtro por `RUST_LOG` (padrão `warn,keyforge=info`).
use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::path::Path;
use std::sync::Mutex;

use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Tamanho a partir do qual o log gira (o evento que passa do limite fica inteiro no arquivo).
pub const LOG_MAX_BYTES: usize = 10 * 1024 * 1024;
/// Arquivos guardados no total: o atual mais 4 antigos (`.1` a `.4`).
pub const LOG_FILES: usize = 5;
const DEFAULT_FILTER: &str = "warn,keyforge=info";
/// Porta padrão do servidor do puffin (o editor usa a seguinte, para os dois rodarem juntos).
const PUFFIN_PORT: u16 = 8585;

/// Quem está registrando: muda o formato, o arquivo e se há métricas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Serviço: JSON, `keyforge.log`, métricas Prometheus.
    Service,
    /// Editor: JSON em `keyforge-editor.log` e `pretty` no terminal.
    Editor,
    /// Comandos de linha (`--run-action`, `--selftest`...): como o editor.
    Cli,
}

impl Mode {
    /// Cada processo tem o seu arquivo: dois processos girando o mesmo arquivo se atropelam.
    pub fn file_name(self) -> &'static str {
        match self {
            Mode::Service => "keyforge.log",
            Mode::Editor | Mode::Cli => "keyforge-editor.log",
        }
    }
}

/// Abre o log com rotação por tamanho (10 MiB, 5 arquivos). No Unix só o dono lê (0600): o log
/// pode ter nomes de apps e caminhos.
fn open_rotating(path: &Path, max_bytes: usize) -> Option<FileRotate<AppendCount>> {
    std::fs::create_dir_all(path.parent()?).ok()?;
    let mut opts = OpenOptions::new();
    opts.read(true).create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    // Confere antes: `FileRotate::new` não devolve erro de abertura.
    opts.open(path).ok()?;
    Some(FileRotate::new(
        path,
        AppendCount::new(LOG_FILES - 1),
        ContentLimit::BytesSurpassed(max_bytes),
        Compression::None,
        Some(opts),
    ))
}

/// Instala o assinante global (uma vez; chamadas seguintes não fazem nada). `dir` é a pasta do
/// log; sem ela, o serviço escreve JSON no stdout.
pub fn init(mode: Mode, dir: Option<&Path>) {
    super::health::mark_start();
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let file = dir.and_then(|d| open_rotating(&d.join(mode.file_name()), LOG_MAX_BYTES));
    let has_file = file.is_some();
    let file_layer = file.map(|f| {
        fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_thread_names(true)
            .with_ansi(false)
            .with_writer(Mutex::new(f))
    });
    let service = mode == Mode::Service;
    let stdout_json = (service
        && (!has_file
            || std::io::stdout().is_terminal()
            || std::env::var_os("KEYFORGE_LOG_STDOUT").is_some()))
    .then(|| {
        fmt::layer()
            .json()
            .with_current_span(true)
            .with_thread_names(true)
            .with_writer(std::io::stdout)
    });
    let pretty = (!service && std::io::stderr().is_terminal()).then(|| {
        fmt::layer()
            .pretty()
            .with_thread_names(true)
            .with_writer(std::io::stderr)
    });
    let installed = tracing_subscriber::registry()
        .with(filter)
        .with(ErrorLayer)
        .with(file_layer)
        .with(stdout_json)
        .with(pretty)
        .try_init();
    if let Err(e) = installed {
        eprintln!("log: assinante não instalado: {e}");
    }
    if service {
        super::metrics::install();
        super::start_profiler(PUFFIN_PORT);
    } else {
        super::start_profiler(PUFFIN_PORT + 1);
    }
    tracing::debug!(?mode, file = has_file, "log iniciado");
}

/// Conta avisos/erros e guarda o último erro para `/health`.
struct ErrorLayer;

impl<S: Subscriber> Layer<S> for ErrorLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        match *meta.level() {
            Level::ERROR => {
                super::metrics::log_event("error");
                let mut msg = MessageVisitor::default();
                event.record(&mut msg);
                super::health::record_error(meta.target(), &msg.0);
            }
            Level::WARN => super::metrics::log_event("warn"),
            _ => {}
        }
    }
}

/// Junta a mensagem e os campos de um evento num texto só.
#[derive(Default)]
struct MessageVisitor(String);

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.push(field, &format!("{value:?}"));
    }
}

impl MessageVisitor {
    fn push(&mut self, field: &Field, value: &str) {
        // Campos da ponte do `log` (módulo, arquivo, linha) não ajudam no resumo do erro.
        if field.name().starts_with("log.") {
            return;
        }
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        if field.name() != "message" {
            self.0.push_str(field.name());
            self.0.push('=');
        }
        self.0.push_str(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn the_log_rotates_by_size_and_keeps_five_files() {
        let dir = std::env::temp_dir().join(format!("kf-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("keyforge.log");
        // Mesma regra com um limite pequeno (o real é 10 MiB).
        const MAX: usize = 1000;
        let mut f = open_rotating(&path, MAX).expect("abre");
        let line = vec![b'x'; MAX / 2];
        for _ in 0..16 {
            f.write_all(&line).unwrap();
            f.write_all(b"\n").unwrap();
        }
        f.flush().unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names.len(), LOG_FILES, "{names:?}");
        for n in &names {
            let len = std::fs::metadata(dir.join(n)).unwrap().len() as usize;
            assert!(len <= MAX + line.len() + 1, "{n}: {len}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_visitor_keeps_message_and_fields() {
        struct Capture(std::sync::Arc<Mutex<String>>);
        impl<S: Subscriber> Layer<S> for Capture {
            fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
                let mut v = MessageVisitor::default();
                event.record(&mut v);
                *crate::sync::lock(&self.0) = v.0;
            }
        }
        let seen = std::sync::Arc::new(Mutex::new(String::new()));
        let sub = tracing_subscriber::registry().with(Capture(seen.clone()));
        tracing::subscriber::with_default(sub, || {
            tracing::error!(key = 3, "falhou");
        });
        assert_eq!(*crate::sync::lock(&seen), "falhou key=3");
    }

    #[test]
    fn production_limits_are_ten_mib_and_five_files() {
        assert_eq!(LOG_MAX_BYTES, 10 * 1024 * 1024);
        assert_eq!(LOG_FILES, 5);
    }

    #[test]
    fn service_and_editor_write_different_files() {
        assert_ne!(Mode::Service.file_name(), Mode::Editor.file_name());
        assert_eq!(Mode::Cli.file_name(), Mode::Editor.file_name());
    }
}
