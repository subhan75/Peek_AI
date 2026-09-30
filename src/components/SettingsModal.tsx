import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SettingsModalProps {
  open: boolean;
  canDismiss: boolean;
  onClose: () => void;
  onSaved: () => void;
}

function SettingsModal({ open, canDismiss, onClose, onSaved }: SettingsModalProps) {
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open) {
    return null;
  }

  async function handleSave() {
    setError(null);
    setSaving(true);
    try {
      await invoke("set_gemini_api_key", { key: apiKey });
      setApiKey("");
      onSaved();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={() => canDismiss && onClose()}
    >
      <div
        style={{
          background: "#1e1e1e",
          color: "#f0f0f0",
          borderRadius: 12,
          padding: "1.5rem",
          width: "min(420px, 90vw)",
          textAlign: "left",
          boxShadow: "0 8px 30px rgba(0, 0, 0, 0.4)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 style={{ marginTop: 0 }}>Gemini API Key</h2>
        <p style={{ fontSize: "0.9em", color: "#aaa" }}>
          Your key is stored in the Windows Credential Manager and is used
          only by the app itself to call the Gemini API directly. It is never
          sent anywhere else.
        </p>
        <input
          type="password"
          autoFocus
          value={apiKey}
          onChange={(e) => setApiKey(e.currentTarget.value)}
          placeholder="Enter your Gemini API key"
          style={{ width: "100%", boxSizing: "border-box", marginBottom: "0.75rem" }}
        />
        {error && <p style={{ color: "tomato", fontSize: "0.9em" }}>{error}</p>}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
          {canDismiss && (
            <button type="button" onClick={onClose} disabled={saving}>
              Cancel
            </button>
          )}
          <button
            type="button"
            onClick={handleSave}
            disabled={saving || apiKey.trim().length === 0}
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default SettingsModal;
