//! Explorer context menus and the `rhyme://` scheme, as HKCU registry entries.
//!
//! Everything is written under `HKEY_CURRENT_USER\Software\Classes`, which is
//! the per-user half of `HKEY_CLASSES_ROOT`: no elevation to install, no
//! machine-wide state to clean up, and removal with the user's profile. The
//! app owns these entries rather than an installer, because the app is the
//! only thing that knows where it currently lives — an update that moves the
//! executable re-registers on its next start (see [`reconcile`]).
//!
//! Explorer runs the stored command line directly, so each verb is a complete
//! quoted command; `%V` is the directory a folder verb was invoked on (it is
//! the one that works for both `Directory` and `Directory\Background`) and
//! `%1` the file a file verb was invoked on.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use windows_registry::CURRENT_USER;

/// Key name for every verb this app installs. Distinct enough not to collide
/// with another program's, stable so an upgrade rewrites rather than doubles.
const VERB: &str = "RhymeTerminal";
const CLASSES: &str = r"Software\Classes";

/// One Explorer verb: where it is registered and what it passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verb {
    /// Class path under `Software\Classes`, e.g. `Directory\shell`.
    pub class: &'static str,
    /// The launch flag and Explorer substitution for this class.
    pub argument: &'static str,
}

/// Right-clicking a folder, the empty space inside a folder, and a file.
/// All three open a terminal *at a directory* — the file verb on the file's
/// own folder — which is the only thing a terminal can do with a selection.
pub const VERBS: [Verb; 3] = [
    Verb {
        class: r"Directory\shell",
        argument: r#"--cwd "%V""#,
    },
    Verb {
        class: r"Directory\Background\shell",
        argument: r#"--cwd "%V""#,
    },
    Verb {
        class: r"*\shell",
        argument: r#"--file "%1""#,
    },
];

/// What is registered right now, for the Settings toggle to render.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Every Explorer verb is present and points at this executable.
    pub context_menus: bool,
    pub uri_scheme: bool,
    /// A verb is registered but names a different executable — an older install.
    pub stale: bool,
    pub executable: String,
}

