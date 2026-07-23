import {
  AlertTriangle,
  Check,
  FileImage,
  ImagePlus,
  LoaderCircle,
  ScanText,
  Square,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type MouseEvent } from "react";
import type {
  NativeScreenshotImportSnapshot,
} from "../lib/nativeBridge";
import type {
  MorningPlanTask,
  PlannerPeriod,
  PlannerPriority,
} from "../lib/morningPlanner";
import type { ScheduleBlock } from "../types";

type ScreenshotImportDialogProps = {
  open: boolean;
  snapshot: NativeScreenshotImportSnapshot;
  onAnalyze: (file: File) => Promise<void>;
  onCancelAnalysis: () => Promise<void>;
  onClose: () => void;
  onConfirm: (tasks: MorningPlanTask[]) => Promise<void>;
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

const durationOptions = [15, 30, 45, 60, 90, 120, 180, 240, 360, 480];

export function ScreenshotImportDialog({
  open,
  snapshot,
  onAnalyze,
  onCancelAnalysis,
  onClose,
  onConfirm,
}: ScreenshotImportDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const previewUrlRef = useRef<string | null>(null);
  const resultKeyRef = useRef("");
  const [previewUrl, setPreviewUrl] = useState("");
  const [draftTasks, setDraftTasks] = useState<MorningPlanTask[]>([]);
  const [fileError, setFileError] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  useEffect(() => {
    if (snapshot.status === "ready") {
      const resultKey = `${snapshot.fileName ?? ""}:${snapshot.tasks.map((task) => task.id).join(",")}`;
      if (resultKeyRef.current !== resultKey) {
        resultKeyRef.current = resultKey;
        setDraftTasks(snapshot.tasks);
      }
    } else {
      resultKeyRef.current = "";
      setDraftTasks([]);
    }
  }, [snapshot.status, snapshot.tasks]);

  useEffect(() => () => {
    if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
  }, []);

  function updatePreview(file?: File) {
    if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
    const url = file ? URL.createObjectURL(file) : "";
    previewUrlRef.current = url || null;
    setPreviewUrl(url);
  }

  async function selectFile(file?: File) {
    if (!file) return;
    setFileError("");
    if (!["image/png", "image/jpeg", "image/webp"].includes(file.type)) {
      setFileError("仅支持 PNG、JPEG 或 WebP 图片");
      return;
    }
    if (file.size === 0 || file.size > 8 * 1024 * 1024) {
      setFileError("截图大小必须在 8 MB 以内");
      return;
    }
    updatePreview(file);
    setDraftTasks([]);
    try {
      await onAnalyze(file);
    } catch (error) {
      setFileError(String(error));
    }
  }

  function updateTask(id: string, update: Partial<MorningPlanTask>) {
    setDraftTasks((current) => current.map((task) => (
      task.id === id ? { ...task, ...update } : task
    )));
  }

  async function confirmTasks() {
    if (saving || draftTasks.length === 0 || draftTasks.some((task) => !task.title.trim())) return;
    setSaving(true);
    setFileError("");
    try {
      await onConfirm(draftTasks);
      dialogRef.current?.close();
    } catch (error) {
      setFileError(String(error));
    } finally {
      setSaving(false);
    }
  }

  async function cancelAnalysis() {
    setFileError("");
    try {
      await onCancelAnalysis();
    } catch (error) {
      setFileError(String(error));
    }
  }

  function handleBackdrop(event: MouseEvent<HTMLDialogElement>) {
    if (!("closedBy" in HTMLDialogElement.prototype) && event.target === event.currentTarget) {
      event.currentTarget.close();
    }
  }

  const error = fileError || snapshot.error;

  return (
    <dialog
      ref={dialogRef}
      className="privacy-dialog screenshot-import-dialog"
      aria-labelledby="screenshot-import-title"
      aria-busy={snapshot.status === "analyzing"}
      onClick={handleBackdrop}
      onClose={onClose}
    >
      <div className="screenshot-import-shell">
        <header className="privacy-dialog-header">
          <div><span className="eyebrow">VISUAL INBOX</span><h2 id="screenshot-import-title">从截图提取事项</h2></div>
          <button className="icon-button" type="button" onClick={() => dialogRef.current?.close()} aria-label="关闭截图导入" data-tooltip="关闭"><X size={18} /></button>
        </header>

        <div className="screenshot-workspace">
          <section className="screenshot-source-band" aria-labelledby="screenshot-source-heading">
            <div className="planner-band-heading"><ImagePlus size={17} /><h3 id="screenshot-source-heading">截图</h3><span>PNG / JPEG / WebP · 8 MB</span></div>
            <input
              ref={fileRef}
              className="calendar-file-input"
              type="file"
              accept="image/png,image/jpeg,image/webp"
              onChange={(event) => void selectFile(event.target.files?.[0])}
            />
            <button
              className="screenshot-picker"
              type="button"
              disabled={snapshot.status === "analyzing"}
              onClick={() => fileRef.current?.click()}
            >
              {previewUrl ? <img src={previewUrl} alt={`待分析截图：${snapshot.fileName ?? "本地截图"}`} /> : <span className="screenshot-placeholder"><FileImage size={30} /><strong>{snapshot.fileName ?? "选择聊天或通知截图"}</strong><small>发送到当前登录的 Codex；分析后从本地临时缓存删除</small></span>}
            </button>
            {snapshot.status === "analyzing" && (
              <div className="screenshot-progress" role="status">
                <LoaderCircle size={17} aria-hidden="true" />
                <span><strong>Codex 正在识别时间与事项</strong><small>办公室中的 Agent 会同步显示这项工作</small></span>
              </div>
            )}
          </section>

          <section className="screenshot-result-band" aria-labelledby="screenshot-result-heading">
            <div className="planner-band-heading"><ScanText size={17} /><h3 id="screenshot-result-heading">确认事项</h3><span>{draftTasks.length} 项</span></div>
            {snapshot.status === "ready" && draftTasks.length > 0 ? (
              <ul className="screenshot-task-list" role="list">
                {draftTasks.map((task, index) => (
                  <li key={task.id}>
                    <div className="screenshot-task-main">
                      <label htmlFor={`screenshot-title-${task.id}`}><span>事项 {index + 1}</span><input id={`screenshot-title-${task.id}`} value={task.title} maxLength={120} required onChange={(event) => updateTask(task.id, { title: event.target.value })} /></label>
                      <button className="icon-button screenshot-remove-task" type="button" onClick={() => setDraftTasks((current) => current.filter((item) => item.id !== task.id))} aria-label={`移除事项 ${task.title || index + 1}`} data-tooltip="移除"><Trash2 size={15} /></button>
                    </div>
                    <div className="screenshot-task-fields">
                      <label htmlFor={`screenshot-duration-${task.id}`}><span>时长</span><select id={`screenshot-duration-${task.id}`} value={task.durationMinutes} onChange={(event) => updateTask(task.id, { durationMinutes: Number(event.target.value) })}>{durationOptions.map((minutes) => <option key={minutes} value={minutes}>{minutes} 分钟</option>)}</select></label>
                      <label htmlFor={`screenshot-priority-${task.id}`}><span>优先级</span><select id={`screenshot-priority-${task.id}`} value={task.priority} onChange={(event) => updateTask(task.id, { priority: event.target.value as PlannerPriority })}>{priorityOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
                      <label htmlFor={`screenshot-period-${task.id}`}><span>偏好</span><select id={`screenshot-period-${task.id}`} value={task.preferredPeriod} onChange={(event) => updateTask(task.id, { preferredPeriod: event.target.value as PlannerPeriod })}>{periodOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
                      <label htmlFor={`screenshot-category-${task.id}`}><span>类别</span><select id={`screenshot-category-${task.id}`} value={task.category} onChange={(event) => updateTask(task.id, { category: event.target.value as ScheduleBlock["category"] })}>{categoryOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
                    </div>
                  </li>
                ))}
              </ul>
            ) : (
              <div className="screenshot-empty-result">
                <ScanText size={24} />
                <strong>{snapshot.status === "analyzing" ? "正在整理结构化事项" : "提取结果会显示在这里"}</strong>
                <span>确认前可以修改标题、时长、优先级和时段偏好</span>
              </div>
            )}
            {snapshot.warnings.length > 0 && <ul className="screenshot-warning-list">{snapshot.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>}
          </section>
        </div>

        {error && <p className="calendar-import-error screenshot-import-error" role="alert"><AlertTriangle size={15} />{error}</p>}

        <footer className="privacy-dialog-footer screenshot-import-footer">
          {snapshot.status === "analyzing" ? (
            <button className="danger-command" type="button" onClick={() => void cancelAnalysis()}><Square size={14} />终止分析</button>
          ) : (
            <button className="ghost-button" type="button" onClick={() => dialogRef.current?.close()}>取消</button>
          )}
          <button className="primary-button" type="button" disabled={snapshot.status !== "ready" || draftTasks.length === 0 || draftTasks.some((task) => !task.title.trim()) || saving} onClick={() => void confirmTasks()}><Check size={16} />{saving ? "加入中" : "加入待办并规划"}</button>
        </footer>
      </div>
    </dialog>
  );
}
