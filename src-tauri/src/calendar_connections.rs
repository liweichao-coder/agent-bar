use crate::calendar_import;
use crate::database::{CalendarConnection, Database, ScheduleBlock, ScheduleDay};
use crate::secret_store;
use chrono::NaiveDate;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::Url;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{Read, Take};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOCAL_FILE_KIND: &str = "local-file";
const SUBSCRIPTION_KIND: &str = "ics-subscription";
const MAX_ICS_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DISPLAY_NAME_CHARS: usize = 80;
const MAX_SOURCE_CHARS: usize = 2_400;
const MIN_REFRESH_MINUTES: i64 = 15;
const MAX_REFRESH_MINUTES: i64 = 1_440;
const REQUEST_TIMEOUT_SECONDS: u64 = 10;
const MAX_SYNC_DAYS: usize = 14;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSyncBatch {
    pub connections: Vec<CalendarConnection>,
    pub schedule_blocks: Vec<ScheduleBlock>,
    pub schedule_days: Vec<ScheduleDay>,
    pub synced_count: usize,
    pub failed_count: usize,
    pub warnings: Vec<String>,
}

pub fn create_connection(
    database: &Database,
    display_name: &str,
    kind: &str,
    source: &str,
    refresh_minutes: i64,
) -> Result<CalendarConnection, String> {
    let display_name = normalize_display_name(display_name)?;
    let refresh_minutes = validate_refresh_minutes(refresh_minutes)?;
    let (normalized_source, source_hint) = normalize_source(kind, source)?;
    let now = now_ms();
    let id = connection_id(kind, &normalized_source, now);
    let connection = CalendarConnection {
        id: id.clone(),
        display_name,
        kind: kind.to_string(),
        source_hint,
        enabled: true,
        refresh_minutes,
        last_sync_at_ms: None,
        last_sync_status: "never".to_string(),
        last_error: None,
        created_at_ms: now,
        updated_at_ms: now,
    };

    secret_store::write(&id, &normalized_source)?;
    if let Err(error) = database.insert_calendar_connection(&connection) {
        let _ = secret_store::delete(&id);
        return Err(error);
    }
    Ok(connection)
}

pub fn set_enabled(database: &Database, id: &str, enabled: bool) -> Result<(), String> {
    validate_connection_id(id)?;
    database.set_calendar_connection_enabled(id, enabled, now_ms())
}

pub fn delete_connection(database: &Database, id: &str) -> Result<(), String> {
    validate_connection_id(id)?;
    if database.calendar_connection(id)?.is_none() {
        return Err("calendar connection was not found".to_string());
    }
    let secret = secret_store::read(id).ok();
    secret_store::delete(id)?;
    if let Err(error) = database.delete_calendar_connection(id) {
        if let Some(secret) = secret {
            let _ = secret_store::write(id, &secret);
        }
        return Err(error);
    }
    Ok(())
}

pub fn snapshot(
    database: &Database,
    day: &str,
    days: &[String],
) -> Result<CalendarSyncBatch, String> {
    validate_sync_days(day, days)?;
    Ok(CalendarSyncBatch {
        connections: database.calendar_connections()?,
        schedule_blocks: database.schedule_for_day(day)?,
        schedule_days: database.schedules_for_days(days)?,
        synced_count: 0,
        failed_count: 0,
        warnings: Vec::new(),
    })
}

pub fn sync_connections_range(
    database: &Database,
    day: &str,
    days: &[String],
    viewer_timezone: &str,
    connection_id: Option<&str>,
    force: bool,
) -> Result<CalendarSyncBatch, String> {
    validate_sync_days(day, days)?;
    if let Some(id) = connection_id {
        validate_connection_id(id)?;
    }
    let now = now_ms();
    let mut warnings = Vec::new();
    let mut synced_count = 0;
    let mut failed_count = 0;
    let connections = database.calendar_connections()?;

    for connection in connections.iter().filter(|connection| {
        connection.enabled
            && connection_id.is_none_or(|id| connection.id == id)
            && (force || connection_due(connection, now))
    }) {
        match sync_connection_days(database, connection, days, viewer_timezone) {
            Ok(connection_warnings) => {
                synced_count += 1;
                warnings.extend(
                    connection_warnings
                        .into_iter()
                        .map(|warning| format!("{}：{warning}", connection.display_name)),
                );
                database.update_calendar_sync_state(&connection.id, now_ms(), "success", None)?;
            }
            Err(error) => {
                failed_count += 1;
                let safe_error = truncate_error(&error);
                warnings.push(format!("{}：{safe_error}", connection.display_name));
                database.update_calendar_sync_state(
                    &connection.id,
                    now_ms(),
                    "error",
                    Some(&safe_error),
                )?;
            }
        }
    }

    Ok(CalendarSyncBatch {
        connections: database.calendar_connections()?,
        schedule_blocks: database.schedule_for_day(day)?,
        schedule_days: database.schedules_for_days(days)?,
        synced_count,
        failed_count,
        warnings,
    })
}

