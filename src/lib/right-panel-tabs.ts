/**
 * Identifiers reserved for tools that share the right-side panel.
 *
 * Git and Tasks remain intentionally unavailable until their panels exist, so
 * the UI never advertises controls that cannot yet do useful work.
 */
export type RightPanelTabId = "files" | "git" | "tasks";

export const availableRightPanelTabs: readonly RightPanelTabId[] = ["files"];

/**
 * Keep the visible panel valid as tools are added or temporarily unavailable.
 */
export function resolveRightPanelTab(requested?: RightPanelTabId): RightPanelTabId {
  if (requested && availableRightPanelTabs.includes(requested)) return requested;
  return availableRightPanelTabs[0];
}
