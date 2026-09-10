use super::*;
use crate::pty::agent::ProcessEntry;

struct Fixture {
    engine: Engine,
    state: Arc<DaemonState>,
    id: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let mut f = Self::auto_start();
        f.engine.groups.get_mut(&f.id).unwrap().status = LoopStatus::Idle;
        f.engine.groups.get_mut(&f.id).unwrap().current_provider = None;
        f.engine.live.insert(f.id, Live::default());
        f
    }
    fn auto_start() -> Self {
        let root = std::env::temp_dir().join(format!("rhyme-loop-runtime-{}", Uuid::new_v4()));
        let mut engine = Engine::open_at(root.clone()).unwrap();
        engine.settings.candidates = vec![candidate("a"), candidate("b")];
        let state = Arc::new(DaemonState::new());
        let response=engine.request(&state,json!({"op":"create","name":"Loop","workspaceId":"test","workspaceIndex":2,"cwd":root})).unwrap();
        let id = Uuid::parse_str(response["id"].as_str().unwrap()).unwrap();
        // Wait for the interactive shell to finish its startup script before
        // exercising Ctrl+C. No real provider or credential is used.
        std::thread::sleep(Duration::from_millis(1000));
        Self { engine, state, id }
    }
    fn sid(&self) -> Uuid {
        self.engine.groups[&self.id].active_session_id.unwrap()
    }
    fn agent(&self, provider: Option<&str>) {
        let mut monitor = self.state.manager.runtime.lock();
        monitor.revision += 1;
        monitor.states.insert(
            self.sid(),
            AgentRuntimeState {
                status: if provider.is_some() {
                    RuntimeStatus::Running
                } else {
                    RuntimeStatus::Exited
                },
                provider: provider.map(str::to_owned),
                pid: provider.map(|_| u32::MAX - 1),
                started_at: Some(1),
            },
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.state.manager.kill_all();
        // SQLite is still open here on Windows; temp artifacts stay isolated.
    }
}
fn candidate(id: &str) -> Candidate {
    serde_json::from_value(json!({"agent":"codex","profileId":id,"label":id,"enabled":true}))
        .unwrap()
}
fn window(usage: f64, reset: u64) -> UsageWindow {
    UsageWindow {
        label: "5h".into(),
        kind: "short".into(),
        percent_used: usage,
        resets_at: Some(json!(reset)),
        received_at: Some(now()),
    }
}
fn running(f: &mut Fixture) {
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = "running".into();
    g.active_profile = Some("codex:a".into());
    g.current_provider = Some("codex".into());
    let l = f.engine.live.get_mut(&f.id).unwrap();
    l.process_seen = true;
    l.launch_at = now();
    f.engine.quota(
        "codex:a".into(),
        Ok(vec![window(89.0, now() + 60_000)]),
        now(),
    );
    f.agent(Some("codex"));
}
#[test]
fn create_schedules_guarded_auto_start_and_is_idempotent() {
    let mut f = Fixture::auto_start();
    let g = &f.engine.groups[&f.id];
    assert_eq!(g.status, "resuming");
    assert_eq!(g.current_provider.as_deref(), Some("codex"));
    assert!(f.engine.live[&f.id].initial_start);
    assert!(g.attempts.is_empty());
    assert_eq!(g.runtime.status, RuntimeStatus::Idle);
    assert_eq!(f.state.manager.list().len(), 1);
    let response = f
        .engine
        .request(&f.state, json!({"op":"create","requestId":f.id}))
        .unwrap();
    assert_eq!(response["activeSessionId"], json!(f.sid()));
    assert_eq!(f.state.manager.list().len(), 1);
}
#[test]
fn ordinary_shell_input_is_not_an_agent_trigger() {
    let mut f = Fixture::new();
    let sid = f.sid();
    assert!(f
        .engine
        .write_managed(&f.state, sid, b"git status\r")
        .unwrap());
    assert_eq!(f.engine.groups[&f.id].status, "idle");
    assert!(f.engine.usage_targets().is_empty());
}
#[test]
fn codex_and_resume_use_the_same_start_gate() {
    for args in [
        vec![],
        vec!["resume".to_string(), Uuid::new_v4().to_string()],
    ] {
        let mut f = Fixture::new();
        let pid = std::process::id();
        f.state.manager.runtime.lock().processes.push(ProcessEntry {
            pid,
            parent_pid: None,
            image_name: "bridge".into(),
            command_args: vec![],
            started_at: 1,
        });
        let request = Uuid::new_v4().to_string();
        let root = f.engine.dir(&f.engine.groups[&f.id]);
        atomic_json(
            &root.join(format!("request-{request}.json")),
            &json!({"id":request,"pid":pid,"provider":"codex","args":args,"cwd":root}),
        )
        .unwrap();
        f.engine.tick(&f.state).unwrap();
        assert_eq!(f.engine.groups[&f.id].status, "preparing");
        assert_eq!(f.engine.groups[&f.id].command_args, args);
        assert!(!root.join(format!("response-{request}.json")).exists());
        assert!(!f.engine.usage_targets().is_empty());
    }
}
#[test]
fn threshold_is_inclusive_and_independent_for_each_window() {
    let c = candidate("a");
    let s = Settings::default();
    assert!(!over_limit(&s, &c, &[window(89.0, 0)]));
    assert!(over_limit(&s, &c, &[window(90.0, 0)]));
    assert!(over_limit(&s, &c, &[window(91.0, 0)]));
    let mut weekly = window(95.0, 0);
    weekly.kind = "weekly".into();
    assert!(over_limit(&s, &c, &[window(1.0, 0), weekly]));
}
#[test]
fn threshold_interrupts_without_any_tool_boundary() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine.quota(
        "codex:a".into(),
        Ok(vec![window(90.0, now() + 60_000)]),
        now(),
    );
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "switching_profile");
    assert_eq!(
        f.engine.groups[&f.id].runtime.status,
        RuntimeStatus::Interrupting
    );
    assert!(f.engine.live[&f.id].interrupt_at.is_some());
    assert_eq!(f.engine.groups[&f.id].active_session_id, Some(f.sid()));
}
#[test]
fn no_replacement_before_confirmed_process_exit() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine
        .request(&f.state, json!({"op":"next","id":f.id}))
        .unwrap();
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "switching_profile");
    assert!(f.engine.groups[&f.id].attempts.is_empty());
}
#[test]
fn interrupt_escalates_to_terminate_then_kill() {
    let mut f = Fixture::new();
    running(&mut f);
    // Seed an unresponsive Agent after graceful interrupt. The synthetic PID
    // deliberately cannot target a real user process.
    f.engine.groups.get_mut(&f.id).unwrap().status = "switching_profile".into();
    f.engine.live.get_mut(&f.id).unwrap().interrupt_at = Some(now() - 5_100);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.live[&f.id].killed, 1);
    f.engine.live.get_mut(&f.id).unwrap().interrupt_at = Some(now() - 8_100);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.live[&f.id].killed, 2);
    assert_eq!(f.engine.groups[&f.id].status, "switching_profile");
}
#[test]
fn ctrl_c_and_agent_exit_return_to_idle_without_stopping_loop() {
    let mut f = Fixture::new();
    running(&mut f);
    let sid = f.sid();
    f.engine.write_managed(&f.state, sid, b"\x03").unwrap();
    f.agent(None);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "idle");
    assert_eq!(f.engine.groups[&f.id].active_session_id, Some(sid));
}
#[test]
fn voluntary_agent_exit_is_not_task_completion() {
    let mut f = Fixture::new();
    running(&mut f);
    f.agent(None);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "idle");
}
#[test]
fn pause_interrupts_and_never_schedules_auto_resume() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine
        .request(&f.state, json!({"op":"pause","id":f.id}))
        .unwrap();
    f.agent(None);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "paused");
    assert!(f.engine.groups[&f.id].resume_at.is_none());
    assert!(f.engine.usage_targets().is_empty());
}
#[test]
fn stop_cancels_waiting_and_survives_restart() {
    let mut f = Fixture::new();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = "waiting_for_usage_reset".into();
    g.resume_at = Some(now() - 1);
    f.engine
        .request(&f.state, json!({"op":"stop","id":f.id}))
        .unwrap();
    f.agent(None);
    f.engine.tick(&f.state).unwrap();
    let restored = Engine::open_at(f.engine.root.clone()).unwrap();
    assert_eq!(restored.groups[&f.id].status, "stopped");
    assert!(restored.groups[&f.id].resume_at.is_none());
    assert!(restored.groups[&f.id].active_session_id.is_none());
}
#[test]
fn restart_preserves_waiting_paused_idle_but_not_stale_running() {
    for status in ["idle", "paused", "waiting_for_usage_reset", "running"] {
        let mut f = Fixture::new();
        f.engine.groups.get_mut(&f.id).unwrap().status = status.into();
        f.engine.save().unwrap();
        let restored = Engine::open_at(f.engine.root.clone()).unwrap();
        assert_eq!(
            restored.groups[&f.id].status,
            if status == "running" { "idle" } else { status }
        );
        assert!(restored.groups[&f.id].runtime.pid.is_none());
    }
}
#[test]
fn switching_queues_input_and_flushes_it_once() {
    let mut f = Fixture::new();
    let sid = f.sid();
    f.engine.groups.get_mut(&f.id).unwrap().status = "switching_profile".into();
    f.engine
        .write_managed(&f.state, sid, b"Write-Output LOOP_QUEUE_MARKER\r")
        .unwrap();
    assert_eq!(f.engine.groups[&f.id].queued_input.len(), 1);
    let mut g = f.engine.groups.remove(&f.id).unwrap();
    f.engine.flush_input(&f.state, &mut g).unwrap();
    f.engine.flush_input(&f.state, &mut g).unwrap();
    assert!(g.queued_input.is_empty());
    f.engine.groups.insert(f.id, g);
}
#[test]
fn reset_uses_latest_exhausted_window_and_earliest_profile() {
    let mut f = Fixture::new();
    let at = now() + 120_000;
    let mut weekly = window(99.0, at + 60_000);
    weekly.kind = "weekly".into();
    f.engine
        .quota("codex:a".into(), Ok(vec![window(90.0, at), weekly]), now());
    f.engine
        .quota("codex:b".into(), Ok(vec![window(94.0, at + 20_000)]), now());
    let mut g = f.engine.groups.remove(&f.id).unwrap();
    f.engine.profiles(&mut g);
    assert_eq!(
        g.profiles
            .iter()
            .find(|p| p.key == "codex:a")
            .unwrap()
            .reset_at,
        Some(at + 60_000)
    );
    assert_eq!(
        g.profiles.iter().filter_map(|p| p.reset_at).min(),
        Some(at + 20_000)
    );
    assert_eq!(reset_millis(&json!("1970-01-01T00:01:00Z")), Some(60_000));
    f.engine.groups.insert(f.id, g);
}
#[test]
fn reset_deadline_requires_new_usage_evidence() {
    let mut f = Fixture::new();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = "waiting_for_usage_reset".into();
    g.resume_at = Some(now() - 1);
    g.current_provider = Some("codex".into());
    f.engine.tick(&f.state).unwrap();
    assert_ne!(f.engine.groups[&f.id].status, "running");
    assert!(f.engine.live[&f.id].guard_at > 0);
}
#[test]
fn stale_or_failed_usage_never_approves_a_launch() {
    let mut f = Fixture::new();
    let stamp = now();
    let c = candidate("a");
    f.engine
        .quota(c.key(), Ok(vec![window(1.0, stamp + 100_000)]), stamp);
    assert!(!f.engine.eligible(&c, stamp + 1));
    f.engine
        .quota(c.key(), Err("429 rate limited".into()), stamp + 2);
    assert!(!f.engine.eligible(&c, stamp + 1));
}

