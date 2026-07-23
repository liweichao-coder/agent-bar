use crate::activity::ForegroundSnapshot;
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_ACTIVITY_RETENTION_DAYS: i64 = 30;
const DEFAULT_ACTIVITY_IDLE_THRESHOLD_MINUTES: i64 = 5;
const MIN_ACTIVITY_IDLE_THRESHOLD_MINUTES: i64 = 1;
const MAX_ACTIVITY_IDLE_THRESHOLD_MINUTES: i64 = 60;
const MIN_ACTIVITY_RETENTION_DAYS: i64 = 1;
const MAX_ACTIVITY_RETENTION_DAYS: i64 = 3_650;
const MAX_EXCLUDED_APPS: usize = 100;

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleBlock {
    pub id: String,
    pub title: String,
    pub start_minute: i32,
    pub end_minute: i32,
    pub category: String,
    pub source: String,
    pub status: String,
    pub locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleDay {
    pub day: String,
    pub blocks: Vec<ScheduleBlock>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerTask {
    pub id: String,
    pub title: String,
    pub duration_minutes: i32,
    pub priority: String,
    pub preferred_period: String,
    pub category: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarConnection {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub source_hint: String,
    pub enabled: bool,
    pub refresh_minutes: i64,
    pub last_sync_at_ms: Option<i64>,
    pub last_sync_status: String,
    pub last_error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    pub id: String,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub app_name: String,
    pub sanitized_window_title: Option<String>,
    pub category: String,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDayRange {
    pub day: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMinutesSummary {
    pub focus: i64,
    pub meeting: i64,
    pub admin: i64,
    pub life: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMinutesSummary {
    pub app_name: String,
    pub minutes: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDaySummary {
    pub day: String,
    pub planned_minutes: i64,
    pub actual_minutes: i64,
    pub categories: CategoryMinutesSummary,
    pub top_apps: Vec<AppMinutesSummary>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventRecord {
    pub id: String,
    pub provider: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub agent_id: Option<String>,
    pub event_name: String,
    pub activity_kind: String,
    pub occurred_at_ms: i64,
    pub metadata_json: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalState {
    pub schedule_blocks: Vec<ScheduleBlock>,
    pub planner_tasks: Vec<PlannerTask>,
    pub calendar_connections: Vec<CalendarConnection>,
    pub activity_records: Vec<ActivityRecord>,
    pub tracking_enabled: bool,
    pub capture_window_titles: bool,
    pub codex_observation_enabled: bool,
    pub excluded_activity_apps: Vec<String>,
    pub activity_retention_days: i64,
    pub activity_idle_threshold_minutes: i64,
    pub window_mode: String,
    pub morning_prompt: MorningPromptState,
    pub storage_kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MorningPromptState {
    pub enabled: bool,
    pub prompt_minute: i64,
    pub dismissed_day: Option<String>,
    pub snoozed_until_ms: Option<i64>,
    pub last_planned_day: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPrivacyUpdate {
    pub excluded_activity_apps: Vec<String>,
    pub activity_retention_days: i64,
    pub deleted_records: usize,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create app data directory: {error}"))?;
        }
        let connection = Connection::open(path)
            .map_err(|error| format!("could not open local database: {error}"))?;
        Self::migrate(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> Self {
        let connection = Connection::open_in_memory().expect("in-memory database");
        Self::migrate(&connection).expect("database migration");
        Self {
            connection: Mutex::new(connection),
        }
    }

    fn migrate(connection: &Connection) -> Result<(), String> {
        let current_version = connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .map_err(|error| format!("could not read database version: {error}"))?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS schedule_blocks (
                   day TEXT NOT NULL,
                   id TEXT NOT NULL,
                   title TEXT NOT NULL,
                   start_minute INTEGER NOT NULL,
                   end_minute INTEGER NOT NULL,
                   category TEXT NOT NULL,
                   source TEXT NOT NULL,
                   status TEXT NOT NULL,
                   locked INTEGER NOT NULL DEFAULT 0,
                   PRIMARY KEY (day, id),
                   CHECK (start_minute >= 0 AND end_minute <= 1440 AND end_minute > start_minute)
                 );
                 CREATE INDEX IF NOT EXISTS schedule_blocks_day_start
                   ON schedule_blocks(day, start_minute);
                 CREATE TABLE IF NOT EXISTS planner_tasks (
                   id TEXT PRIMARY KEY,
                   title TEXT NOT NULL,
                   duration_minutes INTEGER NOT NULL,
                   priority TEXT NOT NULL,
                   preferred_period TEXT NOT NULL,
                   category TEXT NOT NULL,
                   notes TEXT,
                   CHECK (duration_minutes >= 15 AND duration_minutes <= 480),
                   CHECK (priority IN ('critical', 'high', 'normal', 'low')),
                   CHECK (preferred_period IN ('any', 'morning', 'afternoon', 'evening')),
                   CHECK (category IN ('focus', 'meeting', 'life', 'admin'))
                 );
                 CREATE TABLE IF NOT EXISTS activity_records (
                   id TEXT PRIMARY KEY,
                   started_at_ms INTEGER NOT NULL,
                   ended_at_ms INTEGER NOT NULL,
                   app_name TEXT NOT NULL,
                   sanitized_window_title TEXT,
                   category TEXT NOT NULL,
                   source TEXT NOT NULL,
                   CHECK (ended_at_ms >= started_at_ms)
                 );
                 CREATE INDEX IF NOT EXISTS activity_records_time
                   ON activity_records(started_at_ms, ended_at_ms);
                 CREATE TABLE IF NOT EXISTS activity_exclusions (
                   app_name TEXT PRIMARY KEY COLLATE NOCASE,
                   created_at_ms INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS settings (
                   key TEXT PRIMARY KEY,
                   value TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS agent_events (
                   id TEXT PRIMARY KEY,
                   provider TEXT NOT NULL,
                   session_id TEXT NOT NULL,
                   turn_id TEXT,
                   agent_id TEXT,
                   event_name TEXT NOT NULL,
                   activity_kind TEXT NOT NULL,
                   occurred_at_ms INTEGER NOT NULL,
                   metadata_json TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS agent_events_session_time
                   ON agent_events(session_id, occurred_at_ms);
                 CREATE TABLE IF NOT EXISTS calendar_connections (
                   id TEXT PRIMARY KEY,
                   display_name TEXT NOT NULL,
                   kind TEXT NOT NULL,
                   source_hint TEXT NOT NULL,
                   enabled INTEGER NOT NULL DEFAULT 1,
                   refresh_minutes INTEGER NOT NULL DEFAULT 30,
                   last_sync_at_ms INTEGER,
                   last_sync_status TEXT NOT NULL DEFAULT 'never',
                   last_error TEXT,
                   created_at_ms INTEGER NOT NULL,
                   updated_at_ms INTEGER NOT NULL,
                   CHECK (kind IN ('local-file', 'ics-subscription')),
                   CHECK (refresh_minutes >= 15 AND refresh_minutes <= 1440),
                   CHECK (last_sync_status IN ('never', 'success', 'error'))
                 );
                 INSERT OR IGNORE INTO settings(key, value) VALUES
                   ('tracking_enabled', 'false'),
                   ('capture_window_titles', 'false'),
                   ('codex_observation_enabled', 'false'),
                   ('activity_retention_days', '30'),
                   ('activity_idle_threshold_minutes', '5'),
                   ('morning_prompt_enabled', 'true'),
                   ('morning_prompt_minute', '480'),
                   ('morning_prompt_dismissed_day', ''),
                   ('morning_prompt_snoozed_until_ms', '0'),
                   ('morning_plan_last_day', ''),
                   ('calendar_timezone', ''),
                   ('window_mode', 'expanded');",
            )
            .map_err(|error| format!("could not migrate local database: {error}"))?;
        if current_version < 2 {
            connection
                .execute(
                    "UPDATE settings SET value = 'false' WHERE key = 'capture_window_titles'",
                    [],
                )
                .map_err(|error| format!("could not apply privacy-default migration: {error}"))?;
        }
        connection
            .execute_batch("PRAGMA user_version = 10;")
            .map_err(|error| format!("could not update database version: {error}"))?;
        Ok(())
    }

    pub fn load_state(
        &self,
        day: &str,
        day_start_ms: i64,
        day_end_ms: i64,
    ) -> Result<LocalState, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;

        let mut schedule_statement = connection
            .prepare(
                "SELECT id, title, start_minute, end_minute, category, source, status, locked
                 FROM schedule_blocks WHERE day = ?1 ORDER BY start_minute, id",
            )
            .map_err(|error| format!("could not prepare schedule query: {error}"))?;
        let schedule_blocks = schedule_statement
            .query_map([day], |row| {
                Ok(ScheduleBlock {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    start_minute: row.get(2)?,
                    end_minute: row.get(3)?,
                    category: row.get(4)?,
                    source: row.get(5)?,
                    status: row.get(6)?,
                    locked: Some(row.get::<_, i64>(7)? != 0),
                })
            })
            .map_err(|error| format!("could not query schedules: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read schedules: {error}"))?;

        let mut activity_statement = connection
            .prepare(
                "SELECT id, started_at_ms, ended_at_ms, app_name, sanitized_window_title,
                        category, source
                 FROM activity_records
                 WHERE ended_at_ms >= ?1 AND started_at_ms < ?2
                 ORDER BY started_at_ms, id",
            )
            .map_err(|error| format!("could not prepare activity query: {error}"))?;
        let activity_records = activity_statement
            .query_map(params![day_start_ms, day_end_ms], |row| {
                Ok(ActivityRecord {
                    id: row.get(0)?,
                    started_at_ms: row.get(1)?,
                    ended_at_ms: row.get(2)?,
                    app_name: row.get(3)?,
                    sanitized_window_title: row.get(4)?,
                    category: row.get(5)?,
                    source: row.get(6)?,
                })
            })
            .map_err(|error| format!("could not query activities: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read activities: {error}"))?;

        let mut planner_statement = connection
            .prepare(
                "SELECT id, title, duration_minutes, priority, preferred_period, category, notes
                 FROM planner_tasks ORDER BY rowid",
            )
            .map_err(|error| format!("could not prepare planner task query: {error}"))?;
        let planner_tasks = planner_statement
            .query_map([], |row| {
                Ok(PlannerTask {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    duration_minutes: row.get(2)?,
                    priority: row.get(3)?,
                    preferred_period: row.get(4)?,
                    category: row.get(5)?,
                    notes: row.get(6)?,
                })
            })
            .map_err(|error| format!("could not query planner tasks: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read planner tasks: {error}"))?;

        let excluded_activity_apps = Self::excluded_activity_apps(&connection)?;
        let calendar_connections = Self::calendar_connections_with(&connection)?;

        let dismissed_day = Self::setting_text(&connection, "morning_prompt_dismissed_day", "")?;
        let last_planned_day = Self::setting_text(&connection, "morning_plan_last_day", "")?;
        let snoozed_until_ms =
            Self::setting_i64(&connection, "morning_prompt_snoozed_until_ms", 0)?;

        Ok(LocalState {
            schedule_blocks,
            planner_tasks,
            calendar_connections,
            activity_records,
            tracking_enabled: Self::setting_bool(&connection, "tracking_enabled", false)?,
            capture_window_titles: Self::setting_bool(&connection, "capture_window_titles", false)?,
            codex_observation_enabled: Self::setting_bool(
                &connection,
                "codex_observation_enabled",
                false,
            )?,
            excluded_activity_apps,
            activity_retention_days: Self::setting_i64(
                &connection,
                "activity_retention_days",
                DEFAULT_ACTIVITY_RETENTION_DAYS,
            )?,
            activity_idle_threshold_minutes: Self::setting_i64(
                &connection,
                "activity_idle_threshold_minutes",
                DEFAULT_ACTIVITY_IDLE_THRESHOLD_MINUTES,
            )?
            .clamp(
                MIN_ACTIVITY_IDLE_THRESHOLD_MINUTES,
                MAX_ACTIVITY_IDLE_THRESHOLD_MINUTES,
            ),
            window_mode: Self::setting_text(&connection, "window_mode", "expanded")?,
            morning_prompt: MorningPromptState {
                enabled: Self::setting_bool(&connection, "morning_prompt_enabled", true)?,
                prompt_minute: Self::setting_i64(&connection, "morning_prompt_minute", 8 * 60)?,
                dismissed_day: (!dismissed_day.is_empty()).then_some(dismissed_day),
                snoozed_until_ms: (snoozed_until_ms > 0).then_some(snoozed_until_ms),
                last_planned_day: (!last_planned_day.is_empty()).then_some(last_planned_day),
            },
            storage_kind: "sqlite",
        })
    }

    pub fn replace_schedule(&self, day: &str, blocks: &[ScheduleBlock]) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start schedule transaction: {error}"))?;
        transaction
            .execute("DELETE FROM schedule_blocks WHERE day = ?1", [day])
            .map_err(|error| format!("could not replace schedule: {error}"))?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO schedule_blocks(
                       day, id, title, start_minute, end_minute, category, source, status, locked
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(|error| format!("could not prepare schedule insert: {error}"))?;
            for block in blocks {
                statement
                    .execute(params![
                        day,
                        block.id,
                        block.title,
                        block.start_minute,
                        block.end_minute,
                        block.category,
                        block.source,
                        block.status,
                        block.locked.unwrap_or(false),
                    ])
                    .map_err(|error| {
                        format!("could not save schedule block {}: {error}", block.id)
                    })?;
            }
        }
        transaction
            .commit()
            .map_err(|error| format!("could not commit schedule: {error}"))
    }

    pub fn schedule_for_day(&self, day: &str) -> Result<Vec<ScheduleBlock>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        Self::schedule_for_day_with(&connection, day)
    }

    pub fn schedules_for_days(&self, days: &[String]) -> Result<Vec<ScheduleDay>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        days.iter()
            .map(|day| {
                Ok(ScheduleDay {
                    day: day.clone(),
                    blocks: Self::schedule_for_day_with(&connection, day)?,
                })
            })
            .collect()
    }

    pub fn calendar_connections(&self) -> Result<Vec<CalendarConnection>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        Self::calendar_connections_with(&connection)
    }

    pub fn calendar_connection(&self, id: &str) -> Result<Option<CalendarConnection>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        connection
            .query_row(
                "SELECT id, display_name, kind, source_hint, enabled, refresh_minutes,
                        last_sync_at_ms, last_sync_status, last_error, created_at_ms, updated_at_ms
                 FROM calendar_connections WHERE id = ?1",
                [id],
                Self::calendar_connection_from_row,
            )
            .optional()
            .map_err(|error| format!("could not read calendar connection: {error}"))
    }

    pub fn insert_calendar_connection(
        &self,
        connection: &CalendarConnection,
    ) -> Result<(), String> {
        let database = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        database
            .execute(
                "INSERT INTO calendar_connections(
                   id, display_name, kind, source_hint, enabled, refresh_minutes,
                   last_sync_at_ms, last_sync_status, last_error, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    connection.id,
                    connection.display_name,
                    connection.kind,
                    connection.source_hint,
                    connection.enabled,
                    connection.refresh_minutes,
                    connection.last_sync_at_ms,
                    connection.last_sync_status,
                    connection.last_error,
                    connection.created_at_ms,
                    connection.updated_at_ms,
                ],
            )
            .map_err(|error| format!("could not save calendar connection: {error}"))?;
        Ok(())
    }

    pub fn set_calendar_connection_enabled(
        &self,
        id: &str,
        enabled: bool,
        updated_at_ms: i64,
    ) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let changed = connection
            .execute(
                "UPDATE calendar_connections SET enabled = ?2, updated_at_ms = ?3 WHERE id = ?1",
                params![id, enabled, updated_at_ms],
            )
            .map_err(|error| format!("could not update calendar connection: {error}"))?;
        if changed == 0 {
            return Err("calendar connection was not found".to_string());
        }
        Ok(())
    }

    pub fn update_calendar_sync_state(
        &self,
        id: &str,
        synced_at_ms: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        connection
            .execute(
                "UPDATE calendar_connections
                 SET last_sync_at_ms = ?2, last_sync_status = ?3, last_error = ?4,
                     updated_at_ms = ?2
                 WHERE id = ?1",
                params![id, synced_at_ms, status, error],
            )
            .map_err(|error| format!("could not update calendar sync state: {error}"))?;
        Ok(())
    }

    #[cfg(test)]
    pub fn replace_calendar_schedule(
        &self,
        day: &str,
        connection_id: &str,
        blocks: &[ScheduleBlock],
    ) -> Result<(), String> {
        self.replace_calendar_schedule_days(
            connection_id,
            &[ScheduleDay {
                day: day.to_string(),
                blocks: blocks.to_vec(),
            }],
        )
    }

    pub fn replace_calendar_schedule_days(
        &self,
        connection_id: &str,
        days: &[ScheduleDay],
    ) -> Result<(), String> {
        let source = format!("calendar:{connection_id}");
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start calendar sync transaction: {error}"))?;
        for day in days {
            transaction
                .execute(
                    "DELETE FROM schedule_blocks WHERE day = ?1 AND source = ?2",
                    params![day.day, source],
                )
                .map_err(|error| format!("could not clear previous calendar schedule: {error}"))?;
        }
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO schedule_blocks(
                       day, id, title, start_minute, end_minute, category, source, status, locked
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(|error| format!("could not prepare calendar schedule insert: {error}"))?;
            for day in days {
                for block in &day.blocks {
                    statement
                        .execute(params![
                            day.day,
                            block.id,
                            block.title,
                            block.start_minute,
                            block.end_minute,
                            block.category,
                            block.source,
                            block.status,
                            block.locked.unwrap_or(true),
                        ])
                        .map_err(|error| {
                            format!("could not save synced calendar event: {error}")
                        })?;
                }
            }
        }
        transaction
            .commit()
            .map_err(|error| format!("could not commit calendar sync: {error}"))
    }

    pub fn delete_calendar_connection(&self, id: &str) -> Result<(), String> {
        let source = format!("calendar:{id}");
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start calendar delete transaction: {error}"))?;
        transaction
            .execute("DELETE FROM schedule_blocks WHERE source = ?1", [source])
            .map_err(|error| format!("could not delete connected calendar events: {error}"))?;
        transaction
            .execute("DELETE FROM calendar_connections WHERE id = ?1", [id])
            .map_err(|error| format!("could not delete calendar connection: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("could not commit calendar deletion: {error}"))
    }

    fn schedule_for_day_with(
        connection: &Connection,
        day: &str,
    ) -> Result<Vec<ScheduleBlock>, String> {
        let mut statement = connection
            .prepare(
                "SELECT id, title, start_minute, end_minute, category, source, status, locked
                 FROM schedule_blocks WHERE day = ?1 ORDER BY start_minute, id",
            )
            .map_err(|error| format!("could not prepare schedule query: {error}"))?;
        let rows = statement
            .query_map([day], |row| {
                Ok(ScheduleBlock {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    start_minute: row.get(2)?,
                    end_minute: row.get(3)?,
                    category: row.get(4)?,
                    source: row.get(5)?,
                    status: row.get(6)?,
                    locked: Some(row.get::<_, i64>(7)? != 0),
                })
            })
            .map_err(|error| format!("could not query schedules: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read schedules: {error}"))?;
        Ok(rows)
    }

    fn calendar_connection_from_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<CalendarConnection> {
        Ok(CalendarConnection {
            id: row.get(0)?,
            display_name: row.get(1)?,
            kind: row.get(2)?,
            source_hint: row.get(3)?,
            enabled: row.get::<_, i64>(4)? != 0,
            refresh_minutes: row.get(5)?,
            last_sync_at_ms: row.get(6)?,
            last_sync_status: row.get(7)?,
            last_error: row.get(8)?,
            created_at_ms: row.get(9)?,
            updated_at_ms: row.get(10)?,
        })
    }

    fn calendar_connections_with(
        connection: &Connection,
    ) -> Result<Vec<CalendarConnection>, String> {
        let mut statement = connection
            .prepare(
                "SELECT id, display_name, kind, source_hint, enabled, refresh_minutes,
                        last_sync_at_ms, last_sync_status, last_error, created_at_ms, updated_at_ms
                 FROM calendar_connections ORDER BY created_at_ms, id",
            )
            .map_err(|error| format!("could not prepare calendar connections query: {error}"))?;
        let rows = statement
            .query_map([], Self::calendar_connection_from_row)
            .map_err(|error| format!("could not query calendar connections: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read calendar connections: {error}"))?;
        Ok(rows)
    }

    pub fn replace_planner_tasks(&self, tasks: &[PlannerTask]) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start planner task transaction: {error}"))?;
        transaction
            .execute("DELETE FROM planner_tasks", [])
            .map_err(|error| format!("could not replace planner tasks: {error}"))?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO planner_tasks(
                       id, title, duration_minutes, priority, preferred_period, category, notes
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(|error| format!("could not prepare planner task insert: {error}"))?;
            for task in tasks {
                statement
                    .execute(params![
                        task.id,
                        task.title,
                        task.duration_minutes,
                        task.priority,
                        task.preferred_period,
                        task.category,
                        task.notes,
                    ])
                    .map_err(|error| format!("could not save planner task {}: {error}", task.id))?;
            }
        }
        transaction
            .commit()
            .map_err(|error| format!("could not commit planner tasks: {error}"))
    }

    pub fn apply_morning_plan(
        &self,
        day: &str,
        blocks: &[ScheduleBlock],
        tasks: &[PlannerTask],
    ) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start morning plan transaction: {error}"))?;

        transaction
            .execute("DELETE FROM schedule_blocks WHERE day = ?1", [day])
            .map_err(|error| format!("could not replace morning schedule: {error}"))?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO schedule_blocks(
                       day, id, title, start_minute, end_minute, category, source, status, locked
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(|error| format!("could not prepare morning schedule insert: {error}"))?;
            for block in blocks {
                statement
                    .execute(params![
                        day,
                        block.id,
                        block.title,
                        block.start_minute,
                        block.end_minute,
                        block.category,
                        block.source,
                        block.status,
                        block.locked.unwrap_or(false),
                    ])
                    .map_err(|error| {
                        format!(
                            "could not save morning schedule block {}: {error}",
                            block.id
                        )
                    })?;
            }
        }

        for (key, value) in [
            ("morning_plan_last_day", day.to_string()),
            ("morning_prompt_dismissed_day", String::new()),
            ("morning_prompt_snoozed_until_ms", "0".to_string()),
        ] {
            transaction
                .execute(
                    "INSERT INTO settings(key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![key, value],
                )
                .map_err(|error| format!("could not update morning plan state: {error}"))?;
        }

        transaction
            .execute("DELETE FROM planner_tasks", [])
            .map_err(|error| format!("could not replace morning planner tasks: {error}"))?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO planner_tasks(
                       id, title, duration_minutes, priority, preferred_period, category, notes
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(|error| {
                    format!("could not prepare morning planner task insert: {error}")
                })?;
            for task in tasks {
                statement
                    .execute(params![
                        task.id,
                        task.title,
                        task.duration_minutes,
                        task.priority,
                        task.preferred_period,
                        task.category,
                        task.notes,
                    ])
                    .map_err(|error| {
                        format!("could not save morning planner task {}: {error}", task.id)
                    })?;
            }
        }

        transaction
            .commit()
            .map_err(|error| format!("could not commit morning plan: {error}"))
    }

    pub fn set_morning_prompt_settings(
        &self,
        enabled: bool,
        prompt_minute: i64,
    ) -> Result<(), String> {
        if !(0..=1439).contains(&prompt_minute) {
            return Err("morning prompt minute must be between 0 and 1439".to_string());
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start morning prompt transaction: {error}"))?;
        for (key, value) in [
            ("morning_prompt_enabled", enabled.to_string()),
            ("morning_prompt_minute", prompt_minute.to_string()),
        ] {
            transaction
                .execute(
                    "INSERT INTO settings(key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![key, value],
                )
                .map_err(|error| format!("could not update setting {key}: {error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("could not commit morning prompt settings: {error}"))
    }

    pub fn snooze_morning_prompt(&self, until_ms: i64) -> Result<(), String> {
        if until_ms <= 0 {
            return Err("morning prompt snooze time must be positive".to_string());
        }
        self.set_text_setting("morning_prompt_snoozed_until_ms", &until_ms.to_string())
    }

    pub fn dismiss_morning_prompt(&self, day: &str) -> Result<(), String> {
        if day.len() != 10
            || !day.chars().enumerate().all(|(index, character)| {
                if index == 4 || index == 7 {
                    character == '-'
                } else {
                    character.is_ascii_digit()
                }
            })
        {
            return Err("morning prompt day must use YYYY-MM-DD".to_string());
        }
        self.set_text_setting("morning_prompt_dismissed_day", day)
    }

    pub fn set_tracking(&self, enabled: bool) -> Result<(), String> {
        self.set_setting("tracking_enabled", enabled)
    }

    pub fn set_capture_window_titles(&self, enabled: bool) -> Result<(), String> {
        self.set_setting("capture_window_titles", enabled)
    }

    pub fn set_codex_observation(&self, enabled: bool) -> Result<(), String> {
        self.set_setting("codex_observation_enabled", enabled)
    }

    pub fn codex_observation_enabled(&self) -> Result<bool, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        Self::setting_bool(&connection, "codex_observation_enabled", false)
    }

    pub fn capture_window_titles(&self) -> Result<bool, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        Self::setting_bool(&connection, "capture_window_titles", false)
    }

    pub fn tracking_enabled(&self) -> Result<bool, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        Self::setting_bool(&connection, "tracking_enabled", false)
    }

    pub fn activity_idle_threshold_minutes(&self) -> Result<i64, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        Ok(Self::setting_i64(
            &connection,
            "activity_idle_threshold_minutes",
            DEFAULT_ACTIVITY_IDLE_THRESHOLD_MINUTES,
        )?
        .clamp(
            MIN_ACTIVITY_IDLE_THRESHOLD_MINUTES,
            MAX_ACTIVITY_IDLE_THRESHOLD_MINUTES,
        ))
    }

    pub fn set_activity_idle_threshold_minutes(&self, minutes: i64) -> Result<(), String> {
        if !(MIN_ACTIVITY_IDLE_THRESHOLD_MINUTES..=MAX_ACTIVITY_IDLE_THRESHOLD_MINUTES)
            .contains(&minutes)
        {
            return Err(format!(
                "activity idle threshold must be between {MIN_ACTIVITY_IDLE_THRESHOLD_MINUTES} and {MAX_ACTIVITY_IDLE_THRESHOLD_MINUTES} minutes"
            ));
        }
        self.set_text_setting("activity_idle_threshold_minutes", &minutes.to_string())
    }

    pub fn window_mode(&self) -> Result<String, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let mode = Self::setting_text(&connection, "window_mode", "expanded")?;
        Ok(if mode == "compact" {
            "compact".to_string()
        } else {
            "expanded".to_string()
        })
    }

    pub fn set_window_mode(&self, mode: &str) -> Result<(), String> {
        if !matches!(mode, "compact" | "expanded") {
            return Err("window mode must be compact or expanded".to_string());
        }
        self.set_text_setting("window_mode", mode)
    }

    pub fn calendar_timezone(&self) -> Result<Option<String>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let timezone = Self::setting_text(&connection, "calendar_timezone", "")?;
        Ok((!timezone.is_empty()).then_some(timezone))
    }

    pub fn set_calendar_timezone(&self, timezone: &str) -> Result<(), String> {
        self.set_text_setting("calendar_timezone", timezone)
    }

    pub fn set_activity_privacy(
        &self,
        excluded_apps: &[String],
        retention_days: i64,
    ) -> Result<ActivityPrivacyUpdate, String> {
        if !(MIN_ACTIVITY_RETENTION_DAYS..=MAX_ACTIVITY_RETENTION_DAYS).contains(&retention_days) {
            return Err(format!(
                "activity retention must be between {MIN_ACTIVITY_RETENTION_DAYS} and {MAX_ACTIVITY_RETENTION_DAYS} days"
            ));
        }
        let excluded_apps = normalize_excluded_apps(excluded_apps)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start privacy transaction: {error}"))?;
        transaction
            .execute("DELETE FROM activity_exclusions", [])
            .map_err(|error| format!("could not replace activity exclusions: {error}"))?;
        {
            let mut statement = transaction
                .prepare("INSERT INTO activity_exclusions(app_name, created_at_ms) VALUES (?1, ?2)")
                .map_err(|error| format!("could not prepare activity exclusion insert: {error}"))?;
            let created_at_ms = now_ms();
            for app_name in &excluded_apps {
                statement
                    .execute(params![app_name, created_at_ms])
                    .map_err(|error| format!("could not exclude activity app: {error}"))?;
            }
        }
        transaction
            .execute(
                "INSERT INTO settings(key, value) VALUES ('activity_retention_days', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [retention_days.to_string()],
            )
            .map_err(|error| format!("could not update activity retention: {error}"))?;
        let excluded_deleted = transaction
            .execute(
                "DELETE FROM activity_records
                 WHERE EXISTS (
                   SELECT 1 FROM activity_exclusions
                   WHERE activity_exclusions.app_name = activity_records.app_name COLLATE NOCASE
                 )",
                [],
            )
            .map_err(|error| format!("could not delete excluded activity history: {error}"))?;
        let cutoff_ms = retention_cutoff_ms(now_ms(), retention_days);
        let expired_deleted = transaction
            .execute(
                "DELETE FROM activity_records WHERE ended_at_ms < ?1",
                [cutoff_ms],
            )
            .map_err(|error| format!("could not apply activity retention: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("could not commit privacy settings: {error}"))?;
        Ok(ActivityPrivacyUpdate {
            excluded_activity_apps: excluded_apps,
            activity_retention_days: retention_days,
            deleted_records: excluded_deleted + expired_deleted,
        })
    }

    pub fn purge_expired_activity(&self, captured_at_ms: i64) -> Result<usize, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let retention_days = Self::setting_i64(
            &connection,
            "activity_retention_days",
            DEFAULT_ACTIVITY_RETENTION_DAYS,
        )?
        .clamp(MIN_ACTIVITY_RETENTION_DAYS, MAX_ACTIVITY_RETENTION_DAYS);
        connection
            .execute(
                "DELETE FROM activity_records WHERE ended_at_ms < ?1",
                [retention_cutoff_ms(captured_at_ms, retention_days)],
            )
            .map_err(|error| format!("could not purge expired activity: {error}"))
    }

    pub fn clear_activity_records(&self) -> Result<usize, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        connection
            .execute("DELETE FROM activity_records", [])
            .map_err(|error| format!("could not clear activity records: {error}"))
    }

    pub fn load_activity_week_summary(
        &self,
        ranges: &[ActivityDayRange],
    ) -> Result<Vec<ActivityDaySummary>, String> {
        validate_activity_day_ranges(ranges)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let range_start = ranges[0].start_ms;
        let range_end = ranges[ranges.len() - 1].end_ms;

        let mut activity_statement = connection
            .prepare(
                "SELECT started_at_ms, ended_at_ms, app_name, category
                 FROM activity_records
                 WHERE ended_at_ms >= ?1 AND started_at_ms < ?2
                 ORDER BY started_at_ms, id",
            )
            .map_err(|error| format!("could not prepare activity summary query: {error}"))?;
        let activity_records = activity_statement
            .query_map(params![range_start, range_end], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| format!("could not query activity summary: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read activity summary: {error}"))?;

        let mut schedule_statement = connection
            .prepare(
                "SELECT start_minute, end_minute
                 FROM schedule_blocks WHERE day = ?1 ORDER BY start_minute, end_minute",
            )
            .map_err(|error| format!("could not prepare planned summary query: {error}"))?;
        let mut summaries = Vec::with_capacity(ranges.len());

        for range in ranges {
            let schedule_ranges = schedule_statement
                .query_map([&range.day], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(|error| format!("could not query planned summary: {error}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("could not read planned summary: {error}"))?;

            let mut category_ms = HashMap::<String, i64>::new();
            let mut app_ms = HashMap::<String, i64>::new();
            for (started_at_ms, ended_at_ms, app_name, category) in &activity_records {
                let clipped_start = (*started_at_ms).max(range.start_ms);
                let clipped_end = (*ended_at_ms).min(range.end_ms);
                if clipped_end <= clipped_start {
                    continue;
                }
                let duration_ms = clipped_end - clipped_start;
                let category = match category.as_str() {
                    "focus" | "meeting" | "admin" | "life" => category.as_str(),
                    _ => "admin",
                };
                *category_ms.entry(category.to_string()).or_default() += duration_ms;
                *app_ms.entry(app_name.clone()).or_default() += duration_ms;
            }

            let categories = CategoryMinutesSummary {
                focus: rounded_minutes(*category_ms.get("focus").unwrap_or(&0)),
                meeting: rounded_minutes(*category_ms.get("meeting").unwrap_or(&0)),
                admin: rounded_minutes(*category_ms.get("admin").unwrap_or(&0)),
                life: rounded_minutes(*category_ms.get("life").unwrap_or(&0)),
            };
            let actual_minutes = rounded_minutes(category_ms.values().sum());
            let mut top_apps = app_ms
                .into_iter()
                .map(|(app_name, duration_ms)| AppMinutesSummary {
                    app_name,
                    minutes: rounded_minutes(duration_ms),
                })
                .filter(|app| app.minutes > 0)
                .collect::<Vec<_>>();
            top_apps.sort_by(|left, right| {
                right
                    .minutes
                    .cmp(&left.minutes)
                    .then_with(|| left.app_name.cmp(&right.app_name))
            });
            top_apps.truncate(3);

            summaries.push(ActivityDaySummary {
                day: range.day.clone(),
                planned_minutes: union_duration(schedule_ranges),
                actual_minutes,
                categories,
                top_apps,
            });
        }
        Ok(summaries)
    }

    pub fn record_snapshot(
        &self,
        snapshot: &ForegroundSnapshot,
    ) -> Result<Option<ActivityRecord>, String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let excluded = connection
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM activity_exclusions WHERE app_name = ?1 COLLATE NOCASE
                 )",
                [&snapshot.app_name],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| format!("could not check activity exclusion: {error}"))?;
        if excluded {
            return Ok(None);
        }
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start activity transaction: {error}"))?;

        let latest: Option<ActivityRecord> = transaction
            .query_row(
                "SELECT id, started_at_ms, ended_at_ms, app_name, sanitized_window_title,
                        category, source
                 FROM activity_records ORDER BY ended_at_ms DESC LIMIT 1",
                [],
                |row| {
                    Ok(ActivityRecord {
                        id: row.get(0)?,
                        started_at_ms: row.get(1)?,
                        ended_at_ms: row.get(2)?,
                        app_name: row.get(3)?,
                        sanitized_window_title: row.get(4)?,
                        category: row.get(5)?,
                        source: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("could not read latest activity: {error}"))?;

        let record = if let Some(mut latest) = latest.filter(|record| {
            record.app_name == snapshot.app_name
                && record.sanitized_window_title == snapshot.sanitized_window_title
                && snapshot.captured_at_ms - record.ended_at_ms <= 15_000
                && snapshot.captured_at_ms >= record.ended_at_ms
        }) {
            transaction
                .execute(
                    "UPDATE activity_records SET ended_at_ms = ?1 WHERE id = ?2",
                    params![snapshot.captured_at_ms, latest.id],
                )
                .map_err(|error| format!("could not extend activity record: {error}"))?;
            latest.ended_at_ms = snapshot.captured_at_ms;
            latest
        } else {
            let category = classify_app(&snapshot.app_name).to_string();
            let record = ActivityRecord {
                id: format!("activity-{}", snapshot.captured_at_ms),
                started_at_ms: snapshot.captured_at_ms,
                ended_at_ms: snapshot.captured_at_ms,
                app_name: snapshot.app_name.clone(),
                sanitized_window_title: snapshot.sanitized_window_title.clone(),
                category,
                source: "foreground-window".to_string(),
            };
            transaction
                .execute(
                    "INSERT INTO activity_records(
                       id, started_at_ms, ended_at_ms, app_name, sanitized_window_title,
                       category, source
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        record.id,
                        record.started_at_ms,
                        record.ended_at_ms,
                        record.app_name,
                        record.sanitized_window_title,
                        record.category,
                        record.source,
                    ],
                )
                .map_err(|error| format!("could not save activity record: {error}"))?;
            record
        };

        transaction
            .commit()
            .map_err(|error| format!("could not commit activity record: {error}"))?;
        Ok(Some(record))
    }

    pub fn save_agent_events(&self, events: &[AgentEventRecord]) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("could not start agent event transaction: {error}"))?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT OR IGNORE INTO agent_events(
                       id, provider, session_id, turn_id, agent_id, event_name,
                       activity_kind, occurred_at_ms, metadata_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(|error| format!("could not prepare agent event insert: {error}"))?;
            for event in events {
                statement
                    .execute(params![
                        event.id,
                        event.provider,
                        event.session_id,
                        event.turn_id,
                        event.agent_id,
                        event.event_name,
                        event.activity_kind,
                        event.occurred_at_ms,
                        event.metadata_json,
                    ])
                    .map_err(|error| format!("could not save agent event {}: {error}", event.id))?;
            }
        }
        transaction
            .commit()
            .map_err(|error| format!("could not commit agent events: {error}"))
    }

    pub fn recent_agent_events(
        &self,
        since_ms: i64,
        limit: i64,
    ) -> Result<Vec<AgentEventRecord>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT id, provider, session_id, turn_id, agent_id, event_name,
                        activity_kind, occurred_at_ms, metadata_json
                 FROM agent_events
                 WHERE occurred_at_ms >= ?1
                 ORDER BY occurred_at_ms DESC, id DESC
                 LIMIT ?2",
            )
            .map_err(|error| format!("could not prepare agent event query: {error}"))?;
        let mut events = statement
            .query_map(params![since_ms, limit], |row| {
                Ok(AgentEventRecord {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    session_id: row.get(2)?,
                    turn_id: row.get(3)?,
                    agent_id: row.get(4)?,
                    event_name: row.get(5)?,
                    activity_kind: row.get(6)?,
                    occurred_at_ms: row.get(7)?,
                    metadata_json: row.get(8)?,
                })
            })
            .map_err(|error| format!("could not query agent events: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read agent events: {error}"))?;
        events.reverse();
        Ok(events)
    }

    fn set_setting(&self, key: &str, value: bool) -> Result<(), String> {
        self.set_text_setting(key, &value.to_string())
    }

    fn set_text_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "local database lock was poisoned".to_string())?;
        connection
            .execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|error| format!("could not update setting {key}: {error}"))?;
        Ok(())
    }

    fn setting_bool(connection: &Connection, key: &str, fallback: bool) -> Result<bool, String> {
        let value = connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| format!("could not read setting {key}: {error}"))?;
        Ok(value
            .and_then(|stored| stored.parse::<bool>().ok())
            .unwrap_or(fallback))
    }

    fn setting_i64(connection: &Connection, key: &str, fallback: i64) -> Result<i64, String> {
        let value = connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| format!("could not read setting {key}: {error}"))?;
        Ok(value
            .and_then(|stored| stored.parse::<i64>().ok())
            .unwrap_or(fallback))
    }

    fn setting_text(connection: &Connection, key: &str, fallback: &str) -> Result<String, String> {
        connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| format!("could not read setting {key}: {error}"))
            .map(|value| value.unwrap_or_else(|| fallback.to_string()))
    }

    fn excluded_activity_apps(connection: &Connection) -> Result<Vec<String>, String> {
        let mut statement = connection
            .prepare("SELECT app_name FROM activity_exclusions ORDER BY app_name COLLATE NOCASE")
            .map_err(|error| format!("could not prepare activity exclusions query: {error}"))?;
        let apps = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("could not query activity exclusions: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not read activity exclusions: {error}"))?;
        Ok(apps)
    }
}

