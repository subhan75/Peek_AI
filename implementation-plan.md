# Phase-Wise Implementation Plan: Windows Screen-Aware AI Assistant

## Context

The project currently contains only `prd.md` — this is a greenfield build (no existing code, not yet a git repo). The PRD defines an MVP for a Tauri (Rust + React/TypeScript) desktop app that lets a user hotkey-capture their screen, ask a question, send it to a BYO-key VLM (OpenAI/Anthropic), and get back a text answer plus on-screen visual annotations rendered on a transparent overlay.

This plan breaks the PRD's functional requirements (FR-01 through FR-06) into an incremental build order — each phase produces a runnable, testable increment rather than one big-bang implementation — and closes with the core domain entities implied by that architecture, so subsequent implementation work has a shared vocabulary and data model to build against.

---

## Hard Constraint (applies to every phase below)

**Never move to the next phase until the current phase is fully tested and confirmed working as expected.**

- Each phase's "Exit criteria" must be manually verified on Windows before starting the next phase — no exceptions, no "we'll fix it later" carryover.
- If a phase fails its exit criteria, stop and fix it in place; do not layer the next phase's code on top of an unverified one.
- This applies uniformly across Phase 0 through Phase 6.

---

## Phase 0 — Project Scaffolding

- Initialize Tauri + React + TypeScript via `create-tauri-app` (or `npm create tauri-app@latest`).
- Set up repo structure: `src/` (React/TS frontend), `src-tauri/` (Rust backend), root `package.json`, `tauri.conf.json`, `Cargo.toml`.
- Confirm `npm install && npm run tauri dev` boots a blank window on a clean Windows machine — this is the Success Metric baseline from the PRD.
- Set up basic linting/formatting (ESLint/Prettier for TS, `rustfmt` for Rust) and a `.gitignore` (excluding secrets/build artifacts).
- Initialize git repo.

**Exit criteria:** blank Tauri window launches via `npm run tauri dev` with zero manual environment setup.

---

## Phase 1 — Global Hotkey Listener (FR-01)

- Add `tauri-plugin-global-shortcut` (or equivalent) in Rust backend.
- Register a configurable global hotkey (default `Ctrl+Space`).
- On press, emit a Tauri event to the frontend to toggle the app's capture state (show/hide overlay shell).
- Verify hotkey works while focus is in another application (core requirement — must work system-wide, not just when app is focused).

**Exit criteria:** pressing the hotkey from any Windows app triggers a visible state change in under ~200ms.

---

## Phase 2 — Screen Capture Utility (FR-02)

