use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use icalendar::{Calendar, CalendarDateTime, Component, DatePerhapsTime, EventStatus};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAX_ICS_BYTES: usize = 2 * 1024 * 1024;
const MAX_SOURCE_EVENTS: usize = 5_000;
const MAX_OCCURRENCES_PER_EVENT: u16 = 512;
const MAX_PREVIEW_EVENTS: usize = 500;
const MAX_TITLE_CHARS: usize = 120;
const DEFAULT_EVENT_MINUTES: i64 = 30;
const MAX_LOOKBACK_DAYS: i64 = 31;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarImportEvent {
    pub id: String,
    pub title: String,
    pub start_minute: i32,
    pub end_minute: i32,
    pub all_day: bool,
    pub recurring: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarImportPreview {
    pub source_name: String,
    pub events: Vec<CalendarImportEvent>,
    pub skipped_count: usize,
    pub warnings: Vec<String>,
}

struct Candidate {
    event: CalendarImportEvent,
    is_override: bool,
}

pub fn preview_ics(
    ics_text: &str,
    day: &str,
    viewer_timezone: &str,
) -> Result<CalendarImportPreview, String> {
    if ics_text.len() > MAX_ICS_BYTES {
        return Err("日历文件超过 2 MB 限制".to_string());
    }

    let target_date =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| "目标日期格式无效".to_string())?;
    let viewer_tz = normalize_timezone(viewer_timezone)
        .ok_or_else(|| format!("不支持的本地时区：{viewer_timezone}"))?;
    let day_start = local_datetime(viewer_tz, target_date.and_hms_opt(0, 0, 0).unwrap())?;
    let day_end = local_datetime(
        viewer_tz,
        target_date
            .succ_opt()
            .ok_or_else(|| "目标日期超出范围".to_string())?
            .and_hms_opt(0, 0, 0)
            .unwrap(),
    )?;
    let day_start_utc = day_start.with_timezone(&Utc);
    let day_end_utc = day_end.with_timezone(&Utc);

    let calendar: Calendar = ics_text
        .parse()
        .map_err(|error| format!("无法解析 ICS：{error}"))?;
    let source_name = clean_title(calendar.get_name().unwrap_or("导入的日历"));
    let source_event_count = calendar.events().count();
    if source_event_count > MAX_SOURCE_EVENTS {
        return Err(format!("日历包含超过 {MAX_SOURCE_EVENTS} 个原始事件"));
    }

    let mut warnings = Vec::new();
    let mut skipped_count = 0;
    let mut candidates = BTreeMap::<String, Candidate>::new();

    for calendar_event in calendar.calendar_events() {
        let event = calendar_event.event();
        if event.get_status() == Some(EventStatus::Cancelled) {
            skipped_count += 1;
            continue;
        }

        let Some(start_value) = event.get_start() else {
            warnings.push("已跳过缺少开始时间的事件".to_string());
            skipped_count += 1;
            continue;
        };
        let all_day = matches!(start_value, DatePerhapsTime::Date(_));
        let base_start = match to_utc(&start_value, viewer_tz) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(error);
                skipped_count += 1;
                continue;
            }
        };
        let base_end = match event.get_end() {
            Some(end) => match to_utc(&end, viewer_tz) {
                Ok(value) => value,
                Err(error) => {
                    warnings.push(error);
                    skipped_count += 1;
                    continue;
                }
            },
            None if all_day => base_start + Duration::days(1),
            None => {
                warnings.push("部分事件没有结束时间，已按 30 分钟预览".to_string());
                base_start + Duration::minutes(DEFAULT_EVENT_MINUTES)
            }
        };
        let duration = base_end - base_start;
        if duration <= Duration::zero() {
            warnings.push("已跳过结束时间不晚于开始时间的事件".to_string());
            skipped_count += 1;
            continue;
        }

        let title = clean_title(event.get_summary().unwrap_or("未命名日程"));
        let uid_seed = event
            .get_uid()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{title}|{}", base_start.timestamp_millis()));
        let recurrence_id = event
            .get_recurrence_id()
            .and_then(|value| to_utc(&value, viewer_tz).ok());
        let recurring = event.property_value("RRULE").is_some()
            || event.multi_properties().contains_key("RDATE");

        if recurring {
            let recurrence_set = match calendar_event.get_recurrence() {
                Ok(value) => value,
                Err(error) => {
                    warnings.push(format!("已跳过无法展开的重复事件：{error}"));
                    skipped_count += 1;
                    continue;
                }
            };
            let recurrence_tz = recurrence_set.get_dt_start().timezone();
            let lookback = duration.min(Duration::days(MAX_LOOKBACK_DAYS));
            let range_start =
                (day_start_utc - lookback - Duration::seconds(1)).with_timezone(&recurrence_tz);
            let range_end = day_end_utc.with_timezone(&recurrence_tz);
            let result = recurrence_set
                .after(range_start)
                .before(range_end)
                .all(MAX_OCCURRENCES_PER_EVENT);
            if result.limited {
                warnings.push("某个重复事件展开过多，预览已截断".to_string());
            }
            for occurrence in result.dates {
                let occurrence_start = occurrence.with_timezone(&Utc);
                insert_candidate(
                    &mut candidates,
                    &uid_seed,
                    recurrence_id.unwrap_or(occurrence_start),
                    occurrence_start,
                    occurrence_start + duration,
                    &title,
                    all_day,
                    true,
                    recurrence_id.is_some(),
                    day_start_utc,
                    day_end_utc,
                    viewer_tz,
                );
            }
        } else {
            insert_candidate(
                &mut candidates,
                &uid_seed,
                recurrence_id.unwrap_or(base_start),
                base_start,
                base_end,
                &title,
                all_day,
                false,
                recurrence_id.is_some(),
                day_start_utc,
                day_end_utc,
                viewer_tz,
            );
        }
    }

    let mut events = candidates
        .into_values()
        .map(|candidate| candidate.event)
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.start_minute
            .cmp(&right.start_minute)
            .then(left.end_minute.cmp(&right.end_minute))
            .then(left.title.cmp(&right.title))
    });
    if events.len() > MAX_PREVIEW_EVENTS {
        skipped_count += events.len() - MAX_PREVIEW_EVENTS;
        events.truncate(MAX_PREVIEW_EVENTS);
        warnings.push(format!("当天事件超过 {MAX_PREVIEW_EVENTS} 项，预览已截断"));
    }
    warnings.sort();
    warnings.dedup();

    Ok(CalendarImportPreview {
        source_name,
        events,
        skipped_count,
        warnings,
    })
}

