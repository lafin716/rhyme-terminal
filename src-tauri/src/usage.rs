use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    label: String,
    percent_used: f64,
    resets_at: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    received_at: Option<u64>,
}

fn parse_windows(agent: &str, body: &Value) -> Vec<UsageWindow> {
    let mut windows = Vec::new();
    let fields = if agent == "claude" {
        vec![
            ("five_hour", "5h"),
            ("seven_day", "Weekly"),
            ("seven_day_sonnet", "Sonnet · Weekly"),
            ("seven_day_opus", "Opus · Weekly"),
        ]
    } else {
        vec![
            ("primary_window", "Primary"),
            ("secondary_window", "Secondary"),
        ]
    };
    let source = if agent == "claude" {
        body
    } else {
        &body["rate_limit"]
    };
    for (key, label) in fields {
        let window = &source[key];
        let percent_key = if agent == "claude" {
            "utilization"
        } else {
            "used_percent"
        };
        if let Some(percent) = window[percent_key].as_f64().filter(|p| p.is_finite()) {
            let label = match window["limit_window_seconds"].as_u64() {
                Some(18000) => "5h".to_string(),
                Some(604800) => "Weekly".to_string(),
                Some(seconds) => format!("{}h", seconds as f64 / 3600.0),
                None => label.to_string(),
            };
            let reset = if agent == "claude" {
                window["resets_at"]
                    .as_str()
                    .map(|s| Value::String(s.into()))
            } else {
                window["reset_at"]
                    .as_u64()
                    .and_then(|s| s.checked_mul(1000))
                    .map(Value::from)
            };
            windows.push(UsageWindow {
                label,
                percent_used: percent.clamp(0.0, 100.0),
                resets_at: reset,
                received_at: None,
            });
        }
    }
    windows
}

