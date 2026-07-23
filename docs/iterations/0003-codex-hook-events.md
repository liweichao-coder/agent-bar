# Iteration 0003: Codex Hooks 精确事件

状态：Implemented and verified; installation remains opt-in

## 目标

用 Codex 生命周期 Hooks 补齐独立 App Server 无法精确观察外部已加载会话的问题，并把事件安全地落入 Agent Bar 的统一本地事实层。

## 本轮完成

- 新增 `integrations/codex/agent-bar-hook.mjs` 事件投影脚本。
- 新增覆盖 10 类官方生命周期事件的可审阅 `hooks.json` 模板。
- 新增 SQLite `agent_events` 表、时间索引、幂等写入和最近事件查询。
- 数据库版本升级到 v4。
- Rust 每次刷新时读取本地 JSONL、校验事件白名单并按事件 ID 去重。
- 精确 Hook 事件覆盖 App Server 的 `recent` 推断状态。
- `PermissionRequest` 映射为等待，搜索类工具映射为检索，`Stop` 映射为空闲。
- `SubagentStart/SubagentStop` 为子 Agent 创建独立像素角色。

## 最小数据投影

Hook 脚本只保存：

- 事件名和本地时间戳。
- session、turn、tool call 和 subagent 标识。
- 工具名、subagent 类型、启动来源和权限模式。

脚本明确忽略 prompt、tool input、tool result、assistant message、transcript 和 reasoning。Rust 再次执行事件白名单、长度限制和结构化元数据重建，避免直接信任日志内容。

## 行为边界

- 模板未自动复制到 `.codex/hooks.json`，不会在未确认时改变仓库的 Codex 生命周期。
- 观察脚本失败时保持 fail-open，不阻塞原始 Codex 工作。
- `Stop` 和 `SubagentStop` 返回 `continue: true`，不要求 Codex 追加额外工作。
- Hooks 只负责观察；批准决策仍留在原 Codex 客户端，直到 Agent Bar 托管 App Server 会话。

## 验证证据

- 隔离日志测试传入包含模拟 secret 的 prompt 和 tool input，输出 JSONL 不包含 secret。
- `cargo test`: 8 项通过，覆盖事件去重、时间排序和 Hook 精确状态覆盖；1 项 live Codex 测试默认忽略。
- `npm run build`: 生产构建通过。
- `git diff --check`: 无空白错误。

## 下一步

- 做 Hooks 显式安装、信任检查、卸载和健康状态界面。
- 让 Agent Bar 启动并持有一个 App Server 连接，接入 server request 审批和 `turn/interrupt`。
- 将 `AgentEvent` schema 从 Codex 专用表面抽成多适配器公共契约。
