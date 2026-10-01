# Architecture

This document explains how Screen Aware Assistant is put together: the
process/window model, the end-to-end request flow, and the non-obvious design
decisions that exist because of real bugs hit during development. For setup
instructions see [README.md](README.md).

## Tech stack

| Layer | Technology |
|---|---|
| Shell / native runtime | [Tauri v2](https://tauri.app/) |
| Backend | Rust |
| Frontend | React 19 + TypeScript, built with Vite |
| VLM | Google Gemini, called directly over HTTP (`reqwest`) |
| Windows integration | `windows-sys` (raw Win32 calls), `keyring` (Credential Manager), `xcap` (screen capture) |

## Process / window model

The app is a single Tauri process with **two webview windows**, both loading
the same React bundle (`src/main.tsx` branches on the window label to decide
whether to render `App` or `Overlay`):

- **`main`** — the normal, decorated app window: settings modal, the debug
  capture view, and the query input panel.
- **`overlay`** — a frameless, transparent, always-on-top window that is
  resized/repositioned to exactly cover the primary monitor whenever it's
  shown. It renders annotation markers and a draggable "chat card" with the
  answer, on top of whatever app the user is actually looking at.

The overlay window is **declared statically in `tauri.conf.json`** (with
`"create": false`) rather than built at runtime with
`WebviewWindowBuilder::new(...).build()`. This is a deliberate workaround:
dynamically-constructed transparent/always-on-top windows hit several open
Tauri bugs around transparency and z-ordering. A window declared in config
and instantiated via `WebviewWindowBuilder::from_config` goes through the
same code path as the `main` window, which is well-tested. See the doc
comment on `ensure_overlay_window` in `src-tauri/src/overlay.rs`.

## End-to-end request flow

```
1. Global hotkey press (Ctrl+Space), anywhere in Windows
   -> tauri-plugin-global-shortcut (registered in lib.rs::run)
   -> emits a "toggle-capture" event to the main window
   -> App.tsx's listener calls runCapture()

2. capture_screen (Rust command, lib.rs)
   -> Privacy Guard check (privacy.rs::check_foreground_window): if the
      foreground window's process name or title matches a denylist
      (password managers, banking/2FA keywords), capture is refused outright
      and nothing is sent anywhere
   -> xcap grabs the primary monitor into memory (never written to disk)
   -> cursor position is read (GetCursorPos) and converted to a 0-1 fraction
      of the monitor; a square crop centered on the cursor is cut from the
      FULL-RESOLUTION image for fine detail (small icons/text near the
      pointer)
   -> the full screenshot is separately downscaled to a 1568px long edge
      (downscale_for_vlm) before PNG-encoding, to bound payload size/latency
      on 4K+ displays -- the crop is unaffected by this cap
   -> both images (base64 PNG) + cursor fraction + the foreground app's
      identity (app_id) are returned to the frontend

3. User types a question -> QueryPanel.tsx calls analyze_screenshot

4. analyze_screenshot (Rust command, vlm.rs)
   -> looks up this app's recent conversation turns from ConversationStore
      (memory.rs) -- an in-memory-only, per-app-id ring buffer capped at 10
      turns, never persisted to disk or cleared only on process restart
   -> build_prompt() assembles the request: the question, cursor position
      (so "what's this?" resolves to whatever's under the pointer), and, if
      there's history, the prior Q&A text PLUS the prior turn's raw
      annotation JSON (box_2d, still in Gemini's native 0-1000 space) --
      echoed back so a follow-up question about the same on-screen subject
      can reuse those exact coordinates instead of the model re-deriving
      (and drifting from) them
   -> calls Gemini's API with the full screenshot + crop + a JSON response
      schema requiring { text, annotations[] }
   -> HTTP/model errors are classified into actionable messages: 401/403 ->
      "your key was rejected", 429 -> "rate-limited", 5xx -> "Gemini's
      servers are having trouble", connect/timeout errors -> an offline
      message -- rather than one generic "request failed"
   -> Gemini's box_2d ([ymin, xmin, ymax, xmax], each 0-1000) is normalized
      into { x, y, width, height } as 0-1 fractions of the full screen --
      resolution-independent, so the overlay can place markers correctly
      regardless of what resolution the screenshot was downscaled to
   -> the turn (query, answer text, raw annotations JSON) is pushed into
      ConversationStore for this app_id

5. QueryPanel.tsx receives the normalized response and calls show_overlay

6. show_overlay (Rust command, overlay.rs)
   -> stores the payload in OverlayState (so a late-mounting overlay window
      can pull it, see below) and emits it as an "overlay-response" event
   -> creates the overlay window if needed, repositions/resizes it to cover
      the primary monitor in physical pixels, sets it click-through, shows it
   -> Overlay.tsx renders AnnotationMarker elements (positioned via the 0-1
      fractions above, as CSS percentages -- no pixel math needed) and the
      chat card, positioned just below the first annotation by default
```

## Non-obvious design decisions (and the bugs behind them)

These exist because of problems actually hit during development, not
speculative hardening -- worth understanding before touching this code.

**Overlay built from static config, not `WebviewWindowBuilder::new()`.**
Covered above. Open Tauri issues: #12450, #9798, #8308.

**`show_overlay`/`hide_overlay`/drag commands are `async fn`, not `fn`.**
A plain synchronous Tauri command that creates or manipulates a window
(`.build()`, `.show()`, `.set_ignore_cursor_events()`) can deadlock against
the main GUI event loop thread. Tauri's own official example for this exact
pattern uses `async fn`. Async commands that take managed state must use
`tauri::State<'_, T>` (an explicit lifetime), not `tauri::State<T>`.

**Click-through interactivity is polled from a background thread, not
event-driven.** `window.set_ignore_cursor_events(true)` makes Windows route
*all* mouse input (mousemove, mouseenter, mousedown, everything) straight
through to whatever is underneath. While click-through is active, the
overlay webview receives **no mouse events at all**, so it's structurally
impossible to detect "the cursor is now over my chat card" with ordinary
`onMouseEnter`/`onMouseLeave` DOM handlers -- the event that would trigger
turning click-through off can never fire. The fix (`spawn_overlay_cursor_poll`
in `lib.rs`) is a background thread that polls `GetCursorPos` every 33ms and
compares it against the chat card's last-reported screen rect (reported by
the frontend via `set_overlay_hitbox` whenever the card moves or resizes),
toggling `set_ignore_cursor_events` only when the answer changes. Dragging
the card (`begin_overlay_drag`/`end_overlay_drag`) forces click-through off
immediately and tells the poll loop to stand down so it doesn't fight the
drag if a hitbox update lags a frame behind the cursor.

