# Similar Project Notes

调研日期：2026-05-26

## ActivityWatch

定位：开源、跨平台、本地优先的自动时间追踪工具。

值得借鉴：

- 采用 server + watcher + web UI 的松耦合结构。
- watcher 用 heartbeat 上报当前窗口、AFK、浏览器标签等活动。
- 数据按 bucket 组织，事件带 timestamp/duration/data，适合做日历和时间线聚合。
- UI 有 dashboard 和 timeline，可作为“每日活动可视化”的参考。

不直接照搬：

- 它偏“事后统计”，Agent Bar 更需要“实时计划 + 实时 agent 状态 + 可介入控制”。
- 它的 UI 是完整 dashboard，不是桌面常驻 bar。

## OpenHands

定位：面向软件开发 agent 的平台，包含 SDK、CLI、本地 GUI 和云端 GUI。

值得借鉴：

- 把 agent engine、CLI、GUI 拆开，说明核心 agent 状态和 UI 不应该强耦合。
- 本地 GUI 提供 REST API + React 单页应用，这对 Agent Bar 的“本地 daemon + desktop UI”很有启发。
- 适合作为“agent 在做什么”的行为可视化参考：任务、终端、浏览器、代码修改、日志流。

不直接照搬：

- OpenHands 聚焦 coding agent，Agent Bar 应该更泛化：日程、学习、资料整理、科研工作流、桌面自动化都要能接入。

## Langfuse

定位：开源 LLM engineering/observability 平台，核心是 trace、prompt、eval、dataset、metrics。

值得借鉴：

- 对 agent action、retrieval、embedding、LLM call 做 trace，适合解释“为什么 agent 卡住/失败/变慢”。
- 生态接入面广，说明兼容 OpenTelemetry/SDK 风格事件会有长期价值。

不直接照搬：

- 它是面向团队和服务端的观测平台，Agent Bar 更偏个人桌面实时感知。
- 对普通用户而言 trace tree 信息密度过高，需要转译为更自然的状态卡片和时间轴。

## Flowise

定位：可视化构建 AI agent / LLM workflow 的低代码平台。

值得借鉴：

- React + Node/TypeScript 生态，节点式 UI 对 agent 编排友好。
- components/nodes 的插件化接入模式适合参考。

不直接照搬：

- Agent Bar 的首屏不是 workflow builder，而是“状态条 + 当天安排 + 实时活动”。
- 节点编辑器可以作为高级面板，不应该压过常驻 bar 的轻量体验。

## Marvis

定位：腾讯的操作系统级 AI 助手，预置负责调度、文件、系统、应用、浏览器和搜索的多个 Agent。

值得借鉴：

- 用一个简化的虚拟办公室表达多 Agent 分工，角色在空闲、工作和等待时有不同动作。
- 将 Agent 抽象为容易理解的岗位，而不是直接向普通用户展示 trace 和日志。
- 办公室旁同时显示任务进度、运行状态和消耗信息，让动画与真实工作状态有关联。
- 涉及系统修改等操作时保留人工确认。

不直接照搬：

- 公开体验资料显示办公室更偏状态展示，交互和控制能力有限。
- 固定的六种 Agent 角色适合开箱即用产品，但 Agent Bar 需要支持外部 Agent 动态接入。
- Agent Bar 不应承担完整的操作系统助手能力，时间管理与可观察调度才是主线。

## Pixel Agents

定位：将 Claude Code 等终端 Agent 映射成像素办公室角色的开源 VS Code 扩展和独立 Web 应用。

值得借鉴：

- 已实现角色行走、坐到工位、阅读、输入、等待授权、完成提示和子 Agent 可视化。
- 使用 Claude Code Hooks 获取 Session、Tool、Permission 和 Subagent 事件，并提供 JSONL 轮询降级方案。
- 用统一 AgentEvent、Provider adapter、HTTP/WebSocket 服务隔离具体 Agent 与可视化层。
- 前端采用 React + Canvas 2D，游戏状态由独立状态机管理，并使用 BFS 寻路。
- 证明 2D 虚拟办公室不需要 3D 引擎也能有效表达 Agent 活动。

需要补足：

- 当前主要是观察 Claude Code，Codex 等适配仍在路线图中。
- 状态推断在缺少官方 Hook 时会误判，Agent Bar 需要显式展示数据可信度和连接能力。
- 它没有日程、时间统计和人的活动轨道，这正是 Agent Bar 的主要差异。

## Early Conclusion

Agent Bar 最像三类系统的交叉：

- ActivityWatch: 时间/活动事件模型。
- OpenHands: agent 运行过程可视化。
- Langfuse/Flowise: agent 接入、trace、编排和生态。

因此第一版应避免一开始做成“大而全 agent 平台”。更稳的 MVP 是：

1. 一个桌面常驻 bar。
2. 一个 24h timeline。
3. 一个本地 ingestion API。
4. 一个 agent activity stream。
5. 一个可展开详情面板，用来查看 trace、任务、产物和人工介入点。

## References

- ActivityWatch architecture: https://docs.activitywatch.net/en/latest/architecture.html
- ActivityWatch data model: https://docs.activitywatch.net/en/latest/buckets-and-events.html
- OpenHands repository: https://github.com/OpenHands/OpenHands
- Langfuse repository: https://github.com/langfuse/langfuse
- Flowise repository: https://github.com/FlowiseAI/Flowise
- Marvis website: https://marvis.qq.com/
- Pixel Agents repository: https://github.com/pixel-agents-hq/pixel-agents
- Tauri architecture: https://v2.tauri.app/concept/architecture/
- Electron BrowserWindow API: https://www.electronjs.org/docs/latest/api/browser-window
