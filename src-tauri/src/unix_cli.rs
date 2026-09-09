//! Unix command helpers reuse the GUI executable, without a Python/PowerShell dependency.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn read(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
fn write(path: &Path, value: &Value) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, serde_json::to_vec(value)?)?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(temp);
        return Err(error.into());
    }
    Ok(())
}
pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub fn command(mode: &str) -> Result<String> {
    Ok(format!(
        "{} {mode}",
        quote(&std::env::current_exe()?.to_string_lossy())
    ))
}
fn stdin() -> Result<String> {
    let mut input = String::new();
    std::io::stdin()
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 4 * 1024 * 1024 {
        bail!("Input too large");
    }
    Ok(input)
}
fn event(root: &Path, value: &Value) -> Result<()> {
    fs::create_dir_all(root.join("events"))?;
    write(
        &root
            .join("events")
            .join(format!("{:020}-{}.json", now(), uuid::Uuid::new_v4())),
        value,
    )
}
pub fn hook() -> Result<()> {
    let root = std::path::PathBuf::from(
        std::env::var_os("RHYME_LOOP_ATTEMPT_DIR").context("Missing attempt")?,
    );
    let payload: Value = serde_json::from_str(&stdin()?)?;
    let kind = payload["hook_event_name"]
        .as_str()
        .context("Missing event")?;
    if ![
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "PermissionRequest",
        "Stop",
        "SubagentStart",
        "SubagentStop",
        "Interrupt",
    ]
    .contains(&kind)
    {
        bail!("Invalid event");
    }
    let session = payload["session_id"].as_str().context("Missing session")?;
    uuid::Uuid::parse_str(session)?;
    let control = root.join("control.json");
    let boundary = read(&control).is_some_and(|v| v["switchRequested"] == true)
        && ["PreToolUse", "PostToolUse", "PostToolUseFailure", "Stop"].contains(&kind);
    let input = if let Some(text) = payload["tool_input"].as_str() {
        serde_json::from_str(text).unwrap_or(Value::Null)
    } else {
        payload["tool_input"].clone()
    };
    let response = &payload["tool_response"];
    let text = response
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| response.to_string())
        .to_lowercase();
    let shell = [
        "Bash",
        "Shell",
        "exec_command",
        "write_stdin",
        "shell_command",
    ]
    .contains(&payload["tool_name"].as_str().unwrap_or(""));
    let background = input["run_in_background"] == true
        || response["backgrounded"] == true
        || !response["backgroundTaskId"].is_null()
        || !response["background_task_id"].is_null()
        || (kind == "PostToolUse"
            && shell
            && (!response["session_id"].is_null()
                || [
                    "process running with session id",
                    "script running with cell id",
                    "running in background",
                    "running in the background",
                ]
                .iter()
                .any(|phrase| text.contains(phrase))));
    let evidence = json!({"kind":kind,"sessionId":session,"transcriptPath":payload["transcript_path"],
        "toolUseId":payload["tool_use_id"],"subagentId":payload["agent_id"],"atMs":now(),"boundary":boundary,"unknownBackground":background});
    event(&root, &evidence)?;
    let expiry = || -> Result<()> {
        let mut value = evidence.clone();
        value["kind"] = json!("BoundaryExpired");
        value["atMs"] = json!(now());
        value["boundary"] = json!(false);
        event(&root, &value)
    };
    if kind == "SessionStart" {
        let started = Instant::now();
        let mut reported = false;
        loop {
            if read(&root.join("start-approved.json")).is_some_and(|v| v["sessionId"] == session) {
                break;
            }
            if !reported && started.elapsed().as_secs() >= 60 {
                expiry()?;
                reported = true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        write(
            &root.join("startup-ready.json"),
            &json!({"sessionId":session}),
        )?;
    }
    let mut resumed = false;
    if boundary {
        let started = Instant::now();
        let mut reported = false;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            if root.join("release.json").exists() {
                break;
            }
            if read(&control).is_some_and(|v| v["switchRequested"] == false) {
                resumed = true;
                break;
            }
            if !reported && started.elapsed().as_secs() >= 30 {
                expiry()?;
                reported = true;
            }
        }
    }
    let output = if !boundary || resumed {
        json!({})
    } else if kind == "PreToolUse" {
        json!({"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Rhyme Loop switching accounts"}})
    } else {
        json!({"continue":false,"stopReason":"Rhyme Loop switching accounts"})
    };
    println!("{output}");
    Ok(())
}
pub fn statusline(dir: &Path) -> Result<()> {
    let input = stdin()?;
    // Collection failures must never prevent forwarding the original statusline.
    let _ = (|| -> Result<()> {
        let payload: Value = serde_json::from_str(&input)?;
        let mut windows = serde_json::Map::new();
        for key in ["five_hour", "seven_day"] {
            let window = &payload["rate_limits"][key];
            let percent = window["used_percentage"]
                .as_f64()
                .or_else(|| window["used_percentage"].as_str()?.parse::<f64>().ok());
            if let Some(percent) = percent.filter(|v| v.is_finite()) {
                windows.insert(
                    key.into(),
                    json!({"used_percentage":percent,"resets_at":window["resets_at"]}),
                );
            }
        }
        let matches = match (
            std::env::var("CLAUDE_CODE_OAUTH_TOKEN"),
            fs::read_to_string(dir.join("oauth-token.txt")),
        ) {
            (Ok(token), Ok(saved)) if !token.is_empty() => token.trim() == saved.trim(),
            _ => true,
        };
        if matches && !windows.is_empty() {
            write(
                &dir.join("winmux-usage.json"),
                &json!({"version":1,"receivedAt":now(),"rate_limits":windows}),
            )?;
        }
        Ok(())
    })();
    if let Some(original) = read(&dir.join("winmux-statusline.json"))
        .and_then(|v| v["original"]["command"].as_str().map(str::to_owned))
        .filter(|v| !v.is_empty())
    {
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", &original])
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut pipe) = child.stdin.take() {
            let _ = pipe.write_all(input.as_bytes());
        }
        child.wait()?;
    }
    Ok(())
}
