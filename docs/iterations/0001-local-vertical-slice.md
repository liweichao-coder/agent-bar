# Iteration 0001: 本地纵向闭环

状态：Implemented and verified

## 目标

把第一代界面从静态模拟原型推进为可运行的 Windows 桌面应用，并建立“React UI -> Tauri command -> Rust repository -> SQLite”的本地数据闭环。

## 本轮完成

- 建立 Tauri 2 原生工程并在 Windows 运行 `agent-bar.exe`。
- 增加统一应用图标及 Windows、macOS、Linux 等尺寸资源。
- 在 Rust 侧创建 SQLite 数据库、索引、事务和版本迁移。
- 从 React 读取和保存当天计划；普通浏览器继续使用模拟数据。
- 使用 Rust 后台线程每 5 秒采集一次 Windows 前台应用。
- 在 Rust 内存中脱敏窗口标题，再写入 SQLite。
- 合并同一应用和窗口在 15 秒内连续到达的活动心跳。
- 增加活动追踪和窗口标题采集两个独立隐私开关。

## 数据边界

前端不能执行任意 SQL，只能调用以下 Tauri 命令：

- `load_local_state`
- `replace_schedule_blocks`
- `set_tracking_enabled`
- `set_capture_window_titles`
- `capture_foreground_activity`

数据库只保存应用名称和脱敏后的窗口标题。默认设置为：

- `tracking_enabled = false`
- `capture_window_titles = false`

直接开启活动追踪时只记录应用名称；用户明确批准窗口标题读取后，才记录本地脱敏标题。

## 数据库 v2

- `schedule_blocks`: 按日期保存计划时间块。
- `activity_records`: 保存前台活动时间区间。
- `settings`: 保存隐私与采集设置。
- `PRAGMA user_version = 2`。

## 验证证据

- `npm run build`: React/TypeScript 生产构建通过。
- `cargo check`: Tauri、SQLite 和 Windows API 编译通过。
- `cargo test`: 4 项测试通过，覆盖脱敏、长度限制、计划与设置持久化、活动心跳合并。
- `npm run tauri dev`: 启动 `agent-bar.exe`，窗口标题正确且进程响应正常。
- 真实应用数据库中存在当天 8 个计划块。
- v2 迁移后活动表为空，两个采集开关均为 `false`。
- 浏览器降级模式保留 8 个计划、5 个模拟活动和 4 个 Agent，控制台无错误。

## 仍未完成

- 真实 Codex 事件适配器和批准控制。
- 托盘、贴边 Bar、透明置顶和点击穿透技术验证。
- 活动分类修正界面、排除应用列表和数据保留策略。
- 系统日历、Notion、飞书和微信截图导入。
