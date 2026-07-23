use crate::calendar_connections::{self, CalendarSyncBatch};
use crate::database::Database;
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Utc};
use chrono_tz::Tz;
use std::str::FromStr;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub const CALENDAR_SYNC_UPDATED_EVENT: &str = "calendar-sync-updated";
pub const CALENDAR_SYNC_FAILED_EVENT: &str = "calendar-sync-failed";
const POLL_INTERVAL: Duration = Duration::from_secs(60);

pub struct CalendarSyncScheduler {
    wake_sender: SyncSender<()>,
    sync_gate: Arc<Mutex<()>>,
}

impl CalendarSyncScheduler {
    pub fn start(app: AppHandle, database: Arc<Database>) -> Result<Self, String> {
        let (wake_sender, wake_receiver) = sync_channel(1);
        let sync_gate = Arc::new(Mutex::new(()));
        let worker_gate = sync_gate.clone();
        std::thread::Builder::new()
            .name("calendar-sync".to_string())
            .spawn(move || run_scheduler(app, database, worker_gate, wake_receiver))
            .map_err(|error| format!("could not start calendar sync scheduler: {error}"))?;
        Ok(Self {
            wake_sender,
            sync_gate,
        })
    }

    pub fn wake(&self) {
        match self.wake_sender.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => {}
            Err(TrySendError::Disconnected(())) => {}
        }
    }

    pub fn gate(&self) -> Arc<Mutex<()>> {
        self.sync_gate.clone()
    }
}

pub fn validate_timezone(value: &str) -> Result<Tz, String> {
    Tz::from_str(value).map_err(|_| "日历时区必须是有效的 IANA 时区".to_string())
}

pub fn week_sync_window(
    now: DateTime<Utc>,
    viewer_timezone: &str,
) -> Result<(String, Vec<String>), String> {
    let timezone = validate_timezone(viewer_timezone)?;
    let today = now.with_timezone(&timezone).date_naive();
    let monday = today - ChronoDuration::days(today.weekday().num_days_from_monday().into());
    let days = (0..7)
        .map(|offset| {
            (monday + ChronoDuration::days(offset))
                .format("%Y-%m-%d")
                .to_string()
        })
        .collect();
    Ok((today.format("%Y-%m-%d").to_string(), days))
}

pub fn lock_sync_gate(gate: &Mutex<()>) -> Result<MutexGuard<'_, ()>, String> {
    gate.lock()
        .map_err(|_| "calendar sync lock was poisoned".to_string())
}

fn sync_cycle_at(
    database: &Database,
    sync_gate: &Mutex<()>,
    now: DateTime<Utc>,
) -> Result<Option<CalendarSyncBatch>, String> {
    let Some(timezone) = database.calendar_timezone()? else {
        return Ok(None);
    };
    let (day, days) = week_sync_window(now, &timezone)?;
    let _guard = lock_sync_gate(sync_gate)?;
    calendar_connections::sync_connections_range(database, &day, &days, &timezone, None, false)
        .map(Some)
}

fn run_scheduler(
    app: AppHandle,
    database: Arc<Database>,
    sync_gate: Arc<Mutex<()>>,
    wake_receiver: Receiver<()>,
) {
    while let Ok(()) | Err(RecvTimeoutError::Timeout) = wake_receiver.recv_timeout(POLL_INTERVAL) {
        match sync_cycle_at(&database, &sync_gate, Utc::now()) {
            Ok(Some(batch)) => {
                let _ = app.emit(CALENDAR_SYNC_UPDATED_EVENT, batch);
            }
            Ok(None) => {}
            Err(error) => {
                let _ = app.emit(CALENDAR_SYNC_FAILED_EVENT, error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sync_cycle_at, week_sync_window};
    use crate::calendar_connections::{create_connection, delete_connection};
    use crate::database::Database;
    use chrono::{TimeZone, Utc};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn week_window_uses_viewer_timezone_across_utc_midnight() {
        let now = Utc
            .with_ymd_and_hms(2026, 7, 19, 16, 30, 0)
            .single()
            .expect("valid date");
        let (day, days) = week_sync_window(now, "Asia/Shanghai").expect("week window");

        assert_eq!(day, "2026-07-20");
        assert_eq!(days.first().map(String::as_str), Some("2026-07-20"));
        assert_eq!(days.last().map(String::as_str), Some("2026-07-26"));
    }

    #[test]
    fn week_window_rejects_unknown_timezone() {
        assert!(week_sync_window(Utc::now(), "Asia/Not-A-Place").is_err());
    }

    #[test]
    fn sync_cycle_reads_persisted_timezone_and_returns_the_current_week() {
        let database = Database::in_memory();
        database
            .set_calendar_timezone("Asia/Shanghai")
            .expect("save timezone");
        let now = Utc
            .with_ymd_and_hms(2026, 7, 19, 16, 30, 0)
            .single()
            .expect("valid date");

        let batch = sync_cycle_at(&database, &Mutex::new(()), now)
            .expect("sync cycle")
            .expect("configured scheduler");

        assert_eq!(batch.schedule_days.len(), 7);
        assert_eq!(batch.schedule_days[0].day, "2026-07-20");
        assert_eq!(batch.schedule_days[6].day, "2026-07-26");
        assert_eq!(batch.synced_count, 0);
    }

    #[test]
    #[ignore = "uses the current Windows Credential Manager for a background local-file cycle"]
    fn sync_cycle_updates_a_local_file_connection() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("agent-bar-scheduler-{unique}.ics"));
        std::fs::write(
            &path,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:scheduler-event\r\nDTSTART:20260722T010000Z\r\nDTEND:20260722T020000Z\r\nSUMMARY:Background sync\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        )
        .expect("write calendar fixture");
        let database = Database::in_memory();
        database
            .set_calendar_timezone("Asia/Shanghai")
            .expect("save timezone");
        let connection = create_connection(
            &database,
            "Scheduler fixture",
            "local-file",
            path.to_str().expect("UTF-8 fixture path"),
            15,
        )
        .expect("create connection");
        let now = Utc
            .with_ymd_and_hms(2026, 7, 22, 2, 0, 0)
            .single()
            .expect("valid date");

        let batch = sync_cycle_at(&database, &Mutex::new(()), now)
            .expect("sync cycle")
            .expect("configured scheduler");

        assert_eq!(batch.synced_count, 1);
        let event_day = batch
            .schedule_days
            .iter()
            .find(|day| day.day == "2026-07-22")
            .expect("event day");
        assert_eq!(event_day.blocks[0].title, "Background sync");

        delete_connection(&database, &connection.id).expect("delete connection");
        std::fs::remove_file(path).expect("remove fixture");
    }
}
