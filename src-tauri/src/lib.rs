mod credentials;
mod foreground;
mod memory;
mod overlay;
mod privacy;
mod vlm;

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
    crop_base64: Option<String>,
    cursor_x_frac: Option<f64>,
    cursor_y_frac: Option<f64>,
    app_id: Option<String>,
}

/// Global mouse cursor position, in physical screen pixels.
pub(crate) fn cursor_position() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok != 0 {
        Some((point.x, point.y))
    } else {
        None
    }
}

/// The cursor's position as a (0.0-1.0, 0.0-1.0) fraction of the given
/// monitor's bounds, or None if the cursor isn't on that monitor.
fn cursor_fraction_on_monitor(monitor: &xcap::Monitor, width: u32, height: u32) -> Option<(f64, f64)> {
    let (cursor_x, cursor_y) = cursor_position()?;
    let monitor_x = monitor.x().ok()?;
    let monitor_y = monitor.y().ok()?;
    let rel_x = cursor_x - monitor_x;
    let rel_y = cursor_y - monitor_y;
    if rel_x < 0 || rel_y < 0 || rel_x as u32 > width || rel_y as u32 > height {
        return None;
    }
    Some((rel_x as f64 / width as f64, rel_y as f64 / height as f64))
}

/// A square crop centered on the given fractional position, clamped to the
/// image's bounds, encoded as base64 PNG -- gives the VLM a closer look at
/// whatever the user's cursor was pointing at.
fn cursor_crop_base64(image: &image::RgbaImage, cursor_frac: (f64, f64)) -> Option<String> {
    use base64::Engine;

    let width = image.width();
    let height = image.height();
    let crop_size = ((width.min(height) as f64) * 0.5) as u32;
    if crop_size == 0 {
        return None;
    }

    let center_x = (cursor_frac.0 * width as f64) as i64;
    let center_y = (cursor_frac.1 * height as f64) as i64;
    let half = (crop_size / 2) as i64;

    let max_x0 = (width as i64 - crop_size as i64).max(0);
    let max_y0 = (height as i64 - crop_size as i64).max(0);
    let x0 = (center_x - half).clamp(0, max_x0) as u32;
    let y0 = (center_y - half).clamp(0, max_y0) as u32;
    let crop_w = crop_size.min(width);
    let crop_h = crop_size.min(height);

    let cropped = image::imageops::crop_imm(image, x0, y0, crop_w, crop_h).to_image();
    let mut crop_bytes: Vec<u8> = Vec::new();
    cropped
        .write_to(&mut std::io::Cursor::new(&mut crop_bytes), image::ImageFormat::Png)
        .ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(&crop_bytes))
}

#[tauri::command]
fn capture_screen() -> Result<CaptureResult, String> {
    use base64::Engine;
    use std::time::Instant;

    let privacy_check = privacy::check_foreground_window();
    if privacy_check.blocked {
        return Err(format!(
            "Capture blocked: {}. Screen capture was skipped to protect your privacy.",
            privacy_check.reason.unwrap_or_else(|| "the active window looks sensitive".to_string())
        ));
    }
    let app_id = foreground::foreground_window()
        .map(|w| if w.process_name.is_empty() { w.title } else { w.process_name })
        .filter(|id| !id.is_empty());

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

    let cursor_frac = cursor_fraction_on_monitor(&monitor, width, height);
    let crop_base64 = cursor_frac.and_then(|frac| cursor_crop_base64(&image, frac));

    println!(
        "[capture_screen] capture={capture_ms:.1}ms encode={encode_ms:.1}ms base64={encode_bytes_ms:.1}ms png_bytes={} cursor={:?} app_id={:?}",
        png_bytes.len(),
        cursor_frac,
        app_id
    );

    Ok(CaptureResult {
        image_base64,
        width,
        height,
        capture_ms,
        encode_ms,
        encode_bytes_ms,
        crop_base64,
        cursor_x_frac: cursor_frac.map(|(x, _)| x),
        cursor_y_frac: cursor_frac.map(|(_, y)| y),
        app_id,
    })
}

/// Click-through (`set_ignore_cursor_events(true)`) stops the overlay
/// webview from receiving mouse events at all, so it can't detect its own
/// hover via DOM events -- this polls the real OS cursor position instead
/// and flips click-through off only while the cursor is over the chat
/// card's last-reported rect (see overlay::set_overlay_hitbox).
fn spawn_overlay_cursor_poll(app: tauri::AppHandle) {
    use std::time::Duration;
    use tauri::Manager;

    std::thread::spawn(move || {
        let mut last_ignore: Option<bool> = None;
        loop {
            std::thread::sleep(Duration::from_millis(33));

            let Some(window) = app.get_webview_window(overlay::OVERLAY_LABEL) else {
                continue;
            };
            if !window.is_visible().unwrap_or(false) {
                continue;
            }

            let state = app.state::<overlay::OverlayState>();
            if state.is_dragging() {
                continue;
            }

            let inside = match (cursor_position(), state.hitbox()) {
                (Some((cx, cy)), Some((x, y, w, h))) => {
                    let cx = cx as f64;
                    let cy = cy as f64;
                    cx >= x && cx <= x + w && cy >= y && cy <= y + h
                }
                _ => false,
            };

            let desired_ignore = !inside;
            if last_ignore != Some(desired_ignore) {
                let _ = window.set_ignore_cursor_events(desired_ignore);
                last_ignore = Some(desired_ignore);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Emitter;
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState};

    tauri::Builder::default()
        .manage(memory::ConversationStore::default())
        .manage(overlay::OverlayState::default())
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
            spawn_overlay_cursor_poll(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            capture_screen,
            credentials::has_gemini_api_key,
            credentials::set_gemini_api_key,
            credentials::clear_gemini_api_key,
            vlm::analyze_screenshot,
            overlay::show_overlay,
            overlay::hide_overlay,
            overlay::get_overlay_payload,
            overlay::set_overlay_hitbox,
            overlay::clear_overlay_hitbox,
            overlay::begin_overlay_drag,
            overlay::end_overlay_drag
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
