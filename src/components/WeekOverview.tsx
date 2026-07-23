import { formatDuration, timeCategories, type ActivityDaySummary, type TimeCategory } from "../lib/timeStats";

type WeekOverviewProps = {
  days: ActivityDaySummary[];
  today: string;
  loading?: boolean;
};

const categoryLabels: Record<TimeCategory, string> = {
  focus: "专注",
  meeting: "会议",
  admin: "事务",
  life: "生活",
};

export function WeekOverview({ days, today, loading = false }: WeekOverviewProps) {
  const plannedMinutes = days.reduce((total, day) => total + day.plannedMinutes, 0);
  const actualMinutes = days.reduce((total, day) => total + day.actualMinutes, 0);
  const categoryTotals = timeCategories.reduce((totals, category) => ({
    ...totals,
    [category]: days.reduce((total, day) => total + day.categories[category], 0),
  }), { focus: 0, meeting: 0, admin: 0, life: 0 } as Record<TimeCategory, number>);

  return (
    <section className="week-overview" aria-labelledby="week-heading" aria-busy={loading}>
      <header className="section-heading">
        <div>
          <span className="eyebrow">WEEKLY RHYTHM</span>
          <h2 id="week-heading">本周时间结构</h2>
        </div>
        <p>计划 {formatDuration(plannedMinutes)} · 已记录 {formatDuration(actualMinutes)}</p>
      </header>
      <div className="week-list">
        {days.map((item) => {
          const date = parseLocalDay(item.day);
          const comparisonMinutes = Math.max(60, item.plannedMinutes, item.actualMinutes);
          const emptyMinutes = Math.max(0, comparisonMinutes - item.actualMinutes);
          return (
            <div className={`week-row ${item.day === today ? "today" : ""}`} key={item.day}>
              <div className="week-day"><strong>{formatWeekday(date)}</strong><span>{date.getMonth() + 1}/{date.getDate()}</span></div>
              <div
                className="week-meter"
                aria-label={`${formatWeekday(date)}计划 ${formatDuration(item.plannedMinutes)}，已记录 ${formatDuration(item.actualMinutes)}`}
              >
                {timeCategories.map((category) => (
                  <i key={category} className={category} style={{ flex: item.categories[category] }} />
                ))}
                <i className="empty" style={{ flex: emptyMinutes }} />
              </div>
              <strong className="week-total"><span>{formatDuration(item.actualMinutes)}</span><small>/ {formatDuration(item.plannedMinutes)}</small></strong>
            </div>
          );
        })}
      </div>
      <footer className="week-legend">
        {timeCategories.map((category) => (
          <span key={category}><i className={category} />{categoryLabels[category]} {formatDuration(categoryTotals[category])}</span>
        ))}
      </footer>
    </section>
  );
}

function parseLocalDay(day: string) {
  const [year, month, date] = day.split("-").map(Number);
  return new Date(year, month - 1, date);
}

function formatWeekday(date: Date) {
  return new Intl.DateTimeFormat("zh-CN", { weekday: "short" }).format(date);
}
