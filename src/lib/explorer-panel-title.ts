/** Return the current Explorer heading, falling back before a root is available. */
export function explorerPanelTitle(rootName: string): string {
  return rootName || "Explorer";
}