- Implement a Rust Tauri command that grabs the active primary display buffer in memory (e.g. via the `xcap` or `screenshots` crate) — no temp files written to disk.
- Return the captured image to the frontend as a base64-encoded payload (or transfer via Tauri's binary IPC if payload size becomes a concern).
- Wire hotkey trigger (Phase 1) → capture (Phase 2) → log/display captured image in a dev-only debug view to confirm correctness.
- Benchmark hotkey-to-capture latency against the <200ms NFR.

**Exit criteria:** hotkey press reliably produces a correct, in-memory screenshot within the latency budget.

---

## Phase 3 — API Key Management (FR-03)

- Build a React settings modal: provider selection (OpenAI/Anthropic), API key input field.
- Persist keys locally using `tauri-plugin-store` (or `stronghold` if stronger at-rest encryption is desired) — never written in plaintext config files, never sent anywhere but the chosen provider.
- Implement first-launch detection: if no key is stored, show the settings modal automatically.
- Add a Rust-side command layer for read/write/update of stored credentials, keeping key material out of frontend-visible logs.

**Exit criteria:** user can enter/update/persist an API key across app restarts; key is retrievable only by the backend for outgoing requests.

---

## Phase 4 — Query Input + VLM Integration Pipeline (FR-04)

- Build the post-capture overlay input box: "What would you like to know about this?"
- Design a provider-adapter interface (e.g. a Rust trait `VlmProvider` with `analyze(image, prompt) -> VlmResponse`) so OpenAI and Anthropic are pluggable implementations — this directly serves the PRD's success metric of easy contributor extensibility.
- Implement the OpenAI adapter and Anthropic adapter: format screenshot + prompt into each provider's vision API payload, send the request, parse the response.
- Define a normalized internal response shape (text explanation + structured coordinate targets) that adapters must produce, decoupling the rest of the app from provider-specific response formats.
- Handle error cases: missing/invalid key, network failure, malformed/unparsable model output — surfaced to the user, not silent failures.

**Exit criteria:** typing a question after capture returns a normalized text answer + coordinate data from the real API within a few seconds.

---

## Phase 4.5 — Conversational Memory & Privacy Guard

Two low-cost additions identified by comparing architecture with a similar open-source Windows assistant (Clacky). Both are cheap to add on top of the existing Phase 4 pipeline and don't require any new infrastructure. **Scope stays advisory/read-only** — the assistant still only tells the user what to do; it does not take actions or control the system (no agentic tool-use, no file/system mutation). That stays an explicit non-goal for now.

- **Per-app conversational memory**: keep a short, bounded conversation history (e.g. capped at the last ~20 messages) keyed to the active foreground window/app, so a follow-up question like "and what about the second one?" has context instead of every query being answered from a blank slate. History is in-memory only (not persisted to disk), consistent with the "never persisted" rule already applied to screenshots.
- **Privacy Guard**: before capturing, check whether the active foreground window looks sensitive (e.g. password managers, banking apps, a configurable denylist by window title/process name) and skip the capture (or warn the user) rather than silently sending that screen content to the VLM provider. This reinforces the PRD's zero-telemetry/privacy-first posture rather than conflicting with it.

**Exit criteria:** a follow-up question correctly uses context from the prior answer in the same app session; capturing while a denylisted/sensitive window is focused is blocked (or explicitly warned) instead of silently sent to the API.

---

## Phase 5 — Transparent Overlay & Coordinate Drawing Engine (FR-05, FR-06)

- Create the frameless, click-through, always-on-top transparent overlay window (Tauri window config: `transparent: true`, `decorations: false`, `alwaysOnTop: true`, click-through via platform APIs where the user isn't actively interacting with the chat card).
- Build the HTML5 canvas (or SVG) annotation layer that renders rings, bounding boxes, underlines, or pointer arrows.
- Implement coordinate mapping: translate model-returned coordinates (likely normalized 0–1 or model-resolution-relative) into actual screen pixel coordinates, accounting for display scaling/DPI.
- Build the side-by-side chat card component displaying the text answer.
- Wire Phase 4's normalized response into this rendering layer end-to-end.

**Exit criteria:** end-to-end flow — hotkey → capture → query → API → overlay drawing + chat answer — works visibly on screen within the 3-second latency target.

---

## Phase 6 — Polish, Performance & Packaging

- Profile and tune to meet NFRs: <200ms hotkey-to-capture, <100MB idle RAM, <3s total hotkey-to-visual-output.
- Audit for zero telemetry — confirm no network calls except to the user-configured LLM endpoint.
- Harden error handling across the pipeline (expired key, rate limits, offline state, oversized screenshots).
- Write contributor-facing docs: architecture overview, how to add a new provider adapter, how to add new annotation shapes.
- Verify `npm run tauri build` produces a working installer/binary on a clean Windows machine (final Success Metric check).

**Exit criteria:** all three PRD Success Metrics pass — clean build, <3s latency, clean architecture for contributor extension.

---

## Core Entities

Derived from the phases above, these are the system's core domain entities:

1. **AppSettings** — global configuration: selected provider, hotkey binding, UI preferences.
2. **HotkeyBinding** — the registered global shortcut and its active/inactive state.
3. **APIKeyCredential** — `{ provider: OpenAI | Anthropic, key: string (encrypted at rest) }`.
4. **CaptureSession** — represents one full hotkey-to-answer cycle: id, timestamp, current state (`capturing` → `awaiting_query` → `analyzing` → `rendering`), linked Screenshot, Query, and VlmResponse.
5. **Screenshot** — in-memory image buffer + metadata (display id, resolution, DPI scale, captured-at timestamp). Never persisted to disk.
6. **Query** — the user's typed natural-language question tied to a CaptureSession.
7. **VlmProvider** (adapter interface) — abstraction implemented by `OpenAiProvider` and `AnthropicProvider`, exposing a uniform `analyze(screenshot, query) -> VlmResponse` contract.
8. **VlmRequest** — the provider-specific formatted payload (base64 image + prompt) sent over the wire.
9. **VlmResponse** — normalized result: `{ text: string, annotations: Annotation[] }`, decoupled from provider-specific formats.
10. **Annotation** — a single visual marker: `{ type: ring | arrow | box | underline, coordinates, label? }`.
11. **OverlayWindow** — the transparent, click-through, always-on-top window's runtime state: visibility, current annotations being rendered, click-through toggling.
12. **ChatCard** — the answer-display UI element paired with the overlay, holding the current CaptureSession's text answer.
13. **AppConversationHistory** — in-memory, per-foreground-app message history (bounded, e.g. last ~20 messages) keyed by window/app identity, giving follow-up questions context without persisting anything to disk.
14. **PrivacyGuard** — a policy check run before capture that flags the current foreground window as sensitive (denylist by title/process name) and blocks or warns instead of silently capturing and sending it to the VLM provider.

---

## Verification

- Each phase ends with a manual smoke test on Windows (hotkey → observable behavior) before moving to the next phase.
- Phase 6 re-validates all PRD Success Metrics explicitly: clean-machine build, end-to-end latency measurement, and a quick contributor-experience check (can a new provider adapter be added by touching only the adapter layer, without touching capture/overlay code?).
