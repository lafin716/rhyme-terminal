# winmux

A Windows terminal multiplexer (Tauri v2 + Vue 3). It spawns PTY sessions via a background daemon and arranges them in a splittable, tabbed pane layout. As of the orca-shell benchmark, winmux also adopts an orca-style 3-column IDE *shell* around that pane layout — while remaining a terminal multiplexer, not an agent development environment.

## Language

### Layout

**Pane**:
A single rectangular region holding one or more tabs; the leaf of the layout tree (`kind: "leaf"`). Renders one active tab at a time.
_Avoid_: Split (a Pane is not a split), window, view.

**Split**:
An internal node of the layout tree that arranges child Panes/Splits horizontally or vertically. A Split is a container, never a content surface.
_Avoid_: Group, region.

**Shell**:
The fixed chrome framing the center pane area: top MenuBar, left Navigator, right Explorer, bottom StatusBar. The Shell is the orca-style layout borrowed in the benchmark; it is distinct from the pane tree it surrounds.
_Avoid_: Chrome, frame, layout (overloaded).

**Navigator**:
The left Shell panel. Lists Workspaces with their Sessions nested underneath for click-to-focus. Its collapsed-but-visible state is a narrow icon-rail of Workspaces.
_Avoid_: Sidebar (the DOM component is `SideBar`, but the concept is the Navigator), rail, tree.

**Explorer**:
The right Shell panel. A file tree rooted at the active Workspace's pinned root; selecting a file opens it as a FileViewer tab. Lives behind an icon strip that may later hold other tools.
_Avoid_: File browser, files pane, sidebar.

### Work units

**Workspace**:
A named, independent grouping of Sessions with its own pane layout and settings. Each Workspace has a 1-based `index` used to namespace its Sessions in the daemon (`w{index}.{name}`).
_Avoid_: Project, tab group, window.

**Session**:
A live PTY process managed by the daemon and surfaced as a terminal tab. Belongs to exactly one Workspace. Its daemon name is Workspace-prefixed; the un-prefixed form is its display name.
_Avoid_: Terminal (the UI widget is a Terminal; the underlying process is a Session), process, shell (reserved for the layout Shell), tab.

### Commands & navigation

**Palette**:
The per-terminal command menu opened inside a Session (context or radial), holding user-defined commands that are written to that Session's PTY.
_Avoid_: Command palette (reserved for Quick Open), menu.

**Quick Open**:
A global fuzzy finder over Workspaces, Sessions, files, and Palette commands. Distinct from the Palette: Quick Open searches across everything and navigates; the Palette runs a command in one Session.
_Avoid_: Command palette, search, finder, fuzzy find.

**Prefix key**:
The tmux-style leader (Ctrl+B) that arms winmux to interpret the next keystroke as an action key.
_Avoid_: Leader, hotkey, chord.
