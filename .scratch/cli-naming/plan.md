# Rhyme Terminal naming and live mobile server

- Preserve the existing headless server, explicit mobile approval, session protocol, data paths and running PTY sessions.
- Rename the command to `rhyme-terminal-cli` and the dedicated daemon to `rhyme-terminal-daemon`; update source filenames, build targets, help, current documentation and executable integration tests together.
- Let the renamed CLI use its own embedded daemon so a standalone CLI build can serve mobile clients without an additional executable. Retain legacy sidecar lookup for old CLI executable names.
- Verify daemon selection and the existing mobile command/lifecycle/process regressions before starting a persistent server.
- Move the complete working tree to `/Users/lafin/workspace/rhyme-terminal` without losing uncommitted changes. Rebuild there to validate relocated paths.
- Start `rhyme-terminal-cli serve` in its own tmux session on the machine's Tailscale IPv4 address. Check HTTP and WebSocket connectivity and issue a fresh invitation at delivery time. Keep manual device approval and avoid public internet tunneling.
