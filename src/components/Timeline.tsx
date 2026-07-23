import { CalendarDays, LockKeyhole } from "lucide-react";
import type { CSSProperties } from "react";
import type { ActivityBlock, ScheduleBlock } from "../types";

const START_MINUTE = 360;
const END_MINUTE = 1440;
const RANGE = END_MINUTE - START_MINUTE;
const HOURS = Array.from({ length: 19 }, (_, index) => index + 6);

type TimelineProps = {
  schedule: ScheduleBlock[];
  activity: ActivityBlock[];
  currentMinute: number;
  selectedId: string | null;
  onSelect: (id: string) => void;
};

function positionStyle(start: number, end: number): CSSProperties {
  const left = ((start - START_MINUTE) / RANGE) * 100;
  const width = ((end - start) / RANGE) * 100;
  return { "--block-left": `${left}%`, "--block-width": `${width}%` } as CSSProperties;
}

export function formatMinute(minute: number) {
  const hours = Math.floor(minute / 60);
  const minutes = minute % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

export function Timeline({ schedule, activity, currentMinute, selectedId, onSelect }: TimelineProps) {
  const nowPercent = ((currentMinute - START_MINUTE) / RANGE) * 100;
  const visibleNow = nowPercent >= 0 && nowPercent <= 100;

  return (
    <section className="timeline-section" aria-labelledby="timeline-heading">
      <header className="section-heading">
        <div>
          <span className="eyebrow">DAY TIMELINE</span>
          <h2 id="timeline-heading">今天的时间去向</h2>
        </div>
        <div className="timeline-legend" aria-label="时间轴图例">
          <span><i className="legend-dot planned" />计划</span>
          <span><i className="legend-dot actual" />实际</span>
        </div>
      </header>

      <div className="timeline-scroller" tabIndex={0} aria-label="从早上六点到午夜的时间轴，可横向滚动">
        <div className="timeline-canvas">
          <div className="hour-scale" aria-hidden="true">
            {HOURS.map((hour) => <span key={hour}>{String(hour).padStart(2, "0")}:00</span>)}
          </div>
          <div className="timeline-grid" aria-hidden="true">
            {HOURS.map((hour) => <i key={hour} />)}
          </div>
          {visibleNow && (
            <div className="now-marker" style={{ "--now-left": `${nowPercent}%` } as CSSProperties}>
              <span>现在</span>
            </div>
          )}

          <div className="timeline-lane plan-lane">
            <div className="lane-label"><CalendarDays size={15} />计划</div>
            <div className="lane-content">
              {schedule.map((block) => (
                <button
                  key={block.id}
                  type="button"
                  className={`time-block ${block.category} ${block.status} ${selectedId === block.id ? "selected" : ""}`}
                  style={positionStyle(block.startMinute, block.endMinute)}
                  onClick={() => onSelect(block.id)}
                  aria-label={`${block.title}，${formatMinute(block.startMinute)} 到 ${formatMinute(block.endMinute)}`}
                >
                  <span>{block.title}</span>
                  {block.locked && <LockKeyhole size={12} aria-label="已锁定" />}
                </button>
              ))}
            </div>
          </div>

          <div className="timeline-lane actual-lane">
            <div className="lane-label">实际</div>
            <div className="lane-content">
              {activity.map((block) => (
                <button
                  key={block.id}
                  type="button"
                  className={`activity-block ${block.category} ${selectedId === block.id ? "selected" : ""}`}
                  style={positionStyle(block.startMinute, block.endMinute)}
                  onClick={() => onSelect(block.id)}
                  aria-label={`${block.appName}，${formatMinute(block.startMinute)} 到 ${formatMinute(block.endMinute)}`}
                >
                  <strong>{block.appName}</strong>
                  <span>{block.windowTitle}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
