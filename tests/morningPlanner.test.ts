import assert from "node:assert/strict";
import test from "node:test";
import { buildMorningPlan, type MorningPlanTask } from "../src/lib/morningPlanner.ts";
import type { ScheduleBlock } from "../src/types.ts";

const task = (overrides: Partial<MorningPlanTask> = {}): MorningPlanTask => ({
  id: "task-1",
  title: "完成原型",
  durationMinutes: 60,
  priority: "normal",
  preferredPeriod: "any",
  category: "focus",
  ...overrides,
});

const block = (overrides: Partial<ScheduleBlock> = {}): ScheduleBlock => ({
  id: "fixed-1",
  title: "固定日程",
  startMinute: 600,
  endMinute: 660,
  category: "meeting",
  source: "calendar",
  status: "planned",
  locked: true,
  ...overrides,
});

const config = { dayStartMinute: 480, dayEndMinute: 1080, bufferMinutes: 15 };

test("preserves existing blocks and leaves transition buffers", () => {
  const draft = buildMorningPlan([task()], [block()], config);

  assert.equal(draft.blocks.length, 1);
  assert.equal(draft.blocks[0].startMinute, 480);
  assert.equal(draft.blocks[0].endMinute, 540);
  assert.equal(draft.occupiedMinutes, 60);
});

test("schedules higher priority work before lower priority work", () => {
  const draft = buildMorningPlan([
    task({ id: "low", title: "低优先级", priority: "low" }),
    task({ id: "critical", title: "关键任务", priority: "critical" }),
  ], [], config);

  assert.equal(draft.blocks[0].taskId, "critical");
  assert.equal(draft.blocks[1].taskId, "low");
  assert.ok(draft.blocks[1].startMinute >= draft.blocks[0].endMinute + 15);
});

test("uses preferred period and falls back when it is full", () => {
  const fullMorning = block({ startMinute: 480, endMinute: 720 });
  const draft = buildMorningPlan([
    task({ preferredPeriod: "morning" }),
  ], [fullMorning], config);

  assert.equal(draft.blocks[0].startMinute, 735);
  assert.match(draft.blocks[0].reason, /使用其他空档/);
});

test("returns an explicit unscheduled item instead of overlapping", () => {
  const draft = buildMorningPlan([
    task({ durationMinutes: 180 }),
  ], [block({ startMinute: 480, endMinute: 1080 })], config);

  assert.equal(draft.blocks.length, 0);
  assert.equal(draft.unscheduled.length, 1);
  assert.match(draft.unscheduled[0].reason, /没有连续 180 分钟/);
});

test("does not mutate task or schedule inputs", () => {
  const tasks = [task({ title: "  带空格的任务  " })];
  const schedule = [block()];
  const snapshot = JSON.stringify({ tasks, schedule });

  buildMorningPlan(tasks, schedule, config);

  assert.equal(JSON.stringify({ tasks, schedule }), snapshot);
});

test("rejects invalid planning windows", () => {
  assert.throws(
    () => buildMorningPlan([task()], [], { ...config, dayEndMinute: 400 }),
    /规划时段无效/,
  );
});
