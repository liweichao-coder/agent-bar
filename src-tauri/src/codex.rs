use crate::database::{AgentEventRecord, Database};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const RECENT_ACTIVITY_WINDOW_MS: i64 = 3 * 60 * 1000;
const HOOK_EVENT_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPosition {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAgent {
    id: String,
    name: String,
    provider: String,
    task: String,
    detail: String,
    status: String,
    elapsed_minutes: i64,
    accent: &'static str,
    position: CodexPosition,
    capabilities: Vec<&'static str>,
    control_mode: &'static str,
    updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSnapshot {
    pub connection_state: &'static str,
    pub adapter_mode: &'static str,
    pub agents: Vec<CodexAgent>,
    pub last_synced_at_ms: Option<i64>,
    pub message: String,
}

impl CodexSnapshot {
    fn disabled() -> Self {
        Self {
            connection_state: "disabled",
            adapter_mode: "app-server-observer",
            agents: Vec::new(),
            last_synced_at_ms: None,
            message: "Codex observation is disabled".to_string(),
        }
    }

    fn error(message: String) -> Self {
        Self {
            connection_state: "error",
            adapter_mode: "app-server-observer",
            agents: Vec::new(),
            last_synced_at_ms: Some(now_ms()),
            message,
        }
    }
}

pub struct CodexMonitor {
    snapshot: Arc<Mutex<CodexSnapshot>>,
}

impl CodexMonitor {
    pub fn start(database: Arc<Database>) -> Result<Self, String> {
        let snapshot = Arc::new(Mutex::new(CodexSnapshot::disabled()));
        let worker_snapshot = snapshot.clone();
        std::thread::Builder::new()
            .name("codex-observer".to_string())
            .spawn(move || loop {
                let enabled = database.codex_observation_enabled().unwrap_or(false);
                let next = if enabled {
                    refresh_snapshot(&database)
                } else {
                    CodexSnapshot::disabled()
                };
                if let Ok(mut current) = worker_snapshot.lock() {
                    *current = next;
                }
                std::thread::sleep(POLL_INTERVAL);
            })
            .map_err(|error| format!("could not start Codex observer: {error}"))?;
        Ok(Self { snapshot })
    }

    pub fn snapshot(&self) -> Result<CodexSnapshot, String> {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| "Codex observer lock was poisoned".to_string())
    }

    pub fn refresh(&self, database: &Database) -> Result<CodexSnapshot, String> {
        let enabled = database.codex_observation_enabled()?;
        let next = if enabled {
            refresh_snapshot(database)
        } else {
            CodexSnapshot::disabled()
        };
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| "Codex observer lock was poisoned".to_string())?;
        *current = next.clone();
        Ok(next)
    }
}

fn refresh_snapshot(database: &Database) -> CodexSnapshot {
    let _ = ingest_hook_log(database);
    let events = database
        .recent_agent_events(now_ms() - HOOK_EVENT_WINDOW_MS, 500)
        .unwrap_or_default();
    match poll_codex_metadata() {
        Ok(snapshot) => apply_hook_events(snapshot, &events),
        Err(error) if !events.is_empty() => apply_hook_events(
            CodexSnapshot {
                connection_state: "connected",
                adapter_mode: "app-server-observer",
                agents: Vec::new(),
                last_synced_at_ms: Some(now_ms()),
                message: format!("Hook events connected; App Server unavailable: {error}"),
            },
            &events,
        ),
        Err(error) => CodexSnapshot::error(error),
    }
}

