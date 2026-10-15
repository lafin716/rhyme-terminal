use super::{
    model::{self, Flow, Node, Worker},
    providers,
    store::{id, now, Store},
};
use anyhow::{bail, ensure, Context, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub struct Engine {
    pub store: Store,
    pub root: PathBuf,
    active: Mutex<BTreeMap<String, Arc<AtomicBool>>>,
    _ownership: std::fs::File,
}
fn str_field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing {key}"))
}
impl Engine {
    pub fn open(root: PathBuf) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&root)?;
        let mut lock_options = std::fs::OpenOptions::new();
        lock_options.read(true).write(true).create(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            lock_options.share_mode(0);
        }
        let ownership = lock_options
            .open(root.join("engine.lock"))
            .context("Another app owns the Flow engine; close its Flow process before recovery")?;
        let e = Arc::new(Self {
            store: Store::open(&root.join("flow.sqlite3"))?,
            root,
            active: Mutex::new(BTreeMap::new()),
            _ownership: ownership,
        });
        for run in e.store.list("run")? {
            if ["queued", "running", "waiting_approval", "cancelling"]
                .contains(&run["status"].as_str().unwrap_or(""))
            {
                let rid = str_field(&run, "id")?;
                if let Some(task_id) = run["task_id"].as_str() {
                    if e.store.get("task", task_id).is_ok() {
                        e.store.mutate("task", task_id, |task| {
                            task["status"] = json!("ready");
                            Ok(())
                        })?;
                    }
                }
                e.store.mutate("run",rid,|r| { r["status"]=json!("interrupted"); r["error"]=json!("App stopped. Inspect evidence before starting a new run; external effects are never replayed automatically."); Ok(()) })?;
                e.store.event(
                    rid,
                    "reconciled_after_restart",
                    json!({"policy":"inspect_then_new_run"}),
                )?;
                e.store.put(
                    "job",
                    rid,
                    &json!({"id":rid,"run_id":rid,"status":"interrupted"}),
                )?;
            }
        }
        for a in e.store.list("attempt")? {
            if a["status"] == "running" {
                e.store.mutate("attempt", str_field(&a, "id")?, |a| {
                    a["status"] = json!("interrupted");
                    Ok(())
                })?;
            }
        }
        Ok(e)
    }
    pub fn workers_for(&self, f: &Flow) -> Result<BTreeMap<String, Worker>> {
        let mut workers = BTreeMap::new();
        let mut nodes = f.nodes.clone();
        for n in &f.nodes {
            if n.kind == "bounded_repeat" {
                nodes.extend(serde_json::from_value::<Vec<Node>>(
                    n.config["body"].clone(),
                )?);
            }
        }
        for n in nodes {
            if let Some(r) = n.definition_ref {
                let worker: Worker =
                    serde_json::from_value(self.store.definition("worker", &r.id, r.revision)?)?;
                model::validate_worker(&worker)?;
                workers.insert(format!("{}:{}", r.id, r.revision), worker);
            }
        }
        Ok(workers)
    }
    pub fn start(
        self: &Arc<Self>,
        f: Flow,
        task: Value,
        project: &str,
        caps: Vec<String>,
        profile: Option<String>,
    ) -> Result<Value> {
        self.start_with_inputs(f, task, project, caps, profile, json!({}))
    }
    pub fn start_with_inputs(
        self: &Arc<Self>,
        f: Flow,
        task: Value,
        project: &str,
        caps: Vec<String>,
        profile: Option<String>,
        mut inputs: Value,
    ) -> Result<Value> {
        model::validate(&f)?;
        ensure!(
            caps.iter()
                .all(|c| model::CAPABILITIES.contains(&c.as_str())),
            "Unknown capability"
        );
        let project =
            std::fs::canonicalize(project).context("Configure an existing project folder")?;
        ensure!(project.is_dir(), "Project must be a directory");
        ensure!(
            Path::new(str_field(&task, "project")?).canonicalize()? == project,
            "Task project mismatch"
        );
        if let Some(ref p) = profile {
            model::identifier(p)?;
        }
        let workers = self.workers_for(&f)?;
        if f.nodes.iter().any(|n| n.kind == "builtin_action") {
            ensure!(
                f.policies.concurrency == 1,
                "Development delivery flows require serial workspace access"
            );
        }
        ensure!(inputs.is_object(), "Run inputs must be an object");
        inputs["goal"] = task["text"].clone();
        inputs["project"] = json!(project);
        contract(&f.inputs, &inputs)?;
        let mut active = self.active.lock();
        for existing in self.store.list("run")? {
            ensure!(
                existing["task_id"] != task["id"]
                    || !["queued", "running", "waiting_approval", "cancelling"]
                        .contains(&existing["status"].as_str().unwrap_or("")),
                "Task already has an active run"
            );
        }
        let rid = id();
        let run = json!({"id":rid,"task_id":task["id"],"flow_id":f.flow_id,"revision":f.revision,"status":"queued","created_at":now(),"project":project,"flow":f,"workers":workers,"requirement":task,"inputs":inputs,"capabilities":caps,"profile_id":profile,"node_states":{},"results":{},"approvals":{},"workspace":null});
        self.store.put("run", &rid, &run)?;
        self.store.mutate("task", str_field(&task, "id")?, |task| {
            task["status"] = json!("running");
            Ok(())
        })?;
        for instruction in self.store.list("instruction")? {
            if instruction["task_id"] == task["id"]
                && instruction["revision"].as_u64() <= task["revision"].as_u64()
                && instruction["status"] == "pending_next_run"
            {
                self.store
                    .mutate("instruction", str_field(&instruction, "id")?, |i| {
                        i["status"] = json!("applied");
                        i["run_id"] = json!(rid);
                        Ok(())
                    })?;
            }
        }
        self.store.put(
            "job",
            &rid,
            &json!({"id":rid,"run_id":rid,"status":"queued"}),
        )?;
        self.store.event(
            &rid,
            "run_created",
            json!({"requirement_revision":task["revision"]}),
        )?;
        let cancel = Arc::new(AtomicBool::new(false));
        active.insert(rid.clone(), cancel.clone());
        drop(active);
        let engine = self.clone();
        tauri::async_runtime::spawn(async move {
            let outcome = engine.drive(&rid, cancel.clone()).await;
            let status = if cancel.load(Ordering::Acquire) {
                "cancelled"
            } else if outcome.is_ok() {
                "succeeded"
            } else {
                "failed"
            };
            let error = outcome.err().map(|e| format!("{e:#}"));
            let _ = engine.store.mutate("run", &rid, |r| {
                r["status"] = json!(status);
                r["error"] = json!(error);
                r["finished_at"] = json!(now());
                Ok(())
            });
            let _ = engine
                .store
                .put("job", &rid, &json!({"id":rid,"run_id":rid,"status":status}));
            if let Ok(run) = engine.store.get("run", &rid) {
                if let Some(tid) = run["task_id"].as_str() {
                    let _ = engine.store.mutate("task", tid, |t| {
                        t["status"] = json!(if status == "succeeded"
                            && t["revision"] == run["requirement"]["revision"]
                        {
                            "completed"
                        } else {
                            "ready"
                        });
                        Ok(())
                    });
                }
            }
            let _ =
                engine
                    .store
                    .event(&rid, "run_finished", json!({"status":status,"error":error}));
            engine.active.lock().remove(&rid);
        });
        Ok(run)
    }
    pub fn control(&self, rid: &str, action: &str, node: Option<&str>) -> Result<Value> {
        if action == "cancel" {
            let active = self.active.lock();
            let cancel = active
                .get(rid)
                .context("Run is not active; inspect interrupted run")?;
            cancel.store(true, Ordering::Release);
            return self.store.mutate("run", rid, |r| {
                r["status"] = json!("cancelling");
                Ok(())
            });
        }
        ensure!(
            ["approve", "reject"].contains(&action),
            "Unknown control action"
        );
        let node = node.context("node_id required")?;
        let r = self.store.mutate("run", rid, |r| {
            ensure!(
                ![
                    "failed",
                    "succeeded",
                    "cancelled",
                    "interrupted",
                    "cancelling"
                ]
                .contains(&r["status"].as_str().unwrap_or("")),
                "Run not awaiting approval"
            );
            ensure!(
                r["node_states"][node] == "waiting_approval",
                "Node not awaiting approval"
            );
            ensure!(r["approvals"][node].is_null(), "Approval already resolved");
            r["approvals"][node] = json!({"decision":action,"at":now()});
            Ok(())
        })?;
        self.store.event(
            rid,
            "approval_decided",
            json!({"node_id":node,"decision":action}),
        )?;
        Ok(r)
    }
    async fn approval(
        &self,
        rid: &str,
        node: &str,
        reason: Value,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        self.store.mutate("run", rid, |r| {
            r["node_states"][node] = json!("waiting_approval");
            r["status"] = json!("waiting_approval");
            r["approval_requests"][node] = reason.clone();
            Ok(())
        })?;
        self.store.event(
            rid,
            "approval_requested",
            json!({"node_id":node,"reason":reason}),
        )?;
        loop {
            ensure!(!cancel.load(Ordering::Acquire), "Cancelled");
            let r = self.store.get("run", rid)?;
            match r["approvals"][node]["decision"].as_str() {
                Some("approve") => break,
                Some("reject") => bail!("User rejected node {node}"),
                _ => tokio::time::sleep(Duration::from_millis(150)).await,
            }
        }
        self.store.mutate("run", rid, |r| {
            r["node_states"][node] = json!("running");
            r["status"] = json!("running");
            Ok(())
        })?;
        Ok(())
    }
    async fn drive(self: &Arc<Self>, rid: &str, cancel: Arc<AtomicBool>) -> Result<()> {
        self.store.mutate("run", rid, |r| {
            r["status"] = json!("running");
            Ok(())
        })?;
        self.store
            .put("job", rid, &json!({"id":rid,"status":"running"}))?;
        let run = self.store.get("run", rid)?;
        let f: Flow = serde_json::from_value(run["flow"].clone())?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(f.policies.timeout_secs);
        let mut done = BTreeSet::new();
        let mut skipped = BTreeSet::new();
        let mut running = BTreeSet::new();
        let mut results = BTreeMap::new();
        let mut jobs = tokio::task::JoinSet::new();
        loop {
            if cancel.load(Ordering::Acquire) || tokio::time::Instant::now() >= deadline {
                cancel.store(true, Ordering::Release);
                while jobs.join_next().await.is_some() {}
                bail!("Cancelled or run timeout reached");
            }
            for n in &f.nodes {
                if done.contains(&n.id) || running.contains(&n.id) {
                    continue;
                }
                let incoming: Vec<_> = f.edges.iter().filter(|e| e.target == n.id).collect();
                if !incoming.iter().all(|e| done.contains(&e.source)) {
                    continue;
                }
                let enabled = incoming.is_empty()
                    || incoming.iter().any(|e| {
                        !skipped.contains(&e.source)
                            && (e.source_port == "success"
                                || results
                                    .get(&e.source)
                                    .and_then(|v: &Value| v["value"].as_bool())
                                    == Some(e.source_port == "true"))
                    });
                if !enabled {
                    done.insert(n.id.clone());
                    skipped.insert(n.id.clone());
                    self.store.mutate("run", rid, |r| {
                        r["node_states"][&n.id] = json!("skipped");
                        Ok(())
                    })?;
                    continue;
                }
                if running.len() >= f.policies.concurrency {
                    break;
                }
                let bound = model::resolve(&n.input_bindings, &run["inputs"], &results)?;
                let engine = self.clone();
                let node = n.clone();
                let run_id = rid.to_owned();
                let token = cancel.clone();
                running.insert(n.id.clone());
                jobs.spawn(async move {
                    let result = engine.execute_node(&run_id, &node, bound, token, 0).await;
                    (node.id, result)
                });
            }
            if done.len() == f.nodes.len() {
                break;
            }
            if jobs.is_empty() {
                bail!("No runnable node; unresolved dependencies");
            }
            tokio::select! {
                result=jobs.join_next()=>{
                    let (nid,result)=result.context("Missing node job")?.context("Node task panicked")?;
                    running.remove(&nid);
                    match result {
                        Ok(value)=>{done.insert(nid.clone());results.insert(nid.clone(),value.clone());self.store.mutate("run",rid,|r|{r["results"][&nid]=value;r["node_states"][&nid]=json!("succeeded");Ok(())})?;}
                        Err(error)=>{
                            cancel.store(true,Ordering::Release);
                            while jobs.join_next().await.is_some() {};
                            // Internal failure cancels siblings, but the run is a failure rather than a user cancellation.
                            cancel.store(false,Ordering::Release);
                            return Err(error.context(format!("Node {nid}")));
                        }
                    }
                }
                _=tokio::time::sleep(Duration::from_millis(100))=>{}
            }
        }
        contract(&f.outputs, &json!(results))?;
        Ok(())
    }
    fn execute_node<'a>(
        self: &'a Arc<Self>,
        rid: &'a str,
        node: &'a Node,
        bound: Value,
        cancel: Arc<AtomicBool>,
        iteration: u64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            let aid = id();
            let run = self.store.get("run", rid)?;
            let mut attempt = json!({"id":aid,"run_id":rid,"node_id":node.id,"iteration":iteration,"status":"running","input":bound,"requirement_revision":run["requirement"]["revision"],"started_at":now()});
            self.store.put("attempt", &aid, &attempt)?;
            self.store.mutate("run", rid, |r| {
                r["node_states"][&node.id] = json!("running");
                Ok(())
            })?;
            self.store.event(
                rid,
                "node_started",
                json!({"node_id":node.id,"attempt_id":aid}),
            )?;
            let result = tokio::time::timeout(
                Duration::from_secs(node.execution_policy.timeout_secs),
                self.perform(rid, node, &bound, cancel.clone(), iteration),
            )
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("Node {} timed out", node.id)));
            attempt["finished_at"] = json!(now());
            match &result {
                Ok(value) => {
                    attempt["status"] = json!("succeeded");
                    attempt["output"] = value.clone();
                }
                Err(e) => {
                    attempt["status"] = json!(if cancel.load(Ordering::Acquire) {
                        "cancelled"
                    } else {
                        "failed"
                    });
                    attempt["error"] = json!(format!("{e:#}"));
                }
            }
            self.store.put("attempt", &aid, &attempt)?;
            self.store.mutate("run", rid, |r| {
                r["node_states"][&node.id] = attempt["status"].clone();
                Ok(())
            })?;
            self.store.event(
                rid,
                "node_finished",
                json!({"node_id":node.id,"attempt_id":aid,"status":attempt["status"]}),
            )?;
            result
        })
    }
    async fn perform(
        self: &Arc<Self>,
        rid: &str,
        n: &Node,
        bound: &Value,
        cancel: Arc<AtomicBool>,
        iteration: u64,
    ) -> Result<Value> {
        ensure!(!cancel.load(Ordering::Acquire), "Cancelled");
        let r = self.store.get("run", rid)?;
        let timeout = n.execution_policy.timeout_secs;
        let worker = if n.kind == "agent" {
            let d = n.definition_ref.as_ref().context("Missing worker")?;
            Some(serde_json::from_value::<Worker>(
                r["workers"][format!("{}:{}", d.id, d.revision)].clone(),
            )?)
        } else {
            None
        };
        let mut required: Vec<String> = match n.kind.as_str() {
            "command" => vec!["command".into()],
            "agent" => vec!["agent_read".into()],
            "builtin_action" => vec!["git_write".into()],
            _ => vec![],
        };
        if let Some(w) = &worker {
            required.extend(w.permissions.clone());
        }
        let publish = n.config["action"] == "create_draft_pr";
        if publish {
            required.push("external_publish".into());
        }
        let permitted: Vec<String> =
            serde_json::from_value(r["flow"]["policies"]["capabilities"].clone())?;
        ensure!(
            required.iter().all(|c| permitted.contains(c)),
            "Node requests capabilities not permitted by the pinned Flow"
        );
        let granted: Vec<String> = serde_json::from_value(r["capabilities"].clone())?;
        let publish_cwd =
            PathBuf::from(r["workspace"].as_str().unwrap_or(str_field(&r, "project")?));
        let publish_remote = if publish {
            ensure!(
                granted.iter().any(|c| c == "external_publish"),
                "Explicit external_publish run permission required"
            );
            Some(
                self.git(
                    &publish_cwd,
                    &["remote", "get-url", "origin"],
                    cancel.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        let delivery=publish_remote.as_ref().map(|remote|json!({"remote":redact_remote(remote),"branch":r["branch"],"commit":r["commit"],"action":"push and create Draft PR"}));
        if n.kind == "approval" || publish || required.iter().any(|c| !granted.contains(c)) {
            self.approval(rid,&n.id,json!({"node":n,"capabilities":required,"input":bound,"iteration":iteration,"delivery":delivery}),&cancel).await?;
        }
        if let Some(remote) = publish_remote {
            ensure!(
                self.git(
                    &publish_cwd,
                    &["remote", "get-url", "origin"],
                    cancel.clone()
                )
                .await?
                    == remote,
                "Remote changed after approval"
            );
        }
        let r = self.store.get("run", rid)?;
        let cwd = PathBuf::from(r["workspace"].as_str().unwrap_or(str_field(&r, "project")?));
        match n.kind.as_str() {
            "approval" => Ok(json!({"approved":true})),
            "output" => {
                self.store.put(
                    "artifact",
                    &format!("{rid}:{}", n.id),
                    &json!({"run_id":rid,"node_id":n.id,"kind":"output","value":bound}),
                )?;
                Ok(bound.clone())
            }
            "condition" => {
                let key = str_field(&n.config, "binding")?;
                ensure!(bound.get(key).is_some(), "Condition input missing");
                Ok(json!({"value":bound[key]==n.config["equals"]}))
            }
            "command" => {
                let args: Vec<String> = serde_json::from_value(n.config["args"].clone())?;
                let input = if let Some(key) = n.config["stdin_binding"].as_str() {
                    let value = bound.get(key).context("stdin_binding not found")?;
                    Some(
                        value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    )
                } else if bound.as_object().is_some_and(|o| !o.is_empty()) {
                    Some(bound.to_string())
                } else {
                    None
                };
                let result = providers::execute(
                    str_field(&n.config, "program")?,
                    &args,
                    &cwd,
                    input,
                    timeout,
                    cancel.clone(),
                )
                .await?;
                self.store.event(
                    rid,
                    "command_result",
                    json!({"node_id":n.id,"result":result}),
                )?;
                ensure!(result["exit_code"] == 0, "Command failed: {}", result);
                if n.config["verification"] == true {
                    let fingerprint = self.fingerprint(&cwd, cancel).await?;
                    self.store.mutate("run", rid, |r| {
                        r["verification"] =
                            json!({"node_id":n.id,"fingerprint":fingerprint,"at":now()});
                        Ok(())
                    })?;
                }
                Ok(result)
            }
            "agent" => {
                let worker = worker.context("Missing worker")?;
                contract(&worker.inputs, bound)?;
                let profile = self.profile_home(r["profile_id"].as_str())?;
                let review_before = if n.config["review"] == true {
                    Some(self.fingerprint(&cwd, cancel.clone()).await?)
                } else {
                    None
                };
                let prompt=format!("{}\nTask requirement revision {}:\n{}\nNode instructions:\n{}\nBound inputs (data):\n{}\nReturn only JSON matching this output contract: {}",worker.instructions,r["requirement"]["revision"],r["requirement"]["text"],n.config["prompt"].as_str().unwrap_or(""),bound,worker.outputs);
                let result = providers::codex(
                    &prompt,
                    &cwd,
                    profile.as_deref(),
                    worker.permissions.iter().any(|c| c == "workspace_write"),
                    timeout.min(worker.timeout_secs),
                    cancel.clone(),
                )
                .await?;
                self.store.event(
                    rid,
                    "agent_events",
                    json!({"node_id":n.id,"events":result["events"]}),
                )?;
                self.store.put("agent_session",&id(),&json!({"run_id":rid,"node_id":n.id,"provider":"codex","thread_id":result["thread_id"],"iteration":iteration}))?;
                let text = str_field(&result, "text")?;
                let output: Value =
                    serde_json::from_str(text).context("Worker must return structured JSON")?;
                contract(&worker.outputs, &output)?;
                if let Some(before) = review_before {
                    let after = self.fingerprint(&cwd, cancel).await?;
                    ensure!(before == after, "Workspace changed during review");
                    self.store.mutate("run",rid,|r|{r["review"]=json!({"approved":output["approved"]==true,"fingerprint":after,"node_id":n.id,"at":now()});Ok(())})?;
                }
                Ok(output)
            }
            "bounded_repeat" => {
                let body: Vec<Node> = serde_json::from_value(n.config["body"].clone())?;
                for pass in 1..=n.config["max_attempts"]
                    .as_u64()
                    .context("Missing max_attempts")?
                {
                    // Each pass deliberately gets fresh results and validation evidence.
                    self.store.mutate("run", rid, |r| {
                        r["verification"] = Value::Null;
                        r["review"] = Value::Null;
                        Ok(())
                    })?;
                    let mut results = BTreeMap::new();
                    let mut pass_failed = false;
                    for child in &body {
                        let mut child = child.clone();
                        let original = child.id.clone();
                        child.id = format!("{}__{}__{}", n.id, pass, original);
                        let inputs = model::resolve(&child.input_bindings, bound, &results)?;
                        match self
                            .execute_node(rid, &child, inputs, cancel.clone(), pass)
                            .await
                        {
                            Ok(output) => {
                                results.insert(original, output);
                            }
                            Err(error) => {
                                ensure!(!cancel.load(Ordering::Acquire), "Cancelled");
                                self.store.event(rid,"repeat_pass_failed",json!({"node_id":n.id,"pass":pass,"error":format!("{error:#}")}))?;
                                pass_failed = true;
                                break;
                            }
                        }
                    }
                    let until = &n.config["until"];
                    if !pass_failed
                        && results
                            .get(str_field(until, "node")?)
                            .and_then(|v| v.pointer(until["path"].as_str().unwrap_or("")))
                            == Some(&until["equals"])
                    {
                        return Ok(json!({"attempts":pass,"results":results}));
                    }
                }
                bail!("Repeat exhausted all attempts")
            }
            "builtin_action" => self.builtin(rid, n, &cwd, cancel).await,
            _ => bail!("Unsupported node"),
        }
    }
    pub fn profile_home(&self, profile: Option<&str>) -> Result<Option<PathBuf>> {
        if let Some(p) = profile {
            model::identifier(p)?;
            let home = self
                .root
                .parent()
                .context("Missing app data")?
                .join("accounts")
                .join("codex")
                .join(p);
            ensure!(home.is_dir(), "Codex profile requires configuration");
            Ok(Some(home))
        } else {
            Ok(None)
        }
    }
    async fn git(&self, cwd: &Path, args: &[&str], cancel: Arc<AtomicBool>) -> Result<String> {
        let result = providers::execute(
            "git",
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            cwd,
            None,
            120,
            cancel,
        )
        .await?;
        ensure!(result["exit_code"] == 0, "Git failed: {}", result);
        ensure!(
            result["stdout_truncated"] != true && result["stderr_truncated"] != true,
            "Git output exceeds evidence limit; refusing incomplete evidence"
        );
        Ok(result["stdout"]
            .as_str()
            .unwrap_or("")
            .trim_end_matches(['\r', '\n'])
            .to_string())
    }
    async fn fingerprint(&self, cwd: &Path, cancel: Arc<AtomicBool>) -> Result<String> {
        // Includes content hashes of untracked files as well as the tracked diff and HEAD.
        let head = self
            .git(cwd, &["rev-parse", "HEAD"], cancel.clone())
            .await?;
        let diff = self
            .git(cwd, &["diff", "HEAD", "--binary"], cancel.clone())
            .await?;
        let untracked = self
            .git(
                cwd,
                &["ls-files", "--others", "--exclude-standard", "-z"],
                cancel.clone(),
            )
            .await?;
        let mut files = Vec::new();
        for path in untracked.split('\0').filter(|s| !s.is_empty()) {
            files.push(
                self.git(cwd, &["hash-object", "--", path], cancel.clone())
                    .await?,
            );
        }
        Ok(json!({"head":head,"diff":diff,"untracked":untracked,"hashes":files}).to_string())
    }
    async fn builtin(
        &self,
        rid: &str,
        n: &Node,
        cwd: &Path,
        cancel: Arc<AtomicBool>,
    ) -> Result<Value> {
        let r = self.store.get("run", rid)?;
        match str_field(&n.config, "action")? {
            "prepare_worktree" => {
                ensure!(r["workspace"].is_null(), "Workspace already created");
                let branch = format!("rhyme-flow/{rid}");
                let path = self.root.join("worktrees").join(rid);
                std::fs::create_dir_all(path.parent().unwrap())?;
                self.git(
                    cwd,
                    &[
                        "worktree",
                        "add",
                        "-b",
                        &branch,
                        path.to_str().context("Invalid workspace path")?,
                        "HEAD",
                    ],
                    cancel,
                )
                .await?;
                self.store.mutate("run", rid, |r| {
                    r["workspace"] = json!(path);
                    r["branch"] = json!(branch);
                    Ok(())
                })?;
                self.store.put(
                    "workspace",
                    rid,
                    &json!({"id":rid,"kind":"git_worktree","path":path,"branch":branch}),
                )?;
                Ok(json!({"path":path,"branch":branch}))
            }
            "commit" => {
                ensure!(
                    r["workspace"].as_str() == cwd.to_str(),
                    "Commit requires engine-created worktree"
                );
                let fingerprint = self.fingerprint(cwd, cancel.clone()).await?;
                ensure!(
                    r["verification"]["fingerprint"] == fingerprint,
                    "Successful verification of current changes required"
                );
                ensure!(
                    r["review"]["approved"] == true && r["review"]["fingerprint"] == fingerprint,
                    "Approved structured review of current changes required"
                );
                let files: Vec<String> = serde_json::from_value(n.config["files"].clone())
                    .context("Commit requires explicit files list")?;
                ensure!(!files.is_empty(), "Select files to commit");
                for file in &files {
                    let p = Path::new(file);
                    ensure!(
                        !p.is_absolute()
                            && p.components()
                                .all(|c| matches!(c, std::path::Component::Normal(_))),
                        "Only relative explicit file paths allowed"
                    );
                    ensure!(
                        !file.starts_with('-')
                            && !file.contains('*')
                            && !file.contains('?')
                            && !file.starts_with(':'),
                        "Git pathspec magic forbidden"
                    );
                }
                ensure!(
                    self.git(cwd, &["diff", "--cached", "--name-only"], cancel.clone())
                        .await?
                        .is_empty(),
                    "Unexpected staged files; inspect workspace"
                );
                let mut args = vec!["add", "--"];
                args.extend(files.iter().map(String::as_str));
                self.git(cwd, &args, cancel.clone()).await?;
                self.git(
                    cwd,
                    &[
                        "-c",
                        "core.hooksPath=NUL",
                        "commit",
                        "-m",
                        n.config["message"]
                            .as_str()
                            .unwrap_or("Implement Rhyme Flow task"),
                    ],
                    cancel.clone(),
                )
                .await?;
                let sha = self.git(cwd, &["rev-parse", "HEAD"], cancel).await?;
                self.store.mutate("run", rid, |r| {
                    r["commit"] = json!(sha);
                    Ok(())
                })?;
                self.store.put(
                    "changeset",
                    rid,
                    &json!({"id":rid,"run_id":rid,"commit":sha,"files":files}),
                )?;
                Ok(json!({"commit":sha}))
            }
            "create_draft_pr" => {
                let sha = str_field(&r, "commit")?;
                ensure!(
                    self.git(cwd, &["rev-parse", "HEAD"], cancel.clone())
                        .await?
                        == sha,
                    "Commit changed since validation"
                );
                ensure!(
                    self.git(cwd, &["status", "--porcelain"], cancel.clone())
                        .await?
                        .is_empty(),
                    "Worktree must be clean"
                );
                let branch = str_field(&r, "branch")?;
                let existing = providers::execute(
                    "gh",
                    &[
                        "pr", "list", "--head", branch, "--state", "all", "--json", "url",
                    ]
                    .map(String::from),
                    cwd,
                    None,
                    60,
                    cancel.clone(),
                )
                .await?;
                ensure!(
                    existing["exit_code"] == 0,
                    "GitHub requires configuration: {}",
                    existing
                );
                let prs: Value = serde_json::from_str(existing["stdout"].as_str().unwrap_or("[]"))?;
                if let Some(pr) = prs.as_array().and_then(|a| a.first()) {
                    return Ok(json!({"url":pr["url"],"reconciled":true}));
                }
                self.git(
                    cwd,
                    &["push", "--set-upstream", "origin", branch],
                    cancel.clone(),
                )
                .await?;
                let body = self.root.join(format!("{rid}-pr-body.md"));
                std::fs::write(&body,format!("{}\n\nVerified commit: `{sha}`\n\nCreated from pinned Flow {} revision {}.",r["requirement"]["text"].as_str().unwrap_or(""),r["flow_id"],r["revision"]))?;
                let result = providers::execute(
                    "gh",
                    &vec![
                        "pr".into(),
                        "create".into(),
                        "--draft".into(),
                        "--head".into(),
                        branch.into(),
                        "--title".into(),
                        n.config["title"]
                            .as_str()
                            .unwrap_or("Rhyme Flow changes")
                            .into(),
                        "--body-file".into(),
                        body.to_string_lossy().to_string(),
                    ],
                    cwd,
                    None,
                    120,
                    cancel,
                )
                .await?;
                ensure!(
                    result["exit_code"] == 0,
                    "PR creation failed; inspect remote before retry: {}",
                    result
                );
                let url = result["stdout"].as_str().unwrap_or("").trim();
                self.store.put(
                    "artifact",
                    rid,
                    &json!({"id":rid,"kind":"draft_pr","url":url,"commit":sha}),
                )?;
                Ok(json!({"url":url,"commit":sha}))
            }
            _ => bail!("Unsupported action"),
        }
    }
}
pub fn contract(schema: &Value, value: &Value) -> Result<()> {
    model::validate_contract(schema)?;
    if let Some(values) = schema["enum"].as_array() {
        ensure!(values.contains(value), "Value outside contract enum");
    }
    if let Some(items) = schema.get("items") {
        if let Some(values) = value.as_array() {
            for value in values {
                contract(items, value)?;
            }
        }
    }
    if schema["additionalProperties"] == false {
        if let Some(values) = value.as_object() {
            for key in values.keys() {
                ensure!(
                    schema["properties"].get(key).is_some(),
                    "Unexpected property {key}"
                );
            }
        }
    }
    if let Some(kind) = schema["type"].as_str() {
        ensure!(
            match kind {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "boolean" => value.is_boolean(),
                "number" => value.is_number(),
                "integer" => value.is_i64() || value.is_u64(),
                "null" => value.is_null(),
                _ => false,
            },
            "Output contract type mismatch"
        );
    }
    if let Some(required) = schema["required"].as_array() {
        for key in required {
            ensure!(
                value
                    .get(key.as_str().context("Invalid required property")?)
                    .is_some(),
                "Required output missing: {key}"
            );
        }
    }
    if let Some(properties) = schema["properties"].as_object() {
        for (key, schema) in properties {
            if let Some(v) = value.get(key) {
                contract(schema, v)?;
            }
        }
    }
    Ok(())
}
fn redact_remote(remote: &str) -> String {
    if let Some((scheme, rest)) = remote.split_once("://") {
        if let Some((_, host)) = rest.rsplit_once('@') {
            return format!("{scheme}://{host}");
        }
    }
    remote.to_owned()
}
