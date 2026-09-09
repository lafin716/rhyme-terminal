#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(all(desktop, unix))]
    if winmux_lib::unix_cli::dispatch() {
        return;
    }
    if let Err(e) = winmux_lib::daemon::run() {
        eprintln!("winmuxd failed: {e}");
        std::process::exit(1);
    }
}
