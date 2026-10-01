import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SettingsModal from "./components/SettingsModal";
import QueryPanel from "./components/QueryPanel";
import "./App.css";

function App() {
  const [captureActive, setCaptureActive] = useState(false);
  const [hasApiKey, setHasApiKey] = useState<boolean | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [screenshotDataUrl, setScreenshotDataUrl] = useState<string | null>(null);
  const [screenshotBase64, setScreenshotBase64] = useState<string | null>(null);
  const [cropDataUrl, setCropDataUrl] = useState<string | null>(null);
  const [cropBase64, setCropBase64] = useState<string | null>(null);
  const [cursorXFrac, setCursorXFrac] = useState<number | null>(null);
  const [cursorYFrac, setCursorYFrac] = useState<number | null>(null);
  const [appId, setAppId] = useState<string | null>(null);
  const [captureDimensions, setCaptureDimensions] = useState<{ width: number; height: number } | null>(null);
  const [captureLatencyMs, setCaptureLatencyMs] = useState<number | null>(null);
  const [captureBreakdown, setCaptureBreakdown] = useState<{
    captureMs: number;
    encodeMs: number;
    base64Ms: number;
  } | null>(null);
  const [captureError, setCaptureError] = useState<string | null>(null);

  async function runCapture() {
    setCaptureError(null);
    const start = performance.now();
    try {
      const result = await invoke<{
        image_base64: string;
        width: number;
        height: number;
        capture_ms: number;
        encode_ms: number;
        encode_bytes_ms: number;
        crop_base64: string | null;
        cursor_x_frac: number | null;
        cursor_y_frac: number | null;
        app_id: string | null;
      }>("capture_screen");
      setCaptureLatencyMs(performance.now() - start);
      setCaptureBreakdown({
        captureMs: result.capture_ms,
        encodeMs: result.encode_ms,
        base64Ms: result.encode_bytes_ms,
      });
      setCaptureDimensions({ width: result.width, height: result.height });
      setScreenshotDataUrl(`data:image/png;base64,${result.image_base64}`);
      setScreenshotBase64(result.image_base64);
      setCropBase64(result.crop_base64);
      setCropDataUrl(result.crop_base64 ? `data:image/png;base64,${result.crop_base64}` : null);
      setCursorXFrac(result.cursor_x_frac);
      setCursorYFrac(result.cursor_y_frac);
      setAppId(result.app_id);
    } catch (err) {
      setCaptureError(String(err));
    }
  }

  useEffect(() => {
    invoke<boolean>("has_gemini_api_key")
      .then((exists) => {
        setHasApiKey(exists);
        if (!exists) {
          setSettingsOpen(true);
        }
      })
      .catch((err) => {
        console.error("Failed to check for stored API key:", err);
      });
  }, []);

  useEffect(() => {
    const unlisten = listen("toggle-capture", () => {
      setCaptureActive((prev) => {
        const next = !prev;
        if (next) {
          void runCapture();
        } else {
          void invoke("hide_overlay");
        }
        return next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <main className="container">
      <SettingsModal
        open={settingsOpen}
        canDismiss={hasApiKey === true}
        onClose={() => setSettingsOpen(false)}
        onSaved={() => {
          setHasApiKey(true);
          setSettingsOpen(false);
        }}
      />

      <h1>PeekAI</h1>

      <div className="row" style={{ justifyContent: "flex-end" }}>
        <button type="button" onClick={() => setSettingsOpen(true)}>
          ⚙ Settings {hasApiKey ? "(key configured)" : "(no key set)"}
        </button>
      </div>

      <p style={{ fontWeight: "bold", color: captureActive ? "limegreen" : "gray" }}>
        Hotkey (Ctrl+Space) state: {captureActive ? "ACTIVE" : "inactive"}
      </p>

      <div style={{ border: "1px solid #444", padding: "1rem", margin: "1rem 0", textAlign: "left" }}>
        <h3>Latest capture</h3>
        {captureError && <p style={{ color: "tomato" }}>Capture failed: {captureError}</p>}
        {captureLatencyMs !== null && (
          <p>
            Last capture latency (round-trip): <strong>{captureLatencyMs.toFixed(1)} ms</strong>
            {captureDimensions && (
              <> &mdash; {captureDimensions.width}x{captureDimensions.height}px</>
            )}
          </p>
        )}
        {captureBreakdown && (
          <p style={{ fontSize: "0.9em", color: "#aaa" }}>
            Breakdown &mdash; raw grab: <strong>{captureBreakdown.captureMs.toFixed(1)} ms</strong>,
            PNG encode: <strong>{captureBreakdown.encodeMs.toFixed(1)} ms</strong>, base64: {" "}
            <strong>{captureBreakdown.base64Ms.toFixed(1)} ms</strong>
          </p>
        )}
        {screenshotDataUrl && (
          <img
            src={screenshotDataUrl}
            alt="Latest screen capture"
            style={{ maxWidth: "100%", maxHeight: "300px", border: "1px solid #888" }}
          />
        )}
        {cursorXFrac !== null && cursorYFrac !== null && (
          <p style={{ fontSize: "0.85em", color: "#aaa" }}>
            Cursor at capture time: x={cursorXFrac.toFixed(3)}, y={cursorYFrac.toFixed(3)}
          </p>
        )}
        {cropDataUrl && (
          <>
            <p style={{ fontSize: "0.85em", color: "#aaa", marginBottom: "0.25rem" }}>
              Zoomed crop around cursor (also sent to Gemini):
            </p>
            <img
              src={cropDataUrl}
              alt="Zoomed crop around cursor"
              style={{ maxWidth: "250px", border: "1px solid #888" }}
            />
          </>
        )}
      </div>

      <QueryPanel
        imageBase64={screenshotBase64}
        cropBase64={cropBase64}
        cursorXFrac={cursorXFrac}
        cursorYFrac={cursorYFrac}
        appId={appId}
      />
    </main>
  );
}

export default App;
