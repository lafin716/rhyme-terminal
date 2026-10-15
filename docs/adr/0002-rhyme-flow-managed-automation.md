# Add Rhyme Flow as a managed automation surface

Date: 2026-09-07
Status: Accepted for the explicit Rhyme Flow implementation request

This extends ADR-0001's terminal-only product boundary. The terminal pane tree,
manual PTYs, account profiles, shortcuts, and existing localStorage remain intact.
Rhyme Flow is a separate mode in the existing Vue/Tauri application.

The authoritative domain model and validator live in `src-tauri/src/flow/model.rs`.
The Vue editor and natural-language designer submit the same definition. Confirmed
worker and flow revisions are append-only; optimistic revision checks reject stale
edits. A shared JSON fixture checks Rust/TypeScript serialization.

SQLite is stored at the existing Tauri local-data directory under
`flow/flow.sqlite3`. It uses WAL, a five-second busy timeout, transactional schema
migration and a Windows exclusive engine ownership file. Definitions, messages,
requirement revisions, jobs, attempts, approvals, events and artifacts are persisted.
No existing settings or authentication files are migrated or overwritten.

The engine runs on Tokio in the GUI process, independently of component mounting.
Closing the window retains the existing tray behavior. Exiting or crashing the app
closes Windows kill-on-close Job Objects, including descendant processes. On next
startup unfinished runs/attempts/jobs become interrupted. They are never replayed
automatically. Recovery starts a new run against the pinned flow and latest task
requirements. Additional instructions apply to that new run, not an in-flight turn.

Normal edges are a DAG; conditional ports are explicit. Parallel independent nodes
are bounded to eight. Repeat bodies are serial, forbid nesting and approval/delivery
actions, and have 1–10 attempts. Every pass starts with fresh results and verification
evidence. A failed body step ends that pass; the next pass starts from its first step.

Managed Codex uses the installed CLI's JSON event stream. This is distinct from
terminal process detection and manual profile sessions. No other managed agent or
WSL runner is advertised. Existing account directory references are reused; secret
values and environment snapshots are not copied into Run settings.

Development is a package of ordinary workers and nodes. Its isolated worktree starts
from HEAD and leaves parent modifications alone. Delivery flows require serial
workspace access. Verification and review refer to the same complete change
fingerprint; truncated Git evidence is rejected. Commit requires explicit relative
files and an empty index. Draft PR requires a clean verified commit, a run-level
external_publish grant and a separate approval showing the remote, branch and commit.
Ambiguous publication is reconciled read-only with `gh pr list` before another run.

The command capability means trusted host command execution with the current user's
filesystem and network authority. It is not an OS sandbox or a security boundary
against hostile executable code. Imported commands must be inspected before granting
it. Program capability gates protect managed actions; Codex additionally uses its
read-only/workspace-write sandbox. No bypass-sandbox option is supplied.

Flow database errors disable Flow with a recovery message, rather than preventing
the existing terminal from starting. The initial schema intentionally supports a
documented JSON Schema subset and rejects unsupported contract constraints.