fn sync_connection_days(
    database: &Database,
    connection: &CalendarConnection,
    days: &[String],
    viewer_timezone: &str,
) -> Result<Vec<String>, String> {
    let source = secret_store::read(&connection.id)?;
    let ics_text = match connection.kind.as_str() {
        LOCAL_FILE_KIND => read_local_ics(&source)?,
        SUBSCRIPTION_KIND => fetch_subscription_ics(&source)?,
        _ => return Err("不支持的日历连接类型".to_string()),
    };
    let source_name = format!("calendar:{}", connection.id);
    let mut warnings = Vec::new();
    let mut schedule_days = Vec::with_capacity(days.len());
    for day in days {
        let preview = calendar_import::preview_ics(&ics_text, day, viewer_timezone)?;
        warnings.extend(preview.warnings);
        let blocks = preview
            .events
            .into_iter()
            .map(|event| ScheduleBlock {
                id: format!("{}-{}", connection.id, event.id),
                title: event.title,
                start_minute: event.start_minute,
                end_minute: event.end_minute,
                category: if event.all_day { "admin" } else { "meeting" }.to_string(),
                source: source_name.clone(),
                status: "planned".to_string(),
                locked: Some(true),
            })
            .collect::<Vec<_>>();
        schedule_days.push(ScheduleDay {
            day: day.clone(),
            blocks,
        });
    }
    warnings.sort();
    warnings.dedup();
    database.replace_calendar_schedule_days(&connection.id, &schedule_days)?;
    Ok(warnings)
}

pub fn validate_sync_days(day: &str, days: &[String]) -> Result<(), String> {
    if days.is_empty() || days.len() > MAX_SYNC_DAYS {
        return Err(format!("日历同步日期数量需为 1 到 {MAX_SYNC_DAYS} 天"));
    }
    if !days.iter().any(|candidate| candidate == day) {
        return Err("日历同步日期必须包含当前查看日期".to_string());
    }
    let mut parsed_days = Vec::with_capacity(days.len());
    let mut unique_days = HashSet::with_capacity(days.len());
    for value in days {
        let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| format!("无效的日历同步日期：{value}"))?;
        if !unique_days.insert(parsed) {
            return Err("日历同步日期不能重复".to_string());
        }
        parsed_days.push(parsed);
    }
    for pair in parsed_days.windows(2) {
        if pair[0].succ_opt() != Some(pair[1]) {
            return Err("日历同步日期必须按时间连续排列".to_string());
        }
    }
    Ok(())
}