#[test]
fn activation_recheck_skips_a_profile_that_crossed_threshold() {
    let mut f = Fixture::new();
    let root = f.engine.dir(&f.engine.groups[&f.id]);
    let request = Uuid::new_v4().to_string();
    let pid = std::process::id();
    f.state.manager.runtime.lock().processes.push(ProcessEntry {
        pid,
        parent_pid: None,
        image_name: "bridge".into(),
        command_args: vec![],
        started_at: 1,
    });
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = LoopStatus::Preparing;
    g.current_provider = Some("codex".into());
    let stamp = now();
    let live = f.engine.live.get_mut(&f.id).unwrap();
    live.dispatch = Some(Dispatch {
        id: request.clone(),
        pid,
        provider: "codex".into(),
        args: vec![],
        cwd: root.to_string_lossy().into_owned(),
    });
    live.helper_pid = Some(pid);
    live.guard_at = stamp;
    live.selected = Some("codex:a".into());
    f.engine.quota(
        "codex:a".into(),
        Ok(vec![window(91.0, stamp + 60_000)]),
        stamp,
    );
    f.engine.quota(
        "codex:b".into(),
        Ok(vec![window(95.0, stamp + 30_000)]),
        stamp,
    );
    f.engine.tick(&f.state).unwrap();
    assert!(f.engine.groups[&f.id].attempts.is_empty());
    assert_eq!(f.engine.groups[&f.id].status, "waiting_for_usage_reset");
    assert_eq!(
        f.engine.groups[&f.id].waiting_profile_id.as_deref(),
        Some("codex:b")
    );
    let response: Value =
        serde_json::from_slice(&fs::read(root.join(format!("response-{request}.json"))).unwrap())
            .unwrap();
    assert_eq!(response["cancel"], true);
}

