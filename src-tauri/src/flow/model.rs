use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worker {
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub provider: String,
    pub instructions: String,
    pub inputs: Value,
    pub outputs: Value,
    pub permissions: Vec<String>,
    pub timeout_secs: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionRef {
    pub id: String,
    pub revision: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub source: String,
    pub path: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPolicy {
    pub timeout_secs: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_ref: Option<DefinitionRef>,
    pub config: Value,
    pub input_bindings: BTreeMap<String, Binding>,
    pub execution_policy: ExecutionPolicy,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub source_port: String,
    pub target_port: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policies {
    pub concurrency: usize,
    pub timeout_secs: u64,
    pub capabilities: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub schema_version: u32,
    pub flow_id: String,
    pub revision: u32,
    pub name: String,
    pub inputs: Value,
    pub outputs: Value,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub policies: Policies,
    pub layout: BTreeMap<String, Position>,
}
pub const CAPABILITIES: &[&str] = &[
    "command",
    "agent_read",
    "workspace_write",
    "git_write",
    "external_publish",
];
pub fn identifier(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "Invalid identifier: {id}"
    );
    Ok(())
}
fn capabilities(caps: &[String]) -> Result<()> {
    ensure!(
        caps.iter().all(|c| CAPABILITIES.contains(&c.as_str())),
        "Unknown capability"
    );
    Ok(())
}
pub fn validate_worker(w: &Worker) -> Result<()> {
    validate_contract(&w.inputs)?;
    validate_contract(&w.outputs)?;
    identifier(&w.id)?;
    ensure!(
        w.provider == "codex",
        "Only the Codex managed provider is implemented"
    );
    ensure!(
        !w.name.trim().is_empty() && !w.instructions.trim().is_empty(),
        "Worker name/instructions required"
    );
    ensure!(
        (1..=3600).contains(&w.timeout_secs),
        "Worker timeout must be 1..3600"
    );
    ensure!(
        w.inputs.is_object() && w.outputs.is_object(),
        "Contracts must be objects"
    );
    capabilities(&w.permissions)
}
fn validate_node(n: &Node, inner: bool) -> Result<()> {
    identifier(&n.id)?;
    ensure!(
        (1..=3600).contains(&n.execution_policy.timeout_secs),
        "Node timeout must be 1..3600"
    );
    ensure!(n.config.is_object(), "Node config must be an object");
    match n.kind.as_str() {
        "agent" => {
            let r = n
                .definition_ref
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Worker reference required"))?;
            identifier(&r.id)?;
            ensure!(r.revision > 0, "Pin worker revision");
        }
        "command" => {
            ensure!(
                n.config["program"].as_str().is_some_and(|s| !s.is_empty()),
                "Command program required"
            );
            ensure!(
                n.config["args"]
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string)),
                "args must be strings"
            );
        }
        "condition" => {
            ensure!(
                n.config["binding"].is_string() && n.config.get("equals").is_some(),
                "Condition requires binding and equals"
            );
        }
        "approval" | "output" => {}
        "builtin_action" => ensure!(
            ["prepare_worktree", "commit", "create_draft_pr"]
                .contains(&n.config["action"].as_str().unwrap_or("")),
            "Unknown builtin action"
        ),
        "bounded_repeat" => {
            ensure!(!inner, "Nested repeats forbidden");
            ensure!(
                n.config["max_attempts"]
                    .as_u64()
                    .is_some_and(|v| (1..=10).contains(&v)),
                "max_attempts must be 1..10"
            );
            let body: Vec<Node> = serde_json::from_value(n.config["body"].clone())?;
            ensure!(
                !body.is_empty() && body.len() <= 32,
                "Repeat body must have 1..32 steps"
            );
            let mut ids = BTreeSet::new();
            for node in &body {
                validate_node(node, true)?;
                ensure!(
                    node.kind != "approval" && node.kind != "builtin_action",
                    "Repeat body supports agents, commands, conditions and outputs"
                );
                for b in node.input_bindings.values() {
                    ensure!(
                        b.source == "$inputs" || ids.contains(&b.source),
                        "Repeat binding must refer to an earlier body step"
                    );
                }
                ensure!(ids.insert(node.id.clone()), "Duplicate repeat node");
            }
            ensure!(
                n.config["until"]["node"]
                    .as_str()
                    .is_some_and(|s| ids.contains(s))
                    && n.config["until"]["path"].is_string()
                    && n.config["until"].get("equals").is_some(),
                "Repeat until must reference a body result"
            );
        }
        _ => bail!("Unknown node kind: {}", n.kind),
    }
    for b in n.input_bindings.values() {
        ensure!(
            b.path.is_empty() || b.path.starts_with('/'),
            "Binding path must be a JSON pointer"
        );
    }
    Ok(())
}
pub fn validate(f: &Flow) -> Result<()> {
    validate_contract(&f.inputs)?;
    validate_contract(&f.outputs)?;
    ensure!(f.schema_version == 1, "Unsupported schema_version");
    identifier(&f.flow_id)?;
    ensure!(!f.name.trim().is_empty(), "Flow name required");
    ensure!(
        !f.nodes.is_empty() && f.nodes.len() <= 128,
        "Flow must have 1..128 nodes"
    );
    ensure!(
        f.inputs.is_object() && f.outputs.is_object(),
        "Flow contracts must be objects"
    );
    ensure!(
        (1..=8).contains(&f.policies.concurrency) && (1..=86400).contains(&f.policies.timeout_secs),
        "Invalid execution limits"
    );
    capabilities(&f.policies.capabilities)?;
    let mut nodes = BTreeMap::new();
    for n in &f.nodes {
        validate_node(n, false)?;
        ensure!(nodes.insert(n.id.clone(), n).is_none(), "Duplicate node id");
    }
    let mut edge_ids = BTreeSet::new();
    for e in &f.edges {
        identifier(&e.id)?;
        ensure!(edge_ids.insert(&e.id), "Duplicate edge id");
        ensure!(
            nodes.contains_key(&e.source) && nodes.contains_key(&e.target),
            "Dangling edge"
        );
        ensure!(e.target_port == "input", "Unknown target port");
        let conditional = nodes[&e.source].kind == "condition";
        ensure!(
            if conditional {
                ["true", "false"].contains(&e.source_port.as_str())
            } else {
                e.source_port == "success"
            },
            "Invalid source port"
        );
    }
    let mut visited = BTreeSet::new();
    while visited.len() < nodes.len() {
        let ready: Vec<_> = nodes
            .keys()
            .filter(|id| {
                !visited.contains(*id)
                    && f.edges
                        .iter()
                        .filter(|e| &e.target == *id)
                        .all(|e| visited.contains(&e.source))
            })
            .cloned()
            .collect();
        ensure!(!ready.is_empty(), "General edges must form a DAG");
        visited.extend(ready);
    }
    for n in &f.nodes {
        let mut ancestors = BTreeSet::new();
        let mut pending = vec![n.id.clone()];
        while let Some(id) = pending.pop() {
            for e in f.edges.iter().filter(|e| e.target == id) {
                if ancestors.insert(e.source.clone()) {
                    pending.push(e.source.clone());
                }
            }
        }
        for b in n.input_bindings.values() {
            ensure!(
                b.source == "$inputs" || ancestors.contains(&b.source),
                "Binding source must be an ancestor: {}",
                b.source
            );
        }
    }
    Ok(())
}
pub fn resolve(
    bindings: &BTreeMap<String, Binding>,
    inputs: &Value,
    results: &BTreeMap<String, Value>,
) -> Result<Value> {
    let mut out = serde_json::Map::new();
    for (key, b) in bindings {
        let source = if b.source == "$inputs" {
            inputs
        } else {
            results
                .get(&b.source)
                .ok_or_else(|| anyhow::anyhow!("Missing result {}", b.source))?
        };
        let value = source
            .pointer(&b.path)
            .ok_or_else(|| anyhow::anyhow!("Missing binding {}{}", b.source, b.path))?;
        out.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(out))
}

