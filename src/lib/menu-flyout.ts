/** Returns the submenu that should be visible after selecting a first-depth menu. */
export function toggleFlyoutGroup(currentId: string | null, selectedId: string): string | null {
  return currentId === selectedId ? null : selectedId;
}
