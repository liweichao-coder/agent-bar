import { formatDuration, timeCategories, type ActivityDaySummary, type TimeCategory } from "../lib/timeStats";
import type { NativeActivityCaptureState } from "../lib/nativeBridge";

type TimeSummaryProps = {
  summary: ActivityDaySummary;
  tracking: boolean;
  captureStatus: NativeActivityCaptureState["status"];
};

const categoryLabels: Record<TimeCategory, string> = {
  focus: "专注",
  meeting: "会议",
  admin: "事务",
  life: "生活",
};

const captureLabels: Record<NativeActivityCaptureState["status"], string> = {
  active: "正在记录",
  idle: "空闲暂停",
  locked: "锁屏暂停",
  disconnected: "会话断开",
  paused: "记录已暂停",
  unavailable: "采集不可用",
};

export function TimeSummary({ summary, tracking, captureStatus }: TimeSummaryProps) {
  const progressMax = Math.max(1, summary.actualMinutes);
  return (
    <section className="time-summary" aria-labelledby="summary-heading">
      <header className="section-heading compact"><div><span className="eyebrow">ACTUAL</span><h2 id="summary-heading">今日投入</h2></div></header>
      <div className="summary-total">
        <strong>{formatDuration(summary.actualMinutes)}</strong>
        <span>{tracking ? captureLabels[captureStatus] : "记录已暂停"} · 计划 {formatDuration(summary.plannedMinutes)}</span>
      </div>
      <div className="category-bars">
        {timeCategories.map((category) => (
          <div key={category}>
            <span><i className={category} />{categoryLabels[category]}</span>
            <strong>{formatDuration(summary.categories[category])}</strong>
            <progress
              aria-label={`${categoryLabels[category]} ${formatDuration(summary.categories[category])}`}
              max={progressMax}
              value={summary.categories[category]}
            />
          </div>
        ))}
      </div>
      {summary.topApps.length > 0 && (
        <div className="top-apps">
          <span className="eyebrow">TOP APPS</span>
          <ol>
            {summary.topApps.map((app) => <li key={app.appName}><span>{app.appName}</span><strong>{formatDuration(app.minutes)}</strong></li>)}
          </ol>
        </div>
      )}
    </section>
  );
}