/// Deliberate v1 JSON Schema subset; unsupported constraints are never silently ignored.
pub fn validate_contract(schema: &Value) -> Result<()> {
    let object = schema
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Contract must be an object"))?;
    for key in object.keys() {
        ensure!(
            [
                "type",
                "properties",
                "required",
                "items",
                "enum",
                "additionalProperties",
                "description",
                "title"
            ]
            .contains(&key.as_str()),
            "Unsupported contract keyword: {key}"
        );
    }
    if let Some(kind) = schema.get("type") {
        ensure!(
            kind.as_str().is_some_and(|k| [
                "object", "array", "string", "number", "integer", "boolean", "null"
            ]
            .contains(&k)),
            "Invalid contract type"
        );
    }
    if let Some(properties) = schema.get("properties") {
        for value in properties
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("properties must be an object"))?
            .values()
        {
            validate_contract(value)?;
        }
    }
    if let Some(required) = schema.get("required") {
        ensure!(
            required
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string)),
            "required must be string array"
        );
    }
    if let Some(items) = schema.get("items") {
        validate_contract(items)?;
    }
    if let Some(values) = schema.get("enum") {
        ensure!(
            values.as_array().is_some_and(|a| !a.is_empty()),
            "enum must be a nonempty array"
        );
    }
    if let Some(additional) = schema.get("additionalProperties") {
        ensure!(
            additional.is_boolean(),
            "additionalProperties must be boolean"
        );
    }
    Ok(())
}
