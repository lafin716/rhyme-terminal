use super::AgentKind;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Working,
    Completed,
    Error,
}

pub fn transition(
    current: Option<AgentTaskStatus>,
    agent: AgentKind,
    event: AgentTaskEvent<'_>,
) -> Option<AgentTaskStatus> {
    if agent == AgentKind::Terminal {
        return None;
    }

    match event {
        AgentTaskEvent::Input => Some(AgentTaskStatus::Working),
        AgentTaskEvent::Output(chunk)
            if current == Some(AgentTaskStatus::Working) && contains_error(chunk) =>
        {
            Some(AgentTaskStatus::Error)
        }
        AgentTaskEvent::Output(_) => current,
        AgentTaskEvent::Idle if current == Some(AgentTaskStatus::Working) => {
            Some(AgentTaskStatus::Completed)
        }
        AgentTaskEvent::Idle => current,
    }
}

#[derive(Clone, Copy)]
pub enum AgentTaskEvent<'a> {
    Input,
    Output(&'a [u8]),
    Idle,
}

pub struct AgentTaskTracker {
    status: Option<AgentTaskStatus>,
    last_activity: Option<Instant>,
}

impl AgentTaskTracker {
    pub fn new() -> Self {
        Self { status: None, last_activity: None }
    }

    pub fn observe(&mut self, agent: AgentKind, event: AgentTaskEvent<'_>) -> Option<AgentTaskStatus> {
        let updates_activity = match event {
            AgentTaskEvent::Input => true,
            AgentTaskEvent::Output(chunk) => !chunk.is_empty(),
            AgentTaskEvent::Idle => false,
        };
        if updates_activity {
            self.last_activity = Some(Instant::now());
        }
        self.set(transition(self.status, agent, event))
    }

    pub fn complete_if_idle(&mut self, agent: AgentKind, timeout: Duration) -> Option<AgentTaskStatus> {
        if self.last_activity.is_some_and(|at| at.elapsed() >= timeout) {
            self.set(transition(self.status, agent, AgentTaskEvent::Idle))
        } else {
            None
        }
    }

    fn set(&mut self, next: Option<AgentTaskStatus>) -> Option<AgentTaskStatus> {
        if self.status == next { return None; }
        self.status = next;
        next
    }
}

impl Default for AgentTaskTracker {
    fn default() -> Self { Self::new() }
}

fn contains_error(chunk: &[u8]) -> bool {
    let text = String::from_utf8_lossy(chunk).to_ascii_lowercase();
    ["error:", "fatal:", "uncaught exception", "request failed"]
        .iter()
        .any(|marker| text.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::{transition, AgentTaskEvent, AgentTaskStatus};
    use crate::pty::AgentKind;

    #[test]
    fn marks_an_agent_working_when_it_receives_input() {
        assert_eq!(
            transition(None, AgentKind::Codex, AgentTaskEvent::Input),
            Some(AgentTaskStatus::Working),
        );
    }

    #[test]
    fn marks_an_active_agent_complete_after_an_idle_interval() {
        assert_eq!(
            transition(
                Some(AgentTaskStatus::Working),
                AgentKind::Claude,
                AgentTaskEvent::Idle,
            ),
            Some(AgentTaskStatus::Completed),
        );
    }

    #[test]
    fn marks_known_error_output_as_an_error() {
        assert_eq!(
            transition(
                Some(AgentTaskStatus::Working),
                AgentKind::Codex,
                AgentTaskEvent::Output(b"Error: request failed"),
            ),
            Some(AgentTaskStatus::Error),
        );
    }

    #[test]
    fn does_not_start_work_from_an_idle_agent_redraw() {
        assert_eq!(
            transition(None, AgentKind::Claude, AgentTaskEvent::Output(b"\x1b[2J\x1b[H")),
            None,
        );
    }

    #[test]
    fn ignores_regular_terminal_input() {
        assert_eq!(
            transition(None, AgentKind::Terminal, AgentTaskEvent::Input),
            None,
        );
    }
}
