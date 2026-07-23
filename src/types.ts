export type ScheduleStatus = "planned" | "active" | "done" | "skipped";
export type AgentStatus = "working" | "searching" | "waiting" | "recent" | "idle" | "blocked";

export type ScheduleBlock = {
  id: string;
  title: string;
  startMinute: number;
  endMinute: number;
  category: "focus" | "meeting" | "life" | "admin";
  source: "manual" | "calendar" | "agent" | `calendar:${string}`;
  status: ScheduleStatus;
  locked?: boolean;
};

export type ScheduleDay = {
  day: string;
  blocks: ScheduleBlock[];
};

export type ActivityBlock = {
  id: string;
  appName: string;
  windowTitle: string;
  startMinute: number;
  endMinute: number;
  category: ScheduleBlock["category"];
};

export type Agent = {
  id: string;
  name: string;
  provider: string;
  task: string;
  detail: string;
  status: AgentStatus;
  elapsedMinutes: number;
  accent: string;
  position: { x: number; y: number };
  capabilities: Array<"pause" | "stop" | "handoff" | "approve">;
  controlMode?: "managed" | "observed" | "mock";
  updatedAtMs?: number;
};

export type ApprovalRequest = {
  id: string;
  agentId?: string;
  kind: "activity-capture" | "codex-observe" | "agent-tool";
  title: string;
  detail: string;
  risk: "low" | "medium" | "high";
};

export type PlannerSuggestion = {
  id: string;
  title: string;
  reason: string;
  startMinute: number;
  endMinute: number;
  category: ScheduleBlock["category"];
};
