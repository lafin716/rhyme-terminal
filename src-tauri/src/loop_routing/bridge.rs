//! A shell-local command gate. Arguments are JSON data, never shell source.
use super::adapter::Launch;
use anyhow::{Context, Result};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

pub fn atomic_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    std::io::Write::write_all(&mut file, &serde_json::to_vec(value)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)?;
    Ok(())
}

pub fn dispatch() -> bool {
    if std::env::args().nth(1).as_deref() != Some("--rhyme-loop-agent") {
        return false;
    }
    #[cfg(windows)]
    {
        // Release app/daemon binaries use the Windows GUI subsystem. Attach
        // only this helper invocation to its calling terminal; never allocate
        // a new console or change the ordinary daemon startup path.
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        unsafe {
            AttachConsole(u32::MAX);
        }
    }
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("Agent launch: {error}");
            std::process::exit(1);
        }
    }
}
fn run() -> Result<i32> {
    let root = PathBuf::from(std::env::var_os("RHYME_LOOP_DIR").context("Missing loop directory")?);
    let args: Vec<String> = std::env::args().skip(2).collect();
    let provider = args.first().context("Missing provider")?;
    let command_args: Vec<String> = if args.get(1).map(String::as_str) == Some("--encoded-args") {
        use base64::Engine;
        serde_json::from_slice(
            &base64::engine::general_purpose::STANDARD
                .decode(args.get(2).context("Missing encoded argv")?)?,
        )?
    } else {
        args[1..].to_vec()
    };
    let id = uuid::Uuid::new_v4().to_string();
    let request = root.join(format!("request-{id}.json"));
    let response = root.join(format!("response-{id}.json"));
    atomic_json(
        &request,
        &json!({"id":id,"pid":std::process::id(),"provider":provider,"args":command_args,"cwd":std::env::current_dir()?}),
    )?;
    let launch: Launch = loop {
        if let Ok(bytes) = fs::read(&response) {
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            fs::remove_file(&response)?;
            if value["cancel"] == true {
                return Ok(0);
            }
            break serde_json::from_value(value)?;
        }
        // The daemon's heartbeat makes a dead controller fail closed.
        let age = fs::metadata(root.join("heartbeat"))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .unwrap_or(Duration::from_secs(60));
        anyhow::ensure!(
            age < Duration::from_secs(15),
            "Loop controller unavailable; command was not started"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut command = Command::new(&launch.shell);
    command
        .args(&launch.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for key in [
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "CODEX_API_KEY",
    ] {
        if launch.managed {
            command.env_remove(key);
        }
    }
    command.envs(&launch.env);
    // Keep the helper alive while the native CLI handles terminal Ctrl+C.
    // Its lifetime is also the controller's confirmation that startup/exit is
    // still in flight. Child processes retain their own default handlers.
    #[cfg(windows)]
    {
        unsafe extern "system" fn keep_parent_alive(event: u32) -> i32 {
            i32::from(event == 0 || event == 1)
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn SetConsoleCtrlHandler(
                handler: Option<unsafe extern "system" fn(u32) -> i32>,
                add: i32,
            ) -> i32;
        }
        unsafe {
            SetConsoleCtrlHandler(Some(keep_parent_alive), 1);
        }
    }
    #[cfg(unix)]
    unsafe {
        extern "C" fn keep_parent_alive(_: i32) {}
        libc::signal(libc::SIGINT, keep_parent_alive as libc::sighandler_t);
    }
    let status = command.status()?;
    atomic_json(
        &root.join(format!("exit-{id}.json")),
        &json!({"code":status.code()}),
    )?;
    Ok(status.code().unwrap_or(1))
}

pub fn shell(
    root: &Path,
) -> Result<(
    String,
    Vec<String>,
    std::collections::HashMap<String, String>,
)> {
    fs::create_dir_all(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    }
    let executable = std::env::current_exe()?;
    let mut env = std::collections::HashMap::new();
    env.insert("RHYME_LOOP_DIR".into(), root.to_string_lossy().into_owned());
    env.insert(
        "RHYME_LOOP_BRIDGE".into(),
        executable.to_string_lossy().into_owned(),
    );
    #[cfg(windows)]
    {
        let script = root.join("shell.ps1");
        fs::write(&script, include_str!("../../scripts/loop-shell.ps1"))?;
        Ok((
            "powershell.exe".into(),
            vec![
                "-NoLogo".into(),
                "-NoExit".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-File".into(),
                script.to_string_lossy().into_owned(),
            ],
            env,
        ))
    }
    #[cfg(unix)]
    {
        let script = root.join("shell.rc");
        fs::write(&script, "[ ! -f ~/.bashrc ] || . ~/.bashrc\ncodex() { \"$RHYME_LOOP_BRIDGE\" --rhyme-loop-agent codex \"$@\"; }\nclaude() { \"$RHYME_LOOP_BRIDGE\" --rhyme-loop-agent claude \"$@\"; }\n")?;
        Ok((
            "/bin/bash".into(),
            vec![
                "--rcfile".into(),
                script.to_string_lossy().into_owned(),
                "-i".into(),
            ],
            env,
        ))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::io::Write;
    struct Terminal(crate::pty::Session);
    impl Drop for Terminal {
        fn drop(&mut self) {
            let snapshot = crate::pty::agent::process_snapshot();
            let tree = crate::pty::agent::ProcessTree::new(&snapshot);
            let children: Vec<_> = tree
                .descendants([self.0.shell_pid])
                .into_iter()
                .cloned()
                .collect();
            super::super::process_controller::signal(&children, true);
            let _ = self.0.killer.kill();
        }
    }
    fn wait(root: &Path, terminal: &Terminal, condition: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(25);
        while !condition() {
            fs::write(root.join("heartbeat"), b"1").unwrap();
            assert!(
                std::time::Instant::now() < deadline,
                "PTY check timed out: {}",
                output(terminal)
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn output(terminal: &Terminal) -> String {
        String::from_utf8_lossy(
            &terminal
                .0
                .scrollback
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
        )
        .into_owned()
    }
    #[test]
    #[ignore = "Requires cargo build --bin winmuxd; exercises real ConPTY with GUI-subsystem helper"]
    fn gui_helper_preserves_interactive_stdio_and_waits_for_child() {
        let root =
            std::env::temp_dir().join(format!("rhyme-conpty-bridge-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let built = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("winmuxd.exe");
        let mut bytes = fs::read(built).unwrap();
        let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        bytes[pe + 24 + 68..pe + 24 + 70].copy_from_slice(&2u16.to_le_bytes());
        let gui = root.join("bridge.exe");
        fs::write(&gui, bytes).unwrap();
        let (shell, mut args, mut env) = super::shell(&root).unwrap();
        args.insert(0, "-NoProfile".into());
        env.insert(
            "RHYME_LOOP_BRIDGE".into(),
            gui.to_string_lossy().into_owned(),
        );
        let (events, _) = tokio::sync::broadcast::channel(16);
        let mut terminal = Terminal(
            crate::pty::spawn_session(
                events,
                "bridge-qa".into(),
                shell,
                args,
                Some(root.to_string_lossy().into_owned()),
                Some(env),
                120,
                30,
            )
            .unwrap(),
        );
        fs::write(root.join("heartbeat"), b"1").unwrap();
        terminal
            .0
            .writer
            .write_all(b"codex resume '11111111-1111-4111-8111-111111111111' 'space argument'\r")
            .unwrap();
        terminal.0.writer.flush().unwrap();
        let request_path = || {
            fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("request-")
                        && p.extension().is_some_and(|e| e == "json")
                })
        };
        wait(&root, &terminal, || request_path().is_some());
        let request: serde_json::Value =
            serde_json::from_slice(&fs::read(request_path().unwrap()).unwrap()).unwrap();
        assert_eq!(
            request["args"],
            json!([
                "resume",
                "11111111-1111-4111-8111-111111111111",
                "space argument"
            ])
        );
        assert!(!output(&terminal).contains("NATIVE_READY"));
        let marker = root.join("input.txt");
        atomic_json(&root.join(format!("response-{}.json",request["id"].as_str().unwrap())),&Launch{managed:true,shell:"powershell.exe".into(),args:vec!["-NoProfile".into(),"-Command".into(),"[Console]::WriteLine('NATIVE_READY'); $value=[Console]::ReadLine(); [IO.File]::WriteAllText($env:RHYME_TEST_MARKER,$value); [Console]::WriteLine('NATIVE_DONE')".into()],env:std::collections::HashMap::from([("RHYME_TEST_MARKER".into(),marker.to_string_lossy().into_owned())])}).unwrap();
        wait(&root, &terminal, || {
            output(&terminal).contains("NATIVE_READY")
        });
        terminal.0.writer.write_all(b"terminal-input\r").unwrap();
        terminal.0.writer.flush().unwrap();
        wait(&root, &terminal, || marker.exists());
        assert_eq!(fs::read_to_string(marker).unwrap(), "terminal-input");
        wait(&root, &terminal, || {
            output(&terminal).contains("NATIVE_DONE")
        });
        terminal
            .0
            .writer
            .write_all(b"Write-Output SHELL_RETURNED\r")
            .unwrap();
        terminal.0.writer.flush().unwrap();
        wait(&root, &terminal, || {
            output(&terminal).matches("SHELL_RETURNED").count() >= 2
        });
    }
}
