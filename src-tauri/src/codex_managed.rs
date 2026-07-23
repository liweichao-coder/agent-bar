use crate::codex::find_codex_executable;
use crate::screenshot_import::{
    parse_screenshot_result, stage_screenshot, ScreenshotImportRuntime, ScreenshotImportSnapshot,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_PROMPT_BYTES: usize = 8_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedCodexRun {
    thread_id: String,
    turn_id: Option<String>,
    name: String,
    provider: &'static str,
    task: String,
    detail: String,
    status: String,
    elapsed_minutes: i64,
    accent: &'static str,
    position: ManagedPosition,
    capabilities: Vec<&'static str>,
    control_mode: &'static str,
    started_at_ms: i64,
    updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedPosition {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedCodexApproval {
    id: String,
    agent_id: String,
    kind: &'static str,
    title: String,
    detail: String,
    risk: &'static str,
    #[serde(skip_serializing)]
    request_id: Value,
    #[serde(skip_serializing)]
    method: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedCodexSnapshot {
    connection_state: String,
    runs: Vec<ManagedCodexRun>,
    approvals: Vec<ManagedCodexApproval>,
    last_error: Option<String>,
    screenshot_import: ScreenshotImportSnapshot,
}

struct RuntimeState {
    connection_state: String,
    runs: Vec<ManagedCodexRun>,
    approvals: Vec<ManagedCodexApproval>,
    last_error: Option<String>,
    screenshot_import: ScreenshotImportRuntime,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            connection_state: "disconnected".to_string(),
            runs: Vec::new(),
            approvals: Vec::new(),
            last_error: None,
            screenshot_import: ScreenshotImportRuntime::new(),
        }
    }

    fn snapshot(&self) -> ManagedCodexSnapshot {
        let now = now_ms();
        let mut runs = self.runs.clone();
        for run in &mut runs {
            run.elapsed_minutes = ((now - run.started_at_ms).max(0) / 60_000).min(999);
        }
        ManagedCodexSnapshot {
            connection_state: self.connection_state.clone(),
            runs,
            approvals: self.approvals.clone(),
            last_error: self.last_error.clone(),
            screenshot_import: self.screenshot_import.snapshot.clone(),
        }
    }
}

struct Transport {
    child: Child,
    stdin: ChildStdin,
}

pub struct ManagedCodex {
    transport: Arc<Mutex<Option<Transport>>>,
    runtime: Arc<Mutex<RuntimeState>>,
    pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Value>>>>,
    next_request_id: AtomicI64,
    next_approval_id: Arc<AtomicU64>,
    next_screenshot_id: AtomicU64,
    connect_guard: Mutex<()>,
}

impl ManagedCodex {
    pub fn new() -> Self {
        Self {
            transport: Arc::new(Mutex::new(None)),
            runtime: Arc::new(Mutex::new(RuntimeState::new())),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_request_id: AtomicI64::new(1),
            next_approval_id: Arc::new(AtomicU64::new(1)),
            next_screenshot_id: AtomicU64::new(1),
            connect_guard: Mutex::new(()),
        }
    }

    pub fn snapshot(&self) -> Result<ManagedCodexSnapshot, String> {
        self.runtime
            .lock()
            .map(|state| state.snapshot())
            .map_err(|_| "managed Codex state lock was poisoned".to_string())
    }

    pub fn start_run(
        &self,
        prompt: String,
        cwd: Option<String>,
    ) -> Result<ManagedCodexSnapshot, String> {
        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            return Err("task cannot be empty".to_string());
        }
        if prompt.len() > MAX_PROMPT_BYTES {
            return Err(format!("task exceeds {MAX_PROMPT_BYTES} bytes"));
        }
        let workspace = validate_workspace(cwd)?;
        self.ensure_connected()?;

        let thread_response = self.request(
            "thread/start",
            json!({
                "cwd": workspace,
                "approvalPolicy": "on-request",
                "approvalsReviewer": "user",
                "sandbox": "workspace-write",
                "ephemeral": true
            }),
        )?;
        let thread_id = thread_response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| "Codex thread/start returned no thread id".to_string())?
            .to_string();
        let now = now_ms();
        let task = truncate_chars(&prompt, 72);
        let index = self
            .runtime
            .lock()
            .map_err(|_| "managed Codex state lock was poisoned".to_string())?
            .runs
            .len();
        let run = ManagedCodexRun {
            thread_id: thread_id.clone(),
            turn_id: None,
            name: format!("Codex {}", index + 1),
            provider: "Codex App Server",
            task,
            detail: "线程已创建，正在启动任务".to_string(),
            status: "working".to_string(),
            elapsed_minutes: 0,
            accent: managed_accent(index),
            position: ManagedPosition {
                x: 24 + ((index as i32 * 23) % 58),
                y: 42 + ((index as i32 * 17) % 28),
            },
            capabilities: vec!["stop", "approve"],
            control_mode: "managed",
            started_at_ms: now,
            updated_at_ms: now,
        };
        self.runtime
            .lock()
            .map_err(|_| "managed Codex state lock was poisoned".to_string())?
            .runs
            .push(run);

        match self.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{ "type": "text", "text": prompt }]
            }),
        ) {
            Ok(turn_response) => {
                let turn_id = turn_response
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                self.update_run(&thread_id, |run| {
                    run.turn_id = turn_id;
                    run.detail = "Codex 正在处理任务".to_string();
                    run.status = "working".to_string();
                })?;
            }
            Err(error) => {
                self.update_run(&thread_id, |run| {
                    run.detail = format!("任务启动失败：{}", truncate_chars(&error, 120));
                    run.status = "blocked".to_string();
                    run.capabilities.clear();
                })?;
                return Err(error);
            }
        }
        self.snapshot()
    }

    pub fn start_screenshot_import(
        &self,
        file_name: String,
        mime_type: String,
        base64_data: String,
        cache_dir: &Path,
    ) -> Result<ManagedCodexSnapshot, String> {
        {
            let mut state = self
                .runtime
                .lock()
                .map_err(|_| "managed Codex state lock was poisoned".to_string())?;
            if state.screenshot_import.snapshot.status == "analyzing" {
                return Err("已有截图正在分析".to_string());
            }
            state.screenshot_import.clear();
        }

        let workspace = validate_workspace(None)?;
        let sequence = self.next_screenshot_id.fetch_add(1, Ordering::Relaxed);
        let staged = stage_screenshot(cache_dir, &file_name, &mime_type, &base64_data, sequence)?;
        if let Err(error) = self.ensure_connected() {
            let _ = std::fs::remove_file(&staged.path);
            return Err(error);
        }

        let thread_response = self.request(
            "thread/start",
            json!({
                "cwd": workspace,
                "approvalPolicy": "never",
                "approvalsReviewer": "user",
                "sandbox": "read-only",
                "ephemeral": true,
                "dynamicTools": [screenshot_import_tool()]
            }),
        );
        let thread_response = match thread_response {
            Ok(response) => response,
            Err(error) => {
                let _ = std::fs::remove_file(&staged.path);
                return Err(error);
            }
        };
        let thread_id = match thread_response
            .pointer("/thread/id")
            .and_then(Value::as_str)
        {
            Some(thread_id) => thread_id.to_string(),
            None => {
                let _ = std::fs::remove_file(&staged.path);
                return Err("Codex thread/start returned no thread id".to_string());
            }
        };
        let path = staged.path.to_string_lossy().to_string();
        let now = now_ms();
        {
            let mut state = self
                .runtime
                .lock()
                .map_err(|_| "managed Codex state lock was poisoned".to_string())?;
            state.screenshot_import.snapshot = ScreenshotImportSnapshot {
                status: "analyzing".to_string(),
                file_name: Some(staged.display_name.clone()),
                tasks: Vec::new(),
                warnings: Vec::new(),
                error: None,
            };
            state.screenshot_import.thread_id = Some(thread_id.clone());
            state.screenshot_import.temp_path = Some(staged.path);
            let index = state.runs.len();
            state.runs.push(ManagedCodexRun {
                thread_id: thread_id.clone(),
                turn_id: None,
                name: format!("Codex {}", index + 1),
                provider: "Codex App Server",
                task: "从截图提取日程事项".to_string(),
                detail: "正在识别截图中的时间与任务".to_string(),
                status: "working".to_string(),
                elapsed_minutes: 0,
                accent: managed_accent(index),
                position: ManagedPosition {
                    x: 24 + ((index as i32 * 23) % 58),
                    y: 42 + ((index as i32 * 17) % 28),
                },
                capabilities: vec!["stop"],
                control_mode: "managed",
                started_at_ms: now,
                updated_at_ms: now,
            });
        }

        let prompt = screenshot_import_prompt();
        match self.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [
                    { "type": "text", "text": prompt, "text_elements": [] },
                    { "type": "localImage", "path": path, "detail": "original" }
                ]
            }),
        ) {
            Ok(turn_response) => {
                let turn_id = turn_response
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                self.update_run(&thread_id, |run| run.turn_id = turn_id)?;
            }
            Err(error) => {
                self.fail_screenshot_import(&thread_id, &error);
                return Err(error);
            }
        }
        self.snapshot()
    }

    pub fn dismiss_screenshot_import(&self) -> Result<ManagedCodexSnapshot, String> {
        let mut state = self
            .runtime
            .lock()
            .map_err(|_| "managed Codex state lock was poisoned".to_string())?;
        if state.screenshot_import.snapshot.status == "analyzing" {
            return Err("截图仍在分析，请先终止对应 Codex 任务".to_string());
        }
        state.screenshot_import.clear();
        drop(state);
        self.snapshot()
    }

    pub fn cancel_screenshot_import(&self) -> Result<ManagedCodexSnapshot, String> {
        let thread_id = {
            let state = self
                .runtime
                .lock()
                .map_err(|_| "managed Codex state lock was poisoned".to_string())?;
            if state.screenshot_import.snapshot.status != "analyzing" {
                drop(state);
                return self.dismiss_screenshot_import();
            }
            state
                .screenshot_import
                .thread_id
                .clone()
                .ok_or_else(|| "截图分析任务缺少 Codex thread id".to_string())?
        };
        self.interrupt(&thread_id)
    }

    pub fn interrupt(&self, thread_id: &str) -> Result<ManagedCodexSnapshot, String> {
        let turn_id = self
            .runtime
            .lock()
            .map_err(|_| "managed Codex state lock was poisoned".to_string())?
            .runs
            .iter()
            .find(|run| run.thread_id == thread_id)
            .and_then(|run| run.turn_id.clone())
            .ok_or_else(|| "managed run has no active turn".to_string())?;
        self.request(
            "turn/interrupt",
            json!({ "threadId": thread_id, "turnId": turn_id }),
        )?;
        self.update_run(thread_id, |run| {
            run.status = "waiting".to_string();
            run.detail = "正在终止 Codex 任务".to_string();
        })?;
        if let Ok(mut state) = self.runtime.lock() {
            if state.screenshot_import.thread_id.as_deref() == Some(thread_id) {
                state.screenshot_import.cleanup_image();
                state.screenshot_import.snapshot.status = "error".to_string();
                state.screenshot_import.snapshot.error = Some("截图分析已终止".to_string());
            }
        }
        self.snapshot()
    }

    pub fn resolve_approval(
        &self,
        approval_id: &str,
        approved: bool,
    ) -> Result<ManagedCodexSnapshot, String> {
        let approval = self
            .runtime
            .lock()
            .map_err(|_| "managed Codex state lock was poisoned".to_string())?
            .approvals
            .iter()
            .find(|approval| approval.id == approval_id)
            .cloned()
            .ok_or_else(|| "approval request no longer exists".to_string())?;
        let decision = if approved { "accept" } else { "decline" };
        match approval.method.as_str() {
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                self.write_message(json!({
                    "id": approval.request_id,
                    "result": { "decision": decision }
                }))?;
            }
            _ => return Err("unsupported approval request type".to_string()),
        }
        if let Ok(mut state) = self.runtime.lock() {
            state.approvals.retain(|item| item.id != approval_id);
            if let Some(run) = state
                .runs
                .iter_mut()
                .find(|run| run.thread_id == approval.agent_id)
            {
                run.status = if approved { "working" } else { "blocked" }.to_string();
                run.detail = if approved {
                    "操作已批准，Codex 继续工作"
                } else {
                    "操作已拒绝，等待 Codex 调整"
                }
                .to_string();
                run.updated_at_ms = now_ms();
            }
        }
        self.snapshot()
    }

    fn ensure_connected(&self) -> Result<(), String> {
        let _guard = self
            .connect_guard
            .lock()
            .map_err(|_| "managed Codex connection lock was poisoned".to_string())?;
        if self
            .transport
            .lock()
            .map_err(|_| "managed Codex transport lock was poisoned".to_string())?
            .is_some()
        {
            return Ok(());
        }
        let executable = find_codex_executable()
            .ok_or_else(|| "could not find the desktop-bundled Codex executable".to_string())?;
        let mut child = Command::new(executable)
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start Codex App Server: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Codex App Server stdin is unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Codex App Server stdout is unavailable".to_string())?;
        *self
            .transport
            .lock()
            .map_err(|_| "managed Codex transport lock was poisoned".to_string())? =
            Some(Transport { child, stdin });
        self.start_reader(stdout)?;
        let initialized = self
            .request(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "agent_bar",
                        "title": "Agent Bar",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            )
            .and_then(|_| self.write_message(json!({ "method": "initialized" })));
        if let Err(error) = initialized {
            self.stop_transport();
            if let Ok(mut state) = self.runtime.lock() {
                state.connection_state = "error".to_string();
                state.last_error = Some(error.clone());
            }
            return Err(error);
        }
        if let Ok(mut state) = self.runtime.lock() {
            state.connection_state = "connected".to_string();
            state.last_error = None;
        }
        Ok(())
    }

    fn start_reader(&self, stdout: std::process::ChildStdout) -> Result<(), String> {
        let pending = self.pending.clone();
        let runtime = self.runtime.clone();
        let transport = self.transport.clone();
        let next_approval_id = self.next_approval_id.clone();
        std::thread::Builder::new()
            .name("codex-managed-reader".to_string())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    let Ok(message) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    if message.get("method").is_some() {
                        let is_dynamic_tool = message
                            .get("method")
                            .and_then(Value::as_str)
                            .is_some_and(|method| method == "item/tool/call")
                            && message.get("id").is_some();
                        if is_dynamic_tool {
                            let response = handle_dynamic_tool_request(&runtime, &message);
                            let _ = write_shared_message(&transport, response);
                        } else {
                            handle_server_message(&runtime, &next_approval_id, message);
                        }
                    } else if let Some(id) = message.get("id").and_then(Value::as_i64) {
                        if let Ok(mut callbacks) = pending.lock() {
                            if let Some(callback) = callbacks.remove(&id) {
                                let _ = callback.send(message);
                            }
                        }
                    }
                }
                if let Ok(mut state) = runtime.lock() {
                    state.screenshot_import.cleanup_image();
                    if state.screenshot_import.snapshot.status == "analyzing" {
                        state.screenshot_import.snapshot.status = "error".to_string();
                        state.screenshot_import.snapshot.error =
                            Some("Codex App Server 已断开".to_string());
                    }
                    state.connection_state = "disconnected".to_string();
                    state.last_error = Some("Codex App Server disconnected".to_string());
                    state.approvals.clear();
                    for run in &mut state.runs {
                        if !["idle", "blocked"].contains(&run.status.as_str()) {
                            run.status = "blocked".to_string();
                            run.detail = "Codex App Server 已断开".to_string();
                            run.capabilities.clear();
                        }
                    }
                }
                if let Ok(mut callbacks) = pending.lock() {
                    for (_, callback) in callbacks.drain() {
                        let _ = callback.send(json!({
                            "error": { "message": "Codex App Server disconnected" }
                        }));
                    }
                }
                if let Ok(mut slot) = transport.lock() {
                    if let Some(mut disconnected) = slot.take() {
                        let _ = disconnected.child.wait();
                    }
                }
            })
            .map_err(|error| format!("could not start Codex event reader: {error}"))?;
        Ok(())
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| "managed Codex response lock was poisoned".to_string())?
            .insert(id, sender);
        if let Err(error) = self.write_message(json!({
            "method": method,
            "id": id,
            "params": params
        })) {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            return Err(error);
        }
        let response = receiver.recv_timeout(RESPONSE_TIMEOUT).map_err(|error| {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            format!("Codex {method} timed out: {error}")
        })?;
        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown App Server error");
            return Err(format!("Codex {method} failed: {message}"));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    fn write_message(&self, message: Value) -> Result<(), String> {
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| "managed Codex transport lock was poisoned".to_string())?;
        let transport = transport
            .as_mut()
            .ok_or_else(|| "Codex App Server is not connected".to_string())?;
        serde_json::to_writer(&mut transport.stdin, &message)
            .map_err(|error| format!("could not encode Codex request: {error}"))?;
        transport
            .stdin
            .write_all(b"\n")
            .and_then(|_| transport.stdin.flush())
            .map_err(|error| format!("could not write Codex request: {error}"))
    }

    fn update_run(
        &self,
        thread_id: &str,
        update: impl FnOnce(&mut ManagedCodexRun),
    ) -> Result<(), String> {
        let mut state = self
            .runtime
            .lock()
            .map_err(|_| "managed Codex state lock was poisoned".to_string())?;
        let run = state
            .runs
            .iter_mut()
            .find(|run| run.thread_id == thread_id)
            .ok_or_else(|| "managed Codex run was not found".to_string())?;
        update(run);
        run.updated_at_ms = now_ms();
        Ok(())
    }

    fn fail_screenshot_import(&self, thread_id: &str, error: &str) {
        if let Ok(mut state) = self.runtime.lock() {
            if state.screenshot_import.thread_id.as_deref() == Some(thread_id) {
                state.screenshot_import.cleanup_image();
                state.screenshot_import.snapshot.status = "error".to_string();
                state.screenshot_import.snapshot.error = Some(truncate_chars(error, 180));
            }
            if let Some(run) = state.runs.iter_mut().find(|run| run.thread_id == thread_id) {
                run.status = "blocked".to_string();
                run.detail = truncate_chars(error, 120);
                run.capabilities.clear();
                run.updated_at_ms = now_ms();
            }
        }
    }

    fn stop_transport(&self) {
        if let Ok(mut slot) = self.transport.lock() {
            if let Some(mut transport) = slot.take() {
                let _ = transport.child.kill();
                let _ = transport.child.wait();
            }
        }
    }
}

