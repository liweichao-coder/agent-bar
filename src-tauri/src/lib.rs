mod activity;
mod calendar_connections;
mod calendar_import;
mod calendar_scheduler;
mod codex;
mod codex_managed;
mod database;
mod screenshot_import;
mod secret_store;

use activity::{
    activity_capture_state, capture_foreground_window, paused_capture_state, ActivityCaptureState,
    ForegroundSnapshot,
};
use calendar_connections::CalendarSyncBatch;
use calendar_import::CalendarImportPreview;
use calendar_scheduler::CalendarSyncScheduler;
use codex::{CodexMonitor, CodexSnapshot};
use codex_managed::{ManagedCodex, ManagedCodexSnapshot};
use database::{
    ActivityDayRange, ActivityDaySummary, ActivityPrivacyUpdate, ActivityRecord,
    CalendarConnection, Database, LocalState, PlannerTask, ScheduleBlock, ScheduleDay,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewWindow};

const TRAY_EXPAND_ID: &str = "tray-expand";
const TRAY_COMPACT_ID: &str = "tray-compact";
const TRAY_RESTORE_ID: &str = "tray-restore";
const TRAY_QUIT_ID: &str = "tray-quit";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowModeSnapshot {
    mode: &'static str,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
    always_on_top: bool,
}

#[derive(Debug, Clone, Copy)]
struct MonitorGeometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
fn load_local_state(
    database: State<'_, Arc<Database>>,
    day: String,
    day_start_ms: i64,
    day_end_ms: i64,
) -> Result<LocalState, String> {
    database.load_state(&day, day_start_ms, day_end_ms)
}

#[tauri::command]
fn replace_schedule_blocks(
    database: State<'_, Arc<Database>>,
    day: String,
    blocks: Vec<ScheduleBlock>,
) -> Result<(), String> {
    database.replace_schedule(&day, &blocks)
}

#[tauri::command]
fn replace_planner_tasks(
    database: State<'_, Arc<Database>>,
    tasks: Vec<PlannerTask>,
) -> Result<(), String> {
    database.replace_planner_tasks(&tasks)
}

#[tauri::command]
fn apply_morning_plan(
    database: State<'_, Arc<Database>>,
    day: String,
    blocks: Vec<ScheduleBlock>,
    tasks: Vec<PlannerTask>,
) -> Result<(), String> {
    database.apply_morning_plan(&day, &blocks, &tasks)
}

#[tauri::command]
fn set_morning_prompt_settings(
    database: State<'_, Arc<Database>>,
    enabled: bool,
    prompt_minute: i64,
) -> Result<(), String> {
    database.set_morning_prompt_settings(enabled, prompt_minute)
}

#[tauri::command]
fn snooze_morning_prompt(database: State<'_, Arc<Database>>, until_ms: i64) -> Result<(), String> {
    database.snooze_morning_prompt(until_ms)
}

#[tauri::command]
fn dismiss_morning_prompt(database: State<'_, Arc<Database>>, day: String) -> Result<(), String> {
    database.dismiss_morning_prompt(&day)
}

#[tauri::command]
fn preview_calendar_import(
    ics_text: String,
    day: String,
    viewer_timezone: String,
) -> Result<CalendarImportPreview, String> {
    calendar_import::preview_ics(&ics_text, &day, &viewer_timezone)
}

#[tauri::command]
fn create_calendar_connection(
    database: State<'_, Arc<Database>>,
    display_name: String,
    kind: String,
    source: String,
    refresh_minutes: i64,
) -> Result<CalendarConnection, String> {
    calendar_connections::create_connection(
        &database,
        &display_name,
        &kind,
        &source,
        refresh_minutes,
    )
}

#[tauri::command]
async fn set_calendar_connection_enabled(
    database: State<'_, Arc<Database>>,
    scheduler: State<'_, CalendarSyncScheduler>,
    id: String,
    enabled: bool,
) -> Result<Vec<CalendarConnection>, String> {
    let database = database.inner().clone();
    let sync_gate = scheduler.gate();
    let connections = tauri::async_runtime::spawn_blocking(move || {
        let _guard = calendar_scheduler::lock_sync_gate(&sync_gate)?;
        calendar_connections::set_enabled(&database, &id, enabled)?;
        database.calendar_connections()
    })
    .await
    .map_err(|error| format!("calendar connection worker failed: {error}"))??;
    if enabled {
        scheduler.wake();
    }
    Ok(connections)
}

