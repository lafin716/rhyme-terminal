#[cfg(desktop)]
pub mod commands;
#[cfg(desktop)]
pub mod daemon;
#[cfg(desktop)]
pub mod flow;
#[cfg(desktop)]
pub mod ipc;
#[cfg(desktop)]
pub mod launch;
#[cfg(desktop)]
pub mod loop_routing;
#[cfg(desktop)]
pub mod mobile_pairing;
#[cfg(desktop)]
pub mod pty;
#[cfg(desktop)]
pub mod tray;
#[cfg(desktop)]
pub mod usage;
#[cfg(desktop)]
pub mod usage_bridge;

#[cfg(desktop)]
use std::sync::Arc;
#[cfg(desktop)]
use tauri::{Emitter, Manager, WindowEvent};

#[cfg(desktop)]
use crate::ipc::client::DaemonClient;
#[cfg(desktop)]
use crate::ipc::protocol::Event;

pub const DAEMON_ARG: &str = "--winmux-daemon";

#[cfg(desktop)]
pub fn run() {
    platform::prepare_environment();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    // Whatever started this process — Explorer's menu, a shell, an external
    // tool, a `rhyme://` link — becomes one request. If a window is already
    // open it takes the request and this process is done; `--new-window` is
    // the one way to ask for a second one instead.
    let request = launch::cli::from_env();
    if !request.new_window {
        match rt.block_on(launch::instance::deliver(&request)) {
            Ok(true) => return,
            Ok(false) => (),
            // Delivery that failed halfway leaves the user's click unanswered,
            // so open here rather than exit on a guess that it landed.
            Err(error) => eprintln!("could not reach the running instance: {error:#}"),
        }
    }

    let client = rt
        .block_on(DaemonClient::connect_or_spawn())
        .expect("failed to connect/spawn winmuxd");

    // Hold the runtime alive for background tasks (event forwarder, IPC reader).
    let rt = Arc::new(rt);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(mobile_pairing::MobilePairing::default())
        .manage(client.clone())
        .manage(rt.clone())
        .manage(launch::Pending::default())
        .setup(move |app| {
            // The request this process was started with waits with the ones
            // that arrive later, so the page drains both the same way.
            if !request.is_empty() {
                app.state::<launch::Pending>().push(request.clone());
            }
            // An update installs to a new path; repoint whatever the user
            // already asked Explorer to show before it launches a stale copy.
            #[cfg(windows)]
            if let Err(error) = launch::shell_integration::reconcile(launch::MENU_LABEL) {
                eprintln!("could not refresh the Explorer menu entries: {error:#}");
            }
            let launches = app.handle().clone();
            rt.spawn(async move {
                let handler = move |request: launch::LaunchRequest| {
                    // Foreground first: the click that sent this expects the
                    // window, whether or not the page is listening yet.
                    if let Some(window) = launches.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                    if !request.is_empty() {
                        launches.state::<launch::Pending>().push(request.clone());
                        let _ = launches.emit("launch-request", request);
                    }
                };
                if let Err(error) = launch::instance::serve(handler).await {
                    // Not fatal: this window simply will not receive external
                    // launches, and another instance probably owns them.
                    eprintln!("launch endpoint unavailable: {error:#}");
                }
            });
            let flow = app
                .path()
                .app_local_data_dir()
                .map_err(anyhow::Error::from)
                .and_then(|path| flow::runtime::Engine::open(path.join("flow")));
            app.manage(match flow {
                Ok(engine) => flow::FlowService {
                    engine: Some(engine),
                    error: None,
                },
                Err(error) => flow::FlowService {
                    engine: None,
                    error: Some(format!("{error:#}")),
                },
            });
            // Set the window icon explicitly as well as embedding it through the
            // bundle configuration. This keeps dev builds and the tray in sync.
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(icon)?;
            }

            tray::build_tray(app.handle())?;

            let app_handle = app.handle().clone();
            let mut rx = client.events();
            rt.spawn(async move {
                loop {
                    let ev = match rx.recv().await {
                        Ok(ev) => ev,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        // Drop the overrun batch but keep forwarding later state changes.
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    };
                    match ev {
                        Event::PtyOutput { id, data } => {
                            let _ = app_handle
                                .emit("pty-output", serde_json::json!({ "id": id, "data": data }));
                        }
                        Event::PtyExit { id, status } => {
                            let _ = app_handle.emit(
                                "pty-exit",
                                serde_json::json!({ "id": id, "status": status }),
                            );
                        }
                        Event::SessionAdded { info } => {
                            let _ = app_handle.emit("session-added", info);
                        }
                        Event::SessionRemoved { id } => {
                            let _ =
                                app_handle.emit("session-removed", serde_json::json!({ "id": id }));
                        }
                        Event::SessionRenamed { id, name } => {
                            let _ = app_handle.emit(
                                "session-renamed",
                                serde_json::json!({ "id": id, "name": name }),
                            );
                        }
                        Event::SessionActivity { id, bell } => {
                            let _ = app_handle.emit(
                                "session-activity",
                                serde_json::json!({ "id": id, "bell": bell }),
                            );
                        }
                        Event::SessionAgentChanged { id, agent } => {
                            let _ = app_handle.emit(
                                "session-agent-changed",
                                serde_json::json!({ "id": id, "agent": agent }),
                            );
                        }
                        Event::SessionAgentStatusChanged { id, status } => {
                            let _ = app_handle.emit(
                                "session-agent-status-changed",
                                serde_json::json!({ "id": id, "status": status }),
                            );
                        }
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            launch::take_launch_requests,
            launch::shell_integration_status,
            launch::shell_integration_set,
            loop_routing::loop_request,
            flow::flow_request,
            mobile_pairing::mobile_pairing_interfaces,
            mobile_pairing::mobile_pairing_status,
            mobile_pairing::mobile_pairing_start,
            mobile_pairing::mobile_pairing_stop,
            mobile_pairing::mobile_pairing_invite,
            mobile_pairing::mobile_pairing_approve,
            mobile_pairing::mobile_pairing_revoke,
            commands::create_session,
            commands::list_sessions,
            commands::kill_session,
            commands::write_session,
            commands::resize_session,
            commands::attach_session,
            commands::rename_session,
            commands::resolve_account_dir,
            commands::set_account_token,
            commands::get_account_token,
            usage::get_account_usage,
            commands::read_file_preview,
            commands::resolve_resource_path,
            commands::write_file,
            commands::read_directory,
            commands::rename_path,
            commands::delete_path,
            commands::create_entry,
            commands::list_files,
            commands::browser_navigate,
            commands::browser_back,
            commands::browser_forward,
            commands::browser_reload,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(mobile)]
mod mobile;
#[cfg(desktop)]
pub mod platform;
#[cfg(all(desktop, unix))]
pub mod unix_cli;
#[cfg(mobile)]
pub use mobile::run;