fn read_local_ics(source: &str) -> Result<String, String> {
    let path = Path::new(source);
    validate_local_file(path)?;
    let bytes = std::fs::read(path).map_err(|error| format!("无法读取本地日历文件：{error}"))?;
    if bytes.len() as u64 > MAX_ICS_BYTES {
        return Err("日历文件超过 2 MB 限制".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "日历文件不是有效的 UTF-8 文本".to_string())
}

fn fetch_subscription_ics(source: &str) -> Result<String, String> {
    let url = validate_subscription_url(source)?;
    let host = url
        .host_str()
        .ok_or_else(|| "订阅地址缺少主机名".to_string())?;
    let port = url.port_or_known_default().unwrap_or(443);
    let resolved = resolve_public_host(host, port)?;
    let mut builder = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .user_agent("AgentBar/0.1 calendar-sync");
    if !host.parse::<IpAddr>().is_ok() {
        builder = builder.resolve(host, resolved);
    }
    let client = builder
        .build()
        .map_err(|error| format!("无法初始化日历同步：{error}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("无法连接日历订阅：{}", error.without_url()))?;
    if response.status().is_redirection() {
        return Err("订阅地址发生重定向；为避免密钥泄露，请直接填写最终 HTTPS 地址".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("日历订阅返回 HTTP {}", response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ICS_BYTES)
    {
        return Err("日历订阅内容超过 2 MB 限制".to_string());
    }

    let mut bytes = Vec::new();
    let mut limited: Take<_> = response.take(MAX_ICS_BYTES + 1);
    limited
        .read_to_end(&mut bytes)
        .map_err(|error| format!("无法读取日历订阅：{error}"))?;
    if bytes.len() as u64 > MAX_ICS_BYTES {
        return Err("日历订阅内容超过 2 MB 限制".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "日历订阅不是有效的 UTF-8 文本".to_string())
}

fn normalize_display_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    let chars = value.chars().count();
    if chars == 0 || chars > MAX_DISPLAY_NAME_CHARS {
        return Err(format!("显示名称需为 1 到 {MAX_DISPLAY_NAME_CHARS} 个字符"));
    }
    Ok(value.to_string())
}

fn validate_refresh_minutes(value: i64) -> Result<i64, String> {
    if !(MIN_REFRESH_MINUTES..=MAX_REFRESH_MINUTES).contains(&value) {
        return Err(format!(
            "同步间隔需在 {MIN_REFRESH_MINUTES} 到 {MAX_REFRESH_MINUTES} 分钟之间"
        ));
    }
    Ok(value)
}

fn normalize_source(kind: &str, source: &str) -> Result<(String, String), String> {
    let source = source.trim();
    if source.is_empty() || source.chars().count() > MAX_SOURCE_CHARS {
        return Err(format!("连接地址需为 1 到 {MAX_SOURCE_CHARS} 个字符"));
    }
    match kind {
        LOCAL_FILE_KIND => {
            let path = std::fs::canonicalize(PathBuf::from(source))
                .map_err(|error| format!("无法打开本地日历文件：{error}"))?;
            validate_local_file(&path)?;
            let hint = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("本地日历.ics")
                .to_string();
            Ok((path.to_string_lossy().into_owned(), hint))
        }
        SUBSCRIPTION_KIND => {
            let url = validate_subscription_url(source)?;
            let hint = url
                .host_str()
                .ok_or_else(|| "订阅地址缺少主机名".to_string())?
                .to_string();
            Ok((url.to_string(), hint))
        }
        _ => Err("不支持的日历连接类型".to_string()),
    }
}

fn validate_local_file(path: &Path) -> Result<(), String> {
    let metadata = path
        .metadata()
        .map_err(|error| format!("无法读取本地日历文件：{error}"))?;
    if !metadata.is_file() {
        return Err("请选择一个 .ics 文件".to_string());
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        != Some("ics".to_string())
    {
        return Err("请选择一个 .ics 文件".to_string());
    }
    if metadata.len() > MAX_ICS_BYTES {
        return Err("日历文件超过 2 MB 限制".to_string());
    }
    Ok(())
}

fn validate_subscription_url(source: &str) -> Result<Url, String> {
    let url = Url::parse(source).map_err(|_| "订阅地址格式无效".to_string())?;
    if url.scheme() != "https" {
        return Err("日历订阅仅允许 HTTPS 地址".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("订阅地址不能包含用户名或密码字段".to_string());
    }
    if url.host_str().is_none() {
        return Err("订阅地址缺少主机名".to_string());
    }
    if url.fragment().is_some() {
        return Err("订阅地址不能包含 # 片段".to_string());
    }
    if let Some(ip) = url.host_str().and_then(|host| host.parse::<IpAddr>().ok()) {
        if !is_public_ip(ip) {
            return Err("订阅地址不能指向本机或私有网络".to_string());
        }
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return Err("订阅地址不能指向本机或私有网络".to_string());
    }
    Ok(url)
}

fn resolve_public_host(host: &str, port: u16) -> Result<SocketAddr, String> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| "无法解析日历订阅主机".to_string())?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("日历订阅主机没有可用地址".to_string());
    }
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err("日历订阅主机解析到本机或私有网络".to_string());
    }
    Ok(addresses[0])
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return is_public_ipv4(ipv4);
    }
    let segments = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xff00) == 0xff00
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn connection_due(connection: &CalendarConnection, now: i64) -> bool {
    connection.last_sync_at_ms.is_none_or(|last_sync| {
        now.saturating_sub(last_sync) >= connection.refresh_minutes * 60_000
    })
}

fn validate_connection_id(id: &str) -> Result<(), String> {
    if id.starts_with("cal-")
        && id.len() <= 96
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        Ok(())
    } else {
        Err("invalid calendar connection identifier".to_string())
    }
}

fn connection_id(kind: &str, source: &str, now: i64) -> String {
    let digest = Sha256::digest(format!("{kind}|{source}|{now}").as_bytes());
    let mut suffix = String::with_capacity(20);
    for byte in digest.iter().take(10) {
        let _ = write!(suffix, "{byte:02x}");
    }
    format!("cal-{suffix}")
}

fn truncate_error(error: &str) -> String {
    error.chars().take(240).collect()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::{is_public_ip, normalize_source, validate_subscription_url, validate_sync_days};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn subscription_urls_require_public_https_targets() {
        assert!(validate_subscription_url("https://calendar.google.com/private/basic.ics").is_ok());
        assert!(validate_subscription_url("http://calendar.google.com/basic.ics").is_err());
        assert!(validate_subscription_url("https://localhost/calendar.ics").is_err());
        assert!(validate_subscription_url("https://127.0.0.1/calendar.ics").is_err());
        assert!(validate_subscription_url("https://user:pass@example.com/calendar.ics").is_err());
    }

    #[test]
    fn public_ip_filter_rejects_private_and_documentation_ranges() {
        assert!(!is_public_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(!is_public_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!is_public_ip(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))));
        assert!(is_public_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_public_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn source_metadata_keeps_only_a_safe_hint() {
        let (source, hint) = normalize_source(
            "ics-subscription",
            "https://calendar.example/private/token.ics?key=secret",
        )
        .unwrap();
        assert!(source.contains("key=secret"));
        assert_eq!(hint, "calendar.example");
        assert!(!hint.contains("secret"));
    }

    #[test]
    fn weekly_sync_dates_must_be_bounded_consecutive_and_include_today() {
        let week = (20..=26)
            .map(|day| format!("2026-07-{day}"))
            .collect::<Vec<_>>();
        assert!(validate_sync_days("2026-07-22", &week).is_ok());
        assert!(validate_sync_days("2026-07-27", &week).is_err());
        assert!(
            validate_sync_days("2026-07-22", &["2026-07-20".into(), "2026-07-22".into()]).is_err()
        );
        assert!(
            validate_sync_days("2026-07-22", &["2026-07-22".into(), "2026-07-22".into()]).is_err()
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "uses the current Windows Credential Manager for a full local-file sync"]
    fn local_file_connection_syncs_and_cleans_up() {
        use super::{create_connection, delete_connection, sync_connections_range};
        use crate::database::Database;

        let suffix = format!("{}-{}", std::process::id(), super::now_ms());
        let calendar_path = std::env::temp_dir().join(format!("agent-bar-{suffix}.ics"));
        let database_path = std::env::temp_dir().join(format!("agent-bar-{suffix}.sqlite3"));
        std::fs::write(
            &calendar_path,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Agent Bar Test//EN\r\nBEGIN:VEVENT\r\nUID:connected-calendar-test\r\nDTSTART:20260722T090000Z\r\nDTEND:20260722T100000Z\r\nSUMMARY:Connected calendar event\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:connected-calendar-next-day\r\nDTSTART:20260723T010000Z\r\nDTEND:20260723T020000Z\r\nSUMMARY:Next day calendar event\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        )
        .unwrap();
        let database = Database::open(&database_path).unwrap();
        let connection = create_connection(
            &database,
            "Test calendar",
            "local-file",
            calendar_path.to_str().unwrap(),
            30,
        )
        .unwrap();

        let test_result = (|| -> Result<(), String> {
            let days = vec!["2026-07-22".to_string(), "2026-07-23".to_string()];
            let batch = sync_connections_range(
                &database,
                "2026-07-22",
                &days,
                "Asia/Shanghai",
                Some(&connection.id),
                true,
            )?;
            if batch.synced_count != 1 || batch.failed_count != 0 {
                return Err("expected one successful calendar sync".to_string());
            }
            if batch.schedule_days.len() != 2
                || batch.schedule_days[1]
                    .blocks
                    .iter()
                    .all(|block| block.title != "Next day calendar event")
            {
                return Err("expected one fetch to materialize both calendar days".to_string());
            }
            let event = batch
                .schedule_blocks
                .iter()
                .find(|block| block.title == "Connected calendar event")
                .ok_or_else(|| "synced event was not saved".to_string())?;
            if event.start_minute != 17 * 60 || event.end_minute != 18 * 60 {
                return Err("synced event was not converted to the viewer timezone".to_string());
            }
            if connection.source_hint != calendar_path.file_name().unwrap().to_string_lossy() {
                return Err("connection metadata exposed more than the file name".to_string());
            }
            Ok(())
        })();

        let cleanup_result = delete_connection(&database, &connection.id);
        drop(database);
        let _ = std::fs::remove_file(&calendar_path);
        let _ = std::fs::remove_file(&database_path);
        let _ = std::fs::remove_file(format!("{}-wal", database_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", database_path.display()));
        cleanup_result.unwrap();
        test_result.unwrap();
    }
}