#[test]
fn uncertain_input_survives_restart_until_user_resolves_delivery() {
    let mut f = Fixture::new();
    f.engine
        .groups
        .get_mut(&f.id)
        .unwrap()
        .queued_input
        .push(InputChunk {
            id: Uuid::new_v4().to_string(),
            bytes: b"keep this prompt\r".to_vec(),
            delivering: true,
        });
    f.engine.save().unwrap();
    let mut restored = Engine::open_at(f.engine.root.clone()).unwrap();
    assert_eq!(restored.groups[&f.id].status, "paused");
    assert!(restored
        .request(&f.state, json!({"op":"resume","id":f.id}))
        .is_err());
    restored
        .request(
            &f.state,
            json!({"op":"resolve_input","id":f.id,"delivered":false}),
        )
        .unwrap();
    assert_eq!(
        restored.groups[&f.id].queued_input[0].bytes,
        b"keep this prompt\r"
    );
    assert!(!restored.groups[&f.id].queued_input[0].delivering);
}

#[test]
fn stop_remains_terminal_when_agent_refuses_to_exit() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine.groups.get_mut(&f.id).unwrap().status = LoopStatus::Stopped;
    f.engine.live.get_mut(&f.id).unwrap().interrupt_at = Some(now() - 14_000);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "stopped");
    assert!(f.engine.usage_targets().is_empty());
}

