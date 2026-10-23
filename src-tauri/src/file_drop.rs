//! Recovers the real paths of files dropped onto the main window from the OS
//! file manager.
//!
//! The main window keeps `dragDropEnabled: false` because Tauri's native drop
//! handler swallows HTML5 drag-and-drop on Windows, and pane tabs depend on it.
//! The page therefore only sees dropped files as DOM `File` objects, which carry
//! a name but no path. Each platform hands the path back its own way:
//!
//! - Windows: the page posts the `File` objects through WebView2's
//!   `postMessageWithAdditionalObjects`; the handler installed here reads each
//!   `ICoreWebView2File` path and emits [`DROP_EVENT`] back to the page.
//! - macOS: the drag pasteboard still holds the dropped file names after the
//!   drop, so [`dropped_file_paths`] reads them on demand.

/// Event emitted to the main window with `{ id, paths }` for a posted drop.
#[cfg(windows)]
pub const DROP_EVENT: &str = "external-file-drop";

/// Prefix of the web message the page posts with its dropped `File` objects.
/// The rest of the message is the request id echoed back in [`DROP_EVENT`].
#[cfg(windows)]
const DROP_MESSAGE_PREFIX: &str = "rhyme-file-drop:";

#[cfg(windows)]
pub fn install(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use tauri::Emitter;
    use webview2_com::{
        take_pwstr,
        Microsoft::Web::WebView2::Win32::{
            ICoreWebView2File, ICoreWebView2WebMessageReceivedEventArgs2,
        },
        WebMessageReceivedEventHandler,
    };
    use windows_core::{Interface, PWSTR};

    let emitter = window.clone();
    window.with_webview(move |webview| unsafe {
        let handler = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut message = PWSTR::null();
            // Tauri's own IPC messages also arrive here; anything that is not
            // a drop message is left to wry's handler.
            if args.TryGetWebMessageAsString(&mut message).is_err() {
                return Ok(());
            }
            let message = take_pwstr(message);
            let Some(id) = message.strip_prefix(DROP_MESSAGE_PREFIX) else {
                return Ok(());
            };
            let args: ICoreWebView2WebMessageReceivedEventArgs2 = args.cast()?;
            let objects = args.AdditionalObjects()?;
            let mut count = 0u32;
            objects.Count(&mut count)?;
            let mut paths = Vec::new();
            for index in 0..count {
                let Ok(file) = objects.GetValueAtIndex(index)?.cast::<ICoreWebView2File>() else {
                    continue;
                };
                let mut path = PWSTR::null();
                file.Path(&mut path)?;
                paths.push(take_pwstr(path));
            }
            let _ = emitter.emit_to(
                emitter.label(),
                DROP_EVENT,
                serde_json::json!({ "id": id, "paths": paths }),
            );
            Ok(())
        }));
        let result = webview.controller().CoreWebView2().and_then(|core| {
            let mut token = Default::default();
            core.add_WebMessageReceived(&handler, &mut token)
        });
        if let Err(error) = result {
            eprintln!("could not watch for dropped files: {error}");
        }
    })
}

/// File paths of the most recent OS drag, for a drop the page just received.
/// The page checks the names against its `File` objects, since the pasteboard
/// keeps the last drag around until the next one starts.
#[tauri::command]
pub fn dropped_file_paths() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        drag_pasteboard_paths()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
#[allow(deprecated)] // NSFilenamesPboardType is still what Finder drags carry.
fn drag_pasteboard_paths() -> Vec<String> {
    use objc2_app_kit::{NSFilenamesPboardType, NSPasteboard, NSPasteboardNameDrag};
    use objc2_foundation::{NSArray, NSString};

    unsafe {
        let pasteboard = NSPasteboard::pasteboardWithName(NSPasteboardNameDrag);
        let Some(list) = pasteboard.propertyListForType(NSFilenamesPboardType) else {
            return Vec::new();
        };
        let Ok(list) = list.downcast::<NSArray>() else {
            return Vec::new();
        };
        list.iter()
            .filter_map(|item| item.downcast::<NSString>().ok())
            .map(|path| path.to_string())
            .collect()
    }
}
