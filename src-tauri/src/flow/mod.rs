pub mod model;
pub mod package;
pub mod providers;
pub mod runtime;
pub mod store;

use anyhow::{ensure, Context, Result};
use model::{Flow, Worker};
use runtime::Engine;
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing {key}"))
}
pub struct FlowService {
    pub engine: Option<Arc<Engine>>,
    pub error: Option<String>,
}
#[tauri::command]
pub async fn flow_request(
    service: tauri::State<'_, FlowService>,
    request: Value,
) -> Result<Value, String> {
    let engine = service.engine.clone().ok_or_else(|| {
        format!(
            "Flow 저장소 복구 필요: {}",
            service.error.as_deref().unwrap_or("unknown error")
        )
    })?;
    dispatch(engine, request)
        .await
        .map_err(|e| format!("{e:#}"))
}
pub async fn dispatch(e: Arc<Engine>, r: Value) -> Result<Value> {
    ensure!(r.to_string().len() <= 2 * 1024 * 1024, "Request too large");
    match text(&r, "op")? {
        "messages" => {
            let project = Path::new(text(&r, "project")?).canonicalize()?;
            Ok(json!(e.store.messages(
                project.to_str().context("Invalid project path")?
            )?))
        }
        "development_package" => package::development(&e),
        "catalog" => Ok(
            json!({"workers":e.store.definitions("worker")?,"flows":e.store.definitions("flow")?,"runs":e.store.list("run")?.into_iter().map(|mut r|{if let Some(o)=r.as_object_mut(){o.remove("workers");o.remove("flow");o.remove("results");}r}).collect::<Vec<_>>(),"tasks":e.store.list("task")?}),
        ),
        "save_worker" => {
            let w: Worker = serde_json::from_value(r["worker"].clone())?;
            model::validate_worker(&w)?;
            e.store
                .save_definition("worker", &w.id, serde_json::to_value(&w)?)
        }
        "delete_worker" => {
            e.store.archive_worker(text(&r, "id")?)?;
            Ok(json!({"archived":true}))
        }
        "save_flow" | "validate" => {
            let f: Flow = serde_json::from_value(r["flow"].clone())?;
            model::validate(&f)?;
            e.workers_for(&f)?;
            if r["op"] == "validate" {
                Ok(json!({"valid":true}))
            } else {
                e.store
                    .save_definition("flow", &f.flow_id, serde_json::to_value(&f)?)
            }
        }
        "intake" => {
            let project = Path::new(text(&r, "project")?)
                .canonicalize()
                .context("Configure project folder")?;
            let project = project.to_str().context("Invalid project path")?;
            model::identifier(text(&r, "client_id")?)?;
            if let Some(plan) = r.get("plan") {
                validate_intake_plan(&e, project, plan)?;
            }
            e.store.intake_plan(
                project,
                text(&r, "client_id")?,
                text(&r, "text")?,
                r["task_id"].as_str(),
                r.get("plan"),
            )
        }
        "intake_preview" => {
            let project = Path::new(text(&r, "project")?).canonicalize()?;
            let project_text = project.to_str().context("Invalid project path")?;
            let tasks: Vec<_> = e
                .store
                .list("task")?
                .into_iter()
                .filter(|t| t["project"] == project_text)
                .collect();
            let home = e.profile_home(r["profile_id"].as_str())?;
            let prompt=format!("Classify a project chat message into separate intents. Return only JSON {{\"intents\":[{{\"kind\":\"new|duplicate|additional_requirement|idea\",\"text\":\"intent text\",\"task_id\":null,\"flow_id\":null}}]}}. Reuse matching existing Tasks and Flows, never invent ids or definitions. Ideas do not start tasks. Existing task id required for duplicate/additional_requirement. flow_id is only a recommendation from the existing catalog. Do not execute, edit files or publish anything. Existing tasks: {}\nExisting flows: {}\nMessage: {}",json!(tasks),json!(e.store.definitions("flow")?),text(&r,"text")?);
            let result = providers::codex(
                &prompt,
                &project,
                home.as_deref(),
                false,
                120,
                Arc::new(AtomicBool::new(false)),
            )
            .await?;
            let plan: Value = serde_json::from_str(text(&result, "text")?)?;
            validate_intake_plan(&e, project_text, &plan)?;
            Ok(plan)
        }
        "update_task" => e.store.revise_task(
            text(&r, "task_id")?,
            text(&r, "text")?,
            r["expected_revision"]
                .as_u64()
                .context("expected_revision required")?,
        ),
        "run" => {
            let f: Flow = serde_json::from_value(e.store.definition(
                "flow",
                text(&r, "flow_id")?,
                r["revision"].as_u64().context("Missing revision")? as u32,
            )?)?;
            let task = e.store.get("task", text(&r, "task_id")?)?;
            e.start_with_inputs(
                f,
                task,
                text(&r, "project")?,
                serde_json::from_value(r["capabilities"].clone())?,
                r["profile_id"].as_str().map(str::to_string),
                r.get("inputs").cloned().unwrap_or_else(|| json!({})),
            )
        }
        "control" => e.control(
            text(&r, "run_id")?,
            text(&r, "action")?,
            r["node_id"].as_str(),
        ),
        "reconcile_delivery" => {
            let rid = text(&r, "run_id")?;
            let run = e.store.get("run", rid)?;
            ensure!(
                ["failed", "interrupted", "cancelled", "succeeded"]
                    .contains(&run["status"].as_str().unwrap_or("")),
                "Stop the run before reconciliation"
            );
            let branch = text(&run, "branch")?;
            let cwd = Path::new(text(&run, "workspace")?);
            let result = providers::execute(
                "gh",
                &[
                    "pr",
                    "list",
                    "--head",
                    branch,
                    "--state",
                    "all",
                    "--json",
                    "url,state,headRefOid",
                ]
                .map(String::from),
                cwd,
                None,
                60,
                Arc::new(AtomicBool::new(false)),
            )
            .await?;
            ensure!(
                result["exit_code"] == 0 && result["stdout_truncated"] != true,
                "GitHub reconciliation requires configuration: {}",
                result
            );
            let prs: Value =
                serde_json::from_str(result["stdout"].as_str().context("Missing GitHub output")?)?;
            ensure!(prs.is_array(), "Invalid GitHub reconciliation response");
            e.store.mutate("run", rid, |run| {
                run["delivery_reconciliation"] = json!({"prs":prs,"at":store::now()});
                Ok(())
            })?;
            e.store.event(rid, "delivery_reconciled", prs.clone())?;
            Ok(prs)
        }
        "retry" => {
            let run = e.store.get("run", text(&r, "run_id")?)?;
            ensure!(
                ["failed", "interrupted", "cancelled"]
                    .contains(&run["status"].as_str().unwrap_or("")),
                "Only stopped runs can start again"
            );
            if run["commit"].is_string() {
                ensure!(run["delivery_reconciliation"]["prs"].as_array().is_some_and(|p|p.is_empty()),"Reconcile remote results first; an existing PR must be inspected instead of creating another run");
            }
            // New id, current requirement revision, pinned flow; never replay external jobs in-place.
            let task = e.store.get("task", text(&run, "task_id")?)?;
            e.start_with_inputs(
                serde_json::from_value(run["flow"].clone())?,
                task,
                text(&run, "project")?,
                serde_json::from_value(
                    r.get("capabilities").cloned().unwrap_or_else(|| json!([])),
                )?,
                run["profile_id"].as_str().map(str::to_string),
                run["inputs"].clone(),
            )
        }
        "detail" => {
            let rid = text(&r, "run_id")?;
            let run = e.store.get("run", rid)?;
            let attempts = e
                .store
                .list("attempt")?
                .into_iter()
                .filter(|a| a["run_id"] == rid)
                .map(|mut a| {
                    if a["status"] == "running"
                        && run["node_states"][a["node_id"].as_str().unwrap_or("")]
                            == "waiting_approval"
                    {
                        a["status"] = json!("waiting_approval");
                        a["approval_request"] =
                            run["approval_requests"][a["node_id"].as_str().unwrap_or("")].clone();
                    }
                    a
                })
                .collect::<Vec<_>>();
            Ok(
                json!({"run":run,"attempts":attempts,"events":e.store.events(rid)?,"instructions":e.store.list("instruction")?.into_iter().filter(|i|i["task_id"]==run["task_id"]).collect::<Vec<_>>()}),
            )
        }
        "design" => {
            let kind = text(&r, "kind")?;
            ensure!(["worker", "flow"].contains(&kind), "Unknown design kind");
            let project = Path::new(text(&r, "project")?)
                .canonicalize()
                .context("Configure project folder")?;
            let home = e.profile_home(r["profile_id"].as_str())?;
            let schema = if kind == "worker" {
                json!({"id":"worker-id","revision":0,"name":"Worker","provider":"codex","instructions":"...","inputs":{},"outputs":{"type":"object"},"permissions":["agent_read"],"timeout_secs":300})
            } else {
                serde_json::to_value(example_flow())?
            };
            let prompt=format!("You design Rhyme Flow definitions. Return exactly one JSON object, no markdown. This is a proposal only: never execute commands, edit files, publish, or create accounts. Reuse existing worker versions. Preserve the current definition id and revision when editing. Schema example: {}\nNode kinds: agent,command,builtin_action,condition,approval,bounded_repeat,output. Bindings {{source: '$inputs' or ancestor node id, path: JSON pointer}}. Edges {{id,source,target,source_port:'success' or condition 'true'/'false',target_port:'input'}}. Agent definition_ref {{id,revision}}. Condition config {{binding,equals}}. Repeat config {{max_attempts:1..10,body:serial Node[],until:{{node,path,equals}}}}; no nested repeat. Builtins config.action prepare_worktree/commit/create_draft_pr. Every node needs execution_policy.timeout_secs, config, input_bindings. Capabilities command,agent_read,workspace_write,git_write,external_publish. Worker provider only codex. Existing catalog: {}\nCurrent definition: {}\nUser request: {}",schema,json!({"workers":e.store.definitions("worker")?,"flows":e.store.definitions("flow")?}),r["current"],text(&r,"prompt")?);
            let result = providers::codex(
                &prompt,
                &project,
                home.as_deref(),
                false,
                180,
                Arc::new(AtomicBool::new(false)),
            )
            .await?;
            let value: Value = serde_json::from_str(text(&result, "text")?)
                .context("Designer did not return valid JSON")?;
            if kind == "worker" {
                model::validate_worker(&serde_json::from_value(value.clone())?)?;
            } else {
                let f: Flow = serde_json::from_value(value.clone())?;
                model::validate(&f)?;
                e.workers_for(&f)?;
            }
            Ok(value)
        }
        _ => anyhow::bail!("Unknown Flow operation"),
    }
}
fn validate_intake_plan(e: &Engine, project: &str, plan: &Value) -> Result<()> {
    let intents = plan["intents"]
        .as_array()
        .context("intents array required")?;
    ensure!(
        !intents.is_empty() && intents.len() <= 20,
        "Expected 1..20 intents"
    );
    let flows = e.store.definitions("flow")?;
    for intent in intents {
        ensure!(!text(intent, "text")?.trim().is_empty(), "Empty intent");
        ensure!(
            ["new", "duplicate", "additional_requirement", "idea"].contains(&text(intent, "kind")?),
            "Invalid intent kind"
        );
        if let Some(tid) = intent["task_id"].as_str() {
            ensure!(
                e.store.get("task", tid)?["project"] == project,
                "Task belongs to another project"
            );
        }
        if ["duplicate", "additional_requirement"].contains(&text(intent, "kind")?) {
            ensure!(intent["task_id"].is_string(), "Existing task id required");
        }
        if let Some(fid) = intent["flow_id"].as_str() {
            ensure!(
                flows.iter().any(|f| f["flow_id"] == fid),
                "Unknown recommended Flow"
            );
        }
    }
    Ok(())
}
pub fn example_flow() -> Flow {
    serde_json::from_value(json!({"schema_version":1,"flow_id":"command-example","revision":0,"name":"PowerShell 확인","inputs":{},"outputs":{},"nodes":[{"id":"command","kind":"command","config":{"program":"powershell.exe","args":["-NoProfile","-Command","[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new(); Write-Output 'Rhyme Flow 준비 완료'"]},"input_bindings":{},"execution_policy":{"timeout_secs":30}},{"id":"result","kind":"output","config":{},"input_bindings":{"log":{"source":"command","path":"/stdout"}},"execution_policy":{"timeout_secs":30}}],"edges":[{"id":"command-result","source":"command","target":"result","source_port":"success","target_port":"input"}],"policies":{"concurrency":2,"timeout_secs":300,"capabilities":["command"]},"layout":{"command":{"x":60,"y":100},"result":{"x":360,"y":100}}})).expect("Built-in example must follow schema")
}

#[cfg(test)]
mod tests;
