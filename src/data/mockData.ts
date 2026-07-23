import type {
  ActivityBlock,
  Agent,
  ApprovalRequest,
  PlannerSuggestion,
  ScheduleBlock,
} from "../types";
import type { MorningPlanTask } from "../lib/morningPlanner";

export const scheduleBlocks: ScheduleBlock[] = [
  { id: "s1", title: "晨间计划", startMinute: 450, endMinute: 480, category: "admin", source: "agent", status: "done" },
  { id: "s2", title: "论文实验复现", startMinute: 510, endMinute: 660, category: "focus", source: "manual", status: "done", locked: true },
  { id: "s3", title: "组会", startMinute: 690, endMinute: 750, category: "meeting", source: "calendar", status: "done", locked: true },
  { id: "s4", title: "午餐与散步", startMinute: 750, endMinute: 825, category: "life", source: "manual", status: "done" },
  { id: "s5", title: "Agent Bar 原型", startMinute: 840, endMinute: 1020, category: "focus", source: "agent", status: "active" },
  { id: "s6", title: "整理申请材料", startMinute: 1050, endMinute: 1140, category: "admin", source: "manual", status: "planned" },
  { id: "s7", title: "跑步", startMinute: 1170, endMinute: 1230, category: "life", source: "calendar", status: "planned" },
  { id: "s8", title: "阅读与复盘", startMinute: 1260, endMinute: 1350, category: "focus", source: "agent", status: "planned" },
];

export const activityBlocks: ActivityBlock[] = [
  { id: "a1", appName: "Microsoft Edge", windowTitle: "今日任务 · Notion", startMinute: 455, endMinute: 485, category: "admin" },
  { id: "a2", appName: "Visual Studio Code", windowTitle: "aigc-detection / train.py", startMinute: 520, endMinute: 646, category: "focus" },
  { id: "a3", appName: "Feishu", windowTitle: "课题组周会", startMinute: 690, endMinute: 756, category: "meeting" },
  { id: "a4", appName: "Codex", windowTitle: "Agent Bar · implementation", startMinute: 850, endMinute: 965, category: "focus" },
  { id: "a5", appName: "Figma", windowTitle: "Agent Bar wireframe", startMinute: 965, endMinute: 1005, category: "focus" },
];

export const initialAgents: Agent[] = [
  {
    id: "codex-main",
    name: "Codex",
    provider: "OpenAI",
    task: "实现第一代 Agent Bar",
    detail: "正在构建时间轴与桌面状态条",
    status: "working",
    elapsedMinutes: 34,
    accent: "#72d6a5",
    position: { x: 26, y: 58 },
    capabilities: ["pause", "stop", "handoff", "approve"],
  },
  {
    id: "codex-research",
    name: "Research",
    provider: "Codex subagent",
    task: "整理日历接入方案",
    detail: "正在阅读 Microsoft Graph 文档",
    status: "searching",
    elapsedMinutes: 12,
    accent: "#6eb5ff",
    position: { x: 68, y: 33 },
    capabilities: ["stop", "handoff"],
  },
  {
    id: "claude",
    name: "Claude Code",
    provider: "未连接",
    task: "等待任务",
    detail: "适配器尚未启用",
    status: "idle",
    elapsedMinutes: 0,
    accent: "#f0b36f",
    position: { x: 75, y: 68 },
    capabilities: [],
  },
  {
    id: "reviewer",
    name: "Reviewer",
    provider: "Codex subagent",
    task: "检查隐私边界",
    detail: "等待批准读取本地窗口标题",
    status: "waiting",
    elapsedMinutes: 6,
    accent: "#ff8c7a",
    position: { x: 43, y: 34 },
    capabilities: ["stop", "approve"],
  },
];

export const initialApprovals: ApprovalRequest[] = [
  {
    id: "approval-1",
    agentId: "reviewer",
    kind: "activity-capture",
    title: "读取前台窗口标题",
    detail: "仅在本地读取并应用脱敏规则，不上传原始标题。",
    risk: "medium",
  },
];

export const plannerSuggestions: PlannerSuggestion[] = [
  {
    id: "p1",
    title: "补一段实验记录",
    reason: "上午实验已完成，但结果还没有写入周报。",
    startMinute: 1020,
    endMinute: 1050,
    category: "admin",
  },
  {
    id: "p2",
    title: "晚间留出缓冲",
    reason: "今天已有 4 小时高强度工作，建议缩短复盘并提前休息。",
    startMinute: 1350,
    endMinute: 1380,
    category: "life",
  },
];

export const initialMorningTasks: MorningPlanTask[] = plannerSuggestions.map((suggestion, index) => ({
  id: suggestion.id,
  title: suggestion.title,
  durationMinutes: suggestion.endMinute - suggestion.startMinute,
  priority: index === 0 ? "high" : "normal",
  preferredPeriod: suggestion.startMinute < 720
    ? "morning"
    : suggestion.startMinute < 1080
      ? "afternoon"
      : "evening",
  category: suggestion.category,
  notes: suggestion.reason,
}));

export const weekData = [
  { day: "周一", date: "20", focus: 4.8, meeting: 1.2, life: 2.1, total: 8.1 },
  { day: "周二", date: "21", focus: 4.1, meeting: 1.0, life: 1.8, total: 6.9, today: true },
  { day: "周三", date: "22", focus: 3.5, meeting: 2.0, life: 2.0, total: 7.5 },
  { day: "周四", date: "23", focus: 5.0, meeting: 0.5, life: 1.5, total: 7.0 },
  { day: "周五", date: "24", focus: 3.8, meeting: 1.5, life: 2.2, total: 7.5 },
  { day: "周六", date: "25", focus: 2.0, meeting: 0, life: 3.5, total: 5.5 },
  { day: "周日", date: "26", focus: 1.5, meeting: 0, life: 4.0, total: 5.5 },
];
