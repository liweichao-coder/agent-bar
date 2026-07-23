use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_IDLE_THRESHOLD_MINUTES: i64 = 5;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureState {
    pub status: String,
    pub idle_seconds: u64,
    pub threshold_minutes: i64,
    pub capture_allowed: bool,
    pub session_state_available: bool,
    pub checked_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionAvailability {
    Active,
    Locked,
    Disconnected,
    Unknown,
}

fn classify_capture_state(
    idle_ms: u64,
    threshold_minutes: i64,
    session: SessionAvailability,
    checked_at_ms: i64,
) -> ActivityCaptureState {
    let threshold_minutes = threshold_minutes.clamp(1, 60);
    let idle = idle_ms >= threshold_minutes as u64 * 60_000;
    let (status, capture_allowed) = match session {
        SessionAvailability::Disconnected => ("disconnected", false),
        SessionAvailability::Locked => ("locked", false),
        _ if idle => ("idle", false),
        SessionAvailability::Active | SessionAvailability::Unknown => ("active", true),
    };
    ActivityCaptureState {
        status: status.to_string(),
        idle_seconds: idle_ms / 1_000,
        threshold_minutes,
        capture_allowed,
        session_state_available: session != SessionAvailability::Unknown,
        checked_at_ms,
    }
}

pub fn paused_capture_state(threshold_minutes: i64) -> ActivityCaptureState {
    ActivityCaptureState {
        status: "paused".to_string(),
        idle_seconds: 0,
        threshold_minutes: threshold_minutes.clamp(1, 60),
        capture_allowed: false,
        session_state_available: cfg!(windows),
        checked_at_ms: now_ms().unwrap_or_default(),
    }
}

fn elapsed_tick_ms(current_tick: u32, last_input_tick: u32) -> u64 {
    let elapsed = current_tick.wrapping_sub(last_input_tick);
    if elapsed > i32::MAX as u32 {
        0
    } else {
        elapsed as u64
    }
}

#[cfg(windows)]
pub fn activity_capture_state(threshold_minutes: i64) -> Result<ActivityCaptureState, String> {
    use std::ffi::c_void;
    use std::mem::size_of;
    use windows::core::PWSTR;
    use windows::Win32::System::RemoteDesktop::{
        WTSActive, WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW,
        WTS_CURRENT_SESSION,
    };
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut last_input = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if !GetLastInputInfo(&mut last_input).as_bool() {
            return Err("could not read Windows last input time".to_string());
        }
        let idle_ms = elapsed_tick_ms(GetTickCount(), last_input.dwTime);

        let session = (|| -> Result<SessionAvailability, String> {
            let mut buffer = PWSTR::null();
            let mut bytes_returned = 0u32;
            WTSQuerySessionInformationW(
                None,
                WTS_CURRENT_SESSION,
                WTSSessionInfoEx,
                &mut buffer,
                &mut bytes_returned,
            )
            .map_err(|error| format!("could not read Windows session state: {error}"))?;
            if buffer.is_null() {
                return Err("Windows session state returned an empty buffer".to_string());
            }
            let result = (|| {
                if bytes_returned < size_of::<WTSINFOEXW>() as u32 {
                    return Err("Windows session state buffer is too small".to_string());
                }
                let info = *(buffer.0 as *const WTSINFOEXW);
                if info.Level != 1 {
                    return Err("Windows session state used an unsupported level".to_string());
                }
                let level = info.Data.WTSInfoExLevel1;
                if level.SessionState != WTSActive {
                    Ok(SessionAvailability::Disconnected)
                } else {
                    match level.SessionFlags {
                        0 => Ok(SessionAvailability::Locked),
                        1 => Ok(SessionAvailability::Active),
                        _ => Ok(SessionAvailability::Unknown),
                    }
                }
            })();
            WTSFreeMemory(buffer.0 as *mut c_void);
            result
        })()
        .unwrap_or(SessionAvailability::Unknown);

        Ok(classify_capture_state(
            idle_ms,
            threshold_minutes,
            session,
            now_ms()?,
        ))
    }
}

