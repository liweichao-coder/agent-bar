export type MorningReminderState = {
  enabled: boolean;
  promptMinute: number;
  dismissedDay?: string;
  snoozedUntilMs?: number;
  lastPlannedDay?: string;
};

export type MorningReminderEvaluation = {
  due: boolean;
  reason: "disabled" | "already-planned" | "dismissed" | "snoozed" | "before-window" | "after-window" | "due";
};

const REGULAR_WINDOW_END_MINUTE = 12 * 60;
const SNOOZE_WINDOW_END_MINUTE = 18 * 60;

export function evaluateMorningReminder(
  now: Date,
  state: MorningReminderState,
): MorningReminderEvaluation {
  if (!state.enabled) return { due: false, reason: "disabled" };

  const today = localDateKey(now);
  if (state.lastPlannedDay === today) return { due: false, reason: "already-planned" };
  if (state.dismissedDay === today) return { due: false, reason: "dismissed" };

  const minute = now.getHours() * 60 + now.getMinutes();
  const snoozedUntil = state.snoozedUntilMs ? new Date(state.snoozedUntilMs) : undefined;
  if (snoozedUntil && localDateKey(snoozedUntil) === today) {
    if (now.getTime() < snoozedUntil.getTime()) return { due: false, reason: "snoozed" };
    return minute <= SNOOZE_WINDOW_END_MINUTE
      ? { due: true, reason: "due" }
      : { due: false, reason: "after-window" };
  }

  if (minute < state.promptMinute) return { due: false, reason: "before-window" };
  const regularWindowEnd = Math.min(
    SNOOZE_WINDOW_END_MINUTE,
    Math.max(REGULAR_WINDOW_END_MINUTE, state.promptMinute + 4 * 60),
  );
  if (minute > regularWindowEnd) return { due: false, reason: "after-window" };
  return { due: true, reason: "due" };
}

export function localDateKey(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
