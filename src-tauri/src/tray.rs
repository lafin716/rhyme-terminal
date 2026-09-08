use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Listener, Manager, Runtime,
};

use crate::ipc::client::DaemonClient;
use crate::ipc::protocol::Method;

pub fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "rhyme-terminal 열기", true, None::<&str>)?;
    let hide_i = MenuItem::with_id(app, "hide", "창 숨기기", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", "종료 (데몬 유지)", true, None::<&str>)?;
    let kill_server_i = MenuItem::with_id(app, "kill-server", "서버 종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &hide_i, &sep, &quit_i, &kill_server_i])?;
    app.listen("app-language-changed", move |event| {
        let english = serde_json::from_str::<String>(event.payload())
            .map(|language| language == "en")
            .unwrap_or(false);
        for (item, ko, en) in [
            (&show_i, "rhyme-terminal 열기", "Show rhyme-terminal"),
            (&hide_i, "창 숨기기", "Hide window"),
            (&quit_i, "종료 (데몬 유지)", "Quit (keep daemon)"),
            (&kill_server_i, "서버 종료", "Kill server"),
        ] {
            if let Err(error) = item.set_text(if english { en } else { ko }) {
                tracing::warn!("Failed to update tray language: {error}");
            }
        }
    });
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;

    let _tray = TrayIconBuilder::with_id("winmux-tray")
        .tooltip("rhyme-terminal")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "hide" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            "quit" => {
                app.exit(0);
            }
            "kill-server" => {
                if let Some(client) = app.try_state::<Arc<DaemonClient>>() {
                    let client = client.inner().clone();
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = client.request_raw(Method::KillServer).await;
                        app_clone.exit(0);
                    });
                } else {
                    app.exit(0);
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
