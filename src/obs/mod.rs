//! Integração com o OBS Studio pelo obs-websocket 5 (embutido no OBS desde a versão 28). Aqui fica
//! só o protocolo e o estado, sem rede: autenticação, mensagens, qual pedido cada ação faz e o que
//! cada evento muda. A conexão vive no serviço (`service::obs_runtime`).
use crate::config::schema::{ButtonConfig, ObsCommand, RGBAColor};
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Versão do protocolo de pedidos (RPC) que o obs-websocket 5 fala.
const RPC_VERSION: u64 = 1;
/// Eventos assinados: gerais (1), cenas (4), entradas (8) e saídas (64: live, gravação, câmera).
const EVENT_SUBSCRIPTIONS: u64 = 1 | 4 | 8 | 64;
/// Mais entradas que isso não têm o mudo consultado uma a uma na conexão.
const MAX_INPUTS: usize = 64;
/// Estado de saída pausada no evento de gravação.
const RECORD_PAUSED: &str = "OBS_WEBSOCKET_OUTPUT_PAUSED";

/// Códigos de operação das mensagens.
pub mod op {
    pub const HELLO: u64 = 0;
    pub const IDENTIFY: u64 = 1;
    pub const IDENTIFIED: u64 = 2;
    pub const EVENT: u64 = 5;
    pub const REQUEST: u64 = 6;
    pub const RESPONSE: u64 = 7;
}

/// Um aperto de tecla para o OBS (vai do executor das ações para a conexão do serviço).
#[derive(Clone, Debug, PartialEq)]
pub struct ObsCall {
    pub command: ObsCommand,
    pub target: String,
}

impl ObsCall {
    /// Linha de controle que o editor manda ao serviço ("obs-run record", "obs-run scene Jogo").
    pub fn control_line(&self) -> String {
        let name = serde_json::to_value(self.command)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        format!("obs-run {name} {}", self.target)
            .trim_end()
            .to_string()
    }

    /// Lê a linha de controle (sem o "obs-run "); `None` se o comando não existir.
    pub fn from_control(rest: &str) -> Option<Self> {
        let (name, target) = rest.trim().split_once(' ').unwrap_or((rest.trim(), ""));
        let command = serde_json::from_value(json!(name)).ok()?;
        Some(Self {
            command,
            target: target.trim().to_string(),
        })
    }
}

/// O que o serviço sabe do OBS agora.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObsState {
    pub connected: bool,
    pub streaming: bool,
    pub recording: bool,
    pub record_paused: bool,
    pub virtualcam: bool,
    pub replay: bool,
    pub scene: String,
    pub scenes: Vec<String>,
    pub inputs: Vec<String>,
    /// Entradas mudas (as de áudio que o OBS informou).
    pub muted: Vec<String>,
}

impl ObsState {
    /// A tecla `command`/`target` está "ligada" agora (gravando, na cena, mudo...)?
    pub fn is_active(&self, command: ObsCommand, target: &str) -> bool {
        match command {
            ObsCommand::Scene => self.connected && self.scene == target,
            ObsCommand::Stream => self.streaming,
            ObsCommand::Record => self.recording,
            ObsCommand::RecordPause => self.record_paused,
            ObsCommand::Mute => self.muted.iter().any(|m| m == target),
            ObsCommand::VirtualCam => self.virtualcam,
            ObsCommand::ReplayBuffer => self.replay,
            ObsCommand::ReplaySave => false,
        }
    }

    fn set_muted(&mut self, input: &str, muted: bool) {
        self.muted.retain(|m| m != input);
        if muted {
            self.muted.push(input.to_string());
        }
    }

    /// Resumo de uma linha para o editor: `{"connected":..,"scenes":[..],"inputs":[..]}`.
    pub fn info_json(&self) -> String {
        json!({
            "connected": self.connected,
            "scenes": self.scenes,
            "inputs": self.inputs,
        })
        .to_string()
    }
}

/// Resposta da autenticação: base64(sha256(base64(sha256(senha + sal)) + desafio)).
pub fn auth_response(password: &str, salt: &str, challenge: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    b64.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}

/// Mensagem `Identify` em resposta ao `Hello` (com autenticação, se o OBS pedir).
pub fn identify(hello: &Value, password: &str) -> String {
    let mut d = json!({ "rpcVersion": RPC_VERSION, "eventSubscriptions": EVENT_SUBSCRIPTIONS });
    if let Some(auth) = hello.pointer("/d/authentication") {
        let salt = auth.get("salt").and_then(Value::as_str).unwrap_or_default();
        let challenge = auth
            .get("challenge")
            .and_then(Value::as_str)
            .unwrap_or_default();
        d["authentication"] = json!(auth_response(password, salt, challenge));
    }
    json!({ "op": op::IDENTIFY, "d": d }).to_string()
}

/// Mensagem de pedido (`Request`).
pub fn request(id: &str, request_type: &str, data: Value) -> String {
    let mut d = json!({ "requestType": request_type, "requestId": id });
    if !data.is_null() {
        d["requestData"] = data;
    }
    json!({ "op": op::REQUEST, "d": d }).to_string()
}

/// Pedido que a ação faz no OBS: (tipo, dados).
pub fn request_for(command: ObsCommand, target: &str) -> (&'static str, Value) {
    match command {
        ObsCommand::Scene => ("SetCurrentProgramScene", json!({ "sceneName": target })),
        ObsCommand::Stream => ("ToggleStream", Value::Null),
        ObsCommand::Record => ("ToggleRecord", Value::Null),
        ObsCommand::RecordPause => ("ToggleRecordPause", Value::Null),
        ObsCommand::Mute => ("ToggleInputMute", json!({ "inputName": target })),
        ObsCommand::VirtualCam => ("ToggleVirtualCam", Value::Null),
        ObsCommand::ReplayBuffer => ("ToggleReplayBuffer", Value::Null),
        ObsCommand::ReplaySave => ("SaveReplayBuffer", Value::Null),
    }
}

