//! Foreground mobile gateway using the same approval and session protocol as the GUI.
use super::{interfaces, MobilePairing, NetworkInterface, PairingStatus};
use crate::ipc::{client::DaemonClient, protocol::Method};
use crate::pty::SessionInfo;
use anyhow::{bail, Context, Result};
use clap::{Args, Parser};
use serde_json::{json, Value};
use std::{collections::HashSet, io::BufRead, net::IpAddr, path::PathBuf, time::Duration};
use tokio::sync::mpsc;

#[derive(Args, Clone, Debug)]
pub struct ServerOptions {
    /// Local adapter IP to listen on (default: Tailscale, then LAN, then loopback)
    #[arg(long, alias = "ip", value_name = "IP")]
    pub bind: Option<IpAddr>,
    #[arg(long, default_value_t = 43123, value_parser = clap::value_parser!(u16).range(1..))]
    pub port: u16,
    /// Create a session even if the daemon already has sessions
    #[arg(long)]
    pub new_session: bool,
    /// Shell for a new session; implies --new-session
    #[arg(long)]
    pub shell: Option<String>,
    /// Shell argument; repeatable; implies --new-session
    #[arg(long = "shell-arg", allow_hyphen_values = true)]
    pub shell_args: Vec<String>,
    /// Working directory for a new session; implies --new-session
    #[arg(long, value_name = "DIR")]
    pub cwd: Option<PathBuf>,
}

#[derive(Parser)]
#[command(
    name = "rhyme-terminal",
    about = "Run the mobile terminal server without a GUI",
    version
)]
struct ServerCommand {
    #[arg(long, required = true)]
    server: bool,
    #[command(flatten)]
    options: ServerOptions,
}

/// Dispatch before Tauri initialization so installed app bundles also support headless use.
pub fn dispatch() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new("--server")) {
        return false;
    }
    #[cfg(windows)]
    {
        // Release GUI binaries have no console of their own. Attach only for
        // --server; leave the normal GUI/daemon subsystem behavior unchanged.
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        unsafe {
            AttachConsole(u32::MAX);
        }
    }
    let command = ServerCommand::parse();
    let result = tokio::runtime::Runtime::new()
        .map_err(anyhow::Error::from)
        .and_then(|runtime| runtime.block_on(run(command.options, false)));
    if let Err(error) = result {
        eprintln!("Mobile server: {error:#}");
        std::process::exit(1);
    }
    true
}

fn bind_address(requested: Option<IpAddr>, available: &[NetworkInterface]) -> Result<IpAddr> {
    if let Some(ip) = requested {
        if !super::usable_ip(ip) {
            bail!("Use a specific local adapter IP, not an unspecified or multicast address");
        }
        return Ok(ip);
    }
    available
        .iter()
        .filter_map(|interface| {
            interface
                .ip
                .parse::<IpAddr>()
                .ok()
                .map(|ip| (interface, ip))
        })
        .max_by_key(|(interface, ip)| (interface.is_tailscale, !ip.is_loopback(), ip.is_ipv4()))
        .map(|(_, ip)| ip)
        .context("No local network adapter found; specify --bind with a local IP")
}

fn new_session(options: &ServerOptions, sessions: &[SessionInfo], cwd: String) -> Option<Method> {
    if !sessions.is_empty()
        && !options.new_session
        && options.shell.is_none()
        && options.shell_args.is_empty()
        && options.cwd.is_none()
    {
        return None;
    }
    let mut name = "w1.mobile".to_string();
    let mut suffix = 2;
    while sessions.iter().any(|session| session.name == name) {
        name = format!("w1.mobile-{suffix}");
        suffix += 1;
    }
    Some(Method::CreateSession {
        name: Some(name),
        shell: options.shell.clone(),
        shell_args: options.shell_args.clone(),
        cwd: Some(cwd),
        env: None,
        cols: 120,
        rows: 30,
    })
}

fn emit(json_output: bool, kind: &str, message: &str, data: Value) {
    if json_output {
        println!(
            "{}",
            json!({"type": kind, "message": message, "data": data})
        );
    } else {
        println!("{message}");
    }
}

fn print_status(status: &PairingStatus, json_output: bool) {
    emit(
        json_output,
        "status",
        &format!("Server: {}", status.url.as_deref().unwrap_or("stopped")),
        json!({"running": status.running, "url": status.url, "pending": status.pending, "devices": status.devices}),
    );
    if let Some(invite) = &status.invite {
        emit(
            json_output,
            "invite",
            &format!("Invitation (2 minutes, single use): {}", invite.url),
            json!({"url": invite.url, "expiresAt": invite.expires_at}),
        );
    }
    if !json_output {
        for device in &status.devices {
            println!(
                "Device: {}  {}  connected={}",
                device.device_id, device.name, device.connected
            );
        }
    }
}

const HELP: &str = "Commands: status | invite | approve <request-id> | reject <request-id> | revoke <device-id> | help | quit";

