# Agent Bar Current State

Last updated: 2026-07-23

## Snapshot

Agent Bar has moved beyond a static concept into a first local vertical
prototype. It runs as a React application in the browser and as a Tauri Windows
desktop application with Rust and SQLite.

The current prototype is suitable for architecture demonstration, interaction
testing, and continued iteration. It is not yet a production release.

## Implemented

### Time and planning

- Today timeline with planned and observed activity.
- Seven-day schedule on a horizontally scrollable 24-hour scale.
- Daily and weekly time summaries derived from stored activity.
- Morning task inbox, deterministic conflict-free scheduling, draft review,
  and atomic confirmation.
- Persistent morning reminder with snooze and dismiss-for-today behavior.
- Screenshot-to-task extraction through a temporary read-only Codex run,
  followed by user correction before planning.

### Calendar

- ICS import with timezone conversion, recurrence expansion, conflict preview,
  and confirmed merge.
- Persistent local-file and private HTTPS ICS connections.
- Calendar connection secrets stored in Windows Credential Manager rather than
  SQLite.
- Source-isolated, atomic Monday-to-Sunday synchronization.
- Rust background scheduler that continues while the window is hidden to the
  tray and publishes Tauri update/failure events.
- Stored IANA display timezone and shared synchronization lock for background,
  manual, pause, and delete operations.

### Activity and privacy

- Opt-in Windows foreground-application tracking.
- Sanitized window titles, configurable excluded applications, retention
  cleanup, and explicit history deletion.
- Idle, locked, and disconnected-session detection that prevents false active
  heartbeats.
- Local aggregation by category and application without exposing window titles
  in weekly statistics.

### Agent observation and control

- Read-only Codex task metadata observer.
- Optional privacy-minimized Codex lifecycle hook template.
- Agent Bar-managed Codex thread/turn startup through App Server JSONL.
- User approval flow for managed commands and file changes.
- Real interrupt/termination for managed runs.
- Canvas 2D pixel office driven by normalized agent status.

### Desktop shell

- Expanded workbench and compact 68-pixel top-bar modes.
- Always-on-top positioning and tested click-through behavior.
- System tray recovery, mode switching, interaction recovery, and exit.
- SQLite-backed local state.

## Important Boundaries

- Browser mode uses mock data and cannot read desktop activity or SQLite.
- Existing Codex Desktop tasks can be observed only to the extent exposed by
  metadata or explicitly installed hooks.
- Pause/resume, priority changes, and agent handoff are not complete real
  provider controls. UI affordances must not imply otherwise.
- Calendar OAuth account connections, provider write-back, cloud sync, and
  multi-device sync are not implemented.
- Windows validation currently covers a single 1920x1080 display at 100% DPI.
  Multi-monitor, 125%/150% DPI, nonstandard taskbar positions, sleep/wake, and
  shutdown behavior remain test targets.
- The pixel office is a status visualization, not a representation of hidden
  model reasoning.

## Verification Baseline

The latest completed iteration passed:

- 21 Node tests;
- 41 regular Rust tests, with 5 environment-sensitive tests ignored;
- explicit Windows Credential Manager calendar synchronization test;
- Rust formatting check;
- Clippy with warnings treated as errors;
- Tauri debug build without bundling;
- native executable launch probe.

Rerun the commands in `AGENTS.md` on a new machine. Treat this list as historical
evidence, not a substitute for fresh verification.

## Next Iteration

Implement real pause and resume for Agent Bar-managed Codex runs.

The installed Codex App Server schema confirms the required protocol pieces:

- `turn/interrupt` can stop the active turn;
- `turn/start` requires a `threadId` and input;
- a new turn can therefore resume work on the same managed thread after a
  pause.

The implementation should add an explicit execution state rather than infer
paused state from generic approval waiting. It should also record whether an
interrupt was requested for pause or termination before sending the request, so
the completion event cannot race and produce the wrong state.

After that slice, the next product decisions are:

1. provider-neutral priority semantics;
2. same-provider and cross-provider handoff contracts;
3. direct calendar account OAuth;
4. broader Windows display and lifecycle validation.

## Documentation Map

- `README.md`: product overview and local development entry point.
- `docs/product-requirements.md`: product scope and requirements.
- `docs/architecture-sketch.md`: architecture and integration boundaries.
- `docs/research-notes.md`: comparable products and repositories.
- `docs/learning-roadmap.md`: concepts worth understanding for interviews.
- `docs/iterations/`: implementation and verification evidence by iteration.
- `docs/NEW_COMPUTER_SETUP.md`: machine migration checklist.
- `docs/NEW_COMPUTER_CODEX_PROMPT.md`: ready-to-use Codex handoff prompt.
