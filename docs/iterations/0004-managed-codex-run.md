# Iteration 0004: Managed Codex Run

状态：Implemented and verified as a first controlled vertical slice

## 目标

让 Agent Bar 自己持有 Codex App Server 长连接，并在用户明确提交任务后显示真实运行状态、工具审批和终止控制。

## 本轮完成

- 新增 Rust 长生命周期 JSONL 客户端、请求响应分发器和事件读取线程。
- 按需启动桌面版内置 `codex app-server`，完成 `initialize/initialized` 握手。
- 用户从 Agent 面板提交任务后，依次调用 `thread/start` 和 `turn/start`。
- thread 使用临时历史、`workspace-write` 沙箱、`on-request` 审批和用户 reviewer。
- 将 turn、command、file change、tool、web search 和 subagent 事件映射为像素办公室状态。
- 将命令执行和文件修改 server request 投影为 Agent Bar 审批条。
- 支持批准、拒绝和 `turn/interrupt` 终止。
- 托管会话与旁路观察会话并列展示；同一 thread 由托管会话优先提供控制。

## 隐私边界

- 不自动启动模型；只有表单提交才会发送 `turn/start`。
- 不读取或保存 agent message、reasoning、tool input、tool result 和聚合命令输出。
- 用户输入的任务标题和审批命令只保存在进程内存，不写入 SQLite。
- 审批只覆盖当前协议已验证的命令执行和文件修改请求。

## 验证证据

- `npm run build` 通过 TypeScript 和 Vite 生产构建。
- `cargo test --lib` 通过 11 项测试；2 项真实 Codex 测试默认忽略。
- 显式运行无模型 live test，通过 `initialize` 和 `thread/start` 创建临时 thread，未调用 `turn/start`。
- 状态映射测试确认 reasoning 和 agent message 不进入可视状态内容。

## 当前限制

- 暂停、恢复、调整优先级和跨 Agent 移交尚未接入协议。
- `requestUserInput`、MCP elicitation 和权限配置升级不是简单二元审批，当前不在审批 UI 中处理。
- App Server 异常退出会把运行标记为阻塞；自动重连和运行恢复仍需补齐。
- 尚未通过桌面 UI 自动提交真实模型任务，避免在验证阶段未经用户确认产生模型调用。

## 下一步

- 增加连接退出监控、请求超时恢复和审批去重集成测试。
- 抽出通用 `AgentAdapter` 与 `AgentEvent` 契约，准备 Claude Code 适配器。
- 开始 Windows 置顶状态条、多显示器 DPI 和点击穿透技术验证。
