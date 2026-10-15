import type { AgentTaskStatus } from "./tauri";

export function sessionIndicatorClass(status?: AgentTaskStatus): string {
  return status ? `is-${status}` : "is-idle";
}
