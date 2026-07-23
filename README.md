# Agent Bar

Agent Bar 是一个面向桌面常驻的 agent 工作可视化与时间安排可视化项目。

目标不是先做一个聊天窗口，而是做一个能贴近日常工作流的桌面状态条：

- 用 24 小时时间轴展示今日/本周安排、计划块、会议、学习和深度工作时段。
- 用实时活动流展示正在运行的 agent、任务状态、工具调用、产物和异常。
- 提供开放接入协议，让不同 agent 可以把状态、计划、行动和结果写入同一个可视层。
- 长期目标是让人能一眼看懂“今天要做什么、agent 正在帮我做什么、下一步该不该介入”。

## Current Stage

当前仓库已进入第一代本地纵向闭环迭代。React 工作台、Tauri 桌面壳、SQLite 持久化、Windows 前台活动与空闲/会话状态采集、真实日/周时间统计、隐私与数据生命周期控制、ICS 日历导入与 Rust 后台整周同步、截图事项提取、晨间全天规划与每日提醒、只读 Codex 观察器、精确 Hooks 事件和受 Agent Bar 托管的 Codex 控制链路已经建立。

## Development Timeline

- `2026.05`：完成项目立项、相近产品与开源仓库调研、需求边界和技术选型。
- `2026.07`：完成第一代本地纵向原型与 16 个可验证迭代，覆盖时间规划、活动统计、日历同步、Codex 接入和 Windows 桌面状态条。

时间线来自仓库中的真实提交与迭代记录，不回填或改写 Git 历史。

## Prototype v0.1

目前可以体验：

- 桌面顶部状态条：当前时间、今日进度、当前工作、下一事项、记录状态和待批准数量。
- 今日时间轴：在同一时间坐标中对照计划和前台应用活动。
- 周视图：在可横向滚动的 24 小时坐标中查看七天安排，并对照一周专注、会议和生活时间结构。
- 规划建议：把 Agent 建议加入当天时间轴。
- Agent 办公室：Canvas 像素角色展示工作、检索、等待和空闲状态。
- Agent 控制：模拟批准、拒绝、暂停、继续、终止和移交入口。
- 原生模式可在授权后读取本机 Codex 任务名称、来源、更新时间和运行状态元数据。
- Codex 旁路任务只展示真实可用的观察能力，不显示虚假的暂停或终止按钮。
- 专注模式与窄屏响应式布局。
- Tauri 模式把当天计划持久化到 SQLite。
- Rust 后台线程每 5 秒采集一次 Windows 前台应用，窗口标题先脱敏再落库。
- 活动追踪和窗口标题采集默认关闭，分别由用户开启和批准。
- 隐私设置支持排除敏感应用、7 至 365 天保留期限和二次确认清空活动历史。
- 排除应用后会停止后续采集，并立即删除该应用已有的活动记录。
- Codex 元数据观察默认关闭，不读取消息正文或思维链。
- 原生模式可由用户明确提交任务，启动一个临时 Codex thread/turn。
- 托管任务使用 `workspace-write + on-request + user review`，命令和文件修改会回到 Agent Bar 请求批准。
- 托管任务可从 Agent 面板终止；任务提示和待审批内容只保存在运行内存，不写入 SQLite。
- 顶部按钮可以在完整工作台和 68px Windows 置顶状态条之间切换。
- 系统托盘可展开工作台、显示顶部状态条、恢复交互或退出；关闭主窗口默认隐藏到托盘。
- 原生模式可导入 Outlook、Google Calendar、Apple Calendar 和飞书等来源导出的 `.ics` 文件。
- 导入前会展开重复事件、换算时区、预览当天安排和标记时间冲突；确认后只合并日历项，不覆盖手动或 Agent 时间块。
- 原始 ICS、地点、描述和参与人不落库，只保存标题、当天时间区间和不可逆事件 ID。
- 原生模式可把本地 `.ics` 文件或私有 HTTPS ICS 地址保存为持续连接，支持 15 分钟至每天的同步间隔、立即同步、暂停和删除。
- 完整订阅 URL 与本地路径保存到 Windows Credential Manager；SQLite 只保存显示名、类型、脱敏域名或文件名、同步状态和时间。
- 每个连接一次读取后解析本周七天，并在一个 SQLite 事务中只替换自己来源的锁定日程；任一天失败都会保留整周上一次成功结果，不影响手工、Agent 或其他日历来源。
- Tauri 启动独立 Rust 调度线程，每 60 秒检查到期连接；窗口隐藏到托盘后仍会同步，并通过事件把整周快照推回工作台。
- 查看时区使用 IANA 名称持久化；后台同步、立即同步、暂停和删除共享串行同步门，避免迟到写入恢复已停用或已删除的来源。
- HTTPS 订阅禁用重定向，限制 10 秒和 2 MB，并拒绝用户名密码、本机、私网、链路本地和保留地址；DNS 解析通过校验后固定到已验证公网地址。
- 晨间规划可编辑待办时长、优先级、偏好时段、类别、全天范围和事项切换缓冲。
- 本地排程器保留已有安排，优先放置重要任务；无连续空档的任务明确留在待办，不制造时间冲突。
- 全天草案经确认后，以单个 SQLite 事务同步更新时间轴和剩余待办。
- 每日晨间提醒支持配置时间、延后 15/30/60 分钟和今天忽略；确认全天安排后以同一事务记录当日已规划。
- 今日投入和周视图由 SQLite 活动记录与计划动态聚合，支持跨午夜裁剪、重叠计划去重、四类时间去向和 Top Apps。
- 原生模式可选择 PNG、JPEG 或 WebP 截图，由只读临时 Codex thread 提取结构化日程事项；原图在返回、失败或终止后删除。
- 截图结果会先进入可编辑确认界面，再合并到待办并交给晨间规划器，不直接改写时间轴。
- Windows 原生模式使用最后输入时间和 WTS 会话状态，在空闲、锁屏或会话断开时停止活动心跳。
- 空闲阈值默认 5 分钟并可在隐私设置中调整；状态条会说明正在记录、空闲暂停、锁屏暂停或手动暂停。