#[test]
fn regular_keystrokes_do_not_write_loop_database() {
    let mut f = Fixture::new();
    let before = f.engine.db.total_changes();
    let sid = f.sid();
    f.engine.write_managed(&f.state, sid, b"git").unwrap();
    f.engine.write_managed(&f.state, sid, b" status\r").unwrap();
    assert_eq!(before, f.engine.db.total_changes());
}

#[test]
fn rate_limit_cooldown_survives_a_low_usage_refresh() {
    let mut f = Fixture::new();
    let c = candidate("a");
    f.engine
        .quota(c.key(), Ok(vec![window(1.0, now() + 60_000)]), now());
    f.engine.quotas.get_mut(&c.key()).unwrap().blocked_until = now() + 60_000;
    f.engine
        .quota(c.key(), Ok(vec![window(1.0, now() + 60_000)]), now());
    assert!(!f.engine.eligible(&c, 0));
}

#[test]
fn cli_startup_questions_are_not_swallowed_by_pending_handoff_input() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine
        .groups
        .get_mut(&f.id)
        .unwrap()
        .queued_input
        .push(InputChunk {
            id: Uuid::new_v4().to_string(),
            bytes: b"later prompt\r".to_vec(),
            delivering: false,
        });
    let sid = f.sid();
    f.engine.write_managed(&f.state, sid, b"\r").unwrap();
    assert_eq!(f.engine.groups[&f.id].queued_input.len(), 1);
    f.engine.live.get_mut(&f.id).unwrap().startup_ready = true;
    f.engine
        .write_managed(&f.state, sid, b"another prompt\r")
        .unwrap();
    assert_eq!(f.engine.groups[&f.id].queued_input.len(), 2);
}

