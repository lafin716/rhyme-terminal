// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(all(desktop, unix))]
    match std::env::args().nth(1).as_deref() {
        Some("--winmux-hook") => {
            if winmux_lib::unix_cli::hook().is_err() {
                eprintln!("Rhyme Loop lifecycle bridge failed");
                std::process::exit(2);
            }
            return;
        }
        Some("--winmux-statusline") => {
            if let Some(dir) = std::env::args_os().nth(2) {
                let _ = winmux_lib::unix_cli::statusline(std::path::Path::new(&dir));
            }
            return;
        }
        _ => (),
    }
    #[cfg(desktop)]
    if std::env::args().nth(1).as_deref() == Some(winmux_lib::DAEMON_ARG) {
        if let Err(error) = winmux_lib::daemon::run() {
            eprintln!("winmuxd failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    winmux_lib::run()
}
