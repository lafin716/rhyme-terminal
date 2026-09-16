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
/// Selection refuses an account whose provider CLI is not installed, which
/// would otherwise make every scheduling test depend on what happens to be on
/// this machine's PATH. Put empty stub executables in front of it instead —
/// nothing in these tests ever spawns one.
fn stub_native_cli() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join("rhyme-loop-native-stubs");
        fs::create_dir_all(&dir).unwrap();
        for agent in ["claude", "codex"] {
            let stub = dir.join(format!("{agent}{}", if cfg!(windows) { ".exe" } else { "" }));
            if !stub.is_file() {
                fs::write(&stub, b"").unwrap();
            }
        }
        let mut paths = vec![dir];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
    });
}
fn candidate(id: &str) -> Candidate {
    stub_native_cli();
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
fn daemon_restart_recovers_an_in_flight_session_to_paused_not_idle() {
    let mut f = Fixture::auto_start();
    let root = f.engine.root.clone();
    let session_id = Uuid::new_v4().to_string();
    let attempt_id = Uuid::new_v4().to_string();
    {
        let g = f.engine.groups.get_mut(&f.id).unwrap();
        g.status = LoopStatus::Running;
        g.active_profile = Some("codex:a".into());
        g.current_provider = Some("codex".into());
        g.current_agent_session_id = Some(session_id.clone());
        g.attempts.push(
            serde_json::from_value(json!({
                "id": attempt_id, "sessionId": null, "agent": "codex", "profileId": "a",
                "label": "A", "status": "running", "reason": null, "startedAt": now(),
                "endedAt": null,
                "reference": {"agent": "codex", "id": session_id, "transcriptPath": root.join("dummy.jsonl")},
                "expectedSessionId": null,
            }))
            .unwrap(),
        );
    }
    f.engine.save().unwrap();
    drop(f); // release the sqlite connection before reopening the same database
    let reopened = Engine::open_at(root).unwrap();
    let g = reopened.groups.values().next().unwrap();
    // Idle would invite the user to retype the Agent command directly in the
    // terminal, which discards current_agent_session_id/handoff_context and is
    // exactly how a Profile switch loses the ability to resume. Paused forces
    // the explicit Resume action, which replays interrupt → handoff → resume.
    assert_eq!(g.status, "paused");
    assert_eq!(g.current_agent_session_id.as_deref(), Some(session_id.as_str()));
    assert_eq!(g.attempts.last().unwrap().reference.as_ref().unwrap().id, session_id);
}
#[test]
fn daemon_restart_with_no_session_history_still_lands_on_idle() {
    let mut f = Fixture::auto_start();
    let root = f.engine.root.clone();
    f.engine.groups.get_mut(&f.id).unwrap().status = LoopStatus::Preparing;
    f.engine.save().unwrap();
    drop(f);
    let reopened = Engine::open_at(root).unwrap();
    let g = reopened.groups.values().next().unwrap();
    assert_eq!(g.status, "idle");
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
        // Both forms are intercepted by the same shell gate: the Loop takes
        // the command over and nothing runs until it hands one back.
        assert_eq!(f.engine.groups[&f.id].command_args, args);
        assert!(!matches!(
            f.engine.groups[&f.id].status.as_str(),
            "idle" | "running"
        ));
        // Whatever the Loop decides, it never hands the raw command back to
        // the shell: an approved launch is always a managed one, carrying the
        // profile and the lifecycle hooks the Loop needs to follow it.
        let response = root.join(format!("response-{request}.json"));
        if response.is_file() {
            let launch: Value = serde_json::from_slice(&fs::read(&response).unwrap()).unwrap();
            assert!(
                launch["managed"] == true || launch["cancel"] == true,
                "{launch}"
            );
        }
        // Starting an Agent costs no provider request: usage is read while a
        // prompt runs, never to decide whether one may begin.
        assert!(f.engine.usage_targets().is_empty());
    }
}
#[test]
fn threshold_is_inclusive_and_applies_to_the_basis_window() {
    let mut c = candidate("a");
    let s = Settings::default();
    assert!(!over_limit(&s, &c, &[window(89.0, 0)]));
    assert!(over_limit(&s, &c, &[window(90.0, 0)]));
    assert!(over_limit(&s, &c, &[window(91.0, 0)]));
    let mut weekly = window(95.0, 0);
    weekly.kind = "weekly".into();
    // Each window keeps its own threshold, but only this account's basis
    // window switches it — see `each_accounts_own_basis_decides_...`.
    assert!(!over_limit(&s, &c, &[window(1.0, 0), weekly.clone()]));
    c.threshold_basis = ThresholdBasis::Weekly;
    assert!(over_limit(&s, &c, &[window(1.0, 0), weekly]));
}
#[test]
fn reaching_the_limit_switches_profile_at_once() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = true;
    f.engine.quota(
        "codex:a".into(),
        Ok(vec![window(90.0, now() + 60_000)]),
        now(),
    );
    f.engine.tick(&f.state).unwrap();
    let g = &f.engine.groups[&f.id];
    assert_eq!(g.status, "switching_profile");
    assert_eq!(g.runtime.status, RuntimeStatus::Interrupting);
    assert!(f.engine.live[&f.id].interrupt_at.is_some());
    assert_eq!(g.active_session_id, Some(f.sid()));
    // The window's reset is stored, so the cooldown can be computed from it
    // even if the provider never sends a limit event of its own.
    assert!(f.engine.quotas["codex:a"].blocked_until > now());
}
#[test]
fn usage_below_the_limit_keeps_the_agent_running() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = true;
    f.engine.quota(
        "codex:a".into(),
        Ok(vec![window(40.0, now() + 60_000)]),
        now(),
    );
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "running");
    assert!(f.engine.live[&f.id].interrupt_at.is_none());
}
#[test]
fn usage_is_only_read_while_a_prompt_is_running() {
    let mut f = Fixture::new();
    running(&mut f);
    // The Agent has started but has not been prompted yet: nothing is asked of
    // the provider, because nothing it could answer has changed.
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = false;
    f.engine.quotas.clear();
    assert!(f.engine.usage_targets().is_empty());
    // A prompt starts. Now the account that is spending quota — and only that
    // one — is polled.
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = true;
    let targets = f.engine.usage_targets();
    assert_eq!(
        targets.iter().map(|(key, ..)| key.clone()).collect::<Vec<_>>(),
        vec!["codex:a".to_string()]
    );
    // Monitoring off means no provider traffic at all.
    f.engine.quotas.clear();
    f.engine.groups.get_mut(&f.id).unwrap().monitoring = false;
    assert!(f.engine.usage_targets().is_empty());
}
#[test]
fn the_monitor_bar_can_ask_for_one_reading_outside_the_schedule() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = false;
    f.engine.quotas.clear();
    assert!(f.engine.usage_targets().is_empty());
    f.engine
        .request(&f.state, json!({"op":"check_usage","id":f.id}))
        .unwrap();
    assert_eq!(f.engine.usage_targets().len(), 1);
    // One-shot: it does not leave the schedule permanently forced.
    f.engine.quotas.clear();
    assert!(f.engine.usage_targets().is_empty());
}
#[test]
fn backoff_grows_exponentially_with_jitter_and_caps() {
    let d1 = backoff_delay_ms(1);
    let d2 = backoff_delay_ms(2);
    let d3 = backoff_delay_ms(3);
    let d10 = backoff_delay_ms(10);
    assert!((polling::BACKOFF_BASE_SECS * 800..=polling::BACKOFF_BASE_SECS * 1200).contains(&d1));
    assert!(d2 > d1 && d3 > d2);
    assert!(d10 <= polling::BACKOFF_MAX_SECS * 1200);
}
#[test]
fn a_throttled_usage_query_slows_polling_without_disqualifying_the_profile() {
    let mut f = Fixture::new();
    f.engine
        .quota("codex:a".into(), Ok(vec![window(42.0, now() + 60_000)]), now());
    let before = now();
    f.engine.quota(
        "codex:a".into(),
        Err(UsageError {
            message: "429 Too many requests. Try again later".into(),
            rate_limited: true,
            retry_after_ms: Some(600_000),
        }),
        before + 1,
    );
    let q = &f.engine.quotas["codex:a"];
    // Cached usage from the last success is never discarded on 429.
    assert_eq!(q.windows.len(), 1);
    assert_eq!(q.windows[0].percent_used, 42.0);
    assert_eq!(q.consecutive_failures, 1);
    assert!(q.error.as_deref().unwrap().contains("429"));
    // Being told we ask too often paces the next query. It says nothing about
    // the account's own quota, so it must not block the profile, and the
    // sample we already hold is still good enough to keep running on.
    assert_eq!(q.blocked_until, 0);
    assert!(!q.rate_limited);
    // The server's own cooldown is honoured when it is longer than our
    // exponential estimate; a shorter one never buys back a faster retry.
    let retry_in = q.next_fetch_at.saturating_sub(before);
    assert!((570_000..=630_000).contains(&retry_in), "retry_in={retry_in}");
    assert!(crate::usage::usage_windows_are_fresh(
        &q.windows,
        Duration::from_secs(polling::DEFAULT_INTERVAL_SECS)
    ));
}
#[test]
fn repeated_failures_without_retry_after_back_off_the_fetch_cadence_not_just_eligibility() {
    let mut f = Fixture::new();
    let err = || UsageError {
        message: "Usage service is temporarily unavailable".into(),
        rate_limited: false,
        retry_after_ms: None,
    };
    f.engine.quota("codex:a".into(), Err(err()), now());
    let first = f.engine.quotas["codex:a"].next_fetch_at;
    f.engine.quota("codex:a".into(), Err(err()), now() + 1);
    let second = f.engine.quotas["codex:a"].next_fetch_at;
    assert_eq!(f.engine.quotas["codex:a"].consecutive_failures, 2);
    assert!(second > first, "second backoff should be longer: {first} -> {second}");
}
#[test]
fn exhausted_profile_with_a_known_reset_skips_polling_until_near_it() {
    let mut f = Fixture::new();
    let reset = now() + 2 * 3_600_000; // 2h out, beyond MAX_RESET_SKIP_SECS
    f.engine
        .quota("codex:a".into(), Ok(vec![window(95.0, reset)]), now());
    let q = &f.engine.quotas["codex:a"];
    assert!(q.next_fetch_at >= now() + polling::MAX_RESET_SKIP_SECS * 1000 - 1_000);
    assert!(q.next_fetch_at <= now() + polling::MAX_RESET_SKIP_SECS * 1000 + 1_000);
}
#[test]
fn exhausted_profile_reset_skip_targets_the_actual_reset_when_sooner_than_the_cap() {
    let mut f = Fixture::new();
    let reset = now() + 20 * 60_000; // 20 minutes out, well under the 1h cap
    f.engine
        .quota("codex:a".into(), Ok(vec![window(95.0, reset)]), now());
    let q = &f.engine.quotas["codex:a"];
    assert!(q.next_fetch_at <= reset);
    assert!(q.next_fetch_at >= reset.saturating_sub(60_000));
}
#[test]
fn usage_targets_skips_an_exhausted_candidate_until_near_its_reset() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = true;
    let reset = now() + 20 * 60_000;
    f.engine
        .quota("codex:a".into(), Ok(vec![window(95.0, reset)]), now());
    assert!(
        !f.engine
            .usage_targets()
            .iter()
            .any(|(key, ..)| key == "codex:a"),
        "an exhausted candidate with a known reset should not be due yet"
    );
}
#[test]
fn poll_interval_speeds_up_near_the_limit_and_never_beats_the_provider_floor() {
    let mut f = Fixture::new();
    let base = 120;
    f.engine
        .quota("codex:a".into(), Ok(vec![window(50.0, now() + 60_000)]), now());
    assert_eq!(f.engine.poll_interval("codex:a", base), base);
    f.engine
        .quota("codex:a".into(), Ok(vec![window(72.0, now() + 60_000)]), now());
    assert_eq!(
        f.engine.poll_interval("codex:a", base),
        polling::HIGH_INTERVAL_SECS
    );
    f.engine
        .quota("codex:a".into(), Ok(vec![window(90.0, now() + 60_000)]), now());
    assert_eq!(
        f.engine.poll_interval("codex:a", base),
        polling::CRITICAL_INTERVAL_SECS
    );
    // Nothing is ever polled faster than the provider-imposed floor, even if
    // an older configuration named a shorter baseline.
    assert_eq!(
        f.engine.poll_interval("codex:a", 10),
        polling::MIN_INTERVAL_SECS
    );
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
fn voluntary_agent_exit_completes_the_loop() {
    let mut f = Fixture::new();
    running(&mut f);
    f.agent(None);
    f.engine.tick(&f.state).unwrap();
    // The Agent finished and left of its own accord, so the work the Loop was
    // created to carry is done: it stops managing and stops polling.
    assert_eq!(f.engine.groups[&f.id].status, "completed");
    assert!(f.engine.usage_targets().is_empty());
    // Terminal, but recoverable: Resume puts it back under management.
    f.engine
        .request(&f.state, json!({"op":"resume","id":f.id}))
        .unwrap();
    assert_ne!(f.engine.groups[&f.id].status, "completed");
}
#[test]
fn an_agent_that_exits_with_an_error_is_not_reported_as_finished() {
    let mut f = Fixture::new();
    running(&mut f);
    let dispatch = Uuid::new_v4().to_string();
    f.engine.live.get_mut(&f.id).unwrap().dispatch_id = Some(dispatch.clone());
    let root = f.engine.root.join(f.id.to_string());
    fs::write(
        root.join(format!("exit-{dispatch}.json")),
        json!({"code":1}).to_string(),
    )
    .unwrap();
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
    // Fully spent, so it blocks the profile even though the 5h window is the
    // basis here — and its later reset is the one the Loop has to wait for.
    let mut weekly = window(100.0, at + 60_000);
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
fn a_stored_reset_gates_the_cooldown_and_releases_it_without_a_new_sample() {
    let mut f = Fixture::new();
    let c = candidate("a");
    let reset = now() + 60_000;
    f.engine.quota(c.key(), Ok(vec![window(95.0, reset)]), now());
    // A spent account is blocked until the reset it reported, and that instant
    // is what the wait is computed from.
    assert!(!f.engine.eligible(&c));
    assert_eq!(f.engine.quotas[&c.key()].blocked_until, reset);
    let g = f.engine.groups[&f.id].clone();
    assert_eq!(f.engine.cooldown_until(&g), Some((c.key(), reset)));
    // Once it passes, the account is usable again on the evidence in hand —
    // no fresh provider request is needed to end a cooldown.
    f.engine.quotas.get_mut(&c.key()).unwrap().blocked_until = now() - 1;
    f.engine.quotas.get_mut(&c.key()).unwrap().windows[0].resets_at = Some(json!(now() - 1));
    assert!(f.engine.eligible(&c));
}
#[test]
fn an_account_nobody_has_polled_is_allowed_to_start() {
    let mut f = Fixture::new();
    let c = candidate("a");
    // Requirement: the first Agent starts without a usage check. Absence of
    // numbers is not evidence of exhaustion.
    f.engine.quotas.clear();
    assert!(f.engine.eligible(&c));
    // A failed read is not evidence either — the last good sample still stands.
    f.engine
        .quota(c.key(), Ok(vec![window(1.0, now() + 100_000)]), now());
    f.engine
        .quota(c.key(), Err("429 rate limited".into()), now() + 2);
    assert!(f.engine.eligible(&c));
}
#[test]
fn every_profile_spent_waits_for_the_earliest_reset_instead_of_failing() {
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
    // The account that comes back first is the one the countdown names.
    assert_eq!(
        f.engine.groups[&f.id].waiting_profile_id.as_deref(),
        Some("codex:b")
    );
    assert_eq!(f.engine.groups[&f.id].resume_at, Some(stamp + 30_000));
    let response: Value =
        serde_json::from_slice(&fs::read(root.join(format!("response-{request}.json"))).unwrap())
            .unwrap();
    assert_eq!(response["cancel"], true);
    // The cooldown and its reset instants survive a daemon restart.
    f.engine.save().unwrap();
    let restored = Engine::open_at(f.engine.root.clone()).unwrap();
    assert_eq!(restored.quotas["codex:b"].blocked_until, stamp + 30_000);
}
#[test]
fn a_cooldown_that_has_not_ended_is_re_armed_instead_of_retried() {
    let mut f = Fixture::new();
    let stamp = now();
    f.engine.quota(
        "codex:a".into(),
        Ok(vec![window(95.0, stamp + 300_000)]),
        stamp,
    );
    f.engine.quota(
        "codex:b".into(),
        Ok(vec![window(95.0, stamp + 400_000)]),
        stamp,
    );
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = "waiting_for_usage_reset".into();
    g.current_provider = Some("codex".into());
    // The previously scheduled deadline has arrived, but nothing has recovered.
    g.resume_at = Some(stamp - 1);
    f.engine.tick(&f.state).unwrap();
    let g = &f.engine.groups[&f.id];
    assert_eq!(g.status, "waiting_for_usage_reset");
    assert_eq!(g.resume_at, Some(stamp + 300_000));
    assert_eq!(g.waiting_profile_id.as_deref(), Some("codex:a"));
}
#[test]
fn a_recovered_profile_ends_the_cooldown_and_resumes() {
    let mut f = Fixture::new();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.status = "waiting_for_usage_reset".into();
    g.current_provider = Some("codex".into());
    g.resume_at = Some(now() - 1);
    f.engine.tick(&f.state).unwrap();
    assert!(matches!(
        f.engine.groups[&f.id].status.as_str(),
        "resuming" | "preparing"
    ));
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
    assert!(!f.engine.eligible(&c));
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
fn live_policy_is_scoped_persisted_and_used_by_usage_guard() {
    let mut f = Fixture::new();
    let other_id = Uuid::new_v4();
    let mut other = f.engine.groups[&f.id].clone();
    other.id = other_id;
    f.engine.groups.insert(other_id, other);
    let response = f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"strategy":"PRIORITY","pollingIntervalSeconds":60,"autoResume":false,"profile":{"key":"codex:b","shortThreshold":70,"weeklyThreshold":80,"priority":5}}})).unwrap();
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
fn weekly_window(usage: f64) -> UsageWindow {
    UsageWindow {
        label: "Weekly".into(),
        kind: "weekly".into(),
        percent_used: usage,
        resets_at: Some(json!(now() + 60_000)),
        received_at: Some(now()),
    }
}
#[test]
fn each_accounts_own_basis_decides_which_window_switches_it() {
    let settings = Settings::default();
    let mut c = candidate("a");
    let short = window(95.0, now() + 60_000);
    let weekly = weekly_window(95.0);
    // 5h basis (the default): only the 5h window is held to the 90% threshold.
    assert_eq!(c.threshold_basis, ThresholdBasis::Short);
    assert!(over_limit(&settings, &c, &[short.clone()]));
    assert!(!over_limit(&settings, &c, &[weekly.clone()]));
    // A window that is not the basis is still not ignored — once it is fully
    // spent the provider refuses the run whatever the basis says.
    assert!(over_limit(&settings, &c, &[UsageWindow { percent_used: 100.0, ..weekly.clone() }]));
    c.threshold_basis = ThresholdBasis::Weekly;
    assert!(over_limit(&settings, &c, &[weekly]));
    assert!(!over_limit(&settings, &c, &[short.clone()]));
    assert!(over_limit(&settings, &c, &[UsageWindow { percent_used: 100.0, ..short }]));
    // Per-account: the next profile keeps deciding on its own window.
    assert!(!over_limit(&settings, &candidate("b"), &[weekly_window(95.0)]));
    // A per-profile threshold override still applies, to the basis window.
    c.weekly_threshold = Some(30.0);
    assert!(over_limit(&settings, &c, &[weekly_window(31.0)]));
    c.threshold_basis = ThresholdBasis::Short;
    assert!(!over_limit(&settings, &c, &[weekly_window(31.0)]));
}
#[test]
fn an_accounts_basis_is_live_editable_per_loop_and_locked_while_it_is_active() {
    let mut f = Fixture::new();
    f.engine.quota(
        "codex:b".into(),
        Ok(vec![window(20.0, now() + 60_000), weekly_window(93.0)]),
        now(),
    );
    f.engine
        .request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"profile":{"key":"codex:b","thresholdBasis":"weekly"}}}))
        .unwrap();
    let g = &f.engine.groups[&f.id];
    assert_eq!(g.policy.as_ref().unwrap().candidates[1].threshold_basis, ThresholdBasis::Weekly);
    // Scoped: neither the global defaults nor the other account moved.
    assert_eq!(f.engine.settings.candidates[1].threshold_basis, ThresholdBasis::Short);
    assert_eq!(g.policy.as_ref().unwrap().candidates[0].threshold_basis, ThresholdBasis::Short);
    // The card now reports the weekly window, over its 90% threshold.
    let profile = g.profiles.iter().find(|p| p.key == "codex:b").unwrap();
    assert_eq!(profile.threshold_kind, "weekly");
    assert_eq!(profile.threshold, 90.0);
    assert_eq!(profile.usage, Some(93.0));
    assert_eq!(
        Engine::open_at(f.engine.root.clone()).unwrap().groups[&f.id]
            .policy
            .as_ref()
            .unwrap()
            .candidates[1]
            .threshold_basis,
        ThresholdBasis::Weekly
    );
    // Active account: refused, and the rest of the same patch is refused too.
    f.engine.groups.get_mut(&f.id).unwrap().active_profile = Some("codex:b".into());
    let error = f
        .engine
        .request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"strategy":"PRIORITY","profile":{"key":"codex:b","thresholdBasis":"short"}}}))
        .unwrap_err();
    assert!(error.to_string().contains("활성 계정"), "{error}");
    let policy = f.engine.groups[&f.id].policy.as_ref().unwrap();
    assert_eq!(policy.candidates[1].threshold_basis, ThresholdBasis::Weekly);
    assert_eq!(policy.strategy, "SMART");
    // A standby account stays editable while another one runs.
    f.engine
        .request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"profile":{"key":"codex:a","thresholdBasis":"weekly"}}}))
        .unwrap();
    assert_eq!(
        f.engine.groups[&f.id].policy.as_ref().unwrap().candidates[0].threshold_basis,
        ThresholdBasis::Weekly
    );
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
fn a_loop_without_an_explicit_participant_list_still_accepts_profile_edits() {
    let mut f = Fixture::new();
    // An empty list means "every candidate" everywhere else, including the
    // `profiles` list this editor is rendered from — a Loop saved before the
    // field existed used to have every per-profile edit refused.
    f.engine.groups.get_mut(&f.id).unwrap().participants.clear();
    f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"profile":{"key":"codex:b","shortThreshold":70}}})).unwrap();
    assert_eq!(f.engine.groups[&f.id].policy.as_ref().unwrap().candidates[1].short_threshold, Some(70.0));
    // A key that is not a candidate at all is still refused.
    assert!(f.engine.request(&f.state, json!({"op":"update_policy","id":f.id,"patch":{"profile":{"key":"codex:missing","shortThreshold":70}}})).is_err());
}
#[test]
fn live_policy_rejects_invalid_or_nonparticipant_changes_without_mutation() {
    let mut f = Fixture::new();
    for patch in [json!({"profile":{"key":"codex:b","shortThreshold":101}}), json!({"shortThreshold":95}), json!({"profile":{"key":"codex:missing","priority":1}}), json!({"pollingIntervalSeconds":0}), json!({"pollingIntervalSeconds":30})] {
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
    claude_running_with(f, "setup-token");
}
fn claude_running_with(f: &mut Fixture, auth_method: &str) {
    running(f);
    for c in &mut f.engine.settings.candidates { c.agent = "claude".into(); c.auth_method = Some(auth_method.into()); }
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.policy = Some(f.engine.settings.clone());
    g.participants = f.engine.settings.candidates.iter().map(Candidate::key).collect();
    g.active_profile = Some("claude:a".into());
    g.current_provider = Some("claude".into());
    f.agent(Some("claude"));
}
#[test]
fn an_account_with_no_readable_usage_still_runs_until_a_real_limit_arrives() {
    let mut f = Fixture::new();
    claude_running(&mut f);
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = true;
    // A Claude setup-token profile has no usage endpoint of its own; its
    // numbers only appear once it is running. That is a normal startup state,
    // not a fault, and it is no longer special-cased: *any* account with no
    // reading keeps working until something says otherwise.
    f.engine.quota("claude:a".into(), Err("Waiting for session usage. Start this profile and send a message".into()), now());
    let c = f.engine.settings.candidates[0].clone();
    assert!(f.engine.eligible(&c));
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "running");
    assert!(f.engine.live[&f.id].interrupt_at.is_none());
    // A first real reading below the limit changes nothing.
    f.engine.quota("claude:a".into(), Ok(vec![window(89.0, now()+60000)]), now());
    f.engine.tick(&f.state).unwrap();
    assert!(!f.engine.groups[&f.id].profiles[0].usage_pending);
    assert_eq!(f.engine.groups[&f.id].status, "running");
    // Reaching it switches.
    f.engine.quota("claude:a".into(), Ok(vec![window(90.0, now()+60000)]), now());
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "switching_profile");
    assert!(f.engine.live[&f.id].interrupt_at.is_some());
}
#[test]
fn a_failed_reading_never_interrupts_a_healthy_run() {
    let mut f = Fixture::new();
    claude_running_with(&mut f, "oauth");
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = true;
    f.engine.quota("claude:a".into(), Ok(vec![window(20.0, now() + 3_600_000)]), now());
    // One dropped or throttled status query used to take the running account
    // down, and then the account it switched to for the same reason.
    f.engine.quota("claude:a".into(), Err("Login expired. Sign in again through the CLI".into()), now() + 1);
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "running");
    assert!(f.engine.live[&f.id].interrupt_at.is_none());
    // The reason the number stopped moving still reaches the user.
    let g = &f.engine.groups[&f.id];
    let p = g.profiles.iter().find(|p| p.key == "claude:a").unwrap();
    assert_eq!(p.usage, Some(20.0));
    assert_eq!(p.error.as_deref(), Some("Login expired. Sign in again through the CLI"));
    assert!(!p.usage_pending);
}
#[test]
fn a_profile_that_has_never_been_read_reports_pending_not_error() {
    let mut f = Fixture::new();
    claude_running(&mut f);
    let mut g = f.engine.groups.remove(&f.id).unwrap();
    f.engine.quotas.clear();
    f.engine.profiles(&mut g);
    let p = g.profiles.iter().find(|p| p.key == "claude:a").unwrap();
    assert!(p.usage_pending);
    assert!(p.error.is_none());
    f.engine.groups.insert(f.id, g);
}
#[test]
fn a_throttled_refresh_does_not_turn_a_healthy_profile_into_an_error_card() {
    let mut f = Fixture::new();
    running(&mut f);
    let mut g = f.engine.groups.remove(&f.id).unwrap();
    f.engine.quota(
        "codex:a".into(),
        Ok(vec![window(20.0, now() + 3_600_000)]),
        now(),
    );
    f.engine.quota(
        "codex:a".into(),
        Err(UsageError {
            message: "429 Too many requests. Try again later".into(),
            rate_limited: true,
            retry_after_ms: Some(60_000),
        }),
        now() + 1,
    );
    f.engine.profiles(&mut g);
    let p = g.profiles.iter().find(|p| p.key == "codex:a").unwrap();
    // Our own status query was throttled; the account was not. With a sample
    // still in hand the card shows the profile as it is, and the 20% reading
    // stays on screen instead of being replaced by an error state.
    assert!(
        !matches!(p.status, ProfileStatus::RateLimited | ProfileStatus::Error),
        "status={:?}",
        p.status
    );
    assert_eq!(p.usage, Some(20.0));
    // The reason the number stopped moving still reaches the user.
    assert!(p.error.as_deref().unwrap().contains("429"));
    f.engine.groups.insert(f.id, g);
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
    assert!(!f.engine.eligible(&f.engine.settings.candidates[0]));
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

/// Drives one launch with two real profile directories and returns the
/// position, in the configured order, of the account the Loop picked.
/// `spend_first` makes the account listed first look exhausted.
fn launched_position(spend_first: bool) -> usize {
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
    let order: Vec<String> = f.engine.settings.candidates.iter().map(Candidate::key).collect();
    let policy = f.engine.settings.clone();
    let g = f.engine.groups.get_mut(&f.id).unwrap();
    g.policy = Some(policy.clone()); g.participants = order.clone();
    g.status = LoopStatus::Preparing; g.current_provider = Some("claude".into());
    let pid = std::process::id();
    f.state.manager.runtime.lock().processes.push(ProcessEntry { pid, parent_pid: None, image_name: "bridge".into(), command_args: vec![], started_at: 1 });
    let request = Uuid::new_v4().to_string();
    let live = f.engine.live.get_mut(&f.id).unwrap();
    live.initial_start = true;
    live.dispatch = Some(Dispatch { id: request.clone(), pid, provider: "claude".into(), args: vec![], cwd: f.engine.root.to_string_lossy().into_owned() });
    live.helper_pid = Some(pid);
    if spend_first {
        f.engine.quota(order[0].clone(), Ok(vec![window(91.0, now() + 60000)]), now());
    }
    // The other account has no usable reading, which is the normal state of one
    // that has never run: it must not stand in the way of starting.
    f.engine.quota(order[1].clone(), Err("Waiting for session usage. Start this profile and send a message".into()), now());
    f.engine.tick(&f.state).unwrap();
    let g = &f.engine.groups[&f.id];
    assert_eq!(g.attempts.len(), 1, "status={:?} reason={:?} events={:?}", g.status, g.reason, g.events);
    assert!(f.engine.dir(g).join(format!("response-{request}.json")).is_file());
    // No native Agent is spawned by this isolated dispatch test.
    assert!(f.state.manager.runtime.lock().get_state(f.sid()).pid.is_none());
    let chosen = g.active_profile.clone().unwrap();
    order.iter().position(|key| *key == chosen).unwrap()
}
#[test]
fn the_first_launch_follows_the_configured_order_without_reading_usage() {
    assert_eq!(launched_position(false), 0);
}
#[test]
fn a_spent_account_is_skipped_and_the_next_one_launches_without_a_sample() {
    assert_eq!(launched_position(true), 1);
}
#[test]
fn monitoring_can_be_switched_off_and_back_on_from_the_terminal() {
    let mut f = Fixture::new();
    running(&mut f);
    f.engine.groups.get_mut(&f.id).unwrap().pending_work = true;
    f.engine
        .request(&f.state, json!({"op":"set_monitoring","id":f.id,"enabled":false}))
        .unwrap();
    // Off: no provider traffic, and a spent account is left alone rather than
    // being interrupted behind the user's back.
    assert!(f.engine.usage_targets().is_empty());
    f.engine.quota("codex:a".into(), Ok(vec![window(95.0, now() + 60_000)]), now());
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "running");
    assert!(f.engine.live[&f.id].interrupt_at.is_none());
    // Back on: the same evidence is acted on immediately.
    f.engine
        .request(&f.state, json!({"op":"set_monitoring","id":f.id,"enabled":true}))
        .unwrap();
    f.engine.tick(&f.state).unwrap();
    assert_eq!(f.engine.groups[&f.id].status, "switching_profile");
}
