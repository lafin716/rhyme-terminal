use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn default_workspace_index() -> u32 {
    1
}
fn threshold() -> f64 {
    90.0
}
fn agent_order() -> Vec<String> {
    vec!["claude".into(), "codex".into()]
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    #[serde(default)]
    pub priority: i32,
    pub agent: String,
    pub profile_id: Option<String>,
    pub label: String,
    #[serde(default)]
    pub enabled: bool,
    pub short_threshold: Option<f64>,
    pub weekly_threshold: Option<f64>,
    pub config_dir: Option<String>,
    pub auth_method: Option<String>,
    #[serde(default, skip_serializing)]
    pub env: HashMap<String, String>,
}
impl Candidate {
    pub fn key(&self) -> String {
        format!(
            "{}:{}",
            self.agent,
            self.profile_id.as_deref().unwrap_or("system")
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default = "poll_interval")]
    pub polling_interval_seconds: u64,
    #[serde(default = "interrupt_timeout")]
    pub interrupt_timeout_seconds: u64,
    #[serde(default = "kill_timeout")]
    pub force_kill_timeout_seconds: u64,
    #[serde(default = "auto_resume")]
    pub auto_resume: bool,
    #[serde(default = "threshold")]
    pub short_threshold: f64,
    #[serde(default = "threshold")]
    pub weekly_threshold: f64,
    #[serde(default = "agent_order")]
    pub agent_order: Vec<String>,
    #[serde(default)]
    pub candidates: Vec<Candidate>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            polling_interval_seconds: poll_interval(),
            interrupt_timeout_seconds: interrupt_timeout(),
            force_kill_timeout_seconds: kill_timeout(),
            auto_resume: auto_resume(),
            short_threshold: threshold(),
            weekly_threshold: threshold(),
            agent_order: agent_order(),
            candidates: vec![],
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(
                self.strategy.as_str(),
                "SMART" | "LEAST_USAGE" | "ROUND_ROBIN" | "PRIORITY"
            ),
            "Invalid selection strategy"
        );
        ensure!(
            (10..=300).contains(&self.polling_interval_seconds)
                && (1..=10).contains(&self.interrupt_timeout_seconds)
                && (1..=5).contains(&self.force_kill_timeout_seconds),
            "Invalid loop timeouts"
        );
        let valid = |n: f64| n.is_finite() && (1.0..=100.0).contains(&n);
        ensure!(
            valid(self.short_threshold) && valid(self.weekly_threshold),
            "임계값은 1~100%여야 합니다"
        );
        ensure!(
            self.agent_order.len() == 2
                && self.agent_order.contains(&"claude".into())
                && self.agent_order.contains(&"codex".into()),
            "에이전트 순서가 올바르지 않습니다"
        );
        ensure!(self.candidates.len() <= 100, "프로필이 너무 많습니다");
        let mut seen = HashSet::new();
        for c in &self.candidates {
            ensure!(
                matches!(c.agent.as_str(), "claude" | "codex") && seen.insert(c.key()),
                "지원하지 않거나 중복된 프로필입니다"
            );
            ensure!(
                c.short_threshold.map_or(true, valid) && c.weekly_threshold.map_or(true, valid),
                "프로필 임계값은 1~100%여야 합니다"
            );
            ensure!(c.label.len() <= 256, "프로필 이름이 너무 깁니다");
            for (key, value) in &c.env {
                ensure!(
                    !key.is_empty()
                        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && !value.contains('\0'),
                    "환경변수가 올바르지 않습니다"
                );
                ensure!(
                    !matches!(
                        key.to_ascii_uppercase().as_str(),
                        "CLAUDE_CONFIG_DIR" | "CODEX_HOME" | "RHYME_LOOP_ATTEMPT_DIR"
                    ),
                    "프로필 격리 환경변수는 자동 설정됩니다"
                );
            }
        }
        Ok(())
    }
    pub fn ordered(&self) -> Vec<Candidate> {
        self.candidates.iter().filter(|c| c.enabled).cloned().collect()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    pub id: String,
    pub session_id: Option<uuid::Uuid>,
    pub agent: String,
    pub profile_id: Option<String>,
    pub label: String,
    pub status: String,
    pub reason: Option<String>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub reference: Option<super::adapter::SessionReference>,
    #[serde(default)]
    pub expected_session_id: Option<String>,
    #[serde(default)]
    pub continuation: Option<String>,
    #[serde(default)]
    pub continuation_sent_at: Option<u64>,
    #[serde(default)]
    pub continuation_acknowledged: bool,
    /// True only for a new invocation that has never received input or work.
    #[serde(default)]
    pub initial_prompt: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    #[serde(default)]
    pub policy: Option<Settings>,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub runtime: crate::pty::runtime_monitor::AgentRuntimeState,
    #[serde(default)]
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: Vec<ProfileSnapshot>,
    #[serde(default)]
    pub waiting_profile_id: Option<String>,
    #[serde(default)]
    pub resume_at: Option<u64>,
    #[serde(default)]
    pub events: Vec<SystemEvent>,
    #[serde(default)]
    pub queued_input: Vec<InputChunk>,
    #[serde(default)]
    pub handoff_context: Option<String>,
    #[serde(default)]
    pub command_args: Vec<String>,
    #[serde(default)]
    pub current_provider: Option<String>,
    #[serde(default)]
    pub current_agent_session_id: Option<String>,
    pub id: uuid::Uuid,
    pub name: String,
    pub workspace_id: String,
    #[serde(default = "default_workspace_index")]
    pub workspace_index: u32,
    pub cwd: String,
    pub status: LoopStatus,
    pub active_session_id: Option<uuid::Uuid>,
    pub reason: Option<String>,
    pub attempts: Vec<Attempt>,
    pub updated_at: u64,
    #[serde(default)]
    pub pending_work: bool,
}
impl Group {
    pub fn state(&mut self, state: LoopStatus, reason: Option<&str>) {
        if self.status == state && self.reason.as_deref() == reason {
            return;
        }
        self.status = state;
        self.reason = reason.map(str::to_string);
        self.updated_at = now();
        if let Some(message) = reason {
            self.event(self.status.as_str(), message);
        }
    }
    pub fn event(&mut self, kind: &str, message: &str) {
        self.events.push(SystemEvent {
            at: now(),
            kind: kind.into(),
            message: message.into(),
        });
        if self.events.len() > 200 {
            self.events.remove(0);
        }
        self.updated_at = now();
    }
}

