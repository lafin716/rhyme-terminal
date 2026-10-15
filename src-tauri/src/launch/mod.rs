//! One internal shape for every "open a terminal here" request that reaches
//! Rhyme Terminal from outside itself.
//!
//! Explorer's context menus, the command line, an external tool that was told
//! to use this app as its terminal, and (later) a `rhyme://` URL all differ
//! only in how they *spell* the request. Each gets a small adapter that parses
//! its own spelling into a [`LaunchRequest`]; nothing downstream of that knows
//! which one it came from, so a new integration is a new adapter and no change
//! to session handling.
//!
//! ```text
//! Explorer / CLI / external tool / rhyme://  →  adapter  →  LaunchRequest
//!                                                               │
//!                                        first instance ────────┴──── running instance
//!                                                │                        │ (named pipe)
//!                                                └──── frontend `launch-request` event
//!                                                              │
//!                                                     useSessions().create()
//! ```
pub mod cli;
pub mod instance;
#[cfg(windows)]
pub mod shell_integration;
pub mod uri;

use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// Which integration spelled this request. Carried for diagnostics and so the
/// frontend can differ where it genuinely should (an Explorer menu opens a tab
/// quietly; a `--command` run is allowed to occupy the shell), never so that
/// session creation has to branch per integration.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LaunchSource {
    #[default]
    Cli,
    Explorer,
    Uri,
}

/// What an outside caller is asking for. Every field is optional: the empty
/// request is "just show me the window", which is what a plain double-click or
/// a second launch with no arguments means.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    /// Working directory for the new session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Files the request named. The session opens on their *directory* — a
    /// terminal cannot "open" a file — but the paths are kept so the frontend
    /// can offer them (insert at the prompt, and whatever comes later).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    /// Command to run in the new session's shell, instead of an idle prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// `user@host` for an SSH session. Kept apart from `command` so the app
    /// decides how to reach a host; the frontend turns it into `ssh <target>`
    /// unless `command` already says something more specific.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<String>,
    /// Open a separate window rather than a tab in the running one. Only ever
    /// set when the caller asked for it (see the UX note in `deliver`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub new_window: bool,
    #[serde(default)]
    pub source: LaunchSource,
}

impl LaunchRequest {
    /// Nothing was asked for beyond "come to the front".
    pub fn is_empty(&self) -> bool {
        self.cwd.is_none() && self.files.is_empty() && self.command.is_none() && self.ssh.is_none()
    }

    /// Resolve what the caller wrote into what a session can be opened with.
    ///
    /// Callers name paths the way their own UI does — Explorer hands the file
    /// that was right-clicked, a shell hands whatever the user typed, and both
    /// may be relative to a working directory this process happens to start in.
    /// Absolute, existing paths come out; a directory named as a file (or a
    /// file named as the cwd) lands in the field it belongs to. Paths that do
    /// not exist are dropped rather than passed on to fail later at spawn.
    pub fn normalized(mut self, base: &Path) -> Self {
        let mut files = Vec::new();
        let mut cwd = self.cwd.take().and_then(|path| absolute(base, &path));
        // A file given as the working directory means the directory it is in;
        // Explorer's `*\shell` verb and a user's tab-completion both do this.
        if let Some(path) = cwd.clone().filter(|path| path.is_file()) {
            cwd = path.parent().map(Path::to_path_buf);
            files.push(path);
        }
        for path in std::mem::take(&mut self.files) {
            let Some(path) = absolute(base, &path) else {
                continue;
            };
            if path.is_dir() {
                cwd.get_or_insert(path);
            } else {
                cwd.get_or_insert_with(|| path.parent().map(Path::to_path_buf).unwrap_or_default());
                files.push(path);
            }
        }
        cwd = cwd.filter(|path| path.is_dir());
        Self {
            cwd: cwd.map(display_path),
            files: files.into_iter().map(display_path).collect(),
            command: self.command.filter(|value| !value.trim().is_empty()),
            ssh: self.ssh.filter(|value| !value.trim().is_empty()),
            ..self
        }
    }
}

/// Absolute, symlink-free-enough form of a path the caller named, or `None`
/// when nothing is there. `canonicalize` is what proves existence; its
/// verbatim (`\\?\`) prefix is stripped again because that form reaches a
/// shell's argv and several of them mishandle it.
fn absolute(base: &Path, path: &str) -> Option<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let joined = base.join(path);
    joined.canonicalize().ok().map(strip_verbatim)
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        // A UNC path keeps a leading `\\`, so it cannot simply lose the prefix.
        Some(rest) if rest.starts_with("UNC\\") => PathBuf::from(format!(r"\\{}", &rest[4..])),
        Some(rest) => PathBuf::from(rest),
        None => path,
    }
}

