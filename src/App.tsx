import {
  Activity,
  AlertTriangle,
  AlarmClock,
  Bot,
  CalendarPlus,
  CalendarRange,
  Check,
  ChevronRight,
  CirclePause,
  Clock3,
  Focus,
  Hand,
  Eye,
  EyeOff,
  FileUp,
  FolderOpen,
  LayoutDashboard,
  Link2,
  Maximize2,
  Minimize2,
  MoreHorizontal,
  Pause,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  Repeat2,
  ScanText,
  Send,
  Settings,
  ShieldCheck,
  Sparkles,
  Square,
  Trash2,
  Workflow,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type FormEvent, type MouseEvent } from "react";
import { PixelOffice } from "./components/PixelOffice";
import { MorningPlannerDialog } from "./components/MorningPlannerDialog";
import { ScreenshotImportDialog } from "./components/ScreenshotImportDialog";
import { TimeSummary } from "./components/TimeSummary";
import { Timeline, formatMinute } from "./components/Timeline";
import { WeekOverview } from "./components/WeekOverview";
import { WeekSchedule } from "./components/WeekSchedule";
import {
  activityBlocks,
  initialAgents,
  initialApprovals,
  initialMorningTasks,
  scheduleBlocks as initialSchedule,
} from "./data/mockData";
import {
  applyNativeMorningPlan,
  clearNativeActivityRecords,
  configureNativeCalendarSync,
  createNativeCalendarConnection,
  deleteNativeCalendarConnection,
  dismissNativeMorningPrompt,
  loadNativeActivityCaptureState,
  loadNativeLocalState,
  loadNativeActivityWeekSummary,
  loadNativeScheduleDays,
  loadNativeCodexSnapshot,
  loadManagedCodexSnapshot,
  listenNativeCalendarSync,
  listenNativeCalendarSyncFailure,
  listenNativeWindowMode,
  managedRunsToAgents,
  nativeRuntimeAvailable,
  pickNativeCalendarFile,
  persistActivityPrivacy,
  persistActivityIdleThreshold,
  persistCodexObservation,
  persistNativeSchedule,
  persistNativePlannerTasks,
  persistMorningPromptSettings,
  persistTrackingEnabled,
  persistWindowTitleCapture,
  refreshNativeCodexSnapshot,
  previewNativeCalendarImport,
  resolveManagedCodexApproval,
  cancelNativeScreenshotImport,
  dismissNativeScreenshotImport,
  interruptManagedCodexRun,
  startNativeScreenshotImport,
  startManagedCodexRun,
  setNativeWindowMode,
  setNativeCalendarConnectionEnabled,
  snoozeNativeMorningPrompt,
  syncNativeCalendarConnections,
  storedActivitiesToTimeline,
  type NativeManagedCodexSnapshot,
  type NativeCalendarImportPreview,
  type NativeCalendarConnection,
  type NativeCalendarSyncBatch,
  type NativeActivityCaptureState,
  type NativeScreenshotImportSnapshot,
} from "./lib/nativeBridge";
import type { MorningPlanDraft, MorningPlanTask } from "./lib/morningPlanner";
import {
  evaluateMorningReminder,
  localDateKey,
  type MorningReminderState,
} from "./lib/morningReminder";
import {
  browserWeekSummary,
  emptyCategoryMinutes,
  localWeekRanges,
  occupiedScheduleMinutes,
  type ActivityDaySummary,
} from "./lib/timeStats";
import type { ActivityBlock, Agent, ApprovalRequest, ScheduleBlock, ScheduleDay } from "./types";

type ViewMode = "today" | "week";
type CalendarDialogView = "connections" | "file-import";

const statusLabels = {
  working: "编写中",
  searching: "检索中",
  waiting: "等待批准",
  recent: "最近活动",
  idle: "空闲",
  blocked: "已阻塞",
};

const disconnectedCodexAgent: Agent = {
  id: "codex-observer",
  name: "Codex",
  provider: "等待授权",
  task: "本机任务观察",
  detail: "尚未读取 Codex 任务元数据",
  status: "idle",
  elapsedMinutes: 0,
  accent: "#72d6a5",
  position: { x: 26, y: 58 },
  capabilities: [],
  controlMode: "observed",
};

const captureStatusLabels: Record<NativeActivityCaptureState["status"], string> = {
  active: "本地记录中",
  idle: "空闲自动暂停",
  locked: "锁屏自动暂停",
  disconnected: "会话断开暂停",
  paused: "记录已暂停",
  unavailable: "采集不可用",
};

function formatClock(date: Date) {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

function formatDate(date: Date) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "short",
  }).format(date);
}

function formatWorkspaceDate(date: Date) {
  return new Intl.DateTimeFormat("en-US", {
    weekday: "long",
    month: "short",
    day: "numeric",
  }).format(date).toLocaleUpperCase("en-US").replace(",", " ·");
}

function formatCalendarSyncTime(value?: number) {
  if (!value) return "尚未同步";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

async function fileToBase64(file: File) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const chunkSize = 0x8000;
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return window.btoa(binary);
}

function unavailableCaptureState(thresholdMinutes: number): NativeActivityCaptureState {
  return {
    status: "unavailable",
    idleSeconds: 0,
    thresholdMinutes,
    captureAllowed: false,
    sessionStateAvailable: false,
    checkedAtMs: Date.now(),
  };
}

function browserWeekSchedule(date: Date, todaySchedule: ScheduleBlock[]): ScheduleDay[] {
  const today = localDateKey(date);
  const templates: Array<Array<Pick<ScheduleBlock, "title" | "startMinute" | "endMinute" | "category">>> = [
    [{ title: "本周目标梳理", startMinute: 540, endMinute: 630, category: "focus" }],
    [{ title: "课程与资料整理", startMinute: 840, endMinute: 960, category: "focus" }],
    [{ title: "项目同步会", startMinute: 600, endMinute: 660, category: "meeting" }],
    [{ title: "实验复现", startMinute: 570, endMinute: 750, category: "focus" }],
    [{ title: "周报与复盘", startMinute: 960, endMinute: 1050, category: "admin" }],
    [{ title: "运动", startMinute: 600, endMinute: 690, category: "life" }],
    [],
  ];
  return localWeekRanges(date).map((range, index) => ({
    day: range.day,
    blocks: range.day === today ? todaySchedule : templates[index].map((template, blockIndex) => ({
      id: `demo-week-${index}-${blockIndex}`,
      ...template,
      source: index % 2 === 0 ? "calendar" : "agent",
      status: "planned",
      locked: index % 2 === 0,
    })),
  }));
}