async fn command(server: &MobilePairing, input: &str) -> Result<Option<PairingStatus>> {
    let words: Vec<_> = input.split_whitespace().collect();
    let result = match words.as_slice() {
        ["status"] => Ok(server.status().await),
        ["invite"] => server.invite().await,
        ["approve", id] => server.approve(id, true).await,
        ["reject", id] => server.approve(id, false).await,
        ["revoke", id] => server.revoke(id).await,
        ["quit"] => return Ok(None),
        _ => bail!("{HELP}"),
    };
    Ok(Some(result.map_err(anyhow::Error::msg)?))
}

fn stdin_commands() -> mpsc::Receiver<std::io::Result<String>> {
    let (sender, receiver) = mpsc::channel(16);
    // A dedicated thread avoids Tokio's blocking stdin reader keeping runtime
    // shutdown stuck until another Enter after Ctrl+C.
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if sender.blocking_send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

async fn control_loop(
    server: &MobilePairing,
    client: &DaemonClient,
    mut commands: mpsc::Receiver<std::io::Result<String>>,
    json_output: bool,
) -> Result<()> {
    let mut reported = HashSet::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            biased;
            result = &mut shutdown => { result?; return Ok(()); }
            _ = client.disconnected() => bail!("Terminal daemon disconnected; restart the mobile server"),
            line = commands.recv() => {
                let Some(line) = line else { return Ok(()); };
                let line = line?;
                if line.trim().is_empty() { continue; }
                if line.trim() == "help" {
                    emit(json_output, "help", HELP, Value::Null);
                    continue;
                }
                match command(server, &line).await {
                    Ok(Some(status)) => print_status(&status, json_output),
                    Ok(None) => return Ok(()),
                    Err(error) => emit(json_output, "error", &error.to_string(), Value::Null),
                }
            }
            _ = ticker.tick() => {
                let status = server.status().await;
                if !status.running { bail!("Mobile server stopped unexpectedly"); }
                reported.retain(|id| status.pending.iter().any(|pending| &pending.request_id == id));
                for pending in status.pending {
                    if reported.insert(pending.request_id.clone()) {
                        emit(json_output, "pending", &format!("Pairing request from {} — compare code {} on your phone, then type: approve {} (or reject {})",
                            pending.name, pending.verification, pending.request_id, pending.request_id),
                            serde_json::to_value(pending)?);
                    }
                }
            }
        }
    }
}

