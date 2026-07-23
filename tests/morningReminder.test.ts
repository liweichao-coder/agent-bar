import assert from "node:assert/strict";
import test from "node:test";
import { evaluateMorningReminder, type MorningReminderState } from "../src/lib/morningReminder.ts";

const state = (overrides: Partial<MorningReminderState> = {}): MorningReminderState => ({
  enabled: true,
  promptMinute: 8 * 60,
  ...overrides,
});

const at = (day: number, hour: number, minute = 0) => new Date(2026, 6, day, hour, minute);

test("becomes due at the configured morning time", () => {
  assert.deepEqual(evaluateMorningReminder(at(22, 8), state()), { due: true, reason: "due" });
  assert.deepEqual(evaluateMorningReminder(at(22, 7, 59), state()), { due: false, reason: "before-window" });
});

test("stays hidden after the day is planned or dismissed", () => {
  assert.equal(evaluateMorningReminder(at(22, 9), state({ lastPlannedDay: "2026-07-22" })).reason, "already-planned");
  assert.equal(evaluateMorningReminder(at(22, 9), state({ dismissedDay: "2026-07-22" })).reason, "dismissed");
});

test("waits until a same-day snooze expires", () => {
  const snoozedUntil = at(22, 9, 30).getTime();
  assert.equal(evaluateMorningReminder(at(22, 9), state({ snoozedUntilMs: snoozedUntil })).reason, "snoozed");
  assert.deepEqual(evaluateMorningReminder(at(22, 9, 30), state({ snoozedUntilMs: snoozedUntil })), { due: true, reason: "due" });
});

test("does not let an old snooze bypass the next day's prompt time", () => {
  const priorDaySnooze = at(21, 9, 30).getTime();
  assert.equal(evaluateMorningReminder(at(22, 7), state({ snoozedUntilMs: priorDaySnooze })).reason, "before-window");
});

test("regular reminders end at noon but an explicit snooze can extend later", () => {
  assert.equal(evaluateMorningReminder(at(22, 13), state()).reason, "after-window");
  const snoozedUntil = at(22, 13).getTime();
  assert.equal(evaluateMorningReminder(at(22, 13), state({ snoozedUntilMs: snoozedUntil })).due, true);
  assert.equal(evaluateMorningReminder(at(22, 18, 1), state({ snoozedUntilMs: snoozedUntil })).reason, "after-window");
});

test("disabled reminders never become due", () => {
  assert.equal(evaluateMorningReminder(at(22, 9), state({ enabled: false })).reason, "disabled");
});