impl Drop for ManagedCodex {
    fn drop(&mut self) {
        self.stop_transport();
    }
}

fn handle_server_message(
    runtime: &Arc<Mutex<RuntimeState>>,
    next_approval_id: &AtomicU64,
    message: Value,
) {
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    if message.get("id").is_some() {
        handle_server_request(runtime, next_approval_id, method, &message);
        return;
    }
    let params = message.get("params").unwrap_or(&Value::Null);
    let thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let Ok(mut state) = runtime.lock() else {
        return;
    };
    if method == "serverRequest/resolved" {
        if let Some(request_id) = params.get("requestId") {
            state
                .approvals
                .retain(|approval| approval.request_id != *request_id);
        }
        return;
    }
    let Some(run_index) = state.runs.iter().position(|run| run.thread_id == thread_id) else {
        return;
    };
    if method == "turn/completed" {
        let status = params
            .pointer("/turn/status")
            .and_then(Value::as_str)
            .unwrap_or("failed");
        if state.screenshot_import.thread_id.as_deref() == Some(thread_id) {
            state.screenshot_import.cleanup_image();
            if state.screenshot_import.snapshot.status == "analyzing" {
                state.screenshot_import.snapshot.status = "error".to_string();
                state.screenshot_import.snapshot.error = Some(
                    match status {
                        "interrupted" => "截图分析已终止",
                        "failed" => "Codex 未能完成截图分析",
                        _ => "Codex 未返回结构化事项",
                    }
                    .to_string(),
                );
            }
            state.screenshot_import.thread_id = None;
        }
        let run = &mut state.runs[run_index];
        run.status = if status == "failed" {
            "blocked"
        } else {
            "idle"
        }
        .to_string();
        run.detail = match status {
            "completed" => "任务已完成",
            "interrupted" => "任务已终止",
            _ => "任务执行失败",
        }
        .to_string();
        run.capabilities.clear();
        run.updated_at_ms = now_ms();
        state
            .approvals
            .retain(|approval| approval.agent_id != thread_id);
        return;
    }
    if method == "error" {
        let message = params
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("Codex reported an error");
        let detail = truncate_chars(message, 120);
        if state.screenshot_import.thread_id.as_deref() == Some(thread_id) {
            state.screenshot_import.cleanup_image();
            state.screenshot_import.snapshot.status = "error".to_string();
            state.screenshot_import.snapshot.error = Some(detail.clone());
        }
        let run = &mut state.runs[run_index];
        run.status = "blocked".to_string();
        run.detail = detail.clone();
        run.updated_at_ms = now_ms();
        state.last_error = Some(detail);
        return;
    }
    let run = &mut state.runs[run_index];
    match method {
        "turn/started" => {
            run.turn_id = params
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .map(str::to_string);
            run.status = "working".to_string();
            run.detail = "Codex 正在处理任务".to_string();
        }
        "item/started" => {
            let item_type = params
                .pointer("/item/type")
                .and_then(Value::as_str)
                .unwrap_or("");
            let (status, detail) = item_activity(item_type, false);
            if let Some(status) = status {
                run.status = status.to_string();
                run.detail = detail.to_string();
            }
        }
        "item/completed" => {
            let item_type = params
                .pointer("/item/type")
                .and_then(Value::as_str)
                .unwrap_or("");
            let (_, detail) = item_activity(item_type, true);
            if !detail.is_empty() {
                run.status = "working".to_string();
                run.detail = detail.to_string();
            }
        }
        _ => return,
    }
    run.updated_at_ms = now_ms();
}

