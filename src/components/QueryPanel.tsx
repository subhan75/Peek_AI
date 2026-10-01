import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { VlmResponse } from "../types";

interface QueryPanelProps {
  imageBase64: string | null;
  cropBase64?: string | null;
  cursorXFrac?: number | null;
  cursorYFrac?: number | null;
  appId?: string | null;
}

function QueryPanel({ imageBase64, cropBase64, cursorXFrac, cursorYFrac, appId }: QueryPanelProps) {
  const [query, setQuery] = useState("");
  const [analyzing, setAnalyzing] = useState(false);
  const [response, setResponse] = useState<VlmResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);

  if (!imageBase64) {
    return null;
  }

  async function handleAsk() {
    if (!imageBase64 || query.trim().length === 0) {
      return;
    }
    setError(null);
    setResponse(null);
    setAnalyzing(true);
    const start = performance.now();
    try {
      const result = await invoke<VlmResponse>("analyze_screenshot", {
        imageBase64,
        query,
        cropBase64: cropBase64 ?? null,
        cursorXFrac: cursorXFrac ?? null,
        cursorYFrac: cursorYFrac ?? null,
        appId: appId ?? null,
      });
      setLatencyMs(performance.now() - start);
      setResponse(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setAnalyzing(false);
    }
  }

  return (
    <div style={{ border: "1px solid #444", padding: "1rem", margin: "1rem 0", textAlign: "left" }}>
      <h3>Ask about this screenshot</h3>
      <div className="row" style={{ gap: "0.5rem" }}>
        <input
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              void handleAsk();
            }
          }}
          placeholder="What would you like to know about this?"
          style={{ flex: 1 }}
        />
        <button type="button" onClick={handleAsk} disabled={analyzing || query.trim().length === 0}>
          {analyzing ? "Asking..." : "Ask"}
        </button>
      </div>

      {error && <p style={{ color: "tomato" }}>{error}</p>}

      {response && (
        <div style={{ marginTop: "0.75rem" }}>
          {latencyMs !== null && (
            <p style={{ fontSize: "0.85em", color: "#aaa" }}>
              Answered in {(latencyMs / 1000).toFixed(1)}s
            </p>
          )}
          <p>
            <strong>Answer:</strong> {response.text}
          </p>
          {response.annotations.length > 0 && (
            <>
              <p style={{ marginBottom: "0.25rem" }}>
                <strong>Annotations ({response.annotations.length}):</strong>
              </p>
              <pre
                style={{
                  fontSize: "0.8em",
                  color: "#aaa",
                  whiteSpace: "pre-wrap",
                  background: "#111",
                  padding: "0.5rem",
                  borderRadius: 6,
                }}
              >
                {JSON.stringify(response.annotations, null, 2)}
              </pre>
            </>
          )}
        </div>
      )}
    </div>
  );
}

export default QueryPanel;