function App() {
  const calendarConnectionsDemo = !nativeRuntimeAvailable
    && new URLSearchParams(window.location.search).has("calendarConnections");
  const screenshotDemo = !nativeRuntimeAvailable
    && new URLSearchParams(window.location.search).has("screenshotImport");
  const idleDemo = !nativeRuntimeAvailable
    && new URLSearchParams(window.location.search).has("idle");
  const [now, setNow] = useState(() => new Date());
  const [view, setView] = useState<ViewMode>("today");
  const [tracking, setTracking] = useState(!nativeRuntimeAvailable);
  const [activityCapture, setActivityCapture] = useState<NativeActivityCaptureState>(() => ({
    status: idleDemo ? "idle" : nativeRuntimeAvailable ? "paused" : "active",
    idleSeconds: idleDemo ? 360 : 0,
    thresholdMinutes: 5,
    captureAllowed: !nativeRuntimeAvailable && !idleDemo,
    sessionStateAvailable: nativeRuntimeAvailable,
    checkedAtMs: Date.now(),
  }));
  const [codexObservation, setCodexObservation] = useState(!nativeRuntimeAvailable);
  const [focusMode, setFocusMode] = useState(false);
  const [schedule, setSchedule] = useState(initialSchedule);
  const [activity, setActivity] = useState<ActivityBlock[]>(activityBlocks);
  const [morningTasks, setMorningTasks] = useState<MorningPlanTask[]>(nativeRuntimeAvailable ? [] : initialMorningTasks);
  const [agents, setAgents] = useState(nativeRuntimeAvailable ? [disconnectedCodexAgent] : initialAgents);
  const [approvals, setApprovals] = useState(initialApprovals);
  const [selectedAgentId, setSelectedAgentId] = useState(initialAgents[0].id);
  const [selectedTimelineId, setSelectedTimelineId] = useState<string | null>("s5");
  const [notice, setNotice] = useState("本地模拟事件流已连接");
  const [nativeHydrated, setNativeHydrated] = useState(false);
  const [managedPrompt, setManagedPrompt] = useState("");
  const [startingManagedRun, setStartingManagedRun] = useState(false);
  const [captureWindowTitles, setCaptureWindowTitles] = useState(false);
  const [excludedActivityApps, setExcludedActivityApps] = useState<string[]>([]);
  const [activityRetentionDays, setActivityRetentionDays] = useState(30);
  const [activityIdleThresholdMinutes, setActivityIdleThresholdMinutes] = useState(5);
  const [privacyTrackingDraft, setPrivacyTrackingDraft] = useState(false);
  const [privacyTitlesDraft, setPrivacyTitlesDraft] = useState(false);
  const [privacyExclusionsDraft, setPrivacyExclusionsDraft] = useState<string[]>([]);
  const [privacyRetentionDraft, setPrivacyRetentionDraft] = useState(30);
  const [privacyIdleThresholdDraft, setPrivacyIdleThresholdDraft] = useState(5);
  const [excludedAppInput, setExcludedAppInput] = useState("");
  const [privacySaving, setPrivacySaving] = useState(false);
  const [confirmClearActivity, setConfirmClearActivity] = useState(false);
  const [compactMode, setCompactMode] = useState(false);
  const [calendarPreview, setCalendarPreview] = useState<NativeCalendarImportPreview | null>(null);
  const [calendarFileName, setCalendarFileName] = useState("");
  const [calendarImportError, setCalendarImportError] = useState("");
  const [calendarImportLoading, setCalendarImportLoading] = useState(false);
  const [calendarImportSaving, setCalendarImportSaving] = useState(false);
  const [calendarDialogView, setCalendarDialogView] = useState<CalendarDialogView>("connections");
  const [calendarConnections, setCalendarConnections] = useState<NativeCalendarConnection[]>(() => (
    calendarConnectionsDemo ? [{
      id: "cal-demo-campus",
      displayName: "学习与会议",
      kind: "ics-subscription",
      sourceHint: "calendar.google.com",
      enabled: true,
      refreshMinutes: 30,
      lastSyncAtMs: Date.now() - 8 * 60_000,
      lastSyncStatus: "success",
      createdAtMs: Date.now() - 86_400_000,
      updatedAtMs: Date.now() - 8 * 60_000,
    }] : []
  ));
  const [calendarConnectionName, setCalendarConnectionName] = useState("");
  const [calendarConnectionKind, setCalendarConnectionKind] = useState<NativeCalendarConnection["kind"]>("ics-subscription");
  const [calendarConnectionSource, setCalendarConnectionSource] = useState("");
  const [calendarConnectionSourceVisible, setCalendarConnectionSourceVisible] = useState(false);
  const [calendarConnectionRefresh, setCalendarConnectionRefresh] = useState(30);
  const [calendarConnectionBusyId, setCalendarConnectionBusyId] = useState<string | null>(null);
  const [calendarConnectionSaving, setCalendarConnectionSaving] = useState(false);
  const [calendarConnectionError, setCalendarConnectionError] = useState("");
  const [calendarDeleteConfirmId, setCalendarDeleteConfirmId] = useState<string | null>(null);
  const [morningPlannerOpen, setMorningPlannerOpen] = useState(false);
  const [morningReminder, setMorningReminder] = useState<MorningReminderState>({
    enabled: true,
    promptMinute: 480,
  });
  const [snoozeMinutes, setSnoozeMinutes] = useState(15);
  const [demoReminderAcknowledged, setDemoReminderAcknowledged] = useState(false);
  const [weekSummary, setWeekSummary] = useState<ActivityDaySummary[]>([]);
  const [weekSchedule, setWeekSchedule] = useState<ScheduleDay[]>(() => (
    nativeRuntimeAvailable ? [] : browserWeekSchedule(new Date(), initialSchedule)
  ));
  const [weekSummaryLoading, setWeekSummaryLoading] = useState(false);
  const [screenshotImportOpen, setScreenshotImportOpen] = useState(screenshotDemo);
  const [screenshotImport, setScreenshotImport] = useState<NativeScreenshotImportSnapshot>(() => (
    screenshotDemo ? {
      status: "ready",
      fileName: "聊天截图.png",
      tasks: [
        { id: "demo-screenshot-1", title: "整理项目调研结论", durationMinutes: 60, priority: "high", preferredPeriod: "morning", category: "focus", notes: "截图中提到周三前完成" },
        { id: "demo-screenshot-2", title: "提交会议报名", durationMinutes: 30, priority: "critical", preferredPeriod: "afternoon", category: "admin", notes: "具体截止时间需再次确认" },
      ],
      warnings: ["第二项的具体截止时刻在截图中不清晰，请确认后再安排。"],
    } : { status: "idle", tasks: [], warnings: [] }
  ));
  const privacyDialogRef = useRef<HTMLDialogElement>(null);
  const calendarDialogRef = useRef<HTMLDialogElement>(null);
  const calendarFileRef = useRef<HTMLInputElement>(null);

  function applyManagedSnapshot(snapshot: NativeManagedCodexSnapshot) {
    setScreenshotImport(snapshot.screenshotImport);
    const managed = managedRunsToAgents(snapshot.runs);
    const managedIds = new Set(managed.map((agent) => agent.id));
    setAgents((current) => {
      const observed = current.filter((agent) =>
        agent.controlMode !== "managed"
        && agent.id !== disconnectedCodexAgent.id
        && !managedIds.has(agent.id));
      const next = [...managed, ...observed];
      return next.length ? next : [disconnectedCodexAgent];
    });
    setApprovals((current) => [
      ...current.filter((approval) => approval.kind !== "agent-tool"),
      ...snapshot.approvals,
    ]);
  }

  function applyCalendarSyncBatch(batch: NativeCalendarSyncBatch) {
    setCalendarConnections(batch.connections);
    setSchedule(batch.scheduleBlocks);
    setWeekSchedule(batch.scheduleDays);
    if (batch.warnings.length > 0) {
      setCalendarConnectionError(batch.warnings.join("；"));
    } else {
      setCalendarConnectionError("");
    }
  }

  function applyObservedAgents(observed: Agent[]) {
    setAgents((current) => {
      const managed = current.filter((agent) => agent.controlMode === "managed");
      const managedIds = new Set(managed.map((agent) => agent.id));
      const visibleObserved = observed.filter((agent) => !managedIds.has(agent.id));
      const next = [...managed, ...visibleObserved];
      return next.length ? next : [disconnectedCodexAgent];
    });
  }

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!calendarConnectionsDemo) return;
    const frame = window.requestAnimationFrame(() => openCalendarImport());
    return () => window.cancelAnimationFrame(frame);
  }, [calendarConnectionsDemo]);

  useEffect(() => {
    if (!nativeRuntimeAvailable) return;
    let cancelled = false;
    const date = new Date();

    loadNativeLocalState(date)
      .then(async (state) => {
        if (cancelled) return;
        if (state.scheduleBlocks.length > 0) {
          setSchedule(state.scheduleBlocks);
        } else {
          await persistNativeSchedule(date, initialSchedule);
        }
        if (cancelled) return;
        setActivity(storedActivitiesToTimeline(state.activityRecords));
        setMorningTasks(state.plannerTasks);
        setCalendarConnections(state.calendarConnections);
        setMorningReminder(state.morningPrompt);
        setTracking(state.trackingEnabled);
        setCaptureWindowTitles(state.captureWindowTitles);
        setCodexObservation(state.codexObservationEnabled);
        setExcludedActivityApps(state.excludedActivityApps);
        setActivityRetentionDays(state.activityRetentionDays);
        setActivityIdleThresholdMinutes(state.activityIdleThresholdMinutes);
        setActivityCapture(await loadNativeActivityCaptureState().catch(() => (
          unavailableCaptureState(state.activityIdleThresholdMinutes)
        )));
        setCompactMode(state.windowMode === "compact");
        const pendingApprovals: ApprovalRequest[] = [];
        if (!state.codexObservationEnabled) {
          pendingApprovals.push({
            id: "codex-observe",
            kind: "codex-observe",
            title: "读取本机 Codex 任务元数据",
            detail: "读取任务名称、来源、更新时间和运行状态；不读取消息正文。",
            risk: "low",
          });
        }
        if (!state.captureWindowTitles) {
          pendingApprovals.push({
            id: "activity-capture",
            kind: "activity-capture",
            title: "读取前台窗口标题",
            detail: "仅在本地读取并应用脱敏规则，不上传原始标题。",
            risk: "medium",
          });
        }
        setApprovals(pendingApprovals);
        setNativeHydrated(true);
        setNotice("SQLite 本地数据已连接");
      })
      .catch((error) => {
        if (!cancelled) setNotice(`本地数据连接失败：${String(error)}`);
      });

    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!nativeRuntimeAvailable || !nativeHydrated) return;
    let active = true;
    let disposeSync: (() => void) | undefined;
    let disposeFailure: (() => void) | undefined;
    const setup = async () => {
      const [unlistenSync, unlistenFailure] = await Promise.all([
        listenNativeCalendarSync((batch) => {
          if (!active) return;
          applyCalendarSyncBatch(batch);
          if (batch.failedCount > 0) {
            setNotice(`${batch.failedCount} 个日历连接同步失败`);
          } else if (batch.syncedCount > 0) {
            setNotice(`已在后台同步 ${batch.syncedCount} 个日历连接`);
          }
        }),
        listenNativeCalendarSyncFailure((error) => {
          if (active) setNotice(`日历后台同步失败：${error}`);
        }),
      ]);
      if (!active) {
        unlistenSync();
        unlistenFailure();
        return;
      }
      disposeSync = unlistenSync;
      disposeFailure = unlistenFailure;
      await configureNativeCalendarSync(Intl.DateTimeFormat().resolvedOptions().timeZone);
    };
    void setup().catch((error) => {
      if (active) setNotice(`日历后台同步启动失败：${String(error)}`);
    });
    return () => {
      active = false;
      disposeSync?.();
      disposeFailure?.();
    };
  }, [nativeHydrated]);

  useEffect(() => {
    if (!nativeRuntimeAvailable) return;
    let dispose: (() => void) | undefined;
    void listenNativeWindowMode((snapshot) => {
      setCompactMode(snapshot.mode === "compact");
      setNotice(snapshot.mode === "compact" ? "已切换到桌面状态条" : "已从托盘展开工作台");
    }).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, []);

  useEffect(() => {
    if (!nativeRuntimeAvailable || !nativeHydrated) return;
    const timer = window.setTimeout(() => {
      persistNativeSchedule(new Date(), schedule).catch((error) => {
        setNotice(`日程保存失败：${String(error)}`);
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [nativeHydrated, schedule]);

  useEffect(() => {
    if (!nativeRuntimeAvailable || !nativeHydrated) return;
    const timer = window.setTimeout(() => {
      persistNativePlannerTasks(morningTasks).catch((error) => {
        setNotice(`待办保存失败：${String(error)}`);
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [morningTasks, nativeHydrated]);

  useEffect(() => {
    if (!nativeRuntimeAvailable || !nativeHydrated) return;
    let active = true;

    const refresh = async () => {
      if (document.hidden) return;
      try {
        const [state, captureState] = await Promise.all([
          loadNativeLocalState(new Date()),
          loadNativeActivityCaptureState().catch(() => null),
        ]);
        if (active) {
          setActivity(storedActivitiesToTimeline(state.activityRecords));
          setActivityCapture(captureState ?? unavailableCaptureState(state.activityIdleThresholdMinutes));
        }
      } catch (error) {
        if (active) setNotice(`活动刷新失败：${String(error)}`);
      }
    };
    const onVisibilityChange = () => { if (!document.hidden) void refresh(); };
    const timer = window.setInterval(() => void refresh(), 5_000);
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      active = false;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [nativeHydrated]);

  useEffect(() => {
    if (!nativeRuntimeAvailable || !nativeHydrated) return;
    let active = true;
    const refresh = async () => {
      if (document.hidden) return;
      setWeekSummaryLoading(true);
      try {
        const [summary, scheduleDays] = await Promise.all([
          loadNativeActivityWeekSummary(new Date()),
          loadNativeScheduleDays(new Date()),
        ]);
        if (active) {
          setWeekSummary(summary);
          setWeekSchedule(scheduleDays);
        }
      } catch (error) {
        if (active) setNotice(`周统计刷新失败：${String(error)}`);
      } finally {
        if (active) setWeekSummaryLoading(false);
      }
    };
    const onVisibilityChange = () => { if (!document.hidden) void refresh(); };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 30_000);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      active = false;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [nativeHydrated]);

  useEffect(() => {
    if (!nativeRuntimeAvailable) return;
    const today = localDateKey(new Date());
    setWeekSchedule((days) => days.length === 0
      ? days
      : days.map((day) => day.day === today ? { ...day, blocks: schedule } : day));
  }, [schedule]);

  useEffect(() => {
    if (!nativeRuntimeAvailable || !nativeHydrated || !codexObservation) return;
    let active = true;

    const applySnapshot = async () => {
      try {
        const snapshot = await loadNativeCodexSnapshot();
        if (!active) return;
        if (snapshot.connectionState === "connected") {
          applyObservedAgents(snapshot.agents);
        } else if (snapshot.connectionState === "error") {
          setNotice(`Codex 连接失败：${snapshot.message}`);
        }
      } catch (error) {
        if (active) setNotice(`Codex 状态刷新失败：${String(error)}`);
      }
    };
    void applySnapshot();
    const timer = window.setInterval(() => void applySnapshot(), 3_000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [codexObservation, nativeHydrated]);

  useEffect(() => {
    if (!nativeRuntimeAvailable || !nativeHydrated) return;
    let active = true;
    const refresh = async () => {
      try {
        const snapshot = await loadManagedCodexSnapshot();
        if (active) applyManagedSnapshot(snapshot);
      } catch (error) {
        if (active) setNotice(`托管 Codex 状态刷新失败：${String(error)}`);
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 1_000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [nativeHydrated]);

  useEffect(() => {
    if (agents.some((agent) => agent.id === selectedAgentId)) return;
    setSelectedAgentId(agents[0]?.id ?? disconnectedCodexAgent.id);
  }, [agents, selectedAgentId]);

  const currentMinute = now.getHours() * 60 + now.getMinutes();
  const todayKey = localDateKey(now);
  const fallbackWeekSchedule = useMemo(() => browserWeekSchedule(now, schedule), [now, schedule]);
  const fallbackWeekSummary = useMemo(
    () => browserWeekSummary(now, schedule, activity).map((day) => ({
      ...day,
      plannedMinutes: occupiedScheduleMinutes(
        fallbackWeekSchedule.find((scheduleDay) => scheduleDay.day === day.day)?.blocks ?? [],
      ),
    })),
    [activity, fallbackWeekSchedule, now, schedule],
  );
  const visibleWeekSummary = useMemo(() => {
    const base = nativeRuntimeAvailable && weekSummary.length === 7
      ? weekSummary
      : fallbackWeekSummary;
    return base.map((day) => day.day === todayKey ? {
      ...day,
      plannedMinutes: occupiedScheduleMinutes(schedule),
    } : day);
  }, [fallbackWeekSummary, schedule, todayKey, weekSummary]);
  const emptyNativeWeekSchedule = useMemo(() => localWeekRanges(now).map((range) => ({ day: range.day, blocks: [] })), [now]);
  const visibleWeekSchedule = nativeRuntimeAvailable
    ? weekSchedule.length === 7 ? weekSchedule : emptyNativeWeekSchedule
    : fallbackWeekSchedule;
  const todaySummary = visibleWeekSummary.find((day) => day.day === todayKey)
    ?? fallbackWeekSummary.find((day) => day.day === todayKey)!;
  const reminderDemo = !nativeRuntimeAvailable
    && new URLSearchParams(window.location.search).has("morningReminder");
  const reminderEvaluation = evaluateMorningReminder(now, morningReminder);
  const showMorningReminder = (!nativeRuntimeAvailable || nativeHydrated)
    && (reminderEvaluation.due || (reminderDemo && !demoReminderAcknowledged));
  const selectedAgent = agents.find((agent) => agent.id === selectedAgentId) ?? agents[0] ?? disconnectedCodexAgent;
  const activeAgents = agents.filter((agent) => !["idle", "blocked"].includes(agent.status));
  const selectedItem = useMemo(() => {
    const planned = schedule.find((item) => item.id === selectedTimelineId);
    if (planned) return { title: planned.title, detail: `${formatMinute(planned.startMinute)} - ${formatMinute(planned.endMinute)} · ${planned.source}` };
    const actual = activity.find((item) => item.id === selectedTimelineId);
    if (actual) return { title: actual.appName, detail: `${actual.windowTitle} · ${formatMinute(actual.startMinute)} - ${formatMinute(actual.endMinute)}` };
    return null;
  }, [activity, schedule, selectedTimelineId]);
  const calendarPreviewEvents = useMemo(() => calendarPreview?.events.map((event) => ({
    ...event,
    conflicts: schedule.filter((block) =>
      block.id !== event.id
      && block.startMinute < event.endMinute
      && block.endMinute > event.startMinute),
  })) ?? [], [calendarPreview, schedule]);
  const calendarConflictCount = calendarPreviewEvents.filter((event) => event.conflicts.length > 0).length;

  const nextBlock = schedule.find((block) => block.startMinute > currentMinute);
  const nextTimeLabel = nextBlock ? formatMinute(nextBlock.startMinute) : "明日 08:00";
  const nextTitle = nextBlock?.title ?? "晨间规划";
  const dayProgress = Math.max(0, Math.min(100, ((currentMinute - 420) / (1380 - 420)) * 100));

  function updateAgentStatus(agent: Agent, status: Agent["status"], message: string) {
    setAgents((items) => items.map((item) => item.id === agent.id ? { ...item, status } : item));
    setNotice(message);
  }

  async function toggleTracking() {
    const next = !tracking;
    try {
      if (nativeRuntimeAvailable) {
        await persistTrackingEnabled(next);
        setActivityCapture(await loadNativeActivityCaptureState().catch(() => (
          unavailableCaptureState(activityIdleThresholdMinutes)
        )));
      } else {
        setActivityCapture((current) => ({
          ...current,
          status: next ? idleDemo ? "idle" : "active" : "paused",
          captureAllowed: next && !idleDemo,
          checkedAtMs: Date.now(),
        }));
      }
      setTracking(next);
      setNotice(next ? "前台活动记录已开启" : "前台活动记录已暂停");
    } catch (error) {
      setNotice(`无法更新记录状态：${String(error)}`);
    }
  }

  async function resolveApproval(approved: boolean) {
    const request = approvals[0];
    if (!request) return;
    try {
      if (request.kind === "codex-observe") {
        if (nativeRuntimeAvailable) await persistCodexObservation(approved);
        setCodexObservation(approved);
        if (approved && nativeRuntimeAvailable) {
          setNotice("正在连接 Codex App Server");
          const snapshot = await refreshNativeCodexSnapshot();
          if (snapshot.connectionState === "connected") {
            applyObservedAgents(snapshot.agents);
            setNotice(`已读取 ${snapshot.agents.length} 个 Codex 任务`);
          } else {
            setNotice(`Codex 连接失败：${snapshot.message}`);
          }
        }
      } else if (request.kind === "activity-capture") {
        if (nativeRuntimeAvailable) {
          await persistWindowTitleCapture(approved);
          await persistTrackingEnabled(approved);
        }
        setTracking(approved);
      } else if (request.kind === "agent-tool" && nativeRuntimeAvailable) {
        const snapshot = await resolveManagedCodexApproval(request.id, approved);
        applyManagedSnapshot(snapshot);
        setNotice(approved ? "已批准本次 Codex 工具调用" : "已拒绝本次 Codex 工具调用");
        return;
      }
    } catch (error) {
      setNotice(`权限设置失败：${String(error)}`);
      return;
    }
    setApprovals((items) => items.filter((item) => item.id !== request.id));
    if (request.agentId) {
      setAgents((items) => items.map((item) => item.id === request.agentId ? {
        ...item,
        status: approved ? "working" : "blocked",
        detail: approved ? "已获批准，正在应用脱敏规则" : "权限被拒绝，任务已停止",
      } : item));
    }
    if (!approved) setNotice("已拒绝本次本地读取");
  }

  async function toggleCodexObservation() {
    const next = !codexObservation;
    try {
      if (nativeRuntimeAvailable) await persistCodexObservation(next);
      setCodexObservation(next);
      setApprovals((items) => items.filter((item) => item.kind !== "codex-observe"));
      if (!next) {
        setAgents((items) => {
          const managed = items.filter((agent) => agent.controlMode === "managed");
          return managed.length ? managed : [disconnectedCodexAgent];
        });
        setNotice("Codex 任务观察已停止");
        return;
      }
      setNotice("正在连接 Codex App Server");
      if (nativeRuntimeAvailable) {
        const snapshot = await refreshNativeCodexSnapshot();
        if (snapshot.connectionState === "connected") {
          applyObservedAgents(snapshot.agents);
          setNotice(`已读取 ${snapshot.agents.length} 个 Codex 任务`);
        } else {
          setNotice(`Codex 连接失败：${snapshot.message}`);
        }
      }
    } catch (error) {
      setNotice(`无法更新 Codex 观察状态：${String(error)}`);
    }
  }

  async function submitManagedTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const prompt = managedPrompt.trim();
    if (!prompt || startingManagedRun) return;
    setStartingManagedRun(true);
    setNotice("正在创建受控 Codex 任务");
    try {
      const snapshot = await startManagedCodexRun(prompt);
      applyManagedSnapshot(snapshot);
      setManagedPrompt("");
      setSelectedAgentId(snapshot.runs.at(-1)?.threadId ?? selectedAgentId);
      setNotice("Codex 任务已启动，工具调用将在这里请求批准");
    } catch (error) {
      setNotice(`Codex 任务启动失败：${String(error)}`);
    } finally {
      setStartingManagedRun(false);
    }
  }

  async function stopAgent(agent: Agent) {
    if (nativeRuntimeAvailable && agent.controlMode === "managed") {
      try {
        const snapshot = await interruptManagedCodexRun(agent.id);
        applyManagedSnapshot(snapshot);
        setNotice("已向 Codex 发送终止请求");
      } catch (error) {
        setNotice(`终止 Codex 任务失败：${String(error)}`);
      }
      return;
    }
    updateAgentStatus(agent, "blocked", "Agent 已终止");
  }

  function openPrivacySettings() {
    setPrivacyTrackingDraft(tracking);
    setPrivacyTitlesDraft(captureWindowTitles);
    setPrivacyExclusionsDraft(excludedActivityApps);
    setPrivacyRetentionDraft(activityRetentionDays);
    setPrivacyIdleThresholdDraft(activityIdleThresholdMinutes);
    setExcludedAppInput("");
    setConfirmClearActivity(false);
    privacyDialogRef.current?.showModal();
  }

  function openCalendarImport() {
    privacyDialogRef.current?.close();
    setCalendarDialogView("connections");
    setCalendarPreview(null);
    setCalendarFileName("");
    setCalendarImportError("");
    if (calendarFileRef.current) calendarFileRef.current.value = "";
    calendarDialogRef.current?.showModal();
  }

  async function chooseCalendarConnectionFile() {
    setCalendarConnectionError("");
    if (!nativeRuntimeAvailable) {
      setCalendarConnectionError("本地文件连接只在 Agent Bar 桌面版中可用");
      return;
    }
    try {
      const path = await pickNativeCalendarFile();
      if (path) {
        setCalendarConnectionSource(path);
        if (!calendarConnectionName.trim()) {
          const fileName = path.split(/[\\/]/).at(-1)?.replace(/\.ics$/i, "");
          setCalendarConnectionName(fileName || "本地日历");
        }
      }
    } catch (error) {
      setCalendarConnectionError(`无法选择文件：${String(error)}`);
    }
  }

  async function saveCalendarConnection(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (calendarConnectionSaving) return;
    setCalendarConnectionSaving(true);
    setCalendarConnectionError("");
    try {
      if (!nativeRuntimeAvailable) {
        if (!calendarConnectionsDemo) throw new Error("日历连接只在 Agent Bar 桌面版中运行");
        const sourceHint = calendarConnectionKind === "local-file"
          ? calendarConnectionSource.split(/[\\/]/).at(-1) || "本地日历.ics"
          : new URL(calendarConnectionSource).hostname;
        const demoConnection: NativeCalendarConnection = {
          id: `cal-demo-${Date.now()}`,
          displayName: calendarConnectionName.trim(),
          kind: calendarConnectionKind,
          sourceHint,
          enabled: true,
          refreshMinutes: calendarConnectionRefresh,
          lastSyncAtMs: Date.now(),
          lastSyncStatus: "success",
          createdAtMs: Date.now(),
          updatedAtMs: Date.now(),
        };
        setCalendarConnections((items) => [...items, demoConnection]);
        setNotice("演示日历连接已保存");
      } else {
        const connection = await createNativeCalendarConnection(
          calendarConnectionName,
          calendarConnectionKind,
          calendarConnectionSource,
          calendarConnectionRefresh,
        );
        setCalendarConnections((items) => [...items, connection]);
        setCalendarConnectionBusyId(connection.id);
        const batch = await syncNativeCalendarConnections(new Date(), {
          connectionId: connection.id,
          force: true,
        });
        applyCalendarSyncBatch(batch);
        setNotice(batch.failedCount
          ? `${connection.displayName} 已保存，但首次同步失败`
          : `${connection.displayName} 已连接并同步`);
      }
      setCalendarConnectionName("");
      setCalendarConnectionSource("");
      setCalendarConnectionSourceVisible(false);
      setCalendarConnectionRefresh(30);
    } catch (error) {
      setCalendarConnectionError(String(error));
    } finally {
      setCalendarConnectionBusyId(null);
      setCalendarConnectionSaving(false);
    }
  }

  async function syncCalendarConnection(connection: NativeCalendarConnection) {
    if (calendarConnectionBusyId) return;
    setCalendarConnectionBusyId(connection.id);
    setCalendarConnectionError("");
    try {
      if (!nativeRuntimeAvailable) {
        setCalendarConnections((items) => items.map((item) => item.id === connection.id ? {
          ...item,
          lastSyncAtMs: Date.now(),
          lastSyncStatus: "success",
          lastError: undefined,
        } : item));
        setNotice(`${connection.displayName} 演示同步完成`);
      } else {
        const batch = await syncNativeCalendarConnections(new Date(), {
          connectionId: connection.id,
          force: true,
        });
        applyCalendarSyncBatch(batch);
        setNotice(batch.failedCount ? `${connection.displayName} 同步失败` : `${connection.displayName} 已同步`);
      }
    } catch (error) {
      setCalendarConnectionError(String(error));
    } finally {
      setCalendarConnectionBusyId(null);
    }
  }

  async function toggleCalendarConnection(connection: NativeCalendarConnection) {
    if (calendarConnectionBusyId) return;
    setCalendarConnectionBusyId(connection.id);
    try {
      const enabled = !connection.enabled;
      if (nativeRuntimeAvailable) {
        setCalendarConnections(await setNativeCalendarConnectionEnabled(connection.id, enabled));
      } else {
        setCalendarConnections((items) => items.map((item) => item.id === connection.id
          ? { ...item, enabled, updatedAtMs: Date.now() }
          : item));
      }
      setNotice(enabled ? `${connection.displayName} 已继续自动同步` : `${connection.displayName} 已暂停自动同步`);
    } catch (error) {
      setCalendarConnectionError(String(error));
    } finally {
      setCalendarConnectionBusyId(null);
    }
  }

  async function removeCalendarConnection(connection: NativeCalendarConnection) {
    if (calendarDeleteConfirmId !== connection.id) {
      setCalendarDeleteConfirmId(connection.id);
      return;
    }
    if (calendarConnectionBusyId) return;
    setCalendarConnectionBusyId(connection.id);
    try {
      if (nativeRuntimeAvailable) {
        applyCalendarSyncBatch(await deleteNativeCalendarConnection(connection.id, new Date()));
      } else {
        setCalendarConnections((items) => items.filter((item) => item.id !== connection.id));
      }
      setCalendarDeleteConfirmId(null);
      setNotice(`${connection.displayName} 已断开，关联日程已移除`);
    } catch (error) {
      setCalendarConnectionError(String(error));
    } finally {
      setCalendarConnectionBusyId(null);
    }
  }

  function openMorningPlanner() {
    privacyDialogRef.current?.close();
    calendarDialogRef.current?.close();
    setDemoReminderAcknowledged(true);
    setMorningPlannerOpen(true);
  }

  function openScreenshotImport() {
    privacyDialogRef.current?.close();
    calendarDialogRef.current?.close();
    setMorningPlannerOpen(false);
    setScreenshotImportOpen(true);
  }

  async function analyzeScreenshot(file: File) {
    if (!nativeRuntimeAvailable) {
      throw new Error("截图分析只在 Agent Bar 桌面版中运行");
    }
    setScreenshotImport({ status: "analyzing", fileName: file.name, tasks: [], warnings: [] });
    try {
      const snapshot = await startNativeScreenshotImport(file.name, file.type, await fileToBase64(file));
      applyManagedSnapshot(snapshot);
      setNotice("Codex 正在从截图提取日程事项");
    } catch (error) {
      setScreenshotImport({ status: "error", fileName: file.name, tasks: [], warnings: [], error: String(error) });
      throw error;
    }
  }

  async function cancelScreenshotAnalysis() {
    if (!nativeRuntimeAvailable) return;
    const snapshot = await cancelNativeScreenshotImport();
    applyManagedSnapshot(snapshot);
    setNotice("截图分析已终止，临时图片已删除");
  }

  async function confirmScreenshotTasks(tasks: MorningPlanTask[]) {
    const incomingIds = new Set(tasks.map((task) => task.id));
    const merged = [...morningTasks.filter((task) => !incomingIds.has(task.id)), ...tasks];
    if (nativeRuntimeAvailable) {
      await persistNativePlannerTasks(merged);
      applyManagedSnapshot(await dismissNativeScreenshotImport());
    }
    setMorningTasks(merged);
    setScreenshotImportOpen(false);
    setNotice(`已从截图加入 ${tasks.length} 项待办，请确认全天安排`);
    window.setTimeout(() => setMorningPlannerOpen(true), 0);
  }

  async function snoozeMorningReminder() {
    const untilMs = Date.now() + snoozeMinutes * 60_000;
    try {
      if (nativeRuntimeAvailable) await snoozeNativeMorningPrompt(untilMs);
      setMorningReminder((current) => ({ ...current, snoozedUntilMs: untilMs }));
      setDemoReminderAcknowledged(true);
      setNotice(`晨间规划已延后 ${snoozeMinutes} 分钟`);
    } catch (error) {
      setNotice(`无法延后晨间规划：${String(error)}`);
    }
  }

  async function dismissMorningReminderToday() {
    const day = localDateKey(now);
    try {
      if (nativeRuntimeAvailable) await dismissNativeMorningPrompt(day);
      setMorningReminder((current) => ({ ...current, dismissedDay: day }));
      setDemoReminderAcknowledged(true);
      setNotice("今天不再提醒晨间规划");
    } catch (error) {
      setNotice(`无法更新晨间提醒：${String(error)}`);
    }
  }

  async function changeMorningReminder(enabled: boolean, promptMinute: number) {
    try {
      if (nativeRuntimeAvailable) await persistMorningPromptSettings(enabled, promptMinute);
      setMorningReminder((current) => ({ ...current, enabled, promptMinute }));
      setNotice(enabled ? `每日规划提醒已设为 ${formatMinute(promptMinute)}` : "每日规划提醒已关闭");
    } catch (error) {
      setNotice(`无法保存提醒设置：${String(error)}`);
    }
  }

  async function confirmMorningPlan(draft: MorningPlanDraft) {
    const scheduledTaskIds = new Set(draft.blocks.map((block) => block.taskId));
    const imported: ScheduleBlock[] = draft.blocks.map(({ taskId: _taskId, reason: _reason, ...block }) => block);
    const incomingIds = new Set(imported.map((block) => block.id));
    const merged = [
      ...schedule.filter((block) => !incomingIds.has(block.id)),
      ...imported,
    ].sort((left, right) => left.startMinute - right.startMinute || left.endMinute - right.endMinute);
    const remainingTasks = morningTasks.filter((task) => !scheduledTaskIds.has(task.id));

    if (nativeRuntimeAvailable) {
      await applyNativeMorningPlan(new Date(), merged, remainingTasks);
    }
    setSchedule(merged);
    setMorningTasks(remainingTasks);
    setMorningReminder((current) => ({
      ...current,
      dismissedDay: undefined,
      snoozedUntilMs: undefined,
      lastPlannedDay: localDateKey(new Date()),
    }));
    setSelectedTimelineId(imported[0]?.id ?? null);
    setMorningPlannerOpen(false);
    setNotice(`已安排 ${imported.length} 项任务${draft.unscheduled.length ? `，${draft.unscheduled.length} 项保留在待办` : ""}`);
  }

  async function selectCalendarFile(file?: File) {
    if (!file) return;
    setCalendarPreview(null);
    setCalendarFileName(file.name);
    setCalendarImportError("");
    if (!file.name.toLocaleLowerCase().endsWith(".ics")) {
      setCalendarImportError("请选择 .ics 日历文件");
      return;
    }
    if (file.size > 2 * 1024 * 1024) {
      setCalendarImportError("日历文件超过 2 MB 限制");
      return;
    }
    if (!nativeRuntimeAvailable) {
      setCalendarImportError("ICS 解析只在 Agent Bar 桌面版中运行");
      return;
    }

    setCalendarImportLoading(true);
    try {
      const preview = await previewNativeCalendarImport(await file.text(), new Date());
      setCalendarPreview(preview);
      setNotice(`已预览 ${preview.events.length} 项日历安排`);
    } catch (error) {
      setCalendarImportError(String(error));
    } finally {
      setCalendarImportLoading(false);
    }
  }

  async function confirmCalendarImport() {
    if (!calendarPreview || calendarImportSaving) return;
    const imported: ScheduleBlock[] = calendarPreview.events.map((event) => ({
      id: event.id,
      title: event.title,
      startMinute: event.startMinute,
      endMinute: event.endMinute,
      category: event.allDay ? "admin" : "meeting",
      source: "calendar",
      status: "planned",
      locked: true,
    }));
    const incomingIds = new Set(imported.map((block) => block.id));
    const merged = [
      ...schedule.filter((block) => !incomingIds.has(block.id)),
      ...imported,
    ].sort((left, right) => left.startMinute - right.startMinute || left.endMinute - right.endMinute);

    setCalendarImportSaving(true);
    try {
      if (nativeRuntimeAvailable) await persistNativeSchedule(new Date(), merged);
      setSchedule(merged);
      setSelectedTimelineId(imported[0]?.id ?? null);
      calendarDialogRef.current?.close();
      setNotice(`已导入 ${imported.length} 项日历安排${calendarConflictCount ? `，其中 ${calendarConflictCount} 项有时间冲突` : ""}`);
    } catch (error) {
      setCalendarImportError(`保存失败：${String(error)}`);
    } finally {
      setCalendarImportSaving(false);
    }
  }

  function addExcludedApp() {
    const candidate = excludedAppInput.trim();
    if (!candidate) return;
    if (candidate.length > 128) {
      setNotice("应用名称不能超过 128 个字符");
      return;
    }
    if (privacyExclusionsDraft.some((app) => app.toLocaleLowerCase() === candidate.toLocaleLowerCase())) {
      setNotice("该应用已在排除列表中");
      return;
    }
    setPrivacyExclusionsDraft((items) => [...items, candidate].sort((a, b) => a.localeCompare(b)));
    setExcludedAppInput("");
  }

  async function savePrivacySettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (privacySaving) return;
    setPrivacySaving(true);
    try {
      let deletedRecords = 0;
      let savedApps = privacyExclusionsDraft;
      let savedRetention = privacyRetentionDraft;
      if (nativeRuntimeAvailable) {
        const update = await persistActivityPrivacy(privacyExclusionsDraft, privacyRetentionDraft);
        await persistActivityIdleThreshold(privacyIdleThresholdDraft);
        await persistWindowTitleCapture(privacyTitlesDraft);
        await persistTrackingEnabled(privacyTrackingDraft);
        deletedRecords = update.deletedRecords;
        savedApps = update.excludedActivityApps;
        savedRetention = update.activityRetentionDays;
        const [state, summary, captureState] = await Promise.all([
          loadNativeLocalState(new Date()),
          loadNativeActivityWeekSummary(new Date()),
          loadNativeActivityCaptureState().catch(() => null),
        ]);
        setActivity(storedActivitiesToTimeline(state.activityRecords));
        setWeekSummary(summary);
        setActivityCapture(captureState ?? unavailableCaptureState(privacyIdleThresholdDraft));
      } else {
        setActivityCapture((current) => ({
          ...current,
          status: privacyTrackingDraft ? idleDemo ? "idle" : "active" : "paused",
          thresholdMinutes: privacyIdleThresholdDraft,
          captureAllowed: privacyTrackingDraft && !idleDemo,
          checkedAtMs: Date.now(),
        }));
      }
      setTracking(privacyTrackingDraft);
      setCaptureWindowTitles(privacyTitlesDraft);
      setExcludedActivityApps(savedApps);
      setActivityRetentionDays(savedRetention);
      setActivityIdleThresholdMinutes(privacyIdleThresholdDraft);
      setApprovals((items) => items.filter((item) => item.kind !== "activity-capture"));
      privacyDialogRef.current?.close();
      setNotice(deletedRecords > 0 ? `隐私设置已保存，并删除 ${deletedRecords} 条活动记录` : "隐私设置已保存");
    } catch (error) {
      setNotice(`隐私设置保存失败：${String(error)}`);
    } finally {
      setPrivacySaving(false);
    }
  }

  async function clearActivityHistory() {
    try {
      const deleted = nativeRuntimeAvailable
        ? await clearNativeActivityRecords()
        : activity.length;
      setActivity([]);
      setWeekSummary((days) => days.map((day) => ({
        ...day,
        actualMinutes: 0,
        categories: emptyCategoryMinutes(),
        topApps: [],
      })));
      setConfirmClearActivity(false);
      setNotice(`已清空 ${deleted} 条活动记录`);
    } catch (error) {
      setNotice(`活动历史清除失败：${String(error)}`);
    }
  }

  function handlePrivacyBackdrop(event: MouseEvent<HTMLDialogElement>) {
    if (!("closedBy" in HTMLDialogElement.prototype) && event.target === event.currentTarget) {
      event.currentTarget.close();
    }
  }

  async function toggleWindowMode() {
    const next = !compactMode;
    privacyDialogRef.current?.close();
    calendarDialogRef.current?.close();
    try {
      if (nativeRuntimeAvailable) {
        await setNativeWindowMode(next ? "compact" : "expanded");
      }
      setCompactMode(next);
      setNotice(next ? "已切换到桌面状态条" : "已展开完整工作台");
    } catch (error) {
      setNotice(`窗口模式切换失败：${String(error)}`);
    }
  }

  return (
    <div className={`app-shell ${focusMode ? "focus-mode" : ""} ${compactMode ? "compact-mode" : ""}`}>
      <a className="skip-link" href="#main-content">跳到主要内容</a>

      <header className="desktop-bar">
        <div className="brand-block">
          <div className="brand-mark" aria-hidden="true"><Activity size={20} /></div>
          <div><strong>Agent Bar</strong><span className={`capture-status ${activityCapture.status}`}>{captureStatusLabels[activityCapture.status]}</span></div>
        </div>

        <div className="now-block">
          <time dateTime={now.toISOString()}>{formatClock(now)}</time>
          <span>{formatDate(now)}</span>
        </div>

        <div className="day-progress" aria-label={`今天已过去 ${Math.round(dayProgress)}%`}>
          <div className="progress-copy"><span>今日进度</span><strong>{Math.round(dayProgress)}%</strong></div>
          <div className="progress-track"><i style={{ width: `${dayProgress}%` }} /></div>
        </div>

        <div className="bar-current">
          <span className="status-beacon" />
          <div><span>进行中</span><strong>Agent Bar 原型</strong></div>
          <small>还剩 55 分钟</small>
        </div>

        <div className="bar-next">
          <ChevronRight size={17} aria-hidden="true" />
          <div><span>下一项 · {nextTimeLabel}</span><strong>{nextTitle}</strong></div>
        </div>

        <div className="bar-actions">
          <button className="icon-button window-mode-button" type="button" onClick={() => void toggleWindowMode()} aria-label={compactMode ? "展开完整工作台" : "切换到桌面状态条"} data-tooltip={compactMode ? "展开工作台" : "桌面状态条"}>
            {compactMode ? <Maximize2 size={18} /> : <Minimize2 size={18} />}
          </button>
          <button className={`icon-button tracking ${tracking ? "active" : ""} ${tracking && !activityCapture.captureAllowed ? "auto-paused" : ""}`} type="button" onClick={() => void toggleTracking()} aria-pressed={tracking} aria-label={tracking ? `暂停活动记录，当前${captureStatusLabels[activityCapture.status]}` : "继续活动记录"} data-tooltip={tracking ? captureStatusLabels[activityCapture.status] : "继续活动记录"}>
            {tracking ? <CirclePause size={19} /> : <Play size={19} />}
          </button>
          <button className={`icon-button approval-button ${approvals.length ? "has-alert" : ""}`} type="button" aria-label={`${approvals.length} 个待批准操作`} data-tooltip="待批准操作">
            <ShieldCheck size={19} />
            {approvals.length > 0 && <span>{approvals.length}</span>}
          </button>
          <button className="icon-button" type="button" onClick={openPrivacySettings} aria-label="隐私与数据设置" data-tooltip="隐私与数据"><Settings size={19} /></button>
        </div>
      </header>

      <div className="app-body">
        <nav className="side-nav" aria-label="主要功能">
          <button className="active" type="button" aria-label="今日工作台" data-tooltip="今日工作台"><LayoutDashboard size={20} /></button>
          <button type="button" onClick={openCalendarImport} aria-label="导入日历" data-tooltip="导入日历"><CalendarRange size={20} /></button>
          <button type="button" aria-label="Agent" data-tooltip="Agent"><Bot size={20} /></button>
          <button type="button" aria-label="工作流" data-tooltip="工作流"><Workflow size={20} /></button>
          <div className="nav-spacer" />
          <button type="button" aria-label="更多" data-tooltip="更多"><MoreHorizontal size={20} /></button>
        </nav>

        <main id="main-content" tabIndex={-1}>
          <div className="workspace-header">
            <div>
              <span className="eyebrow">{formatWorkspaceDate(now)}</span>
              <h1>把今天过清楚</h1>
              <p>{notice}</p>
            </div>
            <div className="workspace-tools">
              <div className="segmented-control" aria-label="时间范围">
                <button type="button" className={view === "today" ? "active" : ""} aria-pressed={view === "today"} onClick={() => setView("today")}>今天</button>
                <button type="button" className={view === "week" ? "active" : ""} aria-pressed={view === "week"} onClick={() => setView("week")}>本周</button>
              </div>
              <button className="command-button" type="button" onClick={openMorningPlanner}><Sparkles size={17} />晨间规划{morningTasks.length > 0 && <span className="command-count">{morningTasks.length}</span>}</button>
              <button className="command-button" type="button" onClick={openScreenshotImport}><ScanText size={17} />截图提取</button>
              <button className="command-button" type="button" onClick={openCalendarImport}><CalendarPlus size={17} />导入日历</button>
              <button className={`command-button ${focusMode ? "active" : ""}`} type="button" aria-pressed={focusMode} onClick={() => setFocusMode(!focusMode)}><Focus size={17} />专注模式</button>
            </div>
          </div>

          {approvals.length > 0 && (
            <section className="approval-strip" aria-labelledby="approval-heading">
              <div className="approval-icon"><Hand size={20} /></div>
              <div className="approval-copy">
                <span className="eyebrow">NEEDS YOUR ATTENTION</span>
                <h2 id="approval-heading">{approvals[0].title}</h2>
                <p>{approvals[0].detail}</p>
              </div>
              <div className="approval-actions">
                <button type="button" className="ghost-button" onClick={() => void resolveApproval(false)}><X size={16} />拒绝</button>
                <button type="button" className="primary-button" onClick={() => void resolveApproval(true)}><Check size={16} />{approvals[0].kind === "agent-tool" ? "仅本次批准" : "批准并启用"}</button>
              </div>
            </section>
          )}

          {showMorningReminder && (
            <section className="morning-reminder-strip" aria-labelledby="morning-reminder-heading">
              <div className="morning-reminder-icon"><AlarmClock size={20} /></div>
              <div className="morning-reminder-copy">
                <span className="eyebrow">PLAN YOUR DAY</span>
                <h2 id="morning-reminder-heading">今天还没规划</h2>
                <p>{morningTasks.length} 项待安排 · {schedule.length} 项已有日程</p>
              </div>
              <div className="morning-reminder-actions">
                <button type="button" className="primary-button" onClick={openMorningPlanner}><Sparkles size={16} />开始规划</button>
                <label className="reminder-snooze-control" htmlFor="morning-reminder-snooze">
                  <span>延后</span>
                  <select id="morning-reminder-snooze" value={snoozeMinutes} onChange={(event) => setSnoozeMinutes(Number(event.target.value))}>
                    {[15, 30, 60].map((minute) => <option key={minute} value={minute}>{minute} 分钟</option>)}
                  </select>
                </label>
                <button type="button" className="ghost-button" onClick={() => void snoozeMorningReminder()}>稍后提醒</button>
                <button type="button" className="ghost-button" onClick={() => void dismissMorningReminderToday()}>今天忽略</button>
              </div>
            </section>
          )}

          <div className="primary-grid">
            <div className="schedule-column">
              {view === "today" ? (
                <Timeline schedule={schedule} activity={activity} currentMinute={currentMinute} selectedId={selectedTimelineId} onSelect={setSelectedTimelineId} />
              ) : (
                <div className="week-view-stack">
                  <WeekSchedule days={visibleWeekSchedule} today={todayKey} loading={weekSummaryLoading} />
                  <WeekOverview days={visibleWeekSummary} today={todayKey} loading={weekSummaryLoading} />
                </div>
              )}

              <div className="lower-grid">
                <section className="planner-section" aria-labelledby="planner-heading">
                  <header className="section-heading compact">
                    <div><span className="eyebrow">PLANNER</span><h2 id="planner-heading">剩余时间建议</h2></div>
                    <Sparkles size={18} aria-hidden="true" />
                  </header>
                  {morningTasks.length ? (
                    <ul className="suggestion-list">
                      {morningTasks.map((task) => (
                        <li key={task.id}>
                          <div className={`suggestion-time ${task.category}`}>
                            <strong>{task.priority === "critical" ? "必须" : task.priority === "high" ? "优先" : "待排"}</strong>
                            <span>{task.durationMinutes} 分钟</span>
                          </div>
                          <div className="suggestion-copy"><strong>{task.title}</strong><p>{task.notes || "等待排入今天的空闲时段"}</p></div>
                          <button className="icon-button accept" type="button" onClick={openMorningPlanner} aria-label={`规划任务：${task.title}`} data-tooltip="打开规划"><ChevronRight size={18} /></button>
                        </li>
                      ))}
                    </ul>
                  ) : <div className="empty-state"><Check size={20} /><span>待办已全部排入时间轴</span></div>}
                </section>

                <TimeSummary summary={todaySummary} tracking={tracking} captureStatus={activityCapture.status} />
              </div>

              {selectedItem && (
                <div className="selection-toast" role="status">
                  <Clock3 size={16} /><strong>{selectedItem.title}</strong><span>{selectedItem.detail}</span>
                  <button type="button" onClick={() => setSelectedTimelineId(null)} aria-label="关闭时间项详情"><X size={15} /></button>
                </div>
              )}
            </div>

            <aside className="agent-column" aria-labelledby="office-heading">
              <header className="section-heading office-heading">
                <div><span className="eyebrow">AGENT ACTIVITY</span><h2 id="office-heading">协作办公室</h2></div>
                <div className="office-heading-actions">
                  <div className="online-count"><span />{activeAgents.length} 活跃</div>
                  {nativeRuntimeAvailable && (
                    <button className={`icon-button codex-observation ${codexObservation ? "active" : ""}`} type="button" onClick={() => void toggleCodexObservation()} aria-pressed={codexObservation} aria-label={codexObservation ? "停止 Codex 任务观察" : "连接 Codex 任务观察"} data-tooltip={codexObservation ? "停止 Codex 观察" : "连接 Codex"}>
                      {codexObservation ? <Eye size={17} /> : <EyeOff size={17} />}
                    </button>
                  )}
                </div>
              </header>
              {nativeRuntimeAvailable && (
                <form className="managed-task-form" onSubmit={(event) => void submitManagedTask(event)}>
                  <input
                    type="text"
                    value={managedPrompt}
                    onChange={(event) => setManagedPrompt(event.target.value)}
                    maxLength={8000}
                    placeholder="交给 Codex 的任务"
                    aria-label="交给 Codex 的任务"
                    disabled={startingManagedRun}
                  />
                  <button className="icon-button" type="submit" disabled={!managedPrompt.trim() || startingManagedRun} aria-label="启动 Codex 任务" data-tooltip="启动 Codex 任务">
                    <Send size={17} />
                  </button>
                </form>
              )}
              <PixelOffice agents={agents} selectedId={selectedAgent.id} onSelect={setSelectedAgentId} />

              <section className="agent-detail" aria-live="polite">
                <div className="agent-identity">
                  <span className="agent-avatar" style={{ background: selectedAgent.accent }}>{selectedAgent.name.slice(0, 1)}</span>
                  <div><strong>{selectedAgent.name}</strong><span>{selectedAgent.provider}</span></div>
                  <span className={`agent-state ${selectedAgent.status}`}>{statusLabels[selectedAgent.status]}</span>
                </div>
                <h3>{selectedAgent.task}</h3>
                <p>{selectedAgent.detail}</p>
                <div className="agent-meta">
                  <span><AlarmClock size={14} />{selectedAgent.controlMode === "observed" ? `${selectedAgent.elapsedMinutes} 分钟前更新` : `${selectedAgent.elapsedMinutes} 分钟`}</span>
                  <span><Activity size={14} />{selectedAgent.controlMode === "observed" ? "旁路元数据" : "事件流正常"}</span>
                </div>
                <div className="agent-controls" aria-label={`${selectedAgent.name} 控制`}>
                  {selectedAgent.capabilities.includes("pause") && selectedAgent.status !== "idle" && (
                    <button type="button" onClick={() => updateAgentStatus(selectedAgent, selectedAgent.status === "waiting" ? "working" : "waiting", selectedAgent.status === "waiting" ? "Agent 已继续" : "Agent 已暂停")}>
                      {selectedAgent.status === "waiting" ? <Play size={17} /> : <Pause size={17} />}
                      {selectedAgent.status === "waiting" ? "继续" : "暂停"}
                    </button>
                  )}
                  {selectedAgent.capabilities.includes("stop") && (
                    <button type="button" onClick={() => void stopAgent(selectedAgent)}><Square size={16} />终止</button>
                  )}
                  {selectedAgent.capabilities.includes("handoff") && (
                    <button type="button" onClick={() => setNotice("移交功能将在 Codex 适配器接入后启用")}><RotateCcw size={16} />移交</button>
                  )}
                </div>
              </section>

              <div className="agent-roster" aria-label="已连接 Agent">
                {agents.map((agent) => (
                  <button key={agent.id} type="button" className={agent.id === selectedAgent.id ? "selected" : ""} onClick={() => setSelectedAgentId(agent.id)}>
                    <span style={{ background: agent.accent }}>{agent.name.slice(0, 1)}</span>
                    <div><strong>{agent.name}</strong><small>{statusLabels[agent.status]}</small></div>
                  </button>
                ))}
              </div>
            </aside>
          </div>
        </main>
      </div>

      <MorningPlannerDialog
        open={morningPlannerOpen}
        tasks={morningTasks}
        schedule={schedule}
        currentMinute={currentMinute}
        reminderEnabled={morningReminder.enabled}
        reminderMinute={morningReminder.promptMinute}
        onTasksChange={setMorningTasks}
        onReminderChange={(enabled, promptMinute) => void changeMorningReminder(enabled, promptMinute)}
        onClose={() => setMorningPlannerOpen(false)}
        onConfirm={confirmMorningPlan}
      />

      <ScreenshotImportDialog
        open={screenshotImportOpen}
        snapshot={screenshotImport}
        onAnalyze={analyzeScreenshot}
        onCancelAnalysis={cancelScreenshotAnalysis}
        onClose={() => setScreenshotImportOpen(false)}
        onConfirm={confirmScreenshotTasks}
      />

      <dialog
        ref={calendarDialogRef}
        className="privacy-dialog calendar-import-dialog"
        aria-labelledby="calendar-import-title"
        onClick={handlePrivacyBackdrop}
        onClose={() => setCalendarImportError("")}
      >
        <div className="privacy-dialog-shell">
          <header className="privacy-dialog-header">
            <div><span className="eyebrow">CALENDAR SOURCES</span><h2 id="calendar-import-title">日历连接</h2></div>
            <button className="icon-button" type="button" onClick={() => calendarDialogRef.current?.close()} aria-label="关闭日历连接" data-tooltip="关闭"><X size={18} /></button>
          </header>

          <div className="calendar-dialog-tabs" aria-label="日历接入方式">
            <button type="button" className={calendarDialogView === "connections" ? "active" : ""} aria-pressed={calendarDialogView === "connections"} onClick={() => setCalendarDialogView("connections")}><Link2 size={15} />持续连接</button>
            <button type="button" className={calendarDialogView === "file-import" ? "active" : ""} aria-pressed={calendarDialogView === "file-import"} onClick={() => setCalendarDialogView("file-import")}><FileUp size={15} />导入一次</button>
          </div>

          {calendarDialogView === "connections" ? (
            <div className="calendar-connections-view">
              <section className="calendar-connection-list" aria-labelledby="calendar-connections-heading">
                <div className="calendar-section-heading">
                  <div><h3 id="calendar-connections-heading">已连接</h3><span>{calendarConnections.filter((connection) => connection.enabled).length} 个自动同步</span></div>
                </div>
                {calendarConnections.length > 0 ? (
                  <ul>
                    {calendarConnections.map((connection) => {
                      const busy = calendarConnectionBusyId === connection.id;
                      return (
                        <li key={connection.id} className={!connection.enabled ? "paused" : ""}>
                          <span className={`calendar-connection-icon ${connection.kind}`} aria-hidden="true">{connection.kind === "local-file" ? <FolderOpen size={18} /> : <Link2 size={18} />}</span>
                          <div className="calendar-connection-main">
                            <div><strong>{connection.displayName}</strong><span>{connection.kind === "local-file" ? "本地文件" : "ICS 订阅"}</span></div>
                            <small>{connection.sourceHint} · {connection.enabled ? `${connection.refreshMinutes} 分钟同步` : "已暂停"}</small>
                            <small className={`calendar-sync-status ${connection.lastSyncStatus}`}>
                              {connection.lastSyncStatus === "error" ? <AlertTriangle size={12} /> : connection.lastSyncStatus === "success" ? <Check size={12} /> : <Clock3 size={12} />}
                              {connection.lastSyncStatus === "error" ? connection.lastError || "同步失败" : formatCalendarSyncTime(connection.lastSyncAtMs)}
                            </small>
                          </div>
                          <div className="calendar-connection-actions">
                            <button className="icon-button" type="button" disabled={Boolean(calendarConnectionBusyId) || !connection.enabled} onClick={() => void syncCalendarConnection(connection)} aria-label={`立即同步 ${connection.displayName}`} data-tooltip="立即同步"><RefreshCw size={15} className={busy ? "spin" : ""} /></button>
                            <button className="icon-button" type="button" disabled={Boolean(calendarConnectionBusyId)} onClick={() => void toggleCalendarConnection(connection)} aria-label={`${connection.enabled ? "暂停" : "继续"} ${connection.displayName}`} data-tooltip={connection.enabled ? "暂停" : "继续"}>{connection.enabled ? <Pause size={15} /> : <Play size={15} />}</button>
                            <button className={`icon-button ${calendarDeleteConfirmId === connection.id ? "danger" : ""}`} type="button" disabled={Boolean(calendarConnectionBusyId)} onClick={() => void removeCalendarConnection(connection)} aria-label={`${calendarDeleteConfirmId === connection.id ? "确认删除" : "删除"} ${connection.displayName}`} data-tooltip={calendarDeleteConfirmId === connection.id ? "再次点击确认" : "删除连接"}>{calendarDeleteConfirmId === connection.id ? <Check size={15} /> : <Trash2 size={15} />}</button>
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                ) : <p className="calendar-preview-empty">还没有持续同步的日历</p>}
              </section>

              <form className="calendar-connection-form" onSubmit={(event) => void saveCalendarConnection(event)}>
                <fieldset>
                  <legend>添加连接</legend>
                  <div className="calendar-kind-options">
                    <label className={calendarConnectionKind === "ics-subscription" ? "selected" : ""} htmlFor="calendar-kind-subscription"><input id="calendar-kind-subscription" name="calendar-kind" type="radio" value="ics-subscription" checked={calendarConnectionKind === "ics-subscription"} onChange={() => { setCalendarConnectionKind("ics-subscription"); setCalendarConnectionSource(""); }} /><Link2 size={16} /><span><strong>私有 ICS</strong><small>Google、Outlook 等</small></span></label>
                    <label className={calendarConnectionKind === "local-file" ? "selected" : ""} htmlFor="calendar-kind-file"><input id="calendar-kind-file" name="calendar-kind" type="radio" value="local-file" checked={calendarConnectionKind === "local-file"} onChange={() => { setCalendarConnectionKind("local-file"); setCalendarConnectionSource(""); }} /><FolderOpen size={16} /><span><strong>本地文件</strong><small>自动读取 .ics 更新</small></span></label>
                  </div>

                  <div className="calendar-form-grid">
                    <label htmlFor="calendar-connection-name"><span>显示名称</span><input id="calendar-connection-name" name="displayName" value={calendarConnectionName} onChange={(event) => { setCalendarConnectionName(event.target.value); setCalendarConnectionError(""); }} maxLength={80} required /></label>
                    <label htmlFor="calendar-connection-refresh"><span>同步间隔</span><select id="calendar-connection-refresh" name="refreshMinutes" value={calendarConnectionRefresh} onChange={(event) => setCalendarConnectionRefresh(Number(event.target.value))}><option value={15}>15 分钟</option><option value={30}>30 分钟</option><option value={60}>1 小时</option><option value={180}>3 小时</option><option value={1440}>每天</option></select></label>
                  </div>

                  {calendarConnectionKind === "ics-subscription" ? (
                    <label className="calendar-source-field" htmlFor="calendar-connection-source">
                      <span>私有 ICS 订阅地址</span>
                      <small id="calendar-source-help">完整地址仅保存到 Windows 凭据管理器</small>
                      <div><input id="calendar-connection-source" name="source" type={calendarConnectionSourceVisible ? "url" : "password"} value={calendarConnectionSource} onChange={(event) => { setCalendarConnectionSource(event.target.value); setCalendarConnectionError(""); }} autoComplete="off" aria-describedby="calendar-source-help" required /><button className="icon-button" type="button" onClick={() => setCalendarConnectionSourceVisible((visible) => !visible)} aria-label={calendarConnectionSourceVisible ? "隐藏订阅地址" : "显示订阅地址"} data-tooltip={calendarConnectionSourceVisible ? "隐藏" : "显示"}>{calendarConnectionSourceVisible ? <EyeOff size={16} /> : <Eye size={16} />}</button></div>
                    </label>
                  ) : (
                    <label className="calendar-source-field" htmlFor="calendar-connection-source">
                      <span>本地 .ics 文件</span>
                      <small id="calendar-source-help">路径仅保存到 Windows 凭据管理器</small>
                      <div><input id="calendar-connection-source" name="source" value={calendarConnectionSource} onChange={(event) => { setCalendarConnectionSource(event.target.value); setCalendarConnectionError(""); }} aria-describedby="calendar-source-help" required /><button className="icon-button" type="button" onClick={() => void chooseCalendarConnectionFile()} aria-label="选择本地 ICS 文件" data-tooltip="选择文件"><FolderOpen size={16} /></button></div>
                    </label>
                  )}

                  {calendarConnectionError && <p className="calendar-import-error" role="alert"><AlertTriangle size={15} />{calendarConnectionError}</p>}
                  <button className="primary-button calendar-connect-submit" type="submit" disabled={calendarConnectionSaving}><Plus size={16} />{calendarConnectionSaving ? "连接中" : "保存并同步"}</button>
                </fieldset>
              </form>
            </div>
          ) : (
            <>
              <section className="calendar-file-section">
                <input ref={calendarFileRef} className="calendar-file-input" type="file" accept=".ics,text/calendar" onChange={(event) => void selectCalendarFile(event.target.files?.[0])} />
                <button className="calendar-file-command" type="button" onClick={() => calendarFileRef.current?.click()} disabled={calendarImportLoading}>
                  <FileUp size={19} />
                  <span><strong>{calendarImportLoading ? "解析中" : calendarFileName || "选择 .ics 文件"}</strong><small>原始内容仅在内存中解析</small></span>
                </button>
                {calendarImportError && <p className="calendar-import-error" role="alert"><AlertTriangle size={15} />{calendarImportError}</p>}
              </section>

              {calendarPreview && (
                <section className="calendar-preview-section" aria-live="polite">
                  <div className="calendar-preview-summary">
                    <div><span>{calendarPreview.sourceName}</span><strong>{calendarPreview.events.length} 项</strong></div>
                    <div><span>时间冲突</span><strong className={calendarConflictCount ? "warning" : ""}>{calendarConflictCount}</strong></div>
                    <div><span>已跳过</span><strong>{calendarPreview.skippedCount}</strong></div>
                  </div>
                  {calendarPreviewEvents.length > 0 ? (
                    <ul className="calendar-preview-list">
                      {calendarPreviewEvents.map((event) => (
                        <li key={event.id} className={event.conflicts.length ? "has-conflict" : ""}>
                          <time>{event.allDay ? "全天" : `${formatMinute(event.startMinute)} - ${formatMinute(event.endMinute)}`}</time>
                          <div><strong>{event.title}</strong><span>{event.conflicts.length ? `与“${event.conflicts[0].title}”冲突` : "可导入"}</span></div>
                          <div className="calendar-event-flags">{event.recurring && <span title="重复事件"><Repeat2 size={13} />重复</span>}{event.conflicts.length > 0 && <span className="conflict"><AlertTriangle size={13} />冲突</span>}</div>
                        </li>
                      ))}
                    </ul>
                  ) : <p className="calendar-preview-empty">当天没有可导入的事件</p>}
                  {calendarPreview.warnings.length > 0 && <ul className="calendar-warning-list">{calendarPreview.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>}
                </section>
              )}

              <footer className="privacy-dialog-footer">
                <button className="ghost-button" type="button" onClick={() => calendarDialogRef.current?.close()}>取消</button>
                <button className="primary-button" type="button" disabled={!calendarPreview?.events.length || calendarImportSaving} onClick={() => void confirmCalendarImport()}><Check size={16} />{calendarImportSaving ? "导入中" : "确认导入"}</button>
              </footer>
            </>
          )}
        </div>
      </dialog>

      <dialog
        ref={privacyDialogRef}
        className="privacy-dialog"
        aria-labelledby="privacy-dialog-title"
        onClick={handlePrivacyBackdrop}
        onClose={() => setConfirmClearActivity(false)}
        {...{ closedby: "any" }}
      >
        <form className="privacy-dialog-shell" onSubmit={(event) => void savePrivacySettings(event)}>
          <header className="privacy-dialog-header">
            <div><span className="eyebrow">LOCAL DATA</span><h2 id="privacy-dialog-title">隐私与活动记录</h2></div>
            <button className="icon-button" type="button" onClick={() => privacyDialogRef.current?.close()} aria-label="关闭隐私设置" data-tooltip="关闭"><X size={18} /></button>
          </header>

          <section className="privacy-setting-section" aria-labelledby="capture-settings-heading">
            <div className="privacy-section-heading"><h3 id="capture-settings-heading">采集范围</h3><span>{privacyTrackingDraft ? "记录中" : "已暂停"}</span></div>
            <label className="privacy-toggle-row">
              <span><strong>前台应用活动</strong><small>保存应用名称和活动时间</small></span>
              <input type="checkbox" checked={privacyTrackingDraft} onChange={(event) => setPrivacyTrackingDraft(event.target.checked)} />
            </label>
            <label className="privacy-toggle-row">
              <span><strong>脱敏窗口标题</strong><small>关闭后只保存应用名称</small></span>
              <input type="checkbox" checked={privacyTitlesDraft} onChange={(event) => setPrivacyTitlesDraft(event.target.checked)} />
            </label>
            <label className="retention-row privacy-idle-row">
              <span><strong>空闲自动暂停</strong><small>锁屏或会话断开时自动暂停</small></span>
              <select value={privacyIdleThresholdDraft} onChange={(event) => setPrivacyIdleThresholdDraft(Number(event.target.value))}>
                {[1, 3, 5, 10, 15, 30].map((minutes) => <option key={minutes} value={minutes}>{minutes} 分钟</option>)}
              </select>
            </label>
          </section>

          <section className="privacy-setting-section" aria-labelledby="exclusions-heading">
            <div className="privacy-section-heading"><h3 id="exclusions-heading">排除应用</h3><span>{privacyExclusionsDraft.length}/100</span></div>
            <div className="exclusion-entry">
              <input
                type="text"
                value={excludedAppInput}
                onChange={(event) => setExcludedAppInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    addExcludedApp();
                  }
                }}
                maxLength={128}
                placeholder="例如 WeChat.exe"
                aria-label="要排除的应用名称"
              />
              <button className="icon-button" type="button" onClick={addExcludedApp} disabled={!excludedAppInput.trim()} aria-label="添加排除应用" data-tooltip="添加"><Plus size={17} /></button>
            </div>
            {privacyExclusionsDraft.length > 0 ? (
              <ul className="exclusion-list">
                {privacyExclusionsDraft.map((app) => (
                  <li key={app.toLocaleLowerCase()}><span>{app}</span><button className="icon-button" type="button" onClick={() => setPrivacyExclusionsDraft((items) => items.filter((item) => item !== app))} aria-label={`不再排除 ${app}`} data-tooltip="移除"><Trash2 size={15} /></button></li>
                ))}
              </ul>
            ) : <p className="privacy-empty">当前没有排除应用</p>}
            <p className="privacy-consequence">保存后停止采集这些应用，并删除它们已有的活动记录。</p>
          </section>

          <section className="privacy-setting-section" aria-labelledby="retention-heading">
            <div className="privacy-section-heading"><h3 id="retention-heading">数据生命周期</h3><span>仅本地</span></div>
            <label className="retention-row"><span><strong>活动记录保留</strong><small>超过期限后自动删除</small></span><select value={privacyRetentionDraft} onChange={(event) => setPrivacyRetentionDraft(Number(event.target.value))}><option value={7}>7 天</option><option value={30}>30 天</option><option value={90}>90 天</option><option value={365}>1 年</option></select></label>
            {!confirmClearActivity ? (
              <button className="danger-command" type="button" onClick={() => setConfirmClearActivity(true)}><Trash2 size={16} />清空活动历史</button>
            ) : (
              <div className="clear-confirmation" role="alert"><span>这会立即删除全部活动记录。</span><div><button className="ghost-button" type="button" onClick={() => setConfirmClearActivity(false)}>取消</button><button className="danger-command" type="button" onClick={() => void clearActivityHistory()}><Trash2 size={15} />确认清空</button></div></div>
            )}
          </section>

          <footer className="privacy-dialog-footer">
            <button className="ghost-button" type="button" onClick={() => privacyDialogRef.current?.close()}>取消</button>
            <button className="primary-button" type="submit" disabled={privacySaving}><Check size={16} />{privacySaving ? "保存中" : "保存设置"}</button>
          </footer>
        </form>
      </dialog>
    </div>
  );
}

export default App;
