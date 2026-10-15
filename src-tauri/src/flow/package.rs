//! Development is a catalog package over the generic engine, not a special Task type.
use super::{
    model::{Flow, Worker},
    runtime::Engine,
    store,
};
use anyhow::Result;
use serde_json::{json, Value};
pub fn development(e: &Engine) -> Result<Value> {
    let mut workers = vec![];
    for (id,name,instructions,permissions,outputs) in [
        ("rhyme-developer","개발자","Inspect the existing project and implement the task with minimal changes. Do not commit or publish. Return JSON {\"summary\":string}.",vec!["agent_read","workspace_write"],json!({"type":"object","required":["summary"],"properties":{"summary":{"type":"string"}}})),
        ("rhyme-reviewer","검토자","Review the current changes and fresh validation result against the task. Do not edit files or publish. Return JSON {\"approved\":boolean,\"summary\":string}; approve only when correct and verified.",vec!["agent_read"],json!({"type":"object","required":["approved","summary"],"properties":{"approved":{"type":"boolean"},"summary":{"type":"string"}}})),
    ] {
        let w=match e.store.definition("worker",id,1){Ok(w)=>w,Err(_)=>{let w:Worker=serde_json::from_value(json!({"id":id,"revision":0,"name":name,"provider":"codex","instructions":instructions,"inputs":{"type":"object"},"outputs":outputs,"permissions":permissions,"timeout_secs":1200}))?;e.store.save_definition("worker",id,serde_json::to_value(w)?)?}};
        workers.push(w);
    }
    let node = |id: &str, kind: &str, config: Value, bindings: Value| json!({"id":id,"kind":kind,"config":config,"input_bindings":bindings,"execution_policy":{"timeout_secs":1800}});
    let mut dev = node(
        "develop",
        "agent",
        json!({"prompt":"Implement the goal in this isolated worktree. Use prior validation failures to correct the implementation."}),
        json!({"goal":{"source":"$inputs","path":"/goal"}}),
    );
    dev["definition_ref"] = json!({"id":"rhyme-developer","revision":1});
    let build = node(
        "verify",
        "command",
        if cfg!(windows) {
            json!({"program":"powershell.exe","args":["-NoProfile","-Command","& pnpm.cmd build; exit $LASTEXITCODE"],"verification":true})
        } else {
            json!({"program":"/bin/sh","args":["-lc","pnpm build"],"verification":true})
        },
        json!({}),
    );
    let mut review = node(
        "review",
        "agent",
        json!({"review":true,"prompt":"Review all changes and the verification result. Return approved false if anything remains."}),
        json!({"validation":{"source":"verify","path":""},"implementation":{"source":"develop","path":""}}),
    );
    review["definition_ref"] = json!({"id":"rhyme-reviewer","revision":1});
    let nodes = vec![
        node(
            "workspace",
            "builtin_action",
            json!({"action":"prepare_worktree"}),
            json!({}),
        ),
        node(
            "implement_verify_review",
            "bounded_repeat",
            json!({"max_attempts":3,"body":[dev,build,review],"until":{"node":"review","path":"/approved","equals":true}}),
            json!({"goal":{"source":"$inputs","path":"/goal"}}),
        ),
        node(
            "inspect_changes",
            "approval",
            json!({"message":"변경 파일과 검증 증거를 확인하세요. commit 노드의 files 목록을 실행 전에 지정해야 합니다."}),
            json!({"result":{"source":"implement_verify_review","path":""}}),
        ),
        node(
            "commit",
            "builtin_action",
            json!({"action":"commit","files":[],"message":"Implement verified Rhyme Flow task"}),
            json!({}),
        ),
        node(
            "draft_pr",
            "builtin_action",
            json!({"action":"create_draft_pr","title":"Rhyme Flow implementation"}),
            json!({}),
        ),
    ];
    let ids = [
        "workspace",
        "implement_verify_review",
        "inspect_changes",
        "commit",
        "draft_pr",
    ];
    let edges:Vec<_>=ids.windows(2).map(|pair|json!({"id":format!("{}-{}",pair[0],pair[1]),"source":pair[0],"target":pair[1],"source_port":"success","target_port":"input"})).collect();
    let layout: serde_json::Map<_, _> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.to_string(), json!({"x":40+i*230,"y":90})))
        .collect();
    let flow: Flow = serde_json::from_value(
        json!({"schema_version":1,"flow_id":store::id(),"revision":0,"name":"개발 → 검증 → 리뷰 → Draft PR","inputs":{},"outputs":{},"nodes":nodes,"edges":edges,"policies":{"concurrency":1,"timeout_secs":14400,"capabilities":["command","agent_read","workspace_write","git_write","external_publish"]},"layout":layout}),
    )?;
    super::model::validate(&flow)?;
    Ok(
        json!({"flow":flow,"workers":workers,"configuration_required":["프로젝트에 맞게 verify 명령을 수정하세요.","commit.files에 검토할 파일 경로를 명시하세요.","Codex 로그인과 GitHub gh 인증이 필요합니다."]}),
    )
}
