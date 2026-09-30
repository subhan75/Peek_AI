mod credentials;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(serde::Serialize)]
struct CaptureResult {
    image_base64: String,
    width: u32,
    height: u32,
    capture_ms: f64,
    encode_ms: f64,
    encode_bytes_ms: f64,
}

#[tauri::command]
fn capture_screen() -> Result<CaptureResult, String> {
    use base64::Engine;
    use std::time::Instant;

    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or_else(|| "No primary monitor found".to_string())?;

    let capture_start = Instant::now();
    let image = monitor.capture_image().map_err(|e| e.to_string())?;
    let capture_ms = capture_start.elapsed().as_secs_f64() * 1000.0;

    let width = image.width();
    let height = image.height();

    let encode_start = Instant::now();
    let mut png_bytes: Vec<u8> = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let encode_ms = encode_start.elapsed().as_secs_f64() * 1000.0;

    let b64_start = Instant::now();
    let image_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    let encode_bytes_ms = b64_start.elapsed().as_secs_f64() * 1000.0;

    println!(
        "[capture_screen] capture={capture_ms:.1}ms encode={encode_ms:.1}ms base64={encode_bytes_ms:.1}ms png_bytes={}",
        png_bytes.len()
    );

    Ok(CaptureResult {
        image_base64,
        width,
        height,
        capture_ms,
        encode_ms,
        encode_bytes_ms,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Emitter;
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState};

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = app.emit("toggle-capture", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            let shortcut = tauri_plugin_global_shortcut::Shortcut::new(
                Some(Modifiers::CONTROL),
                Code::Space,
            );
            app.global_shortcut().register(shortcut)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            capture_screen,
            credentials::has_gemini_api_key,
            credentials::set_gemini_api_key,
            credentials::clear_gemini_api_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