## Local Development

```powershell
npm install
npm run dev
```

浏览器访问 `http://127.0.0.1:1420/`。生产前端构建使用：

```powershell
npm run build
```

安装 Rust 和 Windows 桌面编译依赖后，使用：

```powershell
npm run tauri dev
```

Tauri 模式的数据保存在系统应用数据目录下的 `agent-bar.sqlite3`。浏览器模式不会读取桌面窗口，也不会写入该数据库，而是保留模拟数据用于界面开发。

## Research Notes

- [docs/research-notes.md](docs/research-notes.md): 相近开源项目观察。
- [docs/architecture-sketch.md](docs/architecture-sketch.md): 初步架构草图和技术候选。
- [docs/CURRENT_STATE.md](docs/CURRENT_STATE.md): 当前已实现能力、验证基线、边界与下一迭代。
- [docs/iterations/0001-local-vertical-slice.md](docs/iterations/0001-local-vertical-slice.md): 第一代本地纵向闭环实现与验证。
- [docs/iterations/0002-codex-observer.md](docs/iterations/0002-codex-observer.md): 真实 Codex 任务观察器实现与验证。
- [docs/iterations/0003-codex-hook-events.md](docs/iterations/0003-codex-hook-events.md): 隐私最小化 Hooks 事件与 SQLite 状态覆盖。
- [docs/iterations/0004-managed-codex-run.md](docs/iterations/0004-managed-codex-run.md): 长连接 App Server、显式任务启动、审批和终止控制。
- [docs/iterations/0005-activity-privacy-controls.md](docs/iterations/0005-activity-privacy-controls.md): 应用排除、保留期限、自动清理和手动清空。
- [docs/iterations/0006-windows-bar-spike.md](docs/iterations/0006-windows-bar-spike.md): Windows 紧凑状态条、置顶和点击穿透技术验证。
- [docs/iterations/0007-tray-recovery.md](docs/iterations/0007-tray-recovery.md): 托盘菜单、窗口模式持久化和点击穿透恢复路径。
- [docs/iterations/0008-ics-calendar-import.md](docs/iterations/0008-ics-calendar-import.md): ICS 时区与重复事件解析、冲突预览和确认合并。
- [docs/iterations/0009-morning-planner.md](docs/iterations/0009-morning-planner.md): 待办队列、确定性全天排程、草案预览和原子确认。
- [docs/iterations/0010-morning-reminder.md](docs/iterations/0010-morning-reminder.md): 每日提醒状态机、延后/忽略、持久化和当日已规划标记。
- [docs/iterations/0011-real-time-statistics.md](docs/iterations/0011-real-time-statistics.md): SQLite 日/周聚合、跨日裁剪、类别统计和 Top Apps。
- [docs/iterations/0012-screenshot-planner-inbox.md](docs/iterations/0012-screenshot-planner-inbox.md): Codex 图片输入、动态工具结构化回传、临时图片生命周期和校对后规划。
- [docs/iterations/0013-windows-idle-session-tracking.md](docs/iterations/0013-windows-idle-session-tracking.md): Windows 最后输入与 WTS 会话状态、空闲阈值、失败降级和真实时间统计边界。
- [docs/iterations/0014-persistent-calendar-connections.md](docs/iterations/0014-persistent-calendar-connections.md): Windows 安全凭据、私有 ICS/本地文件持续连接、来源隔离同步和响应式连接管理界面。
- [docs/iterations/0015-week-calendar-schedule.md](docs/iterations/0015-week-calendar-schedule.md): 有界整周日历同步、跨日原子替换、24 小时周安排和重叠布局。
- [docs/iterations/0016-rust-calendar-scheduler.md](docs/iterations/0016-rust-calendar-scheduler.md): Rust 后台调度、持久时区、Tauri 事件通知和连接并发边界。

