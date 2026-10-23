// Pure geometry for the image viewer: fit/zoom/pan math kept out of the
// component so it can be unit-tested. The image is drawn centred in the
// viewport, then shifted by `offset`, rotated, and scaled.

export interface Size {
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

export const MIN_SCALE = 0.05;
export const MAX_SCALE = 32;

/** Zoom presets the +/- buttons and keys step through. */
const ZOOM_STEPS = [0.05, 0.1, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32];

/** Rotation normalised to 0, 90, 180 or 270 degrees. */
export function normalizeRotation(degrees: number): number {
  return (((Math.round(degrees / 90) * 90) % 360) + 360) % 360;
}

/** The on-screen footprint of the image before scaling; 90/270 swap the axes. */
export function rotatedSize(natural: Size, rotation: number): Size {
  const quarter = normalizeRotation(rotation) % 180 !== 0;
  return quarter ? { width: natural.height, height: natural.width } : { ...natural };
}

export function clampScale(scale: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
}

/**
 * Scale that fits the whole image inside the viewport. Small images are not
 * blown up past their natural size.
 */
export function fitScale(natural: Size, viewport: Size, rotation = 0): number {
  const size = rotatedSize(natural, rotation);
  if (size.width <= 0 || size.height <= 0 || viewport.width <= 0 || viewport.height <= 0) return 1;
  return clampScale(Math.min(1, viewport.width / size.width, viewport.height / size.height));
}

/** Next preset above (`direction` 1) or below (-1) the current scale. */
export function stepScale(scale: number, direction: 1 | -1): number {
  const epsilon = 1e-6;
  if (direction > 0) return ZOOM_STEPS.find((step) => step > scale + epsilon) ?? MAX_SCALE;
  for (let i = ZOOM_STEPS.length - 1; i >= 0; i--) {
    if (ZOOM_STEPS[i] < scale - epsilon) return ZOOM_STEPS[i];
  }
  return MIN_SCALE;
}

/**
 * Offset that keeps the image point under `anchor` fixed while the scale
 * changes from `from` to `to`. `anchor` is relative to the viewport centre.
 */
export function zoomAround(offset: Point, anchor: Point, from: number, to: number): Point {
  const k = to / from;
  return {
    x: anchor.x - k * (anchor.x - offset.x),
    y: anchor.y - k * (anchor.y - offset.y),
  };
}

/**
 * Keeps the image from being dragged out of view: an axis that fits inside the
 * viewport stays centred, a larger one can pan only until its edge meets the
 * viewport edge.
 */
export function clampOffset(offset: Point, natural: Size, viewport: Size, scale: number, rotation = 0): Point {
  const size = rotatedSize(natural, rotation);
  const maxX = Math.max(0, (size.width * scale - viewport.width) / 2);
  const maxY = Math.max(0, (size.height * scale - viewport.height) / 2);
  return {
    x: Math.min(maxX, Math.max(-maxX, offset.x)) || 0,
    y: Math.min(maxY, Math.max(-maxY, offset.y)) || 0,
  };
}