#[cfg(not(windows))]
pub fn activity_capture_state(threshold_minutes: i64) -> Result<ActivityCaptureState, String> {
    Ok(ActivityCaptureState {
        status: "unavailable".to_string(),
        idle_seconds: 0,
        threshold_minutes: threshold_minutes.clamp(1, 60),
        capture_allowed: false,
        session_state_available: false,
        checked_at_ms: now_ms()?,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundSnapshot {
    pub app_name: String,
    pub sanitized_window_title: Option<String>,
    pub captured_at_ms: i64,
}

pub fn now_ms() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?;
    i64::try_from(duration.as_millis()).map_err(|_| "timestamp is out of range".to_string())
}

pub fn sanitize_window_title(title: &str) -> Option<String> {
    static EMAIL: OnceLock<Regex> = OnceLock::new();
    static USER_PATH: OnceLock<Regex> = OnceLock::new();
    static LONG_NUMBER: OnceLock<Regex> = OnceLock::new();
    static URL_QUERY: OnceLock<Regex> = OnceLock::new();
    static WHITESPACE: OnceLock<Regex> = OnceLock::new();

    let email = EMAIL.get_or_init(|| {
        Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").expect("valid email regex")
    });
    let user_path = USER_PATH.get_or_init(|| {
        Regex::new(r"(?i)([A-Z]:\\Users\\)[^\\\s]+").expect("valid Windows user path regex")
    });
    let long_number =
        LONG_NUMBER.get_or_init(|| Regex::new(r"\b\d{7,}\b").expect("valid long number regex"));
    let url_query = URL_QUERY.get_or_init(|| {
        Regex::new(r"(?i)(https?://[^\s?]+)\?[^\s]+").expect("valid URL query regex")
    });
    let whitespace = WHITESPACE.get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"));

    let mut sanitized = title.trim().to_string();
    sanitized = email.replace_all(&sanitized, "[email]").into_owned();
    sanitized = user_path.replace_all(&sanitized, "${1}[user]").into_owned();
    sanitized = long_number.replace_all(&sanitized, "[number]").into_owned();
    sanitized = url_query
        .replace_all(&sanitized, "${1}?[redacted]")
        .into_owned();
    sanitized = whitespace.replace_all(&sanitized, " ").into_owned();

    if sanitized.chars().count() > 120 {
        sanitized = sanitized.chars().take(117).collect::<String>();
        sanitized.push_str("...");
    }

    (!sanitized.is_empty()).then_some(sanitized)
}

#[cfg(windows)]
pub fn capture_foreground_window(include_title: bool) -> Result<ForegroundSnapshot, String> {
    use std::path::Path;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let window = GetForegroundWindow();
        if window.0.is_null() {
            return Err("no foreground window is available".to_string());
        }

        let mut process_id = 0u32;
        GetWindowThreadProcessId(window, Some(&mut process_id));
        if process_id == 0 {
            return Err("could not resolve the foreground process".to_string());
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
            .map_err(|error| format!("could not open foreground process: {error}"))?;
        let mut process_path = vec![0u16; 32_768];
        let mut process_path_len = process_path.len() as u32;
        let process_result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(process_path.as_mut_ptr()),
            &mut process_path_len,
        );
        let _ = CloseHandle(process);
        process_result
            .map_err(|error| format!("could not read foreground process name: {error}"))?;

        let process_path = String::from_utf16_lossy(&process_path[..process_path_len as usize]);
        let app_name = Path::new(&process_path)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("Unknown app")
            .to_string();

        let sanitized_window_title = if include_title {
            let title_len = GetWindowTextLengthW(window);
            if title_len > 0 {
                let mut title = vec![0u16; title_len as usize + 1];
                let copied = GetWindowTextW(window, &mut title);
                sanitize_window_title(&String::from_utf16_lossy(&title[..copied.max(0) as usize]))
            } else {
                None
            }
        } else {
            None
        };

        Ok(ForegroundSnapshot {
            app_name,
            sanitized_window_title,
            captured_at_ms: now_ms()?,
        })
    }
}

#[cfg(not(windows))]
pub fn capture_foreground_window(_include_title: bool) -> Result<ForegroundSnapshot, String> {
    Err("foreground activity capture is currently implemented for Windows only".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_capture_state, elapsed_tick_ms, sanitize_window_title, SessionAvailability,
    };

    #[test]
    fn redacts_common_sensitive_values() {
        let title = r"Resume 13800138000 user@example.com C:\Users\Alice\resume.docx";
        let sanitized = sanitize_window_title(title).expect("sanitized title");

        assert!(!sanitized.contains("13800138000"));
        assert!(!sanitized.contains("user@example.com"));
        assert!(!sanitized.contains("Alice"));
        assert!(sanitized.contains("[number]"));
        assert!(sanitized.contains("[email]"));
        assert!(sanitized.contains(r"C:\Users\[user]"));
    }

    #[test]
    fn trims_and_limits_long_titles() {
        let title = format!("   {}   ", "x".repeat(180));
        let sanitized = sanitize_window_title(&title).expect("sanitized title");

        assert_eq!(sanitized.chars().count(), 120);
        assert!(sanitized.ends_with("..."));
    }

    #[test]
    fn pauses_capture_at_idle_boundary_and_for_locked_sessions() {
        let active = classify_capture_state(299_999, 5, SessionAvailability::Active, 1);
        assert_eq!(active.status, "active");
        assert!(active.capture_allowed);

        let idle = classify_capture_state(300_000, 5, SessionAvailability::Active, 2);
        assert_eq!(idle.status, "idle");
        assert!(!idle.capture_allowed);

        let locked = classify_capture_state(0, 5, SessionAvailability::Locked, 3);
        assert_eq!(locked.status, "locked");
        assert!(!locked.capture_allowed);
    }

    #[test]
    fn unknown_session_state_falls_back_to_idle_detection() {
        let active = classify_capture_state(10_000, 5, SessionAvailability::Unknown, 1);
        assert!(active.capture_allowed);
        assert!(!active.session_state_available);

        let idle = classify_capture_state(600_000, 5, SessionAvailability::Unknown, 2);
        assert!(!idle.capture_allowed);
    }

    #[test]
    fn tick_math_handles_wrap_and_rejects_implausible_reverse_values() {
        assert_eq!(elapsed_tick_ms(1_000, 900), 100);
        assert_eq!(elapsed_tick_ms(10, u32::MAX - 9), 20);
        assert_eq!(elapsed_tick_ms(100, 200), 0);
    }

    #[cfg(windows)]
    #[test]
    fn reads_current_windows_capture_state() {
        let state = super::activity_capture_state(5).expect("read Windows activity state");
        println!(
            "Windows capture state: status={}, idle_seconds={}, session_state_available={}",
            state.status, state.idle_seconds, state.session_state_available
        );
        assert!(matches!(
            state.status.as_str(),
            "active" | "idle" | "locked" | "disconnected"
        ));
        assert_eq!(state.threshold_minutes, 5);
    }
}
