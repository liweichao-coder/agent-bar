# Iteration 0002: Codex 只读任务观察器

状态：Implemented and verified

## 目标

把像素办公室从纯模拟 Agent 推进到真实 Codex 任务元数据，同时保持默认关闭、最小读取和能力诚实三个边界。

## 技术选择

Codex 官方把 App Server 定义为富客户端的深度集成接口，支持会话历史、审批和流式 Agent 事件。第一步使用 App Server 的 JSONL stdio 协议：

1. 短时启动桌面内置 `codex app-server`。
2. 发送 `initialize`，随后发送 `initialized`。
3. 只调用 `thread/list`，按 `updated_at` 读取最近任务。
4. 映射任务名称、来源、结构化状态和更新时间。
5. 获取响应后立即终止子进程。

参考：[Codex App Server](https://developers.openai.com/codex/app-server) 和 [Codex Hooks](https://developers.openai.com/codex/hooks)。

## 本轮完成

- 新增 Rust `CodexMonitor` 后台观察器和 Tauri 命令。
- 新增 SQLite 设置 `codex_observation_enabled`，数据库版本升级到 v3。
- Codex 观察默认关闭；原生界面显示独立批准请求。
- 授权后用真实 Codex 任务替换原生版模拟 Agent。
- 新增 `recent` 状态，区分旁路推断的最近活动和 App Server 明确报告的 `active`。
- 识别主任务与 Subagent 来源，在像素办公室中生成独立角色。
- 旁路任务的 `capabilities` 为空，因此界面不展示暂停、终止或移交按钮。
- 支持通过 `AGENT_BAR_CODEX_PATH` 显式指定 CLI；Windows 默认优先桌面内置 CLI。

## 隐私边界

- 不读取 `thread.preview`，即使 App Server 响应包含该字段。
- 不调用 `thread/read`，不加载 turn、item、消息正文或原始推理内容。
- 不创建 thread，不启动 turn，不产生模型调用。
- 只把完整工作路径缩减为末级目录名称后交给前端。
- 未授权时后台观察器不启动 Codex App Server。

## 状态准确性

独立启动的 App Server 可以读取持久化任务列表，但不会继承另一个 Codex Desktop 进程的内存订阅。因此：

- `active`、`idle`、`systemError` 等明确状态直接映射。
- 最近三分钟更新但返回 `notLoaded` 的任务映射为 `recent`，不标成 `working`。
- 下一轮通过 Hooks 获取已有会话的精确生命周期事件。
- 只有由 Agent Bar 托管的 App Server 会话才开放双向控制。

## 验证证据

- `npm run build`: React/TypeScript 生产构建通过。
- `cargo test`: 6 项本地测试通过，1 项依赖本机 Codex 的测试默认忽略。
- 显式运行 live test：成功连接桌面内置 Codex CLI，并读取到非空真实任务列表。
- live test 只执行初始化和 `thread/list`，未产生模型请求。
- `git diff --check`: 无空白错误。

## 环境发现

本机全局 npm Codex CLI 为较旧版本，且无法解析当前桌面配置中的 `service_tier` 值；Codex Desktop 自带更新的 CLI 并可正常运行 App Server。适配器因此不盲目调用 PATH 中的第一个 `codex`。

## 下一步

- 提供显式、可审阅的 Codex Hooks 安装流程，接收精确生命周期事件。
- 定义统一 `AgentEvent` 持久化表和事件到动画状态的映射。
- 由 Agent Bar 启动一个受托管 Codex thread，接入暂停、终止和批准请求。
- 增加断线退避、CLI 版本能力探测和数据保留策略。