#[tauri::command]
async fn delete_calendar_connection(
    database: State<'_, Arc<Database>>,
    scheduler: State<'_, CalendarSyncScheduler>,
    id: String,
    day: String,
    days: Vec<String>,
) -> Result<CalendarSyncBatch, String> {
    let database = database.inner().clone();
    let sync_gate = scheduler.gate();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = calendar_scheduler::lock_sync_gate(&sync_gate)?;
        calendar_connections::delete_connection(&database, &id)?;
        calendar_connections::snapshot(&database, &day, &days)
    })
    .await
    .map_err(|error| format!("calendar connection worker failed: {error}"))?
}

#[tauri::command]
fn load_schedule_days(
    database: State<'_, Arc<Database>>,
    day: String,
    days: Vec<String>,
) -> Result<Vec<ScheduleDay>, String> {
    calendar_connections::validate_sync_days(&day, &days)?;
    database.schedules_for_days(&days)
}

#[tauri::command]
fn configure_calendar_sync(
    database: State<'_, Arc<Database>>,
    scheduler: State<'_, CalendarSyncScheduler>,
    viewer_timezone: String,
) -> Result<(), String> {
    calendar_scheduler::validate_timezone(&viewer_timezone)?;
    database.set_calendar_timezone(&viewer_timezone)?;
    scheduler.wake();
    Ok(())
}

#[tauri::command]
async fn sync_calendar_connections(
    database: State<'_, Arc<Database>>,
    scheduler: State<'_, CalendarSyncScheduler>,
    day: String,
    days: Vec<String>,
    viewer_timezone: String,
    connection_id: Option<String>,
    force: bool,
) -> Result<CalendarSyncBatch, String> {
    let database = database.inner().clone();
    let sync_gate = scheduler.gate();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = calendar_scheduler::lock_sync_gate(&sync_gate)?;
        calendar_connections::sync_connections_range(
            &database,
            &day,
            &days,
            &viewer_timezone,
            connection_id.as_deref(),
            force,
        )
    })
    .await
    .map_err(|error| format!("calendar sync worker failed: {error}"))?
}

#[tauri::command]
fn set_tracking_enabled(database: State<'_, Arc<Database>>, enabled: bool) -> Result<(), String> {
    database.set_tracking(enabled)
}

#[tauri::command]
fn set_activity_idle_threshold(
    database: State<'_, Arc<Database>>,
    minutes: i64,
) -> Result<(), String> {
    database.set_activity_idle_threshold_minutes(minutes)
}

#[tauri::command]
fn load_activity_capture_state(
    database: State<'_, Arc<Database>>,
) -> Result<ActivityCaptureState, String> {
    let threshold = database.activity_idle_threshold_minutes()?;
    if !database.tracking_enabled()? {
        return Ok(paused_capture_state(threshold));
    }
    activity_capture_state(threshold)
}

#[tauri::command]
fn set_capture_window_titles(
    database: State<'_, Arc<Database>>,
    enabled: bool,
) -> Result<(), String> {
    database.set_capture_window_titles(enabled)
}

#[tauri::command]
fn set_codex_observation_enabled(
    database: State<'_, Arc<Database>>,
    enabled: bool,
) -> Result<(), String> {
    database.set_codex_observation(enabled)
}

#[tauri::command]
fn set_activity_privacy(
    database: State<'_, Arc<Database>>,
    excluded_apps: Vec<String>,
    retention_days: i64,
) -> Result<ActivityPrivacyUpdate, String> {
    database.set_activity_privacy(&excluded_apps, retention_days)
}

#[tauri::command]
fn clear_activity_records(database: State<'_, Arc<Database>>) -> Result<usize, String> {
    database.clear_activity_records()
}

#[tauri::command]
fn load_activity_week_summary(
    database: State<'_, Arc<Database>>,
    ranges: Vec<ActivityDayRange>,
) -> Result<Vec<ActivityDaySummary>, String> {
    database.load_activity_week_summary(&ranges)
}

#[tauri::command]
fn set_window_mode(
    database: State<'_, Arc<Database>>,
    window: WebviewWindow,
    mode: String,
) -> Result<WindowModeSnapshot, String> {
    let compact = match mode.as_str() {
        "compact" => true,
        "expanded" => false,
        _ => return Err("window mode must be compact or expanded".to_string()),
    };
    let snapshot = apply_window_mode(&window, compact)?;
    database.set_window_mode(snapshot.mode)?;
    Ok(snapshot)
}