fn handle_dynamic_tool_request(runtime: &Arc<Mutex<RuntimeState>>, message: &Value) -> Value {
    let request_id = message.get("id").cloned().unwrap_or(Value::Null);
    let params = message.get("params").unwrap_or(&Value::Null);
    let thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let tool = params.get("tool").and_then(Value::as_str).unwrap_or("");
    let arguments = params.get("arguments").unwrap_or(&Value::Null);

    if tool != "submit_extracted_tasks" {
        return dynamic_tool_response(request_id, false, "Agent Bar 不支持该动态工具");
    }
    let Ok(mut state) = runtime.lock() else {
        return dynamic_tool_response(request_id, false, "Agent Bar 状态不可用");
    };
    if state.screenshot_import.thread_id.as_deref() != Some(thread_id)
        || state.screenshot_import.snapshot.status != "analyzing"
    {
        return dynamic_tool_response(request_id, false, "截图导入任务已失效");
    }

    match parse_screenshot_result(arguments, now_ms() as u64) {
        Ok((tasks, warnings)) => {
            let task_count = tasks.len();
            state.screenshot_import.cleanup_image();
            state.screenshot_import.snapshot.status = "ready".to_string();
            state.screenshot_import.snapshot.tasks = tasks;
            state.screenshot_import.snapshot.warnings = warnings;
            state.screenshot_import.snapshot.error = None;
            if let Some(run) = state.runs.iter_mut().find(|run| run.thread_id == thread_id) {
                run.status = "waiting".to_string();
                run.detail = format!("已提取 {task_count} 项，等待确认");
                run.capabilities.clear();
                run.updated_at_ms = now_ms();
            }
            dynamic_tool_response(request_id, true, "结构化事项已接收，请结束本轮")
        }
        Err(error) => {
            state.screenshot_import.cleanup_image();
            state.screenshot_import.snapshot.status = "error".to_string();
            state.screenshot_import.snapshot.error = Some(error.clone());
            if let Some(run) = state.runs.iter_mut().find(|run| run.thread_id == thread_id) {
                run.status = "blocked".to_string();
                run.detail = format!("截图结果无效：{}", truncate_chars(&error, 100));
                run.capabilities.clear();
                run.updated_at_ms = now_ms();
            }
            dynamic_tool_response(request_id, false, &error)
        }
    }
}

