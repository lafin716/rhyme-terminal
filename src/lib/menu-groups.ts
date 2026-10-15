export interface MenuGroup<T> {
  id: string;
  label: string;
  items: T[];
}

/**
 * Converts the former top-level menu definitions into labelled sections for
 * the compact overflow dropdown without altering their item order.
 */
export function groupMenuItems<T>(menus: readonly MenuGroup<T>[]): MenuGroup<T>[] {
  return menus.map((menu) => ({
    id: menu.id,
    label: menu.label,
    items: [...menu.items],
  }));
}