fn display_path(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

/// Requests that arrived before the frontend could listen for them.
///
/// The first instance parses its own command line before there is a window,
/// let alone a page with an event listener, and Explorer's second click can
/// land during startup too. Both are parked here and drained by the frontend
/// on mount, so no request is lost to a race with page load.
#[derive(Default)]
pub struct Pending(Mutex<Vec<LaunchRequest>>);

impl Pending {
    pub fn push(&self, request: LaunchRequest) {
        let mut queue = self.0.lock();
        // A user clicking a context menu repeatedly wants tabs, not a backlog
        // of hundreds if the page never loads.
        if queue.len() < 32 {
            queue.push(request);
        }
    }
    pub fn take(&self) -> Vec<LaunchRequest> {
        std::mem::take(&mut *self.0.lock())
    }
}

/// Drained by the frontend once it is ready to act on launch requests.
#[tauri::command]
pub fn take_launch_requests(pending: tauri::State<'_, Pending>) -> Vec<LaunchRequest> {
    pending.take()
}

/// What the Explorer menu entries read. Kept with the integration rather than
/// in the frontend's locale files: Explorer stores the string in the registry,
/// so it is written once at install time and cannot follow a language change.
pub const MENU_LABEL: &str = "Rhyme Terminal에서 열기";

#[cfg(windows)]
pub use shell_integration::Status as ShellIntegrationStatus;

/// Non-Windows builds answer the same shape so the Settings page needs no
/// platform branch of its own; everything simply reads as unavailable.
#[cfg(not(windows))]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShellIntegrationStatus {
    pub context_menus: bool,
    pub uri_scheme: bool,
    pub stale: bool,
    pub executable: String,
}

#[tauri::command]
pub fn shell_integration_status() -> Result<ShellIntegrationStatus, String> {
    #[cfg(windows)]
    {
        shell_integration::status().map_err(|error| format!("{error:#}"))
    }
    #[cfg(not(windows))]
    {
        Ok(ShellIntegrationStatus::default())
    }
}

/// Install or remove the Explorer verbs and the `rhyme://` scheme, and report
/// what the registry holds afterwards.
#[tauri::command]
pub fn shell_integration_set(
    #[allow(unused_variables)] context_menus: bool,
    #[allow(unused_variables)] uri_scheme: bool,
) -> Result<ShellIntegrationStatus, String> {
    #[cfg(windows)]
    {
        let apply = || -> anyhow::Result<()> {
            if context_menus {
                shell_integration::install_context_menus(MENU_LABEL)?;
            } else {
                shell_integration::remove_context_menus()?;
            }
            if uri_scheme {
                shell_integration::install_uri_scheme()?;
            } else {
                shell_integration::remove_uri_scheme()?;
            }
            Ok(())
        };
        apply().map_err(|error| format!("{error:#}"))?;
        shell_integration::status().map_err(|error| format!("{error:#}"))
    }
    #[cfg(not(windows))]
    {
        Err("Explorer 통합은 Windows에서만 지원합니다".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rhyme-launch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_file_opens_the_directory_it_lives_in_and_keeps_the_file() {
        let dir = temp();
        let file = dir.join("README.md");
        std::fs::write(&file, b"").unwrap();
        let request = LaunchRequest {
            files: vec![file.to_string_lossy().into_owned()],
            ..Default::default()
        }
        .normalized(&dir);
        assert_eq!(request.cwd.as_deref(), Some(&*dir.to_string_lossy()));
        assert_eq!(request.files.len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Explorer's `*\shell` verb passes the file as `%1`; the folder verbs pass
    /// a directory as `%V`. Both arrive as `--cwd`, so it has to accept either.
    #[test]
    fn a_file_named_as_the_working_directory_resolves_to_its_folder() {
        let dir = temp();
        let file = dir.join("notes.txt");
        std::fs::write(&file, b"").unwrap();
        let request = LaunchRequest {
            cwd: Some(file.to_string_lossy().into_owned()),
            ..Default::default()
        }
        .normalized(&dir);
        assert_eq!(request.cwd.as_deref(), Some(&*dir.to_string_lossy()));
        assert_eq!(request.files, vec![file.to_string_lossy().into_owned()]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Spaces, Hangul and the characters a shell would otherwise eat all reach
    /// us already decoded in argv; the only job here is not to mangle them.
    #[test]
    fn unicode_and_spaced_paths_survive_normalization() {
        let root = temp();
        for name in ["개발 프로젝트", "my files", "a&b (1)"] {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let request = LaunchRequest {
                cwd: Some(name.to_owned()),
                ..Default::default()
            }
            .normalized(&root);
            assert_eq!(request.cwd.as_deref(), Some(&*dir.to_string_lossy()));
            assert!(!request.cwd.unwrap().contains(r"\\?\"));
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_path_that_is_not_there_is_dropped_rather_than_passed_on() {
        let dir = temp();
        let request = LaunchRequest {
            cwd: Some(dir.join("missing").to_string_lossy().into_owned()),
            files: vec![dir.join("gone.txt").to_string_lossy().into_owned()],
            ..Default::default()
        }
        .normalized(&dir);
        assert!(request.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_empty_request_serializes_to_just_its_source() {
        let value = serde_json::to_value(LaunchRequest::default()).unwrap();
        assert_eq!(value, serde_json::json!({ "source": "cli" }));
    }
}
