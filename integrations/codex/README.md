# Codex lifecycle adapter

This directory contains an opt-in Codex Hooks template. It is intentionally not installed into `.codex/hooks.json` automatically.

The hook writes only these fields to the local Agent Bar event log:

- lifecycle event name
- session, turn, tool-call and subagent identifiers
- tool name, subagent type, start source and permission mode
- local timestamp

It never writes prompts, tool arguments, tool results, assistant messages, transcript contents or reasoning text.

To exercise the adapter without installing hooks, pipe a documented hook-shaped JSON object to the script. Set `AGENT_BAR_EVENT_LOG` to use an isolated test file.

The checked-in `hooks.json` is a reviewable template for a later one-click installer. Installation must remain an explicit user action because Codex reviews and trusts project-local command hooks separately.
