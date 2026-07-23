# Iteration 0013: Windows Idle and Session Tracking

状态：Implemented and verified on an active Windows session; lock, disconnect, sleep and resume still need native scenario probes

## 目标

避免用户离开电脑、锁屏或远程会话断开后，前台应用仍被持续计入“今日投入”，同时让桌面状态条明确说明当前为什么没有写活动记录。

## 本轮完成

- 使用 Windows `GetLastInputInfo` 读取当前会话最后一次键鼠输入时间，不读取按键、鼠标位置或输入内容。
- 使用 `WTSQuerySessionInformationW(WTSSessionInfoEx)` 读取当前会话的 active、locked、unlocked 和 disconnected 状态。
- 新增统一采集状态：`active`、`idle`、`locked`、`disconnected`、`paused` 和 `unavailable`。
- 后台 5 秒采样循环只有在 `captureAllowed=true` 时才读取前台窗口并写入 SQLite。
- 锁屏或会话断开后，最迟在下一次 5 秒采样时停止活动心跳。
- 空闲状态到达阈值后停止心跳；恢复输入后重新建立活动段，不跨越空闲间隔合并。
- 空闲阈值持久化到 SQLite，后端允许 1 至 60 分钟，界面提供 1、3、5、10、15 和 30 分钟选项，默认 5 分钟。
- 顶部状态条和“今日投入”区分本地记录中、空闲暂停、锁屏暂停、会话断开、手动暂停和采集不可用。
- WTS 查询不可用时退化为输入空闲判断，并把 `sessionStateAvailable=false` 暴露给前端。
- `GetLastInputInfo` 本身失败时采用失败关闭策略，本轮不写活动记录，避免把无法确认的时间算作投入。
- 对 32 位 Windows tick 回绕做 `wrapping_sub`，对疑似反向或异常的大差值按 0 处理，避免误判为超长空闲。

## 数据不变量

1. 键鼠事件内容、按键、坐标和最后输入时间都不写入 SQLite；只在内存中计算空闲秒数。
2. `tracking_enabled=false` 时状态固定为 `paused`，无论系统会话是否活跃都不采集。
3. `locked` 和 `disconnected` 优先级高于空闲阈值，不能因近期有输入而继续采集。
4. 达到空闲阈值时不再生成前台窗口快照，已有记录停在最后一次有效心跳。
5. 相同应用与标题只有在心跳间隔不超过 15 秒时合并；空闲恢复不会连接空闲前后的记录。
6. WTS 不可用时仍可依赖空闲阈值，且界面可识别会话状态信号不可用。
7. 阈值必须在 1 至 60 分钟之间，数据库读取到异常旧值时会夹紧到合法范围。

## Windows API 依据

- [GetLastInputInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo)：返回调用进程所在会话的最后输入 tick，官方明确将其用于空闲检测，并提醒 tick 不保证单调。
- [LASTINPUTINFO](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-lastinputinfo)：调用前必须设置 `cbSize`，`dwTime` 保存最后输入事件的 tick。
- [WTSQuerySessionInformationW](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsquerysessioninformationw)：以 `WTS_CURRENT_SESSION` 查询本地当前会话，并使用 `WTSFreeMemory` 释放返回缓冲区。
- [WTSINFOEX_LEVEL1_W](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ns-wtsapi32-wtsinfoex_level1_w)：`SessionFlags=0` 表示锁定，`1` 表示解锁；Windows 7 / Server 2008 R2 存在官方记录的反转缺陷。

## 验证证据

- TypeScript 生产构建通过，Node 测试 18 项通过。
- Rust 测试 31 项通过，2 项真实 Codex 登录测试忽略。
- 新增测试覆盖阈值前后边界、锁屏优先、WTS 未知状态降级、tick 回绕、异常反向 tick，以及 SQLite 阈值校验与持久化。
- 当前 Windows 真实读取探针通过：状态为 `active`，空闲 2 秒，`sessionStateAvailable=true`。
- `cargo clippy --all-targets --all-features -- -D warnings` 通过。
- 1280x900 验证：顶部显示“空闲自动暂停”，状态文字和操作按钮不重叠，页面无横向溢出。
- 390x844 验证：隐私设置完整显示阈值控件，弹窗无横向溢出，固定操作区可用。
- 浏览器交互验证：空闲阈值可从 5 分钟切换为 10 分钟。
- 浏览器控制台无 warning 或 error。

## 语义说明

- 空闲阈值是阅读、思考和短暂离开座位的缓冲期，因此最后一次输入后的这段阈值时间仍计入当前应用。
- 锁屏、会话断开和采集 API 失败不使用该缓冲，后台会在下一次采样时停止写入。
- 本轮通过“活动记录之间的空白”表达空闲，没有新增一个伪造的 Idle 应用，也没有把空闲时长混入四类投入。

## 当前限制

- 当前原生探针只在活跃 Windows 11 会话验证；锁屏、RDP 断开、睡眠和恢复需要逐场景测试。
- Windows 7 和 Server 2008 R2 的 WTS 锁定标志存在已知反转，本项目目前不把这两个旧系统作为支持目标。
- 只根据键盘和鼠标判断空闲；观看视频、演示、长时间阅读或语音会议可能在阈值后自动暂停，可通过提高阈值缓解。
- 没有保存独立空闲区间，因此暂时不能统计“离席多少时间”或在时间轴上显示空闲块。
- 采样周期固定为 5 秒，尚未处理电源广播、系统时间跳变或 Tauri 生命周期事件。

## 下一步

- 增加可重复的锁屏、解锁、RDP 断开、睡眠和恢复原生探针，并验证活动段不会跨状态合并。
- 评估使用 Windows 会话通知替代或补充 5 秒 WTS 轮询。
- 增加可选的空闲区间统计，但继续与有效投入分开存储和展示。
- 继续日历账号直连与多显示器/DPI 原生验证。