fn normalize_excluded_apps(apps: &[String]) -> Result<Vec<String>, String> {
    if apps.len() > MAX_EXCLUDED_APPS {
        return Err(format!(
            "cannot exclude more than {MAX_EXCLUDED_APPS} applications"
        ));
    }
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for app in apps {
        let app = app.trim();
        if app.is_empty() {
            continue;
        }
        if app.chars().count() > 128 || app.chars().any(char::is_control) {
            return Err("excluded application names must be 1-128 visible characters".to_string());
        }
        if seen.insert(app.to_lowercase()) {
            normalized.push(app.to_string());
        }
    }
    normalized.sort_by_key(|name| name.to_lowercase());
    Ok(normalized)
}

fn retention_cutoff_ms(captured_at_ms: i64, retention_days: i64) -> i64 {
    captured_at_ms.saturating_sub(retention_days.saturating_mul(24 * 60 * 60 * 1_000))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn validate_activity_day_ranges(ranges: &[ActivityDayRange]) -> Result<(), String> {
    if ranges.len() != 7 {
        return Err("activity summary requires exactly seven day ranges".to_string());
    }
    let mut previous_end = None;
    let mut previous_day: Option<NaiveDate> = None;
    let mut days = HashSet::new();
    for range in ranges {
        let day = NaiveDate::parse_from_str(&range.day, "%Y-%m-%d")
            .map_err(|_| "activity summary day must use YYYY-MM-DD".to_string())?;
        if !days.insert(range.day.as_str()) {
            return Err("activity summary days must be unique".to_string());
        }
        let duration = range.end_ms.saturating_sub(range.start_ms);
        if duration <= 0 || duration > 26 * 60 * 60 * 1_000 {
            return Err("activity summary day range must be between 0 and 26 hours".to_string());
        }
        if previous_end.is_some_and(|end| range.start_ms != end) {
            return Err("activity summary day ranges must be contiguous".to_string());
        }
        if previous_day.is_some_and(|date| date.succ_opt() != Some(day)) {
            return Err("activity summary days must be consecutive".to_string());
        }
        previous_end = Some(range.end_ms);
        previous_day = Some(day);
    }
    Ok(())
}

fn rounded_minutes(milliseconds: i64) -> i64 {
    milliseconds.saturating_add(30_000) / 60_000
}

fn union_duration(mut ranges: Vec<(i64, i64)>) -> i64 {
    ranges.retain(|(start, end)| end > start);
    ranges.sort_unstable_by_key(|(start, end)| (*start, *end));
    let Some((mut current_start, mut current_end)) = ranges.first().copied() else {
        return 0;
    };
    let mut total = 0;
    for (start, end) in ranges.into_iter().skip(1) {
        if start <= current_end {
            current_end = current_end.max(end);
        } else {
            total += current_end - current_start;
            current_start = start;
            current_end = end;
        }
    }
    total + current_end - current_start
}

fn classify_app(app_name: &str) -> &'static str {
    let normalized = app_name.to_ascii_lowercase();
    if [
        "code", "codex", "idea", "pycharm", "devenv", "terminal", "wezterm",
    ]
    .iter()
    .any(|candidate| normalized.contains(candidate))
    {
        "focus"
    } else if ["feishu", "teams", "zoom", "webex"]
        .iter()
        .any(|candidate| normalized.contains(candidate))
    {
        "meeting"
    } else {
        "admin"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActivityDayRange, AgentEventRecord, CalendarConnection, Database, PlannerTask,
        ScheduleBlock, ScheduleDay,
    };
    use crate::activity::ForegroundSnapshot;
    use rusqlite::params;

    #[test]
    fn persists_schedule_and_settings() {
        let database = Database::in_memory();
        let schedule = vec![ScheduleBlock {
            id: "block-1".into(),
            title: "Deep work".into(),
            start_minute: 540,
            end_minute: 660,
            category: "focus".into(),
            source: "manual".into(),
            status: "planned".into(),
            locked: Some(true),
        }];

        database
            .replace_schedule("2026-07-21", &schedule)
            .expect("save schedule");
        database
            .replace_planner_tasks(&[PlannerTask {
                id: "task-1".into(),
                title: "Write iteration notes".into(),
                duration_minutes: 45,
                priority: "high".into(),
                preferred_period: "afternoon".into(),
                category: "admin".into(),
                notes: Some("Keep evidence concise".into()),
            }])
            .expect("save planner tasks");
        database.set_tracking(true).expect("enable tracking");
        database
            .set_activity_idle_threshold_minutes(10)
            .expect("save idle threshold");
        database
            .set_codex_observation(true)
            .expect("enable Codex observation");
        database
            .set_window_mode("compact")
            .expect("save window mode");
        database
            .set_calendar_timezone("Asia/Shanghai")
            .expect("save calendar timezone");
        database
            .set_morning_prompt_settings(true, 510)
            .expect("save morning prompt settings");
        database
            .snooze_morning_prompt(1_234_567)
            .expect("snooze morning prompt");
        database
            .dismiss_morning_prompt("2026-07-21")
            .expect("dismiss morning prompt");
        let state = database
            .load_state("2026-07-21", 0, i64::MAX)
            .expect("load state");

        assert_eq!(state.schedule_blocks.len(), 1);
        assert_eq!(state.schedule_blocks[0].title, "Deep work");
        assert_eq!(state.planner_tasks.len(), 1);
        assert_eq!(state.planner_tasks[0].duration_minutes, 45);
        assert_eq!(state.planner_tasks[0].preferred_period, "afternoon");
        assert!(state.tracking_enabled);
        assert_eq!(state.activity_idle_threshold_minutes, 10);
        assert!(state.codex_observation_enabled);
        assert_eq!(state.window_mode, "compact");
        assert_eq!(
            database
                .calendar_timezone()
                .expect("read calendar timezone"),
            Some("Asia/Shanghai".to_string())
        );
        assert!(state.morning_prompt.enabled);
        assert_eq!(state.morning_prompt.prompt_minute, 510);
        assert_eq!(state.morning_prompt.snoozed_until_ms, Some(1_234_567));
        assert_eq!(
            state.morning_prompt.dismissed_day.as_deref(),
            Some("2026-07-21")
        );
    }

    #[test]
    fn validates_activity_idle_threshold() {
        let database = Database::in_memory();
        assert!(database.set_activity_idle_threshold_minutes(0).is_err());
        assert!(database.set_activity_idle_threshold_minutes(61).is_err());
        database
            .set_activity_idle_threshold_minutes(1)
            .expect("minimum idle threshold");
        assert_eq!(
            database
                .activity_idle_threshold_minutes()
                .expect("read threshold"),
            1
        );
    }

    #[test]
    fn calendar_sync_replaces_only_its_own_source() {
        let database = Database::in_memory();
        let connection = CalendarConnection {
            id: "cal-test-source".into(),
            display_name: "Course calendar".into(),
            kind: "ics-subscription".into(),
            source_hint: "calendar.example".into(),
            enabled: true,
            refresh_minutes: 30,
            last_sync_at_ms: None,
            last_sync_status: "never".into(),
            last_error: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        database
            .insert_calendar_connection(&connection)
            .expect("save calendar connection");
        database
            .replace_schedule(
                "2026-07-22",
                &[ScheduleBlock {
                    id: "manual".into(),
                    title: "Keep me".into(),
                    start_minute: 480,
                    end_minute: 540,
                    category: "focus".into(),
                    source: "manual".into(),
                    status: "planned".into(),
                    locked: Some(false),
                }],
            )
            .expect("save manual schedule");

        for (id, title) in [("event-v1", "First"), ("event-v2", "Updated")] {
            database
                .replace_calendar_schedule(
                    "2026-07-22",
                    &connection.id,
                    &[ScheduleBlock {
                        id: id.into(),
                        title: title.into(),
                        start_minute: 600,
                        end_minute: 660,
                        category: "meeting".into(),
                        source: format!("calendar:{}", connection.id),
                        status: "planned".into(),
                        locked: Some(true),
                    }],
                )
                .expect("replace connected calendar schedule");
        }

        let state = database
            .load_state("2026-07-22", 0, i64::MAX)
            .expect("load connected calendar state");
        assert_eq!(state.calendar_connections.len(), 1);
        assert_eq!(state.schedule_blocks.len(), 2);
        assert!(state
            .schedule_blocks
            .iter()
            .any(|block| block.id == "manual"));
        assert!(state
            .schedule_blocks
            .iter()
            .any(|block| block.id == "event-v2" && block.locked == Some(true)));
        assert!(!state
            .schedule_blocks
            .iter()
            .any(|block| block.id == "event-v1"));
    }

    #[test]
    fn calendar_week_sync_rolls_back_every_day_when_one_insert_fails() {
        let database = Database::in_memory();
        for day in ["2026-07-20", "2026-07-21"] {
            database
                .replace_schedule(
                    day,
                    &[ScheduleBlock {
                        id: format!("old-{day}"),
                        title: "Existing calendar event".into(),
                        start_minute: 540,
                        end_minute: 600,
                        category: "meeting".into(),
                        source: "calendar:cal-week-test".into(),
                        status: "planned".into(),
                        locked: Some(true),
                    }],
                )
                .expect("save old calendar event");
        }

        let result = database.replace_calendar_schedule_days(
            "cal-week-test",
            &[
                ScheduleDay {
                    day: "2026-07-20".into(),
                    blocks: vec![ScheduleBlock {
                        id: "new-valid".into(),
                        title: "New valid event".into(),
                        start_minute: 600,
                        end_minute: 660,
                        category: "meeting".into(),
                        source: "calendar:cal-week-test".into(),
                        status: "planned".into(),
                        locked: Some(true),
                    }],
                },
                ScheduleDay {
                    day: "2026-07-21".into(),
                    blocks: vec![ScheduleBlock {
                        id: "new-invalid".into(),
                        title: "Invalid event".into(),
                        start_minute: 1_400,
                        end_minute: 1_500,
                        category: "meeting".into(),
                        source: "calendar:cal-week-test".into(),
                        status: "planned".into(),
                        locked: Some(true),
                    }],
                },
            ],
        );

        assert!(result.is_err());
        for day in ["2026-07-20", "2026-07-21"] {
            let blocks = database
                .schedule_for_day(day)
                .expect("load rolled back day");
            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].id, format!("old-{day}"));
        }
    }

    #[test]
    fn applying_morning_plan_marks_today_and_clears_prompt_actions() {
        let database = Database::in_memory();
        database
            .snooze_morning_prompt(1_234_567)
            .expect("snooze morning prompt");
        database
            .dismiss_morning_prompt("2026-07-21")
            .expect("dismiss morning prompt");

        database
            .apply_morning_plan("2026-07-22", &[], &[])
            .expect("apply empty morning plan");
        let state = database
            .load_state("2026-07-22", 0, i64::MAX)
            .expect("load morning prompt state");

        assert_eq!(
            state.morning_prompt.last_planned_day.as_deref(),
            Some("2026-07-22")
        );
        assert_eq!(state.morning_prompt.snoozed_until_ms, None);
        assert_eq!(state.morning_prompt.dismissed_day, None);
    }

    #[test]
    fn morning_plan_rolls_back_schedule_when_task_save_fails() {
        let database = Database::in_memory();
        let original = ScheduleBlock {
            id: "original".into(),
            title: "Original plan".into(),
            start_minute: 540,
            end_minute: 600,
            category: "focus".into(),
            source: "manual".into(),
            status: "planned".into(),
            locked: Some(true),
        };
        database
            .replace_schedule("2026-07-22", std::slice::from_ref(&original))
            .expect("save original schedule");

        let invalid_task = PlannerTask {
            id: "invalid".into(),
            title: "Too short".into(),
            duration_minutes: 5,
            priority: "normal".into(),
            preferred_period: "any".into(),
            category: "focus".into(),
            notes: None,
        };
        let result = database.apply_morning_plan(
            "2026-07-22",
            &[ScheduleBlock {
                id: "replacement".into(),
                ..original
            }],
            &[invalid_task],
        );

        assert!(result.is_err());
        let state = database
            .load_state("2026-07-22", 0, i64::MAX)
            .expect("load rolled back state");
        assert_eq!(state.schedule_blocks.len(), 1);
        assert_eq!(state.schedule_blocks[0].id, "original");
        assert!(state.planner_tasks.is_empty());
        assert_eq!(state.morning_prompt.last_planned_day, None);
    }

    #[test]
    fn merges_contiguous_activity_samples() {
        let database = Database::in_memory();
        let first = ForegroundSnapshot {
            app_name: "Code".into(),
            sanitized_window_title: Some("agent-bar".into()),
            captured_at_ms: 1_000,
        };
        let second = ForegroundSnapshot {
            captured_at_ms: 6_000,
            ..first.clone()
        };

        database
            .record_snapshot(&first)
            .expect("first sample")
            .expect("first record");
        let merged = database
            .record_snapshot(&second)
            .expect("second sample")
            .expect("merged record");
        let state = database
            .load_state("unused", 0, 10_000)
            .expect("load state");

        assert_eq!(state.activity_records.len(), 1);
        assert_eq!(merged.started_at_ms, 1_000);
        assert_eq!(merged.ended_at_ms, 6_000);
        assert_eq!(merged.category, "focus");
    }

    #[test]
    fn summarizes_week_activity_with_day_clipping_and_planned_union() {
        let database = Database::in_memory();
        database
            .replace_schedule(
                "2026-07-20",
                &[
                    ScheduleBlock {
                        id: "plan-1".into(),
                        title: "Focus".into(),
                        start_minute: 60,
                        end_minute: 120,
                        category: "focus".into(),
                        source: "manual".into(),
                        status: "planned".into(),
                        locked: None,
                    },
                    ScheduleBlock {
                        id: "plan-2".into(),
                        title: "Meeting".into(),
                        start_minute: 90,
                        end_minute: 150,
                        category: "meeting".into(),
                        source: "calendar".into(),
                        status: "planned".into(),
                        locked: Some(true),
                    },
                ],
            )
            .expect("save overlapping plan");
        {
            let connection = database.connection.lock().expect("database lock");
            for (id, start, end, app, category) in [
                (
                    "before-midnight",
                    -30 * 60_000,
                    30 * 60_000,
                    "Code.exe",
                    "focus",
                ),
                (
                    "cross-midnight",
                    23 * 60 * 60_000 + 30 * 60_000,
                    24 * 60 * 60_000 + 30 * 60_000,
                    "Feishu.exe",
                    "meeting",
                ),
                (
                    "day-focus",
                    10 * 60 * 60_000,
                    10 * 60 * 60_000 + 45 * 60_000,
                    "Code.exe",
                    "focus",
                ),
            ] {
                connection
                    .execute(
                        "INSERT INTO activity_records(
                           id, started_at_ms, ended_at_ms, app_name,
                           sanitized_window_title, category, source
                         ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, 'test')",
                        params![id, start, end, app, category],
                    )
                    .expect("insert activity");
            }
        }
        let ranges = (0..7)
            .map(|index| ActivityDayRange {
                day: format!("2026-07-{}", 20 + index),
                start_ms: index * 24 * 60 * 60_000,
                end_ms: (index + 1) * 24 * 60 * 60_000,
            })
            .collect::<Vec<_>>();

        let summary = database
            .load_activity_week_summary(&ranges)
            .expect("load week summary");

        assert_eq!(summary.len(), 7);
        assert_eq!(summary[0].planned_minutes, 90);
        assert_eq!(summary[0].actual_minutes, 105);
        assert_eq!(summary[0].categories.focus, 75);
        assert_eq!(summary[0].categories.meeting, 30);
        assert_eq!(summary[0].top_apps[0].app_name, "Code.exe");
        assert_eq!(summary[0].top_apps[0].minutes, 75);
        assert_eq!(summary[1].actual_minutes, 30);
        assert_eq!(summary[1].categories.meeting, 30);
    }

    #[test]
    fn excludes_apps_case_insensitively_and_deletes_existing_history() {
        let database = Database::in_memory();
        let snapshot = ForegroundSnapshot {
            app_name: "WeChat.exe".into(),
            sanitized_window_title: Some("redacted".into()),
            captured_at_ms: super::now_ms(),
        };
        database
            .record_snapshot(&snapshot)
            .expect("record existing activity")
            .expect("existing record");

        let update = database
            .set_activity_privacy(&["  wechat.EXE  ".into(), "WECHAT.exe".into()], 30)
            .expect("save exclusions");
        let ignored = database
            .record_snapshot(&ForegroundSnapshot {
                captured_at_ms: snapshot.captured_at_ms + 5_000,
                ..snapshot
            })
            .expect("ignore excluded app");
        let state = database
            .load_state("unused", 0, i64::MAX)
            .expect("load privacy state");

        assert!(ignored.is_none());
        assert_eq!(update.deleted_records, 1);
        assert_eq!(state.activity_records.len(), 0);
        assert_eq!(state.excluded_activity_apps, vec!["wechat.EXE"]);
        assert_eq!(state.activity_retention_days, 30);
    }

    #[test]
    fn purges_expired_activity_and_clears_remaining_history() {
        let database = Database::in_memory();
        let now = super::now_ms();
        let old = ForegroundSnapshot {
            app_name: "Code.exe".into(),
            sanitized_window_title: None,
            captured_at_ms: now - 31 * 24 * 60 * 60 * 1_000,
        };
        let recent = ForegroundSnapshot {
            app_name: "Feishu.exe".into(),
            sanitized_window_title: None,
            captured_at_ms: now - 10_000,
        };
        database
            .record_snapshot(&old)
            .expect("record old")
            .expect("old record");
        database
            .record_snapshot(&recent)
            .expect("record recent")
            .expect("recent record");

        assert_eq!(database.purge_expired_activity(now).expect("purge"), 1);
        assert_eq!(database.clear_activity_records().expect("clear"), 1);
        let state = database
            .load_state("unused", 0, i64::MAX)
            .expect("load cleared state");
        assert!(state.activity_records.is_empty());
    }

    #[test]
    fn deduplicates_and_reads_agent_events_in_time_order() {
        let database = Database::in_memory();
        let first = AgentEventRecord {
            id: "event-1".into(),
            provider: "codex".into(),
            session_id: "session-1".into(),
            turn_id: Some("turn-1".into()),
            agent_id: None,
            event_name: "UserPromptSubmit".into(),
            activity_kind: "working".into(),
            occurred_at_ms: 100,
            metadata_json: "{}".into(),
        };
        let second = AgentEventRecord {
            id: "event-2".into(),
            event_name: "Stop".into(),
            activity_kind: "idle".into(),
            occurred_at_ms: 200,
            ..first.clone()
        };

        database
            .save_agent_events(&[first.clone(), first, second])
            .expect("save events");
        let events = database.recent_agent_events(0, 10).expect("load events");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_name, "UserPromptSubmit");
        assert_eq!(events[1].event_name, "Stop");
    }
}
