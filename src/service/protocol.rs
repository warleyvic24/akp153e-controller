//! Protocolo da porta de controle (editor/CLI → serviço): uma linha por conexão, resposta em
//! JSON versionado (`version` + `resp`).
//!
//! O pedido chega de duas formas, ambas normalizadas por [`RequestEnvelope::parse`] num
//! [`ControlRequest`] tipado:
//! - JSON v1: `{"version":1,"cmd":"brightness","level":40}`;
//! - texto (o formato antigo, ainda usado por `platform::shell::send`): `brightness 40`, cru ou
//!   embrulhado em `{"version":1,"cmd":"text","line":"brightness 40"}`.
//!
//! Depois de `parse`, um [`ControlRequest::Text`] é sempre um comando desconhecido: o servidor
//! tem um único caminho de despacho, sem duplicar a lógica do texto antigo.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Versão atual do protocolo JSON. Incrementar em mudanças incompatíveis.
pub(super) const PROTOCOL_VERSION: u32 = 1;

/// Pedido ao serviço.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub(super) enum ControlRequest {
    /// Abre (ou traz para a frente) a janela do editor.
    Show,
    /// Reabre a conexão USB com o aparelho.
    Reconnect,
    /// Relê a config do disco e redesenha o aparelho.
    Reload,
    /// Estado do OBS (conexão, cenas, entradas) em JSON.
    ObsInfo,
    /// Executa um comando no OBS (`ObsCommand` em kebab-case) sobre `target`, se houver.
    ObsRun {
        command: String,
        #[serde(default)]
        target: String,
    },
    /// Apps que estiveram em foco (para a troca automática de perfil).
    Apps,
    /// Brilho ao vivo (0 a `usb::MAX_BRIGHTNESS`); validado no despacho.
    Brightness { level: u8 },
    /// Inicia o autoteste guiado no aparelho.
    Selftest,
    /// Conexão do aparelho e tamanho do ícone.
    Status,
    /// Desenha a grade de calibração; `None` = varredura de tamanhos.
    Calib {
        #[serde(default)]
        px: Option<u32>,
    },
    /// Desenha o padrão de teste; `None` = varredura de tamanhos.
    Pattern {
        #[serde(default)]
        px: Option<u32>,
    },
    /// Linha de texto do formato antigo. Depois de [`RequestEnvelope::parse`] só sobra se o
    /// texto não for um comando conhecido.
    Text { line: String },
}

/// Resposta do serviço.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "resp", rename_all = "kebab-case")]
pub(super) enum ControlResponse {
    /// Pedido aceito.
    Ok,
    /// Pedido recusado, com o motivo.
    Error { message: String },
    /// Estado do OBS (os campos do objeto vão no nível de cima da resposta).
    ObsInfo(Value),
    /// Apps recentes, separados por tabulação.
    Apps { apps: String },
    /// Aparelho conectado e tamanho do ícone em px.
    Status { connected: bool, icon_px: u32 },
    /// Resultado do pedido de autoteste (`ok` = iniciou).
    Selftest { ok: bool, message: String },
}

/// Pedido com a versão do protocolo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct RequestEnvelope {
    pub version: u32,
    #[serde(flatten)]
    pub request: ControlRequest,
}

/// Resposta com a versão do protocolo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct ResponseEnvelope {
    pub version: u32,
    #[serde(flatten)]
    pub response: ControlResponse,
}

impl RequestEnvelope {
    /// Lê um pedido em JSON v1 ou em texto. Texto (cru ou no campo `line`) vira o pedido tipado
    /// equivalente; sem equivalente, fica `ControlRequest::Text`.
    pub fn parse(input: &str) -> Result<Self, ProtocolError> {
        let input = input.trim();
        if !input.starts_with('{') {
            return Ok(Self::current(legacy_to_request(input)));
        }
        let env: Self =
            serde_json::from_str(input).map_err(|e| ProtocolError::Json(e.to_string()))?;
        if env.version > PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got: env.version,
            });
        }
        Ok(match env.request {
            ControlRequest::Text { line } => Self::current(legacy_to_request(line.trim())),
            _ => env,
        })
    }

    fn current(request: ControlRequest) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request,
        }
    }
}

