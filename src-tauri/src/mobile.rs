// The Android app is a remote client. Desktop commands, PTYs and tray plugins
// are deliberately absent; remote pages never receive native IPC permissions.
#[tauri::mobile_entry_point]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running mobile application");
}
