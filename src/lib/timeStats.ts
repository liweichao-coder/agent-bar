import type { ActivityBlock, ScheduleBlock } from "../types";

export const timeCategories = ["focus", "meeting", "admin", "life"] as const;
export type TimeCategory = typeof timeCategories[number];

export type CategoryMinutes = Record<TimeCategory, number>;

export type AppMinutes = {
  appName: string;
  minutes: number;
};

export type ActivityDaySummary = {
  day: string;
  plannedMinutes: number;
  actualMinutes: number;
  categories: CategoryMinutes;
  topApps: AppMinutes[];
};

export type LocalDayRange = {
  day: string;
  startMs: number;
  endMs: number;
};

export function summarizeActivity(blocks: ActivityBlock[]): Omit<ActivityDaySummary, "day" | "plannedMinutes"> {
  const categories = emptyCategoryMinutes();
  const apps = new Map<string, number>();
  let actualMinutes = 0;

  for (const block of blocks) {
    const minutes = Math.max(0, Math.min(1440, block.endMinute) - Math.max(0, block.startMinute));
    if (!minutes) continue;
    actualMinutes += minutes;
    categories[block.category] += minutes;
    apps.set(block.appName, (apps.get(block.appName) ?? 0) + minutes);
  }

  return {
    actualMinutes,
    categories,
    topApps: [...apps.entries()]
      .map(([appName, minutes]) => ({ appName, minutes }))
      .sort((left, right) => right.minutes - left.minutes || left.appName.localeCompare(right.appName))
      .slice(0, 3),
  };
}

export function occupiedScheduleMinutes(blocks: ScheduleBlock[]) {
  const ranges = blocks
    .map((block) => ({
      start: Math.max(0, Math.min(1440, block.startMinute)),
      end: Math.max(0, Math.min(1440, block.endMinute)),
    }))
    .filter((range) => range.end > range.start)
    .sort((left, right) => left.start - right.start || left.end - right.end);

  let total = 0;
  let currentStart = -1;
  let currentEnd = -1;
  for (const range of ranges) {
    if (currentStart < 0) {
      currentStart = range.start;
      currentEnd = range.end;
    } else if (range.start <= currentEnd) {
      currentEnd = Math.max(currentEnd, range.end);
    } else {
      total += currentEnd - currentStart;
      currentStart = range.start;
      currentEnd = range.end;
    }
  }
  return currentStart < 0 ? 0 : total + currentEnd - currentStart;
}

export function localWeekRanges(date: Date): LocalDayRange[] {
  const weekday = date.getDay();
  const daysSinceMonday = weekday === 0 ? 6 : weekday - 1;
  const monday = new Date(date.getFullYear(), date.getMonth(), date.getDate() - daysSinceMonday);

  return Array.from({ length: 7 }, (_, index) => {
    const start = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + index);
    const end = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + index + 1);
    return { day: localDateKey(start), startMs: start.getTime(), endMs: end.getTime() };
  });
}

export function browserWeekSummary(
  date: Date,
  schedule: ScheduleBlock[],
  activity: ActivityBlock[],
): ActivityDaySummary[] {
  const today = localDateKey(date);
  const activitySummary = summarizeActivity(activity);
  return localWeekRanges(date).map((range) => range.day === today ? {
    day: range.day,
    plannedMinutes: occupiedScheduleMinutes(schedule),
    ...activitySummary,
  } : {
    day: range.day,
    plannedMinutes: 0,
    actualMinutes: 0,
    categories: emptyCategoryMinutes(),
    topApps: [],
  });
}

export function formatDuration(minutes: number) {
  const safeMinutes = Math.max(0, Math.round(minutes));
  const hours = Math.floor(safeMinutes / 60);
  const remainder = safeMinutes % 60;
  if (!hours) return `${remainder}m`;
  return remainder ? `${hours}h ${remainder}m` : `${hours}h`;
}

export function emptyCategoryMinutes(): CategoryMinutes {
  return { focus: 0, meeting: 0, admin: 0, life: 0 };
}

function localDateKey(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
