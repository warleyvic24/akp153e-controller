//! Erros tipados das bordas com o mundo de fora (USB, config, áudio, IPC). Cada módulo devolve o
//! seu tipo; quem só mostra ou registra a falha usa o `Display` (mensagens em português, como o
//! resto do log). Camada de baixo: não usa editor, serviço nem CLI.
use thiserror::Error;

/// Falhas da conexão com o aparelho (thread USB).
#[derive(Debug, Error)]
pub enum UsbError {
    /// A biblioteca HID não subiu (sem permissão, sem backend).
    #[error("hidapi: {0}")]
    Init(#[source] hidapi::HidError),
    /// Nenhuma interface com VID:PID conhecido.
    #[error("dispositivo não encontrado")]
    NotFound,
    /// O modelo é conhecido, mas não tem perfil de protocolo para tentar.
    #[error("nenhum perfil de protocolo")]
    NoProfile,
    /// Leitura ou escrita no aparelho falhou (em geral: desconectado).
    #[error("HID: {0}")]
    Hid(#[from] hidapi::HidError),
}

/// Falhas ao importar um perfil exportado.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("arquivo de perfil inválido: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("não é um perfil do Keyforge (ou é de uma versão mais nova)")]
    Unsupported,
}

/// Falhas do mixer e do microfone.
#[derive(Debug, Error)]
pub enum AudioError {
    /// O programa do servidor de som (`pactl`) não pôde ser rodado ou acompanhado.
    #[cfg(not(windows))]
    #[error("pactl indisponível: {0}")]
    Spawn(#[source] std::io::Error),
    #[cfg(not(windows))]
    #[error("pactl {command}: demorou demais")]
    Timeout { command: String },
    #[cfg(not(windows))]
    #[error("pactl {command}: {stderr}")]
    Failed { command: String, stderr: String },
    /// Erro do Core Audio (COM).
    #[cfg(windows)]
    #[error("Core Audio: {0}")]
    Os(#[from] windows::core::Error),
}

/// Falhas da conversa com o serviço pela porta local.
#[derive(Debug, Error)]
pub enum ShellError {
    #[error("serviço fora do ar: {0}")]
    Connect(#[source] std::io::Error),
    #[error("conversa com o serviço: {0}")]
    Io(#[from] std::io::Error),
    #[error("resposta inválida do serviço: {0}")]
    Json(#[from] serde_json::Error),
    #[error("resposta do serviço sem texto")]
    EmptyReply,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_errors_explain_the_failure() {
        assert_eq!(UsbError::NotFound.to_string(), "dispositivo não encontrado");
        assert_eq!(
            UsbError::NoProfile.to_string(),
            "nenhum perfil de protocolo"
        );
        let hid = UsbError::from(hidapi::HidError::HidApiError {
            message: "desconectado".into(),
        });
        assert!(matches!(hid, UsbError::Hid(_)));
        assert!(hid.to_string().contains("desconectado"), "{hid}");
    }

    #[test]
    fn config_parse_errors_keep_their_source() {
        let parse = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let err = ConfigError::from(parse);
        assert!(std::error::Error::source(&err).is_some());
        assert!(err.to_string().starts_with("arquivo de perfil inválido"));
    }

    #[cfg(not(windows))]
    #[test]
    fn audio_errors_name_the_command() {
        let err = AudioError::Failed {
            command: "get-sink-mute @DEFAULT_SINK@".into(),
            stderr: "Connection refused".into(),
        };
        assert_eq!(
            err.to_string(),
            "pactl get-sink-mute @DEFAULT_SINK@: Connection refused"
        );
        let spawn = AudioError::Spawn(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(std::error::Error::source(&spawn).is_some());
    }

    #[test]
    fn shell_errors_distinguish_offline_from_bad_replies() {
        let offline = ShellError::Connect(std::io::ErrorKind::ConnectionRefused.into());
        assert!(offline.to_string().starts_with("serviço fora do ar"));
        let bad = ShellError::from(serde_json::from_str::<serde_json::Value>("x").unwrap_err());
        assert!(matches!(bad, ShellError::Json(_)));
    }
}
