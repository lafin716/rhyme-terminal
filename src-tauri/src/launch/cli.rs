//! Command line → [`LaunchRequest`]. Parsing only: nothing here creates a
//! window, a session or a PTY, so the same flags serve Explorer's context
//! menus, a user's own shell, and an external tool configured to use Rhyme
//! Terminal as its terminal.

use std::path::PathBuf;

use clap::Parser;

use super::{LaunchRequest, LaunchSource};

#[derive(Parser, Debug)]
#[command(
    name = "rhyme-terminal",
    about = "Rhyme Terminal — open a terminal from anywhere",
    version,
    disable_help_flag = true,
    // A GUI launched from Explorer must never die on an argument it does not
    // know, and an external tool may pass switches of its own. `parse` treats
    // any parse failure as "just open the window" (see `parse_or_empty`).
    ignore_errors = true
)]
struct Cli {
    /// Working directory for the new session
    #[arg(long, value_name = "DIR")]
    cwd: Option<String>,
    /// File to open the session next to; repeatable
    #[arg(long = "file", value_name = "PATH")]
    files: Vec<String>,
    /// Command to run in the new session instead of an idle prompt
    #[arg(long, value_name = "COMMAND")]
    command: Option<String>,
    /// SSH target, e.g. `user@host`
    #[arg(long, value_name = "TARGET")]
    ssh: Option<String>,
    /// Open a new window instead of a tab in the running instance
    #[arg(long)]
    new_window: bool,
    /// Directories and files, for callers that pass bare paths
    #[arg(value_name = "PATH")]
    paths: Vec<String>,
}

/// Arguments this process must never read as a launch request: they select a
/// different program entirely and are dispatched before the GUI starts.
pub fn is_reserved(argument: &str) -> bool {
    argument == crate::DAEMON_ARG || argument.starts_with("--winmux-")
}

/// Parse a process's arguments (including argv[0]).
///
/// Anything unrecognised yields the empty request rather than an error: these
/// arguments come from Explorer verbs, other people's tools and users' own
/// shortcuts, and the worst outcome of a misspelling should be a terminal that
/// opens at the default directory — never a window that refuses to appear.
pub fn parse<I, T>(args: I, base: &std::path::Path) -> LaunchRequest
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args: Vec<std::ffi::OsString> = args.into_iter().map(Into::into).collect();
    // A registered scheme arrives as one bare argument. Recognised here rather
    // than in a separate entry point so every integration still comes in
    // through one door.
    if let Some(uri) = args
        .iter()
        .skip(1)
        .filter_map(|arg| arg.to_str())
        .find(|arg| super::uri::is_uri(arg))
    {
        return super::uri::parse(uri).normalized(base);
    }
    let cli = Cli::try_parse_from(args).unwrap_or_else(|_| Cli {
        cwd: None,
        files: vec![],
        command: None,
        ssh: None,
        new_window: false,
        paths: vec![],
    });
    let mut request = LaunchRequest {
        cwd: cli.cwd,
        files: cli.files,
        command: cli.command,
        ssh: cli.ssh,
        new_window: cli.new_window,
        source: LaunchSource::Cli,
    };
    // A bare path is the most common way an external program names a place —
    // `rhyme-terminal.exe D:\work`. Directories become the cwd, files join the
    // file list; `normalized` settles which is which by looking at the disk.
    request.files.extend(cli.paths);
    request.normalized(base)
}

/// The request this process was started with, resolved against its own working
/// directory so relative paths mean what the caller meant.
pub fn from_env() -> LaunchRequest {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    parse(std::env::args_os(), &base)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical *and* prefix-free, which is the form a parsed request holds:
    /// `canonicalize` adds Windows' verbatim `\\?\`, and several shells choke
    /// on it, so `absolute` strips it again on the way out.
    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rhyme-cli-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        super::super::strip_verbatim(dir.canonicalize().unwrap())
    }
    fn parse_args(args: &[&str], base: &std::path::Path) -> LaunchRequest {
        let mut argv = vec!["rhyme-terminal.exe"];
        argv.extend_from_slice(args);
        parse(argv, base)
    }

    #[test]
    fn cwd_flag_names_the_session_directory() {
        let dir = temp();
        let request = parse_args(&["--cwd", &dir.to_string_lossy()], &dir);
        assert_eq!(request.cwd.as_deref(), Some(&*dir.to_string_lossy()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn several_file_flags_are_all_kept() {
        let dir = temp();
        for name in ["a.txt", "b.txt"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let request = parse_args(&["--file", "a.txt", "--file", "b.txt"], &dir);
        assert_eq!(request.files.len(), 2);
        assert_eq!(request.cwd.as_deref(), Some(&*dir.to_string_lossy()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn command_and_ssh_pass_through_untouched() {
        let dir = temp();
        let request = parse_args(&["--command", "git status", "--ssh", "user@host"], &dir);
        assert_eq!(request.command.as_deref(), Some("git status"));
        assert_eq!(request.ssh.as_deref(), Some("user@host"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A path with spaces arrives as one argument — the shell already removed
    /// the quotes — so the parser must not re-split it.
    #[test]
    fn a_spaced_path_stays_one_argument() {
        let root = temp();
        let dir = root.join("my project");
        std::fs::create_dir_all(&dir).unwrap();
        let request = parse_args(&["--cwd", "my project"], &root);
        assert_eq!(request.cwd.as_deref(), Some(&*dir.to_string_lossy()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_bare_path_is_read_as_the_directory_to_open() {
        let root = temp();
        let dir = root.join("개발 프로젝트");
        std::fs::create_dir_all(&dir).unwrap();
        let request = parse_args(&["개발 프로젝트"], &root);
        assert_eq!(request.cwd.as_deref(), Some(&*dir.to_string_lossy()));
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Whatever an unknown integration passes, the window still opens.
    #[test]
    fn unknown_arguments_degrade_to_just_opening_the_window() {
        let dir = temp();
        let request = parse_args(&["--not-a-flag", "-x", "7"], &dir);
        assert!(request.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn no_arguments_is_the_empty_request() {
        let dir = temp();
        assert!(parse_args(&[], &dir).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The scheme handler is registered as `"exe" "%1"`, so the URL reaches the
    /// same argv as everything else.
    #[test]
    fn a_registered_url_is_parsed_as_a_launch_request() {
        let dir = temp();
        let request = parse_args(&["rhyme://ssh?host=example.com&user=test"], &dir);
        assert_eq!(request.ssh.as_deref(), Some("test@example.com"));
        assert_eq!(request.source, LaunchSource::Uri);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn daemon_and_internal_switches_are_reserved() {
        assert!(is_reserved(crate::DAEMON_ARG));
        assert!(!is_reserved("--cwd"));
    }
}
