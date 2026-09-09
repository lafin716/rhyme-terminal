use super::transport::{self, Client as NamedPipeClient};
use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};
use tokio::time::sleep;

use super::protocol::{Event, Method, Request, ServerMsg};
use super::{pipe_name, read_frame, write_frame};

struct Reply {
    value: serde_json::Value,
    events: broadcast::Receiver<Event>,
}
type Pending = Arc<parking_lot::Mutex<HashMap<u64, oneshot::Sender<Result<Reply>>>>>;
struct PendingGuard {
    pending: Pending,
    id: u64,
}
impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.pending.lock().remove(&self.id);
    }
}
struct ReaderTask(tokio::task::AbortHandle);
impl Drop for ReaderTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub struct DaemonClient {
    writer: AsyncMutex<tokio::io::WriteHalf<NamedPipeClient>>,
    pending: Pending,
    events: broadcast::Sender<Event>,
    next_id: AtomicU64,
    _reader: ReaderTask,
    disconnected: tokio::sync::watch::Receiver<bool>,
}

impl DaemonClient {
    pub async fn connect_or_spawn() -> Result<Arc<Self>> {
        let name = pipe_name();
        let mut spawned = false;
        for attempt in 0..50 {
            match transport::connect(&name).await {
                Ok(client) => return Self::from_pipe(client).await,
                Err(_) => {
                    if !spawned {
                        spawn_daemon_detached()?;
                        spawned = true;
                    }
                    if attempt == 49 {
                        bail!("daemon unreachable at {name}");
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
        bail!("daemon unreachable at {name}")
    }

    pub async fn connect_existing() -> Result<Arc<Self>> {
        let name = pipe_name();
        let client = transport::connect(&name)
            .await
            .map_err(|e| anyhow!("daemon not running at {name}: {e}"))?;
        Self::from_pipe(client).await
    }

    pub async fn connect_mobile() -> Result<Arc<Self>> {
        let client = transport::connect(&pipe_name()).await?;
        Self::from_pipe_capacity(client, 64).await
    }

    async fn from_pipe(pipe: NamedPipeClient) -> Result<Arc<Self>> {
        Self::from_pipe_capacity(pipe, 1024).await
    }

    pub(crate) async fn from_pipe_capacity(
        pipe: NamedPipeClient,
        capacity: usize,
    ) -> Result<Arc<Self>> {
        let (read_half, write_half) = tokio::io::split(pipe);
        let (events_tx, _) = broadcast::channel(capacity);
        let pending: Pending = Arc::new(parking_lot::Mutex::new(HashMap::new()));
        let events_client = events_tx.clone();

        let (disconnected_tx, disconnected) = tokio::sync::watch::channel(false);
        let pending_bg = pending.clone();
        let reader_task = tokio::spawn(async move {
            let mut reader = read_half;
            loop {
                let bytes = match read_frame(&mut reader).await {
                    Ok(b) => b,
                    Err(_) => break,
                };
                let msg: ServerMsg = match serde_json::from_slice(&bytes) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                match msg {
                    ServerMsg::Response { id, result } => {
                        if let Some(tx) = pending_bg.lock().remove(&id) {
                            let _ = tx.send(Ok(Reply {
                                value: result,
                                events: events_tx.subscribe(),
                            }));
                        }
                    }
                    ServerMsg::Error { id, message } => {
                        if let Some(tx) = pending_bg.lock().remove(&id) {
                            let _ = tx.send(Err(anyhow!(message)));
                        }
                    }
                    ServerMsg::Event(ev) => {
                        let _ = events_tx.send(ev);
                    }
                }
            }
            disconnected_tx.send_replace(true);
            let mut p = pending_bg.lock();
            for (_, tx) in p.drain() {
                let _ = tx.send(Err(anyhow!("daemon disconnected")));
            }
        });

        Ok(Arc::new(Self {
            writer: AsyncMutex::new(write_half),
            pending,
            events: events_client,
            next_id: AtomicU64::new(1),
            _reader: ReaderTask(reader_task.abort_handle()),
            disconnected,
        }))
    }

    pub async fn request_raw(&self, method: Method) -> Result<serde_json::Value> {
        self.request_with_events(method)
            .await
            .map(|(value, _)| value)
    }

    /// Subscribe in the pipe reader at the response boundary, before subsequent live events.
    pub async fn request_with_events(
        &self,
        method: Method,
    ) -> Result<(serde_json::Value, broadcast::Receiver<Event>)> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request { id, method };
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(id, tx);
        let _guard = PendingGuard {
            pending: self.pending.clone(),
            id,
        };
        let bytes = serde_json::to_vec(&req)?;
        {
            let mut w = self.writer.lock().await;
            write_frame(&mut *w, &bytes).await?;
        }
        let reply = rx.await.map_err(|_| anyhow!("response channel closed"))??;
        Ok((reply.value, reply.events))
    }

    pub async fn request<T: DeserializeOwned>(&self, method: Method) -> Result<T> {
        let value = self.request_raw(method).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn disconnected(&self) {
        let mut rx = self.disconnected.clone();
        let closed = *rx.borrow();
        if !closed {
            let _ = rx.changed().await;
        }
    }

    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
}

fn spawn_daemon_detached() -> Result<()> {
    let (exe, arg) = daemon_command()?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = std::process::Command::new(&exe);
        if let Some(arg) = arg {
            command.arg(arg);
        }
        command
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| anyhow!("failed to spawn daemon: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        let mut command = std::process::Command::new(&exe);
        if let Some(arg) = arg {
            command.arg(arg);
        }
        command
            .spawn()
            .map_err(|e| anyhow!("failed to spawn daemon: {e}"))?;
    }
    Ok(())
}

fn daemon_command() -> Result<(PathBuf, Option<&'static str>)> {
    let current = std::env::current_exe()?;
    let dir = current
        .parent()
        .ok_or_else(|| anyhow!("current exe has no parent dir"))?;
    let name = if cfg!(windows) {
        "winmuxd.exe"
    } else {
        "winmuxd"
    };
    let standalone_daemon = dir.join(name);

    if standalone_daemon.exists() {
        return Ok((standalone_daemon, None));
    }

    let current_name = current
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if current_name.eq_ignore_ascii_case("rhyme-terminal")
        || current_name.eq_ignore_ascii_case("winmux")
    {
        return Ok((current, Some(crate::DAEMON_ARG)));
    }

    bail!(
        "winmuxd executable not found at {}",
        standalone_daemon.display()
    )
}

#[cfg(all(test, windows))]
mod mobile_lifetime_tests {
    use super::*;
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    async fn pair() -> (
        Arc<DaemonClient>,
        tokio::net::windows::named_pipe::NamedPipeServer,
    ) {
        let name = format!(r"\\.\pipe\winmux-test-{}", uuid::Uuid::new_v4());
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&name)
            .unwrap();
        let pipe = ClientOptions::new().open(&name).unwrap();
        server.connect().await.unwrap();
        (DaemonClient::from_pipe(pipe).await.unwrap(), server)
    }
    #[tokio::test]
    async fn response_receiver_excludes_old_output_and_preserves_immediate_new_output() {
        let (client, mut server) = pair().await;
        let server_task = tokio::spawn(async move {
            let request: Request =
                serde_json::from_slice(&read_frame(&mut server).await.unwrap()).unwrap();
            for message in [
                ServerMsg::Event(Event::PtyOutput {
                    id: uuid::Uuid::nil(),
                    data: "b2xk".into(),
                }),
                ServerMsg::Response {
                    id: request.id,
                    result: serde_json::json!({"scrollback":"b2xk"}),
                },
                ServerMsg::Event(Event::PtyOutput {
                    id: uuid::Uuid::nil(),
                    data: "bmV3".into(),
                }),
            ] {
                write_frame(&mut server, &serde_json::to_vec(&message).unwrap())
                    .await
                    .unwrap();
            }
        });
        let (value, mut events) = client
            .request_with_events(Method::AttachSessionAtomic {
                id: uuid::Uuid::nil(),
            })
            .await
            .unwrap();
        assert_eq!(value["scrollback"], "b2xk");
        match tokio::time::timeout(Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .unwrap()
        {
            Event::PtyOutput { data, .. } => assert_eq!(data, "bmV3"),
            _ => panic!("wrong event"),
        }
        assert!(events.try_recv().is_err());
        server_task.await.unwrap();
    }
    #[tokio::test]
    async fn cancelling_request_removes_pending_and_dropping_client_closes_pipe() {
        let (client, mut server) = pair().await;
        let pending = client.pending.clone();
        let request = client.request_raw(Method::Ping);
        assert!(tokio::time::timeout(Duration::from_millis(30), request)
            .await
            .is_err());
        assert!(pending.lock().is_empty());
        let _ = read_frame(&mut server).await.unwrap();
        drop(client);
        assert!(
            tokio::time::timeout(Duration::from_secs(2), read_frame(&mut server))
                .await
                .unwrap()
                .is_err()
        );
    }
}
