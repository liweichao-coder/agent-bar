import assert from "node:assert/strict";
import test from "node:test";
import {
  browserWeekSummary,
  formatDuration,
  localWeekRanges,
  occupiedScheduleMinutes,
  summarizeActivity,
} from "../src/lib/timeStats.ts";
import type { ActivityBlock, ScheduleBlock } from "../src/types.ts";

const activity: ActivityBlock[] = [
  { id: "1", appName: "Code", windowTitle: "A", startMinute: 60, endMinute: 120, category: "focus" },
  { id: "2", appName: "Feishu", windowTitle: "B", startMinute: 120, endMinute: 150, category: "meeting" },
  { id: "3", appName: "Code", windowTitle: "C", startMinute: 150, endMinute: 180, category: "focus" },
];

test("summarizes activity by category and top application", () => {
  const summary = summarizeActivity(activity);
  assert.equal(summary.actualMinutes, 120);
  assert.deepEqual(summary.categories, { focus: 90, meeting: 30, admin: 0, life: 0 });
  assert.deepEqual(summary.topApps, [
    { appName: "Code", minutes: 90 },
    { appName: "Feishu", minutes: 30 },
  ]);
});

test("clips invalid activity bounds to the current day", () => {
  const summary = summarizeActivity([
    { id: "1", appName: "Code", windowTitle: "A", startMinute: -30, endMinute: 30, category: "focus" },
    { id: "2", appName: "Code", windowTitle: "B", startMinute: 1430, endMinute: 1500, category: "focus" },
    { id: "3", appName: "Code", windowTitle: "C", startMinute: 500, endMinute: 400, category: "focus" },
  ]);
  assert.equal(summary.actualMinutes, 40);
});

test("counts overlapping schedule as occupied time only once", () => {
  const blocks: ScheduleBlock[] = [
    { id: "1", title: "A", startMinute: 60, endMinute: 120, category: "focus", source: "manual", status: "planned" },
    { id: "2", title: "B", startMinute: 90, endMinute: 150, category: "meeting", source: "calendar", status: "planned" },
    { id: "3", title: "C", startMinute: 180, endMinute: 210, category: "life", source: "manual", status: "planned" },
  ];
  assert.equal(occupiedScheduleMinutes(blocks), 120);
});

test("builds Monday through Sunday using local day boundaries", () => {
  const ranges = localWeekRanges(new Date(2026, 6, 22, 12));
  assert.deepEqual(ranges.map((range) => range.day), [
    "2026-07-20", "2026-07-21", "2026-07-22", "2026-07-23",
    "2026-07-24", "2026-07-25", "2026-07-26",
  ]);
  assert.ok(ranges.every((range) => range.endMs > range.startMs));
});

test("browser summary places current data on the correct weekday", () => {
  const schedule: ScheduleBlock[] = [
    { id: "1", title: "A", startMinute: 60, endMinute: 120, category: "focus", source: "manual", status: "planned" },
  ];
  const week = browserWeekSummary(new Date(2026, 6, 22, 12), schedule, activity);
  assert.equal(week[2].day, "2026-07-22");
  assert.equal(week[2].plannedMinutes, 60);
  assert.equal(week[2].actualMinutes, 120);
  assert.equal(week[0].actualMinutes, 0);
});

test("formats compact human-readable durations", () => {
  assert.equal(formatDuration(0), "0m");
  assert.equal(formatDuration(45), "45m");
  assert.equal(formatDuration(60), "1h");
  assert.equal(formatDuration(95), "1h 35m");
});
