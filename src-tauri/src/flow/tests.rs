use super::*;
use serde_json::json;
use std::path::PathBuf;

fn temp() -> PathBuf {
    let p = std::env::temp_dir().join(format!("rhyme-flow-test-{}", store::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn shared_frontend_fixture_round_trips_without_contract_drift() {
    let json: Value =
        serde_json::from_str(include_str!("../../../src/lib/flow-contract.fixture.json")).unwrap();
    let flow: Flow = serde_json::from_value(json.clone()).unwrap();
    model::validate(&flow).unwrap();
    assert_eq!(serde_json::to_value(flow).unwrap(), json);
}
#[tokio::test]
async fn approval_node_respects_its_own_timeout() {
    let root = temp();
    let e = Engine::open(root.join("data")).unwrap();
    let mut f = example_flow();
    f.nodes.truncate(1);
    f.edges.clear();
    f.nodes[0].kind = "approval".into();
    f.nodes[0].config = json!({});
    f.nodes[0].execution_policy.timeout_secs = 1;
    let r = e
        .start(f, task(&e, &root), root.to_str().unwrap(), vec![], None)
        .unwrap();
    let r = wait(&e, r["id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "failed");
    assert!(r["error"].as_str().unwrap().contains("timed out"));
    assert!(e
        .control(r["id"].as_str().unwrap(), "approve", Some("command"))
        .is_err());
}
#[test]
fn rejects_cycles_unknown_ports_and_nonancestor_bindings() {
    let mut f = example_flow();
    model::validate(&f).unwrap();
    let mut e = f.edges[0].clone();
    e.id = "cycle".into();
    e.source = "result".into();
    e.target = "command".into();
    f.edges.push(e);
    assert!(model::validate(&f).is_err());
    f.edges.pop();
    f.edges[0].source_port = "false".into();
    assert!(model::validate(&f).is_err());
    f.edges[0].source_port = "success".into();
    f.nodes[0].input_bindings.insert(
        "future".into(),
        model::Binding {
            source: "result".into(),
            path: "".into(),
        },
    );
    assert!(model::validate(&f).is_err());
}
#[test]
fn validates_bounded_repeat_limits_and_nested_body() {
    let mut f = example_flow();
    let body = f.nodes[0].clone();
    f.nodes[0].kind = "bounded_repeat".into();
    f.nodes[0].config = json!({"max_attempts":2,"body":[body],"until":{"node":"command","path":"/exit_code","equals":0}});
    model::validate(&f).unwrap();
    f.nodes[0].config["max_attempts"] = json!(11);
    assert!(model::validate(&f).is_err());
    f.nodes[0].config["max_attempts"] = json!(2);
    f.nodes[0].config["body"][0]["kind"] = json!("bounded_repeat");
    assert!(model::validate(&f).is_err());
}
#[test]
fn immutable_versions_and_optimistic_concurrency() {
    let path = temp();
    let db = store::Store::open(&path.join("db.sqlite")).unwrap();
    let f = serde_json::to_value(example_flow()).unwrap();
    let first = db.save_definition("flow", "example", f.clone()).unwrap();
    assert_eq!(first["revision"], 1);
    assert!(db.save_definition("flow", "example", f).is_err());
    let mut second = first.clone();
    second["name"] = json!("Changed");
    db.save_definition("flow", "example", second).unwrap();
    assert_eq!(db.definition("flow", "example", 1).unwrap(), first);
}
#[test]
fn intake_deduplicates_and_pins_requirement_history() {
    let path = temp();
    let db = store::Store::open(&path.join("db.sqlite")).unwrap();
    let first = db
        .intake("project", "client-1", "Build\nReview", None)
        .unwrap();
    assert_eq!(first["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(
        db.intake("project", "client-1", "Build\nReview", None)
            .unwrap(),
        first
    );
    assert!(db.intake("project", "client-1", "Different", None).is_err());
    let id = first["tasks"][0]["id"].as_str().unwrap();
    let next = db
        .intake("project", "client-2", "Add tests", Some(id))
        .unwrap();
    assert_eq!(next["tasks"][0]["revision"], 2);
    assert_eq!(
        db.get("requirement", &format!("{id}:1")).unwrap()["text"],
        "Build"
    );
    assert!(db
        .intake("other", "client-3", "Add tests", Some(id))
        .is_err());
}
#[test]
fn restart_marks_inflight_run_and_attempt_interrupted() {
    let path = temp();
    {
        let e = Engine::open(path.clone()).unwrap();
        e.store
            .put("run", "run", &json!({"id":"run","status":"running"}))
            .unwrap();
        e.store
            .put(
                "attempt",
                "attempt",
                &json!({"id":"attempt","run_id":"run","status":"running"}),
            )
            .unwrap();
    }
    let e = Engine::open(path).unwrap();
    assert_eq!(e.store.get("run", "run").unwrap()["status"], "interrupted");
    assert_eq!(
        e.store.get("attempt", "attempt").unwrap()["status"],
        "interrupted"
    );
    assert!(e.control("run", "approve", Some("node")).is_err());
}
async fn wait(e: &Arc<Engine>, rid: &str) -> Value {
    for _ in 0..300 {
        let r = e.store.get("run", rid).unwrap();
        if ["succeeded", "failed", "cancelled"].contains(&r["status"].as_str().unwrap_or("")) {
            return r;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("Run did not terminate");
}
fn task(e: &Arc<Engine>, project: &Path) -> Value {
    e.store
        .intake(
            project.canonicalize().unwrap().to_str().unwrap(),
            &store::id(),
            "test",
            None,
        )
        .unwrap()["tasks"][0]
        .clone()
}
#[tokio::test]
async fn command_run_persists_results_and_exit_failure() {
    let root = temp();
    let e = Engine::open(root.join("data")).unwrap();
    let t = task(&e, &root);
    let f = example_flow();
    let r = e
        .start(
            f.clone(),
            t.clone(),
            root.to_str().unwrap(),
            vec!["command".into()],
            None,
        )
        .unwrap();
    let r = wait(&e, r["id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "succeeded", "{r}");
    assert!(r["results"]["result"]["log"]
        .as_str()
        .unwrap()
        .contains("Rhyme Flow"));
    let mut f = f;
    f.nodes[0].config["args"] = json!(["-NoProfile", "-Command", "exit 7"]);
    let r = e
        .start(f, t, root.to_str().unwrap(), vec!["command".into()], None)
        .unwrap();
    let r = wait(&e, r["id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "failed", "{r}");
}
#[tokio::test]
async fn missing_capability_waits_for_explicit_approval_and_reject_stops() {
    let root = temp();
    let e = Engine::open(root.join("data")).unwrap();
    let r = e
        .start(
            example_flow(),
            task(&e, &root),
            root.to_str().unwrap(),
            vec![],
            None,
        )
        .unwrap();
    let rid = r["id"].as_str().unwrap();
    for _ in 0..100 {
        if e.store.get("run", rid).unwrap()["node_states"]["command"] == "waiting_approval" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    e.control(rid, "reject", Some("command")).unwrap();
    assert_eq!(wait(&e, rid).await["status"], "failed");
}
#[tokio::test]
async fn branch_skips_unchosen_path() {
    let root = temp();
    let e = Engine::open(root.join("data")).unwrap();
    let mut f = example_flow();
    f.nodes[0].kind = "condition".into();
    f.nodes[0].config = json!({"binding":"goal","equals":"different"});
    f.nodes[0].input_bindings.insert(
        "goal".into(),
        model::Binding {
            source: "$inputs".into(),
            path: "/goal".into(),
        },
    );
    f.edges[0].source_port = "true".into();
    let r = e
        .start(f, task(&e, &root), root.to_str().unwrap(), vec![], None)
        .unwrap();
    let r = wait(&e, r["id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "succeeded");
    assert_eq!(r["node_states"]["result"], "skipped");
}
#[test]
fn binding_paths_and_contract_types_fail_closed() {
    let binding = std::collections::BTreeMap::from([(
        "x".into(),
        model::Binding {
            source: "$inputs".into(),
            path: "/missing".into(),
        },
    )]);
    assert!(model::resolve(&binding, &json!({}), &Default::default()).is_err());
    assert!(runtime::contract(&json!({"type":"object","required":["approved"],"properties":{"approved":{"type":"boolean"}}}),&json!({"approved":"true"})).is_err());
}

#[test]
fn development_package_pins_workers_and_requires_explicit_file_selection() {
    let e = Engine::open(temp()).unwrap();
    let p = package::development(&e).unwrap();
    let f: Flow = serde_json::from_value(p["flow"].clone()).unwrap();
    model::validate(&f).unwrap();
    e.workers_for(&f).unwrap();
    assert_eq!(f.policies.concurrency, 1);
    assert_eq!(f.nodes[3].config["files"], json!([]));
    assert_eq!(package::development(&e).unwrap()["workers"], p["workers"]);
}
#[test]
fn classified_intents_reuse_tasks_keep_ideas_and_replace_requirements() {
    let root = temp();
    let e = Engine::open(root.join("data")).unwrap();
    let task = task(&e, &root);
    let tid = task["id"].as_str().unwrap();
    let project = task["project"].as_str().unwrap();
    let plan = json!({"intents":[{"kind":"idea","text":"Maybe later","task_id":null},{"kind":"additional_requirement","text":"Add tests","task_id":tid},{"kind":"additional_requirement","text":"And docs","task_id":tid},{"kind":"new","text":"Separate work","task_id":null}]});
    let result = e
        .store
        .intake_plan(project, "planned", "raw message", Some(tid), Some(&plan))
        .unwrap();
    assert_eq!(result["tasks"].as_array().unwrap().len(), 3);
    assert_eq!(e.store.get("task", tid).unwrap()["revision"], 3);
    let revised = e.store.revise_task(tid, "Replacement", 3).unwrap();
    assert_eq!(revised["text"], "Replacement");
    assert_eq!(revised["revision"], 4);
    assert!(e.store.revise_task(tid, "Stale", 3).is_err());
}
#[tokio::test]
async fn repeat_failure_runs_the_entire_body_again() {
    let root = temp();
    let e = Engine::open(root.join("data")).unwrap();
    let mut f = example_flow();
    let mut dev = f.nodes[0].clone();
    dev.id = "develop".into();
    dev.config["args"] = json!([
        "-NoProfile",
        "-Command",
        "Add-Content -LiteralPath passes.txt -Value develop"
    ]);
    let mut verify = dev.clone();
    verify.id = "verify".into();
    verify.config["args"]=json!(["-NoProfile","-Command","if ((Get-Content -LiteralPath passes.txt).Count -lt 2) { exit 8 }; Write-Output 'verified'"]);
    f.nodes.truncate(1);
    f.edges.clear();
    f.nodes[0].kind = "bounded_repeat".into();
    f.nodes[0].config = json!({"max_attempts":3,"body":[dev,verify],"until":{"node":"verify","path":"/exit_code","equals":0}});
    let r = e
        .start(
            f,
            task(&e, &root),
            root.to_str().unwrap(),
            vec!["command".into()],
            None,
        )
        .unwrap();
    let r = wait(&e, r["id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "succeeded", "{r}");
    assert_eq!(r["results"]["command"]["attempts"], 2);
    assert_eq!(
        e.store
            .list("attempt")
            .unwrap()
            .iter()
            .filter(|a| a["node_id"].as_str().unwrap().ends_with("__develop"))
            .count(),
        2
    );
}
#[tokio::test]
async fn independent_nodes_overlap_and_command_consumes_bound_stdin() {
    let root = temp();
    let e = Engine::open(root.join("data")).unwrap();
    let mut f = example_flow();
    let mut first = f.nodes[0].clone();
    first.config["args"] = json!([
        "-NoProfile",
        "-Command",
        "Start-Sleep -Milliseconds 600; Write-Output first"
    ]);
    let mut second = first.clone();
    second.id = "second".into();
    f.nodes = vec![first, second];
    f.edges.clear();
    let r = e
        .start(
            f,
            task(&e, &root),
            root.to_str().unwrap(),
            vec!["command".into()],
            None,
        )
        .unwrap();
    let rid = r["id"].as_str().unwrap();
    assert_eq!(wait(&e, rid).await["status"], "succeeded");
    let events = e.store.events(rid).unwrap();
    let first_finish = events
        .iter()
        .position(|v| v["kind"] == "node_finished")
        .unwrap();
    assert_eq!(
        events[..first_finish]
            .iter()
            .filter(|v| v["kind"] == "node_started")
            .count(),
        2
    );
    let mut f = example_flow();
    f.nodes[0].input_bindings.insert(
        "payload".into(),
        model::Binding {
            source: "$inputs".into(),
            path: "/goal".into(),
        },
    );
    f.nodes[0].config["stdin_binding"] = json!("payload");
    f.nodes[0].config["args"] = json!(["-NoProfile", "-Command", "[Console]::In.ReadToEnd()"]);
    let r = e
        .start(
            f,
            task(&e, &root),
            root.to_str().unwrap(),
            vec!["command".into()],
            None,
        )
        .unwrap();
    let r = wait(&e, r["id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "succeeded", "{r}");
    assert_eq!(
        r["results"]["command"]["stdout"].as_str().unwrap().trim(),
        "test"
    );
}
#[tokio::test]
async fn worktree_is_isolated_and_does_not_modify_parent_changes() {
    let root = temp();
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=FlowTest",
            "-c",
            "user.email=flow@example.invalid",
            "-c",
            "core.hooksPath=NUL",
            "commit",
            "--allow-empty",
            "-m",
            "Initial",
        ],
    ] {
        let result = providers::execute(
            "git",
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &repo,
            None,
            30,
            cancel.clone(),
        )
        .await
        .unwrap();
        assert_eq!(result["exit_code"], 0, "{result}");
    }
    std::fs::write(repo.join("user-change.txt"), "preserve me").unwrap();
    let e = Engine::open(root.join("data")).unwrap();
    let mut f = example_flow();
    f.nodes.truncate(1);
    f.edges.clear();
    f.nodes[0].kind = "builtin_action".into();
    f.nodes[0].config = json!({"action":"prepare_worktree"});
    f.policies.capabilities = vec!["git_write".into()];
    f.policies.concurrency = 1;
    let r = e
        .start(
            f,
            task(&e, &repo),
            repo.to_str().unwrap(),
            vec!["git_write".into()],
            None,
        )
        .unwrap();
    let r = wait(&e, r["id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "succeeded", "{r}");
    assert_eq!(
        std::fs::read_to_string(repo.join("user-change.txt")).unwrap(),
        "preserve me"
    );
    assert!(!Path::new(r["workspace"].as_str().unwrap())
        .join("user-change.txt")
        .exists());
}