impl ResponseEnvelope {
    pub fn new(response: ControlResponse) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            response,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(ControlResponse::Error {
            message: message.into(),
        })
    }

    /// JSON compacto de uma linha. Nunca falha: se a resposta não serializar, vira um erro.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            serde_json::json!({
                "version": self.version,
                "resp": "error",
                "message": format!("falha ao serializar resposta: {e}"),
            })
            .to_string()
        })
    }
}

/// Converte uma linha do formato de texto no pedido tipado. Argumentos que não se convertem
/// (ex.: `calib abc`) deixam o pedido como `Text`, isto é, desconhecido.
fn legacy_to_request(line: &str) -> ControlRequest {
    let (name, arg) = line
        .split_once(' ')
        .map_or((line, ""), |(n, a)| (n, a.trim()));
    let request = match (name, arg) {
        ("show", "") => Some(ControlRequest::Show),
        ("reconnect", "") => Some(ControlRequest::Reconnect),
        ("reload", "") => Some(ControlRequest::Reload),
        ("obs-info", "") => Some(ControlRequest::ObsInfo),
        ("apps", "") => Some(ControlRequest::Apps),
        ("selftest", "") => Some(ControlRequest::Selftest),
        ("status", "") => Some(ControlRequest::Status),
        ("obs-run", rest) if !rest.is_empty() => {
            let (command, target) = rest.split_once(' ').unwrap_or((rest, ""));
            Some(ControlRequest::ObsRun {
                command: command.to_string(),
                target: target.trim().to_string(),
            })
        }
        ("brightness", level) => level
            .parse()
            .ok()
            .map(|level| ControlRequest::Brightness { level }),
        ("calib", px) => optional_px(px).map(|px| ControlRequest::Calib { px }),
        ("pattern", px) => optional_px(px).map(|px| ControlRequest::Pattern { px }),
        _ => None,
    };
    request.unwrap_or_else(|| ControlRequest::Text {
        line: line.to_string(),
    })
}

/// Argumento opcional de tamanho: vazio = `Some(None)`; número = `Some(Some(px))`; resto = `None`.
fn optional_px(arg: &str) -> Option<Option<u32>> {
    if arg.is_empty() {
        Some(None)
    } else {
        arg.parse().ok().map(Some)
    }
}