fn dynamic_tool_response(request_id: Value, success: bool, text: &str) -> Value {
    json!({
        "id": request_id,
        "result": {
            "contentItems": [{ "type": "inputText", "text": truncate_chars(text, 200) }],
            "success": success
        }
    })
}

fn write_shared_message(
    transport: &Arc<Mutex<Option<Transport>>>,
    message: Value,
) -> Result<(), String> {
    let mut transport = transport
        .lock()
        .map_err(|_| "managed Codex transport lock was poisoned".to_string())?;
    let transport = transport
        .as_mut()
        .ok_or_else(|| "Codex App Server is not connected".to_string())?;
    serde_json::to_writer(&mut transport.stdin, &message)
        .map_err(|error| format!("could not encode Codex response: {error}"))?;
    transport
        .stdin
        .write_all(b"\n")
        .and_then(|_| transport.stdin.flush())
        .map_err(|error| format!("could not write Codex response: {error}"))
}

fn handle_server_request(
    runtime: &Arc<Mutex<RuntimeState>>,
    next_approval_id: &AtomicU64,
    method: &str,
    message: &Value,
) {
    if !matches!(
        method,
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
    ) {
        return;
    }
    let params = message.get("params").unwrap_or(&Value::Null);
    let Some(thread_id) = params.get("threadId").and_then(Value::as_str) else {
        return;
    };
    let Some(request_id) = message.get("id").cloned() else {
        return;
    };
    let sequence = next_approval_id.fetch_add(1, Ordering::Relaxed);
    let (title, detail, risk) = if method == "item/commandExecution/requestApproval" {
        let command = params
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("Codex 请求执行命令");
        (
            "Codex 请求执行命令".to_string(),
            truncate_chars(command, 180),
            "high",
        )
    } else {
        let reason = params
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("Codex 请求修改工作区文件");
        (
            "Codex 请求修改文件".to_string(),
            truncate_chars(reason, 180),
            "medium",
        )
    };
    let approval = ManagedCodexApproval {
        id: format!("codex-approval-{sequence}"),
        agent_id: thread_id.to_string(),
        kind: "agent-tool",
        title,
        detail,
        risk,
        request_id,
        method: method.to_string(),
    };
    if let Ok(mut state) = runtime.lock() {
        if state
            .approvals
            .iter()
            .any(|existing| existing.request_id == approval.request_id)
        {
            return;
        }
        state.approvals.push(approval);
        if let Some(run) = state.runs.iter_mut().find(|run| run.thread_id == thread_id) {
            run.status = "waiting".to_string();
            run.detail = "等待你批准 Codex 工具调用".to_string();
            run.updated_at_ms = now_ms();
        }
    }
}

