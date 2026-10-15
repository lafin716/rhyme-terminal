pub mod activity;
pub mod agent;
pub mod agent_status;
pub mod manager;

use anyhow::{anyhow, Result};
use base64::Engine;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ipc::protocol::Event;
use crate::pty::activity::detect_activity;
use crate::pty::agent_status::{AgentTaskEvent, AgentTaskTracker};

const SCROLLBACK_BYTES: usize = 1_000_000;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Terminal,
    Claude,
    Codex,
}

impl Default for AgentKind {
    fn default() -> Self {
        Self::Terminal
    }
}

impl AgentKind {
    fn executable_name(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionInfo {
    pub id: Uuid,
    pub name: String,
    pub shell: String,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
    #[serde(default)]
    pub agent: AgentKind,
}

pub struct Session {
    pub info: SessionInfo,
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub killer: Box<dyn ChildKiller + Send + Sync>,
    pub scrollback: Arc<Mutex<VecDeque<u8>>>,
    pub agent_kind: Arc<Mutex<AgentKind>>,
    pub task_tracker: Arc<Mutex<AgentTaskTracker>>,
    shell_pid: u32,
}

pub fn spawn_session(
    events: broadcast::Sender<Event>,
    name: String,
    shell: String,
    shell_args: Vec<String>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<Session> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| anyhow!("openpty failed: {e}"))?;

    let effective_cwd = cwd
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Path::new)
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok());

    let mut cmd = CommandBuilder::new(&shell);
    cmd.args(shell_args);
    if let Some(cwd) = effective_cwd.as_deref() {
        if !cwd.is_dir() {
            return Err(anyhow!(
                "working directory does not exist: {}",
                cwd.display()
            ));
        }
        cmd.cwd(cwd);
    }
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| anyhow!("spawn shell {shell:?} failed: {e}"))?;
    drop(pair.slave);

    let shell_pid = child
        .process_id()
        .ok_or_else(|| anyhow!("spawned shell did not report a process ID"))?;
    let killer = child.clone_killer();
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| anyhow!("take_writer failed: {e}"))?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| anyhow!("try_clone_reader failed: {e}"))?;

    let id = Uuid::new_v4();
    let scrollback = Arc::new(Mutex::new(VecDeque::with_capacity(SCROLLBACK_BYTES)));
    let agent_kind = Arc::new(Mutex::new(AgentKind::Terminal));
    let task_tracker = Arc::new(Mutex::new(AgentTaskTracker::new()));

    let scrollback_clone = scrollback.clone();
    let agent_kind_clone = agent_kind.clone();
    let task_tracker_clone = task_tracker.clone();
    let events_clone = events.clone();
    thread::Builder::new()
        .name(format!("pty-reader-{id}"))
        .spawn(move || {
            let mut buf = vec![0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = &buf[..n];
                        // Detect activity on the raw bytes before base64 encoding.
                        // This is the only backend spot that sees every Session's
                        // output regardless of attach, so the signal is emitted on
                        // an attach-independent event (see server forward filter).
                        let sig = detect_activity(chunk);
                        {
                            let mut sb = scrollback_clone.lock();
                            sb.extend(chunk.iter().copied());
                            while sb.len() > SCROLLBACK_BYTES {
                                sb.pop_front();
                            }
                        }
                        let encoded = base64::engine::general_purpose::STANDARD.encode(chunk);
                        let _ = events_clone.send(Event::PtyOutput { id, data: encoded });
                        let _ = events_clone.send(Event::SessionActivity { id, bell: sig.bell });
                        let agent = *agent_kind_clone.lock();
                        if let Some(status) = task_tracker_clone.lock().observe(agent, AgentTaskEvent::Output(chunk)) {
                            let _ = events_clone.send(Event::SessionAgentStatusChanged { id, status });
                        }
                    }
                    Err(_) => break,
                }
            }
            let status = child.wait().ok().and_then(|s| {
                let code = s.exit_code();
                if code > i32::MAX as u32 {
                    None
                } else {
                    Some(code as i32)
                }
            });
            let _ = events_clone.send(Event::PtyExit { id, status });
        })
        .map_err(|e| anyhow!("failed to spawn reader thread: {e}"))?;

    let info = SessionInfo {
        id,
        name,
        shell,
        cwd: effective_cwd.map(|value| value.to_string_lossy().into_owned()),
        cols,
        rows,
        agent: AgentKind::Terminal,
    };
    Ok(Session {
        info,
        master: pair.master,
        writer,
        killer,
        scrollback,
        agent_kind,
        task_tracker,
        shell_pid,
    })
}

pub fn scrollback_snapshot(scrollback: &Arc<Mutex<VecDeque<u8>>>) -> String {
    let sb = scrollback.lock();
    let bytes: Vec<u8> = sb.iter().copied().collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}