#[allow(clippy::too_many_arguments)]
fn insert_candidate(
    candidates: &mut BTreeMap<String, Candidate>,
    uid_seed: &str,
    occurrence_key: DateTime<Utc>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    title: &str,
    all_day: bool,
    recurring: bool,
    is_override: bool,
    day_start: DateTime<Utc>,
    day_end: DateTime<Utc>,
    viewer_tz: Tz,
) {
    if end <= day_start || start >= day_end {
        return;
    }
    let clipped_start = start.max(day_start).with_timezone(&viewer_tz);
    let clipped_end = end.min(day_end).with_timezone(&viewer_tz);
    let start_minute = (clipped_start.hour() * 60 + clipped_start.minute()) as i32;
    let mut end_minute = if clipped_end.with_timezone(&Utc) == day_end {
        1440
    } else {
        (clipped_end.hour() * 60 + clipped_end.minute()) as i32
            + i32::from(clipped_end.second() > 0)
    };
    end_minute = end_minute.max(start_minute + 1).min(1440);

    let id = stable_event_id(uid_seed, occurrence_key.timestamp_millis());
    let candidate = Candidate {
        event: CalendarImportEvent {
            id: id.clone(),
            title: title.to_string(),
            start_minute,
            end_minute,
            all_day,
            recurring,
        },
        is_override,
    };
    match candidates.get(&id) {
        Some(existing) if existing.is_override && !is_override => {}
        _ => {
            candidates.insert(id, candidate);
        }
    }
}

fn to_utc(value: &DatePerhapsTime, viewer_tz: Tz) -> Result<DateTime<Utc>, String> {
    match value {
        DatePerhapsTime::Date(date) => local_datetime(
            viewer_tz,
            date.and_hms_opt(0, 0, 0)
                .ok_or_else(|| "日历包含无效日期".to_string())?,
        )
        .map(|value| value.with_timezone(&Utc)),
        DatePerhapsTime::DateTime(CalendarDateTime::Utc(value)) => Ok(*value),
        DatePerhapsTime::DateTime(CalendarDateTime::Floating(value)) => {
            local_datetime(viewer_tz, *value).map(|value| value.with_timezone(&Utc))
        }
        DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone { date_time, tzid }) => {
            let timezone = normalize_timezone(tzid)
                .ok_or_else(|| format!("已跳过无法识别时区 {tzid} 的事件"))?;
            local_datetime(timezone, *date_time).map(|value| value.with_timezone(&Utc))
        }
    }
}

