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
            short_threshold: 90.0,
            weekly_threshold: 90.0,
            agent_order: agent_order(),
            candidates: vec![],
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
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
        self.agent_order
            .iter()
            .flat_map(|agent| {
                self.candidates
                    .iter()
                    .filter(move |c| &c.agent == agent && c.enabled)
                    .cloned()
            })
            .collect()
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
    pub id: uuid::Uuid,
    pub name: String,
    pub workspace_id: String,
    #[serde(default = "default_workspace_index")]
    pub workspace_index: u32,
    pub cwd: String,
    pub status: String,
    pub active_session_id: Option<uuid::Uuid>,
    pub reason: Option<String>,
    pub attempts: Vec<Attempt>,
    pub updated_at: u64,
    #[serde(default)]
    pub pending_work: bool,
}
impl Group {
    pub fn state(&mut self, state: &str, reason: Option<&str>) {
        self.status = state.into();
        self.reason = reason.map(str::to_string);
        self.updated_at = now();
    }
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
    fn persisted_settings_never_include_environment_values() {
        let mut candidate = c("a");
        candidate.env.insert("SECRET".into(), "sensitive".into());
        assert!(!serde_json::to_string(&candidate)
            .unwrap()
            .contains("sensitive"));
    }
}
