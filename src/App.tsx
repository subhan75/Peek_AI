import { useEffect, useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [captureActive, setCaptureActive] = useState(false);
  const [screenshotDataUrl, setScreenshotDataUrl] = useState<string | null>(null);
  const [captureDimensions, setCaptureDimensions] = useState<{ width: number; height: number } | null>(null);
  const [captureLatencyMs, setCaptureLatencyMs] = useState<number | null>(null);
  const [captureBreakdown, setCaptureBreakdown] = useState<{
    captureMs: number;
    encodeMs: number;
    base64Ms: number;
  } | null>(null);
  const [captureError, setCaptureError] = useState<string | null>(null);

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

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
      }>("capture_screen");
      setCaptureLatencyMs(performance.now() - start);
      setCaptureBreakdown({
        captureMs: result.capture_ms,
        encodeMs: result.encode_ms,
        base64Ms: result.encode_bytes_ms,
      });
      setCaptureDimensions({ width: result.width, height: result.height });
      setScreenshotDataUrl(`data:image/png;base64,${result.image_base64}`);
    } catch (err) {
      setCaptureError(String(err));
    }
  }

  useEffect(() => {
    const unlisten = listen("toggle-capture", () => {
      setCaptureActive((prev) => {
        const next = !prev;
        if (next) {
          void runCapture();
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
      <h1>Welcome to Tauri + React</h1>

      <p style={{ fontWeight: "bold", color: captureActive ? "limegreen" : "gray" }}>
        Hotkey (Ctrl+Space) state: {captureActive ? "ACTIVE" : "inactive"}
      </p>

      <div style={{ border: "1px solid #444", padding: "1rem", margin: "1rem 0", textAlign: "left" }}>
        <h3>Debug: Screen Capture (Phase 2)</h3>
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
      </div>

      <div className="row">
        <a href="https://vite.dev" target="_blank">
          <img src="/vite.svg" className="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <p>Click on the Tauri, Vite, and React logos to learn more.</p>

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="submit">Greet</button>
      </form>
      <p>{greetMsg}</p>
    </main>
  );
}

export default App;
