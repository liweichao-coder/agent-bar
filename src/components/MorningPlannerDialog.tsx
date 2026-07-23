import {
  AlarmClock,
  AlertTriangle,
  CalendarClock,
  Check,
  Clock3,
  Plus,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import {
  buildMorningPlan,
  type MorningPlanDraft,
  type MorningPlanTask,
  type PlannerPeriod,
  type PlannerPriority,
} from "../lib/morningPlanner";
import type { ScheduleBlock } from "../types";

type MorningPlannerDialogProps = {
  open: boolean;
  tasks: MorningPlanTask[];
  schedule: ScheduleBlock[];
  currentMinute: number;
  reminderEnabled: boolean;
  reminderMinute: number;
  onTasksChange: (tasks: MorningPlanTask[]) => void;
  onReminderChange: (enabled: boolean, promptMinute: number) => void;
  onClose: () => void;
  onConfirm: (draft: MorningPlanDraft) => Promise<void>;
};

const priorityOptions: Array<{ value: PlannerPriority; label: string }> = [
  { value: "critical", label: "必须完成" },
  { value: "high", label: "高" },
  { value: "normal", label: "普通" },
  { value: "low", label: "低" },
];

const periodOptions: Array<{ value: PlannerPeriod; label: string }> = [
  { value: "any", label: "任意时段" },
  { value: "morning", label: "上午" },
  { value: "afternoon", label: "下午" },
  { value: "evening", label: "晚间" },
];

const categoryOptions: Array<{ value: ScheduleBlock["category"]; label: string }> = [
  { value: "focus", label: "专注" },
  { value: "meeting", label: "会议" },
  { value: "admin", label: "事务" },
  { value: "life", label: "生活" },
];

const durationOptions = [15, 30, 45, 60, 90, 120, 180, 240];
const startOptions = Array.from({ length: 31 }, (_, index) => 360 + index * 30);
const endOptions = Array.from({ length: 29 }, (_, index) => 600 + index * 30);
const reminderTimeOptions = Array.from({ length: 11 }, (_, index) => 360 + index * 30);

export function MorningPlannerDialog({
  open,
  tasks,
  schedule,
  currentMinute,
  reminderEnabled,
  reminderMinute,
  onTasksChange,
  onReminderChange,
  onClose,
  onConfirm,
}: MorningPlannerDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [dayStartMinute, setDayStartMinute] = useState(480);
  const [dayEndMinute, setDayEndMinute] = useState(1440);
  const [bufferMinutes, setBufferMinutes] = useState(10);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      const roundedNow = Math.ceil(currentMinute / 15) * 15;
      setDayStartMinute(roundedNow >= 480 && roundedNow < 1440 ? roundedNow : 480);
      setError("");
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [currentMinute, open]);

  const draft = useMemo(() => buildMorningPlan(tasks, schedule, {
    dayStartMinute,
    dayEndMinute,
    bufferMinutes,
  }), [bufferMinutes, dayEndMinute, dayStartMinute, schedule, tasks]);
  const invalidTaskCount = tasks.filter((task) => !task.title.trim()).length;

  function updateTask(id: string, update: Partial<MorningPlanTask>) {
    onTasksChange(tasks.map((task) => task.id === id ? { ...task, ...update } : task));
  }

  function addTask() {
    const id = typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `${Date.now()}-${tasks.length}`;
    onTasksChange([...tasks, {
      id,
      title: "",
      durationMinutes: 60,
      priority: "normal",
      preferredPeriod: "any",
      category: "focus",
      notes: "",
    }]);
  }

  async function confirmDraft() {
    if (!draft.blocks.length || invalidTaskCount || saving) return;
    setSaving(true);
    setError("");
    try {
      await onConfirm(draft);
      dialogRef.current?.close();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  }

  function handleBackdrop(event: MouseEvent<HTMLDialogElement>) {
    if (!("closedBy" in HTMLDialogElement.prototype) && event.target === event.currentTarget) {
      event.currentTarget.close();
    }
  }

  return (
    <dialog
      ref={dialogRef}
      className="privacy-dialog morning-planner-dialog"
      aria-labelledby="morning-planner-title"
      onClick={handleBackdrop}
      onClose={onClose}
    >
      <div className="morning-planner-shell">
        <header className="privacy-dialog-header">
          <div><span className="eyebrow">MORNING PLAN</span><h2 id="morning-planner-title">安排今天</h2></div>
          <button className="icon-button" type="button" onClick={() => dialogRef.current?.close()} aria-label="关闭晨间规划" data-tooltip="关闭"><X size={18} /></button>
        </header>

        <section className="planner-config-band" aria-labelledby="planner-config-heading">
          <div className="planner-band-heading"><CalendarClock size={17} /><h3 id="planner-config-heading">规划范围</h3><span>{schedule.length} 项已有安排</span></div>
          <div className="planner-config-grid">
            <label htmlFor="planner-start"><span>开始</span><select id="planner-start" value={dayStartMinute} onChange={(event) => {
              const value = Number(event.target.value);
              setDayStartMinute(value);
              if (value >= dayEndMinute) setDayEndMinute(Math.min(1440, value + 60));
            }}>{startOptions.filter((minute) => minute < dayEndMinute).map((minute) => <option key={minute} value={minute}>{formatMinute(minute)}</option>)}</select></label>
            <label htmlFor="planner-end"><span>结束</span><select id="planner-end" value={dayEndMinute} onChange={(event) => setDayEndMinute(Number(event.target.value))}>{endOptions.filter((minute) => minute > dayStartMinute).map((minute) => <option key={minute} value={minute}>{formatMinute(minute)}</option>)}</select></label>
            <label htmlFor="planner-buffer"><span>切换缓冲</span><select id="planner-buffer" value={bufferMinutes} onChange={(event) => setBufferMinutes(Number(event.target.value))}>{[0, 5, 10, 15, 30].map((minute) => <option key={minute} value={minute}>{minute} 分钟</option>)}</select></label>
          </div>
          <div className="planner-reminder-row">
            <AlarmClock size={17} aria-hidden="true" />
            <label htmlFor="planner-reminder-enabled">
              <span><strong>每日规划提醒</strong><small>到点后在工作台显示，确认安排后当天不再提醒</small></span>
              <input
                id="planner-reminder-enabled"
                type="checkbox"
                checked={reminderEnabled}
                onChange={(event) => onReminderChange(event.target.checked, reminderMinute)}
              />
            </label>
            <label className="planner-reminder-time" htmlFor="planner-reminder-time">
              <span>提醒时间</span>
              <select
                id="planner-reminder-time"
                value={reminderMinute}
                disabled={!reminderEnabled}
                onChange={(event) => onReminderChange(reminderEnabled, Number(event.target.value))}
              >
                {reminderTimeOptions.map((minute) => <option key={minute} value={minute}>{formatMinute(minute)}</option>)}
              </select>
            </label>
          </div>
        </section>

        <section className="planner-task-band" aria-labelledby="planner-task-heading">
          <div className="planner-band-heading"><Clock3 size={17} /><h3 id="planner-task-heading">待安排事项</h3><span>{tasks.length} 项</span></div>
          {tasks.length > 0 ? (
            <ul className="planner-task-list" role="list">
              {tasks.map((task, index) => (
                <li key={task.id}>
                  <label className="planner-title-field" htmlFor={`planner-task-${task.id}`}><span>任务 {index + 1}</span><input id={`planner-task-${task.id}`} value={task.title} maxLength={120} required onChange={(event) => updateTask(task.id, { title: event.target.value })} /></label>
                  <label htmlFor={`planner-duration-${task.id}`}><span>时长</span><select id={`planner-duration-${task.id}`} value={task.durationMinutes} onChange={(event) => updateTask(task.id, { durationMinutes: Number(event.target.value) })}>{durationOptions.map((minute) => <option key={minute} value={minute}>{minute} 分钟</option>)}</select></label>
                  <label htmlFor={`planner-priority-${task.id}`}><span>优先级</span><select id={`planner-priority-${task.id}`} value={task.priority} onChange={(event) => updateTask(task.id, { priority: event.target.value as PlannerPriority })}>{priorityOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
                  <label htmlFor={`planner-period-${task.id}`}><span>偏好</span><select id={`planner-period-${task.id}`} value={task.preferredPeriod} onChange={(event) => updateTask(task.id, { preferredPeriod: event.target.value as PlannerPeriod })}>{periodOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
                  <label htmlFor={`planner-category-${task.id}`}><span>类别</span><select id={`planner-category-${task.id}`} value={task.category} onChange={(event) => updateTask(task.id, { category: event.target.value as ScheduleBlock["category"] })}>{categoryOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
                  <button className="icon-button planner-remove-task" type="button" onClick={() => onTasksChange(tasks.filter((item) => item.id !== task.id))} aria-label={`移除任务 ${task.title || index + 1}`} data-tooltip="移除"><Trash2 size={15} /></button>
                </li>
              ))}
            </ul>
          ) : <p className="planner-empty-copy">今天还没有待安排事项</p>}
          <button className="planner-add-task" type="button" onClick={addTask}><Plus size={16} />添加事项</button>
        </section>

        <section className="planner-draft-band" aria-labelledby="planner-draft-heading" aria-live="polite">
          <div className="planner-band-heading"><Sparkles size={17} /><h3 id="planner-draft-heading">全天草案</h3><span>{draft.blocks.length} 项已排入</span></div>
          <div className="planner-draft-summary">
            <div><span>已有安排</span><strong>{formatDuration(draft.occupiedMinutes)}</strong></div>
            <div><span>新增任务</span><strong>{formatDuration(draft.scheduledMinutes)}</strong></div>
            <div><span>剩余空档</span><strong>{formatDuration(draft.availableMinutes)}</strong></div>
          </div>
          {draft.blocks.length > 0 && (
            <ol className="planner-draft-list">
              {draft.blocks.map((block) => (
                <li key={block.id}><time>{formatMinute(block.startMinute)} - {formatMinute(block.endMinute)}</time><div><strong>{block.title}</strong><span>{block.reason}</span></div></li>
              ))}
            </ol>
          )}
          {draft.unscheduled.length > 0 && (
            <ul className="planner-unscheduled-list" role="list">
              {draft.unscheduled.map(({ task, reason }) => <li key={task.id}><AlertTriangle size={14} /><span><strong>{task.title}</strong>{reason}</span></li>)}
            </ul>
          )}
          {invalidTaskCount > 0 && <p className="planner-form-error" role="alert"><AlertTriangle size={14} />{invalidTaskCount} 项任务缺少名称</p>}
          {error && <p className="planner-form-error" role="alert"><AlertTriangle size={14} />{error}</p>}
        </section>

        <footer className="privacy-dialog-footer">
          <button className="ghost-button" type="button" onClick={() => dialogRef.current?.close()}>取消</button>
          <button className="primary-button" type="button" disabled={!draft.blocks.length || invalidTaskCount > 0 || saving} onClick={() => void confirmDraft()}><Check size={16} />{saving ? "应用中" : "确认全天安排"}</button>
        </footer>
      </div>
    </dialog>
  );
}

function formatMinute(minute: number) {
  if (minute === 1440) return "24:00";
  return `${String(Math.floor(minute / 60)).padStart(2, "0")}:${String(minute % 60).padStart(2, "0")}`;
}

function formatDuration(minutes: number) {
  const hours = Math.floor(minutes / 60);
  const remainder = minutes % 60;
  if (!hours) return `${remainder}m`;
  return remainder ? `${hours}h ${remainder}m` : `${hours}h`;
}
