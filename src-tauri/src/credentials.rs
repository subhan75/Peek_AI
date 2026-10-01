use keyring::Entry;

const SERVICE_NAME: &str = "peek-ai";
const GEMINI_KEY_USERNAME: &str = "gemini-api-key";

fn gemini_entry() -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, GEMINI_KEY_USERNAME).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn has_gemini_api_key() -> Result<bool, String> {
    match gemini_entry()?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_gemini_api_key(key: String) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    gemini_entry()?.set_password(trimmed).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_gemini_api_key() -> Result<(), String> {
    match gemini_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Backend-only accessor for outgoing Gemini requests (used by vlm.rs).
/// Deliberately not registered as a Tauri command, so the raw key value
/// never crosses the IPC bridge to the frontend.
pub fn get_gemini_api_key() -> Result<String, String> {
    gemini_entry()?.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => "No Gemini API key configured".to_string(),
        other => other.to_string(),
    })
}
