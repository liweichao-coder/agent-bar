import type { ScheduleBlock } from "../types";

export type PlannerPriority = "critical" | "high" | "normal" | "low";
export type PlannerPeriod = "any" | "morning" | "afternoon" | "evening";

export type MorningPlanTask = {
  id: string;
  title: string;
  durationMinutes: number;
  priority: PlannerPriority;
  preferredPeriod: PlannerPeriod;
  category: ScheduleBlock["category"];
  notes?: string;
};

export type MorningPlanConfig = {
  dayStartMinute: number;
  dayEndMinute: number;
  bufferMinutes: number;
  granularityMinutes?: number;
};

export type MorningPlanDraftBlock = ScheduleBlock & {
  taskId: string;
  reason: string;
};

export type MorningPlanUnscheduled = {
  task: MorningPlanTask;
  reason: string;
};

export type MorningPlanDraft = {
  blocks: MorningPlanDraftBlock[];
  unscheduled: MorningPlanUnscheduled[];
  occupiedMinutes: number;
  scheduledMinutes: number;
  availableMinutes: number;
};

type Interval = { start: number; end: number };

const priorityRank: Record<PlannerPriority, number> = {
  critical: 4,
  high: 3,
  normal: 2,
  low: 1,
};

const periodRanges: Record<PlannerPeriod, Interval> = {
  any: { start: 0, end: 1440 },
  morning: { start: 360, end: 720 },
  afternoon: { start: 720, end: 1080 },
  evening: { start: 1080, end: 1440 },
};

const periodLabels: Record<PlannerPeriod, string> = {
  any: "任意时段",
  morning: "上午",
  afternoon: "下午",
  evening: "晚间",
};

export function buildMorningPlan(
  tasks: MorningPlanTask[],
  existingSchedule: ScheduleBlock[],
  config: MorningPlanConfig,
): MorningPlanDraft {
  const granularity = config.granularityMinutes ?? 15;
  validateConfig(config, granularity);

  const occupied = existingSchedule
    .filter((block) => block.endMinute > config.dayStartMinute && block.startMinute < config.dayEndMinute)
    .map((block) => ({
      start: Math.max(config.dayStartMinute, block.startMinute),
      end: Math.min(config.dayEndMinute, block.endMinute),
    }));
  const initialOccupiedMinutes = unionDuration(occupied);

  const orderedTasks = tasks
    .map((task, index) => ({ task: normalizeTask(task, granularity), index }))
    .filter(({ task }) => task.title.length > 0)
    .sort((left, right) =>
      priorityRank[right.task.priority] - priorityRank[left.task.priority]
      || right.task.durationMinutes - left.task.durationMinutes
      || left.index - right.index);

  const blocks: MorningPlanDraftBlock[] = [];
  const unscheduled: MorningPlanUnscheduled[] = [];

  for (const { task } of orderedTasks) {
    const preferred = intersect(periodRanges[task.preferredPeriod], {
      start: config.dayStartMinute,
      end: config.dayEndMinute,
    });
    const preferredStart = preferred
      ? findStart(occupied, preferred, task.durationMinutes, config.bufferMinutes, granularity)
      : undefined;
    const fallbackStart = preferredStart === undefined && task.preferredPeriod !== "any"
      ? findStart(
        occupied,
        { start: config.dayStartMinute, end: config.dayEndMinute },
        task.durationMinutes,
        config.bufferMinutes,
        granularity,
      )
      : undefined;
    const startMinute = preferredStart ?? fallbackStart;

    if (startMinute === undefined) {
      unscheduled.push({
        task,
        reason: `没有连续 ${task.durationMinutes} 分钟的可用时间`,
      });
      continue;
    }

    const endMinute = startMinute + task.durationMinutes;
    occupied.push({ start: startMinute, end: endMinute });
    blocks.push({
      id: `planner-${task.id}`,
      taskId: task.id,
      title: task.title,
      startMinute,
      endMinute,
      category: task.category,
      source: "agent",
      status: "planned",
      locked: false,
      reason: preferredStart !== undefined
        ? `${priorityLabel(task.priority)} · ${periodLabels[task.preferredPeriod]}`
        : `${priorityLabel(task.priority)} · 偏好时段已满，使用其他空档`,
    });
  }

  blocks.sort((left, right) => left.startMinute - right.startMinute || left.endMinute - right.endMinute);
  const scheduledMinutes = blocks.reduce(
    (total, block) => total + block.endMinute - block.startMinute,
    0,
  );
  const availableMinutes = freeIntervals(
    occupied,
    { start: config.dayStartMinute, end: config.dayEndMinute },
    config.bufferMinutes,
  ).reduce((total, interval) => total + interval.end - interval.start, 0);

  return {
    blocks,
    unscheduled,
    occupiedMinutes: initialOccupiedMinutes,
    scheduledMinutes,
    availableMinutes,
  };
}