/// Pedidos que trazem o estado inteiro logo depois de conectar.
pub fn initial_requests() -> Vec<String> {
    [
        "GetSceneList",
        "GetInputList",
        "GetStreamStatus",
        "GetRecordStatus",
        "GetVirtualCamStatus",
        "GetReplayBufferStatus",
    ]
    .iter()
    .map(|t| request(t, t, Value::Null))
    .collect()
}

fn active(data: &Value) -> bool {
    data.get("outputActive")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn names(list: Option<&Value>, key: &str) -> Vec<String> {
    list.and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.get(key)?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Aplica um evento (`op 5`); devolve se o estado mudou e pedidos a fazer em seguida.
pub fn apply_event(state: &mut ObsState, message: &Value) -> (bool, Vec<String>) {
    let before = state.clone();
    let mut follow = Vec::new();
    let event = message
        .pointer("/d/eventType")
        .and_then(Value::as_str)
        .unwrap_or("");
    let data = message
        .pointer("/d/eventData")
        .cloned()
        .unwrap_or(Value::Null);
    match event {
        "StreamStateChanged" => state.streaming = active(&data),
        "RecordStateChanged" => {
            state.recording = active(&data);
            state.record_paused =
                data.get("outputState").and_then(Value::as_str) == Some(RECORD_PAUSED);
        }
        "VirtualcamStateChanged" => state.virtualcam = active(&data),
        "ReplayBufferStateChanged" => state.replay = active(&data),
        "CurrentProgramSceneChanged" => {
            state.scene = data
                .get("sceneName")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into();
        }
        "SceneListChanged" => {
            let mut scenes = names(data.get("scenes"), "sceneName");
            // O OBS manda de baixo para cima; a lista do editor fica na ordem do painel.
            scenes.reverse();
            state.scenes = scenes;
        }
        "InputMuteStateChanged" => {
            let input = data.get("inputName").and_then(Value::as_str).unwrap_or("");
            let muted = data
                .get("inputMuted")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            state.set_muted(input, muted);
        }
        "InputCreated" | "InputRemoved" | "InputNameChanged" => {
            follow.push(request("GetInputList", "GetInputList", Value::Null));
        }
        _ => {}
    }
    (*state != before, follow)
}

/// Aplica a resposta de um pedido (`op 7`); devolve se mudou e pedidos a fazer em seguida.
pub fn apply_response(state: &mut ObsState, message: &Value) -> (bool, Vec<String>) {
    let before = state.clone();
    let mut follow = Vec::new();
    let ok = message
        .pointer("/d/requestStatus/result")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !ok {
        return (false, follow);
    }
    let kind = message
        .pointer("/d/requestType")
        .and_then(Value::as_str)
        .unwrap_or("");
    let id = message
        .pointer("/d/requestId")
        .and_then(Value::as_str)
        .unwrap_or("");
    let data = message
        .pointer("/d/responseData")
        .cloned()
        .unwrap_or(Value::Null);
    match kind {
        "GetSceneList" => {
            let mut scenes = names(data.get("scenes"), "sceneName");
            scenes.reverse();
            state.scenes = scenes;
            state.scene = data
                .get("currentProgramSceneName")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into();
        }
        "GetInputList" => {
            state.inputs = names(data.get("inputs"), "inputName");
            for input in state.inputs.iter().take(MAX_INPUTS) {
                let id = format!("mute:{input}");
                follow.push(request(&id, "GetInputMute", json!({ "inputName": input })));
            }
        }
        "GetInputMute" => {
            let input = id.strip_prefix("mute:").unwrap_or("");
            let muted = data
                .get("inputMuted")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            state.set_muted(input, muted);
        }
        "GetStreamStatus" => state.streaming = active(&data),
        "GetRecordStatus" => {
            state.recording = active(&data);
            state.record_paused = data
                .get("outputPaused")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
        "GetVirtualCamStatus" => state.virtualcam = active(&data),
        "GetReplayBufferStatus" => state.replay = active(&data),
        _ => {}
    }
    (*state != before, follow)
}

/// Cor da tecla "ligada" quando o usuário não definiu um segundo estado: vermelho para o que está
/// no ar ou mudo, e o próprio fundo clareado para a cena atual.
const LIVE_RED: RGBAColor = RGBAColor {
    r: 200,
    g: 32,
    b: 44,
    a: 255,
};
const SCENE_LIFT: u8 = 70;

/// Aparência da tecla do OBS conforme o estado: ligada usa o segundo estado da tecla, se houver;
/// senão um destaque automático.
pub fn live_look(b: &ButtonConfig, command: ObsCommand, active: bool) -> ButtonConfig {
    let mut shown = b.clone();
    if b.toggled_appearance.is_some() {
        shown.toggle_state = active;
        return shown;
    }
    if active {
        let bg = &mut shown.appearance.background;
        if command == ObsCommand::Scene {
            *bg = RGBAColor {
                r: bg.r.saturating_add(SCENE_LIFT),
                g: bg.g.saturating_add(SCENE_LIFT),
                b: bg.b.saturating_add(SCENE_LIFT),
                a: bg.a,
            };
        } else {
            *bg = LIVE_RED;
        }
    }
    shown
}

#[cfg(test)]
mod tests;
