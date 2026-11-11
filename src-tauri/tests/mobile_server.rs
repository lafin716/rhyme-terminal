#![cfg(desktop)]

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

struct Server {
    child: Child,
    input: Option<ChildStdin>,
    output: mpsc::Receiver<String>,
    session: Option<String>,
}

impl Server {
    fn start(port: u16, cli: bool) -> Self {
        let mut process = Command::new(if cli {
            env!("CARGO_BIN_EXE_rhyme-terminal-cli")
        } else {
            env!("CARGO_BIN_EXE_rhyme-terminal")
        });
        process.arg(if cli { "serve" } else { "--server" });
        if cli {
            process.arg("--json");
        }
        let mut child = process
            .args([
                "--bind",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--new-session",
                "--shell",
                if cfg!(windows) { "cmd.exe" } else { "/bin/sh" },
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input: Some(input),
            output,
            session: None,
        }
    }

    fn line_containing(&self, expected: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let line = self
                .output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("server did not produce the expected output");
            if line.contains(expected) {
                return line;
            }
        }
    }

    fn command(&mut self, line: &str) {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{line}").unwrap();
        input.flush().unwrap();
    }

    fn wait(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "headless server failed");
                return;
            }
            assert!(Instant::now() < deadline, "server did not exit after quit");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Remove only the dedicated test session. Never stop a shared daemon
        // or touch the developer's pre-existing terminal sessions.
        if let Some(id) = &self.session {
            let _ = Command::new(env!("CARGO_BIN_EXE_rhyme-terminal-cli"))
                .args(["kill", id])
                .output();
        }
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
async fn message(socket: &mut Socket, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let frame = socket
                .next()
                .await
                .expect("socket closed")
                .expect("socket failed");
            if let Ok(text) = frame.to_text() {
                let value: Value = serde_json::from_str(text).unwrap();
                if predicate(&value) {
                    return value;
                }
            }
        }
    })
    .await
    .expect("expected WebSocket message was not received")
}
async fn send(socket: &mut Socket, value: Value) {
    socket.send(value.to_string().into()).await.unwrap();
}

#[test]
fn installed_executable_server_help_does_not_start_the_gui() {
    let output = Command::new(env!("CARGO_BIN_EXE_rhyme-terminal"))
        .args(["--server", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("without a GUI"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn headless_server_pairs_and_runs_mobile_input_without_a_gui() {
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let mut server = Server::start(port, false);
    let created = server.line_containing("Created terminal session:");
    let id = created
        .rsplit_once('(')
        .unwrap()
        .1
        .trim_end_matches(')')
        .to_owned();
    server.session = Some(id.clone());
    let invite = server.line_containing("Invitation (2 minutes, single use):");
    let invite = invite.rsplit_once("#invite=").unwrap().1;
    let origin = format!("http://127.0.0.1:{port}");
    assert_eq!(reqwest::get(&origin).await.unwrap().status(), 200);
    let mut request = format!("ws://127.0.0.1:{port}/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Origin", origin.parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();
    send(
        &mut socket,
        json!({"type":"pair", "invite":invite, "name":"CLI test phone"}),
    )
    .await;
    let pending = message(&mut socket, |value| value["type"] == "pending").await;
    let notice = server.line_containing("Pairing request from CLI test phone");
    assert!(notice.contains(pending["verification"].as_str().unwrap()));
    let request_id = notice
        .split("approve ")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    server.command(&format!("approve {request_id}"));
    let authenticated = message(&mut socket, |value| value["type"] == "authenticated").await;
    assert!(authenticated["token"]
        .as_str()
        .is_some_and(|token| !token.is_empty()));
    send(&mut socket, json!({"type":"list", "requestId":1})).await;
    let sessions = message(&mut socket, |value| value["requestId"] == 1).await;
    assert!(sessions["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|session| session["id"] == id));
    send(
        &mut socket,
        json!({"type":"attach", "requestId":2, "id":id}),
    )
    .await;
    assert_eq!(
        message(&mut socket, |value| value["requestId"] == 2).await["type"],
        "result"
    );
    let input = if cfg!(windows) {
        "set RHYME_CLI_RESULT=OK\recho __RHYME_HEADLESS_%RHYME_CLI_RESULT%__\r"
    } else {
        "printf '__RHYME_HEADLESS_%s__\\n' OK\r"
    };
    send(
        &mut socket,
        json!({"type":"input", "requestId":3, "id":id,
        "data":base64::engine::general_purpose::STANDARD.encode(input)}),
    )
    .await;
    let mut terminal = String::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !terminal.contains("__RHYME_HEADLESS_OK__") {
            let event = message(&mut socket, |value| {
                value["type"] == "event" && value["data"]["event"] == "pty_output"
            })
            .await;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(event["data"]["data"].as_str().unwrap())
                .unwrap();
            terminal.push_str(&String::from_utf8_lossy(&bytes));
        }
    })
    .await
    .expect("mobile input did not execute in the PTY");
    let device_id = authenticated["deviceId"].as_str().unwrap();
    server.command(&format!("revoke {device_id}"));
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(frame)) = socket.next().await {
            if frame.is_close() {
                break;
            }
        }
    })
    .await
    .expect("revoking the device did not close its connection");
    server.command("quit");
    server.wait();
    let sessions = Command::new(env!("CARGO_BIN_EXE_rhyme-terminal-cli"))
        .args(["ls", "--json"])
        .output()
        .unwrap();
    assert!(sessions.status.success());
    let sessions: Vec<Value> = serde_json::from_slice(&sessions.stdout).unwrap();
    assert!(
        sessions.iter().any(|session| session["id"] == id),
        "stopping the gateway must preserve the PTY session"
    );
}

#[test]
fn cli_serve_emits_json_and_exits_when_stdin_closes() {
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let mut server = Server::start(port, true);
    let created: Value =
        serde_json::from_str(&server.line_containing("\"type\":\"session\"")).unwrap();
    server.session = Some(created["data"]["id"].as_str().unwrap().to_owned());
    let invitation: Value =
        serde_json::from_str(&server.line_containing("\"type\":\"invite\"")).unwrap();
    assert!(invitation["data"]["url"]
        .as_str()
        .unwrap()
        .contains("#invite="));
    drop(server.input.take());
    let stopped: Value =
        serde_json::from_str(&server.line_containing("\"type\":\"stopped\"")).unwrap();
    assert_eq!(stopped["type"], "stopped");
    server.wait();
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[cfg(unix)]
#[test]
fn ctrl_c_stops_the_server_without_waiting_for_another_stdin_line() {
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let mut server = Server::start(port, false);
    let created = server.line_containing("Created terminal session:");
    server.session = Some(
        created
            .rsplit_once('(')
            .unwrap()
            .1
            .trim_end_matches(')')
            .to_owned(),
    );
    server.line_containing("Commands:");
    // control_loop installs the signal handler before processing the first
    // status command; its response is the synchronization boundary.
    server.command("status");
    server.line_containing("Server:");
    assert_eq!(
        unsafe { libc::kill(server.child.id() as i32, libc::SIGINT) },
        0
    );
    server.wait();
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
}
