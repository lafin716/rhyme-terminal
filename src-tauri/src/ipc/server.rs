use anyhow::{anyhow, Result};
use base64::Engine;
use portable_pty::PtySize;
use serde_json::json;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::broadcast;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};
use uuid::Uuid;

use super::protocol::{AttachResult, Event, Method, Request, ServerMsg};
use super::{pipe_name, read_frame, write_frame};
use crate::pty::agent::process_snapshot;
use crate::pty::manager::SessionManager;
use crate::pty::{scrollback_snapshot, spawn_session};

const DEFAULT_SHELL: &str = "powershell.exe";

pub struct DaemonState {
    pub manager: Arc<SessionManager>,
    pub events: broadcast::Sender<Event>,
}

impl DaemonState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            manager: Arc::new(SessionManager::new()),
            events: tx,
        }
    }
}

pub async fn run_server(state: Arc<DaemonState>) -> Result<()> {
    let name = pipe_name();

    let first = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .map_err(|e| {
            anyhow!("failed to bind named pipe {name}: {e} (another daemon already running?)")
        })?;
    info!("daemon listening on {name}");

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
            for (id, status) in monitor_state.manager.complete_idle_tasks(Duration::from_secs(2)) {
                let _ = monitor_state
                    .events
                    .send(Event::SessionAgentStatusChanged { id, status });
            }
        }
    });

    let mut server = first;
    loop {
        server.connect().await?;
        let connected = server;

        server = ServerOptions::new()
            .create(&name)
            .map_err(|e| anyhow!("failed to create next pipe instance: {e}"))?;

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

    let mut events_rx = state.events.subscribe();
    let writer_evt = writer.clone();
    let attached_evt = attached.clone();
    let event_task = tokio::spawn(async move {
        loop {
            let ev = match events_rx.recv().await {
                Ok(ev) => ev,
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            };
            let forward = match &ev {
                Event::PtyOutput { id, .. } => attached_evt.lock().await.contains(id),
                Event::PtyExit { id, .. } => attached_evt.lock().await.contains(id),
                _ => true,
            };
            if !forward {
                continue;
            }
            let msg = ServerMsg::Event(ev);
            let bytes = match serde_json::to_vec(&msg) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let mut w = writer_evt.lock().await;
            if write_frame(&mut *w, &bytes).await.is_err() {
                break;
            }
        }
    });

    let mut reader = read_half;
    let result: Result<()> = async {
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
            let kill_server = matches!(req.method, Method::KillServer);
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
    }
    .await;

    event_task.abort();
    result
}

async fn dispatch(
    state: Arc<DaemonState>,
    attached: Arc<AsyncMutex<HashSet<Uuid>>>,
    req: Request,
) -> Result<serde_json::Value> {
    match req.method {
        Method::Ping => Ok(json!("pong")),
        Method::CreateSession {
            name,
            shell,
            shell_args,
            cwd,
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
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&data)
                .map_err(|e| anyhow!("invalid base64: {e}"))?;
            {
                let mut map = state.manager.sessions.lock();
                let session = map
                    .get_mut(&id)
                    .ok_or_else(|| anyhow!("session not found"))?;
                session.writer.write_all(&bytes)?;
                session.writer.flush()?;
            }
            if let Some(status) = state.manager.note_input(id) {
                let _ = state.events.send(Event::SessionAgentStatusChanged { id, status });
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
