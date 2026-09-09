// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(all(desktop, unix))]
    if winmux_lib::unix_cli::dispatch() {
        return;
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
