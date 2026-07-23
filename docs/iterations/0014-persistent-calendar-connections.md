# Iteration 0014: Persistent Calendar Connections

状态：Implemented and verified for Windows local-file sync and connection management; live provider endpoints and OAuth adapters remain unverified

## 目标

把“手动导入一次 `.ics`”推进为可持续使用的日历来源：用户连接一次后，Agent Bar 能在启动和运行期间按间隔读取更新，同时确保私有订阅 URL、本地路径和其他来源的日程不会互相污染。

本轮不假装已经完成 Google、Microsoft 或飞书账号授权。先建立 provider-neutral 的连接、凭据、同步和 UI 边界；OAuth 后续只负责取得访问凭据和事件数据，不需要重做时间轴或来源隔离模型。

## 本轮完成

- 新增 `calendar_connections` SQLite 表，保存显示名、`local-file` / `ics-subscription` 类型、脱敏来源提示、启用状态、15 至 1440 分钟同步间隔和最后同步状态。
- 完整本地路径与私有 ICS URL 使用 Windows Credential Manager 的 `CRED_TYPE_GENERIC` 保存，不写入 SQLite。
- 使用 Tauri Dialog 插件选择本地 `.ics` 文件；前端不读取文件内容，也不持久化路径。
- 私有订阅只接受 HTTPS，拒绝 URL 用户名/密码、fragment、本机、私网、链路本地、文档和保留 IP。
- 域名解析后检查所有地址，再把请求固定到通过校验的公网地址，降低 DNS 重绑定带来的 SSRF 风险。
- HTTP 客户端禁用自动重定向，设置 10 秒超时、2 MB 响应上限和固定 User-Agent；网络错误移除 URL 后才进入同步状态。
- 复用现有 ICS 解析器处理时区、全天事件、重复事件、取消事件和稳定事件 ID。
- 每个连接使用 `calendar:<connection-id>` 作为来源，并在 SQLite 事务中只删除和替换自己的当天日程。
- 同步失败只更新错误状态，保留上一次成功日程；删除连接时同步移除其全部日程和系统凭据。
- WebView 启动后立即检查到期连接，此后每 60 秒检查一次；每个连接按自己的同步间隔决定是否执行。
- 日历窗口分为“持续连接”和“导入一次”，支持脱敏来源列表、最后同步状态、立即同步、暂停/继续、二次点击删除和新增连接。
- 私有 URL 输入默认遮蔽并提供显隐按钮；桌面和 390px 窄屏都保留原生日历导入流程。

## 数据与安全不变量

1. `calendar_connections` 不出现完整 URL、查询令牌或本地路径，只保存域名或文件名提示。
2. 不提供明文 SQLite 降级路径；非 Windows 平台在安全凭据适配器完成前拒绝创建持续连接。
3. 网络连接必须是 HTTPS，且不能自动跟随重定向，避免私有 URL 中的 bearer-like token 被转发到另一主机。
4. DNS 返回任意非公网地址时整次请求失败；请求使用已验证地址，TLS 仍以原域名校验。
5. 单个响应或文件最多 2 MB，ICS 最多解析 5000 个原始事件和 500 个当天预览事件。
6. 连接同步只能替换 `source=calendar:<id>` 的块，不能删除手工、Agent、一次性导入或其他连接的数据。
7. 远端失败、解析失败或凭据丢失时不清空旧日程；用户仍能看到上一次成功结果和明确错误。
8. 删除元数据失败时尽力恢复刚删除的系统凭据，降低跨存储操作出现半完成状态的概率。

## 技术依据

- [Microsoft CREDENTIALW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw)：`CRED_TYPE_GENERIC` 由应用定义内容并由 Windows 安全存储，blob 上限足以容纳受限 URL 或路径。
- [Microsoft CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)、[CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw) 与 [CredDeleteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-creddeletew)：凭据与当前用户登录会话的凭据集关联，读取结果使用 `CredFree` 释放。
- [Tauri Dialog plugin](https://v2.tauri.app/plugin/dialog/)：桌面端原生文件选择返回文件系统路径，并通过显式 capability 授权前端调用。
- [reqwest blocking client](https://docs.rs/reqwest/0.12/reqwest/blocking/)：Rust 后端执行有界 HTTPS 请求；同步命令放入 Tauri blocking worker，避免阻塞异步命令执行器。

## 验证证据

- TypeScript 生产构建通过，Node 测试 18 项通过。
- Rust 常规测试 36 项通过，4 项真实环境测试忽略；新增覆盖 HTTPS/私网 URL 校验、脱敏来源提示和来源隔离替换。
- Windows Credential Manager 真实写入、读取和删除往返测试通过，测试凭据已清理。
- Windows 本地文件端到端测试通过：创建连接、读取凭据、解析 UTC 事件、换算为 Asia/Shanghai、写入锁定日程、删除连接及清理凭据。
- `cargo clippy --all-targets --all-features -- -D warnings` 通过。
- `tauri build --debug --no-bundle` 通过，生成 `src-tauri/target/debug/agent-bar.exe`。
- 1280x820 浏览器验证：双栏连接管理、脱敏来源、状态和三项操作完整显示；修复列表横向滚动。
- 390x844 浏览器验证：连接列表与新增表单改为单栏，弹窗自身 `scrollWidth == clientWidth`，无内部横向溢出。
- 浏览器交互验证：持续连接/导入一次切换、暂停/继续、二次删除确认、新增私有 URL 连接均可操作；列表只显示 `outlook.office365.com`，不显示测试查询令牌。

## 当前限制

- 目前不是 OAuth 账号直连；Google、Outlook 和飞书需要用户提供各自导出、发布或订阅得到的 ICS 地址。
- 未使用真实 Google、Microsoft 或飞书私有订阅端点做联网测试，可能遇到需要认证、重定向、非 UTF-8 或供应商扩展字段的兼容问题。
- 自动检查由 WebView 定时器触发；窗口长期隐藏时系统可能节流定时器，当前保证启动和可见运行期间同步，尚未做到完全独立的 Rust 后台调度。
- 只同步当前本地日期；周视图暂未预取未来七天的连接事件。
- Windows Credential Manager 是当前唯一安全凭据后端；macOS Keychain、Linux Secret Service 和 Tauri Stronghold 适配器尚未实现。
- 连接只读，不会在远端创建、编辑或删除事件，也没有 ETag / Last-Modified 条件请求。

## 下一步

- 把同步调度下沉到 Rust 后台，并持久化查看时区，保证托盘隐藏期间按时运行。
- 同步今天至未来七天，周视图直接读取真实日历块。
- 为 Google Calendar、Microsoft Graph 和飞书分别实现 OAuth adapter，继续复用连接状态和来源隔离事务。
- 增加 ETag、Last-Modified、指数退避和按供应商分类的可恢复错误。
- 继续多显示器、DPI、睡眠恢复和锁屏场景验证。