fn poll_codex_metadata() -> Result<CodexSnapshot, String> {
    let executable = find_codex_executable()
        .ok_or_else(|| "could not find a Codex CLI with app-server support".to_string())?;
    let mut child = Command::new(&executable)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not start Codex app-server: {error}"))?;

    let result = query_thread_list(&mut child);
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookLogEvent {
    id: String,
    #[serde(default = "codex_provider")]
    provider: String,
    event_name: String,
    session_id: String,
    turn_id: Option<String>,
    agent_id: Option<String>,
    agent_type: Option<String>,
    tool_name: Option<String>,
    source: Option<String>,
    permission_mode: Option<String>,
    occurred_at_ms: i64,
}

fn codex_provider() -> String {
    "codex".to_string()
}

fn ingest_hook_log(database: &Database) -> Result<(), String> {
    let Some(path) = hook_event_log_path() else {
        return Ok(());
    };
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("could not open Codex hook event log: {error}")),
    };
    let allowed_events = [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "PreCompact",
        "PostCompact",
        "SubagentStart",
        "SubagentStop",
        "Stop",
    ];
    let mut records = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(event) = serde_json::from_str::<HookLogEvent>(&line) else {
            continue;
        };
        if !allowed_events.contains(&event.event_name.as_str())
            || event.id.len() > 128
            || event.session_id.len() > 128
        {
            continue;
        }
        let tool_name = event.tool_name.map(|name| truncate(&name, 120));
        let metadata_json = json!({
            "toolName": tool_name,
            "agentType": event.agent_type.map(|value| truncate(&value, 80)),
            "source": event.source.map(|value| truncate(&value, 80)),
            "permissionMode": event.permission_mode.map(|value| truncate(&value, 40)),
        })
        .to_string();
        records.push(AgentEventRecord {
            id: event.id,
            provider: truncate(&event.provider, 40),
            session_id: event.session_id,
            turn_id: event.turn_id.map(|value| truncate(&value, 128)),
            agent_id: event.agent_id.map(|value| truncate(&value, 128)),
            activity_kind: hook_activity_kind(&event.event_name, tool_name.as_deref()).to_string(),
            event_name: event.event_name,
            occurred_at_ms: event.occurred_at_ms,
            metadata_json,
        });
    }
    database.save_agent_events(&records)
}

fn hook_event_log_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("AGENT_BAR_EVENT_LOG") {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("com.agentbar.desktop").join("codex-events.jsonl"))
    }
    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME").map(PathBuf::from).map(|path| {
            path.join("Library")
                .join("Application Support")
                .join("com.agentbar.desktop")
                .join("codex-events.jsonl")
        })
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|path| path.join(".local").join("share"))
            })
            .map(|path| path.join("com.agentbar.desktop").join("codex-events.jsonl"))
    }
}

fn apply_hook_events(mut snapshot: CodexSnapshot, events: &[AgentEventRecord]) -> CodexSnapshot {
    let mut parent_events: HashMap<&str, &AgentEventRecord> = HashMap::new();
    let mut subagent_events: HashMap<&str, &AgentEventRecord> = HashMap::new();
    for event in events {
        if matches!(event.event_name.as_str(), "SubagentStart" | "SubagentStop") {
            if let Some(agent_id) = event.agent_id.as_deref() {
                subagent_events.insert(agent_id, event);
            }
        } else {
            parent_events.insert(&event.session_id, event);
        }
    }

    for agent in &mut snapshot.agents {
        if let Some(event) = parent_events.remove(agent.id.as_str()) {
            apply_event(agent, event);
        }
    }
    for (session_id, event) in parent_events {
        let mut agent = hook_only_agent(session_id, "Codex", "Codex Hook", snapshot.agents.len());
        apply_event(&mut agent, event);
        snapshot.agents.push(agent);
    }
    for (agent_id, event) in subagent_events {
        let metadata: Value = serde_json::from_str(&event.metadata_json).unwrap_or(Value::Null);
        let name = metadata
            .get("agentType")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("Codex 子 Agent");
        let mut agent = hook_only_agent(
            &format!("hook-agent-{agent_id}"),
            name,
            "Codex Subagent Hook",
            snapshot.agents.len(),
        );
        apply_event(&mut agent, event);
        snapshot.agents.push(agent);
    }
    if !events.is_empty() {
        snapshot.message = format!(
            "Codex metadata connected; {} hook events cached",
            events.len()
        );
    }
    snapshot
}