fn default_strategy() -> String {
    "SMART".into()
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopStatus {
    Idle,
    #[serde(alias = "starting")]
    Preparing,
    Running,
    Paused,
    #[serde(alias = "switching", alias = "switch_pending")]
    SwitchingProfile,
    Handoff,
    #[serde(alias = "waiting")]
    WaitingForUsageReset,
    Resuming,
    Stopped,
    #[serde(alias = "recovery")]
    Error,
}
impl LoopStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Preparing => "preparing",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::SwitchingProfile => "switching_profile",
            Self::Handoff => "handoff",
            Self::WaitingForUsageReset => "waiting_for_usage_reset",
            Self::Resuming => "resuming",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}
impl PartialEq<&str> for LoopStatus {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
#[cfg(test)]
impl From<&str> for LoopStatus {
    fn from(value: &str) -> Self {
        match value {
            "idle" => Self::Idle,
            "preparing" | "starting" => Self::Preparing,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "switching_profile" | "switching" | "switch_pending" => Self::SwitchingProfile,
            "handoff" => Self::Handoff,
            "waiting_for_usage_reset" | "waiting" => Self::WaitingForUsageReset,
            "resuming" => Self::Resuming,
            "stopped" => Self::Stopped,
            "error" => Self::Error,
            _ => panic!("Unknown test LoopStatus: {value}"),
        }
    }
}
fn poll_interval() -> u64 {
    30
}
fn interrupt_timeout() -> u64 {
    5
}
fn kill_timeout() -> u64 {
    3
}
fn auto_resume() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProfileStatus {
    Active,
    Available,
    NearLimit,
    Exhausted,
    RateLimited,
    WaitingReset,
    Disabled,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSnapshot {
    #[serde(default)]
    pub usage_pending: bool,
    pub key: String,
    pub agent: String,
    pub label: String,
    pub status: ProfileStatus,
    pub usage: Option<f64>,
    pub threshold: f64,
    pub remaining: Option<f64>,
    pub reset_at: Option<u64>,
    pub error: Option<String>,
    pub windows: Vec<crate::usage::UsageWindow>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemEvent {
    pub at: u64,
    pub kind: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputChunk {
    pub id: String,
    pub bytes: Vec<u8>,
    #[serde(default)]
    pub delivering: bool,
}

/// Advance from the current candidate without preempting it when a higher priority recovers.
pub fn choose(
    candidates: &[Candidate],
    after: Option<&str>,
    eligible: impl Fn(&Candidate) -> bool,
) -> Option<Candidate> {
    let start = after
        .and_then(|key| candidates.iter().position(|c| c.key() == key))
        .map_or(0, |i| i + 1);
    (0..candidates.len())
        .map(|offset| &candidates[(start + offset) % candidates.len()])
        .find(|c| eligible(c))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(id: &str) -> Candidate {
        Candidate {
            priority: 0,
            agent: "claude".into(),
            profile_id: Some(id.into()),
            label: id.into(),
            enabled: true,
            short_threshold: None,
            weekly_threshold: None,
            config_dir: None,
            auth_method: None,
            env: HashMap::new(),
        }
    }
    #[test]
    fn rotates_and_skips_exhausted_without_losing_priority() {
        let candidates = vec![c("a"), c("b"), c("c")];
        assert_eq!(
            choose(&candidates, Some("claude:a"), |c| c.label != "b")
                .unwrap()
                .label,
            "c"
        );
        assert_eq!(
            choose(&candidates, Some("claude:c"), |_| true)
                .unwrap()
                .label,
            "a"
        );
        assert!(choose(&candidates, None, |_| false).is_none());
        assert!(choose(&[], None, |_| true).is_none());
    }
    #[test]
    fn validates_overrides_and_duplicates() {
        let mut s = Settings::default();
        s.candidates = vec![c("a"), c("a")];
        assert!(s.validate().is_err());
        s.candidates.pop();
        s.candidates[0].short_threshold = Some(0.0);
        assert!(s.validate().is_err());
        s.candidates[0].short_threshold = Some(80.0);
        assert!(s.validate().is_ok());
    }
    #[test]
    fn global_profile_order_is_preserved_across_providers() {
        let mut settings = Settings::default();
        let mut codex = c("first");
        codex.agent = "codex".into();
        let mut waiting = c("waiting");
        waiting.enabled = false;
        settings.candidates = vec![waiting, codex, c("second")];
        assert_eq!(settings.ordered().iter().map(Candidate::key).collect::<Vec<_>>(), vec!["codex:first", "claude:second"]);
    }
    #[test]
    fn persisted_settings_never_include_environment_values() {
        let mut candidate = c("a");
        candidate.env.insert("SECRET".into(), "sensitive".into());
        assert!(!serde_json::to_string(&candidate)
            .unwrap()
            .contains("sensitive"));
    }
}

/// Only these per-loop fields are editable; global defaults and credentials stay untouched.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyPatch {
    pub strategy: Option<String>,
    pub polling_interval_seconds: Option<u64>,
    pub auto_resume: Option<bool>,
    pub profile: Option<ProfilePolicyPatch>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfilePolicyPatch {
    pub key: String,
    pub short_threshold: Option<f64>,
    pub weekly_threshold: Option<f64>,
    pub priority: Option<i32>,
}