fn apply_window_mode(window: &WebviewWindow, compact: bool) -> Result<WindowModeSnapshot, String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| format!("could not read current monitor: {error}"))?;
    let monitor = monitor
        .map(|monitor| {
            let scale = monitor.scale_factor();
            MonitorGeometry {
                x: monitor.position().x as f64 / scale,
                y: monitor.position().y as f64 / scale,
                width: monitor.size().width as f64 / scale,
                height: monitor.size().height as f64 / scale,
            }
        })
        .unwrap_or(MonitorGeometry {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        });
    let geometry = window_geometry(monitor, compact);

    window
        .set_ignore_cursor_events(false)
        .map_err(|error| format!("could not restore window cursor events: {error}"))?;
    window
        .set_always_on_top(compact)
        .map_err(|error| format!("could not update always-on-top: {error}"))?;
    window
        .set_decorations(!compact)
        .map_err(|error| format!("could not update window decorations: {error}"))?;
    window
        .set_shadow(!compact)
        .map_err(|error| format!("could not update window shadow: {error}"))?;
    window
        .set_resizable(!compact)
        .map_err(|error| format!("could not update window resizing: {error}"))?;
    window
        .set_min_size(if compact {
            None
        } else {
            Some(LogicalSize::new(900.0, 620.0))
        })
        .map_err(|error| format!("could not update minimum window size: {error}"))?;
    window
        .set_size(LogicalSize::new(geometry.width, geometry.height))
        .map_err(|error| format!("could not resize Agent Bar: {error}"))?;
    window
        .set_position(LogicalPosition::new(geometry.x, geometry.y))
        .map_err(|error| format!("could not position Agent Bar: {error}"))?;
    if compact {
        let outer = window
            .outer_position()
            .map_err(|error| format!("could not read Agent Bar outer position: {error}"))?;
        let inner = window
            .inner_position()
            .map_err(|error| format!("could not read Agent Bar client position: {error}"))?;
        let scale = window
            .scale_factor()
            .map_err(|error| format!("could not read Agent Bar scale factor: {error}"))?;
        let frame_left = (inner.x - outer.x) as f64 / scale;
        let frame_top = (inner.y - outer.y) as f64 / scale;
        window
            .set_position(LogicalPosition::new(
                geometry.x - frame_left,
                geometry.y - frame_top,
            ))
            .map_err(|error| format!("could not align Agent Bar client area: {error}"))?;
    }

    Ok(WindowModeSnapshot {
        mode: if compact { "compact" } else { "expanded" },
        width: geometry.width,
        height: geometry.height,
        x: geometry.x,
        y: geometry.y,
        always_on_top: compact,
    })
}

#[tauri::command]
fn set_window_click_through(window: WebviewWindow, enabled: bool) -> Result<(), String> {
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|error| format!("could not update click-through mode: {error}"))
}

fn switch_window_from_app(
    app: &AppHandle,
    compact: bool,
    persist: bool,
) -> Result<WindowModeSnapshot, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;
    window
        .show()
        .map_err(|error| format!("could not show Agent Bar: {error}"))?;
    let snapshot = apply_window_mode(&window, compact)?;
    if persist {
        app.state::<Arc<Database>>()
            .set_window_mode(snapshot.mode)?;
    }
    if !compact {
        window
            .unminimize()
            .map_err(|error| format!("could not restore Agent Bar: {error}"))?;
        window
            .set_focus()
            .map_err(|error| format!("could not focus Agent Bar: {error}"))?;
    }
    app.emit_to("main", "window-mode-changed", snapshot.clone())
        .map_err(|error| format!("could not synchronize window mode: {error}"))?;
    Ok(snapshot)
}

fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let expand = MenuItem::with_id(app, TRAY_EXPAND_ID, "展开工作台", true, None::<&str>)?;
    let compact = MenuItem::with_id(app, TRAY_COMPACT_ID, "显示顶部状态条", true, None::<&str>)?;
    let restore = MenuItem::with_id(app, TRAY_RESTORE_ID, "恢复交互并展开", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出 Agent Bar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&expand, &compact, &restore, &separator, &quit])?;
    let mut builder = TrayIconBuilder::with_id("agent-bar-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Agent Bar")
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_EXPAND_ID | TRAY_RESTORE_ID => {
                let _ = switch_window_from_app(app, false, true);
            }
            TRAY_COMPACT_ID => {
                let _ = switch_window_from_app(app, true, true);
            }
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    let tray = builder.build(app)?;
    app.manage(tray);
    Ok(())
}