fn normalize_timezone(value: &str) -> Option<Tz> {
    let normalized = value.trim().trim_start_matches('/');
    normalized.parse::<Tz>().ok().or_else(|| {
        let alias = match normalized {
            "China Standard Time" => "Asia/Shanghai",
            "Tokyo Standard Time" => "Asia/Tokyo",
            "Pacific Standard Time" => "America/Los_Angeles",
            "Mountain Standard Time" => "America/Denver",
            "Central Standard Time" => "America/Chicago",
            "Eastern Standard Time" => "America/New_York",
            "GMT Standard Time" => "Europe/London",
            "W. Europe Standard Time" => "Europe/Berlin",
            _ => return None,
        };
        alias.parse::<Tz>().ok()
    })
}

fn local_datetime(timezone: Tz, value: NaiveDateTime) -> Result<DateTime<Tz>, String> {
    timezone
        .from_local_datetime(&value)
        .single()
        .ok_or_else(|| "日历时间落在夏令时切换的无效或重复区间".to_string())
}

fn clean_title(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = compact.chars().take(MAX_TITLE_CHARS).collect::<String>();
    if title.is_empty() {
        "未命名日程".to_string()
    } else {
        title
    }
}

fn stable_event_id(uid: &str, occurrence_ms: i64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(uid.as_bytes());
    hasher.update(b"|");
    hasher.update(occurrence_ms.to_string().as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("calendar-{}", &digest[..24])
}

#[cfg(test)]
mod tests {
    use super::preview_ics;

    fn calendar(events: &str) -> String {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Agent Bar Test//EN\r\nX-WR-CALNAME:测试日历\r\n{events}END:VCALENDAR\r\n"
        )
    }

    #[test]
    fn previews_utc_event_in_viewer_timezone() {
        let ics = calendar(
            "BEGIN:VEVENT\r\nUID:private-uid@example.com\r\nDTSTART:20260722T010000Z\r\nDTEND:20260722T023000Z\r\nSUMMARY:项目讨论\r\nEND:VEVENT\r\n",
        );
        let preview = preview_ics(&ics, "2026-07-22", "Asia/Shanghai").unwrap();

        assert_eq!(preview.source_name, "测试日历");
        assert_eq!(preview.events.len(), 1);
        assert_eq!(preview.events[0].start_minute, 9 * 60);
        assert_eq!(preview.events[0].end_minute, 10 * 60 + 30);
        assert!(!preview.events[0].id.contains("private-uid"));
    }

    #[test]
    fn expands_daily_recurrence_for_target_day() {
        let ics = calendar(
            "BEGIN:VEVENT\r\nUID:daily-focus\r\nDTSTART;TZID=Asia/Shanghai:20260720T090000\r\nDTEND;TZID=Asia/Shanghai:20260720T100000\r\nRRULE:FREQ=DAILY;COUNT=5\r\nSUMMARY:晨间专注\r\nEND:VEVENT\r\n",
        );
        let preview = preview_ics(&ics, "2026-07-22", "Asia/Shanghai").unwrap();

        assert_eq!(preview.events.len(), 1);
        assert_eq!(preview.events[0].start_minute, 9 * 60);
        assert!(preview.events[0].recurring);
    }

    #[test]
    fn clips_cross_day_and_all_day_events() {
        let ics = calendar(
            "BEGIN:VEVENT\r\nUID:overnight\r\nDTSTART;TZID=Asia/Shanghai:20260721T233000\r\nDTEND;TZID=Asia/Shanghai:20260722T010000\r\nSUMMARY:夜间行程\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:all-day\r\nDTSTART;VALUE=DATE:20260722\r\nDTEND;VALUE=DATE:20260723\r\nSUMMARY:全天事项\r\nEND:VEVENT\r\n",
        );
        let preview = preview_ics(&ics, "2026-07-22", "Asia/Shanghai").unwrap();

        assert_eq!(preview.events.len(), 2);
        assert_eq!(preview.events[0].start_minute, 0);
        assert_eq!(preview.events[0].end_minute, 60);
        assert_eq!(preview.events[1].start_minute, 0);
        assert_eq!(preview.events[1].end_minute, 1440);
        assert!(preview.events[1].all_day);
    }

    #[test]
    fn skips_cancelled_events_and_accepts_windows_timezone_alias() {
        let ics = calendar(
            "BEGIN:VEVENT\r\nUID:cancelled\r\nDTSTART;TZID=China Standard Time:20260722T090000\r\nDTEND;TZID=China Standard Time:20260722T100000\r\nSTATUS:CANCELLED\r\nSUMMARY:已取消\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:active\r\nDTSTART;TZID=China Standard Time:20260722T110000\r\nDTEND;TZID=China Standard Time:20260722T120000\r\nSUMMARY:保留事项\r\nEND:VEVENT\r\n",
        );
        let preview = preview_ics(&ics, "2026-07-22", "Asia/Shanghai").unwrap();

        assert_eq!(preview.events.len(), 1);
        assert_eq!(preview.events[0].title, "保留事项");
        assert_eq!(preview.skipped_count, 1);
    }
}
