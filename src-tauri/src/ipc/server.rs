use super::transport::{Listener, Server as NamedPipeServer};
use anyhow::{anyhow, Result};
use base64::Engine;
use portable_pty::PtySize;
use serde_json::json;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};
use uuid::Uuid;

use super::protocol::{AttachResult, Event, Method, Request, ServerMsg};
use super::{pipe_name, read_frame, write_frame};
use crate::pty::agent::process_snapshot;
use crate::pty::manager::SessionManager;
use crate::pty::{scrollback_snapshot, spawn_session};

#[cfg(windows)]
const DEFAULT_SHELL: &str = "powershell.exe";
#[cfg(unix)]
const DEFAULT_SHELL: &str = "/bin/zsh";

pub struct DaemonState {
    pub routing: crate::loop_routing::Service,
    pub manager: Arc<SessionManager>,
    pub events: broadcast::Sender<Event>,
}

impl DaemonState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            routing: crate::loop_routing::Service::default(),
            manager: Arc::new(SessionManager::new()),
            events: tx,
        }
    }
}

pub async fn run_server(state: Arc<DaemonState>) -> Result<()> {
    let name = pipe_name();

    let mut listener = Listener::bind(&name).await?;
    info!("daemon listening on {name}");
    crate::loop_routing::Service::start(state.clone());

    let monitor_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if !monitor_state.manager.has_sessions() {
                continue;
            }
            let processes = match tokio::task::spawn_blocking(process_snapshot).await {
                Ok(processes) => processes,
                Err(error) => {
                    warn!("agent monitor snapshot task failed: {error}");
                    continue;
                }
            };
            let changes = monitor_state.manager.refresh_agents(&processes);
            for (id, agent) in changes {
                let _ = monitor_state
                    .events
                    .send(Event::SessionAgentChanged { id, agent });
            }
            for (id, status) in monitor_state
                .manager
                .complete_idle_tasks(Duration::from_secs(2))
            {
                let _ = monitor_state
                    .events
                    .send(Event::SessionAgentStatusChanged { id, status });
            }
        }
    });

    loop {
        let connected = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(st, connected).await {
                warn!("client task ended: {e}");
            }
        });
    }
}

