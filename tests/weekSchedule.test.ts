import assert from "node:assert/strict";
import test from "node:test";
import { positionWeekBlocks } from "../src/lib/weekSchedule.ts";
import type { ScheduleBlock } from "../src/types.ts";

function block(id: string, startMinute: number, endMinute: number): ScheduleBlock {
  return {
    id,
    title: id,
    startMinute,
    endMinute,
    category: "focus",
    source: "manual",
    status: "planned",
  };
}

test("reuses a lane for non-overlapping weekly events", () => {
  const result = positionWeekBlocks([
    block("later", 600, 660),
    block("earlier", 480, 540),
  ]);

  assert.deepEqual(result.positioned.map(({ block: item, lane }) => [item.id, lane]), [
    ["earlier", 0],
    ["later", 0],
  ]);
  assert.equal(result.laneCount, 1);
  assert.equal(result.hiddenCount, 0);
});

test("stacks overlapping events without mutating the input", () => {
  const input = [block("long", 480, 720), block("overlap", 540, 600)];
  const snapshot = structuredClone(input);
  const result = positionWeekBlocks(input);

  assert.deepEqual(result.positioned.map(({ lane }) => lane), [0, 1]);
  assert.equal(result.laneCount, 2);
  assert.deepEqual(input, snapshot);
});

test("bounds very dense overlaps and reports hidden events", () => {
  const result = positionWeekBlocks(Array.from({ length: 6 }, (_, index) => (
    block(`event-${index}`, 540, 660)
  )));

  assert.equal(result.positioned.length, 4);
  assert.equal(result.laneCount, 4);
  assert.equal(result.hiddenCount, 2);
});
