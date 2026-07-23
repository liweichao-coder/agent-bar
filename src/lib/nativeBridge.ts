import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { ActivityBlock, Agent, ApprovalRequest, ScheduleBlock, ScheduleDay } from "../types";
import type { MorningPlanTask } from "./morningPlanner";
import type { MorningReminderState } from "./morningReminder";
import { localWeekRanges, type ActivityDaySummary } from "./timeStats";

type StoredActivityRecord = {
  id: string;
  startedAtMs: number;
  endedAtMs: number;
  appName: string;
  sanitizedWindowTitle?: string;
  category: string;
  source: string;
};

export type NativeLocalState = {
  scheduleBlocks: ScheduleBlock[];
  plannerTasks: MorningPlanTask[];
  calendarConnections: NativeCalendarConnection[];
  morningPrompt: MorningReminderState;
  activityRecords: StoredActivityRecord[];
  trackingEnabled: boolean;
  captureWindowTitles: boolean;
  codexObservationEnabled: boolean;
  excludedActivityApps: string[];
  activityRetentionDays: number;
  activityIdleThresholdMinutes: number;
  windowMode: "compact" | "expanded";
  storageKind: "sqlite";
};

export type NativeActivityPrivacyUpdate = {
  excludedActivityApps: string[];
  activityRetentionDays: number;
  deletedRecords: number;
};

export type NativeActivityCaptureState = {
  status: "active" | "idle" | "locked" | "disconnected" | "paused" | "unavailable";
  idleSeconds: number;
  thresholdMinutes: number;
  captureAllowed: boolean;
  sessionStateAvailable: boolean;
  checkedAtMs: number;
};

export type NativeWindowModeSnapshot = {
  mode: "compact" | "expanded";
  width: number;
  height: number;
  x: number;
  y: number;
  alwaysOnTop: boolean;
};

export type NativeCalendarImportEvent = {
  id: string;
  title: string;
  startMinute: number;
  endMinute: number;
  allDay: boolean;
  recurring: boolean;
};

export type NativeCalendarImportPreview = {
  sourceName: string;
  events: NativeCalendarImportEvent[];
  skippedCount: number;
  warnings: string[];
};

export type NativeCalendarConnection = {
  id: string;
  displayName: string;
  kind: "local-file" | "ics-subscription";
  sourceHint: string;
  enabled: boolean;
  refreshMinutes: number;
  lastSyncAtMs?: number;
  lastSyncStatus: "never" | "success" | "error";
  lastError?: string;
  createdAtMs: number;
  updatedAtMs: number;
};

export type NativeCalendarSyncBatch = {
  connections: NativeCalendarConnection[];
  scheduleBlocks: ScheduleBlock[];
  scheduleDays: NativeScheduleDay[];
  syncedCount: number;
  failedCount: number;
  warnings: string[];
};

export type NativeScheduleDay = ScheduleDay;

export type NativeCodexSnapshot = {
  connectionState: "disabled" | "connected" | "error";
  adapterMode: "app-server-observer";
  agents: Agent[];
  lastSyncedAtMs?: number;
  message: string;
};

type NativeManagedCodexRun = Omit<Agent, "id"> & {
  threadId: string;
  turnId?: string;
  startedAtMs: number;
};

export type NativeManagedCodexSnapshot = {
  connectionState: "disconnected" | "connected";
  runs: NativeManagedCodexRun[];
  approvals: ApprovalRequest[];
  screenshotImport: NativeScreenshotImportSnapshot;
  lastError?: string;
};

export type NativeScreenshotImportSnapshot = {
  status: "idle" | "analyzing" | "ready" | "error";
  fileName?: string;
  tasks: MorningPlanTask[];
  warnings: string[];
  error?: string;
};

export const nativeRuntimeAvailable = isTauri();

export function localDayKey(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function localDayRange(date: Date) {
  const start = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const end = new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1);
  return { startMs: start.getTime(), endMs: end.getTime() };
}