#[test]
fn native_startup_identity_check_allows_later_cli_session_selection() {
    let mut f = Fixture::new();
    let first = Uuid::new_v4().to_string();
    let second = Uuid::new_v4().to_string();
    let attempt = Uuid::new_v4().to_string();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = LoopStatus::Preparing;
    g.attempts.push(serde_json::from_value(json!({"id":attempt,"sessionId":null,"agent":"codex","profileId":"a","label":"A","status":"starting","reason":null,"startedAt":now(),"endedAt":null,"reference":null,"expectedSessionId":first})).unwrap());
    let mut g = f.engine.groups.remove(&f.id).unwrap();
    let dir = f.engine.dir(&g).join(attempt).join("events");
    fs::create_dir_all(&dir).unwrap();
    let mut live = Live::default();
    for id in [&first, &second] {
        atomic_json(
            &dir.join(format!("{id}.json")),
            &json!({"kind":"SessionStart","sessionId":id}),
        )
        .unwrap();
        f.engine.read_events(&mut g, &mut live).unwrap();
        assert_eq!(g.current_agent_session_id.as_ref(), Some(id));
    }
    assert!(g.attempts[0].expected_session_id.is_none());
    f.engine.groups.insert(f.id, g);
}

#[test]
fn stop_cancels_an_auto_resume_command_still_arriving_from_the_shell() {
    let mut f = Fixture::new();
    let pid = std::process::id();
    let root = f.engine.dir(&f.engine.groups[&f.id]);
    let id = Uuid::new_v4().to_string();
    f.state.manager.runtime.lock().processes.push(ProcessEntry {
        pid,
        parent_pid: None,
        image_name: "bridge".into(),
        command_args: vec![],
        started_at: 1,
    });
    f.engine.groups.get_mut(&f.id).unwrap().status = LoopStatus::Stopped;
    f.engine.live.get_mut(&f.id).unwrap().awaiting_dispatch = true;
    atomic_json(
        &root.join(format!("request-{id}.json")),
        &json!({"id":id,"pid":pid,"provider":"codex","args":[],"cwd":root}),
    )
    .unwrap();
    f.engine.tick(&f.state).unwrap();
    let response: Value =
        serde_json::from_slice(&fs::read(root.join(format!("response-{id}.json"))).unwrap())
            .unwrap();
    assert_eq!(response["cancel"], true);
    assert!(f.engine.groups[&f.id].attempts.is_empty());
    assert_eq!(f.engine.groups[&f.id].status, "stopped");
}

#[test]
fn legacy_waiting_without_runtime_metadata_requires_explicit_recovery() {
    let mut f = Fixture::new();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.policy = None;
    g.status = LoopStatus::WaitingForUsageReset;
    f.engine.save().unwrap();
    let restored = Engine::open_at(f.engine.root.clone()).unwrap();
    assert_eq!(restored.groups[&f.id].status, "paused");
    assert!(restored.groups[&f.id].active_session_id.is_none());
}

#[test]
fn available_profiles_still_display_their_reset_time() {
    let mut f = Fixture::new();
    let reset = now() + 60_000;
    f.engine
        .quota("codex:a".into(), Ok(vec![window(42.0, reset)]), now());
    let mut g = f.engine.groups.remove(&f.id).unwrap();
    f.engine.profiles(&mut g);
    assert_eq!(
        g.profiles
            .iter()
            .find(|p| p.key == "codex:a")
            .unwrap()
            .reset_at,
        Some(reset)
    );
    f.engine.groups.insert(f.id, g);
}