/// Pedido que não pôde ser lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProtocolError {
    /// JSON malformado ou com campos inválidos.
    Json(String),
    /// Pedido de uma versão do protocolo mais nova que este serviço.
    VersionMismatch { expected: u32, got: u32 },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::Json(e) => write!(f, "JSON inválido: {e}"),
            ProtocolError::VersionMismatch { expected, got } => write!(
                f,
                "versão de protocolo não suportada: esperado <= {expected}, recebido {got}"
            ),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> ControlRequest {
        let env = RequestEnvelope::parse(input).unwrap();
        assert_eq!(env.version, PROTOCOL_VERSION);
        env.request
    }

    #[test]
    fn typed_json_round_trips() {
        let env = RequestEnvelope::current(ControlRequest::Brightness { level: 40 });
        let json = serde_json::to_string(&env).unwrap();
        assert_eq!(json, r#"{"version":1,"cmd":"brightness","level":40}"#);
        assert_eq!(RequestEnvelope::parse(&json).unwrap(), env);
    }

    #[test]
    fn bare_commands_parse() {
        let cases = [
            ("show", ControlRequest::Show),
            ("reconnect", ControlRequest::Reconnect),
            ("reload", ControlRequest::Reload),
            ("obs-info", ControlRequest::ObsInfo),
            ("apps", ControlRequest::Apps),
            ("selftest", ControlRequest::Selftest),
            ("  status  ", ControlRequest::Status),
        ];
        for (text, expected) in cases {
            assert_eq!(parse(text), expected, "{text}");
        }
    }

    #[test]
    fn text_wrapped_in_json_is_normalized_like_bare_text() {
        let wrapped = r#"{"version":1,"cmd":"text","line":"brightness 50"}"#;
        assert_eq!(parse(wrapped), ControlRequest::Brightness { level: 50 });
        assert_eq!(
            parse("brightness 50"),
            ControlRequest::Brightness { level: 50 }
        );
    }

    #[test]
    fn obs_run_keeps_a_target_with_spaces() {
        assert_eq!(
            parse("obs-run scene Cena de jogo"),
            ControlRequest::ObsRun {
                command: "scene".into(),
                target: "Cena de jogo".into(),
            }
        );
        assert_eq!(
            parse("obs-run record"),
            ControlRequest::ObsRun {
                command: "record".into(),
                target: String::new(),
            }
        );
    }

    #[test]
    fn diagnostic_sizes_are_optional_but_must_be_numbers() {
        assert_eq!(parse("calib"), ControlRequest::Calib { px: None });
        assert_eq!(parse("calib 100"), ControlRequest::Calib { px: Some(100) });
        assert_eq!(parse("pattern"), ControlRequest::Pattern { px: None });
        assert_eq!(
            parse("pattern 64"),
            ControlRequest::Pattern { px: Some(64) }
        );
        for bad in ["calib abc", "pattern -1", "calibrate"] {
            assert!(matches!(parse(bad), ControlRequest::Text { .. }), "{bad}");
        }
    }

    #[test]
    fn unknown_or_malformed_text_stays_text() {
        for line in [
            "STATUS",
            "rm -rf /",
            "",
            "show now",
            "brightness x",
            "brightness 300",
        ] {
            assert_eq!(
                parse(line),
                ControlRequest::Text {
                    line: line.to_string()
                },
                "{line:?}"
            );
        }
    }

    #[test]
    fn newer_versions_and_bad_json_are_refused() {
        let newer = format!(r#"{{"version":{},"cmd":"show"}}"#, PROTOCOL_VERSION + 1);
        assert_eq!(
            RequestEnvelope::parse(&newer),
            Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                got: PROTOCOL_VERSION + 1,
            })
        );
        assert!(matches!(
            RequestEnvelope::parse(r#"{"version":1,"cmd":"nope"}"#),
            Err(ProtocolError::Json(_))
        ));
        assert!(matches!(
            RequestEnvelope::parse("{"),
            Err(ProtocolError::Json(_))
        ));
    }

    /// Toda variante de resposta precisa serializar (enum com tag interna não aceita newtype de
    /// string/número, e isso só falharia em tempo de execução).
    #[test]
    fn every_response_variant_serializes() {
        let variants = [
            ControlResponse::Ok,
            ControlResponse::Error {
                message: "x".into(),
            },
            ControlResponse::ObsInfo(serde_json::json!({ "connected": true })),
            ControlResponse::ObsInfo(Value::Null),
            ControlResponse::Apps {
                apps: "firefox\tkitty".into(),
            },
            ControlResponse::Status {
                connected: true,
                icon_px: 96,
            },
            ControlResponse::Selftest {
                ok: true,
                message: String::new(),
            },
        ];
        for v in variants {
            let label = format!("{v:?}");
            serde_json::to_string(&ResponseEnvelope::new(v))
                .unwrap_or_else(|e| panic!("{label} não serializa: {e}"));
        }
    }

    #[test]
    fn responses_carry_version_and_tag() {
        let ok: Value =
            serde_json::from_str(&ResponseEnvelope::new(ControlResponse::Ok).to_json()).unwrap();
        assert_eq!(ok["resp"], "ok");
        assert_eq!(ok["version"], PROTOCOL_VERSION);
        let apps: Value = serde_json::from_str(
            &ResponseEnvelope::new(ControlResponse::Apps {
                apps: "firefox\tkitty".into(),
            })
            .to_json(),
        )
        .unwrap();
        assert_eq!(apps["resp"], "apps");
        assert_eq!(apps["apps"], "firefox\tkitty");
    }
}