fn hook_only_agent(id: &str, name: &str, provider: &str, index: usize) -> CodexAgent {
    let positions = [
        (24, 58),
        (66, 34),
        (74, 68),
        (43, 34),
        (31, 75),
        (55, 69),
        (82, 48),
        (52, 50),
    ];
    let accents = [
        "#72d6a5", "#6eb5ff", "#f0b36f", "#ff8c7a", "#c2a8ff", "#8fd5d2", "#e7c66d", "#9ccf7c",
    ];
    let (x, y) = positions[index % positions.len()];
    CodexAgent {
        id: id.to_string(),
        name: name.to_string(),
        provider: provider.to_string(),
        task: "Codex 会话".to_string(),
        detail: "已接收生命周期事件".to_string(),
        status: "recent".to_string(),
        elapsed_minutes: 0,
        accent: accents[index % accents.len()],
        position: CodexPosition { x, y },
        capabilities: Vec::new(),
        control_mode: "observed",
        updated_at_ms: now_ms(),
    }
}

fn apply_event(agent: &mut CodexAgent, event: &AgentEventRecord) {
    let metadata: Value = serde_json::from_str(&event.metadata_json).unwrap_or(Value::Null);
    let tool_name = metadata.get("toolName").and_then(Value::as_str);
    agent.status = match event.event_name.as_str() {
        "PermissionRequest" => "waiting",
        "Stop" | "SubagentStop" => "idle",
        "PreToolUse" if hook_activity_kind("PreToolUse", tool_name) == "searching" => "searching",
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PreCompact" | "PostCompact"
        | "SubagentStart" => "working",
        _ => "recent",
    }
    .to_string();
    agent.detail = match event.event_name.as_str() {
        "SessionStart" => "会话已开始",
        "UserPromptSubmit" => "正在理解任务",
        "PreToolUse" if tool_name.is_some() => "正在调用工具",
        "PreToolUse" => "正在调用工具",
        "PermissionRequest" => "等待 Codex 中的批准",
        "PostToolUse" => "工具调用已完成",
        "PreCompact" => "正在整理上下文",
        "PostCompact" => "上下文整理完成",
        "SubagentStart" => "子 Agent 正在工作",
        "SubagentStop" => "子 Agent 已完成",
        "Stop" => "本轮已完成",
        _ => "已接收生命周期事件",
    }
    .to_string();
    agent.elapsed_minutes = ((now_ms() - event.occurred_at_ms).max(0) / 60_000).min(999);
    agent.updated_at_ms = event.occurred_at_ms;
}

fn hook_activity_kind(event_name: &str, tool_name: Option<&str>) -> &'static str {
    match event_name {
        "PermissionRequest" => "waiting",
        "Stop" | "SubagentStop" => "idle",
        "PreCompact" | "PostCompact" => "compacting",
        "PreToolUse"
            if tool_name.is_some_and(|name| {
                let name = name.to_ascii_lowercase();
                name.contains("search") || name.contains("web") || name.contains("browser")
            }) =>
        {
            "searching"
        }
        "PreToolUse" | "PostToolUse" => "tool",
        "UserPromptSubmit" | "SubagentStart" => "working",
        _ => "recent",
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn query_thread_list(child: &mut Child) -> Result<CodexSnapshot, String> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Codex app-server stdin is unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Codex app-server stdout is unavailable".to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(message) = serde_json::from_str::<Value>(&line) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        }
    });

    send(
        &mut stdin,
        &json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "agent_bar",
                    "title": "Agent Bar",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
    )?;
    let initialized = receive_response(&receiver, 1)?;
    if let Some(error) = initialized.get("error") {
        return Err(format!("Codex initialization failed: {error}"));
    }

    send(&mut stdin, &json!({ "method": "initialized" }))?;
    send(
        &mut stdin,
        &json!({
            "method": "thread/list",
            "id": 2,
            "params": {
                "limit": 8,
                "sortKey": "updated_at",
                "sortDirection": "desc",
                "sourceKinds": [
                    "cli", "vscode", "exec", "appServer", "subAgent",
                    "subAgentReview", "subAgentCompact", "subAgentThreadSpawn",
                    "subAgentOther", "unknown"
                ],
                "useStateDbOnly": true
            }
        }),
    )?;
    let response = receive_response(&receiver, 2)?;
    if let Some(error) = response.get("error") {
        return Err(format!("Codex thread list failed: {error}"));
    }
    parse_thread_list(&response, now_ms())
}

