use parking_lot::Mutex;
use std::collections::HashMap;
use uuid::Uuid;

use super::agent::ProcessEntry;
use super::agent_status::{AgentTaskEvent, AgentTaskStatus};
use super::runtime_monitor::AgentRuntimeMonitor;
use super::{AgentKind, Session, SessionInfo};

pub struct SessionManager {
    pub sessions: Mutex<HashMap<Uuid, Session>>,
    pub runtime: Mutex<AgentRuntimeMonitor>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            runtime: Mutex::new(AgentRuntimeMonitor::default()),
        }
    }

    pub fn kill_all(&self) {
        let mut map = self.sessions.lock();
        for (_, session) in map.iter_mut() {
            let _ = session.killer.kill();
        }
        map.clear();
        *self.runtime.lock() = AgentRuntimeMonitor::default();
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        self.sessions
            .lock()
            .values()
            .map(|s| s.info.clone())
            .collect()
    }

    pub fn has_sessions(&self) -> bool {
        !self.sessions.lock().is_empty()
    }

    pub fn refresh_agents(&self, processes: &[ProcessEntry]) -> Vec<(Uuid, AgentKind)> {
        let changes = {
            let mut sessions = self.sessions.lock();
            let mut runtime = self.runtime.lock();
            runtime.states.retain(|id, _| sessions.contains_key(id));
            if sessions.is_empty() {
                runtime.processes.clear();
                runtime.revision += 1;
                return Vec::new();
            }
            runtime.processes = processes.to_vec();
            runtime.revision += 1;
            let tree = super::agent::ProcessTree::new(processes);
            sessions
                .iter_mut()
                .filter_map(|(id, session)| {
                    let agent = runtime.observe_tree(*id, session.shell_pid, &tree);
                    if session.info.agent == agent {
                        None
                    } else {
                        session.info.agent = agent;
                        *session.agent_kind.lock() = agent;
                        Some((*id, agent))
                    }
                })
                .collect()
        };

        changes
    }

    pub fn note_input(&self, id: Uuid, bytes: &[u8]) -> Option<AgentTaskStatus> {
        let map = self.sessions.lock();
        let session = map.get(&id)?;
        let agent = *session.agent_kind.lock();
        let status = session
            .task_tracker
            .lock()
            .observe(agent, AgentTaskEvent::Input(bytes));
        status
    }

    pub fn complete_idle_tasks(
        &self,
        timeout: std::time::Duration,
    ) -> Vec<(Uuid, AgentTaskStatus)> {
        self.sessions
            .lock()
            .iter()
            .filter_map(|(id, session)| {
                let agent = *session.agent_kind.lock();
                session
                    .task_tracker
                    .lock()
                    .complete_if_idle(agent, timeout)
                    .map(|status| (*id, status))
            })
            .collect()
    }

    pub fn next_default_name(&self) -> String {
        let n = self.sessions.lock().len() + 1;
        format!("session-{n}")
    }

    pub fn resolve_target(&self, target: &str) -> Option<Uuid> {
        let map = self.sessions.lock();
        for (id, s) in map.iter() {
            if s.info.name == target {
                return Some(*id);
            }
        }
        for (id, _) in map.iter() {
            if id.to_string().starts_with(target) {
                return Some(*id);
            }
        }
        None
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::SessionManager;
    use crate::pty::agent::ProcessEntry;
    use crate::pty::spawn_session;
    use tokio::sync::broadcast;

    fn entry(pid: u32, parent_pid: Option<u32>, image_name: &str, started_at: u64) -> ProcessEntry {
        ProcessEntry {
            pid,
            parent_pid,
            image_name: image_name.to_string(),
            command_args: vec![image_name.to_string()],
            started_at,
        }
    }

    #[test]
    fn has_sessions_is_false_for_a_new_manager() {
        assert!(!SessionManager::new().has_sessions());
    }

    #[test]
    fn refresh_agents_reports_a_change_only_once() {
        let (events, _) = broadcast::channel(1);
        let manager = SessionManager::new();
        let session = spawn_session(
            events,
            "test".to_string(),
            "cmd.exe".to_string(),
            vec!["/C".to_string(), "exit".to_string()],
            None,
            None,
            80,
            24,
        )
        .expect("test session should spawn");
        let id = session.info.id;
        let shell_pid = session.shell_pid;
        manager.sessions.lock().insert(id, session);
        assert!(manager.has_sessions());

        let processes = [
            entry(shell_pid, None, "cmd.exe", 1),
            entry(2, Some(shell_pid), "claude.exe", 2),
        ];

        assert_eq!(
            manager.refresh_agents(&processes),
            vec![(id, crate::pty::AgentKind::Claude)]
        );
        assert!(manager.refresh_agents(&processes).is_empty());

        let shell_only = [entry(shell_pid, None, "cmd.exe", 1)];
        assert_eq!(
            manager.refresh_agents(&shell_only),
            vec![(id, crate::pty::AgentKind::Terminal)]
        );
        assert_eq!(manager.list()[0].agent, crate::pty::AgentKind::Terminal);
        assert!(manager.refresh_agents(&shell_only).is_empty());

        manager.kill_all();
        assert!(!manager.has_sessions());
        assert!(manager.runtime.lock().states.is_empty());
        assert!(manager.runtime.lock().processes.is_empty());
    }
}
