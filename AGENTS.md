# Agent Bar Repository Guide

This file is the durable handoff contract for Codex and other coding agents
working in this repository. Its instructions apply only to this repository and
its subdirectories; they are not global instructions for unrelated projects.

## Product Direction

Agent Bar is a local-first Windows desktop application that combines:

- daily and weekly scheduling;
- foreground-application time tracking and review;
- morning planning and calendar ingestion;
- observable, controllable AI-agent work;
- a compact desktop bar and a simple pixel-office visualization.

The product should help a person understand both where their own time went and
what delegated agents are doing. It is a general productivity tool, not a
single-purpose admissions or study-planning application.

## Current Technical Baseline

- Desktop shell: Tauri 2
- Frontend: React, TypeScript, Vite
- Native backend: Rust
- Local persistence: SQLite through Rust
- Timeline and work UI: React DOM and SVG
- Pixel office: Canvas 2D
- Initial agent provider: Codex App Server and optional lifecycle hooks
- Calendar baseline: ICS import and persistent local/private ICS connections

Read `docs/CURRENT_STATE.md` before planning implementation. Read the relevant
file in `docs/iterations/` before modifying an existing subsystem.

## Working Rules

1. Keep the application local-first and privacy-minimizing.
2. Never store raw window titles before sanitization.
3. Never persist agent prompts, reasoning, tool inputs, tool outputs, calendar
   subscription secrets, or imported screenshots unless a documented feature
   explicitly requires it.
4. Show only controls that the connected provider can genuinely perform. Do
   not present simulated pause, resume, terminate, approval, priority, or
   handoff controls as real provider capabilities.
5. Keep deterministic scheduling constraints outside the language model.
   Agent suggestions require user confirmation before changing the schedule.
6. Preserve source ownership when syncing calendars. A connection may replace
   only blocks created by that same connection.
7. Prefer existing module boundaries and focused changes over broad refactors.
8. Update `docs/CURRENT_STATE.md` and add an iteration note when a meaningful
   vertical slice is completed.
9. Do not commit local databases, credentials, screenshots, build artifacts,
   logs, or generated dependency directories.
10. Do not rewrite Git history or fabricate dates. Project evidence should
    reflect real implementation and verification work.

## Validation

Run the smallest relevant checks while iterating and the full baseline before
publishing a meaningful change:

```powershell
npm install
npm test
npm run build
& "$env:USERPROFILE\.cargo\bin\cargo.exe" fmt --manifest-path src-tauri/Cargo.toml -- --check
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --manifest-path src-tauri/Cargo.toml
& "$env:USERPROFILE\.cargo\bin\cargo.exe" clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build -- --debug --no-bundle
```

Some tests touching Windows Credential Manager are intentionally ignored during
the regular Rust suite and must be run explicitly when that subsystem changes.

## Near-Term Priority

The next planned vertical slice is real lifecycle control for Agent Bar-managed
Codex runs:

- distinguish `running`, `pause-requested`, `paused`, `stop-requested`,
  `completed`, and `failed`;
- implement pause through `turn/interrupt`;
- resume on the same thread with a new `turn/start`;
- remove the pause/stop completion race;
- retain clear capability reporting and focused protocol tests.

Priority changes and cross-agent handoff should remain unavailable until their
provider semantics are designed and implemented.