async fn handle_client(state: Arc<DaemonState>, pipe: NamedPipeServer) -> Result<()> {
    let (read_half, write_half) = tokio::io::split(pipe);
    let writer = Arc::new(AsyncMutex::new(write_half));
    let attached: Arc<AsyncMutex<HashSet<Uuid>>> = Arc::new(AsyncMutex::new(HashSet::new()));

    let failed = Arc::new(tokio::sync::Notify::new());
    let mut event_task = forward_events(
        state.events.subscribe(),
        writer.clone(),
        attached.clone(),
        false,
        failed.clone(),
    );
    let mut reader = read_half;
    let result = async {
        loop {
            let frame = read_frame(&mut reader).await?;
            let req: Request = match serde_json::from_slice(&frame) {
                Ok(r) => r,
                Err(e) => {
                    let err = ServerMsg::Error {
                        id: 0,
                        message: format!("invalid request: {e}"),
                    };
                    let bytes = serde_json::to_vec(&err)?;
                    let mut w = writer.lock().await;
                    write_frame(&mut *w, &bytes).await?;
                    continue;
                }
            };

            let req_id = req.id;
            // Usage may wait on the provider network. Keep terminal input and
            // loop control responsive on this same multiplexed IPC connection.
            if matches!(req.method, Method::AccountUsage { .. }) {
                let usage_state = state.clone();
                let usage_attached = attached.clone();
                let usage_writer = writer.clone();
                tokio::spawn(async move {
                    let response = match dispatch(usage_state, usage_attached, req).await {
                        Ok(value) => ServerMsg::Response {
                            id: req_id,
                            result: value,
                        },
                        Err(error) => ServerMsg::Error {
                            id: req_id,
                            message: error.to_string(),
                        },
                    };
                    if let Ok(bytes) = serde_json::to_vec(&response) {
                        let _ = tokio::time::timeout(Duration::from_secs(5), async {
                            write_frame(&mut *usage_writer.lock().await, &bytes).await
                        })
                        .await;
                    }
                });
                continue;
            }
            let kill_server = matches!(req.method, Method::KillServer);
            if let Method::AttachSessionAtomic { id } = req.method {
                // Wait for a complete frame before stopping the old relay.
                let mut atomic_writer = writer.lock().await;
                event_task.abort();
                let _ = (&mut event_task).await;
                let snapshot = {
                    let map = state.manager.sessions.lock();
                    map.get(&id).map(|session| {
                        let (scrollback, receiver) =
                            snapshot_and_subscribe(&session.scrollback, &state.events);
                        (
                            AttachResult {
                                info: session.info.clone(),
                                scrollback,
                            },
                            receiver,
                        )
                    })
                };
                match snapshot {
                    Some((snapshot, receiver)) => {
                        {
                            let mut ids = attached.lock().await;
                            ids.clear();
                            ids.insert(id);
                        }
                        let response = ServerMsg::Response {
                            id: req_id,
                            result: serde_json::to_value(snapshot)?,
                        };
                        write_frame(&mut *atomic_writer, &serde_json::to_vec(&response)?).await?;
                        event_task = forward_events(
                            receiver,
                            writer.clone(),
                            attached.clone(),
                            true,
                            failed.clone(),
                        );
                    }
                    None => {
                        attached.lock().await.clear();
                        let response = ServerMsg::Error {
                            id: req_id,
                            message: "session not found".into(),
                        };
                        write_frame(&mut *atomic_writer, &serde_json::to_vec(&response)?).await?;
                        event_task = forward_events(
                            state.events.subscribe(),
                            writer.clone(),
                            attached.clone(),
                            true,
                            failed.clone(),
                        );
                    }
                }
                continue;
            }
            let response = match dispatch(state.clone(), attached.clone(), req).await {
                Ok(value) => ServerMsg::Response {
                    id: req_id,
                    result: value,
                },
                Err(e) => ServerMsg::Error {
                    id: req_id,
                    message: e.to_string(),
                },
            };
            let bytes = serde_json::to_vec(&response)?;
            {
                let mut w = writer.lock().await;
                write_frame(&mut *w, &bytes).await?;
                let _ = w.flush().await;
            }
            if kill_server {
                info!("kill-server requested, exiting");
                state.manager.kill_all();
                std::process::exit(0);
            }
        }
    };
    let result: Result<()> = tokio::select! { value=result=>value, _=failed.notified()=>Err(anyhow!("event stream disconnected or lagged")) };
    event_task.abort();
    let _ = event_task.await;
    result
}

fn forward_events(
    mut receiver: broadcast::Receiver<Event>,
    writer: Arc<AsyncMutex<tokio::io::WriteHalf<NamedPipeServer>>>,
    attached: Arc<AsyncMutex<HashSet<Uuid>>>,
    strict: bool,
    failed: Arc<tokio::sync::Notify>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let event = match receiver.recv().await {
                Ok(event) => event,
                Err(broadcast::error::RecvError::Lagged(_)) if !strict => continue,
                Err(_) => break,
            };
            if matches!(&event, Event::PtyOutput {id,..}|Event::PtyExit {id,..} if !attached.lock().await.contains(id))
            {
                continue;
            }
            let Ok(bytes) = serde_json::to_vec(&ServerMsg::Event(event)) else {
                break;
            };
            let write = async { write_frame(&mut *writer.lock().await, &bytes).await };
            if tokio::time::timeout(Duration::from_secs(5), write)
                .await
                .map_or(true, |result| result.is_err())
            {
                break;
            }
        }
        failed.notify_one();
    })
}

