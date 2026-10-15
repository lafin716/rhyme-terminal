# FRONTEND KNOWLEDGE BASE

**Path:** `src/`
**Stack:** Vue 3, TypeScript, Vite, xterm.js

## OVERVIEW
Vue 3 SPA rendered inside Tauri WebView2. Manages a workspace-based tabbed terminal layout with tmux-style keybindings.

## STRUCTURE
```
src/
├── main.ts               # App bootstrap (createApp)
├── App.vue               # Root shell: MenuBar, SideBar, SplitContainer, StatusBar, modals
├── components/           # Vue SFCs
├── composables/          # Reactive state modules
├── lib/                  # Shared TS utilities
└── assets/               # Static images
```

## WHERE TO LOOK
| Task | Location |
|------|----------|
| New terminal UI | `components/Terminal.vue` |
| Pane layout rendering | `components/SplitContainer.vue` |
| Tab bar inside pane | `components/PaneTabs.vue` |
| Sidebar / workspace list | `components/SideBar.vue` |
| Command palette | `components/PalettePopover.vue` |
| Session lifecycle | `composables/useSessions.ts` |
| Workspace state | `composables/useWorkspaces.ts` |
| Layout tree ops | `composables/useLayout.ts` |
| Focus tracking | `composables/useFocus.ts` |
| Keybindings | `composables/useKeybindings.ts` |
| Global shortcuts | `composables/useGlobalShortcuts.ts` |
| Drag-and-drop state | `composables/useDragState.ts` |
| Settings modal | `composables/useSettings.ts` |
| Confirm modal | `composables/useConfirm.ts` |
| Palette logic | `composables/usePalette.ts` |
| Preferences | `composables/usePrefs.ts` |
| Tauri API wrapper | `lib/tauri.ts` |
| Layout types | `lib/layout-types.ts` |
| Keybinding registry | `lib/keybindings.ts` |
| Persistence helpers | `lib/persistence.ts` |

## CONVENTIONS
- Every state module is a composable exporting reactive objects and functions
- Components read state directly from composables (no props drilling for global state)
- `App.vue` registers all action handlers and wires keybindings on mount
- Register every app command shortcut in `src/lib/keybindings.ts` and read its current binding through `useKeybindings`; Settings > Keybindings must support viewing, remapping, clearing, and resetting it. This includes terminal/editor commands, zoom wheel gestures, session index selection, and prefix combinations. Do not add hidden hard-coded app shortcuts.
- Session IDs are strings (UUIDs from backend); layout nodes use `nodeId("leaf")` / `nodeId("split")`
- **Strict TypeScript:** `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch` are enabled
- **Persistence:** localStorage keys use `winmux:{domain}:v1` format with `{ version: 1, ... }` envelope (see `lib/persistence.ts`)

## ANTI-PATTERNS
- Do NOT introduce Pinia or Vuex — composables are the state layer
- Do NOT mutate layout nodes outside `useLayout.ts` helpers (tree must stay consistent)
- Do NOT call `api.writeSession` with raw strings — encode with `stringToBase64` first
- Do NOT forget to `unlisten` Tauri event listeners in `onBeforeUnmount`