#[tauri::command]
fn load_codex_snapshot(monitor: State<'_, CodexMonitor>) -> Result<CodexSnapshot, String> {
    monitor.snapshot()
}

#[tauri::command]
fn refresh_codex_snapshot(
    database: State<'_, Arc<Database>>,
    monitor: State<'_, CodexMonitor>,
) -> Result<CodexSnapshot, String> {
    monitor.refresh(database.inner().as_ref())
}

#[tauri::command]
fn capture_foreground_activity(
    database: State<'_, Arc<Database>>,
) -> Result<Option<ActivityRecord>, String> {
    if !database.tracking_enabled()? {
        return Err("activity tracking is paused".to_string());
    }
    let threshold = database.activity_idle_threshold_minutes()?;
    if !activity_capture_state(threshold)?.capture_allowed {
        return Ok(None);
    }
    let include_title = database.capture_window_titles()?;
    let snapshot: ForegroundSnapshot = capture_foreground_window(include_title)?;
    database.record_snapshot(&snapshot)
}

#[tauri::command]
fn load_managed_codex_snapshot(
    managed: State<'_, ManagedCodex>,
) -> Result<ManagedCodexSnapshot, String> {
    managed.snapshot()
}

#[tauri::command]
fn start_managed_codex_run(
    managed: State<'_, ManagedCodex>,
    prompt: String,
    cwd: Option<String>,
) -> Result<ManagedCodexSnapshot, String> {
    managed.start_run(prompt, cwd)
}

#[tauri::command]
fn start_screenshot_import(
    app: AppHandle,
    managed: State<'_, ManagedCodex>,
    file_name: String,
    mime_type: String,
    base64_data: String,
) -> Result<ManagedCodexSnapshot, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("could not resolve screenshot cache directory: {error}"))?
        .join("screenshot-import");
    managed.start_screenshot_import(file_name, mime_type, base64_data, &cache_dir)
}

#[tauri::command]
fn dismiss_screenshot_import(
    managed: State<'_, ManagedCodex>,
) -> Result<ManagedCodexSnapshot, String> {
    managed.dismiss_screenshot_import()
}

#[tauri::command]
fn cancel_screenshot_import(
    managed: State<'_, ManagedCodex>,
) -> Result<ManagedCodexSnapshot, String> {
    managed.cancel_screenshot_import()
}

#[tauri::command]
fn interrupt_managed_codex_run(
    managed: State<'_, ManagedCodex>,
    thread_id: String,
) -> Result<ManagedCodexSnapshot, String> {
    managed.interrupt(&thread_id)
}