async fn dispatch(
    state: Arc<DaemonState>,
    attached: Arc<AsyncMutex<HashSet<Uuid>>>,
    req: Request,
) -> Result<serde_json::Value> {
    match req.method {
        Method::LoopRequest { request } => state.routing.request(&state, request).await,
        Method::AccountUsage {
            agent,
            dir,
            session_usage,
        } => {
            let windows = crate::usage::query_account_usage(&agent, &dir, session_usage)
                .await
                .map_err(|e| anyhow!(e))?;
            Ok(serde_json::to_value(windows)?)
        }
        Method::AttachSessionAtomic { .. } => {
            Err(anyhow!("atomic attach requires connection handler"))
        }
        Method::Ping => Ok(json!("pong")),
        Method::CreateSession {
            name,
            shell,
            shell_args,
            cwd,
            env,
            cols,
            rows,
        } => {
            let shell = shell.unwrap_or_else(|| DEFAULT_SHELL.to_string());
            let name = name.unwrap_or_else(|| state.manager.next_default_name());
            let session = spawn_session(
                state.events.clone(),
                name,
                shell,
                shell_args,
                cwd,
                env,
                cols,
                rows,
            )?;
            let info = session.info.clone();
            state.manager.sessions.lock().insert(info.id, session);
            let _ = state
                .events
                .send(Event::SessionAdded { info: info.clone() });
            Ok(serde_json::to_value(info)?)
        }
        Method::ListSessions => {
            let list = state.manager.list();
            Ok(serde_json::to_value(list)?)
        }
        Method::KillSession { id } => {
            if state.routing.owned.lock().contains(&id) {
                return Err(anyhow!("루프 그룹의 종료 버튼을 사용하세요"));
            }
            let removed = {
                let mut map = state.manager.sessions.lock();
                map.remove(&id)
            };
            if let Some(mut session) = removed {
                attached.lock().await.remove(&id);
                let _ = state.events.send(Event::SessionRemoved { id });
                // Destructors for ConPTY master / writer can block on Windows
                // (ClosePseudoConsole waits for conhost). Defer to a blocking
                // thread so the response can return immediately.
                tokio::task::spawn_blocking(move || {
                    let _ = session.killer.kill();
                    drop(session);
                });
            }
            Ok(json!(null))
        }
        Method::WriteSession { id, data } => {
            if state.routing.blocked.lock().contains(&id) {
                return Err(anyhow!("루프 그룹이 전환 또는 일시정지 중입니다"));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&data)
                .map_err(|e| anyhow!("invalid base64: {e}"))?;
            if state.routing.write_managed(&state, id, &bytes).await? {
                return Ok(json!(null));
            }
            {
                let mut map = state.manager.sessions.lock();
                let session = map
                    .get_mut(&id)
                    .ok_or_else(|| anyhow!("session not found"))?;
                session.writer.write_all(&bytes)?;
                session.writer.flush()?;
            }
            if let Some(status) = state.manager.note_input(id, &bytes) {
                let _ = state
                    .events
                    .send(Event::SessionAgentStatusChanged { id, status });
            }
            Ok(json!(null))
        }
        Method::ResizeSession { id, cols, rows } => {
            let mut map = state.manager.sessions.lock();
            let session = map
                .get_mut(&id)
                .ok_or_else(|| anyhow!("session not found"))?;
            session.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            session.info.cols = cols;
            session.info.rows = rows;
            Ok(json!(null))
        }
        Method::AttachSession { id } => {
            let (info, scrollback) = {
                let map = state.manager.sessions.lock();
                let session = map.get(&id).ok_or_else(|| anyhow!("session not found"))?;
                (
                    session.info.clone(),
                    scrollback_snapshot(&session.scrollback),
                )
            };
            attached.lock().await.insert(id);
            Ok(serde_json::to_value(AttachResult { info, scrollback })?)
        }
        Method::DetachSession { id } => {
            attached.lock().await.remove(&id);
            Ok(json!(null))
        }
        Method::RenameSession { id, name } => {
            {
                let mut map = state.manager.sessions.lock();
                let session = map
                    .get_mut(&id)
                    .ok_or_else(|| anyhow!("session not found"))?;
                session.info.name = name.clone();
            }
            let _ = state.events.send(Event::SessionRenamed { id, name });
            Ok(json!(null))
        }
        Method::KillServer => Ok(json!(null)),
    }
}

