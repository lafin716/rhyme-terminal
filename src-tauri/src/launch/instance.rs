//! Single instance: hand a [`LaunchRequest`] to the window that is already
//! open, or become that window.
//!
//! This reuses the daemon's own transport — a per-user named pipe on Windows,
//! a 0700 unix socket elsewhere — with the same length-prefixed frames, rather
//! than adding a second IPC mechanism to the app. The endpoint is distinct from
//! the daemon's: the daemon owns PTYs and has no window to raise, and it must
//! keep running when the GUI exits.
//!
//! The listener is the lock. `Listener::bind` opens the pipe with
//! `first_pipe_instance`, so exactly one process can hold it and the race
//! between two launches started together resolves without a second primitive:
//! whoever loses the bind connects to the winner instead.

use std::time::Duration;

use anyhow::{anyhow, Result};

use super::LaunchRequest;
use crate::ipc::{read_frame, transport, write_frame};

/// Endpoint the running GUI listens on. Per user, and distinct from the
/// daemon's `winmux-{user}` pipe.
pub fn endpoint() -> String {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".into());
        format!(r"\\.\pipe\rhyme-terminal-launch-{user}")
    }
    #[cfg(unix)]
    {
        let user = unsafe { libc::geteuid() };
        std::path::PathBuf::from("/tmp")
            .join(format!("rhyme-terminal-{user}"))
            .join("launch.sock")
            .to_string_lossy()
            .into_owned()
    }
}

/// Hand this request to an instance that is already running.
///
/// `Ok(true)` means it was accepted and this process has nothing left to do.
/// `Ok(false)` means nobody is listening, so the caller is the first instance.
/// The acknowledgement is waited for: exiting before the running window has
/// taken the request would lose it if that window is mid-startup.
pub async fn deliver(request: &LaunchRequest) -> Result<bool> {
    let endpoint = endpoint();
    let Ok(mut client) = transport::connect(&endpoint).await else {
        return Ok(false);
    };
    let payload = serde_json::to_vec(request)?;
    write_frame(&mut client, &payload).await?;
    // Bounded: a wedged instance must not hang a context-menu click forever.
    match tokio::time::timeout(Duration::from_secs(5), read_frame(&mut client)).await {
        Ok(Ok(ack)) if ack == b"ok" => Ok(true),
        Ok(Ok(other)) => Err(anyhow!(
            "running instance refused the request: {}",
            String::from_utf8_lossy(&other)
        )),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(anyhow!("running instance did not answer in time")),
    }
}

/// Own the endpoint and pass everything that arrives to `handle`.
///
/// Returns the bound listener's first error. A failure to bind is not fatal to
/// the app — it only means external launches cannot reach this window — so the
/// caller logs it and carries on.
pub async fn serve<F>(handle: F) -> Result<()>
where
    F: Fn(LaunchRequest) + Send + 'static,
{
    let mut listener = transport::Listener::bind(&endpoint()).await?;
    loop {
        let mut stream = listener.accept().await?;
        let frame = match read_frame(&mut stream).await {
            Ok(frame) => frame,
            Err(error) => {
                tracing::warn!("launch request not readable: {error}");
                continue;
            }
        };
        match serde_json::from_slice::<LaunchRequest>(&frame) {
            Ok(request) => {
                handle(request);
                let _ = write_frame(&mut stream, b"ok").await;
            }
            Err(error) => {
                tracing::warn!("launch request not understood: {error}");
                let _ = write_frame(&mut stream, error.to_string().as_bytes()).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// The endpoint is per user and never the daemon's, or a GUI launch would
    /// be answered by a process with no window to raise.
    #[test]
    fn the_launch_endpoint_is_not_the_daemon_endpoint() {
        assert_ne!(endpoint(), crate::ipc::pipe_name());
    }

    #[tokio::test]
    async fn a_request_reaches_the_listening_instance_and_is_acknowledged() {
        // The real endpoint is per user; tests bind their own so a developer's
        // running app is never spoken to.
        let received: Arc<Mutex<Vec<LaunchRequest>>> = Arc::default();
        let seen = received.clone();
        let name = test_endpoint();
        let mut listener = transport::Listener::bind(&name).await.unwrap();
        let server = tokio::spawn(async move {
            let mut stream = listener.accept().await.unwrap();
            let frame = read_frame(&mut stream).await.unwrap();
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&frame).unwrap());
            write_frame(&mut stream, b"ok").await.unwrap();
        });
        let request = LaunchRequest {
            cwd: Some(r"D:\개발 프로젝트".into()),
            command: Some("git status".into()),
            ..Default::default()
        };
        let mut client = transport::connect(&name).await.unwrap();
        write_frame(&mut client, &serde_json::to_vec(&request).unwrap())
            .await
            .unwrap();
        assert_eq!(read_frame(&mut client).await.unwrap(), b"ok");
        server.await.unwrap();
        assert_eq!(received.lock().unwrap().as_slice(), &[request]);
    }

    /// Nobody home: the caller has to learn that it is the first instance
    /// rather than fail.
    #[tokio::test]
    async fn delivery_to_nothing_reports_that_no_instance_is_running() {
        let name = test_endpoint();
        assert!(transport::connect(&name).await.is_err());
    }

    /// Two launches at once: the endpoint itself decides which one is the app.
    #[tokio::test]
    async fn only_one_process_can_own_the_endpoint() {
        let name = test_endpoint();
        let _first = transport::Listener::bind(&name).await.unwrap();
        assert!(transport::Listener::bind(&name).await.is_err());
    }

    fn test_endpoint() -> String {
        let id = uuid::Uuid::new_v4();
        #[cfg(windows)]
        {
            format!(r"\\.\pipe\rhyme-terminal-launch-test-{id}")
        }
        #[cfg(unix)]
        {
            std::path::PathBuf::from("/tmp")
                .join(format!("rhyme-launch-test-{id}"))
                .join("launch.sock")
                .to_string_lossy()
                .into_owned()
        }
    }
}
