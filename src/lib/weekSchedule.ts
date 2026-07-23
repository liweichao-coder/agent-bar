import type { ScheduleBlock } from "../types";

export const MAX_WEEK_EVENT_LANES = 4;

export type PositionedWeekBlock = {
  block: ScheduleBlock;
  lane: number;
};

export function positionWeekBlocks(blocks: ScheduleBlock[]) {
  const laneEnds: number[] = [];
  const positioned: PositionedWeekBlock[] = [];
  let hiddenCount = 0;
  const sorted = [...blocks].sort((left, right) => (
    left.startMinute - right.startMinute || left.endMinute - right.endMinute
  ));

  for (const block of sorted) {
    let lane = laneEnds.findIndex((endMinute) => endMinute <= block.startMinute);
    if (lane < 0) lane = laneEnds.length;
    if (lane >= MAX_WEEK_EVENT_LANES) {
      hiddenCount += 1;
      continue;
    }
    laneEnds[lane] = block.endMinute;
    positioned.push({ block, lane });
  }

  return { positioned, hiddenCount, laneCount: laneEnds.length };
}