#[tauri::command]
fn resolve_managed_codex_approval(
    managed: State<'_, ManagedCodex>,
    approval_id: String,
    approved: bool,
) -> Result<ManagedCodexSnapshot, String> {
    managed.resolve_approval(&approval_id, approved)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let database_path = app
                .path()
                .app_data_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?
                .join("agent-bar.sqlite3");
            let database = Arc::new(Database::open(&database_path).map_err(std::io::Error::other)?);
            database
                .purge_expired_activity(now_ms())
                .map_err(std::io::Error::other)?;
            let stored_window_mode = database.window_mode().map_err(std::io::Error::other)?;
            app.manage(database.clone());
            let calendar_scheduler =
                CalendarSyncScheduler::start(app.handle().clone(), database.clone())
                    .map_err(std::io::Error::other)?;
            calendar_scheduler.wake();
            app.manage(calendar_scheduler);
            let codex_monitor =
                CodexMonitor::start(database.clone()).map_err(std::io::Error::other)?;
            app.manage(codex_monitor);
            app.manage(ManagedCodex::new());
            build_tray(app).map_err(|error| std::io::Error::other(error.to_string()))?;
            let start_compact = std::env::var("AGENT_BAR_START_COMPACT").as_deref() == Ok("1")
                || stored_window_mode == "compact";
            if start_compact {
                let window = app
                    .get_webview_window("main")
                    .ok_or_else(|| std::io::Error::other("main window is unavailable"))?;
                apply_window_mode(&window, true).map_err(std::io::Error::other)?;
            }
            if std::env::var("AGENT_BAR_TEST_CLICK_THROUGH").as_deref() == Ok("1") {
                let window = app
                    .get_webview_window("main")
                    .ok_or_else(|| std::io::Error::other("main window is unavailable"))?;
                window
                    .set_ignore_cursor_events(true)
                    .map_err(std::io::Error::other)?;
            }
            if let Some(delay_ms) = std::env::var("AGENT_BAR_TEST_AUTO_RESTORE_MS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    let restore_handle = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        let _ = switch_window_from_app(&restore_handle, false, false);
                    });
                });
            }
            std::thread::Builder::new()
                .name("activity-tracker".to_string())
                .spawn(move || {
                    let mut last_purge_at_ms = now_ms();
                    loop {
                        if database.tracking_enabled().unwrap_or(false) {
                            let threshold = database
                                .activity_idle_threshold_minutes()
                                .unwrap_or(activity::DEFAULT_IDLE_THRESHOLD_MINUTES);
                            if activity_capture_state(threshold)
                                .map(|state| state.capture_allowed)
                                .unwrap_or(false)
                            {
                                let include_title =
                                    database.capture_window_titles().unwrap_or(false);
                                if let Ok(snapshot) = capture_foreground_window(include_title) {
                                    let _ = database.record_snapshot(&snapshot);
                                }
                            }
                        }
                        let current_ms = now_ms();
                        if current_ms.saturating_sub(last_purge_at_ms) >= 60 * 60 * 1_000 {
                            let _ = database.purge_expired_activity(current_ms);
                            last_purge_at_ms = current_ms;
                        }
                        std::thread::sleep(Duration::from_secs(5));
                    }
                })
                .map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_local_state,
            replace_schedule_blocks,
            replace_planner_tasks,
            apply_morning_plan,
            set_morning_prompt_settings,
            snooze_morning_prompt,
            dismiss_morning_prompt,
            preview_calendar_import,
            create_calendar_connection,
            set_calendar_connection_enabled,
            delete_calendar_connection,
            load_schedule_days,
            configure_calendar_sync,
            sync_calendar_connections,
            set_tracking_enabled,
            set_activity_idle_threshold,
            load_activity_capture_state,
            set_capture_window_titles,
            set_codex_observation_enabled,
            set_activity_privacy,
            clear_activity_records,
            load_activity_week_summary,
            set_window_mode,
            set_window_click_through,
            load_codex_snapshot,
            refresh_codex_snapshot,
            capture_foreground_activity,
            load_managed_codex_snapshot,
            start_managed_codex_run,
            start_screenshot_import,
            dismiss_screenshot_import,
            cancel_screenshot_import,
            interrupt_managed_codex_run,
            resolve_managed_codex_approval,
        ])
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Agent Bar");
    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn window_geometry(monitor: MonitorGeometry, compact: bool) -> MonitorGeometry {
    if compact {
        return MonitorGeometry {
            height: 68.0,
            ..monitor
        };
    }
    let width = (monitor.width * 0.9)
        .clamp(720.0, 1440.0)
        .min(monitor.width);
    let height = (monitor.height * 0.88)
        .clamp(560.0, 900.0)
        .min(monitor.height);
    MonitorGeometry {
        x: monitor.x + (monitor.width - width) / 2.0,
        y: monitor.y + (monitor.height - height) / 2.0,
        width,
        height,
    }
}

#[cfg(test)]
mod window_tests {
    use super::{window_geometry, MonitorGeometry};

    #[test]
    fn compact_bar_uses_monitor_width_and_top_edge() {
        let monitor = MonitorGeometry {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let bar = window_geometry(monitor, true);
        assert_eq!(bar.x, -1920.0);
        assert_eq!(bar.y, 0.0);
        assert_eq!(bar.width, 1920.0);
        assert_eq!(bar.height, 68.0);
    }

    #[test]
    fn expanded_window_is_centered_inside_monitor() {
        let monitor = MonitorGeometry {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let expanded = window_geometry(monitor, false);
        assert_eq!(expanded.width, 1440.0);
        assert_eq!(expanded.height, 900.0);
        assert_eq!(expanded.x, 240.0);
        assert_eq!(expanded.y, 90.0);
    }
}
