//! Managed subprocess boundary. This never attaches to a user's manual terminal.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

const OUTPUT_LIMIT: usize = 1024 * 1024;

async fn drain(mut pipe: impl AsyncRead + Unpin) -> Result<(String, bool)> {
    let mut retained = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        let count = pipe.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        let keep = count.min(OUTPUT_LIMIT.saturating_sub(retained.len()));
        retained.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    Ok((String::from_utf8_lossy(&retained).into_owned(), truncated))
}

pub async fn execute(
    program: &str,
    args: &[String],
    cwd: &Path,
    stdin: Option<String>,
    timeout_secs: u64,
    cancel: Arc<AtomicBool>,
) -> Result<Value> {
    execute_with_home(program, args, cwd, stdin, timeout_secs, cancel, None).await
}

async fn execute_with_home(
    program: &str,
    args: &[String],
    cwd: &Path,
    stdin: Option<String>,
    timeout_secs: u64,
    cancel: Arc<AtomicBool>,
    profile_home: Option<&Path>,
) -> Result<Value> {
    if cancel.load(Ordering::Acquire) {
        bail!("Execution cancelled");
    }
    if timeout_secs == 0 {
        bail!("Execution timeout must be positive");
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(home) = profile_home {
        command.env("CODEX_HOME", home);
    }
    // Suspended creation closes the race where the child could spawn outside its job.
    #[cfg(windows)]
    command.creation_flags(0x08000000 | 0x00000004);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .with_context(|| format!("Could not start {program}"))?;
    #[cfg(windows)]
    let job = match windows_job::Job::attach_and_resume(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill().await;
            return Err(error);
        }
    };
    #[cfg(unix)]
    let group = UnixGroup(child.id().context("Missing process ID")?);
    let out = tokio::spawn(drain(child.stdout.take().context("Missing stdout")?));
    let err = tokio::spawn(drain(child.stderr.take().context("Missing stderr")?));
    let writer = if let Some(input) = stdin {
        let mut pipe = child.stdin.take().context("Missing stdin")?;
        Some(tokio::spawn(async move {
            pipe.write_all(input.as_bytes()).await?;
            pipe.shutdown().await
        }))
    } else {
        None
    };
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_secs));
    tokio::pin!(deadline);
    let mut poll = tokio::time::interval(Duration::from_millis(50));
    let status = loop {
        tokio::select! {
            result = child.wait() => break result.context("Failed to wait for process"),
            _ = &mut deadline => break Err(anyhow::anyhow!("Execution timed out after {timeout_secs}s")),
            _ = poll.tick() => if cancel.load(Ordering::Acquire) { break Err(anyhow::anyhow!("Execution cancelled")); }
        }
    };
    // Closing the job also kills descendants that inherited output handles.
    #[cfg(windows)]
    drop(job);
    #[cfg(unix)]
    drop(group);
    if status.is_err() {
        let _ = child.kill().await;
    }
    if let Some(writer) = writer {
        writer.abort();
    }
    let stdout = out.await.context("stdout reader panicked")??;
    let stderr = err.await.context("stderr reader panicked")??;
    let status = status?;
    Ok(
        json!({"exit_code": status.code().unwrap_or(-1), "stdout": stdout.0, "stderr": stderr.0, "stdout_truncated": stdout.1, "stderr_truncated": stderr.1}),
    )
}

/// Resolve native binaries or the official Node entrypoint; never interpolate a cmd shim.
fn codex_command() -> Result<(String, Vec<String>)> {
    let paths: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    for path in &paths {
        let executable = path.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        if executable.is_file() {
            return Ok((executable.to_string_lossy().into_owned(), vec![]));
        }
    }
    for path in &paths {
        let script = path.join("node_modules/@openai/codex/bin/codex.js");
        if script.is_file() {
            let node = paths
                .iter()
                .map(|p| p.join(if cfg!(windows) { "node.exe" } else { "node" }))
                .find(|p| p.is_file())
                .context("Codex requires Node.js; setup required")?;
            return Ok((
                node.to_string_lossy().into_owned(),
                vec![script.to_string_lossy().into_owned()],
            ));
        }
    }
    bail!("Codex CLI not found; setup required")
}

pub async fn codex(
    prompt: &str,
    cwd: &Path,
    profile_home: Option<&Path>,
    writable: bool,
    timeout_secs: u64,
    cancel: Arc<AtomicBool>,
) -> Result<Value> {
    let (program, mut args) = codex_command()?;
    args.extend(
        [
            "exec",
            "--json",
            "--ignore-user-config",
            "-s",
            if writable {
                "workspace-write"
            } else {
                "read-only"
            },
            "--color",
            "never",
            "-",
        ]
        .map(str::to_owned),
    );
    let result = execute_with_home(
        &program,
        &args,
        cwd,
        Some(prompt.to_owned()),
        timeout_secs,
        cancel,
        profile_home,
    )
    .await?;
    parse_codex_result(&result)
}

fn parse_codex_result(result: &Value) -> Result<Value> {
    if result["exit_code"].as_i64() != Some(0) {
        bail!(
            "Codex failed: {}",
            result["stderr"].as_str().unwrap_or("unknown error")
        );
    }
    if result["stdout_truncated"] == true {
        bail!("Codex event output exceeded the 1 MiB limit; completion cannot be verified");
    }
    let mut events = Vec::new();
    let mut thread_id = None;
    let mut messages = Vec::new();
    let mut completed = false;
    for line in result["stdout"]
        .as_str()
        .unwrap_or("")
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let event: Value = serde_json::from_str(line).context("Invalid Codex JSON event")?;
        match event["type"].as_str().unwrap_or("") {
            "error" | "turn.failed" => bail!("Codex reported an error: {event}"),
            "thread.started" => thread_id = event["thread_id"].as_str().map(str::to_owned),
            "turn.completed" => completed = true,
            "item.completed" if event["item"]["type"] == "agent_message" => {
                if let Some(text) = event["item"]["text"].as_str() {
                    messages.push(text.to_owned());
                }
            }
            _ => {}
        }
        events.push(event);
    }
    if !completed {
        bail!("Codex ended without a completed turn");
    }
    Ok(json!({"text": messages.join("\n\n"), "thread_id": thread_id, "events": events}))
}