/// The command Explorer stores for a verb. Pure, so the quoting is testable
/// without touching the registry.
pub fn command_line(executable: &str, argument: &str) -> String {
    format!(r#""{executable}" {argument}"#)
}

fn executable() -> Result<String> {
    Ok(std::env::current_exe()?
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_owned())
}

/// Install (or refresh) the Explorer verbs. `label` is what the menu reads.
pub fn install_context_menus(label: &str) -> Result<()> {
    let executable = executable()?;
    for verb in VERBS {
        let root = format!(r"{CLASSES}\{}\{VERB}", verb.class);
        let key = CURRENT_USER
            .create(&root)
            .with_context(|| format!("cannot write {root}"))?;
        key.set_string("", label)?;
        // Explorer draws this next to the menu entry; the executable carries
        // its own icon, so it follows the app automatically.
        key.set_string("Icon", format!(r#""{executable}""#))?;
        CURRENT_USER
            .create(format!(r"{root}\command"))?
            .set_string("", command_line(&executable, verb.argument))?;
    }
    Ok(())
}

pub fn remove_context_menus() -> Result<()> {
    for verb in VERBS {
        // Absent is the desired state, so a missing key is success.
        let _ = CURRENT_USER.remove_tree(format!(r"{CLASSES}\{}\{VERB}", verb.class));
    }
    Ok(())
}

/// Register `rhyme://` for this user.
pub fn install_uri_scheme() -> Result<()> {
    let executable = executable()?;
    let root = format!(r"{CLASSES}\{}", super::uri::SCHEME);
    let key = CURRENT_USER.create(&root)?;
    key.set_string("", "URL:Rhyme Terminal")?;
    // Presence of this value, not its content, is what marks a URL protocol.
    key.set_string("URL Protocol", "")?;
    CURRENT_USER
        .create(format!(r"{root}\DefaultIcon"))?
        .set_string("", format!(r#""{executable}",0"#))?;
    CURRENT_USER
        .create(format!(r"{root}\shell\open\command"))?
        .set_string("", command_line(&executable, r#""%1""#))?;
    Ok(())
}

pub fn remove_uri_scheme() -> Result<()> {
    let _ = CURRENT_USER.remove_tree(format!(r"{CLASSES}\{}", super::uri::SCHEME));
    Ok(())
}

/// Read back what is installed, and whether it still points here.
pub fn status() -> Result<Status> {
    let executable = executable()?;
    let expected: Vec<String> = VERBS
        .iter()
        .map(|verb| command_line(&executable, verb.argument))
        .collect();
    let stored: Vec<Option<String>> = VERBS
        .iter()
        .map(|verb| {
            CURRENT_USER
                .open(format!(r"{CLASSES}\{}\{VERB}\command", verb.class))
                .and_then(|key| key.get_string(""))
                .ok()
        })
        .collect();
    let uri = CURRENT_USER
        .open(format!(
            r"{CLASSES}\{}\shell\open\command",
            super::uri::SCHEME
        ))
        .and_then(|key| key.get_string(""))
        .ok();
    let present = stored.iter().all(Option::is_some);
    let matches = stored
        .iter()
        .zip(&expected)
        .all(|(stored, expected)| stored.as_ref() == Some(expected));
    Ok(Status {
        context_menus: present && matches,
        uri_scheme: uri.as_deref() == Some(&*command_line(&executable, r#""%1""#)),
        // Something of ours is registered, but for an executable that has
        // since moved: an update left it behind.
        stale: (stored.iter().any(Option::is_some) && !matches)
            || uri.is_some_and(|uri| uri != command_line(&executable, r#""%1""#)),
        executable,
    })
}

/// Repoint anything already registered at the executable running now.
///
/// Called at startup: an update installs to a new path, and the entry Explorer
/// kept would otherwise launch a version that is no longer there. Only rewrites
/// what the user already chose to have — it never installs on its own.
pub fn reconcile(label: &str) -> Result<()> {
    let status = status()?;
    if !status.stale {
        return Ok(());
    }
    let registered = VERBS.iter().any(|verb| {
        CURRENT_USER
            .open(format!(r"{CLASSES}\{}\{VERB}\command", verb.class))
            .is_ok()
    });
    if registered {
        install_context_menus(label)?;
    }
    if CURRENT_USER
        .open(format!(
            r"{CLASSES}\{}\shell\open\command",
            super::uri::SCHEME
        ))
        .is_ok()
    {
        install_uri_scheme()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explorer parses the stored string itself, so the executable is quoted
    /// (it lives under `C:\Program Files\…` often enough) and the substitution
    /// is quoted too, or a path with a space arrives as several arguments.
    #[test]
    fn the_stored_command_quotes_both_the_executable_and_the_substitution() {
        let line = command_line(
            r"C:\Program Files\Rhyme\rhyme-terminal.exe",
            r#"--cwd "%V""#,
        );
        assert_eq!(
            line,
            r#""C:\Program Files\Rhyme\rhyme-terminal.exe" --cwd "%V""#
        );
    }

    /// The folder verbs use `%V` (it is what `Directory\Background` supplies)
    /// and the file verb `%1`; mixing them up registers a menu that opens the
    /// wrong place, which no test of ours would otherwise catch.
    #[test]
    fn folder_verbs_pass_the_directory_and_the_file_verb_passes_the_file() {
        assert_eq!(VERBS[0].class, r"Directory\shell");
        assert_eq!(VERBS[1].class, r"Directory\Background\shell");
        assert_eq!(VERBS[2].class, r"*\shell");
        assert!(VERBS[0].argument.contains("%V"));
        assert!(VERBS[1].argument.contains("%V"));
        assert!(VERBS[2].argument.contains("%1"));
    }

    /// Install → status → remove, on the live per-user hive. Writes only under
    /// this app's own verb name and puts the machine back as it found it.
    #[test]
    fn installing_registers_every_verb_and_removing_takes_them_all_away() {
        let before = status().unwrap();
        install_context_menus("Rhyme Terminal에서 열기").unwrap();
        install_uri_scheme().unwrap();
        let installed = status().unwrap();
        assert!(installed.context_menus);
        assert!(installed.uri_scheme);
        assert!(!installed.stale);
        remove_context_menus().unwrap();
        remove_uri_scheme().unwrap();
        let removed = status().unwrap();
        assert!(!removed.context_menus);
        assert!(!removed.uri_scheme);
        assert!(!removed.stale);
        // Restore whatever the developer running these tests had.
        if before.context_menus {
            install_context_menus("Rhyme Terminal에서 열기").unwrap();
        }
        if before.uri_scheme {
            install_uri_scheme().unwrap();
        }
    }
}
