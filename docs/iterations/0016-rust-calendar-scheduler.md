# Iteration 0016: Rust Calendar Scheduler

状态：Implemented and verified for tray-lifecycle background polling, timezone-aware week selection, native event delivery, and serialized connection mutations

## 目标

让持续日历连接成为桌面应用能力，而不是某个 React 页面里的定时器。Agent Bar 进程仍在托盘运行时，日历应继续按连接间隔同步；午夜跨日和跨周后，界面也应收到新日期对应的整周快照。

本轮不承诺系统休眠期间执行代码。目标是移除 WebView 轮询依赖，并建立后续处理唤醒、网络恢复和 OAuth token 刷新的原生调度边界。

## 本轮完成

- 新增独立 `calendar-sync` Rust 线程，使用容量为 1 的唤醒通道合并重复请求，并每 60 秒执行一次到期检查。
- React 注册 `calendar-sync-updated` 和 `calendar-sync-failed` 监听后，提交浏览器解析出的 IANA 时区；Rust 校验并持久化该时区，再立即唤醒调度器。
- 调度器使用 UTC 当前时刻和持久时区计算本地今天及周一至周日，不依赖 WebView 传入日期，也能正确跨 UTC 午夜。
- 每轮即使没有连接到期，也返回当前日和整周 SQLite 快照，使工作台在午夜后切换到新一天。
- 删除前端 60 秒 `setInterval` 日历同步；窗口隐藏到托盘时，Rust 线程和 WebView 事件监听仍属于同一应用进程。
- 后台周期和用户“立即同步”共享 `Mutex` 同步门，避免两个网络请求同时替换同一连接来源。
- 暂停和删除连接也通过同步门执行；操作返回后，不会有更早开始的同步迟到写回已停用或已删除来源。
- 重新启用连接会主动唤醒调度器，不必等待下一个 60 秒周期。
- 后台同步批次继续沿用 0015 的整周事务、来源隔离、私有地址防护和错误脱敏规则。

## 生命周期与并发不变量

1. 未配置有效 IANA 时区时，调度器不猜测日期，也不执行同步。
2. 时区由前端只提交名称，SQLite 不保存浏览器区域设置或其他环境信息。
3. 同一进程内最多一个日历同步或连接删除/停用临界区在执行。
4. 删除命令取得同步门后才删除凭据、连接元数据和来源日程；命令成功返回后不会再出现该来源。
5. 后台事件是界面刷新信号，SQLite 仍是事实来源；事件监听暂时不可用不会损坏同步结果。
6. 线程只在 Agent Bar 进程存活时工作，应用完全退出后不会留下单独服务或计划任务。

## 验证证据

- Rust 常规测试 41 项通过，5 项真实环境测试忽略；新增覆盖无效时区、UTC 午夜换日和持久时区驱动的完整同步周期。
- 单独执行 Windows 后台本地文件测试通过：临时 ICS 写入 Credential Manager 连接，经调度周期生成七天快照并命中正确日期，随后连接、文件和凭据全部清理。
- `cargo fmt --check` 和 `cargo clippy --all-targets --all-features -- -D warnings` 通过。
- Node 测试 21 项通过，TypeScript/Vite 生产构建通过。
- `tauri build --debug --no-bundle` 通过，生成 `src-tauri/target/debug/agent-bar.exe`。
- 原生可执行文件探针成功启动并保持运行；WebView 初始化后 SQLite 中 `calendar_timezone` 为 `Asia/Shanghai`，证明前端配置命令已抵达 Rust，测试进程随后正常清理。
- Windows Credential Manager 检查未发现 `AgentBar.CalendarConnection` 测试凭据残留。

## 当前限制

- 60 秒轮询在系统睡眠期间不会运行；恢复后最迟在下一个周期检查，没有单独监听 Windows power/session resume 事件。
- 后台线程只随 Agent Bar 进程运行，没有注册 Windows 服务或系统计划任务。
- 当前没有 ETag、Last-Modified、指数退避或网络恢复抖动控制；连接自己的刷新间隔仍是主要限流边界。
- Tauri 事件没有持久队列；若 WebView 被真正销毁而非隐藏，重建后需要从 SQLite 加载最新快照。
- 尚未实现 OAuth token 刷新、远端写入和供应商增量同步。

## 下一步

- 监听 Windows 睡眠恢复与网络恢复，在恢复后合并触发一次到期检查。
- 为连接增加 ETag、Last-Modified、连续失败次数和有上限的指数退避。
- 增加可观察的调度器健康状态，包括最近检查时间、下次预计同步和后台错误。
- 选择一个实际使用的供应商实现 OAuth adapter，并复用当前同步门和事件协议。