fn item_activity(item_type: &str, completed: bool) -> (Option<&'static str>, &'static str) {
    match (item_type, completed) {
        ("commandExecution", false) => (Some("working"), "正在执行已批准的命令"),
        ("commandExecution", true) => (Some("working"), "命令执行完成"),
        ("fileChange", false) => (Some("working"), "正在修改工作区文件"),
        ("fileChange", true) => (Some("working"), "文件修改完成"),
        ("mcpToolCall" | "dynamicToolCall" | "webSearch", false) => {
            (Some("searching"), "正在调用工具")
        }
        ("mcpToolCall" | "dynamicToolCall" | "webSearch", true) => {
            (Some("working"), "工具调用完成")
        }
        ("collabAgentToolCall" | "subAgentActivity", false) => {
            (Some("working"), "正在协调子 Agent")
        }
        ("collabAgentToolCall" | "subAgentActivity", true) => {
            (Some("working"), "子 Agent 活动已更新")
        }
        _ => (None, ""),
    }
}

fn screenshot_import_tool() -> Value {
    json!({
        "type": "function",
        "name": "submit_extracted_tasks",
        "description": "Return only actionable schedule or task items visible in the supplied screenshot.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "maxItems": 30,
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "minLength": 1, "maxLength": 120 },
                            "durationMinutes": {
                                "type": "integer",
                                "minimum": 15,
                                "maximum": 480,
                                "multipleOf": 15
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["critical", "high", "normal", "low"]
                            },
                            "preferredPeriod": {
                                "type": "string",
                                "enum": ["any", "morning", "afternoon", "evening"]
                            },
                            "category": {
                                "type": "string",
                                "enum": ["focus", "meeting", "admin", "life"]
                            },
                            "notes": { "type": "string", "maxLength": 300 }
                        },
                        "required": [
                            "title", "durationMinutes", "priority", "preferredPeriod", "category"
                        ],
                        "additionalProperties": false
                    }
                },
                "warnings": {
                    "type": "array",
                    "maxItems": 10,
                    "items": { "type": "string", "maxLength": 200 }
                }
            },
            "required": ["tasks"],
            "additionalProperties": false
        }
    })
}

