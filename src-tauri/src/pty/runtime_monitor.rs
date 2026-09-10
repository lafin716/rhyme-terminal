//! Shared process evidence for the sidebar and Agent Loop. Task/turn activity
//! remains a separate concern in AgentTaskTracker.
use super::{
    agent::{ProcessEntry, ProcessTree},
    AgentKind,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeStatus {
    #[default]
    Idle,
    Starting,
    Running,
    Interrupting,
    Exited,
    Error,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeState {
    pub status: RuntimeStatus,
    pub provider: Option<String>,
    pub pid: Option<u32>,
    pub started_at: Option<u64>,
}
#[derive(Default)]
pub struct AgentRuntimeMonitor {
    pub states: HashMap<Uuid, AgentRuntimeState>,
    pub processes: Vec<ProcessEntry>,
    pub revision: u64,
}
impl AgentRuntimeMonitor {
    pub fn observe(&mut self, id: Uuid, shell_pid: u32, processes: &[ProcessEntry]) -> AgentKind {
        self.observe_tree(id, shell_pid, &ProcessTree::new(processes))
    }
    pub fn observe_tree(&mut self, id: Uuid, shell_pid: u32, tree: &ProcessTree<'_>) -> AgentKind {
        let found = tree.runtime_for_shell(shell_pid);
        let previous = self.states.get(&id);
        let state = match &found {
            Some((process, agent)) => AgentRuntimeState {
                status: if previous.is_some_and(|s| {
                    s.pid == Some(process.pid) && s.started_at == Some(process.started_at)
                }) {
                    RuntimeStatus::Running
                } else {
                    RuntimeStatus::Starting
                },
                provider: Some(agent.executable_name().to_string()),
                pid: Some(process.pid),
                started_at: Some(process.started_at),
            },
            None => AgentRuntimeState {
                status: if previous.is_some_and(|s| s.pid.is_some()) {
                    RuntimeStatus::Exited
                } else {
                    RuntimeStatus::Idle
                },
                ..Default::default()
            },
        };
        self.states.insert(id, state);
        found.map(|(_, agent)| agent).unwrap_or(AgentKind::Terminal)
    }
    pub fn get_state(&self, id: Uuid) -> AgentRuntimeState {
        self.states.get(&id).cloned().unwrap_or_default()
    }
    pub fn alive(&self, pid: u32) -> bool {
        self.processes.iter().any(|p| p.pid == pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shares_provider_detection_and_tracks_pid_replacement_and_exit() {
        let id = Uuid::new_v4();
        let mut monitor = AgentRuntimeMonitor::default();
        let mut processes = vec![ProcessEntry {
            pid: 2,
            parent_pid: Some(1),
            image_name: "codex.exe".into(),
            command_args: vec!["codex".into(), "resume".into()],
            started_at: 10,
        }];
        assert_eq!(monitor.observe(id, 1, &processes), AgentKind::Codex);
        assert_eq!(monitor.get_state(id).status, RuntimeStatus::Starting);
        monitor.observe(id, 1, &processes);
        assert_eq!(monitor.get_state(id).status, RuntimeStatus::Running);
        processes[0].started_at = 11;
        monitor.observe(id, 1, &processes);
        assert_eq!(monitor.get_state(id).status, RuntimeStatus::Starting);
        assert_eq!(monitor.observe(id, 1, &[]), AgentKind::Terminal);
        assert_eq!(monitor.get_state(id).status, RuntimeStatus::Exited);
        monitor.observe(id, 1, &[]);
        assert_eq!(monitor.get_state(id).status, RuntimeStatus::Idle);
    }
}
