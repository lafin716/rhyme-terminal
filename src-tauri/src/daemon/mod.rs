use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use crate::ipc::server::{run_server, DaemonState};

pub fn run() -> Result<()> {
    crate::platform::prepare_environment();
    #[cfg(windows)]
    enable_ctrl_c();
    init_logging();
    tracing::info!("winmuxd starting");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let state = Arc::new(DaemonState::new());
        run_server(state).await
    })
}

/// A process started with `CREATE_NEW_PROCESS_GROUP` ignores Ctrl+C, and every
/// shell spawned under it inherits that flag — so `\x03` written to a PTY
/// would never stop a dev server. Clear it so sessions get normal Ctrl+C.
#[cfg(windows)]
fn enable_ctrl_c() {
    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }
    unsafe {
        SetConsoleCtrlHandler(None, 0);
    }
}

fn init_logging() {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file_appender = tracing_appender::rolling::daily(&dir, "winmuxd.log");
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(file_appender)
        .with_ansi(false)
        .try_init();
}

fn log_dir() -> PathBuf {
    #[cfg(windows)]
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("winmux").join("logs");
    }

    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Logs")
            .join("winmux");
    }

    std::env::temp_dir().join("winmux-logs")
}
