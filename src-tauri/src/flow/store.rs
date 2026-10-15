use anyhow::{ensure, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub struct Store {
    db: Mutex<Connection>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut db = Connection::open(path)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(version <= 1, "Flow database was created by a newer app");
        if version == 0 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE definitions(kind TEXT NOT NULL,id TEXT NOT NULL,revision INTEGER NOT NULL,body TEXT NOT NULL,archived INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(kind,id,revision));
                CREATE TABLE records(kind TEXT NOT NULL,id TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(kind,id));
                CREATE TABLE messages(project TEXT NOT NULL,client_id TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(project,client_id));
                CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT,run_id TEXT NOT NULL,body TEXT NOT NULL);
                CREATE INDEX events_run ON events(run_id,seq);
                PRAGMA user_version=1;")?;
            tx.commit()?;
        }
        Ok(Self { db: Mutex::new(db) })
    }
    pub fn definitions(&self, kind: &str) -> Result<Vec<Value>> {
        let db = self.db.lock();
        let mut q = db.prepare("SELECT d.body FROM definitions d WHERE d.kind=?1 AND d.archived=0 AND d.revision=(SELECT MAX(x.revision) FROM definitions x WHERE x.kind=d.kind AND x.id=d.id) ORDER BY d.id")?;
        let rows = q.query_map([kind], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn definition(&self, kind: &str, id: &str, revision: u32) -> Result<Value> {
        let db = self.db.lock();
        let body: String = db.query_row(
            "SELECT body FROM definitions WHERE kind=?1 AND id=?2 AND revision=?3",
            params![kind, id, revision],
            |r| r.get(0),
        )?;
        Ok(serde_json::from_str(&body)?)
    }
    pub fn save_definition(&self, kind: &str, id: &str, mut value: Value) -> Result<Value> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        let latest: u32 = tx.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM definitions WHERE kind=?1 AND id=?2",
            params![kind, id],
            |r| r.get(0),
        )?;
        ensure!(
            value["revision"].as_u64() == Some(latest as u64),
            "Definition changed; reload before saving"
        );
        value["revision"] = json!(latest + 1);
        tx.execute(
            "INSERT INTO definitions(kind,id,revision,body) VALUES(?1,?2,?3,?4)",
            params![kind, id, latest + 1, value.to_string()],
        )?;
        tx.commit()?;
        Ok(value)
    }
    pub fn archive_worker(&self, id: &str) -> Result<()> {
        self.db.lock().execute(
            "UPDATE definitions SET archived=1 WHERE kind='worker' AND id=?1",
            [id],
        )?;
        Ok(())
    }
    pub fn list(&self, kind: &str) -> Result<Vec<Value>> {
        let db = self.db.lock();
        let mut q = db.prepare("SELECT body FROM records WHERE kind=?1 ORDER BY rowid DESC")?;
        let rows = q.query_map([kind], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn get(&self, kind: &str, id: &str) -> Result<Value> {
        let s: String = self.db.lock().query_row(
            "SELECT body FROM records WHERE kind=?1 AND id=?2",
            params![kind, id],
            |r| r.get(0),
        )?;
        Ok(serde_json::from_str(&s)?)
    }
    pub fn put(&self, kind: &str, id: &str, value: &Value) -> Result<()> {
        self.db.lock().execute("INSERT INTO records(kind,id,body) VALUES(?1,?2,?3) ON CONFLICT(kind,id) DO UPDATE SET body=excluded.body", params![kind,id,value.to_string()])?;
        Ok(())
    }
    pub fn mutate(
        &self,
        kind: &str,
        id: &str,
        change: impl FnOnce(&mut Value) -> Result<()>,
    ) -> Result<Value> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        let text: String = tx.query_row(
            "SELECT body FROM records WHERE kind=?1 AND id=?2",
            params![kind, id],
            |r| r.get(0),
        )?;
        let mut value = serde_json::from_str(&text)?;
        change(&mut value)?;
        tx.execute(
            "UPDATE records SET body=?3 WHERE kind=?1 AND id=?2",
            params![kind, id, value.to_string()],
        )?;
        tx.commit()?;
        Ok(value)
    }
    pub fn event(&self, run: &str, kind: &str, data: Value) -> Result<()> {
        self.db.lock().execute(
            "INSERT INTO events(run_id,body) VALUES(?1,?2)",
            params![run, json!({"kind":kind,"at":now(),"data":data}).to_string()],
        )?;
        Ok(())
    }
    pub fn events(&self, run: &str) -> Result<Vec<Value>> {
        let db = self.db.lock();
        let mut q = db.prepare("SELECT body FROM events WHERE run_id=?1 ORDER BY seq")?;
        let rows = q.query_map([run], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn messages(&self, project: &str) -> Result<Vec<Value>> {
        let db = self.db.lock();
        let mut q =
            db.prepare("SELECT body FROM messages WHERE project=?1 ORDER BY rowid DESC LIMIT 100")?;
        let rows = q.query_map([project], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    // Message deduplication and all associated task/revision/instruction changes commit together.
    pub fn intake(
        &self,
        project: &str,
        client_id: &str,
        text: &str,
        target: Option<&str>,
    ) -> Result<Value> {
        self.intake_plan(project, client_id, text, target, None)
    }
    pub fn intake_plan(
        &self,
        project: &str,
        client_id: &str,
        text: &str,
        target: Option<&str>,
        plan: Option<&Value>,
    ) -> Result<Value> {
        ensure!(
            !text.trim().is_empty() && text.len() <= 32000,
            "Message must have 1..32000 bytes"
        );
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT body FROM messages WHERE project=?1 AND client_id=?2",
                params![project, client_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(body) = existing {
            let value: Value = serde_json::from_str(&body)?;
            ensure!(
                value["text"] == text,
                "Client id reused with different text"
            );
            return Ok(value);
        }
        let mut tasks: Vec<Value> = {
            let mut q = tx.prepare("SELECT body FROM records WHERE kind='task'")?;
            let rows = q.query_map([], |r| r.get::<_, String>(0))?;
            rows.map(|r| Ok(serde_json::from_str::<Value>(&r?)?))
                .collect::<Result<Vec<_>>>()?
        };
        tasks.retain(|t| t["project"] == project);
        let intents = plan.and_then(|p| p["intents"].as_array());
        if plan.is_some() {
            ensure!(
                intents.is_some_and(|i| !i.is_empty() && i.len() <= 20),
                "Expected 1..20 classified intents"
            );
        }
        let parts: Vec<&str> = if let Some(intents) = intents {
            intents
                .iter()
                .map(|i| {
                    i["text"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Intent text required"))
                })
                .collect::<Result<Vec<_>>>()?
        } else if target.is_some() {
            vec![text.trim()]
        } else {
            text.split('\n')
                .map(|s| s.trim().trim_start_matches("- ").trim())
                .filter(|s| !s.is_empty())
                .collect()
        };
        let mut decisions = vec![];
        let mut changed = vec![];
        for (index, part) in parts.into_iter().enumerate() {
            let intent = intents.and_then(|i| i.get(index));
            let kind = intent.and_then(|i| i["kind"].as_str()).unwrap_or("new");
            ensure!(
                ["new", "duplicate", "additional_requirement", "idea"].contains(&kind),
                "Unknown intent kind"
            );
            let target = if let Some(intent) = intent {
                intent["task_id"].as_str()
            } else {
                target
            };
            if ["duplicate", "additional_requirement"].contains(&kind) {
                ensure!(target.is_some(), "Existing task required for this intent");
            }
            if kind == "idea" {
                decisions.push(json!({"kind":"idea","task_id":null,"text":part,"flow_id":intent.map(|i|&i["flow_id"])}));
                continue;
            }
            let exact = tasks.iter().find(|t| {
                t["text"]
                    .as_str()
                    .is_some_and(|s| s.trim().eq_ignore_ascii_case(part))
            });
            let chosen = if let Some(target) = target {
                Some(
                    tasks
                        .iter()
                        .find(|t| t["id"] == target)
                        .ok_or_else(|| anyhow::anyhow!("Task does not belong to project"))?
                        .clone(),
                )
            } else {
                exact.cloned()
            };
            if let Some(mut task) = chosen {
                let task_id = task["id"].as_str().unwrap().to_owned();
                if target.is_some() && kind != "duplicate" && task["text"] != part {
                    let revision = task["revision"].as_u64().unwrap_or(1) + 1;
                    task["revision"] = json!(revision);
                    task["text"] =
                        json!(format!("{}\n{}", task["text"].as_str().unwrap_or(""), part));
                    tx.execute(
                        "UPDATE records SET body=?2 WHERE kind='task' AND id=?1",
                        params![task_id, task.to_string()],
                    )?;
                    tx.execute(
                        "INSERT INTO records(kind,id,body) VALUES('requirement',?1,?2)",
                        params![format!("{task_id}:{revision}"), task.to_string()],
                    )?;
                    let instruction_id = id();
                    tx.execute("INSERT INTO records(kind,id,body) VALUES('instruction',?1,?2)", params![instruction_id,json!({"id":instruction_id,"task_id":task_id,"revision":revision,"text":part,"status":"pending_next_run","at":now()}).to_string()])?;
                    decisions.push(
                        json!({"kind":"additional_requirement","task_id":task_id,"text":part}),
                    );
                    if let Some(stored) = tasks.iter_mut().find(|t| t["id"] == task_id) {
                        *stored = task.clone();
                    }
                    changed.push(task);
                } else {
                    decisions.push(json!({"kind":"duplicate","task_id":task_id,"text":part}));
                    changed.push(task);
                }
            } else {
                let task_id = id();
                let task = json!({"id":task_id,"project":project,"text":part,"revision":1,"status":"ready","created_at":now()});
                tx.execute(
                    "INSERT INTO records(kind,id,body) VALUES('task',?1,?2)",
                    params![task_id, task.to_string()],
                )?;
                tx.execute(
                    "INSERT INTO records(kind,id,body) VALUES('requirement',?1,?2)",
                    params![format!("{task_id}:1"), task.to_string()],
                )?;
                decisions.push(json!({"kind":"new","task_id":task_id,"text":part}));
                tasks.push(task.clone());
                changed.push(task);
            }
        }
        if let Some(intents) = intents {
            for (decision, intent) in decisions.iter_mut().zip(intents) {
                decision["flow_id"] = intent["flow_id"].clone();
            }
        }
        let response = json!({"message_id":id(),"project":project,"client_id":client_id,"text":text,"decisions":decisions,"tasks":changed,"at":now(),"classification":if plan.is_some(){"reviewed_ai_plan"}else{"explicit_target_or_line_rules"}});
        tx.execute(
            "INSERT INTO messages(project,client_id,body) VALUES(?1,?2,?3)",
            params![project, client_id, response.to_string()],
        )?;
        tx.commit()?;
        Ok(response)
    }
    pub fn revise_task(&self, task_id: &str, text: &str, expected: u64) -> Result<Value> {
        ensure!(
            !text.trim().is_empty() && text.len() <= 32000,
            "Requirement must have 1..32000 bytes"
        );
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        let body: String = tx.query_row(
            "SELECT body FROM records WHERE kind='task' AND id=?1",
            [task_id],
            |r| r.get(0),
        )?;
        let mut task: Value = serde_json::from_str(&body)?;
        ensure!(
            task["revision"] == expected,
            "Task changed; reload before editing"
        );
        task["revision"] = json!(expected + 1);
        task["text"] = json!(text);
        tx.execute(
            "UPDATE records SET body=?2 WHERE kind='task' AND id=?1",
            params![task_id, task.to_string()],
        )?;
        tx.execute(
            "INSERT INTO records(kind,id,body) VALUES('requirement',?1,?2)",
            params![format!("{task_id}:{}", expected + 1), task.to_string()],
        )?;
        let instruction_id = id();
        tx.execute("INSERT INTO records(kind,id,body) VALUES('instruction',?1,?2)",params![instruction_id,json!({"id":instruction_id,"task_id":task_id,"revision":expected+1,"text":text,"status":"pending_next_run","at":now()}).to_string()])?;
        tx.commit()?;
        Ok(task)
    }
}
