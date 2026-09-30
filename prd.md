# Product Requirements Document (PRD): Open-Source Windows Screen-Aware AI Assistant (MVP Scope)

## 1. Executive Summary

This PRD outlines the Minimum Viable Product (MVP) specification for an open-source, Windows-native desktop companion app built using **Tauri (Rust + React/TypeScript)**. The core objective is to replicate the foundational core loop of HeyClicky: allowing a user to press a global hotkey, capture screen context, pass it to an LLM provider using their own API key, and render visual annotations/drawings directly on a transparent overlay board to explain or answer questions about what is on screen.

---

## 2. Product Scope (In-Scope vs. Out-of-Scope)

### In-Scope (MVP Core Loop)

* **Global Hotkey Trigger:** A shortcut (e.g., `Ctrl + Space` or customizable) that instantly hides/reveals or triggers a capture state.
* **Screen Capture & Context Injection:** Capturing the active primary display buffer instantly without saving temp files to disk.
* **BYO-Key Configuration:** A simple local settings panel where users input their own LLM API keys (e.g., OpenAI, Anthropic).
* **Vision-Language Analysis:** Sending the screenshot + user text query/prompt to the chosen cloud VLM.
* **Transparent Overlay & Drawing Board:** A click-through, always-on-top transparent UI window that can render bounding boxes, highlight rings, or marker arrows over exact pixel coordinates returned by the model.

### Out-of-Scope (Excluded for MVP)

* Full computer-use agents (clicking, dragging, executing system scripts autonomously).
* Real-time bidirectional WebRTC voice conversation loops.
* Cross-app system-wide speech dictation.
* Cloud accounts, user auth databases, or subscription billing gates.

---

## 3. User Flow & Experience

1. **Setup:** User clones the GitHub repo, runs `npm install` and `npm run tauri dev`. On first launch, a minimalist settings modal prompts them to input their LLM API key (stored securely locally via Tauri store).
2. **Trigger:** User encounters a UI element, graph, or code snippet they want explained. They press the global hotkey (`Ctrl + Space`).
3. **Capture & Query:** The app captures the current screen, prompts an input box overlay: *"What would you like to know about this?"*, and the user types a quick question.
4. **Analysis & Visual Answer:** The request is sent to the VLM. The model returns both a text explanation and specific coordinate targets.
5. **Overlay Rendering:** The transparent Tauri overlay window highlights the specific UI element with a visual drawing ring or pointer and displays a side-by-side chat card with the answer.

---

## 4. Functional Requirements

| ID | Feature | Description | Priority |
| --- | --- | --- | --- |
| **FR-01** | Global Hotkey Listener | Register a system-wide hotkey via Rust backend to trigger the UI capture state. | P0 (Must Have) |
| **FR-02** | Screen Capture Utility | Grab a clean screenshot buffer of the active desktop layout on demand. | P0 (Must Have) |
| **FR-03** | API Key Management | Provide a React settings UI to store, update, and read local API keys securely. | P0 (Must Have) |
| **FR-04** | VLM Integration Pipeline | Format screenshot data into a base64 payload and send it to OpenAI/Anthropic APIs. | P0 (Must Have) |
| **FR-05** | Transparent Overlay Canvas | A frameless, click-through HTML5/CSS canvas overlay that sits above all native Windows apps. | P0 (Must Have) |
| **FR-06** | Coordinate Drawing Engine | Parse model outputs (e.g., target bounding boxes/pixels) and draw vector rings, underlines, or pointers on the canvas. | P1 (Should Have) |

---

## 5. Non-Functional Requirements

* **Performance:** The hotkey-to-capture transition must occur in under $200\text{ms}$ to feel snappy and native.
* **Resource Efficiency:** Idle RAM usage must remain under **100 MB** (leveraging Tauri's lightweight Rust footprint).
* **Portability:** Zero manual Python environment or complex virtual environment setup required for end users/contributors.
* **Privacy:** Zero telemetry. Screenshots and API requests travel strictly between the user's local machine and their designated LLM provider endpoint.

---

## 6. Success Metrics (MVP)

* Successfully compiles via `npm run tauri dev` on a clean Windows machine without environment errors.
* Latency under 3 seconds from hotkey execution to visual drawing output on screen.
* Clean separation of code architecture allowing open-source contributors to add custom UI components or provider adapters easily.

---

Would you like to map out the initial folder structure or start drafting the core Tauri/Rust screen-capture command for this PRD?
