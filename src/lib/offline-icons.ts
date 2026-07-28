import type { IconifyIcon } from "@iconify/vue";

export const folderOpenIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M19 20H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h6l2 2h7a2 2 0 0 1 2 2v2h-2V8H4v10l2.4-6H22l-2.3 6.5A2 2 0 0 1 19 20M7.8 14l-1.6 4h11.9l1.4-4z"/>',
};

export const terminalIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M4 3h16a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2m0 2v14h16V5zm2 2.6L10.4 12L6 16.4L7.4 18l6-6l-6-6zM13 17h6v2h-6z"/>',
};

export const chevronLeftIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M15.4 16.6L10.8 12l4.6-4.6L14 6l-6 6l6 6z"/>',
};

export const chevronRightIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M8.6 16.6l4.6-4.6l-4.6-4.6L10 6l6 6l-6 6z"/>',
};

export const plusIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6z"/>',
};

// Left panel toggle: framed layout with the left sidebar column emphasized.
export const panelLeftIcon: IconifyIcon = {
  width: 16,
  height: 16,
  body: '<path fill="currentColor" d="M0 3a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2zm5-1v12h9a1 1 0 0 0 1-1V3a1 1 0 0 0-1-1zM4 2H2a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h2z"/>',
};

// Right panel toggle: framed layout with the right sidebar column emphasized.
export const panelRightIcon: IconifyIcon = {
  width: 16,
  height: 16,
  body: '<path fill="currentColor" d="M2 1a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V3a2 2 0 0 0-2-2zm10 1h2a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-2zm-1 0v12H2a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1z"/>',
};

// Files tool glyph for the Explorer icon strip.
export const filesIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M6 2h9l5 5v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2m8 1.5V8h4.5z"/>',
};

// Closed folder glyph for collapsed directories in the Explorer tree.
export const folderIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M10 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-8z"/>',
};

// Generic document glyph for files in the Explorer tree.
export const fileIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M13 9V3.5L18.5 9M6 2c-1.1 0-2 .9-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z"/>',
};

// "Sync to current terminal" glyph (circular refresh arrows).
export const syncIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M12 4V1L8 5l4 4V6a6 6 0 0 1 6 6c0 1-.25 1.97-.7 2.8l1.46 1.46A7.93 7.93 0 0 0 20 12a8 8 0 0 0-8-8m0 14a6 6 0 0 1-6-6c0-1 .25-1.97.7-2.8L5.24 7.74A7.93 7.93 0 0 0 4 12a8 8 0 0 0 8 8v3l4-4l-4-4z"/>',
};

// Workspace glyph for Quick Open results (2x2 layout grid).
export const workspaceIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M3 3h8v8H3zm10 0h8v8h-8zM3 13h8v8H3zm10 0h8v8h-8z"/>',
};

// Command glyph for Quick Open results (shell prompt `>_`).
export const commandIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M5.7 7.4L10.3 12l-4.6 4.6L7.1 18l6-6l-6-6zM13 16h6v2h-6z"/>',
};

// Bell glyph for the Navigator/StatusBar activity badge (a Session rang a bell).
export const bellIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body: '<path fill="currentColor" d="M21 19v1H3v-1l2-2v-6c0-3.1 2.03-5.83 5-6.71V4a2 2 0 0 1 2-2a2 2 0 0 1 2 2v.29c2.97.88 5 3.61 5 6.71v6zm-7 2a2 2 0 0 1-2 2a2 2 0 0 1-2-2z"/>',
};

// Fireworks glyph for the top-bar celebration button: a burst of sparks
// radiating from a bright center.
export const fireworksIcon: IconifyIcon = {
  width: 24,
  height: 24,
  body:
    '<g fill="currentColor"><circle cx="12" cy="12" r="1.9"/>' +
    '<circle cx="19.5" cy="12" r="1.1"/><circle cx="12" cy="4.5" r="1.1"/>' +
    '<circle cx="4.5" cy="12" r="1.1"/><circle cx="12" cy="19.5" r="1.1"/>' +
    '<circle cx="15.7" cy="8.3" r="0.85"/><circle cx="8.3" cy="8.3" r="0.85"/>' +
    '<circle cx="8.3" cy="15.7" r="0.85"/><circle cx="15.7" cy="15.7" r="0.85"/></g>',
};
