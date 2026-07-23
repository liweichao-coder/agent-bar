import { appendFileSync, mkdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { randomUUID } from "node:crypto";

function eventLogPath() {
  if (process.env.AGENT_BAR_EVENT_LOG) return process.env.AGENT_BAR_EVENT_LOG;
  if (process.platform === "win32") {
    return join(process.env.APPDATA ?? homedir(), "com.agentbar.desktop", "codex-events.jsonl");
  }
  if (process.platform === "darwin") {
    return join(homedir(), "Library", "Application Support", "com.agentbar.desktop", "codex-events.jsonl");
  }
  return join(process.env.XDG_DATA_HOME ?? join(homedir(), ".local", "share"), "com.agentbar.desktop", "codex-events.jsonl");
}

function safeString(value, maxLength = 128) {
  return typeof value === "string" ? value.slice(0, maxLength) : undefined;
}

let eventName;
try {
  const input = JSON.parse(readFileSync(0, "utf8") || "{}");
  eventName = safeString(input.hook_event_name, 40);
  const event = {
    id: randomUUID(),
    provider: "codex",
    eventName,
    sessionId: safeString(input.session_id),
    turnId: safeString(input.turn_id),
    agentId: safeString(input.agent_id),
    agentType: safeString(input.agent_type, 80),
    toolName: safeString(input.tool_name, 120),
    source: safeString(input.source, 80),
    permissionMode: safeString(input.permission_mode, 40),
    occurredAtMs: Date.now(),
  };

  if (event.eventName && event.sessionId) {
    const path = eventLogPath();
    mkdirSync(dirname(path), { recursive: true });
    appendFileSync(path, `${JSON.stringify(event)}\n`, { encoding: "utf8" });
  }
} catch {
  // Observation must fail open and never block the Codex lifecycle.
}

if (eventName === "Stop" || eventName === "SubagentStop") {
  process.stdout.write('{"continue":true}\n');
}