function findStart(
  occupied: Interval[],
  range: Interval,
  duration: number,
  buffer: number,
  granularity: number,
) {
  for (const slot of freeIntervals(occupied, range, buffer)) {
    const start = alignUp(slot.start, granularity);
    if (start + duration <= slot.end) return start;
  }
  return undefined;
}

function freeIntervals(occupied: Interval[], range: Interval, buffer: number) {
  const sorted = occupied
    .filter((interval) => interval.end > range.start && interval.start < range.end)
    .map((interval) => ({
      start: Math.max(range.start, interval.start),
      end: Math.min(range.end, interval.end),
    }))
    .sort((left, right) => left.start - right.start || left.end - right.end);
  const free: Interval[] = [];
  let cursor = range.start;

  for (const interval of sorted) {
    const gapEnd = Math.max(range.start, interval.start - buffer);
    if (gapEnd > cursor) free.push({ start: cursor, end: gapEnd });
    cursor = Math.max(cursor, Math.min(range.end, interval.end + buffer));
  }
  if (cursor < range.end) free.push({ start: cursor, end: range.end });
  return free;
}

function unionDuration(intervals: Interval[]) {
  const sorted = intervals
    .map((interval) => ({ ...interval }))
    .sort((left, right) => left.start - right.start || left.end - right.end);
  let total = 0;
  let current: Interval | undefined;
  for (const interval of sorted) {
    if (!current) {
      current = interval;
    } else if (interval.start <= current.end) {
      current.end = Math.max(current.end, interval.end);
    } else {
      total += current.end - current.start;
      current = interval;
    }
  }
  return total + (current ? current.end - current.start : 0);
}

function normalizeTask(task: MorningPlanTask, granularity: number): MorningPlanTask {
  return {
    ...task,
    title: task.title.trim().replace(/\s+/g, " ").slice(0, 120),
    durationMinutes: Math.min(480, Math.max(15, alignUp(task.durationMinutes, granularity))),
  };
}

function validateConfig(config: MorningPlanConfig, granularity: number) {
  if (!Number.isInteger(config.dayStartMinute)
    || !Number.isInteger(config.dayEndMinute)
    || config.dayStartMinute < 0
    || config.dayEndMinute > 1440
    || config.dayEndMinute <= config.dayStartMinute) {
    throw new Error("规划时段无效");
  }
  if (!Number.isInteger(config.bufferMinutes) || config.bufferMinutes < 0 || config.bufferMinutes > 120) {
    throw new Error("缓冲时间无效");
  }
  if (!Number.isInteger(granularity) || granularity < 5 || granularity > 60) {
    throw new Error("时间粒度无效");
  }
}

function intersect(left: Interval, right: Interval): Interval | undefined {
  const start = Math.max(left.start, right.start);
  const end = Math.min(left.end, right.end);
  return end > start ? { start, end } : undefined;
}

function alignUp(value: number, step: number) {
  return Math.ceil(value / step) * step;
}

function priorityLabel(priority: PlannerPriority) {
  switch (priority) {
    case "critical": return "必须完成";
    case "high": return "高优先级";
    case "normal": return "普通优先级";
    case "low": return "低优先级";
  }
}