fn parse_session_windows(sample: &Value, now_seconds: u64) -> Result<Vec<UsageWindow>, String> {
    let received_at = sample["receivedAt"]
        .as_u64()
        .filter(|v| *v > 0)
        .ok_or("Unable to read session usage")?;
    if sample["version"].as_u64() != Some(1) {
        return Err("Unable to read session usage".into());
    }
    let mut windows = Vec::new();
    for (key, label) in [("five_hour", "5h"), ("seven_day", "Weekly")] {
        let window = &sample["rate_limits"][key];
        let percent = window["used_percentage"].as_f64().filter(|v| v.is_finite());
        let reset = window["resets_at"].as_u64();
        if let (Some(percent), Some(reset)) = (percent, reset) {
            if reset <= now_seconds {
                continue;
            }
            if let Some(milliseconds) = reset.checked_mul(1000) {
                windows.push(UsageWindow {
                    label: label.into(),
                    percent_used: percent.clamp(0.0, 100.0),
                    resets_at: Some(Value::from(milliseconds)),
                    received_at: Some(received_at),
                });
            }
        }
    }
    if windows.is_empty() {
        return Err("Waiting for session usage. Start this profile and send a message".into());
    }
    Ok(windows)
}
// Credentials stay in Rust; only normalized quota windows cross the bridge.
// Fixed origins and disabled redirects prevent forwarding credentials elsewhere.
#[tauri::command]
pub async fn get_account_usage(
    app: AppHandle,
    agent: String,
    profile_id: Option<String>,
    session_usage: Option<bool>,
) -> Result<Vec<UsageWindow>, String> {
    if agent != "claude" && agent != "codex" {
        return Err("Unsupported agent".into());
    }
    let use_session = session_usage.unwrap_or(false);
    if use_session && (agent != "claude" || profile_id.is_none()) {
        return Err("Session usage requires a Claude profile".into());
    }
    let dir = if let Some(id) = profile_id {
        let base = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "Profile directory unavailable")?;
        crate::commands::account_dir_path(&base, &agent, &id)?
    } else {
        let variable = if agent == "claude" {
            "CLAUDE_CONFIG_DIR"
        } else {
            "CODEX_HOME"
        };
        std::env::var_os(variable)
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|home| std::path::PathBuf::from(home).join(format!(".{agent}")))
            })
            .ok_or("Profile directory unavailable")?
    };
    if use_session {
        crate::usage_bridge::install(&dir)?;
        let bytes = tokio::fs::read(dir.join("winmux-usage.json"))
            .await
            .map_err(|_| "Waiting for session usage. Start this profile and send a message")?;
        let sample: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Unable to read session usage")?;
        return parse_session_windows(
            &sample,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }
    let file = dir.join(if agent == "claude" {
        ".credentials.json"
    } else {
        "auth.json"
    });
    let body = tokio::fs::read(file)
        .await
        .map_err(|_| "Sign in through the CLI to view usage")?;
    let credentials: Value =
        serde_json::from_slice(&body).map_err(|_| "Unable to read login credentials")?;
    let token = if agent == "claude" {
        credentials["claudeAiOauth"]["accessToken"].as_str()
    } else {
        credentials["tokens"]["access_token"].as_str()
    }
    .filter(|s| !s.is_empty())
    .ok_or("Subscription usage requires an OAuth login")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Usage client unavailable")?;
    let endpoint = if agent == "claude" {
        "https://api.anthropic.com/api/oauth/usage"
    } else {
        "https://chatgpt.com/backend-api/wham/usage"
    };
    let mut request = client.get(endpoint).bearer_auth(token);
    if agent == "claude" {
        request = request.header("anthropic-beta", "oauth-2025-04-20");
    } else if let Some(id) = credentials["tokens"]["account_id"].as_str() {
        request = request.header("ChatGPT-Account-Id", id);
    }
    let response = request
        .send()
        .await
        .map_err(|_| "Unable to connect to usage service")?;
    match response.status().as_u16() {
        200 => (),
        401 => return Err("Login expired. Sign in again through the CLI".into()),
        403 => return Err("This login cannot access subscription usage".into()),
        429 => return Err("Too many requests. Try again later".into()),
        _ => return Err("Usage service is temporarily unavailable".into()),
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| "Unable to read usage response")?;
    let windows = parse_windows(&agent, &body);
    if windows.is_empty() {
        return Err("No subscription usage data available".into());
    }
    Ok(windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_usage_keeps_sample_time_and_drops_expired_windows() {
        let sample = json!({"version":1,"receivedAt":900000,"rate_limits":{"five_hour":{"used_percentage":0,"resets_at":1100},"seven_day":{"used_percentage":72,"resets_at":2000}}});
        let windows = parse_session_windows(&sample, 1000).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].percent_used, 0.0);
        assert_eq!(windows[0].received_at, Some(900000));
        assert_eq!(windows[0].resets_at, Some(json!(1100000)));
        let later = parse_session_windows(&sample, 1100).unwrap();
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].label, "Weekly");
        assert!(parse_session_windows(&sample, 2000).is_err());
        assert!(parse_session_windows(&json!({}), 1000).is_err());
    }
    #[test]
    fn claude_keeps_zero_and_skips_missing_windows() {
        let windows = parse_windows(
            "claude",
            &json!({"five_hour":{"utilization":0,"resets_at":"2026-09-08T00:00:00Z"},"seven_day":null}),
        );
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].percent_used, 0.0);
        assert_eq!(windows[0].resets_at, Some(json!("2026-09-08T00:00:00Z")));
    }

    #[test]
    fn codex_converts_seconds_and_preserves_window_duration() {
        let windows = parse_windows(
            "codex",
            &json!({"rate_limit":{"primary_window":{"used_percent":42,"reset_at":1000,"limit_window_seconds":18000},"secondary_window":{"used_percent":105,"limit_window_seconds":604800}}}),
        );
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "5h");
        assert_eq!(windows[0].resets_at, Some(json!(1000000)));
        assert_eq!(windows[1].label, "Weekly");
        assert_eq!(windows[1].percent_used, 100.0);
        assert!(parse_windows("codex", &json!({})).is_empty());
    }
}
