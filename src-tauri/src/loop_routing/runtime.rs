use super::{
    adapter::{self, SessionReference},
    model::*,
    Service,
};
use crate::{
    ipc::server::DaemonState,
    pty::{scrollback_snapshot, spawn_session},
    usage::UsageWindow,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use uuid::Uuid;

#[derive(Default)]
struct Live {
    processed: HashSet<String>,
    tools: HashSet<String>,
    deferred_tools: HashSet<String>,
    agents: HashSet<String>,
    boundary: bool,
    idle: bool,
    permission: bool,
    initialized: bool,
    activity_seen: bool,
    uncertain: bool,
    terminating: Option<u64>,
    probed: HashSet<String>,
    last_key: Option<String>,
    cols: u16,
    rows: u16,
}
#[derive(Default)]
struct Quota {
    windows: Vec<UsageWindow>,
    error: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    settings: Settings,
    groups: Vec<Group>,
}
pub struct Engine {
    root: PathBuf,
    db: rusqlite::Connection,
    settings: Settings,
    groups: HashMap<Uuid, Group>,
    live: HashMap<Uuid, Live>,
    quotas: HashMap<String, Quota>,
}
fn data_root() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA unavailable")?)
            .join("com.user.winmux"),
    )
}
fn profile_dir(c: &Candidate) -> Result<PathBuf> {
    if let Some(id) = &c.profile_id {
        crate::commands::account_dir_path(&data_root()?, &c.agent, id).map_err(|e| anyhow!(e))
    } else {
        let variable = if c.agent == "claude" {
            "CLAUDE_CONFIG_DIR"
        } else {
            "CODEX_HOME"
        };
        std::env::var_os(variable)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|p| PathBuf::from(p).join(format!(".{}", c.agent)))
            })
            .context("System profile directory unavailable")
    }
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key].as_str().ok_or_else(|| anyhow!("Missing {key}"))
}
fn over_limit(settings: &Settings, c: &Candidate, windows: &[UsageWindow]) -> bool {
    windows.iter().any(|w| {
        let limit = match w.kind.as_str() {
            "short" => c.short_threshold.unwrap_or(settings.short_threshold),
            "weekly" => c.weekly_threshold.unwrap_or(settings.weekly_threshold),
            _ => return false,
        };
        w.percent_used >= limit
    })
}
impl Engine {
    pub fn open() -> Result<Self> {
        Self::open_at(data_root()?.join("loop-routing"))
    }
    fn open_at(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        let db = rusqlite::Connection::open(root.join("routing.sqlite3"))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS state (id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);")?;
        let saved: Option<String> = db
            .query_row("SELECT data FROM state WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let saved = saved
            .map(|s| serde_json::from_str::<Saved>(&s))
            .transpose()?;
        let (settings, groups) = if let Some(saved) = saved {
            ensure!(saved.version == 1, "Unsupported routing database version");
            saved.settings.validate()?;
            (saved.settings, saved.groups.into_iter().map(|mut g| {
                if g.status != "stopped" {
                    g.active_session_id = None;
                    if !stop_without_conversation(&mut g) {
                        g.state("recovery", Some("데몬이 다시 시작되었습니다. 이전 실행과 작업 상태를 확인한 뒤 재개하세요."));
                    }
                }
                (g.id, g)
            }).collect())
        } else {
            (Settings::default(), HashMap::new())
        };
        let engine = Self {
            root,
            db,
            settings,
            groups,
            live: HashMap::new(),
            quotas: HashMap::new(),
        };
        engine.save()?;
        Ok(engine)
    }
    fn save(&self) -> Result<()> {
        let saved = Saved {
            version: 1,
            settings: self.settings.clone(),
            groups: self.groups.values().cloned().collect(),
        };
        self.db.execute("INSERT INTO state(id,data) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET data=excluded.data", [serde_json::to_string(&saved)?])?;
        Ok(())
    }
    pub fn write_managed(
        &mut self,
        state: &Arc<DaemonState>,
        id: Uuid,
        bytes: &[u8],
    ) -> Result<bool> {
        let Some(g) = self
            .groups
            .values_mut()
            .find(|g| g.active_session_id == Some(id))
        else {
            return Ok(false);
        };
        ensure!(
            matches!(g.status.as_str(), "running" | "starting")
                && !(g.status == "starting"
                    && g.attempts.last().is_some_and(|a| a.continuation.is_some())),
            "루프 그룹이 전환 또는 일시정지 중입니다"
        );
        let mut persist_activity = false;
        // Serialize input with transitions. Even partial input invalidates an old idle boundary.
        if let Some(live) = self
            .live
            .get_mut(&g.id)
            .filter(|_| !bytes.is_empty() && bytes != b"\x1b[I" && bytes != b"\x1b[O")
        {
            if let Some(attempt) = g.attempts.last_mut().filter(|a| a.initial_prompt) {
                attempt.initial_prompt = false;
                persist_activity = true;
            }
            live.activity_seen = true;
            live.idle = false;
            live.boundary = false;
        }
        // Persist once, before forwarding the first input; a restart must not
        // mistake an interrupted conversation for an unused initial prompt.
        if persist_activity {
            self.save()?;
        }
        let mut sessions = state.manager.sessions.lock();
        let session = sessions.get_mut(&id).context("PTY not found")?;
        session.writer.write_all(bytes)?;
        session.writer.flush()?;
        Ok(true)
    }
    pub fn quota(&mut self, key: String, result: Result<Vec<UsageWindow>, String>) {
        match result {
            Ok(windows) => {
                self.quotas.insert(
                    key,
                    Quota {
                        windows,
                        error: None,
                    },
                );
            }
            Err(error) => {
                self.quotas.entry(key).or_default().error = Some(error);
            }
        }
    }
    pub fn usage_targets(&self) -> Vec<(String, String, PathBuf, bool)> {
        if self
            .groups
            .values()
            .all(|g| matches!(g.status.as_str(), "stopped" | "paused" | "recovery"))
        {
            return vec![];
        }
        self.settings
            .ordered()
            .into_iter()
            .filter_map(|c| {
                profile_dir(&c).ok().map(|dir| {
                    (
                        c.key(),
                        c.agent,
                        dir,
                        c.auth_method.as_deref() == Some("setup-token"),
                    )
                })
            })
            .collect()
    }
    pub fn sync_guards(&self, service: &Service) {
        *service.owned.lock() = self
            .groups
            .values()
            .filter_map(|g| g.active_session_id)
            .collect();
        *service.blocked.lock() = self
            .groups
            .values()
            .filter(|g| {
                !matches!(g.status.as_str(), "running" | "starting")
                    || (g.status == "starting"
                        && g.attempts.last().is_some_and(|a| a.continuation.is_some()))
            })
            .filter_map(|g| g.active_session_id)
            .collect();
    }
    fn dir(&self, group: &Group) -> PathBuf {
        self.root.join(group.id.to_string())
    }
    fn attempt_dir(&self, group: &Group) -> Result<PathBuf> {
        Ok(self
            .dir(group)
            .join(&group.attempts.last().context("No routing attempt")?.id))
    }
    fn control(&self, group: &Group, switch: bool) -> Result<()> {
        if group.attempts.is_empty() {
            return Ok(());
        }
        let path = self.attempt_dir(group)?.join("control.json");
        atomic_write(
            &path,
            &serde_json::to_vec(&json!({"switchRequested":switch}))?,
        )
    }
    fn eligible(&self, c: &Candidate, live: &Live) -> bool {
        if let Some(q) = self.quotas.get(&c.key()) {
            if q.error.as_deref().is_some_and(|e| {
                e.contains("expired") || e.contains("Sign in") || e.contains("cannot access")
            }) {
                return false;
            }
            if q.error.is_none()
                && crate::usage::usage_windows_are_fresh(&q.windows, Duration::from_secs(120))
            {
                return !over_limit(&self.settings, c, &q.windows);
            }
        }
        !live.probed.contains(&c.key())
    }
    pub fn request(&mut self, state: &Arc<DaemonState>, r: Value) -> Result<Value> {
        ensure!(
            r.to_string().len() <= 1024 * 1024,
            "Routing request too large"
        );
        match string(&r, "op")? {
            "capabilities" => Ok(json!({"version":1,"explicitStart":true})),
            "configure" => {
                let settings: Settings = serde_json::from_value(r["settings"].clone())?;
                settings.validate()?;
                for candidate in &settings.candidates {
                    profile_dir(candidate)?;
                }
                self.settings = settings;
                self.save()?;
                Ok(json!(self.settings))
            }
            "list" => {
                let mut groups: Vec<_> = self.groups.values().collect();
                groups.sort_by_key(|g| g.updated_at);
                Ok(json!(groups))
            }
            "conversations" => {
                let cwd = PathBuf::from(string(&r, "cwd")?).canonicalize()?;
                ensure!(cwd.is_dir(), "작업 폴더가 아닙니다");
                Ok(json!(self.conversation_options(&cwd)?.into_iter().map(|(reference, candidate, modified, title)| {
                    json!({"sessionId":reference.id,"candidateKey":candidate.key(),"agent":candidate.agent,"label":candidate.label,"title":title,
                        "updatedAt":modified.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64})
                }).collect::<Vec<_>>()))
            }
            "create" => {
                let requested_id = r["requestId"].as_str().map(Uuid::parse_str).transpose()?;
                if let Some(group) = requested_id.and_then(|id| self.groups.get(&id)) {
                    return Ok(json!(group));
                }
                ensure!(
                    !self.settings.ordered().is_empty(),
                    "설정에서 루프 라우팅에 참여할 프로필을 선택하세요."
                );
                let cwd = r["cwd"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(PathBuf::from)
                    .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
                    .context("작업 폴더가 필요합니다")?
                    .canonicalize()?;
                ensure!(cwd.is_dir(), "작업 폴더가 아닙니다");
                let id = requested_id.unwrap_or_else(Uuid::new_v4);
                let name = r["name"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("에이전트 루프")
                    .to_string();
                ensure!(
                    name.chars().count() <= 256 && !name.chars().any(char::is_control),
                    "루프 이름은 제어 문자 없이 256자까지 입력할 수 있습니다"
                );
                let mut group = Group {
                    id,
                    name,
                    workspace_id: string(&r, "workspaceId")?.into(),
                    workspace_index: r["workspaceIndex"]
                        .as_u64()
                        .unwrap_or(1)
                        .clamp(1, u32::MAX as u64) as u32,
                    cwd: cwd.to_string_lossy().into(),
                    status: "waiting".into(),
                    active_session_id: None,
                    reason: Some("시작할 프로필을 확인하고 있습니다".into()),
                    attempts: vec![],
                    updated_at: now(),
                    pending_work: false,
                };
                let start = &r["start"];
                let (reference, candidate, prompt) = match start["kind"].as_str() {
                    Some("prompt") => {
                        let prompt = string(start, "prompt")?.trim();
                        ensure!(
                            !prompt.is_empty()
                                && prompt.chars().count() <= 16000
                                && !prompt.contains('\0'),
                            "시작할 작업을 1~16000자로 입력하세요"
                        );
                        (
                            None,
                            self.settings.ordered()[0].clone(),
                            Some(prompt.to_owned()),
                        )
                    }
                    Some("session") => {
                        let selected = string(start, "sessionId")?;
                        let key = string(start, "candidateKey")?;
                        let (reference, candidate, _, _) = self.conversation_options(&cwd)?.into_iter()
                            .find(|(reference, candidate, _, _)| reference.id == selected && candidate.key() == key)
                            .context("선택한 대화를 찾을 수 없습니다. 목록을 새로고침하고 다시 선택하세요")?;
                        adapter::validate_conversation(&reference)?;
                        (Some(reference), candidate, None)
                    }
                    _ => bail!("새 프롬프트 또는 이어갈 대화를 직접 선택하세요"),
                };
                if reference.is_some() {
                    let active_dirs: Vec<_> = state
                        .manager
                        .sessions
                        .lock()
                        .values()
                        .filter(|session| {
                            !session.exited.load(Ordering::Acquire)
                                && *session.agent_kind.lock() != crate::pty::AgentKind::Terminal
                        })
                        .filter_map(|session| session.info.cwd.clone())
                        .collect();
                    ensure!(!active_dirs.iter().any(|dir| fs::canonicalize(dir).ok().as_ref() == Some(&cwd)),
                        "이 폴더의 에이전트가 다른 터미널에서 실행 중입니다. 해당 실행을 종료한 뒤 다시 선택하세요.");
                }
                group.pending_work = if let Some(reference) = &reference {
                    adapter::conversation_pending(reference)?
                } else {
                    true
                };
                group.attempts.push(Attempt {
                    id: Uuid::new_v4().to_string(),
                    session_id: None,
                    agent: candidate.agent,
                    profile_id: candidate.profile_id,
                    label: candidate.label,
                    status: if prompt.is_some() { "queued" } else { "linked" }.into(),
                    reason: None,
                    started_at: now(),
                    ended_at: Some(now()),
                    reference,
                    expected_session_id: None,
                    continuation: prompt,
                    continuation_sent_at: None,
                    continuation_acknowledged: false,
                    initial_prompt: false,
                });
                self.live.insert(
                    id,
                    Live {
                        cols: r["cols"].as_u64().unwrap_or(120).clamp(20, 500) as u16,
                        rows: r["rows"].as_u64().unwrap_or(30).clamp(5, 200) as u16,
                        ..Default::default()
                    },
                );
                self.groups.insert(id, group.clone());
                self.save()?;
                Ok(json!(group))
            }
            "rename" => {
                let id = Uuid::parse_str(string(&r, "id")?)?;
                let name = string(&r, "name")?.trim();
                ensure!(
                    !name.is_empty()
                        && name.chars().count() <= 256
                        && !name.chars().any(char::is_control),
                    "Loop name must contain 1-256 characters without control characters"
                );
                let group = self.groups.get_mut(&id).context("Loop group not found")?;
                group.name = name.to_owned();
                group.updated_at = now();
                let response = json!(group);
                self.save()?;
                Ok(response)
            }
            "move" => {
                let id = Uuid::parse_str(string(&r, "id")?)?;
                let workspace = string(&r, "workspaceId")?;
                ensure!(
                    !workspace.is_empty() && workspace.len() < 256,
                    "Invalid workspace ID"
                );
                let g = self
                    .groups
                    .get_mut(&id)
                    .context("그룹을 찾을 수 없습니다")?;
                g.workspace_id = workspace.into();
                g.workspace_index = r["workspaceIndex"]
                    .as_u64()
                    .context("Missing workspace index")?
                    .clamp(1, u32::MAX as u64) as u32;
                g.updated_at = now();
                let response = json!(g);
                self.save()?;
                Ok(response)
            }
            "history" => {
                let id = Uuid::parse_str(string(&r, "id")?)?;
                let g = self.groups.get(&id).context("그룹을 찾을 수 없습니다")?;
                let attempt = string(&r, "attemptId")?;
                ensure!(
                    g.attempts.iter().any(|a| a.id == attempt),
                    "실행 이력을 찾을 수 없습니다"
                );
                let file = self.dir(g).join(attempt).join("scrollback.base64");
                Ok(json!(fs::read_to_string(file).unwrap_or_default()))
            }
            op @ ("pause" | "resume" | "next" | "stop") => {
                let id = Uuid::parse_str(string(&r, "id")?)?;
                let mut g = self.groups.remove(&id).context("그룹을 찾을 수 없습니다")?;
                let result = (|| -> Result<Value> {
                    match op {
                        "pause" => {
                            ensure!(g.status != "switching", "전환 중에는 잠시 기다려 주세요");
                            self.control(&g, true)?;
                            g.state("paused", Some("자동 라우팅 일시정지"));
                        }
                        "resume" => {
                            ensure!(
                                g.status != "stopped",
                                "종료한 그룹은 다시 시작할 수 없습니다"
                            );
                            if stop_without_conversation(&mut g) {
                                self.control(&g, false)?;
                                return Ok(json!(g));
                            }
                            let live = self.live.entry(id).or_default();
                            live.probed.clear();
                            live.tools.extend(live.deferred_tools.drain());
                            live.boundary = false;
                            live.idle &= live.tools.is_empty()
                                && live.agents.is_empty()
                                && !live.permission
                                && !live.uncertain;
                            live.last_key = g.attempts.last().map(|a| {
                                format!(
                                    "{}:{}",
                                    a.agent,
                                    a.profile_id.as_deref().unwrap_or("system")
                                )
                            });
                            self.control(&g, false)?;
                            g.state(
                                if g.active_session_id.is_some() {
                                    "running"
                                } else {
                                    "waiting"
                                },
                                None,
                            );
                        }
                        "next" => {
                            ensure!(
                                matches!(
                                    g.status.as_str(),
                                    "running" | "starting" | "switch_pending"
                                ),
                                "실행 중인 그룹에서 전환할 수 있습니다"
                            );
                            self.control(&g, true)?;
                            g.state(
                                "switch_pending",
                                Some("다음 프로필로 전환할 안전한 경계를 기다립니다"),
                            );
                        }
                        "stop" => {
                            if let Some(sid) = g.active_session_id {
                                terminate(state, sid)?;
                            }
                            self.control(&g, false)?;
                            g.state("stopped", Some("사용자가 그룹을 종료했습니다"));
                        }
                        _ => unreachable!(),
                    }
                    Ok(json!(g))
                })();
                self.groups.insert(id, g);
                self.save()?;
                result
            }
            _ => bail!("지원하지 않는 루프 라우팅 명령"),
        }
    }
    pub fn tick(&mut self, state: &Arc<DaemonState>) -> Result<()> {
        let ids: Vec<_> = self.groups.keys().copied().collect();
        let mut changed = false;
        for id in ids {
            let mut g = self.groups.remove(&id).unwrap();
            let before = g.updated_at;
            let mut live = self.live.remove(&id).unwrap_or_default();
            if let Err(e) = self.step(state, &mut g, &mut live) {
                let _ = self.control(&g, true);
                g.state(
                    "recovery",
                    Some(&format!("자동 전환을 중단했습니다: {e:#}")),
                );
            }
            changed |= before != g.updated_at;
            self.groups.insert(id, g);
            self.live.insert(id, live);
        }
        if changed {
            self.save()?;
        }
        Ok(())
    }
    fn step(&mut self, state: &Arc<DaemonState>, g: &mut Group, live: &mut Live) -> Result<()> {
        if g.status == "recovery" {
            return Ok(());
        }
        if !g.attempts.is_empty() && g.status != "stopped" {
            self.read_events(g, live)?;
        }
        if let Some(sid) = g.active_session_id {
            let exited = state
                .manager
                .sessions
                .lock()
                .get(&sid)
                .map(|s| s.exited.load(Ordering::Acquire));
            if exited == Some(true) {
                self.archive(state, g, sid)?;
                g.active_session_id = None;
                if g.status == "switching" {
                    g.state("waiting", Some("다음 프로필을 선택합니다"));
                } else if g.status != "stopped" {
                    g.state(
                        "paused",
                        Some("에이전트가 종료되었습니다. 작업 상태를 확인한 뒤 재개하세요."),
                    );
                }
                live.terminating = None;
            } else if exited.is_none() {
                g.active_session_id = None;
                if g.status != "stopped" {
                    g.state("recovery", Some("현재 PTY를 찾을 수 없습니다"));
                }
                return Ok(());
            }
        }
        if matches!(g.status.as_str(), "stopped" | "paused" | "recovery") {
            return Ok(());
        }
        if g.status == "switching" {
            if live
                .terminating
                .is_some_and(|t| now().saturating_sub(t) > 15_000)
            {
                bail!("이전 프로세스의 종료를 확인하지 못했습니다. 새 실행을 시작하지 않았습니다");
            }
            return Ok(());
        }
        if g.status == "starting" && live.initialized {
            let ready_path = self.attempt_dir(g)?.join("startup-ready.json");
            let ready = fs::read(&ready_path)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
            let attempt = g.attempts.last().context("Missing attempt")?;
            let ready_matches = ready
                .as_ref()
                .and_then(|v| v["sessionId"].as_str())
                .is_some_and(|id| attempt.reference.as_ref().is_some_and(|r| r.id == id));
            if ready_matches && !live.permission {
                if let Some(prompt) = attempt
                    .continuation
                    .clone()
                    .filter(|_| attempt.continuation_sent_at.is_none())
                {
                    let sid = g.active_session_id.context("Missing continuation PTY")?;
                    g.attempts.last_mut().unwrap().continuation_sent_at = Some(now());
                    self.save_group(g)?;
                    let mut sessions = state.manager.sessions.lock();
                    let session = sessions.get_mut(&sid).context("Continuation PTY missing")?;
                    let bytes = format!("\x1b[200~{}\x1b[201~\r", prompt);
                    session.writer.write_all(bytes.as_bytes())?;
                    session.writer.flush()?;
                }
                let attempt = g.attempts.last().unwrap();
                // A verified fresh CLI may never emit Stop before its first prompt.
                // Never overwrite input/work observed while startup was pending.
                if !live.activity_seen && !g.pending_work && attempt.continuation.is_none() {
                    live.idle = true;
                }
                if attempt.continuation.is_none() || attempt.continuation_acknowledged {
                    g.state("running", None);
                }
            }
        }
        if g.status == "starting"
            && g.attempts
                .last()
                .is_some_and(|a| now().saturating_sub(a.started_at) > 60_000)
        {
            g.state("paused",Some("세션 시작 훅을 확인하지 못했습니다. CLI 초기 설정 및 훅 신뢰 승인을 완료하고 재개하세요."));
            return Ok(());
        }
        if g.status == "running" {
            let active = g.attempts.last().context("Missing attempt")?;
            let key = format!(
                "{}:{}",
                active.agent,
                active.profile_id.as_deref().unwrap_or("system")
            );
            let candidate = self
                .settings
                .candidates
                .iter()
                .find(|c| c.key() == key && c.enabled);
            let exceeded = candidate
                .and_then(|c| {
                    self.quotas.get(&key).map(|q| {
                        q.error.is_none()
                            && crate::usage::usage_windows_are_fresh(
                                &q.windows,
                                Duration::from_secs(120),
                            )
                            && over_limit(&self.settings, c, &q.windows)
                    })
                })
                .unwrap_or(false);
            let unknown_too_long = now().saturating_sub(active.started_at) > 120_000
                && !self.quotas.get(&key).is_some_and(|q| {
                    q.error.is_none()
                        && crate::usage::usage_windows_are_fresh(
                            &q.windows,
                            Duration::from_secs(120),
                        )
                });
            if exceeded || unknown_too_long || candidate.is_none() {
                self.control(g, true)?;
                g.state(
                    "switch_pending",
                    Some(if exceeded {
                        "사용량 임계점 도달 · 안전한 경계 대기"
                    } else if candidate.is_none() {
                        "프로필이 라우팅에서 제외되었습니다 · 안전한 경계 대기"
                    } else {
                        "사용량 확인 실패 · 안전한 경계 대기"
                    }),
                );
            }
        }
        if g.status == "switch_pending" && live.uncertain {
            self.control(g, false)?;
            g.state("recovery",Some("백그라운드 작업 또는 누락된 도구 이벤트로 안전한 종료를 확인할 수 없습니다. 자동 전환을 중단하고 기존 작업을 유지합니다."));
            return Ok(());
        }
        if g.status == "switch_pending" {
            if live.initialized
                && !live.permission
                && !live.uncertain
                && live.tools.is_empty()
                && live.agents.is_empty()
                && (live.boundary || live.idle)
            {
                if stop_without_conversation(g) {
                    self.control(g, false)?;
                    return Ok(());
                }
                let sid = g.active_session_id.context("Missing PTY")?;
                g.state("switching", Some("이전 실행 종료 확인 중"));
                // Persist intent before issuing a process termination.
                self.save_group(g)?;
                terminate(state, sid)?;
                live.terminating = Some(now());
            }
        }
        if g.status == "waiting" && g.active_session_id.is_none() {
            if let Some(c) = choose(&self.settings.ordered(), live.last_key.as_deref(), |c| {
                self.eligible(c, live)
                    && (queued_prompt(g).is_some()
                        || previous_conversation(g).is_some_and(|r| r.agent == c.agent)
                        || (g.pending_work && g.attempts.len() > 1))
            }) {
                self.launch(state, g, live, c)?;
            } else if g.reason.as_deref()
                != Some("모든 후보가 소진되었거나 보류 중입니다. 사용량 회복을 기다립니다.")
            {
                g.state(
                    "waiting",
                    Some("모든 후보가 소진되었거나 보류 중입니다. 사용량 회복을 기다립니다."),
                );
            }
        }
        Ok(())
    }
    fn save_group(&self, g: &Group) -> Result<()> {
        let mut groups: Vec<_> = self.groups.values().cloned().collect();
        groups.push(g.clone());
        let saved = Saved {
            version: 1,
            settings: self.settings.clone(),
            groups,
        };
        self.db.execute("INSERT INTO state(id,data) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET data=excluded.data",[serde_json::to_string(&saved)?])?;
        Ok(())
    }
    fn conversation_options(
        &self,
        cwd: &Path,
    ) -> Result<Vec<(SessionReference, Candidate, std::time::SystemTime, String)>> {
        let mut options = vec![];
        for candidate in self.settings.ordered() {
            let dir = profile_dir(&candidate)?;
            for (reference, modified, title) in adapter::conversations(&candidate.agent, &dir, cwd)?
            {
                options.push((reference, candidate.clone(), modified, title));
            }
        }
        options.sort_by(|a, b| b.2.cmp(&a.2));
        let mut seen = std::collections::HashSet::new();
        options.retain(|(reference, _, _, _)| {
            seen.insert((reference.agent.clone(), reference.id.clone()))
        });
        options.truncate(100);
        Ok(options)
    }
    fn launch(
        &mut self,
        state: &Arc<DaemonState>,
        g: &mut Group,
        live: &mut Live,
        c: Candidate,
    ) -> Result<()> {
        if stop_without_conversation(g) {
            return Ok(());
        }
        let previous = previous_conversation(g);
        if previous.as_ref().is_some_and(|r| r.agent != c.agent) && !g.pending_work {
            g.state(
                "stopped",
                Some(
                    "다른 에이전트로 인계할 진행 중 작업이 없습니다. 빈 대화로 전환하지 않습니다.",
                ),
            );
            return Ok(());
        }
        let dir = profile_dir(&c)?;
        ensure!(dir.is_dir(), "프로필 폴더를 찾을 수 없습니다");
        let attempt_id = Uuid::new_v4().to_string();
        let attempt_dir = self.dir(g).join(&attempt_id);
        fs::create_dir_all(&attempt_dir)?;
        let prompt = if let Some(prompt) = queued_prompt(g) {
            Some(prompt.to_owned())
        } else if g.pending_work {
            if let Some(reference) = &previous {
                let handoff = adapter::build_handoff(reference, Path::new(&g.cwd))?;
                let handoff_path = attempt_dir.join("handoff.md");
                atomic_write(&handoff_path, handoff.as_bytes())?;
                Some(format!("계속. 먼저 다음 인계 문서를 읽고 최신 사용자 요청과 실제 작업 파일 상태를 확인하세요: {}\n완료된 작업은 반복하지 말고 미확인 외부 작업은 재실행하지 마세요. 사용자 답변이나 승인이 필요하면 기다리세요.",handoff_path.display()))
            } else {
                None
            }
        } else {
            None
        };
        let resume = previous.as_ref().filter(|r| r.agent == c.agent);
        let mut env = c.env.clone();
        if c.auth_method.as_deref() == Some("setup-token") {
            let token = fs::read_to_string(dir.join("oauth-token.txt"))
                .context("프로필 토큰을 읽을 수 없습니다")?;
            ensure!(!token.trim().is_empty(), "프로필 토큰이 비어 있습니다");
            env.insert("CLAUDE_CODE_OAUTH_TOKEN".into(), token.trim().into());
        }
        let launch = adapter::prepare_launch(
            &c.agent,
            &dir,
            &attempt_dir,
            Path::new(&g.cwd),
            env,
            resume,
            None,
        )?;
        let key = c.key();
        live.probed.insert(key.clone());
        live.last_key = Some(key);
        live.tools.clear();
        live.deferred_tools.clear();
        live.agents.clear();
        live.processed.clear();
        live.boundary = false;
        live.idle = false;
        live.permission = false;
        live.initialized = false;
        live.activity_seen = false;
        live.uncertain = false;
        let initial_prompt = resume.is_none() && prompt.is_none();
        g.attempts.push(Attempt {
            id: attempt_id,
            session_id: None,
            agent: c.agent.clone(),
            profile_id: c.profile_id,
            label: c.label,
            status: "starting".into(),
            reason: g.reason.clone(),
            started_at: now(),
            ended_at: None,
            reference: None,
            expected_session_id: resume.map(|r| r.id.clone()),
            continuation: prompt,
            continuation_sent_at: None,
            continuation_acknowledged: false,
            initial_prompt,
        });
        g.state("starting", Some("에이전트 세션 시작 확인 중"));
        self.save_group(g)?;
        let session = spawn_session(
            state.events.clone(),
            format!("w{}.loop-{}-{}", g.workspace_index, g.id, g.attempts.len()),
            launch.shell,
            launch.args,
            Some(g.cwd.clone()),
            Some(launch.env),
            live.cols.max(80),
            live.rows.max(24),
        )?;
        let info = session.info.clone();
        state.manager.sessions.lock().insert(info.id, session);
        g.active_session_id = Some(info.id);
        g.attempts.last_mut().unwrap().session_id = Some(info.id);
        // The managed PTY is listed for attachment, but not announced as a standalone tab.
        self.save_group(g)?;
        Ok(())
    }
    fn archive(&self, state: &Arc<DaemonState>, g: &mut Group, sid: Uuid) -> Result<()> {
        if let Some(session) = state.manager.sessions.lock().get(&sid) {
            atomic_write(
                &self.attempt_dir(g)?.join("scrollback.base64"),
                scrollback_snapshot(&session.scrollback).as_bytes(),
            )?;
        }
        if let Some(a) = g.attempts.last_mut() {
            a.ended_at = Some(now());
            a.status = "archived".into();
        }
        let removed = state.manager.sessions.lock().remove(&sid);
        if let Some(session) = removed {
            tokio::task::spawn_blocking(move || drop(session));
        }
        // No SessionRemoved event: the group retains its stable tab.
        g.updated_at = now();
        Ok(())
    }
    fn read_events(&self, g: &mut Group, live: &mut Live) -> Result<()> {
        let dir = self.attempt_dir(g)?.join("events");
        if !dir.is_dir() {
            return Ok(());
        }
        let mut events = vec![];
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let key = entry.file_name().to_string_lossy().into_owned();
            if !key.ends_with(".json") || live.processed.contains(&key) {
                continue;
            }
            ensure!(entry.metadata()?.len() <= 64 * 1024, "Hook event too large");
            let event: Value = serde_json::from_slice(&fs::read(entry.path())?)?;
            events.push((key, event));
        }
        events.sort_by(|a, b| {
            a.1["atMs"]
                .as_u64()
                .cmp(&b.1["atMs"].as_u64())
                .then(a.0.cmp(&b.0))
        });
        let processed_dir = self.attempt_dir(g)?.join("processed-events");
        fs::create_dir_all(&processed_dir)?;
        for (key, event) in events {
            let kind = event["kind"].as_str().context("Malformed hook event")?;
            let boundary = event["boundary"].as_bool().unwrap_or(false);
            if event["unknownBackground"].as_bool().unwrap_or(false) {
                live.uncertain = true;
            }
            let session_id = event["sessionId"]
                .as_str()
                .context("Missing agent session id")?;
            ensure!(
                !session_id.is_empty() && session_id.len() < 256,
                "Invalid agent session id"
            );
            let attempt = g.attempts.last_mut().unwrap();
            ensure!(
                attempt
                    .expected_session_id
                    .as_deref()
                    .map_or(true, |id| id == session_id),
                "CLI resumed a different conversation; continuation was not submitted"
            );
            if let Some(reference) = &attempt.reference {
                ensure!(reference.id == session_id, "Hook session mismatch");
            }
            if let Some(path) = event["transcriptPath"].as_str().filter(|p| !p.is_empty()) {
                attempt.reference = Some(SessionReference {
                    agent: attempt.agent.clone(),
                    id: session_id.into(),
                    transcript_path: PathBuf::from(path),
                });
            }
            if matches!(
                kind,
                "UserPromptSubmit"
                    | "PreToolUse"
                    | "PostToolUse"
                    | "PostToolUseFailure"
                    | "PermissionRequest"
                    | "SubagentStart"
                    | "SubagentStop"
                    | "Stop"
            ) {
                live.activity_seen = true;
                attempt.initial_prompt = false;
            }
            match kind {
                "SessionStart" => {
                    ensure!(
                        attempt
                            .expected_session_id
                            .as_deref()
                            .map_or(true, |id| id == session_id),
                        "CLI resumed a different conversation; continuation was not submitted"
                    );
                    live.initialized = true;
                    attempt.status = "running".into();
                    atomic_write(
                        &self.attempt_dir(g)?.join("start-approved.json"),
                        &serde_json::to_vec(&json!({"sessionId":session_id}))?,
                    )?;
                }
                "UserPromptSubmit" => {
                    attempt.continuation_acknowledged = attempt.continuation_sent_at.is_some();
                    g.pending_work = true;
                    live.idle = false;
                    live.boundary = false;
                    live.permission = false;
                }
                "PreToolUse" => {
                    live.idle = false;
                    if boundary {
                        live.boundary = true;
                        if let Some(id) = event["toolUseId"].as_str() {
                            live.deferred_tools.insert(id.into());
                        } else {
                            live.uncertain = true;
                        }
                    } else if let Some(id) = event["toolUseId"].as_str() {
                        live.tools.insert(id.into());
                    } else {
                        live.uncertain = true;
                    }
                }
                "PostToolUse" | "PostToolUseFailure" => {
                    if let Some(id) = event["toolUseId"].as_str() {
                        live.tools.remove(id);
                    } else {
                        live.uncertain = true;
                    }
                    live.permission = false;
                    if boundary {
                        live.boundary = true;
                    }
                }
                "PermissionRequest" => {
                    live.permission = true;
                }
                "SubagentStart" => {
                    if let Some(id) = event["subagentId"].as_str() {
                        live.agents.insert(id.into());
                    } else {
                        live.uncertain = true;
                    }
                }
                "SubagentStop" => {
                    if let Some(id) = event["subagentId"].as_str() {
                        live.agents.remove(id);
                    } else {
                        live.uncertain = true;
                    }
                }
                "Stop" => {
                    live.idle = true;
                    live.permission = false;
                    if !live.boundary {
                        g.pending_work = false;
                    }
                }
                "SessionEnd" => {}
                "BoundaryExpired" => {
                    live.uncertain = true;
                    self.control(g, false)?;
                    g.state("recovery",Some("경계 확인 시간이 초과되어 자동 전환을 중단했습니다. 기존 에이전트 작업 상태를 확인하세요."));
                }
                _ => {}
            }
            if fs::rename(dir.join(&key), processed_dir.join(&key)).is_err() {
                live.processed.insert(key);
            }
            g.updated_at = now();
        }
        Ok(())
    }
}
fn queued_prompt(g: &Group) -> Option<&str> {
    if g.attempts.len() != 1 {
        return None;
    }
    let seed = &g.attempts[0];
    (seed.status == "queued" && seed.continuation_sent_at.is_none())
        .then(|| seed.continuation.as_deref())
        .flatten()
        .filter(|text| !text.trim().is_empty())
}
fn stop_without_conversation(g: &mut Group) -> bool {
    if queued_prompt(g).is_some() {
        return false;
    }
    let result = previous_conversation(g)
        .context("연결된 진행 중 대화를 찾을 수 없습니다")
        .and_then(|reference| adapter::validate_conversation(&reference));
    if let Err(error) = result {
        g.state(
            "stopped",
            Some(&format!(
                "이어갈 세션이 없어 중지했습니다. 새 대화를 자동 시작하지 않습니다: {error}"
            )),
        );
        true
    } else {
        false
    }
}

// A verified untouched launch contains no user conversation to import, even
// when its SessionStart hook supplied a future transcript path. Preserve older
// records conservatively: their initial_prompt field defaults to false.
fn previous_conversation(g: &Group) -> Option<SessionReference> {
    g.attempts
        .iter()
        .rev()
        .filter(|a| g.pending_work || !a.initial_prompt)
        .find_map(|a| a.reference.clone())
}

use rusqlite::OptionalExtension;
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // MoveFileEx replacement through std::fs::rename is atomic on Windows.
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })?;
    Ok(())
}
fn terminate(state: &Arc<DaemonState>, id: Uuid) -> Result<()> {
    let mut killer = state
        .manager
        .sessions
        .lock()
        .get(&id)
        .context("PTY not found")?
        .killer
        .clone_killer();
    // The cloned native process handle cannot target a reused or unrelated PID.
    tokio::task::spawn_blocking(move || {
        let _ = killer.kill();
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Candidate {
        Candidate {
            agent: "claude".into(),
            profile_id: None,
            label: "System".into(),
            enabled: true,
            short_threshold: Some(80.0),
            weekly_threshold: None,
            config_dir: None,
            auth_method: None,
            env: HashMap::new(),
        }
    }
    fn window(kind: &str, n: f64) -> UsageWindow {
        UsageWindow {
            label: kind.into(),
            kind: kind.into(),
            percent_used: n,
            resets_at: None,
            received_at: Some(now()),
        }
    }
    #[test]
    fn limits_use_independent_windows_and_ignore_unrelated_model_quota() {
        let s = Settings::default();
        let c = candidate();
        assert!(over_limit(&s, &c, &[window("short", 80.0)]));
        assert!(!over_limit(
            &s,
            &c,
            &[window("short", 79.0), window("weekly", 89.0)]
        ));
        assert!(over_limit(&s, &c, &[window("weekly", 90.0)]));
        assert!(!over_limit(&s, &c, &[window("model_weekly", 100.0)]));
    }
    #[test]
    fn restart_never_replays_an_incomplete_transition() {
        let root = std::env::temp_dir().join(format!("rhyme-loop-test-{}", Uuid::new_v4()));
        let id = Uuid::new_v4();
        {
            let mut e = Engine::open_at(root.clone()).unwrap();
            e.groups.insert(
                id,
                Group {
                    id,
                    name: "test".into(),
                    workspace_id: "w".into(),
                    workspace_index: 1,
                    cwd: "C:\\".into(),
                    status: "switching".into(),
                    active_session_id: Some(Uuid::new_v4()),
                    reason: None,
                    attempts: vec![],
                    updated_at: now(),
                    pending_work: true,
                },
            );
            e.save().unwrap();
        }
        let e = Engine::open_at(root.clone()).unwrap();
        let g = &e.groups[&id];
        assert_eq!(g.status, "stopped");
        assert!(g.active_session_id.is_none());
        assert!(g.pending_work);
        drop(e);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unknown_profile_is_probed_only_once_and_fresh_quota_recovers_it() {
        let root = std::env::temp_dir().join(format!("rhyme-loop-test-{}", Uuid::new_v4()));
        let mut e = Engine::open_at(root.clone()).unwrap();
        let c = candidate();
        let mut live = Live::default();
        assert!(e.eligible(&c, &live));
        live.probed.insert(c.key());
        assert!(!e.eligible(&c, &live));
        e.quota(c.key(), Ok(vec![window("short", 10.0)]));
        assert!(e.eligible(&c, &live));
        e.quota(c.key(), Err("Sign in again".into()));
        assert!(!e.eligible(&c, &live));
        drop(e);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod regression_tests;