fn send(stdin: &mut ChildStdin, message: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *stdin, message)
        .map_err(|error| format!("could not encode Codex request: {error}"))?;
    stdin
        .write_all(b"\n")
        .and_then(|_| stdin.flush())
        .map_err(|error| format!("could not send Codex request: {error}"))
}

fn receive_response(receiver: &mpsc::Receiver<Value>, expected_id: i64) -> Result<Value, String> {
    loop {
        let message = receiver
            .recv_timeout(RESPONSE_TIMEOUT)
            .map_err(|_| "Codex app-server response timed out".to_string())?;
        if message.get("id").and_then(Value::as_i64) == Some(expected_id) {
            return Ok(message);
        }
    }
}

fn parse_thread_list(response: &Value, captured_at_ms: i64) -> Result<CodexSnapshot, String> {
    let threads = response
        .pointer("/result/data")
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex thread list response had an unexpected shape".to_string())?;
    let positions = [
        (24, 58),
        (66, 34),
        (74, 68),
        (43, 34),
        (31, 75),
        (55, 69),
        (82, 48),
        (52, 50),
    ];
    let accents = [
        "#72d6a5", "#6eb5ff", "#f0b36f", "#ff8c7a", "#c2a8ff", "#8fd5d2", "#e7c66d", "#9ccf7c",
    ];

    let agents = threads
        .iter()
        .enumerate()
        .filter_map(|(index, thread)| {
            let id = thread.get("id")?.as_str()?.to_string();
            let updated_at_ms = thread
                .get("updatedAt")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                * 1000;
            let status_type = thread
                .pointer("/status/type")
                .and_then(Value::as_str)
                .unwrap_or("notLoaded");
            let active_flags = thread
                .pointer("/status/activeFlags")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let waiting = active_flags.iter().any(|flag| {
                flag.as_str()
                    .is_some_and(|flag| flag.to_ascii_lowercase().contains("waiting"))
            });
            let status = match status_type {
                "active" if waiting => "waiting",
                "active" => "working",
                "systemError" => "blocked",
                "idle" => "idle",
                _ if captured_at_ms - updated_at_ms <= RECENT_ACTIVITY_WINDOW_MS => "recent",
                _ => "idle",
            };
            let source = source_label(thread.get("source"));
            let task = thread
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or("未命名 Codex 任务")
                .to_string();
            let name = thread
                .get("agentNickname")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if source.is_subagent {
                        "Codex 子 Agent".to_string()
                    } else {
                        "Codex".to_string()
                    }
                });
            let workspace = thread
                .get("cwd")
                .and_then(Value::as_str)
                .and_then(workspace_name);
            let detail = match (status, workspace) {
                ("working", Some(workspace)) => format!("正在处理 · {workspace}"),
                ("waiting", Some(workspace)) => format!("等待批准 · {workspace}"),
                ("recent", Some(workspace)) => format!("最近有活动 · {workspace}"),
                ("blocked", Some(workspace)) => format!("运行异常 · {workspace}"),
                (_, Some(workspace)) => format!("当前空闲 · {workspace}"),
                ("working", None) => "正在处理".to_string(),
                ("waiting", None) => "等待批准".to_string(),
                ("recent", None) => "最近有活动".to_string(),
                ("blocked", None) => "运行异常".to_string(),
                _ => "当前空闲".to_string(),
            };
            let elapsed_minutes = ((captured_at_ms - updated_at_ms).max(0) / 60_000).min(999);
            let (x, y) = positions[index % positions.len()];
            Some(CodexAgent {
                id,
                name,
                provider: source.label,
                task,
                detail,
                status: status.to_string(),
                elapsed_minutes,
                accent: accents[index % accents.len()],
                position: CodexPosition { x, y },
                capabilities: Vec::new(),
                control_mode: "observed",
                updated_at_ms,
            })
        })
        .collect();

    Ok(CodexSnapshot {
        connection_state: "connected",
        adapter_mode: "app-server-observer",
        agents,
        last_synced_at_ms: Some(captured_at_ms),
        message: "Codex app-server metadata connected".to_string(),
    })
}

struct SourceLabel {
    label: String,
    is_subagent: bool,
}

