/** Separate geometry from pointer UI so touch/drag cancellation can be tested. */
export const GLOSS_HOLD_MS = 560;
export const GLOSS_DRAG_SLOP_PX = 8;

export function hasMovedBeyondGlossSlop(
  originX: number,
  originY: number,
  x: number,
  y: number,
): boolean {
  const dx = x - originX;
  const dy = y - originY;
  return dx * dx + dy * dy > GLOSS_DRAG_SLOP_PX * GLOSS_DRAG_SLOP_PX;
}