## Selected Direction

MVP 技术基线：

- 桌面壳：Tauri 2 + React/TypeScript。
- 前端状态：Zustand 或 TanStack Store。
- 时间轴：React DOM + SVG 自研 24h/weekly timeline，数据模型参考 ActivityWatch 的 event/bucket/heartbeat 思路。
- Agent 可视化：Canvas 2D 像素办公室 + 事件流 + 当前任务面板；复杂度增长后再升级 PixiJS。
- 本地存储：SQLite，作为计划、实际活动和 Agent 事件的本地事实来源。
- Codex 接入：App Server 负责结构化任务与托管控制，Hooks 负责外部会话的精确生命周期旁路上报。
- 通用接入协议：本地事件入口标准化为 `AgentEvent`，后续兼容 Claude Code Hooks、OpenTelemetry 和 MCP 工具边界。
- 日历接入：RFC 5545 ICS 已覆盖一次性导入、私有 URL/本地文件持续连接、本周七天同步和托盘生命周期内的 Rust 后台调度；后续以同一连接模型增加 Microsoft Graph、Google Calendar 和飞书 OAuth 账号直连。
- 晨间规划：确定性本地排程负责无冲突时间约束，持久提醒负责每日入口，Codex 后续负责事项理解、优先级建议和排程解释，最终仍需用户确认。
- 截图收件箱：Codex 视觉理解只提取结构化事项，本地 Rust 校验输出并清理原图，用户校对后再交给确定性排程器。
- 活动可信度：Windows 最后输入时间负责可配置空闲判断，WTS 会话状态负责锁屏和断开检测；两者只控制是否生成活动心跳，不保存原始输入信号。

Tauri 已在单显示器 100% DPI 环境通过贴顶、置顶、68px 客户区、点击穿透和托盘恢复探针；多显示器、非 100% DPI、任务栏位置变化、系统退出和休眠恢复仍是切换 Electron 前的决策门槛。

## Repo Status

The first interactive prototype, native local persistence slice, read-only Codex observer, opt-in lifecycle hooks, and a managed Codex run with approval and interrupt controls are implemented.