#[cfg(windows)]
mod windows_job {
    use super::*;
    use std::ffi::c_void;
    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct BasicLimits {
        process_time: i64,
        job_time: i64,
        flags: u32,
        min_working_set: usize,
        max_working_set: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimits {
        basic: BasicLimits,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            class: i32,
            info: *const c_void,
            length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn NtResumeProcess(process: Handle) -> i32;
    }
    // Integer handle ownership is Send: no alias performs operations on this owned job.
    pub struct Job(usize);
    impl Job {
        pub fn attach_and_resume(child: &tokio::process::Child) -> Result<Self> {
            let process = child.raw_handle().context("Missing process handle")?;
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(std::io::Error::last_os_error())
                        .context("Create managed process job");
                }
                let job = Self(handle as usize);
                let mut limits = ExtendedLimits::default();
                limits.basic.flags = 0x00002000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                if SetInformationJobObject(
                    handle,
                    9,
                    &limits as *const _ as _,
                    std::mem::size_of::<ExtendedLimits>() as u32,
                ) == 0
                {
                    return Err(std::io::Error::last_os_error())
                        .context("Set managed process job limits");
                }
                if AssignProcessToJobObject(handle, process) == 0 {
                    return Err(std::io::Error::last_os_error())
                        .context("Assign managed process job");
                }
                if NtResumeProcess(process) < 0 {
                    bail!("Resume managed process failed");
                }
                Ok(job)
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0 as Handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_rejects_failed_or_incomplete_turns() {
        for event in [
            "{\"type\":\"turn.failed\"}",
            "{\"type\":\"error\"}",
            "{\"type\":\"thread.started\"}",
        ] {
            assert!(parse_codex_result(&json!({"exit_code":0,"stdout":event})).is_err());
        }
        assert!(
            parse_codex_result(&json!({"exit_code":1,"stdout":"","stderr":"offline"})).is_err()
        );
    }
    #[test]
    fn codex_collects_thread_and_final_text() {
        let result = parse_codex_result(&json!({"exit_code":0,"stdout":"{\"type\":\"thread.started\",\"thread_id\":\"abc\"}\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"done\"}}\n{\"type\":\"turn.completed\"}"})).unwrap();
        assert_eq!(result["thread_id"], "abc");
        assert_eq!(result["text"], "done");
    }
    #[cfg(windows)]
    async fn powershell(script: &str, seconds: u64, cancel: Arc<AtomicBool>) -> Result<Value> {
        execute(
            "powershell.exe",
            &[
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                script.into(),
            ],
            &std::env::temp_dir(),
            None,
            seconds,
            cancel,
        )
        .await
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn command_reports_nonzero_exit_and_output() {
        let result = powershell(
            "[Console]::Out.Write('hello'); [Console]::Error.Write('failure'); exit 7",
            15,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        assert_eq!(result["exit_code"], 7);
        assert_eq!(result["stdout"], "hello");
        assert_eq!(result["stderr"], "failure");
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn command_timeout_and_cancellation() {
        assert!(
            powershell("Start-Sleep 30", 1, Arc::new(AtomicBool::new(false)))
                .await
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            trigger.store(true, Ordering::Release);
        });
        assert!(powershell("Start-Sleep 30", 15, cancel)
            .await
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn completed_command_does_not_leave_descendants() {
        let result = powershell(
            "$childProcess = Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command Start-Sleep 30' -WindowStyle Hidden -PassThru; [Console]::Write($childProcess.Id)",
            15,
            Arc::new(AtomicBool::new(false)),
        ).await.unwrap();
        let process_id: u32 = result["stdout"].as_str().unwrap().trim().parse().unwrap();
        // The only interpolated value is a parsed integer, never user text.
        let check = powershell(
            &format!(
                "if (Get-Process -Id {process_id} -ErrorAction SilentlyContinue) {{ exit 9 }}"
            ),
            15,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        assert_eq!(check["exit_code"], 0);
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn command_accepts_unicode_space_directory_and_stdin() {
        let directory =
            std::env::temp_dir().join(format!("rhyme flow 한글 {}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let result = execute("powershell.exe", &["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), "[Console]::InputEncoding=[Text.Encoding]::UTF8; [Console]::OutputEncoding=[Text.Encoding]::UTF8; [Console]::Write([Console]::In.ReadToEnd())".into()], &directory, Some("한글 input".into()), 15, Arc::new(AtomicBool::new(false))).await;
        std::fs::remove_dir(&directory).unwrap();
        assert_eq!(result.unwrap()["stdout"], "한글 input");
    }
    #[tokio::test]
    async fn output_is_capped_but_drained() {
        let input = vec![b'a'; OUTPUT_LIMIT + 100];
        let (text, truncated) = drain(input.as_slice()).await.unwrap();
        assert_eq!(text.len(), OUTPUT_LIMIT);
        assert!(truncated);
    }
}

#[cfg(unix)]
struct UnixGroup(u32);
#[cfg(unix)]
impl Drop for UnixGroup {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.0 as i32), libc::SIGKILL);
        }
    }
}