#[test]
fn first_launch_uses_order_before_smart_usage_and_skips_exhausted() {
    let mut f = Fixture::auto_start();
    let pid = std::process::id();
    f.state.manager.runtime.lock().processes.push(ProcessEntry { pid, parent_pid: None, image_name: "bridge".into(), command_args: vec![], started_at: 1 });
    f.engine.groups.get_mut(&f.id).unwrap().status = LoopStatus::Preparing;
    let stamp = now();
    let live = f.engine.live.get_mut(&f.id).unwrap();
    live.continuation = true;
    live.dispatch = Some(Dispatch { id: Uuid::new_v4().to_string(), pid, provider: "codex".into(), args: vec![], cwd: f.engine.root.to_string_lossy().into_owned() });
    live.helper_pid = Some(pid);
    live.guard_at = stamp;
    f.engine.quota("codex:a".into(), Ok(vec![window(80.0, stamp + 60000)]), stamp);
    f.engine.quota("codex:b".into(), Ok(vec![window(1.0, stamp + 60000)]), stamp);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.live[&f.id].selected.as_deref(), Some("codex:a"));
    assert!(f.engine.groups[&f.id].attempts.is_empty());
    let recheck = f.engine.live[&f.id].guard_at;
    f.engine.quota("codex:a".into(), Ok(vec![window(91.0, stamp + 60000)]), recheck);
    f.engine.quota("codex:b".into(), Ok(vec![window(1.0, stamp + 60000)]), recheck);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.live[&f.id].selected.as_deref(), Some("codex:b"));
    assert!(f.engine.groups[&f.id].attempts.is_empty());
}

#[test]
fn live_policy_is_scoped_persisted_and_used_by_usage_guard() {
    let mut f = Fixture::new();
    let other_id = Uuid::new_v4();
    let mut other = f.engine.groups[&f.id].clone();
    other.id = other_id;
    f.engine.groups.insert(other_id, other);
    let response = f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"strategy":"PRIORITY","pollingIntervalSeconds":10,"autoResume":false,"profile":{"key":"codex:b","shortThreshold":70,"weeklyThreshold":80,"priority":5}}})).unwrap();
    assert_eq!(response["policy"]["candidates"][1]["shortThreshold"], 70.0);
    assert_eq!(f.engine.settings.short_threshold, 90.0);
    assert!(f.engine.settings.candidates[1].short_threshold.is_none());
    assert!(f.engine.groups[&other_id].policy.as_ref().unwrap().candidates[1].short_threshold.is_none());
    let restored = Engine::open_at(f.engine.root.clone()).unwrap();
    let policy = restored.groups[&f.id].policy.as_ref().unwrap();
    assert_eq!(policy.candidates[1].priority, 5);
    assert!(!policy.auto_resume);
    assert!(over_limit(policy, &policy.candidates[1], &[window(75.0, now()+60000)]));
}
#[test]
fn active_threshold_changes_are_rejected_atomically_even_without_a_runtime_snapshot() {
    let mut f = Fixture::new();
    f.engine.groups.get_mut(&f.id).unwrap().active_profile = Some("codex:a".into());
    for field in ["shortThreshold", "weeklyThreshold"] {
        let mut patch = json!({"profile":{"key":"codex:a"},"strategy":"PRIORITY"});
        patch["profile"][field] = json!(95);
        let result = f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":patch}));
        assert!(result.unwrap_err().to_string().contains("활성 계정"));
        assert_eq!(f.engine.groups[&f.id].policy.as_ref().unwrap().strategy, "SMART");
    }
    // A profile which became active after the client opened its editor is also locked.
    f.engine.groups.get_mut(&f.id).unwrap().active_profile = Some("codex:b".into());
    assert!(f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"profile":{"key":"codex:b","shortThreshold":95}}})).is_err());
}
#[test]
fn live_policy_rejects_invalid_or_nonparticipant_changes_without_mutation() {
    let mut f = Fixture::new();
    for patch in [json!({"profile":{"key":"codex:b","shortThreshold":101}}), json!({"shortThreshold":95}), json!({"profile":{"key":"codex:missing","priority":1}}), json!({"pollingIntervalSeconds":0})] {
        assert!(f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":patch})).is_err());
        assert!(f.engine.groups[&f.id].policy.as_ref().unwrap().candidates[1].short_threshold.is_none());
    }
}
#[test]
fn waiting_policy_edit_schedules_usage_revalidation_without_starting_an_agent() {
    let mut f = Fixture::new();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = LoopStatus::WaitingForUsageReset;
    g.resume_at = Some(now() + 60000);
    f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"profile":{"key":"codex:b","shortThreshold":99}}})).unwrap();
    let g = &f.engine.groups[&f.id];
    assert!(g.resume_at.unwrap() <= now());
    assert_eq!(g.status, "waiting_for_usage_reset");
    assert!(g.attempts.is_empty());
}