export async function loadNativeLocalState(date: Date) {
  const range = localDayRange(date);
  return invoke<NativeLocalState>("load_local_state", {
    day: localDayKey(date),
    dayStartMs: range.startMs,
    dayEndMs: range.endMs,
  });
}

export async function persistNativeSchedule(date: Date, blocks: ScheduleBlock[]) {
  await invoke("replace_schedule_blocks", {
    day: localDayKey(date),
    blocks,
  });
}

export async function persistNativePlannerTasks(tasks: MorningPlanTask[]) {
  await invoke("replace_planner_tasks", { tasks });
}

export async function applyNativeMorningPlan(
  date: Date,
  blocks: ScheduleBlock[],
  tasks: MorningPlanTask[],
) {
  await invoke("apply_morning_plan", {
    day: localDayKey(date),
    blocks,
    tasks,
  });
}

export async function persistMorningPromptSettings(enabled: boolean, promptMinute: number) {
  await invoke("set_morning_prompt_settings", { enabled, promptMinute });
}

export async function snoozeNativeMorningPrompt(untilMs: number) {
  await invoke("snooze_morning_prompt", { untilMs });
}

export async function dismissNativeMorningPrompt(day: string) {
  await invoke("dismiss_morning_prompt", { day });
}

export async function previewNativeCalendarImport(icsText: string, date: Date) {
  return invoke<NativeCalendarImportPreview>("preview_calendar_import", {
    icsText,
    day: localDayKey(date),
    viewerTimezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  });
}

