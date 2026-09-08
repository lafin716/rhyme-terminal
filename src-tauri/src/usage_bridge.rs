use serde_json::{json, Value};
use std::path::Path;

const SCRIPT: &str = include_str!("../scripts/usage-statusline.ps1");

/// Wrap only this profile's statusline; preserve its full configuration and command.
/// Repeated installation does not nest wrappers or overwrite user edits elsewhere.
pub fn install(dir: &Path) -> Result<(), String> {
    let settings_path = dir.join("settings.json");
    let mut settings: Value = match std::fs::read(&settings_path) {
        Ok(bytes) => {
            serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
                .map_err(|_| "Unable to read profile statusline settings")?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(_) => return Err("Unable to read profile statusline settings".into()),
    };
    let object = settings
        .as_object_mut()
        .ok_or("Invalid profile statusline settings")?;
    let script = dir.join("winmux-statusline.ps1");
    let command = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
        script.to_string_lossy().replace('\\', "/")
    );
    write_if_changed(&script, SCRIPT.as_bytes())?;
    let metadata = dir.join("winmux-statusline.json");
    if object
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(Value::as_str)
        == Some(&command)
    {
        // Without metadata we cannot safely recover the previous command.
        if !metadata.exists() {
            return Err("Statusline bridge metadata is missing".into());
        }
        return Ok(());
    }
    let original = object.get("statusLine").cloned().unwrap_or(Value::Null);
    if !original.is_null() && original.get("command").and_then(Value::as_str).is_none() {
        return Err("Unsupported profile statusline configuration".into());
    }
    // Save the exact original configuration before installing the wrapper.
    write_if_changed(
        &metadata,
        &serde_json::to_vec_pretty(&json!({"version": 1, "original": original}))
            .map_err(|_| "Unable to encode statusline settings")?,
    )?;
    let mut wrapped = original.as_object().cloned().unwrap_or_default();
    wrapped.insert("type".into(), json!("command"));
    wrapped.insert("command".into(), json!(command));
    object.insert("statusLine".into(), Value::Object(wrapped));
    write_if_changed(
        &settings_path,
        &serde_json::to_vec_pretty(&settings)
            .map_err(|_| "Unable to encode statusline settings")?,
    )
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = std::fs::write(&temporary, bytes).and_then(|_| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|_| "Unable to install profile usage bridge".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn script_captures_only_usage_and_preserves_original_output() {
        use std::io::Write;
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        let dir = std::env::temp_dir().join(format!("winmux-bridge-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let original = dir.join("original.ps1");
        std::fs::write(&original, "[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)\n$p = [Console]::In.ReadToEnd() | ConvertFrom-Json\nWrite-Output ('original:' + $p.model.display_name)").unwrap();
        let command = format!(
            "powershell.exe -NoProfile -File \"{}\"",
            original.to_string_lossy().replace('\\', "/")
        );
        std::fs::write(
            dir.join("settings.json"),
            json!({"statusLine":{"type":"command","command":command}}).to_string(),
        )
        .unwrap();
        install(&dir).unwrap();
        let run = |payload: Value| {
            let mut process = Command::new("powershell.exe");
            process
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(dir.join("winmux-statusline.ps1"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(0x08000000);
            process.env("CLAUDE_CODE_OAUTH_TOKEN", "active-token");
            let mut child = process.spawn().unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(payload.to_string().as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "original:테스트"
            );
        };
        run(
            json!({"model":{"display_name":"테스트"},"secret":"do-not-cache","rate_limits":{"five_hour":{"used_percentage":0,"resets_at":2000000000},"seven_day":{"used_percentage":42,"resets_at":2000000100}}}),
        );
        let first = std::fs::read(dir.join("winmux-usage.json")).unwrap();
        let sample: Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(sample["rate_limits"]["five_hour"]["used_percentage"], 0.0);
        assert_eq!(sample.as_object().unwrap().len(), 3);
        assert!(sample["receivedAt"].as_u64().unwrap() > 0);
        assert!(sample.get("secret").is_none());
        run(json!({"model":{"display_name":"테스트"}}));
        assert_eq!(first, std::fs::read(dir.join("winmux-usage.json")).unwrap());
        run(
            json!({"model":{"display_name":"테스트"},"rate_limits":{"five_hour":{"used_percentage":81,"resets_at":2000000200}}}),
        );
        let updated: Value =
            serde_json::from_slice(&std::fs::read(dir.join("winmux-usage.json")).unwrap()).unwrap();
        assert_eq!(updated["rate_limits"]["five_hour"]["used_percentage"], 81.0);
        // A session still using an old token must not publish into a replaced account.
        std::fs::write(dir.join("oauth-token.txt"), "replacement-token").unwrap();
        std::fs::remove_file(dir.join("winmux-usage.json")).unwrap();
        run(
            json!({"model":{"display_name":"테스트"},"rate_limits":{"five_hour":{"used_percentage":99,"resets_at":2000000300}}}),
        );
        assert!(!dir.join("winmux-usage.json").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn preserves_original_and_does_not_nest_wrappers() {
        let dir = std::env::temp_dir().join(format!("winmux-statusline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let original =
            json!({"type":"command","command":"powershell.exe -File existing.ps1","padding":2});
        std::fs::write(
            dir.join("settings.json"),
            json!({"statusLine": original,"permissions":{"allow":["Read"]}}).to_string(),
        )
        .unwrap();
        install(&dir).unwrap();
        let first = std::fs::read(dir.join("settings.json")).unwrap();
        install(&dir).unwrap();
        assert_eq!(first, std::fs::read(dir.join("settings.json")).unwrap());
        let metadata: Value =
            serde_json::from_slice(&std::fs::read(dir.join("winmux-statusline.json")).unwrap())
                .unwrap();
        assert_eq!(metadata["original"], original);
        let settings: Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(settings["permissions"]["allow"][0], "Read");
        assert_eq!(settings["statusLine"]["padding"], 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
