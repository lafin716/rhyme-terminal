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
        AgentTaskEvent::Input(bytes) if is_task_input(bytes) => Some(AgentTaskStatus::Working),
        AgentTaskEvent::Input(_) => current,
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
    Input(&'a [u8]),
    Output(&'a [u8]),
    Idle,
}

pub struct AgentTaskTracker {
    status: Option<AgentTaskStatus>,
    last_activity: Option<Instant>,
    claude_title: super::claude_title::ClaudeTitle,
    last_agent: AgentKind,
}

impl AgentTaskTracker {
    pub fn new() -> Self {
        Self {
            status: None,
            last_activity: None,
            claude_title: Default::default(),
            last_agent: AgentKind::Terminal,
        }
    }

    pub fn observe(
        &mut self,
        agent: AgentKind,
        event: AgentTaskEvent<'_>,
    ) -> Option<AgentTaskStatus> {
        self.update_agent(agent);
        if let AgentTaskEvent::Output(bytes) = event {
            self.claude_title.feed(bytes);
        }
        if agent == AgentKind::Claude {
            if let Some(status) = self.claude_title.status {
                return self.set(Some(status));
            }
        }
        let updates_activity = match event {
            AgentTaskEvent::Input(bytes) => is_task_input(bytes),
            AgentTaskEvent::Output(chunk) => !chunk.is_empty(),
            AgentTaskEvent::Idle => false,
        };
        if updates_activity {
            self.last_activity = Some(Instant::now());
        }
        self.set(transition(self.status, agent, event))
    }

    pub fn complete_if_idle(
        &mut self,
        agent: AgentKind,
        timeout: Duration,
    ) -> Option<AgentTaskStatus> {
        self.update_agent(agent);
        // An explicit Claude title signal takes precedence over the output-idle
        // heuristic, including long, silent tool calls and late agent detection.
        if agent == AgentKind::Claude {
            if let Some(status) = self.claude_title.status {
                return self.set(Some(status));
            }
        }
        if self.last_activity.is_some_and(|at| at.elapsed() >= timeout) {
            self.set(transition(self.status, agent, AgentTaskEvent::Idle))
        } else {
            None
        }
    }

    fn update_agent(&mut self, agent: AgentKind) {
        if self.last_agent != AgentKind::Terminal && self.last_agent != agent {
            self.claude_title = Default::default();
        }
        self.last_agent = agent;
    }

    fn set(&mut self, next: Option<AgentTaskStatus>) -> Option<AgentTaskStatus> {
        if self.status == next {
            return None;
        }
        self.status = next;
        next
    }
}

impl Default for AgentTaskTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn is_task_input(mut bytes: &[u8]) -> bool {
    // xterm sends focus-in/out reports through the same onData path as typing.
    // They still reach the PTY, but must not start work or extend its idle timer.
    while bytes.starts_with(b"\x1b[I") || bytes.starts_with(b"\x1b[O") {
        bytes = &bytes[3..];
    }
    !bytes.is_empty()
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
    fn claude_title_recovers_work_and_survives_silent_tool_execution() {
        let mut tracker = super::AgentTaskTracker::new();
        tracker.observe(AgentKind::Claude, AgentTaskEvent::Input(b"run\r"));
        tracker.complete_if_idle(AgentKind::Claude, std::time::Duration::ZERO);
        assert_eq!(
            tracker.observe(
                AgentKind::Claude,
                AgentTaskEvent::Output("\x1b]0;◐ Task\x07".as_bytes())
            ),
            Some(AgentTaskStatus::Working)
        );
        assert_eq!(
            tracker.complete_if_idle(AgentKind::Claude, std::time::Duration::ZERO),
            None
        );
        assert_eq!(tracker.status, Some(AgentTaskStatus::Working));
        assert_eq!(
            tracker.observe(
                AgentKind::Claude,
                AgentTaskEvent::Output("\x1b]0;✳ Task\x07".as_bytes())
            ),
            Some(AgentTaskStatus::Completed)
        );
        assert_eq!(
            tracker.observe(AgentKind::Claude, AgentTaskEvent::Input(b"\x1b[I")),
            None
        );
        assert_eq!(tracker.status, Some(AgentTaskStatus::Completed));
    }

    #[test]
    fn claude_title_survives_chunk_boundaries_and_late_process_detection() {
        let mut tracker = super::AgentTaskTracker::new();
        for byte in "\x1b]2;⠂ Task\x1b\\".as_bytes() {
            tracker.observe(AgentKind::Terminal, AgentTaskEvent::Output(&[*byte]));
        }
        assert_eq!(
            tracker.complete_if_idle(AgentKind::Claude, std::time::Duration::ZERO),
            Some(AgentTaskStatus::Working)
        );
        assert_eq!(
            tracker.observe(
                AgentKind::Claude,
                AgentTaskEvent::Output("\x1b]2;✱ Task\x1b\\".as_bytes())
            ),
            Some(AgentTaskStatus::Completed)
        );
    }

    #[test]
    fn ordinary_redraw_and_title_text_cannot_start_claude_work() {
        let mut tracker = super::AgentTaskTracker::new();
        for bytes in [
            "◐ Task",
            "\x1b[2J\x1b[H",
            "\x1b]0;Project ◐\x07",
            "\x1b]8;;https://example.com/◐\x07",
        ] {
            assert_eq!(
                tracker.observe(AgentKind::Claude, AgentTaskEvent::Output(bytes.as_bytes())),
                None
            );
        }
        assert_eq!(tracker.status, None);
    }

    #[test]
    fn claude_title_handles_every_split_and_only_changes_on_complete_signals() {
        for title in ["\x1b]0;◑ Task\x07", "\x1b]2;⠄ Task\x1b\\"] {
            for split in 0..title.len() {
                let mut tracker = super::AgentTaskTracker::new();
                assert_eq!(
                    tracker.observe(
                        AgentKind::Claude,
                        AgentTaskEvent::Output(&title.as_bytes()[..split])
                    ),
                    None
                );
                assert_eq!(
                    tracker.observe(
                        AgentKind::Claude,
                        AgentTaskEvent::Output(&title.as_bytes()[split..])
                    ),
                    Some(AgentTaskStatus::Working)
                );
                assert_eq!(
                    tracker.observe(AgentKind::Claude, AgentTaskEvent::Output(title.as_bytes())),
                    None
                );
            }
        }
    }

    #[test]
    fn claude_title_discards_oversized_osc_and_recovers() {
        let mut tracker = super::AgentTaskTracker::new();
        let oversized = format!("\x1b]0;◐ {}\x07", "x".repeat(2048));
        assert_eq!(
            tracker.observe(
                AgentKind::Claude,
                AgentTaskEvent::Output(oversized.as_bytes())
            ),
            None
        );
        assert_eq!(
            tracker.observe(
                AgentKind::Claude,
                AgentTaskEvent::Output("\x1b]0;◐ Task\x07".as_bytes())
            ),
            Some(AgentTaskStatus::Working)
        );
    }

    #[test]
    fn claude_title_is_scoped_to_claude_and_cleared_on_agent_exit() {
        let mut tracker = super::AgentTaskTracker::new();
        assert_eq!(
            tracker.observe(
                AgentKind::Codex,
                AgentTaskEvent::Output("\x1b]0;◐ Task\x07".as_bytes())
            ),
            None
        );
        assert_eq!(
            tracker.observe(AgentKind::Claude, AgentTaskEvent::Output(b"redraw")),
            None
        );
        tracker.observe(
            AgentKind::Claude,
            AgentTaskEvent::Output("\x1b]0;◐ Task\x07".as_bytes()),
        );
        tracker.complete_if_idle(AgentKind::Terminal, std::time::Duration::ZERO);
        tracker.observe(AgentKind::Terminal, AgentTaskEvent::Output(b"shell prompt"));
        assert_eq!(
            tracker.observe(AgentKind::Claude, AgentTaskEvent::Output(b"welcome")),
            None
        );
        assert_eq!(tracker.status, None);
    }

    #[test]
    fn focus_reports_preserve_agent_status() {
        for agent in [AgentKind::Claude, AgentKind::Codex] {
            for current in [
                None,
                Some(AgentTaskStatus::Completed),
                Some(AgentTaskStatus::Error),
                Some(AgentTaskStatus::Working),
            ] {
                for bytes in [b"\x1b[I".as_slice(), b"\x1b[O", b"\x1b[O\x1b[I", b""] {
                    assert_eq!(
                        transition(current, agent, AgentTaskEvent::Input(bytes)),
                        current,
                        "agent={agent:?}, input={bytes:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn focus_reports_do_not_reset_activity_or_emit_status_changes() {
        let mut tracker = super::AgentTaskTracker::new();
        tracker.observe(AgentKind::Codex, AgentTaskEvent::Input(b"run\r"));
        tracker.last_activity = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
        let last_activity = tracker.last_activity;
        assert_eq!(
            tracker.observe(AgentKind::Codex, AgentTaskEvent::Input(b"\x1b[O")),
            None
        );
        assert_eq!(tracker.last_activity, last_activity);
        tracker.complete_if_idle(AgentKind::Codex, std::time::Duration::ZERO);
        assert_eq!(
            tracker.observe(AgentKind::Codex, AgentTaskEvent::Input(b"\x1b[I")),
            None
        );
        assert_eq!(tracker.status, Some(AgentTaskStatus::Completed));
    }

    #[test]
    fn real_input_alongside_focus_reports_still_starts_work() {
        for bytes in [
            b"\x1b[Irun\r".as_slice(),
            b"run\r\x1b[O",
            b"\r",
            b"\x1b[A",
            "안녕".as_bytes(),
        ] {
            assert_eq!(
                transition(None, AgentKind::Claude, AgentTaskEvent::Input(bytes)),
                Some(AgentTaskStatus::Working)
            );
        }
    }

    #[test]
    fn marks_an_agent_working_when_it_receives_input() {
        assert_eq!(
            transition(None, AgentKind::Codex, AgentTaskEvent::Input(b"run\r")),
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
            transition(
                None,
                AgentKind::Claude,
                AgentTaskEvent::Output(b"\x1b[2J\x1b[H")
            ),
            None,
        );
    }

    #[test]
    fn ignores_regular_terminal_input() {
        assert_eq!(
            transition(None, AgentKind::Terminal, AgentTaskEvent::Input(b"ls\r")),
            None,
        );
    }
}
