import { useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Annotation, OverlayPayload } from "../types";
import "./Overlay.css";

const EDGE_MARGIN = 16;
const GAP_BELOW_ANNOTATION = 16;

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

/// Tells the Rust-side cursor-poll loop where the card currently is, in the
/// webview's own logical/CSS pixels. Click-through means the webview never
/// sees mousemove/mouseenter while it's active, so hover can't be detected
/// with ordinary DOM events -- the backend polls the real OS cursor
/// position against this rect instead (see overlay::set_overlay_hitbox).
function reportHitbox(rect: { left: number; top: number; width: number; height: number }) {
  void invoke("set_overlay_hitbox", { x: rect.left, y: rect.top, width: rect.width, height: rect.height });
}

/// Places the card just below the first annotation (so it reads as "about
/// that highlighted thing") when there is one, falling back to
/// bottom-center otherwise -- then clamps to the viewport so it's never
/// partly off-screen.
function defaultCardPosition(
  annotations: Annotation[],
  cardWidth: number,
  cardHeight: number,
): { left: number; top: number } {
  const first = annotations[0];
  let left: number;
  let top: number;

  if (first) {
    left = first.x * window.innerWidth;
    top = (first.y + (first.height ?? 0)) * window.innerHeight + GAP_BELOW_ANNOTATION;
  } else {
    left = (window.innerWidth - cardWidth) / 2;
    top = window.innerHeight - cardHeight - 48;
  }

  return {
    left: clamp(left, EDGE_MARGIN, window.innerWidth - cardWidth - EDGE_MARGIN),
    top: clamp(top, EDGE_MARGIN, window.innerHeight - cardHeight - EDGE_MARGIN),
  };
}

function AnnotationMarker({ annotation }: { annotation: Annotation }) {
  const left = `${annotation.x * 100}%`;
  const top = `${annotation.y * 100}%`;
  const width = `${(annotation.width ?? 0.02) * 100}%`;
  const height = `${(annotation.height ?? 0.02) * 100}%`;

  if (annotation.type === "underline") {
    return <div className="annotation annotation-underline" style={{ left, top: `calc(${top} + ${height})`, width }} />;
  }

  const className = annotation.type === "arrow" ? "annotation annotation-arrow" : "annotation annotation-ring";
  if (annotation.type === "arrow" || annotation.type === "ring") {
    return <div className={className} style={{ left, top, width, height }} />;
  }

  return <div className="annotation annotation-box" style={{ left, top, width, height }} />;
}

function Overlay() {
  const [payload, setPayload] = useState<OverlayPayload | null>(null);
  const [cardPos, setCardPos] = useState({ left: 0, top: 0 });
  const cardRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ pointerId: number; startX: number; startY: number; origLeft: number; origTop: number } | null>(null);

  useLayoutEffect(() => {
    invoke<OverlayPayload | null>("get_overlay_payload").then((current) => {
      if (current) {
        setPayload(current);
      }
    });

    const unlisten = listen<OverlayPayload | null>("overlay-response", (event) => {
      setPayload(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Recomputes the default position whenever a new answer comes in, using
  // the card's actual rendered size once it exists so a long answer (taller
  // card) still clamps correctly against the bottom edge -- then reports
  // the resulting rect so the card is immediately hoverable/clickable.
  useLayoutEffect(() => {
    if (!payload) {
      return;
    }
    const card = cardRef.current;
    const width = card?.offsetWidth ?? Math.min(window.innerWidth * 0.9, 480);
    const height = card?.offsetHeight ?? 140;
    const pos = defaultCardPosition(payload.response.annotations, width, height);
    setCardPos(pos);
    reportHitbox({ ...pos, width, height });
  }, [payload]);

  function startDrag(event: React.PointerEvent) {
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      origLeft: cardPos.left,
      origTop: cardPos.top,
    };
    void invoke("begin_overlay_drag");
    (event.target as HTMLElement).setPointerCapture(event.pointerId);
  }

  function onDrag(event: React.PointerEvent) {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) {
      return;
    }
    const card = cardRef.current;
    const width = card?.offsetWidth ?? 0;
    const height = card?.offsetHeight ?? 0;
    const left = clamp(drag.origLeft + (event.clientX - drag.startX), 0, Math.max(0, window.innerWidth - width));
    const top = clamp(drag.origTop + (event.clientY - drag.startY), 0, Math.max(0, window.innerHeight - height));
    setCardPos({ left, top });
  }

  function endDrag(event: React.PointerEvent) {
    if (dragRef.current?.pointerId !== event.pointerId) {
      return;
    }
    dragRef.current = null;

    const card = cardRef.current;
    if (card) {
      reportHitbox({ left: cardPos.left, top: cardPos.top, width: card.offsetWidth, height: card.offsetHeight });
    }
    // Hand control back to the poll loop only after the drop position's
    // hitbox has been reported, so it doesn't briefly evaluate against a
    // stale (pre-drag) rect.
    void invoke("end_overlay_drag");
  }

  if (!payload) {
    return null;
  }

  const { query, response } = payload;

  function close() {
    setPayload(null);
    void invoke("clear_overlay_hitbox");
    void invoke("hide_overlay");
  }

  return (
    <div className="overlay-root">
      {response.annotations.map((annotation, index) => (
        <AnnotationMarker key={index} annotation={annotation} />
      ))}

      <div ref={cardRef} className="chat-card" style={{ left: cardPos.left, top: cardPos.top }}>
        <div
          className="chat-card-header"
          onPointerDown={startDrag}
          onPointerMove={onDrag}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
        >
          <strong>{query}</strong>
          <button type="button" onClick={close} aria-label="Close">
            &times;
          </button>
        </div>
        <p className="chat-card-text">{response.text}</p>
      </div>
    </div>
  );
}

export default Overlay;
