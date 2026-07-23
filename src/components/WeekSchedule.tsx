import { CalendarRange, LockKeyhole } from "lucide-react";
import { useMemo, useState, type CSSProperties } from "react";
import { positionWeekBlocks } from "../lib/weekSchedule";
import type { ScheduleBlock, ScheduleDay } from "../types";
import { formatMinute } from "./Timeline";

const DAY_MINUTES = 24 * 60;
const HOUR_MARKS = [0, 6, 12, 18, 24];

type WeekScheduleProps = {
  days: ScheduleDay[];
  today: string;
  loading?: boolean;
};

export function WeekSchedule({ days, today, loading = false }: WeekScheduleProps) {
  const [selected, setSelected] = useState<{ day: string; id: string } | null>(null);
  const totalEvents = days.reduce((total, day) => total + day.blocks.length, 0);
  const selectedBlock = useMemo(() => {
    if (!selected) return null;
    const day = days.find((item) => item.day === selected.day);
    const block = day?.blocks.find((item) => item.id === selected.id);
    return block ? { day: selected.day, block } : null;
  }, [days, selected]);
  const firstDate = days[0] ? parseLocalDay(days[0].day) : null;
  const lastDate = days.at(-1) ? parseLocalDay(days.at(-1)!.day) : null;

  return (
    <section className="week-schedule" aria-labelledby="week-schedule-heading" aria-busy={loading}>
      <header className="section-heading">
        <div>
          <span className="eyebrow">WEEK SCHEDULE</span>
          <h2 id="week-schedule-heading">本周安排</h2>
        </div>
        <p>{firstDate && lastDate ? `${formatMonthDay(firstDate)} - ${formatMonthDay(lastDate)} · ` : ""}{totalEvents} 项</p>
      </header>

      <div className="week-schedule-scroller" tabIndex={0} aria-label="本周七天的 24 小时日程，可横向滚动">
        <div className="week-schedule-canvas">
          <div className="week-hour-axis" aria-hidden="true">
            <span />
            <div>{HOUR_MARKS.map((hour) => <time key={hour}>{String(hour).padStart(2, "0")}:00</time>)}</div>
          </div>
          <ol className="week-schedule-days">
            {days.map((day) => {
              const date = parseLocalDay(day.day);
              const { positioned, hiddenCount, laneCount } = positionWeekBlocks(day.blocks);
              return (
                <li key={day.day} className={day.day === today ? "today" : ""}>
                  <div className="week-schedule-day">
                    <strong>{formatWeekday(date)}</strong>
                    <span>{formatMonthDay(date)}</span>
                  </div>
                  <div className="week-day-track" style={{ "--week-lanes": Math.max(1, laneCount) } as CSSProperties}>
                    <div className="week-track-grid" aria-hidden="true">{HOUR_MARKS.map((hour) => <i key={hour} style={{ left: `${(hour / 24) * 100}%` }} />)}</div>
                    {positioned.map(({ block, lane }) => (
                      <button
                        key={block.id}
                        type="button"
                        className={`week-event ${block.category} ${selected?.day === day.day && selected.id === block.id ? "selected" : ""}`}
                        style={blockStyle(block, lane)}
                        onClick={() => setSelected({ day: day.day, id: block.id })}
                        aria-label={`${formatWeekday(date)} ${formatMonthDay(date)}，${block.title}，${formatMinute(block.startMinute)} 到 ${formatMinute(block.endMinute)}`}
                      >
                        <span>{block.title}</span>
                        {block.locked && <LockKeyhole size={10} aria-hidden="true" />}
                      </button>
                    ))}
                    {day.blocks.length === 0 && <span className="week-day-empty">无固定安排</span>}
                    {hiddenCount > 0 && <span className="week-event-overflow">另有 {hiddenCount} 项重叠安排</span>}
                  </div>
                </li>
              );
            })}
          </ol>
        </div>
      </div>

      <footer className="week-schedule-detail" aria-live="polite">
        {selectedBlock ? (
          <>
            <CalendarRange size={15} aria-hidden="true" />
            <strong>{selectedBlock.block.title}</strong>
            <span>{formatWeekday(parseLocalDay(selectedBlock.day))} · {formatMinute(selectedBlock.block.startMinute)} - {formatMinute(selectedBlock.block.endMinute)} · {sourceLabel(selectedBlock.block.source)}</span>
          </>
        ) : <span>选择一个时间块查看详情</span>}
      </footer>
    </section>
  );
}

function blockStyle(block: ScheduleBlock, lane: number): CSSProperties {
  const start = Math.max(0, Math.min(DAY_MINUTES, block.startMinute));
  const end = Math.max(start + 1, Math.min(DAY_MINUTES, block.endMinute));
  return {
    "--event-left": `${(start / DAY_MINUTES) * 100}%`,
    "--event-width": `${((end - start) / DAY_MINUTES) * 100}%`,
    "--event-lane": lane,
  } as CSSProperties;
}

function parseLocalDay(day: string) {
  const [year, month, date] = day.split("-").map(Number);
  return new Date(year, month - 1, date);
}

function formatWeekday(date: Date) {
  return new Intl.DateTimeFormat("zh-CN", { weekday: "short" }).format(date);
}

function formatMonthDay(date: Date) {
  return `${date.getMonth() + 1}/${date.getDate()}`;
}

function sourceLabel(source: ScheduleBlock["source"]) {
  if (source.startsWith("calendar")) return "日历";
  if (source === "agent") return "Agent";
  return "手动安排";
}
