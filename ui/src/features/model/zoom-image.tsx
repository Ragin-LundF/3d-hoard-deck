import { useEffect, useRef, useState, type PointerEvent } from "react";
import { t } from "../../shared/i18n";
import { clamp, fitted, MAX_ZOOM, MIN_ZOOM, wheelFactor, zoomAt, type Size, type View } from "./zoom";

const STEP = 1.5;
const FIT: View = { zoom: 1, x: 0, y: 0 };

/** An image that fills its box; +/-, the wheel or a pinch zoom in, dragging moves a zoomed image. */
export function ZoomImage({ src, alt }: { src: string; alt: string }) {
  const frame = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState<Size>({ width: 0, height: 0 });
  // Remembered with its source, so a new image is not drawn with the previous one's proportions.
  const [image, setImage] = useState<Size & { src: string }>({ width: 0, height: 0, src: "" });
  const [view, setView] = useState<View>(FIT);
  const [drag, setDrag] = useState<{ x: number; y: number } | null>(null);
  const loaded = image.src === src;
  const size: Size = loaded ? image : { width: 0, height: 0 };

  useEffect(() => setView(FIT), [src]);
  useEffect(() => {
    const element = frame.current;
    if (!element) return;
    const observer = new ResizeObserver(() => setBox({ width: element.clientWidth, height: element.clientHeight }));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  // A smaller window leaves less room to move; the offset follows.
  useEffect(() => setView((current) => clamp(current, box, size)), [box, size.width, size.height]);

  // Not a React handler: wheel listeners must be non-passive to stop the page from scrolling or zooming.
  useEffect(() => {
    const element = frame.current;
    if (!element) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      // WebKit may report a pinch both ways; the gesture events win then.
      if (event.ctrlKey && gesturing) return;
      const rect = element.getBoundingClientRect();
      const at = { x: event.clientX - rect.left - rect.width / 2, y: event.clientY - rect.top - rect.height / 2 };
      const factor = wheelFactor(event.deltaY, event.ctrlKey);
      setView((current) => clamp(zoomAt(current, factor, at), { width: rect.width, height: rect.height }, size));
    };
    let gesturing = false;
    // Safari's WebKit (the macOS app) reports a trackpad pinch as gesture events with a running scale.
    let previous = 1;
    const onGestureStart = (event: Event) => {
      event.preventDefault();
      previous = 1;
      gesturing = true;
    };
    const onGestureEnd = () => {
      gesturing = false;
    };
    const onGestureChange = (event: Event) => {
      event.preventDefault();
      const gesture = event as Event & { scale: number; clientX: number; clientY: number };
      const rect = element.getBoundingClientRect();
      const at = { x: gesture.clientX - rect.left - rect.width / 2, y: gesture.clientY - rect.top - rect.height / 2 };
      const factor = gesture.scale / previous;
      previous = gesture.scale;
      setView((current) => clamp(zoomAt(current, factor, at), { width: rect.width, height: rect.height }, size));
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    element.addEventListener("gesturestart", onGestureStart);
    element.addEventListener("gesturechange", onGestureChange);
    element.addEventListener("gestureend", onGestureEnd);
    return () => {
      element.removeEventListener("gestureend", onGestureEnd);
      element.removeEventListener("wheel", onWheel);
      element.removeEventListener("gesturestart", onGestureStart);
      element.removeEventListener("gesturechange", onGestureChange);
    };
  }, [size.width, size.height]);

  const change = (next: View) => setView(clamp(next, box, size));
  const zoomBy = (factor: number) => change(zoomAt(view, factor, { x: 0, y: 0 }));
  const onPointerDown = (event: PointerEvent) => {
    // The zoom buttons sit inside the frame; capturing their pointer would swallow the click.
    if (view.zoom <= MIN_ZOOM || (event.target as HTMLElement).closest("button")) return;
    setDrag({ x: event.clientX - view.x, y: event.clientY - view.y });
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const onPointerMove = (event: PointerEvent) => {
    if (drag) change({ ...view, x: event.clientX - drag.x, y: event.clientY - drag.y });
  };
  const shown = fitted(box, size);

  return (
    <div
      ref={frame}
      className={`zoom${view.zoom > MIN_ZOOM ? " zoomed" : ""}${drag ? " dragging" : ""}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={() => setDrag(null)}
      onPointerCancel={() => setDrag(null)}
    >
      <img
        src={src}
        alt={alt}
        draggable={false}
        onLoad={(event) => setImage({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight, src })}
        style={{
          visibility: loaded ? "visible" : "hidden",
          width: shown.width || undefined,
          height: shown.height || undefined,
          transform: `translate(${view.x}px, ${view.y}px) scale(${view.zoom})`,
        }}
      />
      <div className="vgroup zoom-controls" role="group" aria-label={t("Zoom")}>
        <button onClick={() => zoomBy(1 / STEP)} disabled={view.zoom <= MIN_ZOOM} aria-label={t("Zoom out")} title={t("Zoom out")}>−</button>
        <button onClick={() => zoomBy(STEP)} disabled={view.zoom >= MAX_ZOOM} aria-label={t("Zoom in")} title={t("Zoom in")}>+</button>
        <button onClick={() => setView(FIT)} disabled={view.zoom <= MIN_ZOOM} title={t("Fit to the box")}>{t("Fit")}</button>
      </div>
    </div>
  );
}