export async function pickNativeCalendarFile() {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "iCalendar", extensions: ["ics"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function createNativeCalendarConnection(
  displayName: string,
  kind: NativeCalendarConnection["kind"],
  source: string,
  refreshMinutes: number,
) {
  return invoke<NativeCalendarConnection>("create_calendar_connection", {
    displayName,
    kind,
    source,
    refreshMinutes,
  });
}

export async function setNativeCalendarConnectionEnabled(id: string, enabled: boolean) {
  return invoke<NativeCalendarConnection[]>("set_calendar_connection_enabled", { id, enabled });
}

export async function deleteNativeCalendarConnection(id: string, date: Date) {
  const days = localWeekRanges(date).map((range) => range.day);
  return invoke<NativeCalendarSyncBatch>("delete_calendar_connection", {
    id,
    day: localDayKey(date),
    days,
  });
}

export async function loadNativeScheduleDays(date: Date) {
  const days = localWeekRanges(date).map((range) => range.day);
  return invoke<NativeScheduleDay[]>("load_schedule_days", {
    day: localDayKey(date),
    days,
  });
}

export async function configureNativeCalendarSync(viewerTimezone: string) {
  await invoke("configure_calendar_sync", { viewerTimezone });
}

export function listenNativeCalendarSync(
  onChange: (batch: NativeCalendarSyncBatch) => void,
) {
  return listen<NativeCalendarSyncBatch>("calendar-sync-updated", (event) => {
    onChange(event.payload);
  });
}

export function listenNativeCalendarSyncFailure(onError: (error: string) => void) {
  return listen<string>("calendar-sync-failed", (event) => {
    onError(event.payload);
  });
}

export async function syncNativeCalendarConnections(
  date: Date,
  options: { connectionId?: string; force?: boolean } = {},
) {
  const days = localWeekRanges(date).map((range) => range.day);
  return invoke<NativeCalendarSyncBatch>("sync_calendar_connections", {
    day: localDayKey(date),
    days,
    viewerTimezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    connectionId: options.connectionId,
    force: options.force ?? false,
  });
}

export async function persistTrackingEnabled(enabled: boolean) {
  await invoke("set_tracking_enabled", { enabled });
}

export async function persistActivityIdleThreshold(minutes: number) {
  await invoke("set_activity_idle_threshold", { minutes });
}

export async function loadNativeActivityCaptureState() {
  return invoke<NativeActivityCaptureState>("load_activity_capture_state");
}

export async function persistWindowTitleCapture(enabled: boolean) {
  await invoke("set_capture_window_titles", { enabled });
}

export async function persistCodexObservation(enabled: boolean) {
  await invoke("set_codex_observation_enabled", { enabled });
}

export async function persistActivityPrivacy(excludedApps: string[], retentionDays: number) {
  return invoke<NativeActivityPrivacyUpdate>("set_activity_privacy", {
    excludedApps,
    retentionDays,
  });
}

export async function clearNativeActivityRecords() {
  return invoke<number>("clear_activity_records");
}

export async function loadNativeActivityWeekSummary(date: Date) {
  return invoke<ActivityDaySummary[]>("load_activity_week_summary", {
    ranges: localWeekRanges(date),
  });
}

export async function setNativeWindowMode(mode: NativeWindowModeSnapshot["mode"]) {
  return invoke<NativeWindowModeSnapshot>("set_window_mode", { mode });
}

export async function setNativeWindowClickThrough(enabled: boolean) {
  await invoke("set_window_click_through", { enabled });
}

export function listenNativeWindowMode(
  onChange: (snapshot: NativeWindowModeSnapshot) => void,
) {
  return listen<NativeWindowModeSnapshot>("window-mode-changed", (event) => {
    onChange(event.payload);
  });
}

export async function loadNativeCodexSnapshot() {
  return invoke<NativeCodexSnapshot>("load_codex_snapshot");
}

export async function refreshNativeCodexSnapshot() {
  return invoke<NativeCodexSnapshot>("refresh_codex_snapshot");
}

export async function loadManagedCodexSnapshot() {
  return invoke<NativeManagedCodexSnapshot>("load_managed_codex_snapshot");
}

export async function startManagedCodexRun(prompt: string, cwd?: string) {
  return invoke<NativeManagedCodexSnapshot>("start_managed_codex_run", {
    prompt,
    cwd,
  });
}

export async function startNativeScreenshotImport(
  fileName: string,
  mimeType: string,
  base64Data: string,
) {
  return invoke<NativeManagedCodexSnapshot>("start_screenshot_import", {
    fileName,
    mimeType,
    base64Data,
  });
}

export async function cancelNativeScreenshotImport() {
  return invoke<NativeManagedCodexSnapshot>("cancel_screenshot_import");
}

export async function dismissNativeScreenshotImport() {
  return invoke<NativeManagedCodexSnapshot>("dismiss_screenshot_import");
}

export async function interruptManagedCodexRun(threadId: string) {
  return invoke<NativeManagedCodexSnapshot>("interrupt_managed_codex_run", { threadId });
}

export async function resolveManagedCodexApproval(approvalId: string, approved: boolean) {
  return invoke<NativeManagedCodexSnapshot>("resolve_managed_codex_approval", {
    approvalId,
    approved,
  });
}

export function managedRunsToAgents(runs: NativeManagedCodexRun[]): Agent[] {
  return runs.map(({ threadId, turnId: _turnId, startedAtMs: _startedAtMs, ...run }) => ({
    ...run,
    id: threadId,
  }));
}

export function storedActivitiesToTimeline(records: StoredActivityRecord[]): ActivityBlock[] {
  return records.map((record) => {
    const startedAt = new Date(record.startedAtMs);
    const endedAt = new Date(record.endedAtMs);
    const startMinute = startedAt.getHours() * 60 + startedAt.getMinutes();
    const rawEndMinute = endedAt.getHours() * 60 + endedAt.getMinutes();
    const category = ["focus", "meeting", "life", "admin"].includes(record.category)
      ? record.category as ActivityBlock["category"]
      : "admin";
    return {
      id: record.id,
      appName: record.appName,
      windowTitle: record.sanitizedWindowTitle ?? "窗口标题已隐藏",
      startMinute,
      endMinute: Math.min(1440, Math.max(startMinute + 1, rawEndMinute)),
      category,
    };
  });
}