fn screenshot_import_prompt() -> &'static str {
    "分析用户主动提供的聊天或日程截图，只提取截图中明确可行动的事项、会议、截止日期或行程。不要执行命令、搜索网络、读取其他文件或复述聊天全文。估算时长时使用 15 分钟倍数；不确定的日期、时间或责任人写入简短 notes 或 warnings，不要编造。必须且只能调用 submit_extracted_tasks 一次；即使没有事项，也以空 tasks 数组调用。"
}

fn validate_workspace(cwd: Option<String>) -> Result<PathBuf, String> {
    let path = match cwd {
        Some(cwd) if !cwd.trim().is_empty() => PathBuf::from(cwd),
        _ => Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .to_path_buf(),
    };
    if !path.is_absolute() {
        return Err("workspace path must be absolute".to_string());
    }
    if !path.is_dir() {
        return Err("workspace path does not exist or is not a directory".to_string());
    }
    path.canonicalize()
        .map_err(|error| format!("could not resolve workspace path: {error}"))
}

fn managed_accent(index: usize) -> &'static str {
    ["#72d6a5", "#79baff", "#e7c66d", "#ef9d89"][index % 4]
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{result}...")
    } else {
        result
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
    use super::{
        handle_dynamic_tool_request, item_activity, truncate_chars, validate_workspace,
        ManagedCodex, RuntimeState, MAX_PROMPT_BYTES,
    };
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[test]
    fn truncates_private_display_text_without_splitting_unicode() {
        assert_eq!(truncate_chars("测试任务内容", 4), "测试任务...");
        assert_eq!(truncate_chars("short", 10), "short");
    }

    #[test]
    fn maps_only_activity_types_not_message_content() {
        assert_eq!(item_activity("commandExecution", false).0, Some("working"));
        assert_eq!(item_activity("reasoning", false), (None, ""));
        assert_eq!(item_activity("agentMessage", false), (None, ""));
    }

    #[test]
    fn default_workspace_is_absolute_and_present() {
        let workspace = validate_workspace(None).expect("default workspace");
        assert!(workspace.is_absolute());
        assert!(workspace.is_dir());
        assert_eq!(MAX_PROMPT_BYTES, 8_000);
    }

    #[test]
    fn accepts_dynamic_screenshot_tool_output_for_the_active_thread() {
        let runtime = Arc::new(Mutex::new(RuntimeState::new()));
        {
            let mut state = runtime.lock().expect("runtime lock");
            state.screenshot_import.thread_id = Some("thread-1".to_string());
            state.screenshot_import.snapshot.status = "analyzing".to_string();
        }
        let response = handle_dynamic_tool_request(
            &runtime,
            &json!({
                "id": 7,
                "method": "item/tool/call",
                "params": {
                    "threadId": "thread-1",
                    "tool": "submit_extracted_tasks",
                    "arguments": {
                        "tasks": [{
                            "title": "确认报名窗口",
                            "durationMinutes": 30,
                            "priority": "critical",
                            "preferredPeriod": "morning",
                            "category": "admin"
                        }],
                        "warnings": []
                    }
                }
            }),
        );
        assert_eq!(response.pointer("/result/success"), Some(&json!(true)));
        let state = runtime.lock().expect("runtime lock");
        assert_eq!(state.screenshot_import.snapshot.status, "ready");
        assert_eq!(state.screenshot_import.snapshot.tasks.len(), 1);
        assert_eq!(
            state.screenshot_import.snapshot.tasks[0].title,
            "确认报名窗口"
        );
    }

    #[test]
    #[ignore = "requires an installed and authenticated Codex desktop CLI"]
    fn handshakes_and_starts_ephemeral_thread_without_starting_a_turn() {
        let managed = ManagedCodex::new();
        managed.ensure_connected().expect("initialize App Server");
        let workspace = validate_workspace(None).expect("default workspace");
        let response = managed
            .request(
                "thread/start",
                json!({
                    "cwd": workspace,
                    "approvalPolicy": "on-request",
                    "approvalsReviewer": "user",
                    "sandbox": "workspace-write",
                    "ephemeral": true
                }),
            )
            .expect("start ephemeral thread");
        assert!(response
            .pointer("/thread/id")
            .and_then(|id| id.as_str())
            .is_some());
    }
}
