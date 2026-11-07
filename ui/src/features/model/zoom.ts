export const MIN_ZOOM = 1;
export const MAX_ZOOM = 8;

export type Size = { width: number; height: number };
export type View = { zoom: number; x: number; y: number };

/** The image scaled to fill the box as far as its shape allows; small images are enlarged too. */
export function fitted(box: Size, image: Size): Size {
  if (image.width <= 0 || image.height <= 0) return box;
  const scale = Math.min(box.width / image.width, box.height / image.height);
  return { width: image.width * scale, height: image.height * scale };
}

/** Keeps the zoom between box size and `MAX_ZOOM`, and the offset so the image never leaves the box. */
export function clamp(view: View, box: Size, image: Size): View {
  const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, view.zoom));
  const shown = fitted(box, image);
  const limit = (content: number, frame: number) => Math.max(0, (content * zoom - frame) / 2);
  const bound = (value: number, max: number) => Math.min(max, Math.max(-max, value));
  return { zoom, x: bound(view.x, limit(shown.width, box.width)), y: bound(view.y, limit(shown.height, box.height)) };
}