fn claude_running(f: &mut Fixture) {
    running(f);
    for c in &mut f.engine.settings.candidates { c.agent = "claude".into(); c.auth_method = Some("setup-token".into()); }
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.policy = Some(f.engine.settings.clone());
    g.participants = f.engine.settings.candidates.iter().map(Candidate::key).collect();
    g.active_profile = Some("claude:a".into());
    g.current_provider = Some("claude".into());
    f.agent(Some("claude"));
}
#[test]
fn claude_missing_usage_can_start_and_run_until_real_threshold_arrives() {
    let mut f = Fixture::new();
    claude_running(&mut f);
    let stamp = now();
    f.engine.quota("claude:a".into(), Err("Waiting for session usage. Start this profile and send a message".into()), stamp);
    let c = f.engine.settings.candidates[0].clone();
    assert!(f.engine.eligible(&c, stamp));
    assert!(!f.engine.eligible(&c, stamp + 1), "each launch still attempts fresh usage");
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "running");
    assert!(f.engine.groups[&f.id].profiles[0].usage_pending);
    assert!(f.engine.groups[&f.id].profiles[0].error.is_none());
    assert!(f.engine.live[&f.id].interrupt_at.is_none());
    f.engine.quota("claude:a".into(), Ok(vec![window(89.0, now()+60000)]), now());
    f.engine.tick(&f.state).unwrap();
    assert!(!f.engine.groups[&f.id].profiles[0].usage_pending);
    assert_eq!(f.engine.groups[&f.id].status, "running");
    f.engine.quota("claude:a".into(), Ok(vec![window(90.0, now()+60000)]), now());
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "switching_profile");
    assert!(f.engine.live[&f.id].interrupt_at.is_some());
}
#[test]
fn runtime_usage_fallback_keeps_known_limits_and_actual_cooldowns() {
    let mut c = candidate("a"); c.agent = "claude".into(); c.auth_method = Some("setup-token".into());
    let settings = Settings::default();
    let mut q = Quota { error: Some("usage request failed".into()), ..Quota::default() };
    assert!(q.runtime_usage_fallback(&settings, &c));
    q.windows = vec![window(91.0, now()+60000)];
    assert!(!q.runtime_usage_fallback(&settings, &c));
    q.windows[0].resets_at = Some(json!(now()-1));
    assert!(q.runtime_usage_fallback(&settings, &c));
    q.blocked_until = now()+60000;
    assert!(!q.runtime_usage_fallback(&settings, &c));
    q.blocked_until = 0;
    c.agent = "codex".into();
    assert!(!q.runtime_usage_fallback(&settings, &c));
}
#[test]
fn claude_stop_failure_switches_even_when_usage_cannot_be_read() {
    let mut f = Fixture::new(); claude_running(&mut f);
    f.engine.quota("claude:a".into(), Err("Waiting for session usage".into()), now());
    let mut g = f.engine.groups[&f.id].clone();
    g.pending_work = true;
    let attempt = Uuid::new_v4().to_string();
    g.attempts.push(serde_json::from_value(json!({"id":attempt,"sessionId":f.sid(),"agent":"claude","profileId":"a","label":"A","status":"running","startedAt":now(),"endedAt":null,"reason":null})).unwrap());
    let dir = f.engine.dir(&g).join(&attempt).join("events"); fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("1.json"), json!({"kind":"StopFailure","errorCode":"usage_limit_reached"}).to_string()).unwrap();
    f.engine.groups.insert(f.id, g);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "switching_profile");
    assert!(f.engine.groups[&f.id].pending_work);
    assert!(f.engine.quotas["claude:a"].blocked_until > now());
    assert!(!f.engine.eligible(&f.engine.settings.candidates[0], 0));
    assert!(f.engine.live[&f.id].interrupt_at.is_some());
}
#[test]
fn claude_failure_normalization_does_not_scan_regular_conversations() {
    let payload = json!({"hook_event_name":"StopFailure","error":"rate_limit","last_assistant_message":"You've hit your limit"});
    let code = adapter::hook_error_code(&payload).unwrap();
    assert_eq!(adapter::limit_event(&json!({"errorCode":code})), Some(adapter::LimitEvent::UsageLimitReached));
    assert_eq!(adapter::hook_error_code(&json!({"hook_event_name":"StopFailure","error":"rate_limit","last_assistant_message":"API Error: Rate limit reached"})).as_deref(), Some("rate_limit"));
    assert!(adapter::hook_error_code(&json!({"hook_event_name":"Stop","last_assistant_message":"Tests assert quota exceeded"})).is_none());
}