pub async fn run(options: ServerOptions, json_output: bool) -> Result<()> {
    crate::platform::prepare_environment();
    let ip = bind_address(options.bind, &interfaces().map_err(anyhow::Error::msg)?)?;
    let cwd = options.cwd.clone().unwrap_or(std::env::current_dir()?);
    let cwd = cwd
        .canonicalize()
        .context("Session working directory does not exist")?;
    if !cwd.is_dir() {
        bail!("Session working directory must be a directory");
    }
    let cwd = crate::launch::strip_verbatim(cwd)
        .to_string_lossy()
        .into_owned();
    let server = MobilePairing::default();
    let status = server
        .start(ip.to_string(), options.port)
        .await
        .map_err(anyhow::Error::msg)?;
    let client = DaemonClient::connect_or_spawn().await?;
    let sessions: Vec<SessionInfo> = client.request(Method::ListSessions).await?;
    if let Some(method) = new_session(&options, &sessions, cwd) {
        let session: SessionInfo = client.request(method).await?;
        emit(
            json_output,
            "session",
            &format!(
                "Created terminal session: {} ({})",
                session.name, session.id
            ),
            serde_json::to_value(session)?,
        );
    } else {
        emit(
            json_output,
            "sessions",
            &format!("Sharing {} existing terminal session(s)", sessions.len()),
            serde_json::to_value(sessions)?,
        );
    }
    print_status(&status, json_output);
    emit(json_output, "help", HELP, Value::Null);
    let result = control_loop(&server, &client, stdin_commands(), json_output).await;
    server.stop().await;
    emit(
        json_output,
        "stopped",
        "Mobile server stopped. Terminal sessions remain in the daemon.",
        Value::Null,
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::AgentKind;

    fn options(args: &[&str]) -> ServerOptions {
        ServerCommand::try_parse_from(
            ["rhyme-terminal", "--server"]
                .into_iter()
                .chain(args.iter().copied()),
        )
        .unwrap()
        .options
    }

    fn interface(ip: &str, is_tailscale: bool) -> NetworkInterface {
        NetworkInterface {
            name: "test".into(),
            ip: ip.into(),
            is_tailscale,
        }
    }

    fn session(name: &str) -> SessionInfo {
        SessionInfo {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            shell: "test-shell".into(),
            cwd: None,
            cols: 120,
            rows: 30,
            agent: AgentKind::Terminal,
        }
    }

    #[test]
    fn server_flags_validate_addresses_ports_and_preserve_shell_arguments() {
        let parsed = options(&[
            "--bind",
            "127.0.0.1",
            "--port",
            "43124",
            "--shell",
            "test-shell",
            "--shell-arg=-i",
            "--cwd",
            ".",
        ]);
        assert_eq!(parsed.bind, Some("127.0.0.1".parse().unwrap()));
        assert_eq!(parsed.port, 43124);
        assert_eq!(parsed.shell_args, ["-i"]);
        for invalid in [
            vec!["--port", "0"],
            vec!["--port", "65536"],
            vec!["--bind", "example.com"],
            vec!["--unknown"],
        ] {
            assert!(ServerCommand::try_parse_from(
                ["rhyme-terminal", "--server"].into_iter().chain(invalid)
            )
            .is_err());
        }
        assert_eq!(options(&[]).port, 43123);
    }

    #[test]
    fn default_binding_prefers_tailscale_then_lan_over_loopback() {
        let adapters = [
            interface("127.0.0.1", false),
            interface("192.168.1.20", false),
            interface("100.64.0.2", true),
        ];
        assert_eq!(
            bind_address(None, &adapters).unwrap().to_string(),
            "100.64.0.2"
        );
        assert_eq!(
            bind_address(None, &adapters[..2]).unwrap().to_string(),
            "192.168.1.20"
        );
        assert_eq!(
            bind_address(Some("127.0.0.1".parse().unwrap()), &adapters)
                .unwrap()
                .to_string(),
            "127.0.0.1"
        );
        assert!(bind_address(Some("0.0.0.0".parse().unwrap()), &adapters).is_err());
        assert!(bind_address(Some("::".parse().unwrap()), &adapters).is_err());
        assert!(bind_address(None, &[]).is_err());
    }

    #[test]
    fn initial_session_is_created_when_empty_and_existing_sessions_are_reused() {
        let defaults = options(&[]);
        let Some(Method::CreateSession {
            name,
            cwd,
            cols,
            rows,
            ..
        }) = new_session(&defaults, &[], "/work".into())
        else {
            panic!("missing initial shell");
        };
        assert_eq!(name.as_deref(), Some("w1.mobile"));
        assert_eq!(cwd.as_deref(), Some("/work"));
        assert_eq!((cols, rows), (120, 30));
        assert!(new_session(&defaults, &[session("w1.dev")], "/work".into()).is_none());
    }

    #[test]
    fn explicit_shell_or_directory_creates_another_uniquely_named_session() {
        let existing = [session("w1.mobile"), session("w1.mobile-2")];
        for args in [
            vec!["--new-session"],
            vec!["--shell", "test-shell"],
            vec!["--shell-arg=-i"],
            vec!["--cwd", "."],
        ] {
            let Some(Method::CreateSession { name, .. }) =
                new_session(&options(&args), &existing, "/work".into())
            else {
                panic!("explicit options must create a session");
            };
            assert_eq!(name.as_deref(), Some("w1.mobile-3"));
        }
    }

    async fn running() -> MobilePairing {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = reserved.local_addr().unwrap().port();
        drop(reserved);
        let server = MobilePairing::default();
        server.start("127.0.0.1".into(), port).await.unwrap();
        server
    }

    #[tokio::test]
    async fn terminal_commands_keep_explicit_approval_and_revocation() {
        let server = running().await;
        let token = server.pairings.lock().invite.as_ref().unwrap().0.clone();
        let pending = server
            .pairings
            .lock()
            .pair(&token, "phone", super::super::now_ms())
            .unwrap();
        assert!(server.pairings.lock().devices.is_empty());
        assert!(
            command(&server, &format!("approve {} extra", pending.request_id))
                .await
                .is_err()
        );
        assert!(server.pairings.lock().devices.is_empty());
        let approved = command(&server, &format!("approve {}", pending.request_id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(approved.devices.len(), 1);
        assert!(approved.pending.is_empty());
        let revoked = {
            let pairings = server.pairings.lock();
            pairings.devices.values().next().unwrap().cancel.subscribe()
        };
        let status = command(
            &server,
            &format!("revoke {}", approved.devices[0].device_id),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(status.devices.is_empty());
        assert!(*revoked.borrow());
        assert!(command(&server, "quit").await.unwrap().is_none());
        server.stop().await;
    }

    #[tokio::test]
    async fn terminal_invites_expire_previous_link_and_stop_revokes_credentials() {
        let server = running().await;
        let old = server.pairings.lock().invite.as_ref().unwrap().0.clone();
        command(&server, "invite").await.unwrap();
        assert!(server
            .pairings
            .lock()
            .pair(&old, "old", super::super::now_ms())
            .is_err());
        let current = server.pairings.lock().invite.as_ref().unwrap().0.clone();
        let pending = server
            .pairings
            .lock()
            .pair(&current, "phone", super::super::now_ms())
            .unwrap();
        let token = server
            .pairings
            .lock()
            .approve(&pending.request_id, true, super::super::now_ms())
            .unwrap()
            .unwrap();
        let cancelled = server.pairings.lock().devices[&token].cancel.subscribe();
        let stopped = server.stop().await;
        assert!(!stopped.running);
        assert!(stopped.invite.is_none());
        assert!(stopped.devices.is_empty());
        assert!(stopped.pending.is_empty());
        assert!(*cancelled.borrow());
        assert!(command(&server, "invite").await.is_err());
    }
}
