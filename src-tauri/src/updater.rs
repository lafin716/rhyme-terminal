//! In-app updates from the signed `latest.json` the release workflow publishes.
//!
//! The page drives the flow: `update_check` asks the release endpoint whether a
//! newer signed build exists and remembers it, `update_install` downloads that
//! build (reporting progress as `update-progress` events), stops the daemon and
//! hands over to the platform installer. The daemon runs from the same
//! executable as the window, so it has to go first: Windows cannot replace a
//! running .exe, and on macOS a surviving old daemon would serve the new UI.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::ipc::client::DaemonClient;
use crate::ipc::protocol::Method;

/// The update found by the last successful check, waiting for the user.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

/// Returns the newer release, or `None` when this build is already current.
#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|update| UpdateInfo {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        notes: update.body.clone().filter(|body| !body.trim().is_empty()),
        date: update.date.map(|date| date.to_string()),
    });
    *pending.0.lock() = update;
    Ok(info)
}

/// Downloads and installs the update found by the last check, then restarts.
/// On Windows the installer takes over and this process exits inside
/// `install`; elsewhere the app relaunches itself once the bundle is swapped.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
    client: State<'_, Arc<DaemonClient>>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .clone()
        .ok_or_else(|| "No update is pending; check for updates first".to_string())?;

    let progress = app.clone();
    let mut downloaded = 0u64;
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit("update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    // Only stop the daemon once the signed package is safely on disk, so a
    // failed download never costs the user their sessions.
    if let Err(error) = client.request_raw(Method::KillServer).await {
        tracing::warn!("could not stop the daemon before updating: {error:#}");
    }
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}