#[test]
fn exhausted_profile_is_skipped_and_setup_token_launch_is_approved_without_a_sample() {
    let mut f = Fixture::new();
    struct TestProfiles(Vec<PathBuf>);
    impl Drop for TestProfiles {
        fn drop(&mut self) { for dir in &self.0 { let _ = fs::remove_dir_all(dir); } }
    }
    let mut created = TestProfiles(vec![]);
    for c in &mut f.engine.settings.candidates {
        c.agent = "claude".into(); c.auth_method = Some("setup-token".into());
        c.profile_id = Some(Uuid::new_v4().to_string());
        let dir = profile_dir(c).unwrap();
        fs::create_dir_all(dir.parent().unwrap()).unwrap();
        // Create exclusively: never open or remove an existing user profile.
        fs::create_dir(&dir).unwrap();
        created.0.push(dir.clone());
        fs::write(dir.join("oauth-token.txt"), "test-only-placeholder").unwrap();
        c.config_dir = Some(dir.to_string_lossy().into_owned());
    }
    let first = f.engine.settings.candidates[0].key();
    let second = f.engine.settings.candidates[1].key();
    let policy = f.engine.settings.clone();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.policy = Some(policy.clone()); g.participants = policy.candidates.iter().map(Candidate::key).collect();
    g.status = LoopStatus::Preparing; g.current_provider = Some("claude".into());
    let pid = std::process::id();
    f.state.manager.runtime.lock().processes.push(ProcessEntry { pid, parent_pid: None, image_name: "bridge".into(), command_args: vec![], started_at: 1 });
    let request = Uuid::new_v4().to_string();
    let live = f.engine.live.get_mut(&f.id).unwrap();
    live.dispatch = Some(Dispatch { id: request.clone(), pid, provider: "claude".into(), args: vec![], cwd: f.engine.root.to_string_lossy().into_owned() });
    live.helper_pid = Some(pid); live.guard_at = now();
    f.engine.quota(first.clone(), Ok(vec![window(91.0, now()+60000)]), now());
    f.engine.quota(second.clone(), Err("Waiting for session usage. Start this profile and send a message".into()), now());
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.live[&f.id].selected.as_deref(), Some(second.as_str()));
    let stamp = f.engine.live[&f.id].guard_at;
    f.engine.quota(second.clone(), Err("Waiting for session usage".into()), stamp);
    f.engine.tick(&f.state).unwrap();
    let g = &f.engine.groups[&f.id];
    assert_eq!(g.active_profile.as_deref(), Some(second.as_str()), "status={:?} reason={:?} events={:?}", g.status, g.reason, g.events);
    assert_eq!(g.attempts.len(), 1);
    assert!(g.events.iter().any(|e| e.kind == "USAGE_CHECK_DEFERRED"));
    assert!(f.engine.dir(g).join(format!("response-{request}.json")).is_file());
    // No native Agent is spawned by this isolated dispatch test.
    assert!(f.state.manager.runtime.lock().get_state(f.sid()).pid.is_none());
}