/// PTY append and publish use this same lock, so every output byte is in exactly one side of the barrier.
fn snapshot_and_subscribe(
    scrollback: &parking_lot::Mutex<std::collections::VecDeque<u8>>,
    events: &broadcast::Sender<Event>,
) -> (String, broadcast::Receiver<Event>) {
    let sb = scrollback.lock();
    let receiver = events.subscribe();
    let bytes: Vec<u8> = sb.iter().copied().collect();
    (
        base64::engine::general_purpose::STANDARD.encode(bytes),
        receiver,
    )
}

#[cfg(all(test, windows))]
mod mobile_stream_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    #[test]
    fn concurrent_snapshot_and_live_output_have_no_missing_or_duplicate_bytes() {
        let scrollback = Arc::new(parking_lot::Mutex::new(std::collections::VecDeque::new()));
        let (events, _) = broadcast::channel(2048);
        let producer_sb = scrollback.clone();
        let producer_events = events.clone();
        let start = Arc::new(std::sync::Barrier::new(2));
        let producer_start = start.clone();
        let producer = std::thread::spawn(move || {
            producer_start.wait();
            for _ in 0..1000 {
                let mut sb = producer_sb.lock();
                sb.push_back(b'x');
                producer_events
                    .send(Event::PtyOutput {
                        id: Uuid::nil(),
                        data: "eA==".into(),
                    })
                    .ok();
                drop(sb);
                std::thread::yield_now();
            }
        });
        start.wait();
        let (snapshot, mut receiver) = snapshot_and_subscribe(&scrollback, &events);
        producer.join().unwrap();
        let mut all = base64::engine::general_purpose::STANDARD
            .decode(snapshot)
            .unwrap();
        while let Ok(Event::PtyOutput { data, .. }) = receiver.try_recv() {
            all.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap(),
            );
        }
        assert_eq!(all, vec![b'x'; 1000]);
    }
    #[tokio::test]
    async fn atomic_switch_waits_for_inflight_frame_before_replacing_relay() {
        let name = format!(r"\\.\pipe\winmux-mobile-frame-test-{}", Uuid::new_v4());
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .out_buffer_size(512)
            .create(&name)
            .unwrap();
        let mut client = ClientOptions::new().open(&name).unwrap();
        server.connect().await.unwrap();
        let state = Arc::new(DaemonState::new());
        let task = tokio::spawn(handle_client(state.clone(), server));
        write_frame(
            &mut client,
            &serde_json::to_vec(&Request {
                id: 1,
                method: Method::Ping,
            })
            .unwrap(),
        )
        .await
        .unwrap();
        read_frame(&mut client).await.unwrap();
        state
            .events
            .send(Event::SessionRenamed {
                id: Uuid::nil(),
                name: "x".repeat(100_000),
            })
            .unwrap();
        let mut length = [0u8; 4];
        client.read_exact(&mut length).await.unwrap();
        let attach = serde_json::to_vec(&Request {
            id: 2,
            method: Method::AttachSessionAtomic { id: Uuid::nil() },
        })
        .unwrap();
        client
            .write_all(&(attach.len() as u32).to_le_bytes())
            .await
            .unwrap();
        client.write_all(&attach).await.unwrap();
        client.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        let mut body = vec![0u8; u32::from_le_bytes(length) as usize];
        tokio::time::timeout(Duration::from_secs(2), client.read_exact(&mut body))
            .await
            .unwrap()
            .unwrap();
        match serde_json::from_slice::<ServerMsg>(&body).unwrap() {
            ServerMsg::Event(Event::SessionRenamed { name, .. }) => assert_eq!(name.len(), 100_000),
            _ => panic!("expected intact output frame"),
        }
        let response = tokio::time::timeout(Duration::from_secs(2), read_frame(&mut client))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            serde_json::from_slice::<ServerMsg>(&response).unwrap(),
            ServerMsg::Error { id: 2, .. }
        ));
        drop(client);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
    }
}
