import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

// Dynamic imports keep each window's module graph separate, so the overlay
// window never evaluates App.css's opaque :root background (which would
// defeat its transparent:true window setting).
async function bootstrap() {
  if (getCurrentWindow().label === "overlay") {
    const { default: Overlay } = await import("./components/Overlay");
    root.render(
      <React.StrictMode>
        <Overlay />
      </React.StrictMode>,
    );
  } else {
    const { default: App } = await import("./App");
    root.render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  }
}

void bootstrap();
