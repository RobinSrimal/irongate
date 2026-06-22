mod auth;

use tauri::{menu::MenuBuilder, Manager};

const TRAY_ICON: tauri::image::Image<'_> =
    tauri::include_image!("./icons/irongate-desktop-logo.png");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let menu = MenuBuilder::new(app)
                .text("show", "Show Irongate")
                .separator()
                .text("quit", "Quit")
                .build()?;

            match tauri::tray::TrayIconBuilder::new()
                .icon(TRAY_ICON)
                .icon_as_template(false)
                .menu(&menu)
                .tooltip("Irongate")
                .on_menu_event(|app, event| match event.id().0.as_str() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)
            {
                Ok(tray) => {
                    app.manage(tray);
                }
                Err(error) => {
                    eprintln!("failed to create tray icon: {error}");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::login_with_provider,
            auth::login_with_password,
            auth::refresh_session,
            auth::logout,
            auth::stored_session_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
