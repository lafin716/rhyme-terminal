import type { IconifyIcon } from "@iconify/vue";
import type { AgentKind } from "./tauri";
import { claudeIcon, codexIcon, terminalIcon } from "./offline-icons";

export function sessionAgentIcon(agent: AgentKind): IconifyIcon {
  switch (agent) {
    case "claude":
      return claudeIcon;
    case "codex":
      return codexIcon;
    case "terminal":
      return terminalIcon;
  }
}
