use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_SCREENSHOT_BYTES: usize = 8 * 1024 * 1024;
const MAX_SCREENSHOT_NAME_CHARS: usize = 120;
const MAX_TASKS: usize = 30;
const MAX_TITLE_CHARS: usize = 120;
const MAX_NOTES_CHARS: usize = 300;
const MAX_WARNINGS: usize = 10;
const MAX_WARNING_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotImportTask {
    pub id: String,
    pub title: String,
    pub duration_minutes: i32,
    pub priority: String,
    pub preferred_period: String,
    pub category: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotImportSnapshot {
    pub status: String,
    pub file_name: Option<String>,
    pub tasks: Vec<ScreenshotImportTask>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

impl ScreenshotImportSnapshot {
    pub fn idle() -> Self {
        Self {
            status: "idle".to_string(),
            file_name: None,
            tasks: Vec::new(),
            warnings: Vec::new(),
            error: None,
        }
    }
}

pub(crate) struct ScreenshotImportRuntime {
    pub snapshot: ScreenshotImportSnapshot,
    pub thread_id: Option<String>,
    pub temp_path: Option<PathBuf>,
}

impl ScreenshotImportRuntime {
    pub fn new() -> Self {
        Self {
            snapshot: ScreenshotImportSnapshot::idle(),
            thread_id: None,
            temp_path: None,
        }
    }

    pub fn cleanup_image(&mut self) {
        if let Some(path) = self.temp_path.take() {
            let _ = fs::remove_file(path);
        }
    }

    pub fn clear(&mut self) {
        self.cleanup_image();
        self.snapshot = ScreenshotImportSnapshot::idle();
        self.thread_id = None;
    }
}

impl Drop for ScreenshotImportRuntime {
    fn drop(&mut self) {
        self.cleanup_image();
    }
}

pub(crate) struct StagedScreenshot {
    pub display_name: String,
    pub path: PathBuf,
}

pub(crate) fn stage_screenshot(
    cache_dir: &Path,
    file_name: &str,
    mime_type: &str,
    base64_data: &str,
    sequence: u64,
) -> Result<StagedScreenshot, String> {
    if base64_data.len() > (MAX_SCREENSHOT_BYTES * 4 / 3) + 16 {
        return Err("截图超过 8 MB 限制".to_string());
    }
    let bytes = STANDARD
        .decode(base64_data)
        .map_err(|_| "截图数据不是有效的 Base64".to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_SCREENSHOT_BYTES {
        return Err("截图必须在 1 字节到 8 MB 之间".to_string());
    }
    let extension = detect_image_extension(&bytes, mime_type)?;
    fs::create_dir_all(cache_dir).map_err(|error| format!("无法创建截图临时目录：{error}"))?;
    let path = cache_dir.join(format!("screenshot-import-{sequence}.{extension}"));
    fs::write(&path, bytes).map_err(|error| format!("无法暂存截图：{error}"))?;
    Ok(StagedScreenshot {
        display_name: clean_file_name(file_name),
        path,
    })
}

pub(crate) fn parse_screenshot_result(
    arguments: &Value,
    id_seed: u64,
) -> Result<(Vec<ScreenshotImportTask>, Vec<String>), String> {
    let tasks = arguments
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex 返回结果缺少 tasks 数组".to_string())?;
    if tasks.len() > MAX_TASKS {
        return Err(format!("截图一次最多导入 {MAX_TASKS} 项事项"));
    }

    let mut parsed = Vec::with_capacity(tasks.len());
    for (index, task) in tasks.iter().enumerate() {
        let title = required_text(task, "title", MAX_TITLE_CHARS)?;
        let duration_minutes = task
            .get("durationMinutes")
            .and_then(Value::as_i64)
            .ok_or_else(|| format!("事项 {} 缺少有效时长", index + 1))?;
        if !(15..=480).contains(&duration_minutes) || duration_minutes % 15 != 0 {
            return Err(format!(
                "事项 {} 时长必须是 15 至 480 分钟的 15 分钟倍数",
                index + 1
            ));
        }
        let priority = enum_text(task, "priority", &["critical", "high", "normal", "low"])?;
        let preferred_period = enum_text(
            task,
            "preferredPeriod",
            &["any", "morning", "afternoon", "evening"],
        )?;
        let category = enum_text(task, "category", &["focus", "meeting", "admin", "life"])?;
        let notes = optional_text(task, "notes", MAX_NOTES_CHARS)?;
        parsed.push(ScreenshotImportTask {
            id: format!("screenshot-{id_seed}-{index}"),
            title,
            duration_minutes: duration_minutes as i32,
            priority,
            preferred_period,
            category,
            notes,
        });
    }

    let warnings = arguments
        .get("warnings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|warning| !warning.is_empty())
                .take(MAX_WARNINGS)
                .map(|warning| truncate_chars(warning, MAX_WARNING_CHARS))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok((parsed, warnings))
}

fn detect_image_extension(bytes: &[u8], mime_type: &str) -> Result<&'static str, String> {
    let detected = if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some(("png", "image/png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("jpg", "image/jpeg"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else {
        None
    };
    let Some((extension, expected_mime)) = detected else {
        return Err("仅支持 PNG、JPEG 或 WebP 截图".to_string());
    };
    if mime_type != expected_mime {
        return Err("截图扩展格式与 MIME 类型不一致".to_string());
    }
    Ok(extension)
}

fn clean_file_name(file_name: &str) -> String {
    let name = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("screenshot");
    let visible = name
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let trimmed = visible.trim();
    if trimmed.is_empty() {
        "screenshot".to_string()
    } else {
        truncate_chars(trimmed, MAX_SCREENSHOT_NAME_CHARS)
    }
}

fn required_text(value: &Value, key: &str, max_chars: usize) -> Result<String, String> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("截图事项缺少 {key}"))?;
    if text.chars().count() > max_chars || text.chars().any(char::is_control) {
        return Err(format!("截图事项 {key} 格式无效"));
    }
    Ok(text.to_string())
}

fn optional_text(value: &Value, key: &str, max_chars: usize) -> Result<Option<String>, String> {
    let Some(text) = value.get(key).and_then(Value::as_str) else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > max_chars || text.chars().any(char::is_control) {
        return Err(format!("截图事项 {key} 格式无效"));
    }
    Ok(Some(text.to_string()))
}

fn enum_text(value: &Value, key: &str, allowed: &[&str]) -> Result<String, String> {
    let candidate = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("截图事项缺少 {key}"))?;
    if !allowed.contains(&candidate) {
        return Err(format!("截图事项 {key} 取值无效"));
    }
    Ok(candidate.to_string())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_bounded_structured_tasks() {
        let (tasks, warnings) = parse_screenshot_result(
            &json!({
                "tasks": [{
                    "title": "提交报名材料",
                    "durationMinutes": 30,
                    "priority": "critical",
                    "preferredPeriod": "morning",
                    "category": "admin",
                    "notes": "截图显示截止到周五"
                }],
                "warnings": ["具体截止时间不清晰"]
            }),
            42,
        )
        .expect("parse screenshot tasks");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "screenshot-42-0");
        assert_eq!(tasks[0].duration_minutes, 30);
        assert_eq!(warnings, vec!["具体截止时间不清晰"]);
    }

    #[test]
    fn rejects_invalid_duration_and_enum_values() {
        let result = parse_screenshot_result(
            &json!({
                "tasks": [{
                    "title": "任务",
                    "durationMinutes": 17,
                    "priority": "urgent",
                    "preferredPeriod": "any",
                    "category": "admin"
                }]
            }),
            1,
        );
        assert!(result.is_err());
    }

    #[test]
    fn validates_image_magic_and_mime_together() {
        assert_eq!(
            detect_image_extension(
                &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
                "image/png"
            ),
            Ok("png")
        );
        assert!(detect_image_extension(&[0xff, 0xd8, 0xff], "image/png").is_err());
        assert!(detect_image_extension(b"not an image", "image/png").is_err());
    }
}
