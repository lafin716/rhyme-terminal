export const TITLEBAR_HEIGHT = 36;
export const WINDOW_CONTROLS_WIDTH = 172;
export const COMPACT_MENU_WIDTH = 102;

// Reserve intersections, including when several narrow split panes meet the controls.
export function titlebarInsets(
  rect: { top: number; left: number; right: number; width: number },
  viewportWidth: number,
  menuWidth: number,
) {
  if (rect.top >= TITLEBAR_HEIGHT) return { left: 0, right: 0 };
  const left = Math.min(rect.width, Math.max(0, menuWidth - rect.left));
  const right = Math.min(rect.width - left, Math.max(0, rect.right - (viewportWidth - WINDOW_CONTROLS_WIDTH)));
  return { left, right };
}
