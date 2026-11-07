import { useEffect, useRef, useState, type PointerEvent, type WheelEvent } from "react";
import { t } from "../../shared/i18n";
import { clamp, fitted, MAX_ZOOM, MIN_ZOOM, type Size, type View } from "./zoom";

const STEP = 1.5;
const FIT: View = { zoom: 1, x: 0, y: 0 };

/** An image that fills its box; +/- or the wheel zoom in, dragging moves a zoomed image. */
export function ZoomImage({ src, alt }: { src: string; alt: string }) {
  const frame = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState<Size>({ width: 0, height: 0 });
  const [image, setImage] = useState<Size>({ width: 0, height: 0 });
  const [view, setView] = useState<View>(FIT);
  const [drag, setDrag] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => setView(FIT), [src]);
  useEffect(() => {
    const element = frame.current;
    if (!element) return;
    const observer = new ResizeObserver(() => setBox({ width: element.clientWidth, height: element.clientHeight }));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const change = (next: View) => setView(clamp(next, box, image));
  const zoomBy = (factor: number) => change({ ...view, zoom: view.zoom * factor, x: view.x * factor, y: view.y * factor });
  const onWheel = (event: WheelEvent) => zoomBy(event.deltaY < 0 ? 1.15 : 1 / 1.15);
  const onPointerDown = (event: PointerEvent) => {
    // The zoom buttons sit inside the frame; capturing their pointer would swallow the click.
    if (view.zoom <= MIN_ZOOM || (event.target as HTMLElement).closest("button")) return;
    setDrag({ x: event.clientX - view.x, y: event.clientY - view.y });
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const onPointerMove = (event: PointerEvent) => {
    if (drag) change({ ...view, x: event.clientX - drag.x, y: event.clientY - drag.y });
  };
  const shown = fitted(box, image);

  return (
    <div
      ref={frame}
      className={`zoom${view.zoom > MIN_ZOOM ? " zoomed" : ""}${drag ? " dragging" : ""}`}
      onWheel={onWheel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={() => setDrag(null)}
      onPointerCancel={() => setDrag(null)}
    >
      <img
        src={src}
        alt={alt}
        draggable={false}
        onLoad={(event) => setImage({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })}
        style={{
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