**Physical vs. logical pixels.** `WebviewWindowBuilder::position()` /
`inner_size()` and CSS/`getBoundingClientRect()` all work in *logical*
(DPI-scaled) pixels, but `Monitor::position()`/`size()` and `GetCursorPos`
are in *physical* pixels. `set_overlay_hitbox` converts the card's logical
rect to physical using the overlay window's `scale_factor()` and
`outer_position()` before storing it, so the poll loop can compare directly
against `GetCursorPos` without drift on scaled displays.

**The overlay pulls its payload on mount instead of only listening for it.**
`show_overlay`'s `emit()` can fire before a freshly-created overlay webview's
`listen()` call has registered (the page is still mounting) -- a classic
event race. `Overlay.tsx` calls `get_overlay_payload` once on mount (which
reads from `OverlayState`, populated synchronously before the window is even
created) in addition to listening for live `"overlay-response"` events, so
it never misses the first answer.

**Follow-up annotations reuse the prior turn's exact coordinates.** Early
versions re-derived a bounding box from scratch on every turn, which caused
visible drift ("hallucination") between turns for the same on-screen
subject. The fix echoes the previous turn's raw `box_2d` JSON back into the
prompt's conversation history and explicitly instructs the model to reuse it
verbatim unless the follow-up clearly points somewhere else
(`ConversationTurn.annotations_json` in `memory.rs`, prompt instruction in
`vlm.rs::build_prompt`).

## Security & privacy model

- **API key**: entered once via the Settings modal, stored in the Windows
  Credential Manager via the `keyring` crate. `get_gemini_api_key()` is a
  plain Rust function, deliberately *not* registered as a `#[tauri::command]`
  — the raw key value can never cross the IPC bridge to the frontend.
- **No disk persistence**: screenshots and crops exist only in memory for the
  duration of a single capture/query cycle. Conversation history
  (`ConversationStore`) is in-memory only, capped at 10 turns per foreground
  app, and gone on process restart.
- **Privacy Guard** (`privacy.rs`): checked before every capture, blocks
  outright if the foreground window's process name or title matches a
  denylist (password managers, banking/2FA-related keywords) — capture never
  happens, so nothing sensitive is ever even read into memory, let alone sent
  to Gemini.
- **Zero telemetry**: the only outbound network calls anywhere in the app go
  to `generativelanguage.googleapis.com`. No analytics or crash-reporting SDK
  is in the dependency tree.

## File map

| File | Purpose |
|---|---|
| `src-tauri/src/lib.rs` | App bootstrap, hotkey registration, `capture_screen` command, screenshot downscaling, the overlay cursor-poll thread |
| `src-tauri/src/vlm.rs` | Prompt construction, Gemini API call, response parsing/normalization, error classification |
| `src-tauri/src/memory.rs` | Per-app, in-memory, capped conversation history |
| `src-tauri/src/overlay.rs` | Overlay window lifecycle: show/hide, positioning, hitbox tracking, drag state |
| `src-tauri/src/credentials.rs` | Gemini API key read/write/clear via Windows Credential Manager |
| `src-tauri/src/foreground.rs` | Foreground window title + owning process name (Win32) |
| `src-tauri/src/privacy.rs` | Denylist check run before every capture |
| `src/App.tsx` | Main window: hotkey state, debug capture view, wires capture -> query panel |
| `src/components/QueryPanel.tsx` | Question input, calls `analyze_screenshot` then `show_overlay` |
| `src/components/SettingsModal.tsx` | API key entry UI |
| `src/components/Overlay.tsx` | Overlay window content: annotation markers, draggable chat card |
| `src/types.ts` | Shared frontend types (`Annotation`, `VlmResponse`, `OverlayPayload`) |
| `tauri.conf.json` | Window declarations (including the static `overlay` window), bundle config |

## Extending the app

See the "Extending it" section in [README.md](README.md) for how to add a
new annotation shape or VLM provider.
