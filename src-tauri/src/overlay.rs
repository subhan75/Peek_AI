use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindowBuilder};

pub const OVERLAY_LABEL: &str = "overlay";

/// A physical-screen-pixel rectangle (x, y, width, height).
type PhysicalRect = (f64, f64, f64, f64);

/// `set_ignore_cursor_events(true)` makes Windows route ALL mouse input
/// (including mousemove/mouseenter) straight through to whatever is
/// underneath -- the webview never receives those events, so it can't
/// detect "the cursor is over my card" via ordinary DOM hover handlers while
/// click-through is active. Instead, the frontend reports the card's screen
/// rect here whenever it moves, and lib.rs's cursor-poll loop toggles
/// click-through based on comparing that rect against GetCursorPos -- the
/// standard workaround for this Tauri/Windows limitation.
#[derive(Default)]
pub struct OverlayState {
    payload: Mutex<Option<serde_json::Value>>,
    hitbox: Mutex<Option<PhysicalRect>>,
    dragging: AtomicBool,
}

impl OverlayState {
    pub fn is_dragging(&self) -> bool {
        self.dragging.load(Ordering::Relaxed)
    }

    pub fn hitbox(&self) -> Option<PhysicalRect> {
        *self.hitbox.lock().unwrap()
    }
}

/// Repositions/resizes the overlay to exactly cover the primary monitor (in
/// physical pixels, so it lines up with capture_screen's screenshot
/// regardless of DPI scaling), since the monitor layout can change between
/// captures.
fn position_overlay(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "Overlay window not found".to_string())?;
    let monitor = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No primary monitor found".to_string())?;

    window
        .set_position(PhysicalPosition::new(monitor.position().x, monitor.position().y))
        .map_err(|e| e.to_string())?;
    window
        .set_size(PhysicalSize::new(monitor.size().width, monitor.size().height))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The overlay is declared in tauri.conf.json (with `"create": false`) so it
/// is built here from that same config via `from_config`, rather than
/// assembled field-by-field through the builder -- dynamically-constructed
/// transparent/always-on-top windows have several open Tauri bugs around
/// transparency and z-order that windows built from a static config entry
/// (the same path the main window goes through) don't hit.
fn ensure_overlay_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(OVERLAY_LABEL).is_some() {
        return Ok(());
    }

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == OVERLAY_LABEL)
        .cloned()
        .ok_or_else(|| "Overlay window not declared in tauri.conf.json".to_string())?;

    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Shows (creating if needed) the transparent overlay, positioned over the
/// primary monitor and click-through by default so it never blocks
/// interaction with whatever app is underneath. The payload is stored before
/// the window is created/shown so get_overlay_payload can always return it,
/// and also emitted directly in case the overlay is already mounted and
/// listening from a previous query.
#[tauri::command]
pub async fn show_overlay(
    app: AppHandle,
    payload: serde_json::Value,
    state: tauri::State<'_, OverlayState>,
) -> Result<(), String> {
    *state.payload.lock().unwrap() = Some(payload.clone());

    ensure_overlay_window(&app)?;
    position_overlay(&app)?;

    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "Overlay window not found".to_string())?;
    window.set_ignore_cursor_events(true).map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;

    let _ = app.emit_to(OVERLAY_LABEL, "overlay-response", payload);
    Ok(())
}

/// Pulled by the overlay window on mount, since it can't rely on having been
/// ready in time to catch show_overlay's emit.
#[tauri::command]
pub fn get_overlay_payload(state: tauri::State<OverlayState>) -> Option<serde_json::Value> {
    state.payload.lock().unwrap().clone()
}

#[tauri::command]
pub async fn hide_overlay(app: AppHandle, state: tauri::State<'_, OverlayState>) -> Result<(), String> {
    *state.payload.lock().unwrap() = None;
    *state.hitbox.lock().unwrap() = None;
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Reports the chat card's current screen rect (in the webview's logical/CSS
/// pixels) so the cursor-poll loop knows where clicks should be let through.
/// Converted to physical pixels here (using the overlay window's own origin
/// and scale factor, since it always exactly covers the primary monitor) so
/// the poll loop can compare it directly against GetCursorPos.
#[tauri::command]
pub fn set_overlay_hitbox(
    app: AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    state: tauri::State<OverlayState>,
) -> Result<(), String> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "Overlay window not found".to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let origin = window.outer_position().map_err(|e| e.to_string())?;

    *state.hitbox.lock().unwrap() = Some((
        origin.x as f64 + x * scale,
        origin.y as f64 + y * scale,
        width * scale,
        height * scale,
    ));
    Ok(())
}

#[tauri::command]
pub fn clear_overlay_hitbox(state: tauri::State<OverlayState>) {
    *state.hitbox.lock().unwrap() = None;
}

/// Called when the user starts dragging the card's header: forces the
/// window interactive immediately (rather than waiting for the next poll
/// tick) and tells the poll loop to back off so it doesn't fight the drag by
/// flipping click-through back on if a hitbox update lags behind the cursor.
#[tauri::command]
pub async fn begin_overlay_drag(app: AppHandle, state: tauri::State<'_, OverlayState>) -> Result<(), String> {
    state.dragging.store(true, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        window.set_ignore_cursor_events(false).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hands control back to the poll loop, which re-evaluates click-through
/// against whatever hitbox was last reported (the drop position).
#[tauri::command]
pub fn end_overlay_drag(state: tauri::State<OverlayState>) {
    state.dragging.store(false, Ordering::Relaxed);
}
