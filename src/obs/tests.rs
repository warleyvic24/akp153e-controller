use super::*;

fn event(kind: &str, data: Value) -> Value {
    json!({ "op": op::EVENT, "d": { "eventType": kind, "eventIntent": 0, "eventData": data } })
}

fn response(kind: &str, id: &str, data: Value) -> Value {
    json!({ "op": op::RESPONSE, "d": {
        "requestType": kind, "requestId": id,
        "requestStatus": { "result": true, "code": 100 },
        "responseData": data,
    }})
}

#[test]
fn authentication_matches_the_protocol_example() {
    // Exemplo da documentação do obs-websocket 5 (protocol.md, "Creating an authentication string").
    let auth = auth_response(
        "supersecretpassword",
        "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=",
        "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY=",
    );
    assert_eq!(auth, "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4=");
}

#[test]
fn identify_carries_auth_only_when_asked() {
    let open: Value = serde_json::from_str(&identify(&json!({"op": 0, "d": {}}), "x")).unwrap();
    assert_eq!(open["op"], op::IDENTIFY);
    assert!(open["d"].get("authentication").is_none());
    let hello = json!({"op": 0, "d": {"authentication": {"salt": "s", "challenge": "c"}}});
    let locked: Value = serde_json::from_str(&identify(&hello, "senha")).unwrap();
    assert_eq!(
        locked["d"]["authentication"],
        auth_response("senha", "s", "c")
    );
}

#[test]
fn each_command_asks_obs_for_the_right_request() {
    let (kind, data) = request_for(ObsCommand::Scene, "Jogo");
    assert_eq!(
        (kind, data["sceneName"].as_str()),
        ("SetCurrentProgramScene", Some("Jogo"))
    );
    assert_eq!(request_for(ObsCommand::Record, "").0, "ToggleRecord");
    let (kind, data) = request_for(ObsCommand::Mute, "Mic/Aux");
    assert_eq!(
        (kind, data["inputName"].as_str()),
        ("ToggleInputMute", Some("Mic/Aux"))
    );
    let msg: Value = serde_json::from_str(&request("1", "ToggleStream", Value::Null)).unwrap();
    assert_eq!(msg["op"], op::REQUEST);
    assert!(msg["d"].get("requestData").is_none());
}

#[test]
fn events_update_the_state_and_report_changes() {
    let mut s = ObsState::default();
    let (changed, _) = apply_event(
        &mut s,
        &event(
            "RecordStateChanged",
            json!({"outputActive": true, "outputState": "OBS_WEBSOCKET_OUTPUT_STARTED"}),
        ),
    );
    assert!(changed && s.recording && !s.record_paused);
    let (changed, _) = apply_event(
        &mut s,
        &event(
            "RecordStateChanged",
            json!({"outputActive": true, "outputState": RECORD_PAUSED}),
        ),
    );
    assert!(changed && s.record_paused);
    apply_event(
        &mut s,
        &event("CurrentProgramSceneChanged", json!({"sceneName": "Jogo"})),
    );
    assert_eq!(s.scene, "Jogo");
    apply_event(
        &mut s,
        &event(
            "InputMuteStateChanged",
            json!({"inputName": "Mic", "inputMuted": true}),
        ),
    );
    assert!(s.is_active(ObsCommand::Mute, "Mic"));
    let (changed, _) = apply_event(
        &mut s,
        &event(
            "InputMuteStateChanged",
            json!({"inputName": "Mic", "inputMuted": true}),
        ),
    );
    assert!(!changed, "nada mudou");
    let (_, follow) = apply_event(&mut s, &event("InputCreated", json!({"inputName": "Novo"})));
    assert_eq!(follow.len(), 1, "relê a lista de entradas");
}

#[test]
fn responses_fill_scenes_inputs_and_mutes() {
    let mut s = ObsState::default();
    let scenes = json!({"currentProgramSceneName": "Início", "scenes": [
        {"sceneName": "Fim", "sceneIndex": 0}, {"sceneName": "Início", "sceneIndex": 1}]});
    apply_response(&mut s, &response("GetSceneList", "GetSceneList", scenes));
    assert_eq!(s.scenes, ["Início", "Fim"], "na ordem do painel do OBS");
    assert_eq!(s.scene, "Início");
    let inputs = json!({"inputs": [{"inputName": "Mic"}, {"inputName": "Desktop"}]});
    let (_, follow) = apply_response(&mut s, &response("GetInputList", "GetInputList", inputs));
    assert_eq!(follow.len(), 2, "pergunta o mudo de cada entrada");
    apply_response(
        &mut s,
        &response("GetInputMute", "mute:Desktop", json!({"inputMuted": true})),
    );
    assert_eq!(s.muted, ["Desktop"]);
    let failed = json!({"op": 7, "d": {"requestType": "GetReplayBufferStatus", "requestId": "x",
        "requestStatus": {"result": false, "code": 604}}});
    assert!(!apply_response(&mut s, &failed).0);
}

#[test]
fn a_live_key_turns_red_or_uses_its_second_state() {
    let mut b = ButtonConfig::blank();
    assert_eq!(
        live_look(&b, ObsCommand::Record, true)
            .appearance
            .background,
        LIVE_RED
    );
    assert_eq!(
        live_look(&b, ObsCommand::Record, false)
            .appearance
            .background,
        b.appearance.background
    );
    let scene = live_look(&b, ObsCommand::Scene, true).appearance.background;
    assert_eq!(scene.r, SCENE_LIFT);
    b.toggled_appearance = Some(b.appearance.clone());
    assert!(live_look(&b, ObsCommand::Record, true).toggle_state);
    assert!(!live_look(&b, ObsCommand::Record, false).toggle_state);
}

#[test]
fn a_call_survives_the_control_port() {
    let call = ObsCall {
        command: ObsCommand::Scene,
        target: "Cena com espaço".into(),
    };
    assert_eq!(call.control_line(), "obs-run scene Cena com espaço");
    let back = ObsCall::from_control(call.control_line().strip_prefix("obs-run ").unwrap());
    assert_eq!(back, Some(call));
    let rec = ObsCall::from_control("record").unwrap();
    assert_eq!((rec.command, rec.target.as_str()), (ObsCommand::Record, ""));
    assert!(ObsCall::from_control("formatar-disco").is_none());
}

#[test]
fn info_for_the_editor_is_one_line_of_json() {
    let s = ObsState {
        connected: true,
        scenes: vec!["A".into()],
        inputs: vec!["Mic".into()],
        ..ObsState::default()
    };
    let info = s.info_json();
    assert!(!info.contains('\n'));
    assert_eq!(ObsState::from_info_json(&info), Some(s.clone()));
    assert_eq!(
        ObsState::from_info_json(r#"{"connected":true,"scenes":["A"]}"#).map(|o| o.connected),
        Some(true),
        "serviço antigo, sem os estados"
    );
    let v: Value = serde_json::from_str(&info).unwrap();
    assert_eq!(v["scenes"][0], "A");
}
