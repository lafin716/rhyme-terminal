use super::*;

struct Fixture {
    engine: Engine,
    group: Group,
    session_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("rhyme-loop-regression-{}", Uuid::new_v4()));
        let engine = Engine::open_at(root.clone()).unwrap();
        let session_id = Uuid::new_v4().to_string();
        let group: Group = serde_json::from_value(json!({
            "id": Uuid::new_v4(), "name": "fixture", "workspaceId": "test", "workspaceIndex": 3,
            "cwd": root, "status": "starting", "updatedAt": now(), "pendingWork": false,
            "attempts": [{"id": Uuid::new_v4().to_string(), "agent": "claude", "label": "test",
                "status": "starting", "startedAt": now(), "expectedSessionId": session_id}]
        }))
        .unwrap();
        fs::create_dir_all(engine.attempt_dir(&group).unwrap().join("events")).unwrap();
        Self {
            engine,
            group,
            session_id,
        }
    }

    fn event(&self, sequence: u64, kind: &str, extra: Value) {
        let mut event = json!({"kind": kind, "sessionId": self.session_id, "atMs": sequence});
        for (key, value) in extra.as_object().unwrap() {
            event[key] = value.clone();
        }
        fs::write(
            self.engine
                .attempt_dir(&self.group)
                .unwrap()
                .join("events")
                .join(format!("{sequence:020}.json")),
            serde_json::to_vec(&event).unwrap(),
        )
        .unwrap();
    }

    fn finish(self) {
        let root = self.engine.root.clone();
        drop(self);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn mismatched_start_never_approves_continuation() {
    let mut f = Fixture::new();
    f.event(
        1,
        "SessionStart",
        json!({"sessionId": Uuid::new_v4().to_string(), "transcriptPath": f.engine.root.join("wrong-conversation.jsonl")}),
    );
    let mut live = Live::default();
    assert!(f.engine.read_events(&mut f.group, &mut live).is_err());
    assert!(!live.initialized);
    assert!(f.group.attempts[0].reference.is_none());
    assert!(!f
        .engine
        .attempt_dir(&f.group)
        .unwrap()
        .join("start-approved.json")
        .exists());
    f.finish();
}

#[test]
fn matching_start_approves_only_the_verified_conversation() {
    let mut f = Fixture::new();
    f.event(1, "SessionStart", json!({}));
    let mut live = Live::default();
    f.engine.read_events(&mut f.group, &mut live).unwrap();
    let approval: Value = serde_json::from_slice(
        &fs::read(
            f.engine
                .attempt_dir(&f.group)
                .unwrap()
                .join("start-approved.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(approval["sessionId"], f.session_id);
    assert!(live.initialized);
    assert_eq!(f.group.attempts[0].status, "running");
    f.finish();
}

#[tokio::test]
async fn one_parallel_tool_finishing_does_not_allow_switching() {
    let mut f = Fixture::new();
    f.event(1, "SessionStart", json!({}));
    f.event(2, "PreToolUse", json!({"toolUseId": "first"}));
    f.event(3, "PreToolUse", json!({"toolUseId": "second"}));
    f.event(
        4,
        "PostToolUse",
        json!({"toolUseId": "first", "boundary": true}),
    );
    let mut live = Live::default();
    f.engine.read_events(&mut f.group, &mut live).unwrap();
    assert!(live.boundary);
    assert_eq!(live.tools, HashSet::from(["second".into()]));
    f.group.status = "switch_pending".into();
    // No PTY is needed: the outstanding tool must prevent reaching termination.
    f.engine
        .step(&Arc::new(DaemonState::new()), &mut f.group, &mut live)
        .unwrap();
    assert_eq!(f.group.status, "switch_pending");
    assert!(live.terminating.is_none());
    f.finish();
}

#[tokio::test]
async fn resume_tracks_released_pretool_until_its_completion() {
    let mut f = Fixture::new();
    link_test_conversation(&mut f);
    let state = Arc::new(DaemonState::new());
    f.group.active_session_id = Some(Uuid::new_v4());
    f.group.status = "running".into();
    let id = f.group.id;
    f.engine.groups.insert(id, f.group.clone());
    f.engine
        .request(&state, json!({"op": "pause", "id": id}))
        .unwrap();
    f.group = f.engine.groups[&id].clone();
    f.event(
        1,
        "PreToolUse",
        json!({"toolUseId": "released-tool", "boundary": true}),
    );
    let mut live = Live::default();
    f.engine.read_events(&mut f.group, &mut live).unwrap();
    assert!(live.tools.is_empty());
    assert!(live.deferred_tools.contains("released-tool"));
    live.idle = true;
    f.engine.groups.insert(id, f.group.clone());
    f.engine.live.insert(id, live);
    f.engine
        .request(&state, json!({"op": "resume", "id": id}))
        .unwrap();
    let mut live = f.engine.live.remove(&id).unwrap();
    assert!(live.tools.contains("released-tool"));
    assert!(live.deferred_tools.is_empty());
    assert!(!live.boundary);
    assert!(!live.idle);
    let control: Value = serde_json::from_slice(
        &fs::read(f.engine.attempt_dir(&f.group).unwrap().join("control.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(control["switchRequested"], false);
    f.event(2, "PostToolUse", json!({"toolUseId": "released-tool"}));
    f.engine.read_events(&mut f.group, &mut live).unwrap();
    assert!(live.tools.is_empty());
    f.finish();
}

#[test]
fn processed_event_history_does_not_hide_new_events_after_4096_files() {
    let mut f = Fixture::new();
    let dir = f.engine.attempt_dir(&f.group).unwrap().join("events");
    for sequence in 0..4097 {
        fs::write(dir.join(format!("{sequence:020}.json")), b"{}").unwrap();
    }
    // Select the actual final directory entry so the previous pre-filter limit
    // reliably misses it regardless of filesystem enumeration ordering.
    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let latest = entries.last().unwrap();
    let mut live = Live::default();
    live.processed.extend(entries.iter().take(4096).cloned());
    fs::write(
        dir.join(latest),
        serde_json::to_vec(
            &json!({"kind": "UserPromptSubmit", "sessionId": f.session_id, "atMs": 5000}),
        )
        .unwrap(),
    )
    .unwrap();
    f.engine.read_events(&mut f.group, &mut live).unwrap();
    assert!(f.group.pending_work);
    f.finish();
}

#[tokio::test]
async fn pending_work_without_transcript_cannot_launch_a_fresh_conversation() {
    let mut f = Fixture::new();
    f.group.pending_work = true;
    let candidate: Candidate = serde_json::from_value(
        json!({"agent": "claude", "profileId": "invalid/id", "label": "test", "enabled": true}),
    )
    .unwrap();
    let count = f.group.attempts.len();
    f.engine
        .launch(
            &Arc::new(DaemonState::new()),
            &mut f.group,
            &mut Live::default(),
            candidate,
        )
        .unwrap();
    assert_eq!(f.group.status, "stopped");
    assert_eq!(f.group.attempts.len(), count);
    assert!(f.group.active_session_id.is_none());
    f.finish();
}

/// On assertion failure, terminate only the isolated child created by this test.
struct TestChildGuard {
    pid: u32,
    exited: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for TestChildGuard {
    fn drop(&mut self) {
        if !self.exited.load(Ordering::Acquire) {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("taskkill.exe")
                .args(["/PID", &self.pid.to_string(), "/T", "/F"])
                .creation_flags(0x08000000)
                .output();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn managed_switch_waits_for_real_child_exit_and_preserves_history() {
    use base64::Engine as _;

    let mut f = Fixture::new();
    let state = Arc::new(DaemonState::new());
    let marker = "RHYME_LOOP_ISOLATED_CHILD_READY";
    let session = spawn_session(
        state.events.clone(),
        format!("w3.loop-test-{}", f.group.id),
        "powershell.exe".into(),
        vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            format!("[Console]::WriteLine('{marker}'); Start-Sleep -Seconds 60"),
        ],
        Some(f.engine.root.to_string_lossy().into_owned()),
        None,
        100,
        30,
    )
    .unwrap();
    let sid = session.info.id;
    let child = TestChildGuard {
        pid: session.shell_pid,
        exited: session.exited.clone(),
    };
    state.manager.sessions.lock().insert(sid, session);

    // Wait for actual PTY output, not a synthetic scrollback fixture.
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let output = {
                let sessions = state.manager.sessions.lock();
                scrollback_snapshot(&sessions[&sid].scrollback)
            };
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(output)
                .unwrap();
            if String::from_utf8_lossy(&bytes).contains(marker) {
                break;
            }
            assert!(
                !child.exited.load(Ordering::Acquire),
                "isolated child exited before ready"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("isolated PTY did not produce its ready marker");

    let transcript = f.engine.root.join(format!("{}.jsonl", f.session_id));
    fs::write(
        &transcript,
        serde_json::to_vec(&json!({
            "sessionId": f.session_id, "type": "user",
            "message": {"role": "user", "content": "isolated test work"}
        }))
        .unwrap(),
    )
    .unwrap();
    f.group.status = "running".into();
    f.group.pending_work = true;
    f.group.active_session_id = Some(sid);
    f.group.attempts[0].session_id = Some(sid);
    f.group.attempts[0].reference = Some(SessionReference {
        agent: "claude".into(),
        id: f.session_id.clone(),
        transcript_path: transcript,
    });
    let group_id = f.group.id;
    let candidate: Candidate = serde_json::from_value(json!({
        "agent": "claude", "label": "isolated", "enabled": true
    }))
    .unwrap();
    let key = candidate.key();
    f.engine.settings.candidates = vec![candidate];
    f.engine.quota(
        key,
        Ok(vec![UsageWindow {
            kind: "short".into(),
            label: "5h".into(),
            percent_used: 100.0,
            resets_at: None,
            received_at: Some(now()),
        }]),
    );
    f.engine.groups.insert(group_id, f.group.clone());
    f.engine.live.insert(
        group_id,
        Live {
            initialized: true,
            tools: HashSet::from(["in-flight".into()]),
            ..Default::default()
        },
    );

    assert!(!f
        .engine
        .write_managed(&state, Uuid::new_v4(), b"ignored")
        .unwrap());
    f.engine.tick(&state).unwrap();
    assert_eq!(f.engine.groups[&group_id].status, "switch_pending");
    assert!(!child.exited.load(Ordering::Acquire));
    assert!(f
        .engine
        .write_managed(&state, sid, b"must not reach child")
        .is_err());
    assert_eq!(state.manager.sessions.lock().len(), 1);

    f.event(
        1,
        "PostToolUse",
        json!({"toolUseId": "in-flight", "boundary": true}),
    );
    f.engine.tick(&state).unwrap();
    f.engine.sync_guards(&state.routing);
    assert_eq!(f.engine.groups[&group_id].status, "switching");
    assert_eq!(f.engine.groups[&group_id].active_session_id, Some(sid));
    assert!(state.routing.owned.lock().contains(&sid));
    assert!(state.routing.blocked.lock().contains(&sid));
    assert_eq!(state.manager.sessions.lock().len(), 1);

    tokio::time::timeout(Duration::from_secs(15), async {
        while !child.exited.load(Ordering::Acquire) {
            assert_eq!(f.engine.groups[&group_id].attempts.len(), 1);
            assert_eq!(state.manager.sessions.lock().len(), 1);
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("managed child termination was not confirmed");

    f.engine.tick(&state).unwrap();
    f.engine.sync_guards(&state.routing);
    let group = &f.engine.groups[&group_id];
    assert_eq!(group.status, "waiting");
    assert!(group.active_session_id.is_none());
    assert_eq!(group.attempts.len(), 1);
    assert_eq!(group.attempts[0].status, "archived");
    assert!(group.attempts[0].ended_at.is_some());
    assert!(state.manager.sessions.lock().is_empty());
    assert!(!state.routing.owned.lock().contains(&sid));
    let history = fs::read_to_string(
        f.engine
            .attempt_dir(group)
            .unwrap()
            .join("scrollback.base64"),
    )
    .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(history)
        .unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains(marker));
    f.finish();
}

fn initial_prompt_fixture() -> Fixture {
    let mut f = Fixture::new();
    f.group.attempts[0].expected_session_id = None;
    f.group.attempts[0].initial_prompt = true;
    let candidate: Candidate = serde_json::from_value(json!({
        "agent":"claude", "label":"test", "enabled":true
    }))
    .unwrap();
    f.engine.settings.short_threshold = 5.0;
    f.engine.settings.candidates = vec![candidate];
    f.engine.quota(
        "claude:system".into(),
        Ok(vec![UsageWindow {
            kind: "short".into(),
            label: "5h".into(),
            percent_used: 4.0,
            resets_at: None,
            received_at: Some(now()),
        }]),
    );
    f.event(
        1,
        "SessionStart",
        json!({"transcriptPath":f.engine.root.join("not-created-yet.jsonl")}),
    );
    fs::write(
        f.engine
            .attempt_dir(&f.group)
            .unwrap()
            .join("startup-ready.json"),
        json!({"sessionId":f.session_id}).to_string(),
    )
    .unwrap();
    f
}

#[tokio::test]
async fn initial_prompt_is_idle_only_after_matching_startup_ready() {
    let mut f = initial_prompt_fixture();
    let state = Arc::new(DaemonState::new());
    let mut live = Live::default();
    let ready = f
        .engine
        .attempt_dir(&f.group)
        .unwrap()
        .join("startup-ready.json");
    fs::write(&ready, json!({"sessionId":Uuid::new_v4()}).to_string()).unwrap();
    f.engine.step(&state, &mut f.group, &mut live).unwrap();
    assert!(!live.idle);
    fs::write(&ready, json!({"sessionId":f.session_id}).to_string()).unwrap();
    f.engine.step(&state, &mut f.group, &mut live).unwrap();
    assert_eq!(f.group.status, "running");
    assert!(
        live.idle,
        "verified initial prompt must not wait forever for a Stop event"
    );
    f.finish();
}

#[tokio::test]
async fn prompt_submitted_before_startup_ready_is_not_initial_idle() {
    let mut f = initial_prompt_fixture();
    f.event(2, "UserPromptSubmit", json!({}));
    let mut live = Live::default();
    f.engine
        .step(&Arc::new(DaemonState::new()), &mut f.group, &mut live)
        .unwrap();
    assert!(!live.idle);
    assert!(f.group.pending_work);
    f.finish();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_initial_prompt_stops_without_transcript() {
    let mut f = initial_prompt_fixture();
    let state = Arc::new(DaemonState::new());
    let session = spawn_session(
        state.events.clone(),
        format!("w3.idle-test-{}", f.group.id),
        "powershell.exe".into(),
        vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            "Start-Sleep -Seconds 60".into(),
        ],
        Some(f.engine.root.to_string_lossy().into_owned()),
        None,
        100,
        30,
    )
    .unwrap();
    let sid = session.info.id;
    let _child = TestChildGuard {
        pid: session.shell_pid,
        exited: session.exited.clone(),
    };
    state.manager.sessions.lock().insert(sid, session);
    f.group.active_session_id = Some(sid);
    let id = f.group.id;
    f.engine.groups.insert(id, f.group.clone());
    f.engine.tick(&state).unwrap();
    assert_eq!(f.engine.groups[&id].status, "running");
    f.engine
        .request(&state, json!({"op":"pause","id":id}))
        .unwrap();
    f.engine
        .request(&state, json!({"op":"resume","id":id}))
        .unwrap();
    assert_eq!(
        f.engine.groups[&id].status, "stopped",
        "resume without a conversation must stop"
    );
    assert!(f.engine.live[&id].terminating.is_none());
    terminate(&state, sid).unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !_child.exited.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("isolated child did not exit before cleanup");
    drop(_child);
    state.manager.sessions.lock().remove(&sid);
    f.finish();
}

#[tokio::test]
async fn daemon_restart_resume_does_not_reuse_a_previous_idle_boundary() {
    let mut f = initial_prompt_fixture();
    let id = f.group.id;
    f.group.status = "switch_pending".into();
    f.group.active_session_id = Some(Uuid::new_v4());
    f.engine.groups.insert(id, f.group.clone());
    f.engine.live.insert(
        id,
        Live {
            idle: true,
            initialized: true,
            ..Default::default()
        },
    );
    f.engine.save().unwrap();
    f.engine = Engine::open_at(f.engine.root.clone()).unwrap();
    assert_eq!(f.engine.groups[&id].status, "stopped");
    assert!(f.engine.groups[&id].active_session_id.is_none());
    assert!(f
        .engine
        .request(
            &Arc::new(DaemonState::new()),
            json!({"op":"resume","id":id})
        )
        .is_err());
    f.finish();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_keyboard_input_before_startup_ready_prevents_initial_idle_switch() {
    let mut f = initial_prompt_fixture();
    let state = Arc::new(DaemonState::new());
    let session = spawn_session(
        state.events.clone(),
        format!("w3.input-test-{}", f.group.id),
        "powershell.exe".into(),
        vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            "Start-Sleep -Seconds 60".into(),
        ],
        Some(f.engine.root.to_string_lossy().into_owned()),
        None,
        100,
        30,
    )
    .unwrap();
    let sid = session.info.id;
    let _child = TestChildGuard {
        pid: session.shell_pid,
        exited: session.exited.clone(),
    };
    state.manager.sessions.lock().insert(sid, session);
    f.group.active_session_id = Some(sid);
    let id = f.group.id;
    f.engine.groups.insert(id, f.group.clone());
    f.engine.live.insert(id, Live::default());
    assert!(f
        .engine
        .write_managed(&state, sid, b"draft without Enter")
        .unwrap());
    f.engine.tick(&state).unwrap();
    assert!(!f.engine.live[&id].idle);
    let saved = Engine::open_at(f.engine.root.clone()).unwrap();
    assert!(!saved.groups[&id].attempts.last().unwrap().initial_prompt);
    drop(saved);
    f.engine.quota(
        "claude:system".into(),
        Ok(vec![UsageWindow {
            kind: "short".into(),
            label: "5h".into(),
            percent_used: 6.0,
            resets_at: None,
            received_at: Some(now()),
        }]),
    );
    f.engine.tick(&state).unwrap();
    assert_eq!(f.engine.groups[&id].status, "switch_pending");
    assert!(f.engine.live[&id].terminating.is_none());
    terminate(&state, sid).unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !_child.exited.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("isolated child did not exit before cleanup");
    drop(_child);
    state.manager.sessions.lock().remove(&sid);
    f.finish();
}

#[tokio::test]
async fn restart_skips_only_persisted_untouched_initial_conversations() {
    let mut f = initial_prompt_fixture();
    let mut live = Live::default();
    f.engine
        .step(&Arc::new(DaemonState::new()), &mut f.group, &mut live)
        .unwrap();
    assert!(f.group.attempts[0].reference.is_some());
    let id = f.group.id;
    f.engine.groups.insert(id, f.group.clone());
    f.engine.save().unwrap();
    f.engine = Engine::open_at(f.engine.root.clone()).unwrap();
    let group = f.engine.groups.get_mut(&id).unwrap();
    assert!(
        previous_conversation(group).is_none(),
        "never import an unused startup transcript after restart"
    );
    group.pending_work = true;
    assert!(
        previous_conversation(group).is_some(),
        "pending work must never be discarded"
    );
    group.pending_work = false;
    group.attempts[0].initial_prompt = false;
    assert!(
        previous_conversation(group).is_some(),
        "older or used conversations still require their transcript"
    );
    f.finish();
}

#[tokio::test]
async fn rename_loop_preserves_identity_and_persists_while_stopped() {
    let mut f = Fixture::new();
    let id = f.group.id;
    f.group.status = "stopped".into();
    f.engine.groups.insert(id, f.group.clone());
    let state = Arc::new(DaemonState::new());
    let value = f
        .engine
        .request(
            &state,
            json!({"op":"rename","id":id,"name":"  Checkout loop  "}),
        )
        .unwrap();
    assert_eq!(value["name"], "Checkout loop");
    assert_eq!(value["id"], id.to_string());
    assert_eq!(value["status"], "stopped");
    for name in [
        "".to_owned(),
        "   ".to_owned(),
        "x".repeat(257),
        "line\nbreak".to_owned(),
    ] {
        assert!(f
            .engine
            .request(&state, json!({"op":"rename","id":id,"name":name}))
            .is_err());
    }
    f.engine = Engine::open_at(f.engine.root.clone()).unwrap();
    assert_eq!(f.engine.groups[&id].name, "Checkout loop");
    f.finish();
}

fn link_test_conversation(f: &mut Fixture) {
    let path = f.engine.root.join(format!("{}.jsonl", f.session_id));
    fs::write(&path,json!({"sessionId":f.session_id,"type":"user","message":{"role":"user","content":"Existing work"}}).to_string()).unwrap();
    f.group.attempts[0].reference = Some(SessionReference {
        agent: "claude".into(),
        id: f.session_id.clone(),
        transcript_path: path,
    });
    f.group.attempts[0].initial_prompt = false;
}

#[tokio::test]
async fn missing_or_invalid_conversation_stops_before_pty_launch() {
    for corrupt in [false, true] {
        let mut f = Fixture::new();
        if corrupt {
            link_test_conversation(&mut f);
            fs::write(
                &f.group.attempts[0]
                    .reference
                    .as_ref()
                    .unwrap()
                    .transcript_path,
                b"not JSON",
            )
            .unwrap();
        }
        let state = Arc::new(DaemonState::new());
        let id = f.group.id;
        f.group.status = "recovery".into();
        f.engine.groups.insert(id, f.group.clone());
        f.engine
            .request(&state, json!({"op":"resume","id":id}))
            .unwrap();
        assert_eq!(f.engine.groups[&id].status, "stopped");
        assert!(state.manager.sessions.lock().is_empty());
        f.finish();
    }
}

#[tokio::test]
async fn creation_requires_explicit_start_and_never_discovers_implicitly() {
    let mut f = Fixture::new();
    f.engine.settings.candidates = vec![serde_json::from_value(json!({"agent":"claude","profileId":format!("missing_{}",Uuid::new_v4()),"label":"missing","enabled":true})).unwrap()];
    let state = Arc::new(DaemonState::new());
    let base = json!({"op":"create","workspaceId":"test","cwd":f.engine.root,"name":"new"});
    for start in [
        Value::Null,
        json!({"kind":"prompt","prompt":"  "}),
        json!({"kind":"session","sessionId":Uuid::new_v4(),"candidateKey":"claude:missing"}),
    ] {
        let mut request = base.clone();
        request["start"] = start;
        assert!(f.engine.request(&state, request).is_err());
        assert!(f.engine.groups.is_empty());
        assert!(state.manager.sessions.lock().is_empty());
    }
    let mut request = base;
    request["start"] = json!({"kind":"prompt","prompt":"Build a checkout flow"});
    request["requestId"] = json!(Uuid::new_v4());
    let value = f.engine.request(&state, request.clone()).unwrap();
    let repeated = f.engine.request(&state, request).unwrap();
    assert_eq!(value["id"], repeated["id"]);
    assert_eq!(f.engine.groups.len(), 1);
    let id = Uuid::parse_str(value["id"].as_str().unwrap()).unwrap();
    let mut group = f.engine.groups[&id].clone();
    assert_eq!(group.status, "waiting");
    assert_eq!(queued_prompt(&group), Some("Build a checkout flow"));
    assert!(previous_conversation(&group).is_none());
    assert!(!stop_without_conversation(&mut group));
    // Once an actual invocation exists, a failed startup must not replay the seed.
    let mut attempt = group.attempts[0].clone();
    attempt.status = "starting".into();
    group.attempts.push(attempt);
    assert!(queued_prompt(&group).is_none());
    assert!(stop_without_conversation(&mut group));
    assert!(state.manager.sessions.lock().is_empty());
    f.finish();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_prompt_is_sent_once_after_verified_startup() {
    struct Capture(Arc<parking_lot::Mutex<Vec<u8>>>);
    impl std::io::Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut f = initial_prompt_fixture();
    f.group.pending_work = true;
    f.group.attempts[0].continuation = Some("Build checkout".into());
    let state = Arc::new(DaemonState::new());
    let mut session = spawn_session(
        state.events.clone(),
        format!("w3.idle-test-{}", f.group.id),
        "powershell.exe".into(),
        vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            "Start-Sleep -Seconds 60".into(),
        ],
        Some(f.engine.root.to_string_lossy().into_owned()),
        None,
        100,
        30,
    )
    .unwrap();
    let sid = session.info.id;
    let _child = TestChildGuard {
        pid: session.shell_pid,
        exited: session.exited.clone(),
    };
    let bytes = Arc::new(parking_lot::Mutex::new(Vec::new()));
    // Keep ConPTY's input handle open while capturing writes. Dropping it sends
    // EOF to PowerShell and lets a fast runner retire the session before cleanup.
    let _pty_input = std::mem::replace(&mut session.writer, Box::new(Capture(bytes.clone())));
    state.manager.sessions.lock().insert(sid, session);
    f.group.active_session_id = Some(sid);

    let mut live = Live::default();
    let ready = f
        .engine
        .attempt_dir(&f.group)
        .unwrap()
        .join("startup-ready.json");
    fs::write(&ready, json!({"sessionId":Uuid::new_v4()}).to_string()).unwrap();
    f.engine.step(&state, &mut f.group, &mut live).unwrap();
    assert!(bytes.lock().is_empty());
    fs::write(&ready, json!({"sessionId":f.session_id}).to_string()).unwrap();
    f.engine.step(&state, &mut f.group, &mut live).unwrap();
    f.engine.step(&state, &mut f.group, &mut live).unwrap();
    assert_eq!(&*bytes.lock(), b"\x1b[200~Build checkout\x1b[201~\r");
    assert!(f.group.attempts[0].continuation_sent_at.is_some());
    terminate(&state, sid).unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !_child.exited.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("isolated child did not exit before cleanup");
    drop(_child);
    state.manager.sessions.lock().remove(&sid);
    f.finish();
}
