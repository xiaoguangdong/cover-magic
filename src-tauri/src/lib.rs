use base64::{engine::general_purpose, Engine as _};

#[tauri::command]
fn save_export_image(path: String, data_url: String) -> Result<(), String> {
  let (_, encoded_image) = data_url
    .split_once(',')
    .ok_or_else(|| "invalid image data url".to_string())?;

  let image_bytes = general_purpose::STANDARD
    .decode(encoded_image)
    .map_err(|error| format!("failed to decode image data: {error}"))?;

  std::fs::write(path, image_bytes).map_err(|error| format!("failed to save image: {error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![save_export_image])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
