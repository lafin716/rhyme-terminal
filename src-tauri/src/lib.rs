pub mod commands;
pub mod daemon;
pub mod ipc;
pub mod pty;
pub mod tray;

use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};

use crate::ipc::client::DaemonClient;
use crate::ipc::protocol::Event;

pub const DAEMON_ARG: &str = "--winmux-daemon";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let client = rt
        .block_on(DaemonClient::connect_or_spawn())
        .expect("failed to connect/spawn winmuxd");

    // Hold the runtime alive for background tasks (event forwarder, IPC reader).
    let rt = Arc::new(rt);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(client.clone())
        .manage(rt.clone())
        .setup(move |app| {
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
            commands::create_session,
            commands::list_sessions,
            commands::kill_session,
            commands::write_session,
            commands::resize_session,
            commands::attach_session,
            commands::rename_session,
            commands::read_file_preview,
            commands::write_file,
            commands::read_directory,
            commands::list_files,
            commands::browser_navigate,
            commands::browser_back,
            commands::browser_forward,
            commands::browser_reload,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
