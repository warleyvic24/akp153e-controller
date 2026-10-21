//! Testes de caixa-preta: rodam o executável de verdade pela linha de comando.
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::Duration;

const REQUEST_WAIT: Duration = Duration::from_secs(10);

fn run_action(arg: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_akp153e"));
    cmd.arg("--run-action");
    cmd.args(arg);
    cmd.output().expect("o executável deveria iniciar")
}

#[test]
fn an_action_that_does_nothing_exits_cleanly() {
    let out = run_action(Some(r#"{"type":"none"}"#));
    assert!(out.status.success(), "{:?}", out.status);
}

#[test]
fn invalid_json_exits_with_code_2() {
    assert_eq!(run_action(Some("isto não é json")).status.code(), Some(2));
}

#[test]
fn a_missing_action_exits_with_code_2() {
    assert_eq!(run_action(None).status.code(), Some(2));
}

#[test]
fn a_webhook_action_reaches_the_server_with_method_and_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let n = stream.read(&mut buf).unwrap();
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        let _ = tx.send(String::from_utf8_lossy(&buf[..n]).into_owned());
    });
    let action = json!({"type": "http-webhook", "url": format!("http://{addr}/hook"), "method": "post", "body": "ping"});
    assert!(run_action(Some(&action.to_string())).status.success());
    let request = rx
        .recv_timeout(REQUEST_WAIT)
        .expect("o servidor deveria receber a requisição");
    assert!(request.starts_with("POST /hook"), "{request}");
    assert!(request.ends_with("ping"), "{request}");
}

#[test]
fn an_unknown_webhook_method_sends_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let action =
        json!({"type": "http-webhook", "url": format!("http://{addr}/x"), "method": "DELET"});
    assert!(run_action(Some(&action.to_string())).status.success());
    assert!(
        listener.accept().is_err(),
        "nenhuma conexão deveria ter chegado"
    );
}

#[test]
fn a_launch_action_starts_the_program_with_its_arguments() {
    let out = std::env::temp_dir().join(format!("akp_cli_launch_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let action = json!({"type": "launch-app", "path": "cmd", "args": ["/C", "echo", "ok", ">", out.display().to_string()]});
    assert!(run_action(Some(&action.to_string())).status.success());
    let written = std::fs::read_to_string(&out).expect("o programa deveria ter gravado o arquivo");
    let _ = std::fs::remove_file(&out);
    assert!(written.trim().starts_with("ok"), "{written:?}");
}
