mod auth;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
