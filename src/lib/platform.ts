export type DesktopPlatform = "windows" | "macos" | "other";

function detectPlatform(): DesktopPlatform {
  const value = `${navigator.platform} ${navigator.userAgent}`.toLowerCase();
  if (value.includes("mac")) return "macos";
  if (value.includes("win")) return "windows";
  return "other";
}

export const desktopPlatform = detectPlatform();
export const isMacOS = desktopPlatform === "macos";
export const isWindows = desktopPlatform === "windows";

export function hasPrimaryModifier(event: Pick<KeyboardEvent, "ctrlKey" | "metaKey">): boolean {
  return isMacOS ? event.metaKey : event.ctrlKey;
}