fn source_label(source: Option<&Value>) -> SourceLabel {
    if let Some(source) = source.and_then(Value::as_str) {
        let label = match source {
            "vscode" => "Codex Desktop",
            "cli" => "Codex CLI",
            "exec" => "Codex Exec",
            "appServer" => "Codex App Server",
            _ => "Codex",
        };
        return SourceLabel {
            label: label.to_string(),
            is_subagent: false,
        };
    }
    if source.is_some_and(|source| source.get("subAgent").is_some()) {
        return SourceLabel {
            label: "Codex Subagent".to_string(),
            is_subagent: true,
        };
    }
    SourceLabel {
        label: "Codex".to_string(),
        is_subagent: false,
    }
}

fn workspace_name(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

pub(crate) fn find_codex_executable() -> Option<PathBuf> {
    if let Some(path) = env::var_os("AGENT_BAR_CODEX_PATH").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }

    #[cfg(windows)]
    {
        let bin_root = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)?
            .join("OpenAI")
            .join("Codex")
            .join("bin");
        let mut candidates = fs::read_dir(bin_root)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("codex.exe"))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        candidates.sort_by_key(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH)
        });
        candidates.pop()
    }

    #[cfg(not(windows))]
    {
        Some(PathBuf::from("codex"))
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_recent_observed_threads_without_exposing_preview_text() {
        let now = 1_800_000_000_000_i64;
        let response = json!({
            "id": 2,
            "result": {
                "data": [{
                    "id": "thread-1",
                    "name": "Agent Bar",
                    "preview": "private prompt content",
                    "updatedAt": (now / 1000) - 30,
                    "cwd": "C:/work/agent-bar",
                    "source": "vscode",
                    "status": { "type": "notLoaded" }
                }]
            }
        });
        let snapshot = parse_thread_list(&response, now).expect("snapshot");
        assert_eq!(snapshot.connection_state, "connected");
        assert_eq!(snapshot.agents[0].status, "recent");
        assert_eq!(snapshot.agents[0].task, "Agent Bar");
        assert!(!snapshot.agents[0].detail.contains("private"));
        assert_eq!(snapshot.agents[0].control_mode, "observed");
    }

    #[test]
    fn maps_waiting_runtime_status() {
        let response = json!({
            "result": {
                "data": [{
                    "id": "thread-2",
                    "name": null,
                    "updatedAt": 1,
                    "source": { "subAgent": { "thread_spawn": {} } },
                    "status": {
                        "type": "active",
                        "activeFlags": ["waitingOnApproval"]
                    }
                }]
            }
        });
        let snapshot = parse_thread_list(&response, 2_000).expect("snapshot");
        assert_eq!(snapshot.agents[0].status, "waiting");
        assert_eq!(snapshot.agents[0].provider, "Codex Subagent");
        assert_eq!(snapshot.agents[0].name, "Codex 子 Agent");
    }

    #[test]
    fn hook_events_override_recent_inference_with_exact_activity() {
        let response = json!({
            "result": {
                "data": [{
                    "id": "session-1",
                    "name": "Agent Bar",
                    "updatedAt": 1,
                    "source": "vscode",
                    "status": { "type": "notLoaded" }
                }]
            }
        });
        let snapshot = parse_thread_list(&response, 10_000_000).expect("snapshot");
        let event = AgentEventRecord {
            id: "event-1".into(),
            provider: "codex".into(),
            session_id: "session-1".into(),
            turn_id: Some("turn-1".into()),
            agent_id: None,
            event_name: "PermissionRequest".into(),
            activity_kind: "waiting".into(),
            occurred_at_ms: now_ms(),
            metadata_json: json!({ "toolName": "Bash" }).to_string(),
        };

        let snapshot = apply_hook_events(snapshot, &[event]);
        assert_eq!(snapshot.agents[0].status, "waiting");
        assert_eq!(snapshot.agents[0].detail, "等待 Codex 中的批准");
    }

    #[test]
    #[ignore = "requires an installed and authenticated Codex desktop CLI"]
    fn reads_live_codex_task_metadata() {
        let snapshot = poll_codex_metadata().expect("live Codex snapshot");
        assert_eq!(snapshot.connection_state, "connected");
        assert!(!snapshot.agents.is_empty());
    }
}
